# bise pages: meetings, a brief before and the follow-ups after

Read this when a timer wakes you with "brief …" or "follow-ups for …", or the user asks for
briefs before his meetings. One page per meeting, id `meeting-<slug>-<YYYY-MM-DD>`
(`meeting-q3-renewal-2026-05-19`): the brief is its v1, the follow-ups its v2. No card from
either, nor from anything on them, overdue or not: the menu-bar count says the page is there,
and his open rows reach the morning page through `sb page waiting`. So a meeting's checklists
never take `data-plan` (that is for a plan worked through with him, `blocks.md`), and a doubt
(an address, who owns an action) is a `question` with `data-card="none"` (the lint refuses one
without it here). Read `sb people` and the thread before asking him for an address or a name.

**An overdue promise of his** (he told Hélène he would send the SSO timeline by the 26th, and
no sent mail has it) leads the brief, right under the heading: a `callout` `data-tone="risk"`,
`<h3>overdue</h3>` and `<p>you told Hélène the SSO timeline in writing by 26 sep.</p>`, then
the drafted message in its own `email` or `message` block, waiting for his word.

**The timers.** When you make the morning page (`morning.md`), also set, for each of today's
meetings with other people, two one-shot timers in his time zone:
`sb every day 14:20 "brief Q3 renewal (meeting-q3-renewal-2026-05-19, bise-pages meetings.md)" --times 1 --until 23:59`
ten minutes before it starts, and `… "follow-ups for Q3 renewal (…)" --times 1 --until 23:59`
fifteen minutes after it ends (`--until 23:59`: a time already passed today never fires
tomorrow). A meeting cancelled or moved: stop or replace its timers (`sb every`, `sb every
--stop <n>`).

## The brief (v1), read in a minute before the meeting

1. `heading`: the meeting's title, the meta line `14:30 · 30 min · Camille Roux, Léa Martin`.
2. `## who`: a `prose` list, one line per person from `sb people` (`Camille Roux · buyer
   at Northwind, prefers mail to calls`); someone not in it: their name and role from the invite.
3. `## last time`: the last thread or meeting with them, two or three lines, linked (`<sup>`
   and `sources`): the Granola notes of the last meeting, the last mail or Slack thread.
4. `## open`: what is still open between you, as a `checklist` (`data-who`, `data-due`): his
   steps `data-who="yours"`, theirs by name, decisions waiting.
5. `## ask`: three questions to ask, a `prose` `<ol>`, short and concrete.
6. `sources`.

## The follow-ups (v2), from Granola's notes once the meeting is over

Republish the same id, the follow-ups on top, the brief folded into one line (`brief: who, last
time, open` linking to v1 through the version menu):
1. `heading`: `Q3 renewal: what was decided`, the meta line `today 14:30 · from Granola's notes`.
2. `## decided`: a `prose` list, one decision per line, each with its `<sup>` to the notes.
3. `## who does what`: a `checklist`, one row per action, `data-who` the person (his own
   `data-who="yours"`), `data-due` when one was said.
4. The follow-up drafts, each in its own block under the action it comes from: a mail in an
   `email` block (`data-verb="draft"` when he finishes it in Gmail), a Slack message in a
   `message` block. Nothing leaves before his word (`leaving.md`).
5. `sources`: the Granola notes, the invite.

No notes in Granola an hour after the meeting: no v2, one line in the morning page's `needs
you` (`no notes for the Q3 renewal: anything to follow up?`). See `examples/meeting-brief.html`
and `examples/meeting-followup.html`.
