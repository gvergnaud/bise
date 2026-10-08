// The agent's cursor over the driven window (design §5.1/§5.2, designer
// m_3551): a click-through, non-activating panel ordered just above the
// target window, so it is covered when the window is. Purely visual: the
// real cursor never moves. Main thread only.
import AppKit

final class CursorView: NSView {
    var name = ""
    var point = CGPoint(x: 40, y: 40)       // view coordinates (flipped)
    var ring: CGFloat? = nil                // 0…1 while the click ring plays
    override var isFlipped: Bool { true }

    static let ink = NSColor(srgbRed: 0x14 / 255, green: 0x12 / 255, blue: 0x11 / 255, alpha: 1)
    static let edge = NSColor(srgbRed: 0xec / 255, green: 0xe6 / 255, blue: 0xda / 255, alpha: 1)
    static let pink = NSColor(srgbRed: 0xf4 / 255, green: 0xa6 / 255, blue: 0xb0 / 255, alpha: 1)

    override func draw(_ dirty: NSRect) {
        let p = point
        if let r = ring {
            let radius = 6 + 16 * r
            let c = NSBezierPath(ovalIn: NSRect(x: p.x - radius, y: p.y - radius, width: radius * 2, height: radius * 2))
            CursorView.pink.withAlphaComponent(1 - r).setStroke()
            c.lineWidth = 2
            c.stroke()
        }
        // the arrow: tip at p
        let a = NSBezierPath()
        let pts: [(CGFloat, CGFloat)] = [(0, 0), (0, 17), (4.2, 13), (7.2, 19.6), (10, 18.4), (7.1, 12), (12.6, 12)]
        a.move(to: NSPoint(x: p.x + pts[0].0, y: p.y + pts[0].1))
        for q in pts.dropFirst() { a.line(to: NSPoint(x: p.x + q.0, y: p.y + q.1)) }
        a.close()
        NSGraphicsContext.saveGraphicsState()
        let glow = NSShadow()
        glow.shadowColor = CursorView.pink.withAlphaComponent(0.85)
        glow.shadowBlurRadius = 8
        glow.shadowOffset = .zero
        glow.set()
        CursorView.ink.setFill()
        a.fill()
        NSGraphicsContext.restoreGraphicsState()
        CursorView.edge.setStroke()
        a.lineWidth = 1.5
        a.lineJoinStyle = .round
        a.stroke()
        // the name pill, 2px pink left edge
        guard !name.isEmpty else { return }
        let font = NSFont.systemFont(ofSize: 11, weight: .medium)
        let text = NSAttributedString(string: name, attributes: [.font: font, .foregroundColor: CursorView.edge])
        let ts = text.size()
        let box = NSRect(x: p.x + 14, y: p.y + 18, width: ts.width + 14, height: ts.height + 6)
        let pill = NSBezierPath(roundedRect: box, xRadius: 4, yRadius: 4)
        CursorView.ink.withAlphaComponent(0.94).setFill()
        pill.fill()
        NSGraphicsContext.saveGraphicsState()
        pill.addClip()
        CursorView.pink.setFill()
        NSRect(x: box.minX, y: box.minY, width: 2, height: box.height).fill()
        NSGraphicsContext.restoreGraphicsState()
        text.draw(at: NSPoint(x: box.minX + 8, y: box.minY + 3))
    }
}

final class Overlay {
    static let shared = Overlay()

    final class Entry {
        let panel: NSPanel
        let view: CursorView
        var windowID: CGWindowID = 0
        var timer: Timer?
        var anim: Timer?
        var windowFrame = CGRect.zero        // CG global, top-left origin
        var pointCG = CGPoint.zero           // CG global
        init(panel: NSPanel, view: CursorView) { self.panel = panel; self.view = view }
    }
    private var entries: [String: Entry] = [:]

