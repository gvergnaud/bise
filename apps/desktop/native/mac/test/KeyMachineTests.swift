// The laws of KeyMachine, run by build.sh --test through the swift
// interpreter (cat'ed after src/KeyMachine.swift: no binary on disk).

var failures = 0
func law(_ name: String, _ ok: Bool) {
    if ok { print("ok   \(name)") } else { print("FAIL \(name)"); failures += 1 }
}

// run a script of (event, time) and collect every output and swallow
func run(_ steps: [(KeyIn, Double)], _ setup: (inout KeyMachine) -> Void = { _ in }) -> (out: [KeyOut], swallowed: [Bool]) {
    var m = KeyMachine()
    setup(&m)
    var out: [KeyOut] = []
    var sw: [Bool] = []
    for (e, t) in steps {
        // the caller ticks when the machine asks, before the next event
        if let at = m.nextTick, at <= t { out += m.feed(.tick, at: at).out }
        let r = m.feed(e, at: t)
        out += r.out
        sw.append(r.swallow)
    }
    return (out, sw)
}

law("hold fn past 250 ms: talkStart, release: talkEnd",
    run([(.fnDown, 0), (.fnUp, 1.0)]).out == [.talkStart(.fn), .talkEnd])
law("short fn: a tap, no talk",
    run([(.fnDown, 0), (.fnUp, 0.1)]).out == [.tap])
law("two taps within 400 ms: tap then double (hands-free)",
    run([(.fnDown, 0), (.fnUp, 0.1), (.fnDown, 0.2), (.fnUp, 0.3)]).out == [.tap, .double])
law("hands-free ends on the next tap",
    run([(.fnDown, 0), (.fnUp, 0.1), (.fnDown, 0.2), (.fnUp, 0.3), (.fnDown, 3), (.fnUp, 3.1)]).out
        == [.tap, .double, .talkEnd])
law("hands-free: holding fn does not start a second talk, release ends it",
    run([(.fnDown, 0), (.fnUp, 0.1), (.fnDown, 0.2), (.fnUp, 0.3), (.fnDown, 3), (.fnUp, 4)]).out
        == [.tap, .double, .talkEnd])
let combo = run([(.fnDown, 0), (.keyDown(18, repeated: false), 0.1), (.keyUp(18), 0.15), (.fnUp, 0.2)])
law("fn + 1: the key, swallowed down and up, no tap, no talk",
    combo.out == [.key("1")] && combo.swallowed == [true, true, true, true])
let fnA = run([(.fnDown, 0), (.keyDown(0, repeated: false), 0.1), (.keyUp(0), 0.15), (.fnUp, 0.2)])
law("fn + a (round 10): the key a, swallowed down and up, no tap, no talk",
    fnA.out == [.key("a")] && fnA.swallowed == [true, true, true, true])
law("a alone: typing, passes", run([(.keyDown(0, repeated: false), 0), (.keyUp(0), 0.1)]).swallowed == [false, false])
let dbl = run([(.fnDown, 0), (.fnUp, 0.1), (.fnDown, 0.2), (.fnUp, 0.3)])
law("fn alone (tap, double): its own down and up are swallowed, so macOS's 🌐 key (emoji picker) never fires",
    dbl.out == [.tap, .double] && dbl.swallowed == [true, true, true, true])
law("a fn up with no fn down seen (the tap started mid-press): swallowed too",
    run([(.fnUp, 0)]).swallowed == [true])
law("fn + 1 late (after the hold began): cancel, then the key",
    run([(.fnDown, 0), (.keyDown(18, repeated: false), 0.5), (.fnUp, 0.6)]).out == [.talkStart(.fn), .cancel, .key("1")])
law("fn + esc during a hold: cancel only",
    run([(.fnDown, 0), (.keyDown(53, repeated: false), 0.5), (.fnUp, 0.6)]).out == [.talkStart(.fn), .cancel])
law("fn + esc outside a hold: the key esc",
    run([(.fnDown, 0), (.keyDown(53, repeated: false), 0.1), (.fnUp, 0.2)]).out == [.key("esc")])
let arrow = run([(.fnDown, 0), (.keyDown(123, repeated: false), 0.1), (.fnUp, 0.2)])
law("fn + left arrow: passes, no tap, no talk", arrow.out == [] && arrow.swallowed == [true, false, true])
for code in [51, 117] {
    let held = run([(.fnDown, 0), (.keyDown(code, repeated: false), 0.5), (.keyDown(code, repeated: true), 0.7),
                    (.keyUp(code), 0.8), (.fnUp, 1.0)])
    law("fn ⌫ (\(code)) during a hold: the key delete once, swallowed, the talk goes on to its release",
        held.out == [.talkStart(.fn), .key("delete"), .talkEnd] && held.swallowed == [true, true, true, true, true])
    let free = run([(.fnDown, 0), (.fnUp, 0.1), (.fnDown, 0.2), (.fnUp, 0.3), (.fnDown, 1.0), (.keyDown(code, repeated: false), 1.1),
                    (.keyUp(code), 1.2), (.fnUp, 1.3), (.fnDown, 2.0), (.fnUp, 2.1)])
    law("fn ⌫ (\(code)) hands-free: the key delete, the talk stays on, the next tap ends it",
        free.out == [.tap, .double, .key("delete"), .talkEnd])
    let idle = run([(.fnDown, 0), (.keyDown(code, repeated: false), 0.1), (.fnUp, 0.2)])
    law("fn ⌫ (\(code)) outside a talk: his forward delete passes, no tap, no talk",
        idle.out == [] && idle.swallowed == [true, false, true])
}
law("auto-repeat of a bise key: swallowed, said once",
    run([(.fnDown, 0), (.keyDown(18, repeated: false), 0.1), (.keyDown(18, repeated: true), 0.6), (.keyUp(18), 0.7), (.fnUp, 0.8)]).out
        == [.key("1")])
