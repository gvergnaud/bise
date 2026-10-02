# Agent plugins in the harness and Switchboard

Status: implemented, 2026-09-28 (task `plugins`). Code: `rust/plugins`
(`bend-plugins`), `runtime/plugins.bend`, `runtime/skills.bend`,
`runtime/mcp.bend`, `rust/tui/src/plugins.rs`.

## Goal

Load [Agent Plugins 1.0](https://agent-plugins.org/specification)
packages: a folder with a `plugin.json`, optional `skills/` and an
optional `mcp.json`. The same plugin works in every Switchboard task and in a
`bend-harness --headless` session, because both run the same Bend REPL
(`repl-live`).

Supported now (the portable base of the spec):

- discovery in a user root and a workspace root;
- `plugin.json` validation against the closed 1.0.0 schema, with stable
  diagnostic codes;
- `skills/*/SKILL.md` in the skills catalog and the `skill` tool;
- local `stdio` MCP servers from `mcp.json`: started with the session,
  their tools callable as `tools.<namespace>.<tool>`;
- remote MCP servers from `mcp.json`: Streamable HTTP (`"type": "http"`
  or `"streamable-http"`) and the legacy HTTP+SSE transport (`"type":
  "sse"`), with `url` and `headers` (`${VAR}`, `${VAR:-default}`), the
  shape Claude Code, Cursor and Vibe use, so their `.mcp.json` works as
  is (below, "Remote servers");
- a list with diagnostics: `bend-harness plugins` (CLI) and `/plugins`
  (TUI);
- enable and disable, per plugin name.

Not supported yet, reported as `plugin.component.unsupported` in the
diagnostics (the rest of the plugin still loads):

- everything under `ai.mistral.vibe/` (hooks, agents, knowledge, views),
  `connectors.json`, `libraries.json`;
- the `ai.mistral.vibe` manifest extension (`toolNamespace`,
  `toolOverrides`, `displayName`): read by no one, reported once.

No Vibe compatibility layer: no `~/.vibe/plugins`, no Claude/Codex/Kimi
foreign formats.

## What Vibe does, and where we differ

Sources: `~/mistral/dashboard/vibe/vibe/core/plugins/` (Python resolver,
`_native.py`, `_naming.py`, `_diagnostics.py`) and the schemas and
fixtures in `vibe_sdk/harness/plugins/`.

| Topic | Vibe CLI | Bend harness |
| --- | --- | --- |
| Roots | `~/.vibe/plugins/`, `<project>/.vibe/plugins/` (trusted folders only) | `~/.agents/plugins/`, `<workspace>/.agents/plugins/` (same convention as our `.agents/skills`) |
| Precedence | project over user, same name | same |
| Same name twice in one root | both dropped (`plugin.name.collision`) | same |
| Pinning | each session copies the tree into a read-only content-addressed store | none: the plugin runs from its folder. `/reload` picks up edits |
| `PLUGIN_DATA` | per session, under the session dir | durable: `~/.bend-harness/plugin-data/<name>/` |
| Skill names | `<namespace>:<skill>` | same |
| MCP tool names | `tools.<group>.<tool>`, group = namespace, `toolOverrides` rename | `tools.<namespace>.<tool>`, no overrides; a name collision drops the later tool |
| Tool exposure | `programmatic` by default, overridable | programmatic only: `search_tool_functions`, `run_typescript`, direct call by dotted name (like our connectors) |
| stdio transport | persistent clients in the Vibe MCP registry | persistent servers behind a per-session loopback bridge (below) |
| HTTP transports | yes | Streamable HTTP and SSE, static headers, behind the same bridge (below) |
| Enable / disable | none at plugin level (mounting = enabling); `disabled` per MCP server in config | `bend-harness plugins disable <name>`, stored in `~/.bend-harness/plugins.json` |
| Hooks, knowledge, agents, views, connectors, libraries | loaded (some broken locally, see the plugin-creator skill) | listed as unsupported |
| Inspect | `/plugins`, `/reload-plugins` (behind `--experimental-harness`) | `/plugins` (TUI), `bend-harness plugins` (CLI), `/reload` re-resolves |

## Where the code goes

The Bend REPL cannot hold a child process open: Base `Process.run`
takes stdin as one string and waits for the exit. A stdio MCP server
needs a pipe that stays open. The REPL already speaks MCP Streamable
HTTP (`runtime/mcp.bend`, for the Mistral connectors). So the work splits:

- **Rust, new crate `rust/plugins` (`bend-plugins`)**: the resolver
  (discovery, validation, diagnostics), the enable state, a stdio MCP
  client, and a small loopback HTTP bridge. It is linked into
  `bend-harness` (subcommand `plugins`) and into `bend-tui` (`/plugins`).
- **Bend, `runtime/`**: start the bridge at live startup, read two more
  index files. No new protocol code: plugin tools go through the same
  `mcp_call` path as connector tools, with a loopback URL instead of the
  gateway URL.

```text
repl-live ──start──▶ bend-harness plugins serve --dir D --parent <repl pid> --workspace W
    │                   ├─ resolve plugins (user root, workspace root, enable state)
    │                   ├─ spawn each stdio server, initialize + tools/list (10 s)
    │                   ├─ write D/skills-index.txt, D/mcp-index.txt, D/report.txt
    │                   ├─ touch D/ready
    │                   └─ serve http://127.0.0.1:<p>/<token>/<plugin>/<server>
    │                        until the REPL pid is gone
    └─ mcp_call "ns.tool" ──POST (initialize, initialized, tools/call)──▶ bridge ──stdin/stdout──▶ server
```

`D` is `~/.bend-harness/run/<repl port>/plugins/`. Ports are unique among
live REPLs, so two sessions (or two Switchboard tasks) never share it.

### Resolver (`bend-plugins`)

1. **Roots.** User root: `$BEND_PLUGINS_HOME` or `~/.agents/plugins`.
   Workspace root: `<workspace>/.agents/plugins`, where the workspace is
   `$BEND_WORKDIR` (set by the Switchboard hub per task) or the REPL's
   cwd. Each direct child directory with a `plugin.json` is a candidate.
   An unreadable root gives `plugin.discovery.root_unreadable`; a missing
   one is silent.
2. **Manifest.** JSON object, closed schema: `$schema` must be the 1.0.0
   URL, `name` must match `^(?!.*(?:--|\.\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$`
   (1-64 chars), optional fields typed as in the schema, `extensions` an
   object of objects, no other key. Any failure: `plugin.manifest.invalid`
   with the JSON path, and the whole plugin is dropped.
3. **Namespace.** `name` with every character outside `[A-Za-z0-9_$]`
   turned into `_`, and a leading `_` before a digit. Reserved:
   `file_system`, `process`, `self`, `skill`, `subagent`, `vibe`
   (`plugin.namespace.reserved`, fatal). Two surviving plugins with the
   same namespace: `plugin.namespace.collision`, both dropped.
4. **Precedence.** Same `name` in both roots: the workspace one wins,
   the user one is listed as `shadowed`. Same `name` twice in one root:
   both dropped (`plugin.name.collision`).
5. **Enable state.** `~/.bend-harness/plugins.json`:
   `{"disabled": ["name", ...], "enabled": ["name", ...]}`. A disabled
   plugin is listed, its components are not loaded. **Opt-in**: a
   manifest with `"extensions": {"dev.bise": {"default": "off"}}` (bise's
   own extension, never reported as unsupported) loads only when its name
   is in `enabled`; `disabled` still wins. `plugins enable <name>` adds to
   `enabled` and drops from `disabled`, `disable` the reverse. The
   built-in `computer` plugin is opt-in: `/computer-use` turns it on,
   `/computer-use off|uninstall` turns it off (docs/computer-use-ship.md
   §1).
6. **Skills.** Each `skills/<dir>/SKILL.md`, realpath inside the plugin
   root, with a YAML frontmatter holding a one-line `name` and
   `description`. Published as `<namespace>:<name>`. A bad one:
   `plugin.skill.invalid`, skipped.
7. **MCP.** `mcp.json` must match the 1.0.0 MCP schema (`$schema` const,
   `mcpServers` object). A bad file: `plugin.mcp.invalid`, no server
   loads. No `$schema` is fine (a file copied from Claude Code, Cursor
   or Vibe); a different one is an error. Per server: `type` `stdio`
   (the default with a `command`), `http`/`streamable-http` (the default
   with a `url` and no `command`) or `sse`; another type is
   `plugin.component.unsupported`. A remote server takes `url` and
   `headers` only (see "Remote servers"). A stdio server: `command` is a bare executable (looked up in `PATH`)
   or `./x` resolved inside the root. `${PLUGIN_ROOT}` and
   `${PLUGIN_DATA}` are replaced in `command`, `args`, `env` values and
   `cwd`; `cwd` must start with `./`, `${PLUGIN_ROOT}` or
   `${PLUGIN_DATA}` and stay inside it (default: the plugin root); an env
   key named `PLUGIN_ROOT`/`PLUGIN_DATA` is rejected. The server gets the
   REPL environment plus `PLUGIN_ROOT`, `PLUGIN_DATA` and its `env`.
   A bad server: `plugin.mcp.server_invalid`, skipped.
8. **Unsupported components.** Present on disk and reported, never read:
   `ai.mistral.vibe/{hooks.toml,agents,knowledge,views}`,
   `connectors.json`, `libraries.json`, and the `ai.mistral.vibe`
   manifest extension.

Every diagnostic has a code, a severity (`error` drops the plugin,
`warning` drops one component, `info`), the plugin name or root, and one
sentence. The resolver is pure over a file-system snapshot, so its unit
tests use temp dirs.

### Bridge (`bend-harness plugins serve`)

- Spawns every stdio server of every enabled plugin in parallel, sends
  `initialize` (protocol `2025-06-18`), `notifications/initialized`,
  `tools/list`. 10 s per server; a failure gives
  `plugin.mcp.connection_failed` (with the last stderr lines) and the
  server is left out. stderr goes to `D/<plugin>.<server>.log`.
- Tool names are made identifiers the same way as namespaces. Two tools
  of one plugin with the same name (from two servers):
  `plugin.tool.name_collision`, the later one is dropped.
- Writes, then touches `D/ready`:
  - `D/mcp-index.txt`: the same line format as the connector index,
    `<cid> <namespace> <tool> : #<description> | input: <schema>`, with
    `cid = http://127.0.0.1:<port>/<token>/<plugin>/<server>`;
  - `D/skills-index.txt`: `name\tdescription\tpath` lines;
  - `D/report.txt`: the human report (the same text as the CLI).
- Serves HTTP/1.1 on `127.0.0.1:0`. The path carries a random token, so
  another local process cannot call the tools without reading `D`.
  JSON-RPC over POST: `initialize` answers with the server's cached
  result, notifications answer `202`, anything else is forwarded with a
  fresh id and answered as `application/json` (60 s timeout). A server
  that died is respawned once on the next call.
- No server up (no plugin, or every server failed): it exits as soon
  as the files are written, so no process lingers.
- Polls the REPL pid every 500 ms; when it is gone, stops the servers
  (a short grace period, then a kill) and exits. `/reload` therefore restarts the
  bridge with the new REPL, and plugin edits apply on `/reload`.

### Bend runtime

- `repl-live` startup, before the skills scan and before the system
  prompt is built (it lists the skills): `Pl.start()` runs
  one `/bin/sh` script: clear `D`, start the bridge in the background
  (`$BEND_HARNESS_BIN`, else `./bend-harness`, else the dev build under
  `rust/target`), wait for `D/ready` (at most 15 s). No binary: no
  plugins, no error. Scripted runs never start it (hermetic suites).
- `BEND_HARNESS_BIN` is set on the REPL by `bend-harness --headless`
  and by the hub (`Opts.exe`).
- Skills: the scan keeps the user roots in the shared index (the TUI
  `$` popup reads it). The workspace root (`$BEND_WORKDIR/.agents/skills`,
  today `$PWD`, which is the app root in Switchboard, so workspace skills
  were not found) and `D/skills-index.txt` go to a per-session index
  `~/.bend-harness/run/<port>/skills-index.txt`. The catalog and the
  `skill` tool read the session index, then the shared one.
- MCP: `mcp_call` and `search_tool_functions` read `D/mcp-index.txt`,
  then the connector index. `gateway_url(cid)` keeps a cid that starts
  with `http://127.0.0.1:` as is, and the Mistral key is not sent to it.

### UI and CLI

- `bend-harness plugins [list] [--workspace W] [--json]`: each plugin
  with scope, version, state (`loaded`, `disabled`, `shadowed`,
  `invalid`), root, skills, MCP servers, unsupported components, then the
  diagnostics. Static: it does not start servers.
- `bend-harness plugins enable|disable <name>`: edits
  `~/.bend-harness/plugins.json`; applies at the agents' next idle (below).

### Reload on change

A plugin installed, removed, enabled, disabled or edited while bise runs
reaches every agent without a restart of the TUI (the user: a voice
recording, a draft, the scroll must survive). The hub keeps, per live
REPL, its workspace and `bend_plugins::resolve::fingerprint` of its roots
(the enable state; in the built-in, user and workspace roots each plugin
folder and the size and mtime of its `plugin.json`, `mcp.json` and
`skills/*/SKILL.md`), taken at spawn. Every 2 s on the tick, and when an
agent goes idle, it compares: a REPL whose fingerprint moved relaunches
at its next idle, same session and port (the path a key change and a
version switch take, `switch_idle_repls`); a busy one finishes its turn
first. The new REPL starts a fresh bridge and lists the new skills in its
prompt: a restored session keeps its saved system prompt (prompt cache,
BISE-268) unless the hub sets `BEND_FRESH_PROMPT=1`, which it does when
the plugins fingerprint differs from the one its prompt was last built
with (`<agent dir>/prompt-plugins.fp`; a version switch or a TUI restart
counts too). The prompt's `## Plugins` section (BEND_TOOLS_NOTE,
`tools_env::session_note`) names each loaded plugin, its description and
its skills, so an agent knows what a plugin is for before it searches.
Only REPLs restart: the TUI and the hub keep running. Test:
`tests/plugins_reload_e2e.py` (install → relaunch at idle, the
conversation kept, the plugin loaded; nothing changes → no relaunch; a
disable → a relaunch).

