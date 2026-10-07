---
name: bise-pages
description: Make a page for the user instead of a long reply, with the bise kit, and publish it with sb page publish; then answer the notes the user leaves on it. Use when an answer is longer than a short message or has structure (a table, options to pick, numbers, a plan, sources to check), when the user will want to change it or pick from it, or for a draft of something that leaves (a mail, a post, an update for others): a weekly update, a comparison, a decision, a list to review. Also when a message says "you sent N notes on your page".
---

# bise pages: show it as a page

A page is a short HTML fragment (about 50 lines) of kit blocks. bise frames it, draws it in the
user's look and opens it; the user leaves notes, you publish the next version, the tab updates
in place. Text blocks need no CSS. When the page is about something to see (a feature proposal,
a screen, its states, looks side by side), draw it: a `ui` block with the kit's components, real
terminal cells for the TUI (`ui.md`). Never scripts.

This file is enough for a first version: publish the skeleton before you open anything else.
Next to it, read only what you need, when you need it: `ui.md` (mocks, states, pickers), `blocks.md` (blocks, plans, the lint),
`leaving.md` (send, post, drafts), `watch.md` (standing orders), `mentions.md` (what's asked of
him on Slack and mail), `promises.md` (what he said he'd do), `morning.md`, `meetings.md`,
`feedback.md` (bug reports), `about-you.md` (what bise keeps about the user).

## Page or one line

A page is for what the user will read, edit or act on: drafts, lists of items, plans,
comparisons. A fact, a yes or no, a status, a time is one line in your reply:
"what's my next meeting" → `14:30, Q3 renewal with Camille.`; "did the deploy pass" →
`yes, 12:04.` A page: "answer my mails", "the launch post for Slack", "plan the offsite",
"which venue".

## Before you write

Run `sb taste`: the user's taste rules (never a path like `~/bise/taste.md`). Follow them, and
publish every version with `--taste` (the page says `following your taste · 4 rules`). No rules:
no `--taste`. Who is who: `sb people`.

## Publish fast, then fill it

The user should see the page within about 10 seconds, then watch it fill in.
- **A started page:** when your brief gives a page id (main ran `sb page start`), the page is
  open and shows each command's one-line description live: say it in the user's words
  (`reading #launch in Slack`). Publish to that id.
- **Skeleton first**, before you gather anything: the title, the headings you expect and a
  `writing…` callout. Then publish again as each part is ready (same `--id`, same block ids:
  the tab swaps the blocks in place). Last publish: no `writing` callout, the sources added.
- **A page of items** (mails, comments, bugs): never read everything first. List them (subjects
  only), draft the first 2 or 3 and publish them within about 45 seconds of the ask, the callout
  saying `5 of 20 read`; then a version per 2 or 3 more; what you skipped last.

```html
<section data-kit="heading" data-id="title"><h1>the week, in three lines</h1><p>week 41 · for #team</p></section>
<section data-kit="callout" data-id="writing"><h3>writing…</h3><p>reading Slack and Linear for this week.</p></section>
<section data-kit="heading" data-id="h-numbers"><h2>numbers</h2></section>
```

Write the fragment in your temp folder, then:

```
sb page publish $TMPDIR/weekly-update.html --id weekly-update --title "weekly update" --taste
```

`--id`: lowercase, digits and `-`, the same for every version; `--title`: lowercase. It prints
`published weekly-update v1 · <url>`: say one line about it, never paste the HTML. A refusal
prints the lint's lines: fix each, publish again.

## Nothing leaves before his word

While you write and until the user's word on the page (`send`, `approve`, "put it in my
drafts"), nothing goes to their accounts: no draft in Gmail or Outlook, no message, no post, no
label. Then `leaving.md`. Any other write (close an issue, an invite) is on the page first as
its own `action` block (`data-do="close OPS-12"`); on his word, do exactly what he approved.
Code work is never an action: it is a `start an agent` item.

## The blocks

Each block is a `<section data-kit="…" data-id="…">`; ids short, unique, kept across versions.
- `heading`: `<h1>` once first (+ a `<p>` meta line above it), or `<h2>`/`<h3>`.
- `prose`: `<p>`, lists, `<blockquote>`, inline `<strong> <em> <code> <a> <sup>`.
- `table`: one `<table>` with `<thead>`, then an optional `<p>` caption; one number per cell.
- `callout`: an `<h3>` label + `<p>`; `data-tone` `note`, `good` or `risk`.
- `question`: a `<p>` + `<ol>` of 2 to 4 short options; answered: `data-answer="2"`. It makes
  a card; `data-card="none"` when it only waits on the page.
- `sources`: `<ol>` of `<li><a href="…">name · tool</a>, when</li>`, last; facts cite `<sup>n</sup>`.
- `review`: `<ol>` of `<li data-id>` to handle one by one; `data-verb` `send`, `open-reply`,
  `start`, `keep` (`blocks.md`). Items to handle (mails, reports, comments) are always a
  `review`, one item each, never headings and prose.
- `message`: a draft for a channel, `data-to="Slack · #launch"`, `<p>`s; never `<pre>`.
- `email`: `data-to`, `<p data-field="to">`, `subject`, the body; `data-verb="draft"` for his drafts.
- `compare`: 2 to 4 `<li data-id>` side by side: `<h3>`, `<ul>` of facts, `<p>` why.
- `ui`: a mock, terminal cells, states side by side, a rich picker, a timeline: `ui.md`.
- `checklist`: `<li data-id data-who data-due="2026-05-20">`, `data-done` when done. A plan
  worked through with him (buy the domain, set up the offsite): read "a plan" in `blocks.md`
  before you publish one.

**A draft of anything that leaves goes in its own block, never in prose:** a Slack message or a
post in a `message` block (`data-to="Slack · #team"`: `copy` and `open #team`), a mail or a
newsletter in an `email` block. In prose it can't be copied, sent or put in his drafts.

Words: the user's language and voice, lowercase headings, short sentences, concrete. Times in
his own time zone (`tomorrow 9:30`), never UTC.

## Answer the notes

A message `you sent 3 notes on your page "weekly update" (weekly-update v1, <url>)` comes with a
`<page-notes>` block (each note's id, block, quote, kind, words).
1. Change the page for each note; same ids for the same blocks; keep the user's own edits. An
   answered question gets `data-answer`.
2. Publish the next version: `sb page publish $TMPDIR/weekly-update.html --id weekly-update
   --notes-done n1,n3 --note-answer n2="the March numbers are not out yet: i kept February"`.
   Every note gets one or the other.
3. Reply in one line: `v2 is up: 3 notes done.`

A note with `taste: true`: also keep the preference, in general words, never the page's content:
`sb taste add "numbers: one per cell, the change in its own column" --from "the weekly update"`.
A note of kind `start` goes to main, not to you.
