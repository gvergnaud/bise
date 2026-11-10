# Usage telemetry

What bise counts about its own use, and what it would send. The plan is
the page `usage-metrics-plan`; the code is the crate `rust/usage`
(`bise_usage`). Today nothing is counted or sent: the global switch
`bise_usage::SENDING_ON` is off until step 9 of the plan, which turns it
on together with `site/privacy.html`.

schema version: 1

The machine-readable schema is `rust/usage/schema.json`, generated from the
crate (`bise_usage::schema::schema`). A test fails when the code's schema
and that snapshot differ, and when this page misses one of its names: a
new key changes the code, the snapshot and this page in one commit
(`BISE_USAGE_BLESS=1 cargo test -p bise-usage` rewrites the snapshot).

## The gate

`bise_usage::sharing_allowed` is the one rule. Counting, writing a day's
file and sending a packet all need its permit. It is off when any of these
holds, in this order: the global switch is off; `BISE_TELEMETRY=0`;
`DO_NOT_TRACK=1`; `CI` is set; a test home (a cargo test's temp HOME or
`BISE_TEST_HOME`); the app runs off-screen; a dev build; his
`telemetry.share` is false (prefs.json; unset reads as on); his note about
it was not shown yet (`telemetry.note_seen`). The one send allowed without
the permit is the delete request.

## Where it lives

Under `~/.bise/usage/` (paths from `bise_home::Home::usage_*`): one folder
per UTC day with one file per counting process (`hub-<8 hex>-<pid>.json`,
`core-<pid>.json`, `tui-<8 hex>-<pid>.json`: the hex is the hub id's hash,
never the project's name), `sent/`, `send.lock`, `state.json`,
`install-id`, `delete-request` and `retry.json`.

## The sender

Each hub and the app's core run `bise_usage::hourly` on a thread of its
own; each hour it takes a non-blocking flock on `send.lock`, so one
process of the machine sends (`bise_usage::sender::tick`). In order:

- a pending "Delete my data" request (`delete-request`) is posted first,
  without the permit; its id is deleted after a 2xx only. A delete is
  queued only if the server ever took a packet from this Mac;
- sharing off by his choice (his pref, `BISE_TELEMETRY=0`,
  `DO_NOT_TRACK=1`) deletes the spool, `sent/`, `state.json` and the id
  (`bise telemetry off` does it at once); off for another reason (the
  switch, a dev build, a test, CI, the note not seen) changes nothing;
- with the permit: the spool keeps 30 days (the oldest goes first),
  `sent/` 7; the closed UTC days, 15 min after midnight, are sent oldest
  first, at most 30 a body of at most 64 KB. A 2xx moves each packet to
  `sent/<day sent>/<day>.json`; a 4xx drops them and the hub's log says
  so; no answer keeps them and waits (`retry.json`: from a minute,
  doubling up to 6 hours, with jitter).

`bise telemetry show` prints whether sharing is on, the id, the next
packets and those sent; `on` and `off` set `telemetry.share`.

## The body

`POST /v1/packets` carries `{"packets": [...]}`, at most 30 packets, one
per closed UTC day. `POST /v1/delete` carries `{"install_id": "<uuid>"}`.
Both are plain JSON (`bise_usage::encode`), sent uncompressed by the
system's curl (`bise_usage::curl`: no cookie, no user agent, his
`~/.curlrc` not read, 10 s at most). The busiest day possible is about
1.3 KB, so 30 days fit one body.

## A packet's fields

| field | what |
|---|---|
| `schema` | the schema version |
| `day` | the UTC day, `YYYY-MM-DD` |
| `install_id` | a random UUID v4 drawn on this Mac, renewed after 13 months at the latest, deleted when sharing is turned off; never the host id |
| `cohort_week` | the ISO week of the first launch, `YYYY-Www` |
| `active_days_7` | active days among the 7 that end on `day` |
| `active_days_28` | active days among the 28 that end on `day` |
| `first_today_of_week` | `day` was the first active day of its ISO week |
| `version` | the build's version, `[0-9a-z.-]`, else `unknown` |
| `channel` | `stable`, `beta` or `dev` |
| `surface` | which surfaces counted: `desktop`, `terminal`, `both`, `headless` |
| `os_major` | macOS's major version |
| `arch` | `arm64`, `x86_64` or `other` |
| `counts` | key → how many times, 1 to 999 |
| `minutes` | key → minutes, a multiple of 5, capped per key |
| `hist` | key → one count per fixed bucket, each at most 999 |
| `milestones` | milestone → delay since the first launch, sent once |

