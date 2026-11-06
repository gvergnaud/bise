// The laws of Lines (architect m_9461: the Swift side checks itself
// against bise-proto's fixtures, no Rust dependency), run by build.sh --test
// after src/KeyMachine.swift + src/FrontRules.swift + src/Lines.swift.
// HELPER_FIXTURES = rust/proto/fixtures.

import Foundation

var failures = 0
func law(_ name: String, _ ok: Bool) {
    if ok { print("ok   \(name)") } else { print("FAIL \(name)"); failures += 1 }
}

let dir = ProcessInfo.processInfo.environment["HELPER_FIXTURES"] ?? "../../../../rust/proto/fixtures"
func lines(_ f: String) -> [String] {
    guard let s = try? String(contentsOfFile: dir + "/" + f, encoding: .utf8) else { print("FAIL no fixture \(f) in \(dir)"); exit(1) }
    return s.split(separator: "\n").map(String.init).filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
}
func json(_ l: String) -> NSDictionary { (try? JSONSerialization.jsonObject(with: Data(l.utf8))) as? NSDictionary ?? [:] }

// every command line decodes to a known command, as Rust's HelperCmd
let want: [HelperCmd] = [
    .config(exclude: ["com.agilebits.onepassword7"], rightOption: true, quote: true),
    .config(exclude: [], rightOption: false, quote: false),
    .route(open: true), .route(open: false), .ownMic(open: true), .readContext(id: "c7"),
    .perm("accessibility", ask: true), .perm("mic", ask: false), .perm("screen", ask: true),
    .shot(id: "s1", dir: "/Users/u/.bise/run/shots"), .quit,
]
let cmds = lines("helper_cmd.jsonl")
law("cmd: one expectation per fixture line", cmds.count == want.count)
for (l, w) in zip(cmds, want) {
    let got = try? Lines.decode(l).get()
    law("cmd: \(l)", got == w)
}

// every event line is what the encoders build
let safari = FnCtx(app: "Safari", bundle: "com.apple.Safari", url: "https://github.com/acme/checkout/pull/42",
                   title: "checkout: fix the card field on Safari #42", selection: "the card number field loses focus",
                   windowText: "Conversation 3  Commits 2  Files changed 4")
let code = FnCtx(app: "Code", bundle: "com.microsoft.VSCode", title: "main.rs — telemetry", file: "/Users/u/w/telemetry/src/main.rs")
let built: [[String: Any]] = [
    Lines.hello(version: "0.1.0", pid: 4242, responsible: (4200, "/Applications/bise.app/Contents/MacOS/bise")),
    Lines.hello(version: "0.1.0", pid: 4242, responsible: nil),
    Lines.fn(.talkStart(.fn)), Lines.fn(.talkStart(.rightOption)), Lines.fn(.talkEnd), Lines.fn(.cancel),
    Lines.fn(.tap), Lines.fn(.double), Lines.fn(.key("space")), Lines.fn(.key("1")), Lines.fn(.plainEsc),
    Lines.context(safari, id: nil, bundle: "com.apple.Safari"),
    Lines.context(code, id: "c7", bundle: "com.microsoft.VSCode"),
    Lines.context(FrontRules.build(FrontRaw(app: "1Password", bundle: "com.1password.1password", title: "Vault"), skip: SkipList.builtIn),
                  id: "c8", bundle: "com.1password.1password"),
    safari.front, code.front, FnCtx().front,
    Lines.perms(accessibility: "allowed", mic: "ask", screen: "ask"),
    Lines.perms(accessibility: "ask", mic: "denied", screen: "allowed"),
    Lines.quiet(on: true, why: "call"), Lines.quiet(on: true, why: "sharing"),
    Lines.quiet(on: true, why: "full_screen"), Lines.quiet(on: false, why: ""),
    Lines.shot(id: "s1", path: "/Users/u/.bise/run/shots/fn-1791100400000.png", app: "Safari",
               title: "checkout: fix the card field on Safari #42", skipped: false),
    Lines.shot(id: "s2", path: nil, app: "1Password", title: "", skipped: true),
    Lines.shot(id: "s3", path: nil, app: nil, title: nil, skipped: false),
    Lines.error("shot: no dir", id: "s4"),
    Lines.error("not JSON: expected value at line 1 column 1"),
]
let evs = lines("helper_ev.jsonl")
law("ev: one encoder call per fixture line", evs.count == built.count)
for (l, b) in zip(evs, built) {
    // through the wire: encode, parse back, compare as JSON values
    law("ev: \(l.prefix(70))", json(Lines.encode(b)) == json(l))
}

// the quiet machine's words are QuietWhy's, the key names the fixtures'
let whys = Set(evs.map(json).filter { $0["ev"] as? String == "quiet" }.compactMap { $0["why"] as? String })
law("quiet: the fixtures name every why the machine says", whys == ["call", "sharing", "full_screen", ""])

// bad lines: an error, never a crash; a newer app's command: unknown
law("bad: not JSON", (try? Lines.decode("hello").get()) == nil)
law("bad: no cmd", (try? Lines.decode(#"{"ev":"fn"}"#).get()) == nil)
law("bad: a perm we don't know", (try? Lines.decode(#"{"cmd":"perm","perm":"contacts","action":"ask"}"#).get()) == nil)
law("bad: open must be a bool", (try? Lines.decode(#"{"cmd":"route","open":1}"#).get()) == nil)
law("bad: a shot without a dir", (try? Lines.decode(#"{"cmd":"shot","id":"s"}"#).get()) == nil)
law("newer: an unknown cmd decodes as unknown", (try? Lines.decode(#"{"cmd":"vibrate","ms":3}"#).get()) == .unknown("vibrate"))
law("newer: unknown fields are ignored", (try? Lines.decode(#"{"cmd":"route","open":true,"later":1}"#).get()) == .route(open: true))
law("wire: one line, no newline inside", !Lines.encode(Lines.error("a\nb")).contains("\n"))

if failures > 0 { print("\(failures) failed"); exit(1) }
print("all Lines laws hold")
