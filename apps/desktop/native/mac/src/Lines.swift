// Lines: bise-mac-helper's typed lines, pure (bise_proto::helper, the
// Rust side; architect m_9461). Electron's main writes HelperCmd lines on
// the helper's stdin, the helper writes HelperEv lines on its stdout, one
// JSON object per line. The fixtures rust/proto/fixtures/helper_*.jsonl are
// the one source: test/LinesTests.swift decodes every command line and
// builds every event line with the encoders here, so the two sides can't
// drift without a red test. No AppKit here (build.sh --test runs it in the
// swift interpreter).

import Foundation

// what Electron asks (HelperCmd)
enum HelperCmd: Equatable {
    case config(exclude: [String], rightOption: Bool, quote: Bool)
    case route(open: Bool)
    case ownMic(open: Bool)
    case readContext(id: String)
    case perm(String, ask: Bool)           // accessibility | mic | screen; ask, else open
    case shot(id: String, dir: String)
    case thumb(id: String, path: String, out: String, px: Int)
    case quit
    case unknown(String)                   // a newer app's command: said back as an error, never fatal
}

// why a line isn't a command (a bug of the sender): said back as {ev:'error'}
struct LineError: Error, Equatable { let why: String }

enum Lines {
    static let proto = 1
    static let perms: Set<String> = ["accessibility", "mic", "screen"]

    // one line in -> a command, or why it isn't one (a bug of the sender)
    static func decode(_ line: String) -> Result<HelperCmd, LineError> {
        guard let data = line.data(using: .utf8),
              let any = try? JSONSerialization.jsonObject(with: data),
              let o = any as? [String: Any] else { return .failure(LineError(why: "not a JSON object")) }
        guard let tag = o["cmd"] as? String else { return .failure(LineError(why: "no \"cmd\"")) }
        func str(_ k: String) -> String? { o[k] as? String }
        func bool(_ k: String) -> Bool? { (o[k] as? NSNumber).flatMap { CFGetTypeID($0) == CFBooleanGetTypeID() ? $0.boolValue : nil } }
        func bad(_ k: String) -> Result<HelperCmd, LineError> { .failure(LineError(why: "\(tag): bad or missing \"\(k)\"")) }
        switch tag {
        case "config":
            guard let ex = o["exclude"] as? [Any], ex.allSatisfy({ $0 is String }) else { return bad("exclude") }
            guard let ro = bool("right_option") else { return bad("right_option") }
            // quote absent (an older app's line): false, his selection never read
            if o["quote"] != nil && bool("quote") == nil { return bad("quote") }
            return .success(.config(exclude: ex as! [String], rightOption: ro, quote: bool("quote") ?? false))
        case "route":
            guard let v = bool("open") else { return bad("open") }
            return .success(.route(open: v))
        case "own_mic":
            guard let v = bool("open") else { return bad("open") }
            return .success(.ownMic(open: v))
        case "read_context":
            guard let id = str("id") else { return bad("id") }
            return .success(.readContext(id: id))
        case "perm":
            guard let p = str("perm"), perms.contains(p) else { return bad("perm") }
            guard let a = str("action"), a == "ask" || a == "open" else { return bad("action") }
            return .success(.perm(p, ask: a == "ask"))
        case "shot":
            guard let id = str("id") else { return bad("id") }
            guard let dir = str("dir"), !dir.isEmpty else { return bad("dir") }
            return .success(.shot(id: id, dir: dir))
        case "thumb":
            guard let id = str("id") else { return bad("id") }
            guard let path = str("path"), path.hasPrefix("/") else { return bad("path") }
            guard let out = str("out"), out.hasPrefix("/") else { return bad("out") }
            guard let px = (o["px"] as? NSNumber)?.intValue, px > 0, px <= 4096 else { return bad("px") }
            return .success(.thumb(id: id, path: path, out: out, px: px))
        case "quit":
            return .success(.quit)
        default:
            return .success(.unknown(tag))
        }
    }

    // one event out, one line (no newline inside: JSON escapes it)
    static func encode(_ ev: [String: Any]) -> String {
        guard JSONSerialization.isValidJSONObject(ev),
              let d = try? JSONSerialization.data(withJSONObject: ev, options: [.sortedKeys, .withoutEscapingSlashes]),
              let s = String(data: d, encoding: .utf8) else { return #"{"ev":"error","message":"helper: an event didn't encode"}"# }
        return s
    }

    // MARK: the events (HelperEv)

    static func hello(version: String, pid: Int32, responsible: (pid: Int32, path: String)?) -> [String: Any] {
        var e: [String: Any] = ["ev": "hello", "proto": proto, "version": version, "pid": Int(pid)]
        if let r = responsible { e["responsible"] = ["pid": Int(r.pid), "path": r.path] }
        return e
    }

    // the key machine's outputs as {ev:'fn'}
    static func fn(_ o: KeyOut) -> [String: Any] {
        switch o {
        case .talkStart(let k): return ["ev": "fn", "act": "talk_start", "by": k == .fn ? "fn" : "right_option"]
        case .talkEnd: return ["ev": "fn", "act": "talk_end"]
        case .cancel: return ["ev": "fn", "act": "cancel"]
        case .tap: return ["ev": "fn", "act": "tap"]
        case .double: return ["ev": "fn", "act": "double"]
        case .key(let k): return ["ev": "fn", "act": "key", "key": k]
        case .plainEsc: return ["ev": "fn", "act": "esc"]
        }
    }

    // the read's result; an excluded app: an empty context, excluded
    static func context(_ c: FnCtx, id: String?, bundle: String?) -> [String: Any] {
        var e: [String: Any] = ["ev": "context", "context": c.context]
        if let id { e["id"] = id }
        if let b = bundle { e["bundle"] = b }
        if c.app == nil { e["excluded"] = true }
        return e
    }

    // each 'allowed' | 'denied' | 'ask'
    static func perms(accessibility: String, mic: String, screen: String) -> [String: Any] {
        ["ev": "perms", "accessibility": accessibility, "mic": mic, "screen": screen]
    }

    static func quiet(on: Bool, why: String) -> [String: Any] { ["ev": "quiet", "on": on, "why": why] }

    static func shot(id: String, path: String?, app: String?, title: String?, skipped: Bool) -> [String: Any] {
        var e: [String: Any] = ["ev": "shot", "id": id]
        if let v = path { e["path"] = v }
        if let v = app { e["app"] = v }
        if let v = title, !v.isEmpty { e["title"] = v }
        if skipped { e["skipped"] = true }
        return e
    }

    // Quick Look's picture written at the command's out (ok), its length in ms when it has one
    static func thumb(id: String, ok: Bool, ms: Int?) -> [String: Any] {
        var e: [String: Any] = ["ev": "thumb", "id": id]
        if ok { e["ok"] = true }
        if let ms { e["ms"] = ms }
        return e
    }

    static func error(_ message: String, id: String? = nil) -> [String: Any] {
        var e: [String: Any] = ["ev": "error", "message": message]
        if let id { e["id"] = id }
        return e
    }
}
