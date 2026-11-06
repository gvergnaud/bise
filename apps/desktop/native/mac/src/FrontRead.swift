// FrontRead: the Accessibility read of the front app at fn (S9, lead m_9025).
// Read-only attribute reads on its own queue: each call bounded by a 50 ms
// messaging timeout, the tree walk by a node count and a deadline; what was
// read by then goes (FrontRules decides what of it leaves the Mac). Never
// sets anything on his apps (no AXManualAccessibility / AXEnhancedUserInterface:
// a Chromium tab's URL comes from AppleScript instead). App calls it only
// outside QA and harness mode: tests give FrontRaw fixtures.

import AppKit
import ApplicationServices

enum FrontRead {
    private static let queue = DispatchQueue(label: "bise.ambient.front", qos: .userInitiated)
    static let maxNodes = 400
    static let maxDepth = 30
    static let textRoles: Set<String> = ["AXStaticText", "AXHeading", "AXTextArea", "AXTextField", "AXLink"]

    // `done` runs on the main queue
    // `selection`: read AXSelectedText too (fn space only, Helper.keyOut)
    static func read(pid: pid_t, app: String?, bundle: String?, selection: Bool, within: Double,
                     done: @escaping @MainActor (FrontRaw) -> Void) {
        queue.async {
            let end = DispatchTime.now().uptimeNanoseconds + UInt64(max(0, within) * 1_000_000_000)
            var raw = FrontRaw(app: app, bundle: bundle)
            let a = AXUIElementCreateApplication(pid)
            AXUIElementSetMessagingTimeout(a, 0.05)
            // the selection first when asked: what he points at matters most
            if selection, let f = element(a, kAXFocusedUIElementAttribute) { raw.selection = string(f, kAXSelectedTextAttribute) }
            if let w = element(a, kAXFocusedWindowAttribute) {
                raw.title = string(w, kAXTitleAttribute)
                raw.document = string(w, kAXDocumentAttribute)
                walk(w, into: &raw, end: end)
            }
            DispatchQueue.main.async { MainActor.assumeIsolated { done(raw) } }
        }
    }

    // breadth first: the window's top texts before its deep ones
    private static func walk(_ root: AXUIElement, into raw: inout FrontRaw, end: UInt64) {
        var level = [root]
        var nodes = 0
        var chars = 0
        for _ in 0..<maxDepth {
            var next: [AXUIElement] = []
            for e in level {
                if DispatchTime.now().uptimeNanoseconds > end || nodes >= maxNodes { return }
                nodes += 1
                let role = string(e, kAXRoleAttribute) ?? ""
                if role == "AXWebArea", raw.axURL == nil { raw.axURL = string(e, "AXURL") }
                if textRoles.contains(role), chars < FrontRules.textCap,
                   let t = string(e, kAXValueAttribute) ?? string(e, kAXTitleAttribute) {
                    let cut = String(t.prefix(FrontRules.textCap - chars))
                    raw.texts.append(cut)
                    chars += cut.count
                }
                next += children(e)
            }
            if next.isEmpty { return }
            level = next
        }
    }

    private static func value(_ e: AXUIElement, _ name: String) -> CFTypeRef? {
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(e, name as CFString, &v) == .success else { return nil }
        return v
    }

    // a string, or a URL as its string (AXURL, some AXDocument)
    private static func string(_ e: AXUIElement, _ name: String) -> String? {
        guard let v = value(e, name) else { return nil }
        if let s = v as? String { return s }
        if CFGetTypeID(v) == CFURLGetTypeID() { return (v as! URL).absoluteString }
        return nil
    }

    private static func element(_ e: AXUIElement, _ name: String) -> AXUIElement? {
        guard let v = value(e, name), CFGetTypeID(v) == AXUIElementGetTypeID() else { return nil }
        return (v as! AXUIElement)
    }

    private static func children(_ e: AXUIElement) -> [AXUIElement] {
        guard let v = value(e, kAXChildrenAttribute), let list = v as? [AnyObject] else { return [] }
        return list.compactMap { o in CFGetTypeID(o) == AXUIElementGetTypeID() ? (o as! AXUIElement) : nil }
    }
}
