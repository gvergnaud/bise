# MCP Apps in bise (views)

Status: being built on the feature branch `mcp-apps` (task
`mcp-apps-lead`). Plan: page `mcp-apps-plan` v9 and Gabriel's six
decisions; review: page `mcp-apps-review` v2; build status: page
`mcp-apps-build`. Spec: [MCP Apps, SEP-1865, 2026-01-26](https://modelcontextprotocol.io/extensions/apps/overview).

A *view* is the HTML UI an MCP tool declares (`_meta.ui.resourceUri:
"ui://…"`) or a Vibe plugin ships (`ai.mistral.vibe/views/<name>/`). bise
shows it inline under its tool row in the thread and as a panel tab, in a
sandbox of its own origin; the view can call its own server's tools,
write to the agent, and use the mic, camera or MIDI when it declared them
and Gabriel accepted them when he added the plugin.

This page is the contract between the pieces: each piece's agent builds
its side against it, so the pieces can land in any order. A change to a
contract below is a change to this page first, in the same commit.

## Gabriel's decisions (mcp-apps-plan v9)

| question | his choice |
| --- | --- |
| q-origin | each view is served by the hub's pages server on `v-<hash>.localhost:<port>`: one sandbox for the app and the browser |
| q-calls | `app`-only tools of the view's own server: free; tools the model sees too: the agent's approval mode, like its own calls |
| q-message | `ui/message` goes at once and shows "via <plugin>" |
| q-tui | the TUI shows the tool's text, `o` opens the view in the app, `O` in his browser (a web host served by the hub) |
| q-vibe | the spec only: `mistral/*`, connector tools and Le Chat's look are not supported; such a call answers a clear error, the rest of the view works |
| q-perm | mic, camera, MIDI from the start, per view, when declared and accepted at the plugin's add |

Le Chat's code was read to understand, nothing is copied; the host, the
proxy page and the CSP are written from the public spec. The public SDK
`@modelcontextprotocol/ext-apps` (MIT) may be used on the host side.

## Fixtures (piece 0)

`tests/fixtures/mcp-apps/`, every test's fake data:

- `fake-app/`: stdio server. `show_counter` (view `ui://fake-app/counter`,
  `structuredContent: {count}`), `bump` (`visibility: ["app"]`, the count
  lives in the server's memory), `whisper` (`visibility: ["model"]`),
  `show_hostile` (view `ui://fake-app/hostile`), `slow` (`ms`). Without
  the UI extension in `initialize` it lists no `_meta.ui`.
  `$FAKE_APP_LOG` logs each method and tool called.
- `fake-app/views/hostile.html`: every escape a view can try (top
  window and its document, cookie, undeclared fetch/img/frame (a frame
  counts as blocked when CSP fired its `frame-src` violation), `<base>`, popup,
  top navigation, spoofed host messages, model-only tool, other server's
  tool, `mistral/*`, undeclared mic/camera/MIDI/clipboard, a 200-message
  flood). Each probe is a `[data-probe][data-result=blocked|leaked]` row;
  `body[data-done]` when all ran. A host is safe when no row says
  `leaked` (the counter of accepted flood messages must stay ≤ 5).
- `fake-views/`: a plugin with one static Vibe view
  (`ai.mistral.vibe/views/hello`), no server.
- `fake-score/`: imitates the piano composer: `compose` (score in
  `structuredContent`), `set_notes` and `export` (app only; `export`
  answers a base64 `audio/midi` resource for `ui/download-file`); its
  view also calls `mistral/complete`.
- `check.py`: validates any plugin folder against the spec's shapes
  (`python3 tests/fixtures/mcp-apps/check.py [dir…]`). The ten example
  plugins pass it too.

## The path of a view

```
model ─ tool call ─▶ REPL (mcp.bend) ─ POST + x-bise-call ─▶ bridge ─▶ server
                         │                                      │
                         │ tool_view #<id> : {json}             └ full result ▶ <hub>/agents/<dir>/views/<call>.json
                         ▼
            hub fold: ToolItem.view ─ thread/entry ─▶ app, TUI
app card ─ view/resource ─▶ hub ─▶ html + csp + permissions + input + result
app card ─ iframe v-<hash>.localhost:<port>/mcp-app (proxy) ─ inner iframe (srcdoc, CSP) = the view
view ─ ui/* postMessage ─▶ proxy ─▶ app host (views/) ─ view/call, view/message, view/context ─▶ hub
hub ─ view/call ─▶ visibility (rust/plugins) ─▶ approvals (off-turn entry) ─▶ agent's bridge, or the hub's views bridge
```

## Contracts

### C1. The tool index line (piece 1 writes, piece 3 reads)

Today: `<cid> <group> <tool> : #<desc> | input: <schema>`. A tool with a
view gets ` | ui: <uri>` after the schema (the last field; `uri` has no
space: percent-encoded if needed). An `app`-only tool is never in the
index (the spec forbids showing it to the model). A Vibe static view is
an index line on the plugin's virtual server `<cid>` = the bridge's
`/<plugin>~views` route, tool name = the view's `name` with `-` → `_`.

### C2. The full result (pieces 2 and 3)

- The REPL sends `x-bise-call: <call>` on a `tools/call` POST whose index
  line has `ui:`; `<call>` = `<turn-start epoch ms>-<tool id>`
  (`[0-9]+-[0-9]+`, unique in an agent's dir across REPL restarts).
- The bridge writes the whole JSON-RPC `result` to
  `Paths::agent_views_dir(<dir>)/<call>.json` as
  `{"v":1,"plugin","server","uri","tool","args":{…},"result":{…},"at_ms"}`,
  atomically, at most 4 MiB (bigger: `result` replaced by
  `{"truncated":true}` and the view shows "result too large").
  The REPL never builds that path: the hub gives it in an Internal
  variable declared in `bise_home::env::VARS` (its row and
  `bise_env.py`), filled from `Paths::agent_views_dir` when the REPL is
  created; the REPL passes it to the bridge as is (`--views-dir`). A
  headless session without a hub: none, nothing written.
- The prune has one owner: the daily images prune
  (`switchboard::image_care`, `keep_days`) takes `views/` too; no second
  loop.
- The model's text is unchanged (`res_text`).

### C3. The wire line (piece 3 emits, piece 4 reads)

`tool_view #<id> : {"plugin":"…","server":"…","uri":"ui://…","call":"…"}`,
right after the `tool_result #<id>` line of the same call. A new `Rec`
(`Rec::ToolView { id, view: ToolViewRef }`), published under the wire
rule (`tests/released.rs`, fixtures): an older client drops the line.
The fold attaches it by `#<id>`, never by adjacency (another line can
come between the two; a test has one there).

### C4. The thread entry (piece 4)

`ToolItem.view: Option<ToolView>` with `ToolView { plugin, server, uri,
call, state }`, `state: "live" | "expired"` (expired: the file is gone;
the hub says so when `view/resource` finds no file, and the fold marks
it on replay). TS types from `tests/ts.rs`.

### C5. Client methods on hub.sock (piece 5; docs/client-protocol.md)

All take `{"agent": "<name>", "call": "<call>"}` plus:

| method | params | result |
| --- | --- | --- |
| `view/resource` | — | `{origin, html, csp, permissions, prefersBorder, toolInput, toolResult, tool}`; `-32030` expired |
| `view/call` | `method: "tools/call" \| "resources/read"`, `params` | the server's result; errors below |
| `view/message` | `content` (spec content blocks, text only at first) | `{}` once the message is in the thread |
| `view/context` | `content` | `{}`; replaces this view's context for the next send |

`view/call` errors (JSON-RPC, `data.reason`): `-32031 not-visible`
(model-only tool, other server, unknown tool), `-32032 plugin-off`,
`-32033 denied` (he refused in the approval card), `-32034
not-supported` (`mistral/*`, connector tools: "not supported in bise",
decision q-vibe), `-32035 rate-limited`, `-32036 server-restarted` (the
call ran, on a fresh server: in-memory state is gone; said once).

Never an agent op: the views methods are on hub.sock only (an agent's
`sb` cannot call them; issue 16, a test that an agent's `sb` is refused).

`view/message` also carries `gesture_ms`: when Electron's main process
last saw a pointer-down inside that view's frame rect (C7). The hub
decides with it (risk 1); the renderer and the proxy never do. The
renderer sends the core cmd `{"cmd": "view_message", "agent", "call",
"content"}`; Electron main (`core/ipc.ts`, on `CORE_IPC.cmd`) sets
`gesture_ms` from the last `input-event` mouseDown inside that
`(agent, call)` frame's rect, or removes it when there is none (a
`gesture_ms` from the renderer is never kept); the core passes it on as
`view/message {agent, call, content, gesture_ms}`. `view_context` goes
the same way without `gesture_ms`.

Signed by architect (m_50047), with these owners:

- off-turn approval cards are sb-core cards: a kind declared in
  `cardkind.bend` (or `approval` with a typed place `view:<agent>/<call>`),
  opened and closed by the existing entries, with laws: never in the
  agent's `waiting` slot, an answer touches only its card, both answer
  orders give the same state (ALL PROOFS CHECK + `old_core_replay`);
- the limits (30 `view/call` a minute, 1 `view/message` every 10 s, the
  5 s gesture window) are a pure shell function per view with its tests,
  like a throttle (`StateGate`): they decide no wake;
- the "via <plugin>" author is the message's sender, so sb-core: a typed
  sender (like the pseudo-senders bise and github, `Party`), never
  `you`, never a phrase; delivered to that view's agent only, `queued`
  (never steer). Laws: never `from = user`, one delivery, the right
  recipient. The prompt wraps it like a page note, and the auto mode's
  approval checker never counts it as his words (a test: "yes do it" in
  a `ui/message` approves nothing);
- `view/context` is the `Part` agreed in m_49840, kept by the hub, lost
  on a hub restart and said so.

### C6. Origins and the pages server (piece 6)

- A view's origin: `http://v-<hash>.localhost:<pages port>`, `<hash>` =
  16 hex of sha256(`<plugin>\0<server>\0<uri>`): one origin per view, the
  same across calls (its storage persists like a site's), never shared
  between two plugins.
- On a view host the pages server answers `GET /mcp-app` (the proxy page)
  and nothing else: `/api`, `/p`, `/kit`… answer 403. On the main host
  `/mcp-app` is 404. `host_ok` accepts `v-[0-9a-f]{16}.localhost:<port>`
  exactly (no other subdomain: the DNS-rebinding guard stays).
- The proxy page (ours, from the spec's "sandbox proxy" section): checks
  it is cross-origin with its parent, sends
  `ui/notifications/sandbox-proxy-ready`, takes
  `ui/notifications/sandbox-resource-ready {html, csp, permissions}` from
  its parent only, builds the inner iframe (`srcdoc`, `sandbox="allow-scripts
  allow-same-origin allow-forms"`, `allow=` from the granted permissions)
  and relays JSON-RPC both ways, checking `event.source` each way.
- The CSP is a pure function (`rust/switchboard/src/pages/view_csp.rs`):
  bise's floor (`default-src 'none'`, `script-src 'unsafe-inline'`,
  `style-src 'unsafe-inline'`, `img-src data: blob:`, `media-src data:
  blob:`, `font-src data:`, `connect-src 'none'`, `frame-src 'none'`,
  `base-uri 'none'`, `form-action 'none'`) plus the declared origins only,
  each directive per the spec (`connectDomains` → connect-src,
  `resourceDomains` → img/script/style/font/media-src, `frameDomains` →
  frame-src, `baseUriDomains` → base-uri). An entry that is not an origin
  (path, `*` alone, `data:`, a `'keyword'`) is dropped. `domain` is
  ignored (said in the plugin's page): a stable origin on localhost is
  not possible.
- Desktop: `main/guard.ts` and the webRequest filter accept the view
  hosts of the current pages port; `setPermissionRequestHandler` grants
  `media` (audio, video), `midi`, `midiSysex: never`, `clipboard-sanitized-write`
  to a view origin only when its resource declared it and the plugin's
  grant (C8) has it; everything else denied. MIDI is not in the spec's
  permissions: a view declares it in bise's own key next to `ui` on the
  resource, `_meta["dev.bise/ui"]: {"permissions": {"midi": {}}}` (other
  hosts ignore it; the spec's `_meta.ui.permissions` stays clean), and
  its view should degrade when `requestMIDIAccess` is refused.
  The pages server knows each origin's sandbox: the hub registers
  `ViewSandbox {csp, perms}` (`pages/views.rs`) when it answers
  `view/resource`, and serves `/mcp-app` with that CSP as its header
  (never from the query: a view can navigate its own frame); an unknown
  hash is 404. The layer tells Electron main its live frames
  (`window.bise.views.frames([{origin, agent, call, rect, perms}])`, the
  whole list on each change); main grants a request from `origin` only
  within its `perms`. The camera needs
  `com.apple.security.device.camera` in
  `packaging/entitlements/desktop.plist` and `NSCameraUsageDescription`
  (the mic's are already there).

### C7. The desktop host (pieces 7, 8; apps/desktop/src/views/)

- The ui/* protocol lives in `apps/desktop/src/views/` (never in
  `FRAME_KINDS`). The host answers a message only when `event.source` is
  that view's proxy window and `event.origin` its view origin.
- Host capabilities announced: `openLinks`, `downloadFile`,
  `serverTools`, `serverResources`, `logging`, `message` (text),
  `updateModelContext` (text), `sandbox {permissions, csp}` (what was
  granted). Not announced: `sampling`.
- Host context: `theme`, `styles.variables` (our tokens mapped on the
  spec's CSS variables), `displayMode`, `availableDisplayModes: ["inline",
  "fullscreen"]`, `containerDimensions`, `locale`, `timeZone`,
  `platform: "desktop"`, `toolInfo`.
- Order: nothing before `ui/notifications/initialized`; then
  `tool-input`, then `tool-result` (or `tool-cancelled` when the call
  failed or was stopped); `ui/resource-teardown` before an unmount.
- Display modes: `inline` (card under the tool row) and `fullscreen` (a
  panel tab, 640 wide). The iframe is never moved: one stable layer keeps
  every live view and places it over its card or its tab. `pip` is not
  offered.

### C8. The plugin's grant (piece 10)

At add (`Add.tsx`, TUI `plugins_add.rs`) the screen lists the views,
their domains and their permissions; "add and run it" stores the grant in
`plugins.json` as parts `<plugin>#view:<uri>` with `perms: [...]`. A view
not granted (a server that adds one later) runs with no permission and
its card says so.

### C9. The web host (piece 11)

`GET /v/<agent>/<call>` on the main pages host serves the host page (the
same `views/` host code, built for the browser); its calls go to `POST
/api/view` (`x-bise-token` and `Origin` checked like every `/api` route)
with C5's methods in the body. The TUI's `O` opens that URL; `o` asks
the app (`app/open` with the entity `view:<agent>/<call>`).

## The three risks (mcp-apps-review v2) and how each is handled

1. **The security chain view → tools → agent.** A view is third-party
   JS that can call tools without a click and write to the agent.
   - Visibility first, a pure function with its tests
     (`rust/plugins/src/visibility.rs`, piece 1): only tools of the
     view's own server; `["model"]`-only refused; plugin or tool off
     refused; `mistral/*` and connectors `not-supported`. It lands
     before any `view/call` can run.
   - Then approvals: tools the model also sees go through the agent's
     approval mode (q-calls), in an off-turn entry of the gate (risk 3);
     the card says "<plugin> view wants to run <tool>".
   - **The proxy is not a security boundary; the origin is.** The proxy
     page and the view share one origin (`srcdoc` + `allow-same-origin`):
     the view can run code in the proxy and forge anything it reports.
     Every rule is enforced in the hub (visibility, approvals, limits,
     the gesture window) and in the host's origin check. Never add a
     check to the proxy believing it protects anything.
   - Limits in the hub: at most 30 `view/call` a minute per view,
     `view/message` at most 1 every 10 s per view and only within 5 s of
     a pointer-down inside that view's frame, as seen by Electron's main
     process (the webContents' input event, its position in the frame's
     rect), sent typed as `gesture_ms`: a view cannot send a message on
     its own, and a pointer-down it forges unlocks nothing (a hostile
     test); `-32035` past it.
   - A view's message is never the user's: its sender is `via <plugin>`
     (a typed sb-core sender, not a phrase), queued to its agent only,
     wrapped in the prompt like a page note, and never counted as his
     words by the auto mode's approval checker.
   - The hostile view is the test: Rust and vitest parts before each
     land, its Playwright check on the `mcp-apps` tip after each sandbox
     land and before `sb feature ready`: a leak never reaches main.
   - architect signs the seams (sandbox, view/call, ui/message,
     permissions) before the pieces 5b, 6 and 9 land.
2. **The fixed iframe in the redesign.** The redesign is on main
   (native-mac merged 10-26); placement agreed with native-lead
   (m_50044):
   - one layer for the whole window, mounted once in `app/` next to the
     Frame's columns, so an iframe never changes parent between the
     thread and the tab. Its container is `position: absolute` with no
     `z-index` (no stacking context): a thread view takes z 2 (under the
     panel's floating sheet below 1176 px), a tab view z 4; bise's
     floats (15), separator (30), scrim (40-41), tips and dropdowns stay
     above;
   - each frame sits on its anchor's rect, clipped to its column's
     visible rect (`clip-path: inset(…)`), read by `ResizeObserver`, the
     thread's scroll and `requestAnimationFrame`;
   - the anchors (all in `views/`): one mount line in `ThreadRows.tsx`
     that registers in the views store and reserves the card's height
     (the height the view announces, through the row so Virtuoso and
     scroll-steady measure it right), one in the `view` branch of
     `Thing.tsx` (content `.pn-in`), plus a `view` kind in
     `shared/entity.ts` and `look.tsx`. Those mount lines are declared
     exceptions in their commits;
   - an anchor unmounted by Virtuoso hides its frame (`visibility`),
     never removes it, up to 8 live views; past that the oldest is torn
     down (`ui/resource-teardown`) and remounts from its stored result;
   - `mcp-apps` merges main often (`sb feature sync mcp-apps`): main
     still changes `ThreadList` and `PluginPage`;
   - designer signs the card and the tab, ui-review shoots them light
     and dark at 1440 and 1100 (sheet);
   - fallback if the layer misbehaves under the panel: the tab reloads
     the view and replays input and result (local state lost).
3. **Calls outside a turn in the gate and the bridge.**
   - The gate gets a second entry for view calls: `approvals::judge`
     unchanged, no gate file and no runtime nonce, its own cards keyed by
     `(agent, call, n)`, never in the agent's `waiting` slot (the agent's
     own pending call is untouched). Daemon tests on a throwaway hub: a
     view call waiting while the agent's own call waits; both answered
     in either order.
   - The hub calls the agent's bridge when its REPL lives (the bridge's
     loopback port is in the session dir), else a views bridge the hub
     owns (`bise plugins serve --views --parent <hub pid>`): one per hub,
     started on the first need, stopped after `IDLE_STOP` with no view
     call and with the hub; its pid file in `Paths` so a restarted hub
     kills an orphan.
   - The agent's own path changes by one header in `mcp.bend`: Bend
     tests of the header and a scripted probe that every plugin tool
     still answers the same text.

## Build order and who builds what

Pieces of the plan, with the review's corrections (5 is split in two;
6, 8 and 11 are bigger than planned):

| # | piece | depends on | lane |
| --- | --- | --- | --- |
| 0 | fixtures, hostile view, check.py | — | lead |
| 1 | the bridge knows views (resolve views, announce the extension, `_meta.ui`, index `ui:`, hide app-only, Vibe static views, resource cache, visibility fn) | 0 | server |
| 2 | full result kept (C2) | 1 | server |
| 3 | runtime emits the view (header, C3) | 2 | server |
| 4 | the thread carries the view (C4) | 3 | server |
| 5a | `view/resource` | 4 | server |
| 5b | `view/call` + off-turn gate entry + views bridge + limits | 5a | server |
| 6 | sandbox origin, proxy page, CSP, guard, permissions, entitlements | 0 | sandbox |
| 7 | desktop host (ui/*, inline card) against FakeCore | 0 | host |
| 8 | display modes, the stable layer, panel tab | 7 | host |
| 9 | `view/message`, `view/context`, author "via" | 5b, 7 | server + host |
| 10 | plugins place: VIEWS group, add screen, grant (C8), TUI | 1 | host |
| 11 | TUI fallback and the web host (C9) | 5b, 6 | sandbox |
| 12 | Vibe compat errors (`not-supported`) | 5b | server |
| 13 | e2e, Playwright on the feature tip, QA | all | lead |
| — | the ten example plugins (`examples/plugins/`) | 1 | lead |
