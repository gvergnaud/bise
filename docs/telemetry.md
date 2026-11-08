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
- features: `artifact_opened`, `plugin_enabled`, `connector_call`, `voice_started`, `dictation_started`, `computer_use_call`, `search`
- setup: `setup_started`, `setup_done`, `setup_failed_auth`, `setup_failed_network`, `setup_failed_other`
- crashes: `crash_tui`, `crash_app_renderer`, `crash_app_child`, `crash_core`, `crash_hub`

`minutes` (in steps of 5): `agent_work` (the agents' turns added up, at
most 14400: ten agents a whole day), `app_open`, `app_focused`,
`app_engaged`, `tui_attached`, `tui_focused` (each at most 1440).

`hist`: `peak_agents`, the most agents working at once in a hub, one
observation per hub and day, in the buckets 1, 2, 3-4, 5-8, 9-16, 17+.

`milestones`: `setup`, `first_agent`, `first_artifact`, `first_land`, each
with its delay: `under_1h`, `under_1d`, `under_7d`, `later`.

## What is never sent

No text: no prompt, message, brief, file, path, name of a project or an
agent, URL, error text or key. A key is a value of a closed enum, never a
string read from outside. The PII law (`bise_usage::law`, its tests in
`rust/usage/src/law_tests.rs`) feeds PII-like data through the crate's
inputs and checks the bytes of the body: only the schema's fields and keys,
strings from closed lists, no `/` or `@`, no word of the fixture, numbers
within bounds.
