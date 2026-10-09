# The client protocol (hub.sock)

How a client talks to a project's hub: the terminal (`bise`), the desktop
core (`bise ambient-core`), `sb`'s one-shot questions, and any client you
write. JSON-RPC 2.0, one JSON object per line, on the hub's `hub.sock`.

`agent.sock` (the agents' `sb` ops) is out of scope: it is unchanged and
never speaks this protocol.

The code is the reference; this page tells you where to look and what
holds:

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
- **One** (`hub/notice`, `confirm/ask`, `card/open`, `client/focused`):
  for one connection (a notice, a yes/no question), not numbered.

A notification method this client doesn't know (a newer hub) is ignored:
`rpc::ev` decodes it as `HubEv::Unknown`, never an error.

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
  `before` (scrolling up).
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

## For one release only

The release that ships this protocol keeps a few paths for a hub or a
client of the release before it. Each has a TODO naming this list; they
go in the release after:

- the hub answers an older terminal's `{"op":"hello"}` with only `{exe,
  reload}`, so that terminal re-executes as the hub's version (and gets
  this protocol);
- the desktop core falls back to the older typed hello (`{"cmd":
  "hello","proto":1}`) when a hub doesn't answer `initialize`
  (`older_door`, `Read::Older`: `rust/tui/src/ambient/core/hubs.rs`,
  `hub_rpc.rs`), and says `this project's hub is older: update bise` for
  a command that hub lacks;
- `sb`'s questions to a hub (`rust/switchboard/src/switch/ask.rs`, the
  version switch at launch, BISE-255) try `initialize` first and fall
  back to the older hello and `version` op (`rpc::init_answer`'s
  `Older`);
- `bise stop` (`switchboard::client::stop`) says `initialize` then
  `hub/stop`; an older hub gets the older hello and `stop_hub` op on a new
  connection (`older_stop`, `rust/switchboard/src/client.rs`).

## After the release

Written where it can't be forgotten (the plan's list, architect m_13367):

- remove the four paths above;
- the hub owns the queue order (enqueue-style), not the clients;
- type the app <-> core wire's untyped rows (`app_rpc`'s CORE_CMDS and
  CORE_EVS) into `AppCmd` / `CoreEv`.