law("keyUp of a swallowed key after fn was released: still swallowed",
    run([(.fnDown, 0), (.keyDown(49, repeated: false), 0.1), (.fnUp, 0.15), (.keyUp(49), 0.2)]).swallowed
        == [true, true, true, true])
law("right option held alone: talkStart, release: talkEnd",
    run([(.roptDown, 0), (.roptUp, 1)]).out == [.talkStart(.rightOption), .talkEnd])
let ropt = run([(.roptDown, 0), (.keyDown(0, repeated: false), 0.1), (.roptUp, 0.2)])
law("right option + a letter: typing, nothing swallowed, no talk", ropt.out == [] && ropt.swallowed == [false, false, false])
law("right option + a letter after the hold began: cancel",
    run([(.roptDown, 0), (.keyDown(0, repeated: false), 0.5), (.roptUp, 0.6)]).out == [.talkStart(.rightOption), .cancel])
law("right option + shift: no talk",
    run([(.roptDown, 0), (.otherModifiers, 0.1), (.roptUp, 1)]).out == [])
law("right option off (setting): nothing",
    run([(.roptDown, 0), (.roptUp, 1)], { $0.rightOption = false }).out == [])
let esc = run([(.keyDown(53, repeated: false), 0), (.keyUp(53), 0.1)])
law("plain esc: passes untouched, and says plainEsc once", esc.out == [.plainEsc] && esc.swallowed == [false, false])
law("plain keys without fn pass untouched",
    run([(.keyDown(18, repeated: false), 0), (.keyUp(18), 0.1)]).swallowed == [false, false])

// app.log's key lines (the dogfood, B2)
law("log: a hold past the threshold", KeyLog.line(.talkStart(.fn)) == "keys: fn held past the tap threshold")
law("log: hands-free, release, cancel, chords",
    KeyLog.line(.double) == "keys: fn fn → hands-free" && KeyLog.line(.talkEnd) == "keys: release → talk end"
        && KeyLog.line(.cancel) == "keys: hold cancelled (another key)" && KeyLog.line(.key("o")) == "keys: fn + o")
law("log: his own esc is not logged", KeyLog.line(.plainEsc) == nil)
law("round 9: fn p is a bise key (your pages)", biseKeys[35] == "p")

// S8: the route line after a talk takes a plain tab (amb-web c091e239)
var rm = KeyMachine()
let tabOff = rm.feed(.keyDown(48, repeated: false), at: 0)
law("route: no route line, tab is his app's", !tabOff.swallow && tabOff.out.isEmpty)
rm.routeOpen = true
let tabOn = rm.feed(.keyDown(48, repeated: false), at: 1)
law("route: the route line shows, tab is bise's (app_key tab)", tabOn.swallow && tabOn.out == [.key("tab")])
law("route: its key up is eaten too", rm.feed(.keyUp(48), at: 1.1).swallow)
law("route: a held tab repeats nothing", rm.feed(.keyDown(48, repeated: true), at: 1.2).out.isEmpty)
let otherKey = rm.feed(.keyDown(0, repeated: false), at: 1.3)
law("route: other keys stay his", !otherKey.swallow && otherKey.out.isEmpty)
rm.routeOpen = false
law("route: the line gone, tab is his again", !rm.feed(.keyDown(48, repeated: false), at: 2).swallow)
law("log: the route line's tab is said as such", KeyLog.line(.key("tab")) == "keys: tab on the route line")

// the selection is read only at fn space (architect m_53938)
law("selection: a talk reads the app, never the selection",
    KeyOut.talkStart(.fn).frontAsk(quote: true) == .app && KeyOut.talkStart(.rightOption).frontAsk(quote: true) == .app)
law("selection: fn space reads the app and the selection", KeyOut.key("space").frontAsk(quote: true) == .appAndSelection)
law("selection: his pref off, fn space reads the app only", KeyOut.key("space").frontAsk(quote: false) == .app)
law("selection: other keys read nothing",
    [KeyOut.tap, .double, .talkEnd, .cancel, .key("1"), .key("tab"), .plainEsc].allSatisfy { $0.frontAsk(quote: true) == nil })

if failures > 0 { print("\(failures) failed"); exit(1) }
print("all KeyMachine laws hold")
