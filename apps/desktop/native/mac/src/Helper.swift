// Helper: bise-mac-helper's shell. Electron's main spawns it and talks to
// it in typed lines (Lines.swift, bise_proto::helper): commands on stdin,
// events on stdout, its log on stderr. It owns the fn tap (Keys +
// KeyMachine), the front app's read (FrontRead + FrontRules + Tab),
// macOS's permission states (Perms), quiet moments (QuietMachine +
// QuietMonitor's probes) and the screenshot (Shot). It decides nothing
// about bise: no window, no Dock icon, never active (.prohibited), never
// a prompt unless asked. stdin closed = Electron is gone: it exits.

import AppKit

@MainActor
final class Helper {
    static let version = "0.1.0"

    private let out = FileHandle.standardOutput
    private let keys = Keys()
    private var skip = SkipList.builtIn
    private var quote = true          // his context.quote: fn space may read his selection
    private var quiet = QuietMachine()
    private var permsSaid: [String: String] = [:]
    private var timers: [Timer] = []
    private var buffer = Data()

    // MARK: start

    func start() {
        signal(SIGPIPE, SIG_IGN)
        send(Lines.hello(version: Helper.version, pid: getpid(), responsible: Responsible.me()))
        keys.onOut = { [weak self] o in self?.keyOut(o) }
        sayPerms()
        if AXIsProcessTrusted() { keys.start() }
        readStdin()
        // the permissions every 2 s (cheap reads: he may grant one in System
        // Settings at any time), the quiet probes every 1 s
        timers.append(Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { _ in
            MainActor.assumeIsolated { [weak self] in self?.sayPerms() }
        })
        timers.append(Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { _ in
            MainActor.assumeIsolated { [weak self] in self?.sampleQuiet() }
        })
        let ws = NSWorkspace.shared.notificationCenter
        ws.addObserver(forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main) { n in
            let app = n.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication
            MainActor.assumeIsolated { [weak self] in self?.frontSwitched(app) }
        }
        ws.addObserver(forName: NSWorkspace.didWakeNotification, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { [weak self] in self?.keys.reenable() }
        }
        Log.say("started (pid \(getpid()), parent \(getppid()))")
    }

    // MARK: lines

    private func send(_ ev: [String: Any]) {
        guard let d = (Lines.encode(ev) + "\n").data(using: .utf8) else { return }
        do { try out.write(contentsOf: d) } catch { exit(0) }   // nobody reads us: Electron is gone
    }

    private func readStdin() {
        FileHandle.standardInput.readabilityHandler = { h in
            let d = h.availableData
            DispatchQueue.main.async {
                MainActor.assumeIsolated { [weak self] in
                    if d.isEmpty { Log.say("stdin closed: quit"); exit(0) }
                    self?.take(d)
                }
            }
        }
    }

    private func take(_ d: Data) {
        buffer.append(d)
        while let nl = buffer.firstIndex(of: 0x0A) {
            let line = String(decoding: buffer[buffer.startIndex..<nl], as: UTF8.self)
            buffer.removeSubrange(buffer.startIndex...nl)
            if line.trimmingCharacters(in: .whitespaces).isEmpty { continue }
            switch Lines.decode(line) {
            case .success(let c): run(c)
            case .failure(let e): send(Lines.error(e.why))
            }
        }
    }

    private func run(_ c: HelperCmd) {
        switch c {
        case .config(let ex, let ro, let q):
            skip = SkipList.with(ex)
            keys.rightOption = ro
            quote = q
        case .route(let open):
            keys.routeOpen = open
        case .ownMic(let open):
            quiet.own(open: open, at: uptime)
        case .readContext(let id):
            readFront(id: id, selection: false)
        case .perm(let p, let ask):
            if ask { Perms.ask(p) { [weak self] in self?.sayPerms() } } else { Perms.open(p) }
        case .shot(let id, let dir):
            let skip = self.skip
            Task { @MainActor [weak self] in
                let r = await Shot.take(into: dir, skip: skip)
                self?.send(Lines.shot(id: id, path: r?.path, app: r?.app, title: r?.title, skipped: r?.skipped ?? false))
            }
        case .thumb(let id, let path, let out, let px):
            Task { @MainActor [weak self] in
                async let ok = Thumb.picture(of: path, into: out, px: px)
                async let ms = Thumb.length(of: path)
                let (o, m) = await (ok, ms)
                self?.send(Lines.thumb(id: id, ok: o, ms: m))
            }
        case .quit:
            Log.say("quit")
            exit(0)
        case .unknown(let tag):
            send(Lines.error("unknown cmd \(tag)"))
        }
    }

    // MARK: fn

    private func keyOut(_ o: KeyOut) {
        if let l = KeyLog.line(o) { Log.say(l) }
        send(Lines.fn(o))
        quiet.own(open: keys.talking, at: uptime)
        // the context goes with the talk (read before he speaks) and with fn space; his
        // selection with fn space only (KeyOut.frontAsk)
        if let ask = o.frontAsk(quote: quote) { readFront(id: nil, selection: ask == .appAndSelection) }
    }

    // MARK: the front app

    private var me: Set<pid_t> { [getpid(), getppid()] }

