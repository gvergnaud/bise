// KeyMachine: what fn and right option mean, as a pure state machine (no
// AppKit, no event tap): Keys.swift feeds it the tap's events and acts on
// its outputs. Tested by build.sh --test (the swift interpreter, no binary).
//
// fn (and right option, the fallback for keyboards without fn):
//   held past `holdAfter` with no other key   -> talkStart; release -> talkEnd
//   tapped (released before `holdAfter`)      -> tap; two taps within
//                                                `doubleWithin` -> double:
//                                                hands-free, until the
//                                                next tap or fn release -> talkEnd
// fn's own flagsChanged is always swallowed (macOS's 🌐 key action never
// fires: no emoji picker on a double fn).
// fn + a bise key (SPEC §4) is swallowed, before or after the hold began:
//   fn + esc while talking -> cancel; any other bise key while talking
//   -> cancel, then the key. fn + ⌫ while talking -> the key 'delete' (the
//   capsule drops the talk's screenshot), the talk goes on. fn + another key
//   (arrows, delete outside a talk, F-keys) passes and spoils the hold. Right option never swallows: any key or
//   modifier with it spoils it (it's ⌥ + a letter, typing).

import Foundation

enum TalkKey: Equatable { case fn, rightOption }

enum KeyOut: Equatable {
    case talkStart(TalkKey)
    case talkEnd
    case cancel
    case tap
    case double
    case key(String)
    case plainEsc                // esc without fn: passes, and hides bise (§8)
}

// which keys read the front app, and which of them read his selection
// (architect m_53938): only fn space, the spotlight, reads AXSelectedText,
// shown there as the ❝ chip he sees and can drop; a talk reads the app,
// never the selection; his pref off (context.quote, the helper's config
// `quote`): no key reads it
enum FrontAsk: Equatable { case app, appAndSelection }

extension KeyOut {
    func frontAsk(quote: Bool) -> FrontAsk? {
        switch self {
        case .talkStart: return .app
        case .key("space"): return quote ? .appAndSelection : .app
        default: return nil
        }
    }
}

// what app.log says for each key decision (the dogfood, B2: 'fn does
// nothing' must show why next time); the hold's own gate logs in App
enum KeyLog {
    static func line(_ o: KeyOut) -> String? {
        switch o {
        case .talkStart(let k): return "keys: \(k == .fn ? "fn" : "right option") held past the tap threshold"
        case .double: return "keys: fn fn → hands-free"
        case .talkEnd: return "keys: release → talk end"
        case .cancel: return "keys: hold cancelled (another key)"
        case .tap: return "keys: fn tap"
        case .key("tab"): return "keys: tab on the route line"
        case .key(let k): return "keys: fn + \(k)"
        case .plainEsc: return nil   // his own esc: not ours to log
        }
    }
}

// a key event as the machine sees it
enum KeyIn: Equatable {
    case fnDown, fnUp
    case roptDown, roptUp
    case otherModifiers          // shift, control, option (left), command changed
    case keyDown(Int, repeated: Bool)
    case keyUp(Int)
    case tick                    // a timer: maybe the hold threshold passed
}

// the bise keys after fn (SPEC §4), by macOS virtual key code (ANSI
// positions: fn + 1 is the key 1 of the top row, whatever the layout)
let biseKeys: [Int: String] = [
    49: "space", 18: "1", 19: "2", 20: "3", 21: "4", 23: "5", 22: "6", 26: "7",
    28: "8", 25: "9", 29: "0", 34: "i", 31: "o", 35: "p", 47: ".", 43: ",", 36: "enter", 76: "enter",
    0: "a",     // round 10: fn a, all your agents (the finder, like fn space)
    53: "esc",
]

// ⌫ with fn: forward delete (117, made below us by HID) or delete (51). bise's only during a
// talk ('delete'); outside one, fn ⌫ is his forward delete and passes
let talkDeleteKeys: Set<Int> = [51, 117]

struct KeyMachine {
    var holdAfter: Double = 0.25
    var doubleWithin: Double = 0.4
    var rightOption = true       // the fallback key (a setting)
    // S8 (amb-web c091e239): after a talk, the route line ('→ telemetry')
    // takes a plain tab for its ~2 s (to correct the route); never longer
    var routeOpen = false

    private(set) var fnIsDown = false
    private var fnAt = 0.0
    private var fnSpoiled = false    // a key came with fn: not a talk, not a tap
    private var roptIsDown = false
    private var roptAt = 0.0
    private var roptSpoiled = false
    private(set) var talking: TalkKey? = nil   // a hold is on
    private(set) var handsFree = false
    private var lastTap = -10.0
    private var swallowed = Set<Int>()   // key codes whose keyDown we ate: eat their keyUp

