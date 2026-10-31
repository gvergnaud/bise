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
  top navigation, model-only tool, other server's tool, `mistral/*`,
  undeclared mic/camera/MIDI/clipboard, a 200-message flood, a 40-call
  flood), and three controls that must work (an app-visible call, the
  declared fetch, a blob frame of its own). Each is a
  `[data-probe][data-kind=probe|control][data-result]` row; a probe says
  `blocked` only for the refusal the host owes it (its JSON-RPC code and
  reason, a CSP violation of its directive, a SecurityError, a
  permissions-policy refusal), `broken` for any other failure (no answer,
  a 503), `leaked` when it got through; a control says `ok` or `broken`.
  `body[data-done]` when all ran. A host is safe when every probe says
  `blocked` and every control `ok`.
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
space: a space, a control character, `|` and `%` percent-encoded,
`views::ui_field`; the views file (C2) keeps the server's own URI). An
`app`-only tool (`_meta.ui.visibility` without `"model"`) is never in the
index (the spec forbids showing it to the model) but stays callable on
its server's route. A Vibe static view is an index line on the plugin's
virtual server `~views`: `<cid>` = `<base>/<plugin>/~views`, tool name =
the view's `name` with `-` → `_`, URI `ui://<plugin>/views/<name>`; its
call answers its arguments as `structuredContent`, its `resources/read`
the `index.html` with `view.json`'s `ui` as `_meta.ui`. No mcp.json
server id may start with `~`.

The bridge announces `capabilities.extensions["io.modelcontextprotocol/ui"]
= {mimeTypes: ["text/html;profile=mcp-app"]}` at `initialize` (stdio and
remote), keeps each server's `ui://` resources once per start, and
writes `D/bridge.json` = `{"v":1,"base":"http://127.0.0.1:<port>/<token>","pid"}`
before `ready` (`D` = `$BEND_RUN_DIR/<repl port>/plugins`): the hub
reaches a server at `<base>/<plugin>/<server>` (a raw `tools/list` there
keeps `_meta.ui` and the app-only tools). `GET <base>/<plugin>/~ui`
(`bridge::UI_ROUTE`) lists the plugin's views as its servers declare them
now, for the add screen (C8): `{"views":[{"server","tool","uri","csp",
"permissions","bise_permissions" (the resource's
`_meta["dev.bise/ui"].permissions`),"prefersBorder","ready","error"?}]}`,
one row per tool with a view (its static views last, server `~views`),
read through the `ui://` cache (a lazy server is started for it); 404 for
a plugin the bridge does not run. Code: `rust/plugins/src/views.rs`.

### C2. The full result (pieces 2 and 3)

- The REPL sends `x-bise-call: 1` on a `tools/call` POST whose index
  line has `ui:` (a plugin tool on its loopback bridge only;
  `Mcp.asks_view`).
- The bridge picks the call id `<call>` = `<its now ms>-<n>` (the first
  `n` whose file is free, taken by an exclusive create:
  `[0-9]+-[0-9]+`, unique in an agent's dir across REPL restarts, two
  views of one `run_typescript` get two ids) and writes the whole
  JSON-RPC `result` to `Paths::agent_views_dir(<dir>)/<call>.json` as
  `{"v":1,"plugin","server","uri","tool","args":{…},"result":{…},"at_ms"}`,
  atomically, at most 4 MiB (bigger: `result` replaced by
  `{"truncated":true}` and the view shows "result too large"; args too
  big as well: `args` too). An error answer keeps `"error":{code,message}`
  instead of `result` (the view gets `tool-cancelled`). It answers the
  view's ref in the header `x-bise-view: {"plugin","server","uri","call"}`
  (ASCII JSON, `views::view_header`); a tool without a view, no views
  folder or a failed write: no header.
  The REPL never builds that path: the hub gives it in the Internal
  variable `BEND_VIEWS_DIR` (`bise_home::env::VARS`, `bise_env.py`),
  `<agent dir>/views` (`Paths::agent_views_dir`,
  `bise_home::hub_state::agent_views`), set with the agent's other
  folders (`switchboard::tools_env::temp_env`); the REPL passes it to
  the bridge as is (`--views-dir`, bend/runtime/plugins.bend). A
  headless session without a hub: none, nothing written. Code:
  `views::{call_ok, file_text, keep, view_header}`, `bridge::keep_for_view`.
- The prune has one owner: the daily images prune
  (`switchboard::image_care`, `keep_days`) takes `views/` too; no second
  loop.
- The model's text is unchanged (`res_text`), and its tool description
  has no `ui:` field (`Mcp.desc_only`).

