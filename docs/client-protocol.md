# The client protocol (hub.sock)

How a client talks to a project's hub: the terminal (`bise`), the desktop
core (`bise ambient-core`), `sb`'s one-shot questions, and any client you
write. JSON-RPC 2.0, one JSON object per line, on the hub's `hub.sock`.

`agent.sock` (the agents' `sb` ops) is out of scope: it is unchanged and
never speaks this protocol.

The code is the reference; this page tells you where to look and what
holds.

**A notification is a card on the wire and in the code.** What waits in
the user's inbox for his answer is a *notification* in every word he and
the agents read (`sb notify`, `notification #12`); its methods, events,
journal lines and fields keep the name `card` (`card/answer`,
`hub/cards`, `card_opened`, `"card": 12`) so old journals and older apps
keep working. Never rename them to match the words
(`docs/issues/27-notif-rename-aliases.md`).


| what | where |
|---|---|
| the envelope (Request, Response, Notification, error codes) | `rust/proto/src/jsonrpc.rs` |
| the methods and notifications (tables), `initialize`, the watermark | `rust/proto/src/rpc.rs` |
| every command and event (their fields are the params) | `rust/proto/src/hub.rs` (`HubCmd`, `HubEv`) |
| the rows (agents, cards, artifacts, pages...) | `rust/proto/src/rows.rs` |
| a thread's entries (the hub's fold) | `rust/proto/src/thread.rs`, `thread/` |
| the slash commands' catalog | `rust/proto/src/commands.rs` |
| the hub's side | `rust/switchboard/src/daemon/rpc.rs`, `daemon/proto/cmds.rs` |
| TypeScript types for a JS client | `rust/proto/tests/ts.rs` generates them (`--features ts`) |
| the laws | `rust/proto/src/rpc_tests.rs`, `rust/proto/tests/fixtures.rs`, `rust/proto/tests/released.rs` |

## The envelope

- A client sends a **request** `{"jsonrpc": "2.0", "id": 7, "method":
  "turn/send", "params": {...}}` and gets exactly one **response** with
  the same `id`: `{"jsonrpc": "2.0", "id": 7, "result": ...}` or `{...,
  "error": {"code", "message", "data"?}}`. The id is the client's (a
  number or a string); it is what correlates an answer.
- The hub sends **notifications** `{"jsonrpc": "2.0", "method":
  "hub/agents", "params": {...}}` (no id, no answer).
- A response is written **before** the notifications its action causes:
  after `turn/send`'s `{}`, the thread's new entries follow.
- A client sends a notification of its own once: `initialized`, after
  `initialize`'s result.

## initialize

The first line of a connection is `initialize`; anything else before it
gets `-32002` (and a second `initialize` too). The hub judges it like any
client connection (docs/issues/16: a process an agent started is refused,
`-32010`).

```json
{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"proto":1,"client":{"name":"my-client","version":"0.1"},"capabilities":{}}}
```

Its result, `rpc::InitializeResult`: the hub's identity (`project`,
`workspace`, `name`, `exe`, `state_dir`, `version`, `reload`,
`pages_url`), what it serves (`methods`, `notifications`: the names of
this hub's version, so a client can tell what an older hub lacks), and
`hub`: the hub-wide state now (`rpc::HubState {watermark, state}`:
`state` holds one hub-wide notification of each kind as it would be sent
now, unnumbered, since the watermark covers them). A client applies each
of them as it applies a notification. Then send `initialized`.

`rpc::init_answer` is the one reading of the line that answers
`initialize` (ready, refused, an older hub, another line): use it rather
than parsing the answer yourself.

There is no thread burst: a client reads the threads it shows with
`thread/subscribe` (below).

## Methods

One method per action or read. The table is `rpc::METHODS` (method name,
the `HubCmd` tag whose fields are its params, the `HubEv` tag whose fields
are its result when it is a read), plus the protocol's own
`rpc::OWN_METHODS` (`initialize`, `hub/read`, `commands/list`). Don't copy
the list: read it from the table, or from `initialize`'s `methods`.

- A method's `params` are its command's fields as they are, `project`
  included (the hub refuses another project's id).
- A **read** answers its event's fields (`thread/subscribe` the thread's
  last page, `diff/read` the diff, `*/list` the rows).
- An **action** answers `{}`; what it changed comes in the notifications.
  A method that answers the hub's words (`rpc::OWN_RESULTS`:
  `command/run`, `artifacts/add`, `version/info`...) answers a
  `CommandRunResult {notice?}`.
