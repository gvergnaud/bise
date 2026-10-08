// The C5 ops on app targets: apps, snapshot, screenshot, act, plus
// permissions/request/diag, the agents' stop/pause state and the refs.
import AppKit
import ApplicationServices
import CUCore

let helperVersion = "0.1.0"

/// The refs of one target: stable while the element lives (C2).
final class Registry {
    private var next = 1
    private var byRef: [Int: AXUIElement] = [:]
    private var byKey: [AXKey: Int] = [:]
    func ref(_ e: AXUIElement) -> Int {
        if let r = byKey[AXKey(e: e)] { return r }
        if byRef.count > 50_000 { byRef.removeAll(); byKey.removeAll() } // old refs go stale
        let r = next; next += 1
        byRef[r] = e; byKey[AXKey(e: e)] = r
        return r
    }
    func element(_ r: Int) -> AXUIElement? { byRef[r] }
}

/// One resolved app target.
struct AppTarget {
    let target: String       // "app:<bundle id>"
    let bundle: String
    let app: NSRunningApplication
    let element: AXUIElement
    var name: String { app.localizedName ?? bundle }
    var pid: pid_t { app.processIdentifier }
}

final class Engine {
    static let shared = Engine()
    private let lock = NSLock()
    private var stopped = Set<String>()
    private var paused: [String: Set<String>] = [:]        // agent → targets
    private var driving: [String: (target: String, pid: pid_t)] = [:]
    private var registries: [String: Registry] = [:]       // target → refs
    private var queues: [String: DispatchQueue] = [:]
    /// Last time we posted an event to an app: the takeover monitor ignores
    /// what follows it closely.
    var lastPost = Date.distantPast
    var emit: ([String: Any]) -> Void = { _ in }

    // MARK: state (any thread)

    func locked<T>(_ f: () -> T) -> T { lock.lock(); defer { lock.unlock() }; return f() }

    func queue(for key: String) -> DispatchQueue {
        locked {
            if let q = queues[key] { return q }
            let q = DispatchQueue(label: "cu.target.\(key)")
            queues[key] = q
            return q
        }
    }
    func registry(_ target: String) -> Registry {
        locked {
            if let r = registries[target] { return r }
            let r = Registry(); registries[target] = r; return r
        }
    }
    func isStopped(_ agent: String) -> Bool { locked { stopped.contains(agent) } }

    /// {"stop"|"resume"|"release"|"drop": agent} (C4/C5 control lines).
    func control(_ kind: String, agent: String) {
        var resumed = false
        locked {
            switch kind {
            case "stop": stopped.insert(agent); driving[agent] = nil
            case "resume":
                resumed = stopped.remove(agent) != nil || !(paused[agent]?.isEmpty ?? true)
                paused[agent] = nil
            case "release": driving[agent] = nil
            case "drop": driving[agent] = nil; paused[agent] = nil; stopped.remove(agent)
            default: break
            }
        }
        if kind != "resume" { DispatchQueue.main.async { Overlay.shared.hide(agent: agent) } }
        if resumed { emit(["event": "resumed", "agent": agent]) }
    }

    /// The user clicked or typed in app `pid` (Takeover): pause who drives it.
    func userInput(pid: pid_t, force: Bool = false) {
        if !force && Date().timeIntervalSince(lastPost) < 0.3 { return }
        var hit: [(String, String)] = []
        locked {
            for (agent, d) in driving where d.pid == pid && !(paused[agent]?.contains(d.target) ?? false) {
                paused[agent, default: []].insert(d.target)
                hit.append((agent, d.target))
            }
        }
        for (agent, target) in hit {
            FileHandle.standardError.write("bise Computer Use: \(agent) paused on \(target): user input in pid \(pid)\(force ? " (simulated)" : "")\n".data(using: .utf8)!)
            DispatchQueue.main.async { Overlay.shared.hide(agent: agent) }
            emit(["event": "paused", "agent": agent, "target": target])
        }
    }

    // MARK: requests