### C3. The wire line (piece 3 emits, piece 4 reads)

`tool_view #<id> : {"plugin":"…","server":"…","uri":"ui://…","call":"…"}`
(the ref is the bridge's `x-bise-view` header as is), after the
`tool_result #<id>` line of the tool whose call it was: `#<id>` is the
id of the tool running when the MCP call started (a direct call's own
id, or the `run_typescript` that called it; two view tools in one
program: two lines with the same id). How (bend/runtime/mcp.bend's
header): the runtime writes the running tool's id to
`<BEND_AGENT_RUN>/bend-tool-<port>.txt` at each tool's start; mcp.bend
reads it at the call's start and appends `<id> <ref>` to
`bend-views-<port>.txt`; at each tool's end the runtime takes that file
whole (moved aside, read, removed) and emits one line per ref, each
under its own id (a stopped call that finishes later goes to its own
row). This holds while a REPL runs one tool at a time. A new `Rec`
(`Rec::ToolView { id, view: ToolViewRef }`), published under the wire
rule (`tests/released.rs`, fixtures): an older client drops the line.
The fold attaches it by `#<id>`, never by adjacency (another line can
come between the two; a test has one there); a ref whose row is gone is
dropped, never put on another row.

### C4. The thread entry (piece 4)

`ToolItem.views: Vec<ToolView>` (in the order of their lines; empty for
a tool with no view) with `ToolView { plugin, server, uri, call, state }`,
`state: "live" | "expired"` (expired: the file is gone; the hub says so
when `view/resource` finds no file). The fold reads lines only: it makes
every view `live`; the hub marks the expired ones before it sends
entries, with `bise_proto::thread::expire_views(entries, |call| <no
views file>)`. Code: `Rec::ToolView` / `ToolViewRef` (lines.rs, a ref
without its four strings, a call id not `[0-9]+-[0-9]+` or a uri not
`ui://`: dropped), `ToolMove::View`, `Fold::view` (by id, the same call
once). TS types from `tests/ts.rs` (`ToolView`, `ViewState` in
`apps/desktop/src/proto/thread.ts`).

### C5. Client methods on hub.sock (piece 5; docs/client-protocol.md)

The desktop reaches them through the app's request path as
`hub/view_resource`, `hub/view_call`, `hub/view_message` and
`hub/view_context` (answered in the request's own reply, no
`view_reply` event); Electron main stamps `gesture_ms` on
`hub/view_message` (below). The codes and reasons live in
`bise_proto::view_rpc`; the visibility rule in
`rust/plugins/src/visibility.rs` (landed 76d6f674b).

All take `{"agent": "<name>", "call": "<call>"}` plus:

| method | params | result |
| --- | --- | --- |
| `view/resource` | — | `{origin, html, csp, permissions, declared, prefersBorder, toolInput, toolResult, tool}` (`declared`: the permissions it asks, `permissions`: those granted, C8); `-32030` expired |
| `view/call` | `method: "tools/call" \| "resources/read"`, `params` | the server's result; errors below |
| `view/message` | `content` (spec content blocks, text only at first) | `{}` once the message is in the thread |
| `view/context` | `content` | `{}`; replaces this view's context for the next send |

And one per plugin, not per view (C8, the add's step 2): `plugin/views
{plugin}` (the app's `hub/plugin_views {project, plugin}`) asks the hub's
views bridge `GET <base>/<plugin>/~ui` and answers its `{views: [{server,
tool, uri, csp, permissions, bise_permissions, prefersBorder, ready,
error?}]}` as is; `-32032` when the plugin is off, `-32000 server-gone`
when the bridge can't answer.

`view/resource`'s `permissions` are `declared ∩ granted`: the grant from
`bend_plugins::state::view_grant(grants, plugin, uri)`, read at each
`view/resource` (never asked: none).

`view/call` errors (JSON-RPC, `data.reason`): `-32031 not-visible`
(model-only tool, other server, unknown tool), `-32032 plugin-off`,
`-32033 denied` (he refused in the approval card), `-32034
not-supported` (`mistral/*`, connector tools: "not supported in bise",
decision q-vibe), `-32035 rate-limited` (30 calls a minute, 1 message
every 10 s). `view/message` without his fresh click: `-32037 no-gesture`.
A call that ran on a server that is not the one before (a new bridge,
the hub's views bridge): its result as the server gave it, with
`_meta["dev.bise/server-restarted"]: true` on the first one (in-memory
state is gone; said once per view). The hub alone sets that key: a
server's own is overwritten (`view_call::mark_restarted`). Never an
error: the tool ran, maybe after his ok, and a view that ran it again
would run it twice (`-32036` was `server-restarted` before: never
reused).
Any method: `-32000 server-gone` when its server can't be reached (no
bridge, the bridge refused), and a server's own JSON-RPC error as it
gave it.

Never an agent op: the views methods are on hub.sock only (an agent's
`sb` cannot call them; issue 16: an agent's process is refused at
`initialize`, and agent.sock serves no `view*` op, the socket table's
test in `bise_proto::socket`).

As built (pieces 5a, 5b, 12): `rust/switchboard/src/daemon/views.rs` is
the shell's side, one dispatcher for hub.sock and the pages server's
`/api/view` (`Shell::view_request`); the pure parts are
`crate::view_call` (params, the views file, the bridge route, a
resource's html/csp/declared permissions, a message's text blocks) and
`crate::view_limits`. A call goes to the agent's own bridge while its
REPL's pid runs (its `repl.pid`, never only its port known: a REPL just
killed leaves a bridge that refuses as "turned off"), else to the hub's views bridge (`bise plugins serve
--workspace --parent <hub>` in `<state>/views-bridge`, started on the
first need, stopped when idle or with the hub). A model-visible call
goes through the approvals as the agent's own call would (`approvals::judge`:
allowed runs, a rule's no refuses `denied`, else a card); the card is
sb-core's `view_confirm_open`, its answer by card id, never the agent's
wait. A view's `view/call` waits 11 min in the ambient core
(`VIEW_CALL_KEPT`, not `ASK_LATE`), past the card's 10 min. Contexts are
joined to his next `turn/send` to that agent as `<view_context
plugin=… uri=…>` parts (`Part::ViewContext`, never from a client). The
thread's `ToolView.state` reads `expired` when its views file is gone,
at subscribe, page and each live entry (C4).

`view/message` also carries `gesture_ms`: when Electron's main process
last saw a pointer-down inside that view's frame rect (C7). The hub
decides with it (risk 1); the renderer and the proxy never do. The
renderer sends `hub/view_message {agent, call, content}` on the app's
request path; Electron main (`core/ipc.ts`) sets `gesture_ms` from the
last `input-event` mouseDown inside that `(agent, call)` frame's rect,
or removes it when there is none (a `gesture_ms` from the renderer is
never kept); the core passes it on as `view/message {agent, call,
content, gesture_ms}`. `hub/view_context` goes the same way without
`gesture_ms`.

The view's sender in sb-core is the typed field `Msg.view = <plugin>`
(`Msg.via` already names routes); off-turn view cards carry the place
`view:<agent>/<call>` and a due act that closes them as "timed out"
(architect m_50085).

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
  its parent only, builds the inner iframe (a `blob:` url of the html with
  the CSP in a `<meta>`, `sandbox="allow-scripts allow-same-origin
  allow-forms"`, `allow=` from the granted permissions) and relays
  JSON-RPC both ways, checking `event.source` each way. A blob, not the
  spec's `srcdoc`: both inherit the proxy's CSP header, but a srcdoc asks
  Electron for MIDI or the clipboard as `about:srcdoc`, with no origin,
  while a blob url carries the view's (checked in Electron 44). The host's
  outer iframe has the same `sandbox` (no `allow-popups`, no
  `allow-top-navigation`): Electron's window-open handler can't tell a
  view's `window.open` from the app's.
- The CSP is a pure function (`rust/switchboard/src/pages/view_csp.rs`):
  bise's floor (`default-src 'none'`, `script-src 'unsafe-inline'`,
  `style-src 'unsafe-inline'`, `img-src data: blob:`, `media-src data:
  blob:`, `font-src data:`, `connect-src 'none'`, `frame-src blob:` (the
  view's own frame), `base-uri 'none'`, `form-action 'none'`,
  `frame-ancestors 'self' file: http://127.0.0.1:<pages port>
  http://localhost:<pages port>`, plus `http://localhost:5173` in a dev
  build only (`BISE_DEV`): no other site and no other local server (agents
  run dev servers all day) may frame a view's proxy and feed it html that
  would read the view's storage, architect m_50106) plus the declared
  origins only,
  each directive per the spec (`connectDomains` → connect-src,
  `resourceDomains` → img/script/style/font/media-src, `frameDomains` →
  frame-src, `baseUriDomains` → base-uri). An entry that is not an origin
  (path, `*` alone, `data:`, a `'keyword'`, a wildcard port or TLD, a
  loopback host without a port or on the pages port) is dropped; 32 per
  list at most. `domain` is ignored (said in the plugin's page): a stable
  origin on localhost is not possible.
- Desktop (`main/viewrules.ts` the pure rules, `main/viewframes.ts` the
  wiring): `main/guard.ts` lets the app set a frame to a view's proxy,
  the proxy make its blob view, a frame nested in a view go where its CSP
  lets it; a view or its proxy leaving for any other url is dropped,
  never sent to his browser. The webRequest filter lets through a view's
  proxy and the subframes nested in a view. The session's permission
  handlers grant `media` (audio, video), `midi` and `midiSysex` (Chromium
  asks `midiSysex` for any `requestMIDIAccess`), `clipboard-sanitized-write`
  to a view origin only when its live frame's `perms` has it (declared
  and granted, C8); everything else denied, and denied to any subframe
  whose origin can't be told (`about:srcdoc`, `about:blank`). MIDI is not in the spec's
  permissions: a view declares it in bise's own key next to `ui` on the
  resource, `_meta["dev.bise/ui"]: {"permissions": {"midi": {}}}` (other
  hosts ignore it; the spec's `_meta.ui.permissions` stays clean), and
  its view should degrade when `requestMIDIAccess` is refused.
  The pages server knows each origin's sandbox: the hub registers
  `ViewSandbox {csp, perms}` (`pages/views.rs`) when it answers
  `view/resource`, and serves `/mcp-app` with that CSP as its header
  (never from the query: a view can navigate its own frame); an unknown
  hash is 404. The registry is in memory: after a hub restart the host
  asks `view/resource` again before it loads the proxy. The origin's hash
  is sha256, never a fast hash: with a 64-bit non-cryptographic one a
  hostile plugin could craft a `uri` that lands on another plugin's
  origin. The layer tells Electron main its live frames
  (`window.bise.views.frames([{origin, agent, call, rect, perms}])`, the
  whole list on each change); main grants a request from `origin` only
  within its `perms`. The camera needs
  `com.apple.security.device.camera` in
  `packaging/entitlements/desktop.plist` and `NSCameraUsageDescription`
  (the mic's are already there).
  The proxy puts the hub's CSP meta after the view's own leading
  `<!doctype>`: before it the view would run in quirks mode. A
  pointer-down is placed by its screen point (`globalX/Y` minus the
  window's content corner, over the zoom: `viewrules.pointInWindow`),
  never by its `x/y`, which are relative to the view's own frame when
  Chromium hands it to the proxy's widget.
  The check of all this through the real hub is
  `tests/mcp_apps_hostile_e2e.py`: a throwaway hub, fake-app's
  `show_hostile`, its web host page (`GET /v/main/<call>`, C9) in
  headless Chromium (`*.localhost` mapped to 127.0.0.1); it wants the
  proxy on `v-<hash>.localhost`, standards mode, `body[data-done]` and
  exactly its rows by name (3 controls `ok`, then 21 probes `blocked`;
  every https request answered by the test, every permission granted and
  the devices faked, so a refusal can only be the sandbox's). Two rules run it (architect m_50500):
  on the `mcp-apps` tip after each sandbox or host land and before
  `sb feature ready` (risk 1); and, once merged, as the `mcp-apps` step
  of `scripts/nightly_desktop.sh` (owner mcp-apps), so in the nightly
  and on a release's exact sha (`desktop_tag.py` reads its state). Never
  in run_all's TESTS (Playwright). Its red, `desktop/mcp-apps-hostile`,
  is a leak: `tests/known_reds.py` refuses to list it, so no desktop tag
  goes through it.

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

Gabriel's q-perm: mic, camera, MIDI and clipboard per view, granted when
he adds the plugin; nothing runs before "add and run it" (mcp-apps-lead
m_50127, option A).

- **Step 1, the add screen** (`plugins/Add.tsx`, TUI `plugins_add.rs`):
  `PluginPreview.views` lists the views known before anything runs (its
  Vibe static views), each a `PluginPart` of kind `view` with
  `view: {uri, server?, declared, domains, granted, asked}`; a tick per
  declared permission (all unticked at first). "add and run it" sends
  `plugin_add_confirm`, then one `plugin_grant {plugin, uri, perms}` per
  previewed view (an unticked one: `perms: []`, asked and nothing
  granted). The TUI lists them, ticks none (granted from the app).
- **Step 2, its views** (once it runs; `plugins/ViewsGroup.tsx`
  `ViewsStep`): after the confirm of a plugin with a server, the add
  screen asks `hub/plugin_views {project, plugin}` (through
  `window.bise.views.call`; the hub asks its views bridge's
  `GET <base>/<plugin>/~ui`, ba6ebf0f9, mcp-apps-calls) while it says
  "starting its server…", then lists each view found (step 1's skipped)
  with its ticks; "done" sends their `plugin_grant`; nothing found, or
  an error, closes it at once. The hub's project is the add's, else
  bise's home hub.
- **Storage**: `plugins.json`'s `view_grants`, by view part id
  (`<plugin>#view:<uri>`, the uri escaped by `PluginId`):
  `{"fake-app#view:ui%3A%2F%2Ffake-app%2Fcounter": ["midi"]}`. No entry:
  never asked; `[]`: asked, nothing granted. `bend_plugins::state::grant`
  is the one writer (the same lock as the on/off lists), `grants` reads
  them, `view_grant(grants, plugin, uri) -> Option<perms>` is the pure
  rule the hub's `view/resource` uses for `permissions` = declared ∩
  granted (it also returns `declared`). A removed plugin's grants go with
  it (`state::forget`).
- **The plugin's page**: a VIEWS group (`plugins/ViewsGroup.tsx`; TUI
  `plugins_screen.rs`) shows each view part: the origins it reaches, a
  tick per declared permission (a tick sends the view's whole grant), or
  "not granted" for a view he was never asked about (a server's view that
  came later). The catalog's rows (`catalog::views`) hold the static
  views and every view with a grant.
- A view never asked runs with no permission; its card says so
  (`view/resource`'s `declared` minus `permissions`).

### C9. The web host (piece 11)

`GET /v/<agent>/<call>` on the main pages host serves the host page (the
same `views/` host code, built for the browser); its calls go to `POST
/api/view` (`x-bise-token` and `Origin` checked like every `/api` route)
with C5's methods in the body (`{method, params: {agent, call, ...}}`,
answered 200 `{result}` or `{error: {code, message, data}}`; the hub's
one dispatcher, `PageMsg::View` -> `Shell::view_request`). The page
(`pages/view_web.rs`) carries the token, loads `/kit/views/web.js` and
`.css` and fills `<main id="bise-view" data-agent data-call>`; its CSP
frames only `http://*.localhost:<pages port>` and fetches only itself.
`view/message` there is refused every time (`-32037`, reason
`no-gesture`): no Electron main sees a click in a browser.

The TUI (architect m_50261, designer m_50266; a change to q-tui's bare
`o`/`O`, which can't work: the feed has no row focus and letters go to
the composer):

- under a tool row, one dim line per view of `ToolItem.views`:
  `⊞ counter · fake-app   app ↗   browser ↗` (G_VIEW, ASCII `+`);
  expired: `⊞ counter · fake-app · expired`, faint, no link. `app ↗`
  opens it in the desktop app, `browser ↗` opens
  `http://127.0.0.1:<pages port>/v/<agent>/<call>` in his browser;
- `/views` (a client command): the thread's views newest first, `⏎` in
  the app, `b` in his browser, `esc` back: the keyboard path when clicks
  are off;
- the words are the catalog's (`words::views::tui_*`).

Opening in the app goes through the hub, never a URL scheme (architect
m_50268: a `bise://` scheme would launch the app instead of saying it
isn't running, and any web page could trigger it):

- `initialize`'s `client` carries `kind: terminal | desktop | web`
  (absent: unknown, an older client is never taken for the app); the
  desktop core says `desktop`, the TUI `terminal`;
- `app/open` (`HubCmd::AppOpen {project, what, agent, id}`, `what:
  OpenWhat::View`, `agent`, `id` = the call) is forwarded as the
  notification `app/open` (`HubEv::AppOpen`, scope one) to the
  connections that said `desktop` only; its result is `{delivered: n}`,
  and the TUI flashes `words::views::tui_no_app` on 0. The shell
  forwards it, no sb-core: a UI hint between his own clients, no
  message, no agent, nothing journaled; client socket only (an agent's
  `sb` can't send it);
- the desktop core passes the hub's `app_open` event to its window like
  `card_open` (a one-shot read through `onEvent`, `store/apply.ts`); the
  window's views host opens that view's panel tab (`what: view`, `agent`,
  `id` = the call).

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
     land, its Playwright check (`tests/mcp_apps_hostile_e2e.py`, C6) on
     the `mcp-apps` tip after each sandbox or host land and before
     `sb feature ready`: a leak never reaches main. After the merge it
     runs in the nightly's desktop checks and on a release's exact sha.
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
