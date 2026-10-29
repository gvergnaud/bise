# Agent plugins in the harness and Switchboard

Status: implemented, 2026-09-28 (task `plugins`). Code: `rust/plugins`
(`bend-plugins`), `runtime/plugins.bend`, `runtime/skills.bend`,
`runtime/mcp.bend`, `rust/tui/src/plugins.rs`, `plugins_screen.rs`,
`plugins_add.rs`.

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

- everything under `ai.mistral.vibe/` but views (hooks, agents,
  knowledge), `connectors.json`, `libraries.json`; views without the
  `ai.mistral.vibe` extension in plugin.json;
- the `ai.mistral.vibe` extension's fields (`toolNamespace`,
  `toolOverrides`, `displayName`): read by no one. The extension itself
  is known: it turns on the plugin's static views
  (`ai.mistral.vibe/views/<name>/`, docs/mcp-apps.md), each one a tool of
  the plugin's virtual server `~views` (`bend_plugins::views`).

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
| Hooks, knowledge, agents, connectors, libraries | loaded (some broken locally, see the plugin-creator skill) | listed as unsupported |
| Views (MCP Apps, static views) | loaded | loaded: docs/mcp-apps.md |
| Inspect | `/plugins`, `/reload-plugins` (behind `--experimental-harness`) | `/plugins` (TUI), `bend-harness plugins` (CLI), `/reload` re-resolves |

## Contract (the unified Plugins list)

Plan: page `plugins` (how-* blocks, architect m_38884, m_38890,
m_38967). One list for real plugins, Mistral connectors, local MCP
servers and skills folders; each plugin and each part on or off; all on
by default but the opt-in plugins. The state stays his files
(`plugins.json`), no hub.sock method. What exists now is the contract;
the bodies marked `todo!("plugins chunk ...")` come with their chunk.

- **Ids** (`bend_plugins::id::PluginId`, parsed and written there only):
  `docs` (a real plugin, its bare name: today's `plugins.json` names stay
  valid), `mistral:gmail`, `skills:personal|vibe|workspace|bise`; parts
  `docs#skill:docs`, `linear#mcp:linear`,
  `linear#tool:linear/create_issue`, `mistral:gmail#tool:send_email` (a
  connector is its own server). `: / # %` and the space are
  percent-escaped in every name; `parse(display(id)) == id`.
- **Shared data** (`bend_plugins::model`): `ConnectorEntry` (+
  `ConnectorAuth`), `ToolEntry`, `SkillEntry`, `StateV2` (the four lists
  of `plugins.json`: `disabled`, `enabled`, `parts_off`, `parts_on`).
- **State** (`bend_plugins::state`, done): `read(path) -> StateV2`,
  `set(path, &PluginId, on)` (the one writer, same flock),
  `StateV2::is_on(&id, opt_in)`, `StateV2::locked(&id)` (`skills:bise`
  and its parts: always on, `set` refuses to turn them off). See
  "Enable state" below.
- **Connectors** (`bend_plugins::connectors`, landed): the one owner of
  Mistral's catalog for every agent and the UI. `load(cache, key,
  now_ms, force)` fetches `GET /v1/connectors/bootstrap?supports_mcp=true
  &include_auth_actionable_connectors=true&builtin_connectors=…` (one
  param per `BUILTINS`: web_search, image_generation) when the cache is
  older than `TTL_MS` (10 min), under the cache's flock (two bridges
  never fetch at once; the second reads the fresh file), through a tmp
  file. A failed fetch (no answer, a non-200, a body with no
  `connectors` list) keeps the last good connectors, sets
  `Catalog::error` (`Problem::Fetch`) and is not tried again for
  `FAIL_BACKOFF_MS` (30 s) unless forced. `cached(cache)`: the file as
  is, no network (a list that opens now). `key() -> Key`: auth.json's
  `mistral` API key, else `MISTRAL_API_KEY`, else `NoKey`; auth.json in
  a locked keychain is `Locked` (never "no key": `load` keeps the
  cache, writes nothing and says `Problem::Locked`). Never in the
  cache. Each connector is a `ConnectorEntry` (name = its alias:
  vibe's normalize, then `_2`, `_3` against the earlier aliases;
  `auth` Ready, OAuth, NoAuthConnect, CredentialsSetup, or Unknown: not
  ready and no auth_action bise knows, not connectable from bise; each
  tool's input schema as serde_json's compact text, keys sorted).
  `index_lines(&Catalog, &StateV2)`: the ready connectors' tools minus
  an off connector or tool, in the index format
  `<cid> <alias> <tool> : #<desc>[ | input: <schema>]`, with bise's
  note after image_generation's descriptions (`IMAGE_NOTE`); the same
  lines as `mcp.bend` wrote (`connectors/fixtures/index.txt`, a schema
  compared as JSON: a float may be written another way), except two
  bug fixes: a not-ready connector lists no tool (mcp.bend wrote them
  with an empty alias) and no two connectors share an alias (mcp.bend
  gave `a_2` twice for `a`, `a`, `a_2`).
  `connect(&World, &id)` (`bise plugins connect mistral:<name>`, his
  click; `connect_now` runs it on the caller's own thread, never a draw
  or core loop): an OAuth or credentials connector opens
  `GET /v1/connectors/{id}/auth_url`'s `auth_url` (`BISE_OPEN`, else
  the system opener), a no-auth one POSTs his default connection
  (`/v1/connectors/{id}/user/credentials`); then a forced fetch every
  3 s until it is ready, 5 min at most. `bise plugins refresh`: a
  forced fetch. `BISE_MISTRAL_URL`: a fake Mistral in tests
  (`tests/fake_bootstrap.py`). Cache: `Home::connectors_cache()`
  (`~/.bise/connectors/bootstrap.json`).