    init() {}

    // the time the caller should call `.tick` at, if any
    var nextTick: Double? {
        if fnIsDown && !fnSpoiled && talking == nil { return fnAt + holdAfter }
        if roptIsDown && !roptSpoiled && talking == nil { return roptAt + holdAfter }
        return nil
    }

    // feed one event at time `t` (seconds): (swallow it?, what to do)
    mutating func feed(_ e: KeyIn, at t: Double) -> (swallow: Bool, out: [KeyOut]) {
        var out: [KeyOut] = []
        var swallow = false
        switch e {
        case .fnDown:
            // fn's own down and up never reach macOS's 'Press 🌐 key to'
            // (emoji, input source, dictation): bise owns fn (the dogfood:
            // a double fn opened the emoji picker). Keys still carry the fn
            // flag, and fn + arrows/delete/F-keys are made below us (HID)
            swallow = true
            if !fnIsDown { fnIsDown = true; fnAt = t; fnSpoiled = false }
        case .fnUp:
            swallow = true
            guard fnIsDown else { break }
            fnIsDown = false
            if talking == .fn {
                talking = nil
                out.append(.talkEnd)
            } else if handsFree && !fnSpoiled {
                handsFree = false
                out.append(.talkEnd)
            } else if !fnSpoiled && t - fnAt < holdAfter {
                out.append(contentsOf: tapped(t))
            }
        case .roptDown:
            guard rightOption, !roptIsDown else { break }
            roptIsDown = true; roptAt = t
            roptSpoiled = fnIsDown || talking != nil
        case .roptUp:
            guard roptIsDown else { break }
            roptIsDown = false
            if talking == .rightOption {
                talking = nil
                out.append(.talkEnd)
            } else if !roptSpoiled && t - roptAt < holdAfter {
                out.append(contentsOf: tapped(t))
            }
        case .otherModifiers:
            if roptIsDown { out.append(contentsOf: spoilRopt()) }
        case .keyDown(let code, let repeated):
            if fnIsDown && (talking != nil || handsFree) && talkDeleteKeys.contains(code) {
                // fn ⌫ during a talk drops its screenshot (designer m_21902): bise's key, and
                // the talk goes on (its release still ends a hold; a hands-free talk ignores it)
                swallow = true
                swallowed.insert(code)
                if !repeated { fnSpoiled = true; out.append(.key("delete")) }
            } else if fnIsDown, let name = biseKeys[code] {
                swallow = true
                swallowed.insert(code)
                if repeated { break }
                fnSpoiled = true
                if name == "esc" && (talking != nil || handsFree) {
                    talking = nil; handsFree = false
                    out.append(.cancel)
                } else {
                    if talking != nil { talking = nil; out.append(.cancel) }
                    out.append(.key(name))
                }
            } else if routeOpen && code == 48 && !fnIsDown {
                // tab while the route line shows: bise's, never his app's
                swallow = true
                swallowed.insert(code)
                if !repeated { out.append(.key("tab")) }
            } else {
                if code == 53 && !fnIsDown && !repeated { out.append(.plainEsc) }
                if fnIsDown && !fnSpoiled {
                    fnSpoiled = true
                    if talking == .fn { talking = nil; out.append(.cancel) }
                }
                if roptIsDown { out.append(contentsOf: spoilRopt()) }
            }
        case .keyUp(let code):
            if swallowed.remove(code) != nil { swallow = true }
        case .tick:
            if talking == nil && !handsFree {
                if fnIsDown && !fnSpoiled && t - fnAt >= holdAfter {
                    talking = .fn; out.append(.talkStart(.fn))
                } else if roptIsDown && !roptSpoiled && t - roptAt >= holdAfter {
                    talking = .rightOption; out.append(.talkStart(.rightOption))
                }
            }
        }
        return (swallow, out)
    }

    private mutating func spoilRopt() -> [KeyOut] {
        guard !roptSpoiled else { return [] }
        roptSpoiled = true
        if talking == .rightOption { talking = nil; return [.cancel] }
        return []
    }

    private mutating func tapped(_ t: Double) -> [KeyOut] {
        if handsFree {
            handsFree = false
            return [.talkEnd]
        }
        if t - lastTap < doubleWithin {
            lastTap = -10
            handsFree = true
            return [.double]
        }
        lastTap = t
        return [.tap]
    }
}