    /// One request → its reply line (C5). Runs on the target's queue.
    func handle(_ req: [String: Any]) -> [String: Any] {
        let id = req["id"] ?? NSNull()
        let op = req["op"] as? String ?? ""
        let agent = req["agent"] as? String ?? "agent"
        if let name = req["name"] as? String, !name.isEmpty {
            DispatchQueue.main.async { Overlay.shared.label(agent, name) }
        }
        let args = req["args"] as? [String: Any] ?? [:]
        do {
            let result: Any
            switch op {
            case "permissions": result = permissions()
            case "request": result = try request(req["what"] as? String ?? args["what"] as? String ?? "")
            case "diag":
                var d = Diag.info()
                d["overlays"] = DispatchQueue.main.sync { Overlay.shared.state() }
                result = d
            case "simulate_input":
                // tests only: what Takeover does when the user clicks in the app
                let t = try resolve(args["target"] as? String)
                userInput(pid: t.pid, force: true)
                result = ["ok": true]
            case "apps": result = try apps()
            case "snapshot": result = try snapshot(agent: agent, args: args)
            case "screenshot": result = try Shot.take(engine: self, agent: agent, args: args)
            case "act": result = try act(agent: agent, args: args)
            case "status": result = ["helper": "running", "version": helperVersion].merging(permissions()) { a, _ in a }
            default: throw CUError("bad_args", "unknown op \"\(op)\"; the helper knows apps, snapshot, screenshot, act, permissions, request")
            }
            return ["id": id, "ok": true, "result": result]
        } catch let e as CUError {
            return ["id": id, "ok": false, "error": e.json]
        } catch {
            return ["id": id, "ok": false, "error": CUError("bad_args", "\(error)").json]
        }
    }

    func permissions() -> [String: Any] {
        ["accessibility": AXIsProcessTrusted(), "screen_recording": CGPreflightScreenCaptureAccess()]
    }