- **List** (`bend_plugins::catalog::rows(&Inputs) -> Vec<app_rows::Plugin>`):
  the one builder; the TUI and the desktop's core call it in their own
  process. Inputs read by their owners first: the resolution, the
  connectors' cache (never a fetch), the skills folders' skills (off ones
  too), the on/off file, the MCP status folder, the OAuth token folder,
  the logins open in his browser. Rows in order: `skills:bise` (locked
  on), the real plugins (a folder bise wrote for an MCP server,
  `import::is_bise_mcp`, is kind `mcp` and shows its command's name and
  package or its host, never an argument, header, env value or URL
  path), the ready connectors (their tools as parts; `web_search` and
  `image_generation` scope `built-in`), the skills folders (personal,
  the project's, Vibe's; none when empty), then the connectors not
  connected yet: state `available` with `connect` or `set_up`, which the
  desktop core sends as `browse {source: "mistral"}`, never in
  `plugins`. Each MCP part carries, from its last status file, its words
  (`status`), `tool_count`, its tools by name (each switchable) and
  `error` (Some: it failed; display only); a remote one its `login`.
  `prompts`: the manifest's `dev.bise.prompts`, at most 3, blank ones
  out. A connector row carries its own `login` (it is its own server):
  `in` when ready, `out` when not connected, `pending` with no button
  while its connect runs (the core keeps its id with the open logins);
  none for every other kind. The `plugins` and `browse {mistral}` events
  carry `connectors: {kind: no_key|locked|fetch, why?}` when the
  connectors' cache has a problem (`catalog::problem`): the window words
  it from the kind (a locked keychain is never "connect your account").
  The core's add previews (`add::Held`) go when it stops or after
  `add::HELD_TTL` (30 min) without his answer, swept before each command.
- **Paths** (`bise_home::Home`): `connectors_cache()`, `marketplaces()`
  (`~/.bise/marketplaces.json`), `mcp_secrets_dir()`
  (`~/.bise/secrets/mcp-env/`).