A day is active when he wrote to an agent or answered a card.

## The keys

`counts` (each at most 999):

- agents: `agent_created_by_user`, `agent_created_by_main`, `agent_created_by_agent`
- his activity: `user_message`, `card_opened`, `card_answered`
- turns: `turn_done`, `turn_failed`, `turn_stalled`, `compaction`, `provider_retry`
- schedules and triggers: `schedule_set`, `schedule_fired`, `trigger_set`, `trigger_fired`
- work: `page_published`, `worktree_placed`, `land`
- the inbox: `inbox_opened`
- artifacts opened, by the app's kind of artifact: `artifact_opened_page`, `artifact_opened_doc`,
  `artifact_opened_sheet`, `artifact_opened_slides`, `artifact_opened_image`,
  `artifact_opened_link`, `artifact_opened_code`, `artifact_opened_other` (any other kind)
- features: `plugin_enabled`, `connector_call`, `voice_started`, `dictation_started`, `computer_use_call`, `search`
- setup: `setup_started`, `setup_done`, `setup_failed_auth`, `setup_failed_network`, `setup_failed_other`
- crashes: `crash_tui`, `crash_app_renderer`, `crash_app_child`, `crash_core`, `crash_hub`

`minutes` (in steps of 5): `agent_work` (the agents' turns added up, at
most 14400: ten agents a whole day), `app_open`, `app_focused`,
`app_engaged`, `tui_attached`, `tui_focused` (each at most 1440).

`hist`: `peak_agents`, the most agents working at once in a hub, one
observation per hub and day, in the buckets 1, 2, 3-4, 5-8, 9-16, 17+.

`milestones`: `setup`, `first_agent`, `first_artifact`, `first_land`, each
with its delay: `under_1h`, `under_1d`, `under_7d`, `later`.

## Who counts what

The hub (`switchboard::usage_count`, pure; its shell
`daemon/usage_spool.rs`) counts from facts already decided, live only (a
boot replay of the journal runs no effect, so a restart counts nothing
twice):

- sb-core's journal effects, by an exhaustive match on the line's kind
  (`bise_proto::core::JournalKind`): `task_created` by its parent (him,
  main, another agent), `message_sent` from him, `card_opened`,
  `card_closed` with his answer, `every_set`, `every_fired` (delivered),
  `wake_set`, `wake_end` on its event (`hit`), `worktree_placed` without
  an error; every other kind is "not counted" in so many words;