    func request(_ what: String) throws -> [String: Any] {
        let pane: String
        switch what {
        case "accessibility":
            _ = AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary)
            pane = "Privacy_Accessibility"
        case "screen_recording":
            _ = CGRequestScreenCaptureAccess()
            pane = "Privacy_ScreenCapture"
        default: throw CUError("bad_args", "request takes what: accessibility or screen_recording")
        }
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?\(pane)") {
            DispatchQueue.main.async { NSWorkspace.shared.open(url) }
        }
        return permissions()
    }

    func needAccessibility() throws {
        if !AXIsProcessTrusted() {
            throw CUError("no_permission", "bise Computer Use has no Accessibility permission; ask the user to turn it on in /computer-use")
        }
    }

    // MARK: targets and windows

    func resolve(_ target: String?) throws -> AppTarget {
        guard let t = target, t.hasPrefix("app:"), t.count > 4 else {
            throw CUError("bad_args", "target must be app:<bundle id> here, e.g. app:com.apple.TextEdit")
        }
        let bundle = String(t.dropFirst(4))
        if let r = Refusals.check(bundle: bundle) { throw r }
        let running = NSRunningApplication.runningApplications(withBundleIdentifier: bundle).filter { !$0.isTerminated }
        guard let app = running.first(where: { $0.activationPolicy == .regular }) ?? running.first else {
            throw CUError("not_found", "no running app has the bundle id \(bundle); ask the user to open it")
        }
        let el = AXUIElementCreateApplication(app.processIdentifier)
        AXUIElementSetMessagingTimeout(el, 3)
        return AppTarget(target: t, bundle: bundle, app: app, element: el)
    }

    /// The app's windows. AXWindows of a hidden or busy app (TextEdit while
    /// it autosaves) is sometimes empty for a moment: ask again, then fall
    /// back to its window children.
    func windowList(_ t: AppTarget) -> [AXUIElement] {
        for attempt in 0..<3 {
            let w = t.element.elements(kAXWindowsAttribute).filter { ($0.role ?? "") == "AXWindow" }
            if !w.isEmpty { return w }
            if attempt < 2 { Thread.sleep(forTimeInterval: 0.1) }
        }
        return t.element.children.filter { ($0.role ?? "") == "AXWindow" }
    }

    /// args.window: exact title first, else a unique substring; none = the main window.
    func window(_ t: AppTarget, _ want: String?) throws -> AXUIElement {
        let wins = windowList(t)
        let titles = wins.map { $0.title ?? "" }
        if let w = want {
            if let i = titles.firstIndex(of: w) { return try checked(t, wins[i]) }
            let hits = wins.indices.filter { titles[$0].localizedCaseInsensitiveContains(w) }
            if hits.count == 1 { return try checked(t, wins[hits[0]]) }
            let cands = titles.map { "- window \"\($0)\"" }
            if hits.isEmpty { throw CUError("not_found", "\(t.name) has no window titled \"\(w)\"", candidates: Array(cands.prefix(10))) }
            throw CUError("ambiguous", "\(hits.count) windows of \(t.name) match \"\(w)\"; give the exact title", candidates: hits.prefix(10).map { cands[$0] })
        }
        if let m = t.element.element(kAXMainWindowAttribute) { return try checked(t, m) }
        if let f = t.element.element(kAXFocusedWindowAttribute) { return try checked(t, f) }
        if let first = wins.first { return try checked(t, first) }
        throw CUError("not_found", "\(t.name) has no open window; ask the user to open one")
    }

    private func checked(_ t: AppTarget, _ w: AXUIElement) throws -> AXUIElement {
        if let r = Refusals.check(bundle: t.bundle, windowTitle: w.title) { throw r }
        return w
    }

    func header(_ t: AppTarget, _ w: AXUIElement) -> String {
        let title = w.title ?? ""
        return title.isEmpty ? t.name : "\(title) · \(t.name)"
    }

    func walk(_ t: AppTarget, _ w: AXUIElement, budget: Int) -> Walk {
        let reg = registry(t.target)
        return Walker.walk(w, budget: budget, ref: reg.ref)
    }

    // MARK: apps

    func apps() throws -> [[String: Any]] {
        try needAccessibility()
        let me = Bundle.main.bundleIdentifier
        return NSWorkspace.shared.runningApplications
            .filter { $0.activationPolicy == .regular && !$0.isTerminated }
            .compactMap { app -> [String: Any]? in
                guard let b = app.bundleIdentifier, b != me, Refusals.check(bundle: b) == nil else { return nil }
                let el = AXUIElementCreateApplication(app.processIdentifier)
                AXUIElementSetMessagingTimeout(el, 1)
                let focused = el.element(kAXFocusedWindowAttribute) ?? el.element(kAXMainWindowAttribute)
                let wins: [[String: Any]] = el.elements(kAXWindowsAttribute).map { w in
                    ["title": w.title ?? "", "focused": focused.map { CFEqual($0, w) } ?? false]
                }
                return ["target": "app:\(b)", "name": app.localizedName ?? b, "pid": Int(app.processIdentifier), "windows": wins]
            }
    }

    // MARK: snapshot

    func snapshot(agent: String, args: [String: Any]) throws -> [String: Any] {
        try needAccessibility()
        if isStopped(agent) { throw stoppedError() }
        let t = try resolve(args["target"] as? String)
        let w = try window(t, args["window"] as? String)
        let budget = max(1, (args["max_nodes"] as? NSNumber)?.intValue ?? 400)
        let walk = self.walk(t, w, budget: budget)
        let text = Snapshot.text(header: header(t, w), nodes: walk.nodes)
        return ["target": t.target, "title": w.title ?? "", "text": text,
                "refs": walk.nodes.filter { $0.ref != nil }.count, "truncated": walk.truncated]
    }

    func stoppedError() -> CUError {
        CUError("stopped", "the user stopped you; ask before you start again", summary: "you stopped it")
    }

    // MARK: act

    func act(agent: String, args: [String: Any]) throws -> [String: Any] {
        let action = args["action"] as? String ?? ""
        let verbs = ["click": "click", "fill": "fill", "type": "type in", "press": "press", "select": "select",
                     "check": "check", "hover": "hover", "scroll": "scroll", "goto": "go to", "close": "close",
                     "wait": "wait for", "read": "read"]
        guard let verb = verbs[action] else {
            throw CUError("bad_args", "unknown action \"\(action)\"; use one of \(verbs.keys.sorted().joined(separator: ", "))")
        }
        if action == "goto" { throw CUError("bad_args", "goto works on tabs only", summary: "couldn't go there: not a browser tab") }
        try needAccessibility()
        if isStopped(agent) { throw stoppedError() }
        let t: AppTarget
        do { t = try resolve(args["target"] as? String) } catch var e as CUError {
            e.summary = e.summary ?? (e.code == "refused" ? "couldn't use that app: off limits for agents" : "couldn't \(verb): the app isn't open")
            throw e
        }
        if locked({ paused[agent]?.contains(t.target) ?? false }) {
            throw CUError("paused", "the user is using \(t.name); wait until they give it back",
                          summary: "paused · you took the wheel in \(t.name)")
        }
        let act = Act(engine: self, agent: agent, target: t, args: args, action: action, verb: verb)
        locked { driving[agent] = (t.target, t.pid) }
        return try act.run()
    }
}

// MARK: diag (the TCC check: who is responsible for this process)

enum Diag {
    typealias RespFn = @convention(c) (pid_t) -> pid_t
    static func responsible(_ pid: pid_t) -> pid_t? {
        guard let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "responsibility_get_pid_responsible_for_pid") else { return nil }
        return unsafeBitCast(sym, to: RespFn.self)(pid)
    }
    static func path(_ pid: pid_t) -> String {
        var buf = [CChar](repeating: 0, count: 4096)
        return proc_pidpath(pid, &buf, UInt32(buf.count)) > 0 ? String(cString: buf) : ""
    }
    static func info() -> [String: Any] {
        let me = getpid()
        let r = responsible(me) ?? -1
        return ["pid": Int(me), "responsible_pid": Int(r), "responsible_path": r > 0 ? path(r) : "",
                "bundle_path": Bundle.main.bundlePath, "bundle_id": Bundle.main.bundleIdentifier ?? "",
                "version": helperVersion, "accessibility": AXIsProcessTrusted(),
                "screen_recording": CGPreflightScreenCaptureAccess()]
    }
}
