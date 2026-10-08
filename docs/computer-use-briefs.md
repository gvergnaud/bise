# Computer use: one brief per agent

Main spawns these. Design: [computer-use-design.md](computer-use-design.md)
(read §1, §3, §5-§8 before you start; §10 holds the user's decisions).
Spike: [research/computer-use-spike/](research/computer-use-spike/).
Designer signs off the UI (m_3551, m_3554). The contract owner is
`computer-use`: a change to a contract below goes through it first.

## Order (waves)

| Wave | Agents | Starts | Why |
|---|---|---|---|
| 1 | **cu-broker**, **cu-extension**, **cu-apps**, **cu-sdk** | at once, in parallel | each builds against the contracts below and a fake of its neighbour; nobody waits for anybody |
| 1b | **cu-setup** | at once, against a fake `state.json` and a fake broker | the TUI part only reads files and calls `bise computer-use` |
| 2 | (main) integration on `computer-use`: one real run, Chrome + Edge, the bench | when wave 1 reports done | first time the four parts meet |
| 3 | **cu-approvals** | after the `approvals` branch has its gate (approvals-auto) | the cards need the gate; in `yolo` nothing asks, so the MVP works without it |
| 3 | **cu-store** | after wave 2 | the listing needs the real extension, its screenshots and its final permissions |
| later | **cu-safari** (after cu-apps), **cu-firefox** (spike first) | when the user says go | design §4.1b |

Costs (agent-days): cu-broker 2, cu-extension 2, cu-sdk 1.5, cu-setup
1.5, cu-apps 5, cu-approvals 1, cu-store 0.5 (+ review days), wave 2
0.5. Web MVP ≈ 7.5 with the Chromium family; apps in parallel.

## Rules for every agent (paste into each brief)

- **Launch freeze**: nothing lands on main before 17:30 today (Paris).
  Nothing of this feature lands on main at all until the user has tried a
  local build: everything goes to the local branch **`computer-use`**
  (main creates it from main's tip; never pushed).
- Work in your own worktree: `tests/gate.sh new <your name>`, then
  `git checkout --detach computer-use`. Commit onto
  `refs/heads/computer-use` without checking it out, with a private
  index and a compare-and-swap (the recipe of approvals-plan.md §5):
  ```sh
  old=$(git rev-parse refs/heads/computer-use)
  export GIT_INDEX_FILE=$TMPDIR/cu.idx
  git diff $base -- <your files> > $TMPDIR/mine.patch   # base = the tip your worktree is on
  git read-tree $old && git apply --cached $TMPDIR/mine.patch
  new=$(git commit-tree $(git write-tree) -p $old -F $TMPDIR/msg.txt)
  git update-ref refs/heads/computer-use $new $old      # refused if someone committed first: redo
  git checkout --detach computer-use                    # then keep building on everyone's work
  ```
  Your hunks only, never `git add` of whole files. Never stash, reset,
  amend, rebase or push; never touch the shared index or the shared
  folder's branch.
- **Your files only** (the ownership column below). A one-line hook in a
  file you don't own (a `mod` line, a match arm): say it in your report
  with the exact hunk.
- **Never the user's browser or apps.** Tests use a throwaway Chrome
  profile (`--user-data-dir` in `$TMPDIR`, the extension loaded with CDP
  `Extensions.loadUnpacked` over `--remote-debugging-pipe`, see the
  spike), a temp `HOME` for the native host manifests, and for apps
  only TextEdit / Calculator / a test app you build. Never the live hub.
  A window that pops up during a test closes at the end of it.
- **Never steal focus, in tests too** (the user, m_3672; it is also the
  product's rule). Tests run Chrome with `--headless=new` unless the
  test needs a visible window: headless Chrome 154 loads the extension
  with `Extensions.loadUnpacked`, makes tab groups, runs
  `chrome.debugger` and native messaging, and shows the same 5 s
  hidden-tab trap (checked: the spike runs headless now, the frontmost
  app never changed). When a headed window is really needed: `open -g
  -n -a 'Google Chrome' --args --user-data-dir=<throwaway> --no-first-run
  --no-default-browser-check` (optionally `--window-position` far
  off-screen); never `osascript … activate`, never a focus call. Every
  app the helper or a test launches: `open -g`. Close your throwaway
  Chromes when the test ends (also on failure); never kill a Chrome that
  isn't yours (match on your `--user-data-dir`).
- Never print env or keys.
- `tests/gate.sh` (quick) per commit, `gate.sh full` once at the end, in
  the foreground; `gate.sh done <name>` at the very end.
- Any text the user sees, or any look the design doesn't fix: ask
  designer. Contract change: ask `computer-use`.
- Done = your list below, the gate green, a report with the commits, what
  you didn't do, and 3 lines "how to try it".

## File ownership

| Agent | Owns |
|---|---|
| cu-broker | `rust/computer-use/` (new crate `bise-computer-use`), the `computer-use` subcommand hunk in `rust/harness/src/main.rs`, the built-in plugin root in `rust/plugins/` (scope `built-in`), `plugins/computer/plugin.json` + `mcp.json` |
| cu-extension | `computer-use/extension/` (MV3, its tests in `computer-use/extension/test/`) |
| cu-apps | `computer-use/macos-app/` (Swift package + bundle script), the helper's hunk in `packaging/` (sign + notarize) |
| cu-sdk | `rust/jsrt/src/computer.ts` (+ its `include_str!` hunk in `rust/jsrt/src/main.rs`), `plugins/computer/skills/computer-use/SKILL.md`, the raw tools' descriptions |
| cu-setup | `rust/tui/src/computer_use.rs` (+ hooks in the TUI: slash command, `↖` mark, divider, tool rows), the hub's stop hook |
| cu-approvals | the computer-use part of the gate on the `approvals` branch, `[computer_use]` in approvals.toml |
| cu-store | `computer-use/store/` (listing texts, privacy policy, icons, screenshots), the installer step |

`computer-use/` is a new root folder: main checks it against
root-layout-plan.md (it may become `projects/computer-use/`) before
wave 1 starts; the paths move as one rename either way.

## Contracts (written by `computer-use`, frozen at spawn)

### C1. The raw tools (what agents and the gate see)

Namespace `computer` (the built-in plugin). Targets are strings:
`tab:<chrome tab id>` or `app:<bundle id>`.

| Tool | Args | Result |
|---|---|---|
| `status` | `{}` | `{ browsers: [{ name, version, connected, extension_version }], apps: { helper: "absent"\|"stopped"\|"running", accessibility, screen_recording }, me: { stopped, paused: [target] } }` |
| `open` | `{ url, browser? }` | `{ target, url, title }` (a background tab in the agent's group) |
| `tabs` | `{}` | `[{ target, url, title, user_touched }]` (this agent's tabs only) |
| `apps` | `{}` | `[{ target, name, pid, windows: [{ title, focused }] }]` |
| `snapshot` | `{ target, window?, max_nodes? = 400 }` | `{ target, url?, title, text, refs, truncated }` |
| `screenshot` | `{ target, window?, ref?, max_width? = 1280 }` | `{ path, mime, width, height }` (a JPEG in the agent's `TMPDIR`) |
| `act` | `{ target, window?, action, ref? \| locator?, text?, keys?, value?, url?, direction?, amount?, timeout_ms? = 5000 }` | `{ ok: true, url?, title, changed, summary }` |

- `window` (app targets only, ignored for `tab:`): a window title, exact
  match first, else a unique substring; 0 or several → `not_found` /
  `ambiguous` with the titles as candidates; absent = the app's main
  window. Refs belong to the window they came from. (Amended m_3611.)
- `action`: `click`, `fill`, `type`, `press`, `select`, `check`,
  `hover`, `scroll`, `goto`, `close`, `wait`, `read` (`read` returns the
  element's text in `changed`).
- `locator`: `{ role?, name?, name_re?, text?, text_re?, label?, exact?,
  nth? }`; `*_re` is a JS regex source, flags after a `/`
  (`"Anker.*2 m/i"`). Zero matches: `not_found`; several and no `nth`:
  `ambiguous`; both list up to 10 candidates as snapshot lines.
- `summary`: the one line the TUI shows after `↖`, user-facing:
  `clicked "Add to cart" · amazon.fr`, `typed in the search box · figma`.
- `changed`: a short aria diff (≤ 20 lines) of the target after the
  action, `""` when nothing changed.
- Errors: `{ code, message, candidates? }` with `code` in
  `not_set_up`, `no_browser`, `no_helper`, `no_permission`, `not_found`,
  `ambiguous`, `stale_ref`, `stopped`, `paused`, `refused`, `timeout`,
  `needs_front`, `bad_args`. `message` is one sentence the agent can
  act on ("the user stopped you in Chrome; ask before you start again").
  An `act` error also carries `summary`, the user-facing failure line
  without the `✗` (the TUI adds it in err colour): `couldn't click "Add
  to cart": a popup covers it`, `couldn't find "Email"`. Built by
  whoever acts (extension, helper); the broker passes it through
  (cu-extension m_3630, designer's row shape).
- **How an error reaches code mode** (cu-sdk m_3802): the MCP server
  answers a C1 error as a normal result, `isError: false`, body
  `{"error":{code,message,candidates?,summary?}}`. (bise's runtime ends
  the whole program on an `isError` tool result,
  `bend/runtime/main.bend` `exec_program.decide`, so the SDK could not
  catch it.) The SDK throws an `Error` with `code`, `candidates`,
  `summary`; uncaught, the program fails with `<code>: <message>` and the
  candidates in one go. Raw `tools.computer.*` callers get the `{error}`
  object. The TUI draws the `✗` tool row when the result holds `error`
  (its `summary`), not from `isError`. `isError: true` stays for
  transport failures only (broker unreachable, bad JSON).

Settled details (cu-extension m_3613, cu-apps m_3609; the same on web
and apps):

- `snapshot.refs` = the number of `[eN]` refs in `text`.
- A 6th tab for one agent: `refused`, "you already have 5 tabs open;
  close one (act close) first".
- `scroll`: `direction` `up|down|left|right`, default `down`; `amount`
  in CSS px (web) or points (apps), default 80 % of the visible height
  of the scrolled element or window.
- `wait`: with a ref/locator, until visible; `text` alone, until the
  page/window contains it; nothing, sleeps `amount` ms (≤ `timeout_ms`).
- `check`: `value: false` unchecks.
- `press`: Playwright key names (`Enter`, `Control+A`, `Meta+C`),
  several chords space-separated (`"Tab Tab Enter"`); with a
  ref/locator, it focuses that element first.
- `read` without ref/locator: the page's (window's) text, cut at 4000
  characters; on apps, the static texts and values, one per line.
- Apps: `goto` → `bad_args` "goto works on tabs only"; `hover` posts a
  mouse-moved event to the app's pid (the real cursor stays).

### C2. The snapshot text

One node per line, 2 spaces per depth:
`- <role> "<name>" [e<N>]` then, when set, ` value="…"` (cut at 80),
` (disabled)`, ` (focused)`, ` (checked)`, ` (expanded)`. Roles are ARIA
names on both sides; the helper maps AX roles: `AXButton`→`button`,
`AXTextField`/`AXTextArea`→`textbox`, `AXSearchField`→`searchbox`,
`AXCheckBox`→`checkbox`, `AXRadioButton`→`radio`, `AXPopUpButton`→
`combobox`, `AXMenuItem`→`menuitem`, `AXLink`→`link`, `AXStaticText`→
`text`, `AXImage`→`img`, `AXTabGroup`→`tablist`, `AXSlider`→`slider`,
`AXWindow`→`window`, `AXGroup`→`group`; any other: the AX role without
`AX`, lowercased. Refs live until the target navigates or its window
closes (`stale_ref` after). Password fields: `value="•••"`, never the
value. The first line is `# <title> · <host or app name>`.

### C3. Agent side ↔ broker

Unix socket `~/.bise/run/computer-use.sock` (mode 0600), JSON lines.
Hello: `{"op":"hello","agent":"<SB_AGENT>","session":"<id>","tmpdir":"…"}`.
Then `{"id":n,"op":"<tool>","args":{…}}` → `{"id":n,"ok":true,"result":…}`
or `{"id":n,"ok":false,"error":{…C1}}`. One connection per agent session
(the MCP server `bise computer-use mcp`). The broker routes `tab:` to the
extension of the browser that owns the tab, `app:` to the helper.

Who connects (docs/issues/18, `computer-use/src/who.rs`): the broker reads
each peer's process (`bise_peer::judge::judge_any`). An agent is keyed
`<hub id>.<dir>` by the tag of its process (the last tag of the nearest
tagged process), never by the hello's `agent`, which only names an
untagged one (`bise --headless`); `state.json` (v2) and `events.jsonl`
carry the key with its `name` and `hub`. A browser relay must come from
outside any agent; the agents' socket takes no command.

Commands (`bise computer-use stop …`, setup-check, the TUI, bise ambient)
go to their own socket, `~/.bise/run/computer-use-ctl.sock`
(`bise_home::socket::COMPUTER_USE_CTL`, denied in the agents' sandbox),
served to the user's processes only (else `refused`): they say
`{"op":"hello","role":"ctl"}` (no ack, no agent session) and send the same
request lines; an agent names another by its key. `show` is a command op only, never an agent tool
(bise ambient, docs/ambient-pages.md §2.8): `{"op":"show","args":{"url":
"http(s)://…","url_prefix"?,"browser"?}}` → `{"tab_id","created","url",
"browser"}`; the first connected browser unless `browser`; no wait for a
browser (`no_browser`/`not_set_up` at once: the caller opens the page
itself).

### C4. Broker ↔ extension (native messaging)

Host name `dev.bise.computer_use`; manifest written by the broker into
each installed Chromium browser's `NativeMessagingHosts/` (design
§4.1b), `path` = the shim `~/.bise/bin/bise-chrome-host`. The extension
id is pinned with `key` in its manifest (cu-extension generates the key
pair and commits the public key; the Web Store keeps the same id).

- ext → host, first: `{"hello":{"browser":"chrome|edge|brave|vivaldi|opera|arc","version":"154.0…","extension_version":"…"}}`
- host → ext: `{"id":n,"agent":"…","op":"open|tabs|snapshot|screenshot|act","args":{…}}`
  → ext → host `{"id":n,"ok":…,"result"|"error":…}` (C1 shapes;
  `screenshot` returns `{"data":"<base64 jpeg>",…}`, the broker writes
  the file).
- host → ext: `{"id":n,"op":"show","args":{"url","url_prefix"?}}`, no
  agent: the tab whose URL is under `url_prefix` (default `url`; equal, or
  the next char is `/ ? #`) in a group titled exactly `bise` comes
  forward as is, else `url` opens active in that group (created pink in
  the last focused window); its window takes focus → `{"tab_id",
  "created","url"}`. No debugger, no overlay, not an agent's tab, no
  5-tab limit: the user asked to see the page.
- host → ext: `{"stop":"<agent>"}`, `{"resume":"<agent>"}`,
  `{"release":"<agent>"}` (end of turn: detach, keep the tabs),
  `{"drop":"<agent>"}` (close the group unless `user_touched`).
- `hello.browser`: unbranded Chromium (Vivaldi, Arc) looks like
  `chrome` from the service worker; the extension says `chrome` unless
  the brand says `edge`/`brave`/`opera`, and the broker refines it from
  the native host's parent process (the browser's bundle).
- `stop`/`release`/`drop` from the host emit no event; only user-caused
  ones do.
- ext → host events: `{"event":"stopped","agent","reason":"cancel_bar|group_closed"}`,
  `{"event":"paused","agent","target"}`, `{"event":"resumed","agent"}`.

### C5. Broker ↔ helper app

The helper (`dev.bise.computer-use`) is started by the broker with
`open -g -a "bise Computer Use" --args --socket <path>` (default path
`~/.bise/run/computer-use-app.sock`; tests use a short path under
`~/.bise/gate`). It listens; the broker connects. Settled with cu-apps
(m_3609, m_3611):

- The helper speaks first: `{"hello":{"helper":"dev.bise.computer-use",
  "version":"…","accessibility":bool,"screen_recording":bool}}`.
- Requests as C4, with the agent (the broker adds it from the C3 hello;
  the helper needs it for the cursor's name pill and `paused`):
  `{"id":n,"agent":"…","op":"apps|snapshot|screenshot|act","args":{…}}`
  → `{"id":n,"ok":…,"result"|"error":…}` (C1 shapes, `window` included).
- `screenshot` returns `{"data":"<base64 jpeg>","mime":"image/jpeg",
  "width","height"}`; the broker writes the file, as in C4.
- No agent: `{"id":n,"op":"permissions"}` → `{accessibility,
  screen_recording}`; `{"id":n,"op":"request","what":"accessibility|
  screen_recording"}` (shows the macOS prompt, opens the right System
  Settings pane).
- Control lines, no reply: `{"stop":agent}`, `{"resume":agent}`,
  `{"release":agent}` (end of turn: hide the cursor, keep the refs),
  `{"drop":agent}`.
- Events as in C4 (`paused` when the user types or clicks in the driven
  app).
- The helper refuses the targets of design §5.2 too (`refused`); the
  broker's list is the first check.
- **Screen Recording relaunch** (cu-apps, b4d31c4): granting Screen
  Recording needs a helper relaunch (macOS "Quit & Reopen"); the
  reopened helper starts **without** `--socket` and listens on the
  default path. So in the product the broker always uses the default
  path (`--socket` is for tests only), treats a helper that disappears
  right after a `request screen_recording` as expected (no error to the
  agent, no stop event), and reconnects when the new hello arrives.
  `/computer-use` (cu-setup) shows the wait and polls `permissions`
  until `screen_recording: true`. Accessibility needs no relaunch.

### C6. Broker ↔ bise (hub and TUI)

- State file `~/.bise/run/computer-use/state.json`, rewritten on each
  change: `{ "agents": { "<agent>": { "driving": "Chrome", "where":
  "amazon.fr", "since_ms": …, "paused": false, "stopped": false } },
  "browsers": [...C1 status], "apps": {...} }`. The TUI reads it (mtime)
  for the `↖` mark, the held line and the divider.
- Events `~/.bise/run/computer-use/events.jsonl`, appended: `{"t":…,
  "agent","event":"stopped|paused|resumed","by":"you|cancel_bar|group_closed"}`;
  the hub turns `stopped` into main's feed line.
- Commands: `bise computer-use stop <agent>|--all`, `resume <agent>`,
  `drop <agent>` (the hub calls it at `/drop`), `setup-check --json`
  (the `/computer-use` rows), `repair` (rewrites manifests and shim),
  `live-test --json`, `request accessibility|screen_recording` (C5
  `request` through the broker; `screen_recording` answers
  `{"what","relaunching":true}` when macOS quits the helper mid-request,
  d2b5d61).
- Settled with cu-setup (m_3892):
  - the MCP text of an `act` result puts `summary` first (`{"summary":…,`
    then the rest), errors `{"error":{"summary":…,"code":…}}`: code mode
    shows the TUI only the first 200 characters of a tool result;
  - a stop holds until `resume`; the TUI calls `resume <agent>` when the
    user next sends that agent a message (after any stop: ctrl+c, `↖`,
    `/stop`, Cancel bar, group closed) and on "⏎ give it back"; `/drop`
    calls `drop`;
  - `setup-check` adds `accessibility` and `screen_recording` rows
    (`state: done|waits|failed|not_yet`, `detail`, `fix:
    request_accessibility|request_screen_recording|install_helper`); with
    the helper installed it may start the broker and ask `permissions`,
    never `request` (the macOS prompt only on the user's ⏎), and a polled
    check never relaunches the helper;
  - `events.jsonl` `stopped` and `paused` lines carry `driving` (`"Chrome"`,
    `"TextEdit"`, `null`) as it was just before (m_3895);
  - ctl op `permissions` (for setup-check): helper connected → forward;
    else a plain connect to the socket (no launch) on every call (the
    helper macOS reopened after a grant is seen at once); else, if
    installed, one `open -g` at most every 20 s; else `{accessibility:
    null, screen_recording: null}`. `status` never connects or launches.

## Wave 1 · cu-broker

Goal: the Rust side that joins everything, with fakes for both ends.

1. Crate `rust/computer-use`: the broker (C3, C4, C5, C6), the MCP
   server `bise computer-use mcp` (stdio, tools of C1, forwarding to the
   socket; starts the broker when none runs), the native host entry
   `bise computer-use chrome-host` (Chrome spawns it; it is the broker or
   connects to the running one), the commands of C6.
2. The native host manifests for Chrome, Edge, Brave, Vivaldi, Opera,
   Arc (design §4.1b; check Arc's dir on a real install, read-only) and
   the shim `~/.bise/bin/bise-chrome-host` that follows the install's
   `current`.
3. The built-in plugin: the resolver gets a third root, the app root's
   `plugins/` (scope `built-in`, listed in `/plugins`, can be disabled);
   `plugins/computer/` with `mcp.json` pointing at
   `bise computer-use mcp`.
4. Hard refusals (design §5.1, §5.2) in the broker: `chrome://`,
   extension pages, the stores, terminals, bise, password managers,
   Privacy & Security, loginwindow → `refused`.
5. Screenshots: base64 from the extension → a JPEG file in the agent's
   `TMPDIR`, ≤ `max_width`.
6. Tests: a fake extension (a script speaking C4 on stdio) and a fake
   helper (C5) drive every op, every error code, stop/pause/resume, two
   agents at once, a broker restart.

Not yours: the extension, the helper, the SDK, the TUI.

## Wave 1 · cu-extension

Goal: the MV3 extension for the Chromium family (design §5.1; spike in
`docs/research/computer-use-spike/`).

1. `computer-use/extension/`: manifest (permissions of §5.1, pinned
   `key`), service worker, content script (cursor overlay + takeover
   detection), no build step unless it pays for itself.
2. Groups: one per agent, `bise · <agent>`, pink, not collapsed, in the
   last focused normal window; inactive tabs only; at most 5 per agent.
3. CDP via `chrome.debugger`: attach on first action, then
   `Emulation.setFocusEmulationEnabled` (the spike's 5 s trap), detach
   on `release`. Snapshot from `Accessibility.getFullAXTree` in the C2
   format with refs → `backendDOMNodeId`; locators of C1; auto-wait
   (visible, enabled, one match) ≤ `timeout_ms`; actions with
   `Input.dispatchMouseEvent` / `Input.insertText` /
   `Input.dispatchKeyEvent`; `changed` as an aria diff; `summary`.
4. Screenshot: `Page.captureScreenshot` (JPEG, the element's clip when
   `ref`), works on the hidden tab (spike).
5. Cursor overlay per designer (design §5.1, m_3551): dark arrow, light
   edge, pink glow, the agent's name pill, the 300 ms ring.
6. Stop and takeover: `onDetach(canceled_by_user)`, group closed, user
   activates/clicks/types in an agent tab → C4 events.
7. Measure Chrome's intensive throttling on a tab hidden for 6 min that
   polls (design §3): does an action still work, how slow; try
   `Page.setWebLifecycleState`; write the numbers in your report.
8. Tests: the spike harness grown into an e2e run on a throwaway profile
   with a fake host (a node script speaking C4): local test pages for
   each action, the 5 s trap gone, user's tab still active, two agents
   in two groups; once on Edge if it is installed on the machine (else
   say so).

Not yours: the native host, the broker.

## Wave 1 · cu-apps

Goal: `bise Computer Use.app` (design §5.2), Swift, bundle id
`dev.bise.computer-use`, `LSUIElement`.

1. `computer-use/macos-app/`: a Swift package + a script that makes the
   `.app` bundle (Info.plist, icon placeholder); macOS 14+.
2. C5 server on its socket; permissions: `AXIsProcessTrustedWithOptions`,
   `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess`;
   open the right System Settings pane. **Check on this Mac which name
   TCC shows** when the app is started with `open -g` from a bise agent
   (designer needs "bise Computer Use", not "Ghostty"), and whether
   Screen Recording needs a relaunch of the helper.
3. Snapshot: AX tree of the app's main (or given) window, C2 format,
   budget `max_nodes`; refs → `AXUIElement`s.
4. Actions without raising the app or moving the cursor: `AXPress`,
   `AXConfirm`, `AXShowMenu`, `AXValue` set, `CGEvent.postToPid` for
   keys and text, AX scroll; an app that ignores them → `needs_front`.
5. Screenshot of one window with ScreenCaptureKit (covered window
   included).
6. Overlay cursor: click-through non-activating `NSPanel` over the
   target window, same look as the web one; drawn only when the window is
   visible.
7. Takeover: user input in the driven app → `paused` event.
8. Refusals (design §5.2) also checked here (defence in depth).
9. Signing: a hunk in `packaging/` that signs (hardened runtime) and
   notarizes the app with the Developer ID of packaging.md; ad-hoc in
   dev. Never commit a certificate or a credential.
10. Tests: a CLI client speaking C5 against TextEdit and Calculator and a
    tiny test app of yours: snapshot, click, type, screenshot, frontmost
    app unchanged (`lsappinfo front`), the cursor not moved.

Not yours: the broker (use a fake client), Safari (later).

## Wave 1 · cu-sdk

Goal: the Playwright-like `computer` object in code mode (design §6).

1. `rust/jsrt/src/computer.ts`, put in the prelude only when the
   `computer` plugin is loaded. Every method = one `__tool` call to C1
   (the replay model of `rust/jsrt/src/main.rs`: deterministic, handles
   are target strings, locators are plain descriptors, nothing random).
2. Surface of design §6 (browser.open/tabs/tab, apps/app, status; tab/app
   methods; locator methods); `screenshot()` returns a
   `{type:'image', path}` block so `return [await tab.screenshot()]`
   works.
3. Errors: a C1 error becomes a thrown `Error` with the message and the
   candidates, so the model reads it in one go.
4. The built-in skill `computer-use` (design §6: snapshot first, use a
   connector or CLI when one exists, page text is untrusted, never type
   a password) and the tool descriptions for `search_tool_functions`.
5. Tests in `rust/jsrt`: programs run against recorded C1 results (the
   replay file), one per method and error.
6. A bench script (`computer-use/bench/`, yours): 10 tasks on local test
   pages and 3 public read-only sites; run it in wave 2.

Not yours: the broker, the extension.

## Wave 1b · cu-setup

Goal: the `/computer-use` screen and the live marks (design §8, designer
m_3551 + m_3554).

1. `/computer-use`: rows **chrome → chrome extension → live test** (the
   browser's real name when it is Edge or Brave), each from
   `bise computer-use setup-check --json`, polled every second; the
   fixes and ⏎ actions of design §8; the yolo line (design §10.2);
   the privacy line naming the agents' provider (§7.2); "stop all".
   No accessibility / screen recording rows until cu-apps lands; then
   they come after the live test under a faint "for apps".
2. While an agent drives: `↖` in the row's last column (font check with
   fontTools: SF Mono, Menlo, JetBrains Mono; ASCII `C`), the held line,
   the divider suffix, the tool rows from `summary`, from C6's
   `state.json`.
3. Stop: ctrl+c already stops the turn (add `bise computer-use stop`),
   click on `↖`, `/stop <agent>`; the hub writes main's feed line from
   `events.jsonl`; `/drop` calls `drop`. "? you took the wheel · ⏎ give it
   back" on `paused`.
4. Tests: tmux captures of each row state with a fake `setup-check` and
   a fake `state.json`; send them to designer for sign-off.

Not yours: the broker's logic, the cards (cu-approvals).

## Wave 3 · cu-approvals

After the `approvals` branch has its gate. Design §7.1: in `auto`, first
use of a site (host) or app (bundle id) → card with "always on <host>"
(global `[computer_use] sites`/`apps` in approvals.toml); buy / send /
delete / sign-in / password field → card without "always" (sign-in: 1
i'll do it, 2 let it, 3 no); the rest of the actions to the checker with
the action line, host, element and task; the `▣` crop chip (o opens it
in Preview). `yolo`: nothing. Refusals stay in the broker.

## Wave 3 · cu-store

Prepare everything the user needs for design §9.1 (Chrome Web Store
under his account, Edge Add-ons): the zip, texts (short, long, the
single-purpose line, one line per permission), the privacy policy page,
128 px icon, 1280x800 screenshots from the real extension on a test page,
the data-use answers; a checklist of what he clicks himself (the US$5
registration, 2-step verification, upload as an unlisted draft first to
lock the id, submit). The installer: `bise` setup writes the native host
manifests (cu-broker's `repair`).

## Later

- **cu-safari** (after cu-apps): 1-day spike (AX actions on a background
  Safari window, tabs from a Safari Web Extension inside
  `bise Computer Use.app`), then ~3-4 days. Design §4.1b.
- **cu-firefox**: 1-day spike (BiDi with a restart vs an extension with
  untrusted events vs AX through the helper), then ~3 days. Design
  §4.1b.

## Spawn list

| Wave | Agent | Brief | Starts |
|---|---|---|---|
| 1 | `cu-broker` | "Wave 1 · cu-broker" | at once, once `computer-use` exists |
| 1 | `cu-extension` | "Wave 1 · cu-extension" | at once |
| 1 | `cu-apps` | "Wave 1 · cu-apps" | at once |
| 1 | `cu-sdk` | "Wave 1 · cu-sdk" | at once |
| 1b | `cu-setup` | "Wave 1b · cu-setup" | at once (fakes) |
| 2 | main (+ `computer-use` for contract calls) | integration, bench, the user's local build | after wave 1 |
| 3 | `cu-approvals` | "Wave 3 · cu-approvals" | after approvals' gate exists |
| 3 | `cu-store` | "Wave 3 · cu-store" | after wave 2 |