- A refusal is an error (below), never silence.

**command/run vs typed methods.** `command/run {agent, line}` is the line
he typed, as he typed it: the hub's one parser (`router::parse`) reads it
(plain text is a send, `@name text` a message to that agent, `/flow`,
`/stop`... a hub command) and runs the same handler a typed method would.
Use it only for words a person typed. A program sends with the typed
methods (`turn/send`, `turn/interrupt`, `card/answer`, `agent/new`...):
no parsing, no surprise when the words look like a command. The screen's
own commands (`client: true` in `commands/list`) never reach the hub.

`commands/list` answers the slash commands' catalog (name, usage,
arguments, completion kinds), for a client's popup and `/help`.

## Notifications

The table is `rpc::NOTIFICATIONS` (method name, the `HubEv` tag whose
fields are its params, and its scope). Three scopes:

- **Hub** (`hub/agents`, `hub/cards`, `hub/artifacts`, `hub/scheduled`,
  `hub/pages`, `hub/versions`, `release/progress`...): sent to every
  initialized connection, numbered. Each carries `epoch` and `seq` next to
  its fields.
- **Thread** (`thread/entry`, `thread/typing`): sent to the connections
  subscribed to that thread; the entry's `pos` is its number.
- **One** (`hub/notice`, `card/open`, `client/focused`): for one
  connection (a notice, a card to open), not numbered.

A notification method this client doesn't know (a newer hub) is ignored:
`rpc::ev` decodes it as `HubEv::Unknown`, never an error.

### What each agent does, who messages whom (les bims)

Two hub-wide facts, for every agent without subscribing its thread. Kinds,
names, ids and times only: no message body, no report text, no tool args.

| where | what |
|---|---|
| `hub/agents`, an agent's `doing` | `{kind, since_ms}`: `think`, `tool`, `edit`, `search`, `computer`, `image`, `publish`, `compact`, `build` (a newer kind: `unknown`). Set only while its turn runs, or while it is paused on a backgrounded command (`build`); absent otherwise. A tool call's kind is `bise_proto::rows::DoingKind::of_call` (one table, `RULES`). "Long" is the client's, from `since_ms`. It goes with the rows (the 100 ms state gate). |
| `hub/activity {project, item}` | `item` (`bise_proto::activity::ActivityEv`, tag `kind`): `message {id, from, to, mode: steer\|queued, expects_reply, at_ms}` (`from`/`to`: `{who: you}`, `{who: agent, name}`, `{who: bise}`, `{who: github}`), `report {agent, report: progress\|done\|failed\|blocked, at_ms}`, or `unknown`. Sent as sb-core journals the message or the report; a hub start replays none, so a client gets only what happens after its `initialize`. Numbered like every hub-wide notification: a gap is a loss, and nothing replays it (the rows stay true). |

### The watermark and resync

The hub-wide notifications are numbered by one counter per hub run:
`Watermark {epoch, seq}`. `epoch` is the hub run's start (ms): a restart
or a reload is a new epoch. `seq` moves by one per hub-wide event, and
every initialized connection gets every one of them, so a gap means a
loss.

A client keeps the watermark of the state it holds (from `initialize`'s
`hub.watermark`) and, for each numbered notification
(`Watermark::take`):

- `seq` at or below its own, same epoch: seen already, skip it;
- the next `seq`, same epoch: apply it;
- a gap, or another epoch: read the hub again, `hub/read`, whose result
  (`HubState`) replaces its hub-wide state and watermark.

The law (`rpc_tests.rs`): a reducer that does this ends equal to
`hub/read` over seeded runs of losses, duplicates, late lines and
restarts.

### Threads

- `thread/subscribe {agent, limit?}` answers the thread's last page
  (`HubEv::Thread {entries, before, more}`) and from then on its entries
  come as `thread/entry` and its step as `thread/typing` (`""` when the
  turn ends).
- An entry comes again when it changes (a tools entry growing, a card
  answered): the same `pos` replaces, a new `pos` appends.
- `thread/page {agent, before, limit?}` answers the entries before
  `before` (scrolling up). The hub reads a page through the transcript's
  offset index: its cost is the page's, not the thread's length.