Skills follow the same path, with no timer. The hub also keeps, per live
REPL, a fingerprint of the skill folders its startup scan reads
(runtime/skills.bend: `~/.agents/skills`, `~/.vibe/skills`,
`<workspace>/.agents/skills`, for main the app root's `prompts/skills`;
each `<skill>/SKILL.md` path with its size and mtime, stats only). It
compares it when an agent goes idle and right before a turn starts (the
`say` of an idle agent, `skills_before_turn`): a SKILL.md added, edited
or removed relaunches the REPL first, same session, and the `say` waits
in the switch queue, so the turn right after the change already lists
the new skill. `prompt-plugins.fp` holds both fingerprints, so the
restored session takes a fresh prompt. The workspace is the agent's
`BEND_WORKDIR` (its hub workspace): a task working in a `gate.sh new`
worktree still reads the shared folder's `.agents/skills`, like its
plugins and AGENTS.md. The TUI's `$` popup scans the same folders
itself (and the loaded plugins' skills), rebuilt when their stats move,
checked when the popup asks after a second without asking
(`rust/tui/src/skills.rs`). Tests: `tests/skills_reload_e2e.py` (no
change → no relaunch; add, edit, remove → that turn's prompt has it),
`tests/tui_skills_reload_tmux.py` (`$` shows an added skill, an edited
description, not a removed one).
- `/plugins` in the TUI prints the workspace's static listing (the
  single-agent TUI, gone with BISE-113, also showed its session's
  `D/report.txt`).

## Tests

- `cargo test -p bend-plugins`: unit tests for every diagnostic code,
  precedence, containment (a symlink out of the root), placeholder
  expansion, the enable state (`src/resolve/tests.rs`, `src/state.rs`);
  `tests/bridge.rs` runs the fixture plugin
  (`rust/plugins/tests/fixtures/hello-plugin`: skill `greet`, a
  dependency-free Python stdio server with one tool `shout`) through the
  real bridge: index files, `initialize` + `initialized` + `tools/call`
  over HTTP on one connection, a wrong token is a 404, `PLUGIN_DATA`
  written, the bridge stops with its parent; a failing server is a
  `plugin.mcp.connection_failed` diagnostic and the skill still loads.
- `cargo test -p bend-tui plugins`: `/plugins` text.
- `tests/plugins_live.py` (live model): a real
  `bend-harness --headless` session on a throwaway workspace loads
  `hello_plugin:greet` and calls `tools.hello_plugin.shout` from
  `run_typescript`.
- `tests/plugins_sb_live.py` (live model): the
  same from a Switchboard task on a throwaway hub.
- `bend PROOF.bend`, the scripted e2e suite and the TUI tmux suite stay
  green (scripted runs never start the bridge).

### Remote servers (Streamable HTTP, SSE)

```json
{"mcpServers": {
  "linear": {"type": "http", "url": "https://mcp.linear.app/mcp",
             "headers": {"Authorization": "Bearer ${LINEAR_API_KEY}"}},
  "legacy": {"type": "sse", "url": "https://example.com/sse"}
}}
```

- **Where.** The bridge holds them like the stdio servers: the REPL
  calls the same loopback URL, nothing changes on the Bend side, and a
  token never reaches the REPL, a session or `/log`.
- **Config.** `url` (http or https) and `headers` (string values).
  `${PLUGIN_ROOT}`/`${PLUGIN_DATA}` are replaced at resolve time;
  `${VAR}` and `${VAR:-default}` (Claude Code's syntax) only when the
  bridge connects, from its environment (the REPL's, so the user's shell
  and bise's `.env`). An unset variable: `header Authorization uses
  ${LINEAR_API_KEY}, which is not set`, and the server is left out.
  Listings show the host and the header names, never a value or the
  URL's path (it may hold a key).
- **Client** (`rust/plugins/src/http.rs`, `remote.rs`): blocking
  HTTP/1.1, rustls with the webpki roots, one connection per request
  (a server that restarted is just a new connection). Streamable HTTP:
  each message a POST with `Accept: application/json,
  text/event-stream`; the answer as JSON or as an event stream (chunked
  or not; other messages on it are handled: `ping` answered, the rest
  refused); `Mcp-Session-Id` from `initialize` and
  `MCP-Protocol-Version` on every later request; a 404 on a session is a
  new handshake and the request again; a server announcing
  `tools.listChanged` gets a GET event stream for its notifications,
  reopened with backoff when it drops (405: none). SSE: the GET stream's
  `endpoint` event gives the POST URL (same origin only); the answers
  come on the stream; a dropped stream is reopened, with a new
  handshake, at the next request. A request that may have reached the
  server is never sent twice: only a connect failure or an expired
  session retries; a stream that drops mid-call fails that call.
  Timeouts: 20 s to connect, list and hand-shake, 60 s per call.
- **`tools/list`** follows `nextCursor`. On
  `notifications/tools/list_changed` (remote or stdio) the bridge lists
  that server again and rewrites `D/mcp-index.txt` (the REPL reads it at
  each search and call).
- **Errors** are one line naming the host: `mcp.example.com refused the
  connection`, `HTTP 403 from mcp.example.com: forbidden`,
  `mcp.example.com answered 401: it needs a login or a token in
  "headers"`, `mcp.example.com did not answer within 20s`; they go to
  `plugin.mcp.connection_failed` like a stdio server's.
- **State for `/plugins`.** The bridge writes each remote server's last
  state to `~/.bise/mcp-status/<plugin>/<server>.json`
  (`$BEND_MCP_STATUS`; host, transport, tool count or the error line,
  time). The static listing (`/plugins`, `bise plugins list`) shows it:
  `mcp linear: http mcp.linear.app · connected · 23 tools · 2 min ago`,
  or `· ✗ <error> · …`, or `· not connected yet` before any session.
- **Importers.** `bise plugins import-mcp` keeps remote servers: Claude
  Code's `type`/`url`/`headers` as they are; Codex's `url` with
  `http_headers`, `env_http_headers` (`{"X-Org": "VAR"}` becomes
  `"${VAR}"`) and `bearer_token_env_var` (`Authorization: Bearer
  ${VAR}`). It prints the host and the header names, never a value.
- **Login (OAuth).** A remote server without an `Authorization` header
  in its mcp.json can log in (`rust/plugins/src/oauth.rs`, `login.rs`;
  the MCP authorization spec 2025-06-18). Discovery: the 401's
  `WWW-Authenticate` `resource_metadata`, else
  `/.well-known/oauth-protected-resource[/path]`, then the authorization
  server's RFC 8414 / OIDC metadata (an older server without any: its
  origin's `/authorize`, `/token`, `/register`); a server whose metadata
  lacks PKCE S256 is refused. Client: mcp.json's `"oauth": {"clientId",
  "clientSecret", "scopes", "callbackPort"}` (Claude Code's keys; GitHub
  and Slack have no dynamic registration), else the one registered
  before, else RFC 7591 registration as a public client. The login:
  PKCE S256, a random state, `resource` = the server URL (RFC 8707), the
  metadata's scopes, the browser on `http://127.0.0.1:<port>/callback`
  (the registered port again next time), 5 minutes. Tokens: one file per
  server URL in `~/.bise/secrets/mcp-oauth/` (`$BEND_MCP_SECRETS`;
  folder 0700, files 0600, written by rename), never printed, never in a
  session, a report or /log. Sent as `Authorization: Bearer`; refreshed a
  minute before expiry or after a 401, under a lock on the file (every
  agent's bridge shares it, the refresh token rotates); a `resource`
  refused on refresh is tried once without it; a refresh refused for
  good drops the tokens and keeps the client. Not yet: client ID
  metadata documents (CIMD, spec 2025-11-25).
- **Login in the bridge.** A server that answers 401 at start stays in
  the bridge without tools (`plugin.mcp.login_needed`, status "needs a
  login"); every 2 s the bridge looks at its store file and connects it
  once a login lands there, rewriting the index: no restart. A call that
  gets a 401 the login can't fix answers the agent `linear needs the user
  to log in (/plugins login). tell them, or go on without it.`
- **UI** (designer m_4519). `/plugins`: `mcp linear · mcp.linear.app ·
  needs a login · /plugins login`. `/plugins login` opens the popup, one
  row per server that can log in: `linear   mcp.linear.app · needs a
  login` or `· logged in · 23 tools`; ⏎ runs the login in the
  background: `opening your browser to log in to linear…`, then `logged
  in to linear: 23 tools, your agents have them now.` or `▲ couldn't log
  in to linear: <reason>. /plugins login tries again.` The browser tab:
  `bise :* is logged in to linear. you can close this tab.` or `the login
  didn't go through: <reason>. /plugins login in bise tries again.` Once
  per server and TUI run, when a session found it needs a login: `linear
  needs a login: /plugins login`. CLI: `bise plugins login [SERVER]`,
  `bise plugins logout SERVER`. Later: an inbox item for it.
- **Tests:** `tests/fake_mcp_http.py` (a Streamable HTTP or SSE server
  with a required header, pagination, session expiry, dropped streams,
  list_changed, a 401; `--oauth`: its own protected resource and
  authorization server metadata, registration, authorize, token with
  PKCE checked, rotating refresh tokens, expire/revoke/deny knobs)
  driven by `rust/plugins/tests/remote.rs` (the client, and a plugin
  with two remote servers through the real bridge) and
  `rust/plugins/tests/oauth.rs` (login, store, refresh after a 401 and
  before expiry, revoked, denied, no registration, the bridge connecting
  after a login); `tests/tui_mcp_login_tmux.py` (the quiet line,
  /plugins, the popup, both thread lines, both browser pages).

## Known limits

- The TUI `$` skill popup reads the shared index only: workspace and
  plugin skills work for the agent but are not offered in the popup.
- Plugin edits apply at the next session start or `/reload` (no
  file watching, no `/reload-plugins`).
- The Switchboard `/plugins` shows the static listing of the hub
  workspace, not the per-task bridge report.

## Later

- An inbox item when a remote server needs a login; CIMD clients.
- `ai.mistral.vibe` extension: `toolNamespace`, `toolOverrides`.
- Per-server enable/disable, a `/plugins` picker with toggles.
- Plugin descriptions in the system prompt (the spec's default guidance).
- Hooks, agents, knowledge, views.
