# Feature flags

On/off switches so a feature that is not released yet can land on main and
ship in a release, off, while Gabriel uses it on his own Mac. Booleans only.
Design: page `feature-flags`, his pick `q-shape` (flags only in
config.toml), signed by architect (m_67929).

## The rule

1. A feature that is not released lands behind its flag, off by default.
   Its merge names it with the trailer `Bise-Flag: <name>` (release
   branches).
2. He turns it on for himself in `~/.bise/config.toml`:

   ```toml
   [flags]
   board = true
   ```

3. The release is the update that **deletes the flag and its checks**: the
   feature is then in every bise. A flag name is never reused.

Nothing is fetched and nothing leaves the Mac. To turn a released feature
off again for everyone: a fix release (release branches).

## The flags

One typed list, `bise_proto::flags::Flag` (`rust/proto/src/flags.rs`): each
flag has its name, its default (off) and the feature it hides.

No flag is live today: tasks shipped without one (v2026.11.19).

## What a flag reads as

`bise_proto::flags::resolve` (pure), the first that says something:

1. `BISE_FLAG="name=on,other=off"` (`on|off|true|false|1|0`), a test-kind
   variable: a hub gets it, no sb-core, REPL or agent command inherits it;
2. the `[flags]` table of `~/.bise/config.toml`: his hand file, bise reads
   it (`bise_home::flags`), never writes it; a value that is not a boolean
   is skipped and `bise flags` says so;
3. the flag's default.

`bise flags` prints each flag, its value and where the value came from
(`no flags` today).

## Wiring the first flag

Each flag wires only the surfaces its feature has. The full wiring for a
flag read by sb-core, the hub, main's prompt and the desktop app was built
and signed at **7a72d2f5b** (kept on the local branch
`sb/feature-flags-wiring`, for the then tasks flag): copy the parts the
feature needs from there.

- **sb-core** (a verb refused when off): `In flags {on}` in
  `rust/proto/src/core/input.rs`, `bend/hub/flags.bend` (keys `flag/<name>`
  in `R.last`), the gate in `core.bend`, laws `bend/laws/flags.bend` and
  proofs `bend/proofs/flags.bend`. A core with no flags input has every
  flag at its default.
- **the hub**: `rust/switchboard/src/daemon/flags.rs` reads the flags at its
  boot and sends them to sb-core before any client; `core_photo.rs`
  `resend_flags()` sends them again right after the `Link::Init` of an
  sb-core it revived (architect m_66560).
- **main's prompt**: `CmdDoc.flag` drops a command's rows when its flag is
  off (`prompts.rs`).
- **the app**: `CoreEv::Flags { on }` from the app core
  (`rust/tui/src/ambient/core/flags.rs`), the desktop store's `flags` slice
  and `useFlag(name)`. Before the core's first `flags`, the store must read
  each flag's default, not off.
- **a release hold** keyed on a flag (a cut refused while a held feature is
  on): it reads the registry; the generated list for it,
  `fixtures/flags.json` and `list_json`, is at 7a72d2f5b, and the hook in
  `scripts/release_branch.py`'s history.

When the feature ships, that wiring goes with the flag unless another flag
uses it.
