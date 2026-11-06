# 24 · the ambient core's untyped stdio lines

Status: done (debt-t6, architect m_23230): both lists are empty and
gone, every line is an `AppCmd` or `CoreEv` variant. Written by amb-core.
Label: protocol, desktop.

## The problem

The desktop app talks to `bise ambient-core` in JSON-RPC
(`rust/proto/src/app_rpc.rs`). Most names come from typed enums:
`AppCmd` and `CoreEv` (`rust/proto/src/app_cmd.rs`, `core_ev.rs`), `HubCmd` and `HubEv`.
Their fields have a Rust type and a generated TS type
(`apps/desktop/src/proto/*.ts`), and a field that changes on one side
breaks the other side's build.

Two lists in `app_rpc.rs` are still untyped. The core reads and writes
these lines as raw `serde_json::Value`, and the app reads them as
`Record<string, unknown>`. A renamed or dropped field fails silently,
with no red. It was accepted on the ambient-app branch to ship.

`CORE_CMDS` (gone, debt-t6): 19 commands were parsed by hand in
`rust/tui/src/ambient/core/cmd.rs`. They are `AppCmd` variants now, with
their fixture lines; cmd.rs decodes the line once and keeps only the
rules that are not shape (an agent's `@` and empty name, absolute files,
the history's limit). `answer`'s card and `every_stop`'s id are numbers
only (the old parse took `"#3"`; nothing sent it):

    shot, send, start, answer, hush, cut_in, quiet, voice,
    every_stop, fake_words, agent_preview, agent_history, agent_unwatch,
    agent_send, follow, stop, archive, unarchive

(`fn_context` went in the spotlight's ❝ change, architect m_53938: no app
sent it, and its selection hid in the model's text; his selection now goes
as a visible quote part.)

`CORE_EVS` (gone, debt-t6): 13 events were written as `json!` by the
core. They are `CoreEv` variants now, built typed and serialized at one
place (`Core::ev`):

    state, main, hub, sent, word, phase, page, agent_entry, agent_history,
    agent_preview, agent_queued, agent_typing, fake_voice

## The rule

- No new untyped tag. A new command is an `AppCmd` variant and a new
  event is a `CoreEv` variant, with its fixture line.
- The next change that touches an untyped tag also types it, in the same
  sha. Its row leaves `CORE_CMDS` or `CORE_EVS`, and its variant goes in
  `AppCmd` or `CoreEv`, with a fixture and the blessed TS. The app's
  reader moves to the generated type.
- The law `the_untyped_lists_only_shrink`
  (`rust/proto/src/app_rpc_tests.rs`) pins the lists as they are today.
  An added tag fails it. Typing a tag also fails it until that tag
  leaves the pinned list, in the same sha.

## Proof when done

Both lists are empty. Then the two consts, their law and this issue go.

Done: both consts and the law `the_untyped_lists_only_shrink` are gone;
this file stays as a record, like 10-23. What changed on the wire (only
tests read these lines):

- `agent_send`'s answer is `agent_sent {agent, mode}`, apart from the
  talk's `sent {text, voice, shot}`: two facts, two tags.
- an agents-panel entry (`agent_entry`, `agent_history`) is the hub's own
  `thread::Entry`: its kind `from_agent`, no more the panel's `from-agent`.
  An answered card's entry carries `question: ""` and `options: []`.
- `answer`'s card and `every_stop`'s id are numbers, never `"#3"`.

No `Value` field is left in these lines. The ones the types still carry
are older and not this issue's: `AppCmd::PrefsSet.value` and
`CoreEv::Prefs.prefs` (his preferences, until `bise_home::prefs` types
them).

Left open (architect m_23256): six tags are both an `AppCmd` and a
`HubCmd` (send, answer, stop, archive, unarchive, follow), so the app's
command union `AppCmd | HubCmd` (`CoreCmd`, the harness's `FakeCore`)
has two members per tag, and four playwright specs narrow them with
`'project' in c`. A renamed field would break that silently. The fix:
type each port with its own enum (AppCmd at the core's, HubCmd at a
hub's) so no reader narrows by field presence.

Lines only tests read (the core's Rust tests, `tests/ambient_core_e2e.py`
and the other e2e, `dev/plugins/ambient-qa`), never the desktop app:
ambient-lead's product call (delete or keep), as its own change:

- commands sent only by tests: `start`, `hush`, `cut_in`,
  `quiet`, `voice`, `every_stop`, `fake_words`, `agent_preview`,
  `agent_history`, `agent_unwatch`, `agent_send`, `follow`, and `send`,
  `answer`, `stop`, `archive`, `unarchive` without a project (the app
  sends those with one: `hub/<tag>`). The app sends only `shot`.
- events no window reads: `state`, `main`, `hub`, `sent`, `agent_sent`,
  `word`, `phase`, `page`, `agent_entry`, `agent_history`,
  `agent_preview`, `agent_queued`, `agent_typing`, `fake_voice` (the
  store knows their tags and applies none).