- the live REPL lines (never the week's transcripts), exhaustive on the
  observation: `turn_done` completed or failed (interrupted: neither),
  `turn_stalled`, `compaction_started`, `provider_retry`; `agent_work` is
  the time between an agent's lines inside a turn, a silence over 15 min
  taken out; `peak_agents` is the day's most agents in a turn at once;
- the shell where it happens: a land that moved a ref, a page published.

It writes its tally every 5 minutes and at a clean stop, with the permit
read again each time (off: the counts are dropped). Not counted by the
hub: `connector_call`, `computer_use_call` (a typed obs line from the
runtime, plan step 10), `crash_hub` (step 10), `search` (the desktop).

A TUI attached to a hub (`bend_tui`'s `usage_count`, its source
`tui-<8 hex>-<pid>.json`) counts its own time and nothing it reads:
`tui_attached`, the time its loop ran (a span that held the loop counts
5 min at most), `tui_focused`, the part of it its terminal had the focus
(focus reporting), and `crash_tui`, the panic logs it wrote to `crashes/`
(a log never leaves). It writes every 5 minutes and when it ends, a panic
included, through `bise_usage::spool::sink`, which asks for the permit at
each write.

## The app's counts

The app's core (`bise ambient-core`) counts with a `Usage`
(`rust/tui/src/ambient/core/usage.rs`), given only when
`sharing_allowed` says yes (`bise_usage::spool::sink`, its source
`core-<pid>.json`, which asks for the permit at each write):

- each app command by its name, through one match with no `_ =>`
  (`of_cmd`: the palette is `search`, a plugin turned on `plugin_enabled`,
  fn's talk and voice mode `voice_started`, the mic `dictation_started`;
  his words and answers are the hub's to count);
- the setup's provider check at its end (`of_check`): `setup_done`, or its
  failure's class;
- `usage_time`, a command Electron's main sends once a minute and before
  quit: the seconds open, focused and engaged from its clock
  (`apps/desktop/src/main/usage/clock.ts`: open = the Mac awake and
  unlocked, focused = a bise window in front, engaged = focused with his
  input in the last 60 s), and the gestures the core doesn't see (the
  inbox, an artifact by kind, the onboarding shown, a renderer, child
  process or core crash). The window sends main only a signal for his
  input (no key, no content) and a gesture's closed name
  (`apps/desktop/src/app/usage.ts`). An off-screen run starts none of it.

## The server

bise.dev's Vercel project (`site/`) receives and shows the packets
(plan steps 7 and 8; architect m_58375). Functions in
`site/api/telemetry/` (TypeScript `.mts`, the committed `.mjs` is what
Vercel runs; no build step, no dependency), region `fra1`:

- `telemetry.bise.dev/v1/packets` and `/v1/delete` (the app's
  `DEFAULT_BASE`; `site/vercel.json` rewrites that host's two paths to
  `api/telemetry/packets` and `delete`). A body is taken whole or refused
  with a 4xx: JSON (plain or gzip), at most 64 KB, every field, key and
  value checked against `_schema.mts`, a byte copy of `rust/usage/schema.json`
  (the validator reads the schema's types, never a list of its own), each
  day closed (before today, UTC) and at most 40 days old. No store yet: 503,
  the app keeps its days.
- the store: Neon Postgres (Vercel Marketplace, Frankfurt), reached over
  Neon's HTTP SQL endpoint by `fetch` (`_sql.mts`). One table, `packets`, a
  row per install and day (`PRIMARY KEY (install_id, day)`, a resend is
  kept once), made at first use. Rows older than 13 months are deleted at
  each ingest and by a daily cron (`/api/telemetry/purge`, no secret: it
  only deletes what is past retention). "Delete my data" deletes every row
  of that id.
- `bise.dev/telemetry`: the dashboard, behind one password (Basic auth,
  `TELEMETRY_DASHBOARD_PASSWORD` in the project's environment; unset, it
  shows nothing). Its reads count installs per cell and never select
  `install_id`; a number from 1 to 9 installs shows `< 10`.
- never kept: the functions read the body and the content type, encoding
  and length (and the dashboard's Authorization), never the address, the
  user agent or another header, and log nothing. Vercel's own platform
  logs see the request's address for a while: the privacy page says so at
  step 9. No log drain and no Web Analytics on that project.

Tests: `tests/site_telemetry.test.ts` (the crate's real bodies from
`rust/usage/tests/fixtures`, each refusal, the routes end to end on a
throwaway Postgres behind a fake of Neon's endpoint). After a deploy, by
hand: `node scripts/telemetry_smoke.ts` (one test packet in, then deleted).

## What is never sent

No text: no prompt, message, brief, file, path, name of a project or an
agent, URL, error text or key. A key is a value of a closed enum, never a
string read from outside. The PII law (`bise_usage::law`, its tests in
`rust/usage/src/law_tests.rs`) feeds PII-like data through the crate's
inputs and checks the bytes of the body: only the schema's fields and keys,
strings from closed lists, no `/` or `@`, no word of the fixture, numbers
within bounds.