- **Wire** (`bise_proto`, the desktop core's protocol):
  `app_rows::Plugin {id, name, kind, scope, title, what?, icon?, state,
  on, locked, action?, parts[], logins[]}`, `app_rows::PluginPart {id,
  kind, name, what?, on, status?, login?, tools[], tool_count?}`.
  `name`, `state`, `scope` and `what` keep their released meaning (the
  wire rule); `logins` stays on the wire for the released law but is
  always empty since desktop-v2026.10.16: a server's login is
  `parts[].login`.
  Commands: `plugins`, `plugin_set {name, on}` (`name`: any id; a bare
  name is that plugin), `plugin_connect {id}`, `plugins_refresh`,
  `plugin_add {add: mcp|import|folder|git|marketplace|agent}`,
  `plugin_login`, `plugin_logout`. Events: `plugins {project?, items}`,
  `browse {source, items}`. Fixtures: `rust/proto/fixtures/core_ev.jsonl`,
  `app_cmd.jsonl`.
- **Slash**: `/plugins [on|off|add|connect|login|logout|list|refresh]
  [<id>]` and `/skills [on|off] [<id>]` (`bise_proto::commands`; and
  `bise plugins on|off`). Bare, both open the Plugins screen (below);
  `/plugins add` opens it on the add menu.

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
    │                   ├─ every 2 s: plugins.json, roots, connectors' cache
    │                   │    → start/stop servers, rewrite D/mcp-index.txt
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
5. **Enable state.** `~/.bend-harness/plugins.json` (v2, one writer:
   `bend_plugins::state::set`, under a flock, tmp + rename, other keys
   kept): `{"disabled": [...], "enabled": [...], "parts_off": [...],
   "parts_on": [...]}`; `disabled`/`enabled` hold plugin ids (a real
   plugin's bare name, `mistral:gmail`, `skills:vibe`), `parts_*` part
   ids (`docs#skill:docs`, `linear#mcp:linear`,
   `linear#tool:linear/create_issue`). Everything is on by default. A
   disabled plugin is listed, its components are not loaded. **Opt-in**: a
   manifest with `"extensions": {"dev.bise": {"default": "off"}}` (bise's
   own extension, never reported as unsupported) loads only when its name
   is in `enabled`; `disabled` still wins. The same extension's
   `"prompts": [...]` is what to try a plugin with (the list's `prompts`):
   at most 3 kept, each at most 120 characters (`resolve::cut_prompts`,
   a longer list or line: warning `plugin.prompts.cut`); bise's built-in
   plugins have three each. A part is on only when its
   plugin is on, it is not in `parts_off`, and, for a plugin's tool, its
   server's `#mcp:` part is not off (`parts_on` never overrides an off
   owner or server). The resolver lists a loaded plugin's off skills and
   off servers in `off_skills` / `off_servers` (Info
   `plugin.skill.off`, `plugin.mcp.server_off`), never in `skills` /
   `servers` / `remotes`. `skills:bise` is locked on. `plugins on <id>` adds to
   `enabled` (`parts_on`) and drops from `disabled` (`parts_off`), `off`
   the reverse. The
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
   Any server also takes Codex's (and Vibe's) keys: `startup_timeout_sec`
   (or `startup_timeout_ms`; start, handshake and first `tools/list`,
   default 30 s), `tool_timeout_sec` (one call, default 300 s),
   `enabled_tools` (only these, the server's own names) and
   `disabled_tools` (never these; a tool left out is neither in the
   index nor callable), and `"enabled": false` (left out,
   `plugin.mcp.server_off`).
   A bad server: `plugin.mcp.server_invalid`, skipped.
8. **Unsupported components.** Present on disk and reported, never read:
   `ai.mistral.vibe/{hooks.toml,agents,knowledge}`, `connectors.json`,
   `libraries.json`, `ai.mistral.vibe/views` when the manifest lacks the
   `ai.mistral.vibe` extension, and any other manifest extension.
9. **Static views.** With the `ai.mistral.vibe` extension, each
   `ai.mistral.vibe/views/<name>/` (`view.json` + `index.html`, the
   name its folder's, inside the plugin, at most 100) is a view
   `ui://<plugin>/views/<name>`; a bad one: `plugin.view.invalid`, dropped.
   An mcp.json server id may not start with `~` (`~views` is the static
   views' route).

Every diagnostic has a code, a severity (`error` drops the plugin,
`warning` drops one component, `info`), the plugin name or root, and one
sentence. The resolver is pure over a file-system snapshot, so its unit
tests use temp dirs.

### Bridge (`bend-harness plugins serve`)

- Lazy stdio servers (`bend_plugins::tools_cache`): a stdio server runs
  only while an agent uses it. Its tools come from the cache every
  session shares (`Home::mcp_tools_cache()`, one file per server spec;
  24 h TTL). The cache's four rules (architect, t_40590; the holes were
  found by the mcp-cache-audit page):
  1. **Writes, cold lists only.** The cache is written by the launch's
     start-list-stop, or by a real start's `tools/list` taken before its
     first call is forwarded. A list after `list_changed` or after a call
     can be that session's own (FastMCP `enable_components`, the TS SDK's
     enable/disable): it updates that session's index only.
  2. **The key is what can change the list.** Plugin, server, command,
     args, cwd, the mtimes of mcp.json and plugin.json, our `initialize`
     params (`stdio::init_params`), and an HMAC-SHA256 (a per-home salt,
     `Home::mcp_tools_salt()`, 0600, made on first use) of the env the
     server starts with: each declared env value resolved as the start
     does (`secrets::lookup`: the env, then his typed-in secrets) and the
     inherited env minus `bise_home::env`'s internal and test names, the
     per-agent temp names (`env::PER_CHILD_NAMES`) and the bridge's
     `/bin/sh` names (`tools_cache::SHELL_NAMES`). A token or a
     `GITHUB_TOOLSETS` from the hub's env is in it (GitHub filters its
     tools by a classic PAT's scopes at startup); every agent of one hub
     shares one key; a hub restarted from another shell pays one cold
     start. A value is never stored or logged, only the hash. The env is
     not stripped (`env_clear`): users rely on inherited tokens.
  3. **A stale list is fixed once.** A `tools/call` answered -32602
     (`tools_cache::UNKNOWN_TOOL`, read from the typed code, never the
     text) lists that server again for the session (its index follows);
     the cache entry is dropped only when the called tool is missing from
     the fresh list (a bad argument keeps it), so the next cold start
     writes it again. The call gets the server's error plus `its tools
     now: ...` (at most `bridge::NAMES_SHOWN`, then `and N more`). Once
     per call, never a retry loop.
  4. **The shared host** (Later, below) is keyed by this key.
  A miss: one bridge starts it, lists, stops it and writes the
  cache under the key's flock; the other cold bridges wait at most the
  server's startup timeout, then read the list (or leave the server out,
  logged). The first call starts it, lists it (rule 1) and only then
  forwards the call. No call for 10 min
  (`tools_cache::IDLE_STOP`): stopped, the next call starts it again, so
  a server that keeps state between calls loses it then. Its status
  (`status::Status::idle`) says listed, not running (the Plugins list:
  "N tools · starts when used"), never an error. Remote servers hold no
  local process and connect at start as before. Measured (24 session
  bridges, 6 node + 1 python fake plugins, a throwaway home): before,
  168 server processes, 6424 MB RSS, with no call; after, 0 processes
  and 0 MB, every bridge still listing all 7 servers' tools; one call:
  1 process, 41 MB.
- Spawns every remote server of every enabled plugin (and a stdio one
  to list it, on a cache miss) in parallel, sends
  `initialize` (protocol `2025-06-18`), `notifications/initialized`,
  `tools/list`. 30 s per server unless its `startup_timeout_sec` says
  otherwise. The index files wait at most 12 s for them (the REPL waits
  15 s): a server still starting then is in the bridge without tools
  (`still starting` in the report; a call answers `<server> is still
  starting: try again in a moment`) and joins the index when it is up.
  A failure gives
  `plugin.mcp.connection_failed` (with the last stderr lines) and the
  server is left out. stderr goes to `D/<plugin>.<server>.log`.
- Tool names are made identifiers the same way as namespaces. Two tools
  of one plugin with the same name (from two servers):
  `plugin.tool.name_collision`, the later one is dropped.
- Writes, then touches `D/ready`:
  - `D/mcp-index.txt`: the session's one tool index (the REPL reads no
    other): one line per tool, `<cid> <namespace> <tool> : #<description>
    | input: <schema>` (` | ui: <uri>` last for a tool with a view; a
    tool only a view may call, `_meta.ui.visibility: ["app"]`, is never
    there: docs/mcp-apps.md C1), the plugins' tools first (`cid =
    http://127.0.0.1:<port>/<token>/<plugin>/<server>`), each plugin's
    static views (`cid = …/<plugin>/~views`), then his Mistral
    connectors' (`connectors::index_lines`, `cid` = the connector's id),
    minus every tool, server, connector or plugin turned off;
  - `D/skills-index.txt`: `name\tdescription\tpath` lines, every skill
    of this session that is on: the loose skill folders
    (`bend_plugins::skills`, scanned while the servers start) and the
    loaded plugins' skills, first name kept (workspace, plugins, bise,
    personal, vibe). The only skills index the REPL reads;
  - `D/report.txt`: the human report (the same text as the CLI);
  - with `--views-dir` (the hub's `BEND_VIEWS_DIR`): a `tools/call` of a
    tool with a view that carries `x-bise-call: <call>` leaves its whole
    result in `<views dir>/<call>.json` (docs/mcp-apps.md C2);
  - `D/bridge.json` (`bridge::BRIDGE_FILE`): `{"v":1,"base","pid"}`,
    where the hub reaches this bridge for a view's calls
    (docs/mcp-apps.md C5).
- Announces the MCP Apps extension at `initialize`
  (`views::client_capabilities`), so a server lists its tools' `_meta.ui`;
  a `resources/read` of a `ui://` URI is answered from the server once
  per start (`views::ResourceCache`).
- Serves HTTP/1.1 on `127.0.0.1:0`. The path carries a random token, so
  another local process cannot call the tools without reading `D`.
  JSON-RPC over POST: `initialize` answers with the server's cached
  result, notifications answer `202`, anything else is forwarded with a
  fresh id and answered as `application/json` (the server's
  `tool_timeout_sec`, 300 s by default; the REPL waits up to an hour
  for a plugin tool, 60 s for a connector). A server
  that died is respawned once on the next call.
- Every 2 s (`bridge::POLL`) it reads `plugins.json`, the roots'
  fingerprint and the connectors' catalog (`bridge::shared_catalog`: the
  cache every session shares, parsed only when the file moved or its
  catalog is past `connectors::TTL_MS`; `connectors::load` fetches a
  stale one under the cache's lock). A change re-resolves the plugins:
  a server on now and not running starts (turned on, added to an
  mcp.json, a new plugin; `still starting` until it is up), one off or
  gone refuses new calls at once, finishes the call in flight (at most
  10 s, `bridge::STOP_GRACE`) and stops; then `D/mcp-index.txt` and
  `D/report.txt` are written again. A tool, server or connector toggle
  never relaunches the REPL ("at their next call").
- A call to a tool turned off (the tool, its server or its plugin) is
  refused with `<namespace>.<tool> is turned off in plugins`
  (`bridge::off_error`), `plugins.json` read at the call: the agent
  learns it even before the index moves. A connector turned off has no
  line, so the REPL cannot reach the gateway for it.
- `${VAR}` in a stdio server's `env` and a remote server's `url` and
  `headers`: the env's value, else the one he saved for that plugin in
  `Home::mcp_secrets_dir()` (`bend_plugins::secrets`); neither and no
  default: the server does not start, `env X uses ${VAR}, which is not
  set` in the report (never a value).
- It stays up with no server (no plugin, or every server failed): it
  keeps the connectors' lines current.
- Polls the REPL pid every 500 ms; when it is gone, stops the servers
  (a short grace period, then a kill) and exits. `/reload` restarts the
  bridge with the new REPL.

### Bend runtime

- `repl-live` startup, before the skills scan and before the system
  prompt is built (it lists the skills): `Pl.start()` runs
  one `/bin/sh` script: clear `D`, start the bridge in the background
  (`$BEND_HARNESS_BIN`, else `./bend-harness`, else the dev build under
  `rust/target`), wait for `D/ready` (at most 15 s). No binary: no
  plugins, no error. Scripted runs never start it (hermetic suites).
- `BEND_HARNESS_BIN` is set on the REPL by `bend-harness --headless`
  and by the hub (`Opts.exe`).
- Skills: no scan in Bend. The catalog and the `skill` tool read
  `D/skills-index.txt` only, written by the bridge before `D/ready`; a
  missing one means no skills this session, said once on repl.err.
- MCP: `mcp_call` and `search_tool_functions` read `D/mcp-index.txt`
  only (`Mcp.index_content`); there is no shared connector index any
  more (no `BEND_MCP_INDEX`, no Bend bootstrap: the bridge's Rust writes
  every line). A scripted probe writes its own `D/mcp-index.txt`.
  `gateway_url(cid)` keeps a cid that starts with `http://127.0.0.1:` as
  is, and the Mistral key is not sent to it.

### UI and CLI

- `bend-harness plugins [list] [--workspace W] [--json]`: each plugin
  with scope, version, state (`loaded`, `disabled`, `shadowed`,
  `invalid`), root, skills, MCP servers, unsupported components, then the
  diagnostics. Static: it does not start servers.
- `bend-harness plugins on|off <id>`: a plugin or a part (any
  `PluginId`), through `state::set`; applies at the agents' next idle
  (below).
- The place's 'connect your tools' band draws `catalog::summary`'s
  suggestions; each one carries a why (`PluginSuggest.whys`, the same
  order as its titles). A Mistral connector says what bise saw in his
  projects first (`signals`: git remote hosts, tracked file names, the
  KEY names of `.env.example`-like files; never a value, never `.env`,
  nothing leaves the Mac; read by the desktop core off the send path, at
  most hourly), else its generic why (`settings.plugins_why_<tool>`, 60
  characters at most), else its description's first sentence cut at 60
  on a word. Connectors seen in more projects come first, then `USEFUL`.
  Imports, waits and a project's own plugins say their own reasons
  (`seen`). The card's tip keeps the description and where it was seen.
  Thread word counts (the history index) are not read: architect m_41049.

### Add (the add menu, `bise plugins add`, `bise plugins marketplace`)

One library both apps call (`bend_plugins::add`, `bend_plugins::marketplace`),
in two steps so nothing of a plugin runs before his click:

- `add::stage(&Request, &Ctx) -> Result<Staged, AddError>` builds the plugin
  in `<user root>/.staging/<token>/<name>` (`add::staging_dir`; a dot folder
  is never a plugin, for discovery and the fingerprint alike; folders older
  than a day are swept) and resolves it alone. `Request`: `Mcp` (a command
  line or a URL; his secret values), `Import` (Claude Code's
  `~/.claude.json`, Codex's `~/.codex/config.toml`, Cursor's
  `~/.cursor/mcp.json`, through `import::plan`; `add::found` lists them),
  `Folder`, `Git` (`add::git_clone`: `--depth 1`, no hook, no submodule, no
  `ext::`, no prompt, a timeout; GitHub's `owner/repo` works). A Codex
  (`.codex-plugin/plugin.json`) or Claude Code (`.claude-plugin/plugin.json`)
  plugin is converted: a 1.0 `plugin.json`, its `.mcp.json` through
  `import::plan_for(.., Home::Plugin)` (`${CLAUDE_PLUGIN_ROOT}` read as
  `${PLUGIN_ROOT}`), every key left out listed; Codex's
  `interface.defaultPrompt` (a string or a list) becomes
  `extensions.dev.bise.prompts`, cut by the resolver's own rule
  (`resolve::cut_prompts`: 3 of 120 characters; a cut is listed), its
  other `interface` keys listed by name. Its `Preview`
  (`bise_proto::app_rows::PluginPreview`): each server's exact argv (with its
  installed paths) or transport and host, its skills, the names of its
  secrets, what was dropped, notes (a built-in it shadows), `replaces`.
- `add::install(Staged, &Ctx)`: the secrets first, then one rename into
  `<user root>/<name>` (the commit point; bise's own MCP folder of that name,
  `import::is_bise_mcp`, is moved aside and put back on a failure). A name he
  has is `AddError::Exists`. `add::discard` drops a preview.
- Right after his confirm (never before: the preview forbids running it
  unseen; never on a timer), `add::try_servers(&dir, &Ctx)` starts each of
  its MCP servers once with the bridge's own start (`bridge::try_plugin`:
  initialize + tools/list within its startup timeout), stops it, and
  writes its status: the row shows `connected · N tools`, `✗ <why>`, or
  `needs a login` on a 401. A failed try keeps the plugin installed and
  on. The desktop core runs it on a thread after `plugin_add_confirm`
  (`SetupPorts::plugin_try`, `Done::Tried`: the list again).
