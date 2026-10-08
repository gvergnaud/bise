// The C5 server: a unix socket (mode 0600), JSON lines. The helper speaks
// first (hello); requests run on one serial queue per target, so two
// agents on two apps run side by side; control lines act at once.
import AppKit
import CUCore

final class Conn {
    let fd: Int32
    private let wlock = NSLock()
    private(set) var open = true
    init(fd: Int32) { self.fd = fd }

    func send(_ obj: [String: Any]) {
        guard var data = try? JSONSerialization.data(withJSONObject: obj, options: [.withoutEscapingSlashes]) else { return }
        data.append(0x0A)
        wlock.lock(); defer { wlock.unlock() }
        guard open else { return }
        data.withUnsafeBytes { (buf: UnsafeRawBufferPointer) in
            var off = 0
            while off < buf.count {
                let n = Darwin.write(fd, buf.baseAddress! + off, buf.count - off)
                if n <= 0 { if errno == EINTR { continue }; open = false; return }
                off += n
            }
        }
    }
    func close() { wlock.lock(); open = false; wlock.unlock(); Darwin.close(fd) }
}

final class Server {
    let path: String
    private var listenFD: Int32 = -1
    private let lock = NSLock()
    private var conns: [Int32: Conn] = [:]
    let engine = Engine.shared

    init(path: String) { self.path = path }

    static func unixAddr(_ path: String) -> sockaddr_un? {
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: addr.sun_path) else { return nil }
        withUnsafeMutableBytes(of: &addr.sun_path) { p in
            for (i, b) in bytes.enumerated() { p[i] = b }
            p[bytes.count] = 0
        }
        return addr
    }

    /// Another helper already answers on this path.
    static func alive(_ path: String) -> Bool {
        guard var addr = unixAddr(path) else { return false }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        defer { Darwin.close(fd) }
        return withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        } == 0
    }

    func start() throws {
        let dir = (path as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        guard var addr = Server.unixAddr(path) else { throw CUError("bad_args", "socket path too long: \(path)") }
        unlink(path)
        listenFD = socket(AF_UNIX, SOCK_STREAM, 0)
        let old = umask(0o177)
        let r = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(listenFD, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        umask(old)
        guard r == 0 else { throw CUError("bad_args", "bind \(path): \(String(cString: strerror(errno)))") }
        chmod(path, 0o600)
        guard listen(listenFD, 16) == 0 else { throw CUError("bad_args", "listen: \(String(cString: strerror(errno)))") }
        engine.emit = { [weak self] ev in self?.broadcast(ev) }
        Thread.detachNewThread { [weak self] in self?.acceptLoop() }
    }

    func broadcast(_ obj: [String: Any]) {
        lock.lock(); let all = Array(conns.values); lock.unlock()
        for c in all { c.send(obj) }
    }

    private func acceptLoop() {
        while true {
            let fd = accept(listenFD, nil, nil)
            if fd < 0 { if errno == EINTR || errno == ECONNABORTED { continue }; return }
            var on: Int32 = 1
            setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
            let c = Conn(fd: fd)
            lock.lock(); conns[fd] = c; lock.unlock()
            Thread.detachNewThread { [weak self] in self?.serve(c) }
        }
    }

    private func serve(_ c: Conn) {
        let p = engine.permissions()
        c.send(["hello": ["helper": Bundle.main.bundleIdentifier ?? "dev.bise.computer-use", "version": helperVersion,
                          "accessibility": p["accessibility"] ?? false, "screen_recording": p["screen_recording"] ?? false]])
        var buf = Data()
        var chunk = [UInt8](repeating: 0, count: 65536)
        while true {
            let n = read(c.fd, &chunk, chunk.count)
            if n <= 0 { if n < 0 && errno == EINTR { continue }; break }
            buf.append(contentsOf: chunk[0..<n])
            while let nl = buf.firstIndex(of: 0x0A) {
                let line = buf.subdata(in: buf.startIndex..<nl)
                buf.removeSubrange(buf.startIndex...nl)
                if !line.isEmpty { dispatch(line, c) }
            }
            if buf.count > 16 << 20 { break }   // a broken peer
        }
        lock.lock(); conns[c.fd] = nil; lock.unlock()
        c.close()
    }

    private func dispatch(_ line: Data, _ c: Conn) {
        guard let obj = (try? JSONSerialization.jsonObject(with: line)) as? [String: Any] else {
            c.send(["ok": false, "error": CUError("bad_args", "a line that is not a JSON object").json]); return
        }
        for kind in ["stop", "resume", "release", "drop", "pause"] {
            if let agent = obj[kind] as? String { engine.control(kind, agent: agent); return }
        }
        let args = obj["args"] as? [String: Any] ?? [:]
        let key = args["target"] as? String ?? "_"
        let engine = self.engine
        engine.queue(for: key).async { c.send(engine.handle(obj)) }
    }
}