    private func frontSwitched(_ app: NSRunningApplication?) {
        // bise's own windows coming in front keep the last 'in front' row
        guard let app, !me.contains(app.processIdentifier) else { return }
        send(FrontRules.build(FrontRaw(app: app.localizedName, bundle: app.bundleIdentifier), skip: skip).front)
        checkFullScreen()
    }

    private func readFront(id: String?, selection: Bool) {
        let skip = self.skip
        guard let front = NSWorkspace.shared.frontmostApplication, !me.contains(front.processIdentifier) else {
            send(Lines.context(FnCtx(), id: id, bundle: nil))
            return
        }
        let base = FrontRaw(app: front.localizedName, bundle: front.bundleIdentifier)
        let settle: @MainActor (FrontRaw) -> Void = { [weak self] raw in
            let c = FrontRules.build(raw, skip: skip)
            self?.send(Lines.context(c, id: id, bundle: raw.bundle))
            self?.send(c.front)
            Log.say("front: \(c.app ?? "excluded") (\(c.context.keys.sorted().joined(separator: ",")))")
        }
        // never-seen apps: not even read; no Accessibility: the name only
        if SkipList.skips(bundle: base.bundle, title: "", apps: skip) || !AXIsProcessTrusted() { settle(base); return }
        let tab = base.bundle.map { Tab.browsers.contains($0) } ?? false
        let job = FrontJob(raw: base, left: tab ? 2 : 1)
        let done: @MainActor () -> Void = {
            guard !job.sent else { return }
            job.sent = true
            settle(job.raw)
        }
        FrontRead.read(pid: front.processIdentifier, app: base.app, bundle: base.bundle, selection: selection,
                       within: FrontRules.budget - 0.03) { r in
            let url = job.raw.tabURL
            job.raw = r
            job.raw.tabURL = url
            job.left -= 1
            if job.left == 0 { done() }
        }
        if tab, let b = base.bundle {
            Tab.active(b) { url in
                job.raw.tabURL = url
                job.left -= 1
                if job.left == 0 { done() }
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + FrontRules.budget) { MainActor.assumeIsolated { done() } }
    }

    // MARK: permissions

    private func sayPerms() {
        let p = Perms.now()
        let words = p.compactMapValues { $0 as? String }
        guard words != permsSaid else { return }
        let wasTrusted = permsSaid["accessibility"] == "allowed"
        permsSaid = words
        send(p)
        // Accessibility just granted: the tap can start now
        if !wasTrusted && words["accessibility"] == "allowed" && !keys.running { keys.start() }
    }

    // MARK: quiet

    private var uptime: Double { ProcessInfo.processInfo.systemUptime }

    private func sampleQuiet() {
        let t = uptime
        if let m = MicProbe.running() { quiet.mic(running: m, at: t) }
        if let c = CameraProbe.running() { quiet.camera(running: c, at: t) }
        if let s = SharingProbe.present() { quiet.sharing(s) }
        sayQuiet()
    }

    // the front app's front window covers a whole screen (a video, slides, a game)
    private func checkFullScreen() {
        guard let app = NSWorkspace.shared.frontmostApplication, !me.contains(app.processIdentifier),
              let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]]
        else { return }
        let mainH = NSScreen.screens.first?.frame.height ?? 0
        let screens = NSScreen.screens.map { CGRect(x: $0.frame.minX, y: mainH - $0.frame.maxY, width: $0.frame.width, height: $0.frame.height) }
        var full = false
        for w in list where (w[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == app.processIdentifier
            && (w[kCGWindowLayer as String] as? NSNumber)?.intValue == 0 {
            if let bd = w[kCGWindowBounds as String] as? NSDictionary, let b = CGRect(dictionaryRepresentation: bd) {
                full = screens.contains { $0.equalTo(b) }
            }
            break   // the front window only
        }
        quiet.fullScreen(full)
        sayQuiet()
    }

    private func sayQuiet() {
        guard let c = quiet.change() else { return }
        Log.say("quiet \(c.on) \(c.why)")
        send(Lines.quiet(on: c.on, why: c.why))
    }
}

// one read in flight: the AX read and, for a Chromium browser, its tab
@MainActor
final class FrontJob {
    var raw: FrontRaw
    var left: Int
    var sent = false
    init(raw: FrontRaw, left: Int) { self.raw = raw; self.left = left }
}

// the process macOS charges our permissions to (TCC's responsible
// process): read through libquarantine's responsibility_get_pid_responsible_for_pid,
// looked up at run time (missing = we can't tell, never a crash)
enum Responsible {
    private typealias Fn = @convention(c) (pid_t) -> pid_t
    private static let fn: Fn? = {
        guard let p = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "responsibility_get_pid_responsible_for_pid") else { return nil }
        return unsafeBitCast(p, to: Fn.self)
    }()

    static func me() -> (pid: Int32, path: String)? {
        guard let fn else { return nil }
        let pid = fn(getpid())
        guard pid > 0 else { return nil }
        var buf = [CChar](repeating: 0, count: 4096)
        guard proc_pidpath(pid, &buf, UInt32(buf.count)) > 0 else { return (pid, "") }
        return (pid, String(cString: buf))
    }
}