- Remove (`bend_plugins::remove::remove(&Ctx, workspace, &PluginId)`;
  `plugin_remove {project?, name}`, `/plugins remove NAME` then `… yes`,
  `bise plugins remove NAME [--yes]`): only a plugin of his plugins folder.
  A built-in, a repo's own plugin, a Mistral connector, a skills folder or
  a part is `RemoveError::Refused` with its kind. The folder is renamed
  into `.staging` first (it never resolves half-removed; a crash is swept
  with the stale previews), then `secrets::remove` (a failure puts the
  folder back), then best effort: `state::forget` (its ids and its parts'
  out of `plugins.json`), its remote servers' logins through
  `login::logout` (kept when another plugin calls the same server),
  `status::forget`, the folder deleted. `${PLUGIN_DATA}` stays. Live
  sessions: each bridge's next poll retires its servers (a call gets
  `off_error`'s words), no `/reload`.
- The desktop core holds a `Staged` by token (`add::Held`): `plugin_add`
  (`add::Request::from_wire`: an MCP server's secrets by name only) answers
  `plugin_preview {token, preview}`, then `plugin_add_confirm {token,
  secrets}` (his values for the preview's secret names, a
  `bise_proto::app_cmd::Secret` that prints `•••`; a name the preview
  doesn't list refuses, a name left out stays `${NAME}`) or
  `plugin_add_cancel {token}`. The terminal holds it in its own process.
- Secrets (`bend_plugins::secrets`, the one owner of the layout):
  `<Home::mcp_secrets_dir()>/<escaped plugin id>.json` = `{"VAR": "value"}`
  through `bise_secrets` (file or keychain), `mcp.json` keeps `${VAR}` (a
  header `X-Api-Key` is `${X_API_KEY}`); the bridge reads them when the
  environment has no value.
- Marketplaces: his list in `Home::marketplaces()` (`{"marketplaces":
  [url]}`, one writer, flock); `marketplace::read` clones into
  `Home::marketplace_cache()/<hash>` (a failed refresh keeps the last one),
  reads `.agents/plugins/marketplace.json` (Codex's, `openai/plugins`) or
  `.claude-plugin/marketplace.json`; `marketplace::rows` are Browse rows
  (available, never on); `marketplace::request` is the git add with its
  folder.
- "Make a plugin with an agent": `add::agent_brief(words)`, sent to main the
  way "start an agent" is; the agent hands him a folder, added through the
  same preview and click.

### Reload on change

A plugin installed, removed, enabled, disabled or edited while bise runs
reaches every agent without a restart of the TUI (the user: a voice
recording, a draft, the scroll must survive). The hub keeps, per live
REPL, its workspace and the fingerprint of what its prompt shows of its
plugins (`daemon/prompt_inputs.rs`, architect B1): the `## Plugins`
note (every plugin on, its description, its skills on) and each of those
skills' name, description and path; never `plugins.json`'s bytes. A
server, a tool or a Mistral connector turned on or off, or an
`mcp.json` edit, moves nothing: the session's bridge applies it (its
index, its servers), so one toggle never relaunches every REPL (the
26-REPL burst, 959d1e474); a plugin or a skill turned on or off, a
plugin added or removed, a description or a SKILL.md edited does. Taken
at spawn. Every 2 s on the tick, and when an
agent goes idle, it compares ([`prompt_inputs::stale`]): a REPL whose fingerprint moved relaunches
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
Only REPLs restart: the TUI and the hub keep running; a relaunch goes
through `repl_start`'s limiter. Test: `prompt_inputs.rs`'s units (20
REPLs: a tool, server or connector toggle relaunches none; a skill or a
plugin toggle relaunches each once; an `mcp.json` edit moves nothing, a
SKILL.md edit does).

Skills follow the same path, with no timer. One scanner owns the loose
skill folders, `bend_plugins::skills`: the hub, the TUI's `$` popup and
the bridge call it. Its roots (`skills::roots`): `<workspace>/.agents/skills`
(`skills:workspace`), the app root's `prompts/skills-all` where the
desktop is on and `prompts/skills` for main (`skills:bise`, locked on),
`~/.agents/skills` (`skills:personal`), `~/.vibe/skills` (`skills:vibe`).
The hub picks an agent's roots and passes them to its REPL's bridge in
`$BISE_SKILL_ROOTS` (`skills::Root::{encode,decode}`; without it, a
bare REPL, the user's and the workspace's folders only). Each folder is
a plugin and each skill a part (`skills:personal#skill:grill-me`): an
off one is left out of the session's index. No read of a skill folder
blocks: each listing, stat and read runs on a thread waited at most 3 s
(the TUI: 300 ms), a file that is not a regular one (a FIFO) is never
opened, a stuck path is skipped, named once (hub.log, the bridge's
log) and not waited on again while it hangs. The hub also keeps, per
live REPL, `skills::fingerprint` of its roots (each `<skill>/SKILL.md`
path with its size and mtime, stats only, and the on/off of each folder
and of its skills: a skill toggle changes the prompt). It
compares it when an agent goes idle and right before a turn starts (the
`say` of an idle agent, `skills_before_turn`): a SKILL.md added, edited
or removed relaunches the REPL first, same session, and the `say` waits
in the switch queue, so the turn right after the change already lists
the new skill. `prompt-plugins.fp` holds both fingerprints, so the
restored session takes a fresh prompt. The workspace is the agent's
`BEND_WORKDIR` (its hub workspace): a task working in a `gate.sh new`
worktree still reads the shared folder's `.agents/skills`, like its
plugins and AGENTS.md. The TUI's `$` popup lists main's skills with
the same scanner (and the loaded plugins' skills, the off ones left
out), rebuilt when their fingerprint moves, checked when the popup asks
after a second without asking (`rust/tui/src/skills.rs`). Tests: the
unit tests of `rust/plugins/src/skills_tests.rs` (roots per role and
desktop flag, an off skill left out, `skills:bise` locked, the
fingerprint on a change and a toggle, a FIFO and a stuck read) and
`rust/tui/src/skills.rs`; `tests/skills_hang_e2e.py` (a stuck SKILL.md
never stalls main's first call).
- `/plugins` in the TUI opens the Plugins screen, full screen like
  `/scheduled` (plugins-ui's terminal screens, page plugins §5-§6):
  `rust/tui/src/plugins_screen.rs` is pure (its state, its lines from
  `catalog::rows`' rows at 150 and 80 columns, what a key does as an
  `Effect`), `plugins.rs` reads the rows (every 2 s while open) and runs
  the effects: space turns a row or a part on or off through
  `state::set` and the divider says when his agents see it (`at their
  next turn` for a skill, `at their next call` for a tool, a server or a
  connector), ⏎ opens a plugin's page (its parts, a server's tools
  folded under it, `1-3` put a `dev.bise.prompts` line in main's
  composer), `c` connects a connector (`connectors::connect_now` on its
  own thread), `l` logs in to a server, `b` Browse (the connectors not
  connected, then each marketplace), `a` the add menu
  (`rust/tui/src/plugins_add.rs`: `add::stage` off the draw thread, the
  preview, `add::install` on his ⏎). `/skills` is the same screen on the
  skills folders. Marks read data (`on`, `action`, `parts[].login`,
  `parts[].error`), never the status words. `/plugins list` still
  prints the static listing.

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
- `cargo test -p bend-tui plugins`: `/plugins` text, and the screen's
  render tests (`plugins_screen_tests.rs`: each state at 150 and 80
  columns, the keys' effects; `PLUGINS_SHOTS=<dir>` writes the frames).
- `tests/tui_plugins_tmux.py`: the screen in a real TUI on a throwaway
  hub and fake plugins (after the land, on the branch).
- `tests/plugins_live.py` (live model): a real
  `bend-harness --headless` session on a throwaway workspace loads
  `hello_plugin:greet` and calls `tools.hello_plugin.shout` from
  `run_typescript`.
- `tests/plugins_sb_live.py` (live model): the
  same from a Switchboard task on a throwaway hub.
- the laws (`tests/proof_run.py`: every `bend/proofs/*.bend`), the scripted e2e suite and the TUI tmux suite stay
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
  Redirects: a 307 or 308 (and, for a GET, a 301, 302 or 303) on the
  server's own origin is followed with the same headers, at most 10
  (`/mcp` moved to `/mcp/`); another origin is refused (`<host>
  redirected to another site (<other host>): refused`), so a token
  never leaves it.
  Timeouts: the server's `startup_timeout_sec` (30 s) to connect, list
  and hand-shake, its `tool_timeout_sec` (300 s) per call.
- **`tools/list`** follows `nextCursor`. On
  `notifications/tools/list_changed` (remote or stdio) the bridge lists
  that server again and rewrites `D/mcp-index.txt` (the REPL reads it at
  each search and call): that session's index only, never the shared
  tools cache (its rule 1).
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
  The login's client comes along: Claude Code's `"oauth"` as is,
  Codex's `oauth.client_id`/`client_secret`/`callback_port` and its
  `scopes` list in those keys. Codex's `startup_timeout_sec`,
  `startup_timeout_ms`, `tool_timeout_sec`, `enabled_tools` and
  `disabled_tools` are kept as they are (stdio and remote); its stdio
  `cwd` (an absolute folder) becomes `sh -c 'cd "$0" && exec "$@"'`;
  its `env_vars` need nothing (bise's servers get the whole
  environment). Still ignored and said: `required`, `tools.<name>`
  approval modes, `startup_readiness`, `supports_parallel_tool_calls`.
- **Login (OAuth).** A remote server without an `Authorization` header
  in its mcp.json can log in (`rust/plugins/src/oauth.rs`, `login.rs`;
  the MCP authorization spec 2025-06-18). Discovery: the 401's
  `WWW-Authenticate` `resource_metadata`, else
  `/.well-known/oauth-protected-resource[/path]`, then the authorization
  server's RFC 8414 / OIDC metadata (an older server without any: its
  origin's `/authorize`, `/token`, `/register`); a server whose metadata
  lacks PKCE S256 is refused; a metadata URL that times out or answers
  5xx is an error, never a guess of the default paths. The endpoints
  must be https (http only on 127.0.0.1/localhost), and the login page
  must be on the issuer's or the token endpoint's origin (the RFC 9700
  mix-up attack; Codex's rule and its Figma/Robinhood exceptions)
  unless the metadata says `authorization_response_iss_parameter_supported`:
  then the redirect's `iss` must be the issuer (RFC 9207), else the
  code is never sent. Client: mcp.json's `"oauth": {"clientId",
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
  good (a 400/401 OAuth error) drops the tokens and keeps the client;
  a token endpoint that is down (no answer, 5xx, 429,
  `temporarily_unavailable`) keeps them, and the access token is still
  sent until it expires.
  The client, in the 2025-11-25 spec's order: mcp.json's `clientId`;
  else, when the login server says `client_id_metadata_document_supported`
  and takes public clients (`none`), bise's client ID metadata document
  `https://bise.dev/oauth/client.json` (site/oauth/client.json: a native
  public client, `http://127.0.0.1/callback` and
  `http://localhost/callback` on any port, RFC 8252), no registration;
  else the one registered before; else RFC 7591 registration.
  Step-up: a 403 with `WWW-Authenticate: Bearer
  error="insufficient_scope", scope="…"` keeps that scope in the store
  (`wanted_scope`), the agent reads the login line below and /plugins
  says `needs a login`; the next login asks for the scopes granted
  before plus the wanted ones (the challenge's scope first, then
  mcp.json's, then the metadata's). Logout (`/plugins logout`, `bise
  plugins logout`): when the metadata named a `revocation_endpoint`
  (kept in the store), the refresh token then the access token are
  revoked there first (RFC 7009, best effort), then forgotten.
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
  needs a login: /plugins login`.
  The browser on another machine (bise over SSH; designer m_4625): the
  link is always shown under the opening line, `or open this link:
  <url>`, then `browser on another machine? log in there. the last page
  won't load: copy its address, then /plugins login linear <address>`;
  `/plugins login linear <address>` hands that address to the login
  that waits (its origin and path must be the redirect's, its `state`
  this login's): a wrong one says `▲ that isn't the address linear sent
  you back to. copy the whole address from the browser's address bar.`
  and the login keeps waiting; none waiting: `▲ no login to linear is
  waiting. /plugins login starts one.` The CLI prints `or open this
  link:`, the url, a blank row, `browser on another machine? log in
  there. the last page won't load:` / `paste its address here and press
  enter.` and reads pasted lines at a `> ` prompt (a wrong one: `that
  isn't the address linear sent you back to. paste the whole address
  from the browser's address bar.`, the prompt again). mcp.json's
  `"oauth": {"callbackUrl": "http://localhost:8765/oauth/back"}` (Codex's
  `callback_url`; its port is required) is the redirect bise sends; it
  listens on that port (on every interface when the host is not this
  machine: a forwarded port). CLI: `bise plugins login [SERVER]`,
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
  after a login, the step-up, the revocation, CIMD, a pasted address and
  callbackUrl); `tests/tui_mcp_login_tmux.py` (the quiet line,
  /plugins, the popup, both thread lines, both browser pages, the
  browser on another machine at 150 and 80 columns);
  `tests/plugins_ts_e2e.py` (a stdio, a remote, a list_changed and a
  late server's tools called as `tools.<plugin>.<tool>` in run_typescript
  and found by search_tool_functions, through the hub and live REPLs).

## Known limits

- A plugin's MCP servers and tools follow edits and toggles within one
  bridge poll (2 s); its skills only at the next session start or
  `/reload` (the prompt lists them).
- The Switchboard `/plugins` shows the static listing of the hub
  workspace, not the per-task bridge report.
- Before plugins merges into main (architect m_39445), owner plug-catalog:
  a test that sends `plugin_add_confirm` with a planted value through the
  desktop core and finds it in no file under the test home but
  `<mcp_secrets_dir>/<plugin>.json`, and a check that the core's command
  log, its /debug ring and the TUI's /debug view never keep a confirm's
  values. The desktop's recorder of sends: plug-desktop.
- Changing a secret of a plugin already added (a rotated key): not yet;
  today only `bise plugins add --mcp` again, or the add's confirm, writes one.
- The desktop's remove button on a user plugin's row (and its confirm
  dialog) is plug-desktop's; the core's `plugin_remove` is there.

## Later

- One shared server per hub (architect m_40002), only if the lazy
  servers' RSS measure says it is still needed: one 'plugin host' child
  process per hub, its code in `bend_plugins` (`host.rs`), never in the
  hub's daemon; the hub's shell only spawns, tags and reaps it (procs),
  sb-core decides nothing. Each session's bridge stays: its own index,
  its call-time off check (`StateV2::is_on`) and off words, and it
  forwards allowed calls to the host over a socket whose path comes from
  `Paths` (the hub dir), handed over in an internal env var row. Only a
  server that declares itself shareable (a `dev.bise` manifest flag,
  default per session) is shared: one with per-session state
  (PLUGIN_DATA, cwd, a login) stays per session. Its key is the tools
  cache's key (rule 2 above); "shareable" also means it sends no
  `list_changed` after a call; the host forwards a `list_changed` to
  every bridge it serves.
- An inbox item when a remote server needs a login.
- `ai.mistral.vibe` extension: `toolNamespace`, `toolOverrides`.
- A `/plugins` picker with per-server toggles (mcp.json's `"enabled":
  false` works by hand).
- Plugin descriptions in the system prompt (the spec's default guidance).
- Hooks, agents, knowledge.