    /// The window is on screen (not minimized, hidden, or on another Space).
    static func onScreen(_ id: CGWindowID) -> Bool {
        guard let info = CGWindowListCopyWindowInfo([.optionIncludingWindow], id) as? [[String: Any]],
              let w = info.first else { return false }
        return (w[kCGWindowIsOnscreen as String] as? Bool) ?? false
    }

    /// CG global (top-left origin) → Cocoa screen (bottom-left origin).
    static func cocoa(_ r: CGRect) -> NSRect {
        let h = NSScreen.screens.first?.frame.height ?? 0
        return NSRect(x: r.minX, y: h - r.maxY, width: r.width, height: r.height)
    }

    /// What each agent's pill shows: C5 `name` (its key is `<hub id>.<dir>`,
    /// docs/issues/18); the key itself until a request named it.
    private var labels: [String: String] = [:]

    func label(_ agent: String, _ name: String) {
        labels[agent] = name
        entries[agent]?.view.name = name
    }

    func entry(_ agent: String) -> Entry {
        if let e = entries[agent] { return e }
        let panel = NSPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.ignoresMouseEvents = true
        panel.level = .normal
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [.ignoresCycle, .fullScreenAuxiliary, .transient]
        let view = CursorView(frame: .zero)
        view.name = labels[agent] ?? agent
        panel.contentView = view
        let e = Entry(panel: panel, view: view)
        entries[agent] = e
        return e
    }

    func show(agent: String, windowID: CGWindowID, windowFrame: CGRect, point: CGPoint, click: Bool) {
        let e = entry(agent)
        let from = e.windowID == windowID && e.panel.isVisible ? e.pointCG : CGPoint(x: windowFrame.midX, y: windowFrame.midY)
        e.windowID = windowID
        e.windowFrame = windowFrame
        place(e)
        guard e.panel.isVisible else { return }
        // glide ~150 ms, then the ring 300 ms
        e.anim?.invalidate()
        let start = Date()
        e.anim = Timer.scheduledTimer(withTimeInterval: 1 / 60, repeats: true) { [weak self] t in
            guard let self = self else { t.invalidate(); return }
            let dt = Date().timeIntervalSince(start)
            let k = min(1, dt / 0.15)
            let ease = 1 - pow(1 - k, 3)
            e.pointCG = CGPoint(x: from.x + (point.x - from.x) * ease, y: from.y + (point.y - from.y) * ease)
            e.view.ring = click && dt > 0.15 ? CGFloat(min(1, (dt - 0.15) / 0.3)) : nil
            self.draw(e)
            if dt >= (click ? 0.45 : 0.15) { e.view.ring = nil; self.draw(e); t.invalidate() }
        }
        // follow the window while the agent drives (moved, covered, minimized)
        if e.timer == nil {
            e.timer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in self?.place(e) }
        }
    }

    private func place(_ e: Entry) {
        guard Overlay.onScreen(e.windowID) else { e.panel.orderOut(nil); return }
        if let info = CGWindowListCopyWindowInfo([.optionIncludingWindow], e.windowID) as? [[String: Any]],
           let b = info.first?[kCGWindowBounds as String] as? NSDictionary,
           let r = CGRect(dictionaryRepresentation: b) {
            e.windowFrame = r
        }
        e.panel.setFrame(Overlay.cocoa(e.windowFrame), display: false)
        e.panel.order(.above, relativeTo: Int(e.windowID))
        draw(e)
    }

    private func draw(_ e: Entry) {
        e.view.point = CGPoint(x: e.pointCG.x - e.windowFrame.minX, y: e.pointCG.y - e.windowFrame.minY)
        e.view.needsDisplay = true
    }

    func hide(agent: String) {
        guard let e = entries[agent] else { return }
        e.timer?.invalidate(); e.timer = nil
        e.anim?.invalidate(); e.anim = nil
        e.panel.orderOut(nil)
    }

    /// For the tests: is this agent's cursor showing, and over which window.
    func state() -> [[String: Any]] {
        entries.map { ["agent": $0.key, "visible": $0.value.panel.isVisible, "window_id": Int($0.value.windowID)] }
    }
}