- `thread/find {agent, needle, rid}` answers `thread_hits {rid, needle,
  hits: [{pos, n}], upto}`: every entry up to line `upto` that holds
  `needle` (case-insensitive, without overlaps, on the entry's find text:
  `bise_proto::thread::find`), oldest first, at most 5,000 (the newest).
  The hub searches off its loop; a newer find of the same agent on the
  same connection stops the older one, which then never answers. Match
  the entries after `upto` yourself as they come. An older hub refuses it
  (`hub_older`): search what you hold, and say the count is partial.
- `thread/unsubscribe {agent}` stops them.
- After a reconnection, subscribe again; the entries above the last
  `pos` you hold are new.

An entry is the hub's fold of the transcript (`bise_proto::thread::fold`):
a client never parses transcript lines.

## Errors

| code | meaning |
|---|---|
| -32700, -32600, -32601, -32602 | JSON-RPC's own: not JSON, not a request, no such method, bad params |
| -32002 | a request before `initialize`, or a second `initialize` |
| -32010 | refused: this client may not use this hub (docs/issues/16) |
| -32011 | the hub refused the command, `message` its words (`no agent named x`); `data.kind` an `ErrorKind`; a failed `turn/send` says `data.reason`: `refused` (nothing reached the thread) or `undelivered` (the hub wrote its undelivered line in the thread) |
| -32012 | this project's hub is older than the client: it doesn't serve that method (`data.kind: hub_older`) |

`data.kind` is `hub::ErrorKind` (`hub_older`, `hub_refused`, an unknown
value from a newer hub decodes as `Unknown`).

The MCP App views' methods (`view/resource`, `view/call`, `view/message`,
`view/context`; docs/mcp-apps.md C5) answer their own codes, the reason
typed in `data.reason` (`bise_proto::view_rpc::ViewError`): -32030
`expired`, -32031 `not-visible`, -32032 `plugin-off`, -32033 `denied`,
-32034 `not-supported`, -32035 `rate-limited`, -32037 `no-gesture`
(-32036 is never reused: a restarted server is a result's
`_meta["dev.bise/server-restarted"]`); -32000 `server-gone` when the view's server can't be
reached; a server's own error as it gave it.

## The wire rule

On the wire an existing field, list or enum value never changes meaning.
A new fact goes in a new optional field or list, a new kind in a new tag
or method (an older reader skips it); a new enum value only in an enum
whose released version already has `Unknown`.

It is held by a law: `rust/proto/tests/released.rs` reads
`rust/proto/fixtures/released/` (the fixtures of the last release, as it
wrote them, never edited) through today's types and checks that every
released key and value comes back unchanged. `fixtures.rs` checks that
every tag has a fixture line and round-trips; `rpc_tests.rs` that every
fixture goes through its notification or result unchanged.

## For one release only (gone)

The release that shipped this protocol (v2026.10.2-30) kept a few paths
for a hub or a client of the release before it. They went once the
release after had shipped (debt-t3):

- the hub's hello stub and older door: an older client's
  `{"op":"hello"}` gets `exe` and `reload`, then the connection closes;
  `HubCmd::Hello` is no command any more (an older `{"cmd":"hello"}`
  decodes as `Unknown`);
- the fallbacks of `sb`'s version switch at launch (`switch/ask.rs`) and
  of `bise stop` (`client::stop`) to an older hub's hello and op;
- the desktop core's older door (`older_door`, its typed hello and
  typed-line reader in `rust/tui/src/ambient/core/hubs.rs`).

A hub that doesn't serve `initialize` (`rpc::init_answer`'s `NotRpc`) is
never asked anything: `bise stop` and the switch say `rpc::HUB_OLDER`,
and the desktop core refuses that project's connection with those words
(kind `hub_older`, no reconnect until he retries). The core's home hub
is the one exception: the app owns it and moves it to its own version
(`home_switch.rs`). A command a newer-than-it hub lacks is another fact,
with its own words (`HUB_LACKS`: `this project's hub is older: update
bise`).

## After the release

Written where it can't be forgotten (the plan's list, architect m_13367):

- remove the four paths above;
- the hub owns the queue order (enqueue-style), not the clients;
- type the app <-> core wire's untyped rows (`app_rpc`'s CORE_EVS) into
  `CoreEv` (the commands are `AppCmd` now: docs/issues/24).
