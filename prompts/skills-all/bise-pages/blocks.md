# bise pages: the blocks in detail

Read this when you use a block beyond heading, prose, table, callout and sources, or when the
lint refuses a page. Every block is a `<section data-kit="…" data-id="…">`; ids are short,
unique, and stay the same across versions (the user's notes anchor on them): `p1`, `t1`, `q1`.

| `data-kit` | what | inside |
|---|---|---|
| `heading` | the page's title, or a section's | `<h1>` (once, first) + an optional `<p>` meta line; or `<h2>` / `<h3>` alone |
| `prose` | the text | `<p>`, `<ul>`/`<ol>`/`<li>`, `<blockquote>`, `<pre><code>`, inline `<strong> <em> <code> <a> <sup>` |
| `table` | rows to read or compare | one `<table>` (`<thead>` + `<tbody>`, `<th>`/`<td>`), then an optional `<p>` caption |
| `callout` | one thing not to miss | a first `<h3>` (a short label: `heads-up`, `not checked`) + `<p>`; `data-tone="note"` (default), `"good"` or `"risk"` |
| `question` | a decision the user makes | one `<p>` (the question) + `<ol>` of 2 to 4 short options; after the answer, `data-answer="2"` |
| `sources` | where each fact comes from | `<ol>` of `<li><a href="…">name · tool</a>, when</li>`, last on the page |
| `review` | items the user handles one by one (mails to answer, tickets, comments) | `<ol>` of `<li data-id="m1">`: a first `<p>` saying what it is (`Léa Martin · offsite dates · 9:12`), then the proposal. `data-verb` on the block: `send` (drafts that leave: `send`, `send all 3`), `open-reply` (comments on read-only places, `watch.md`), `start` (feedback an agent can fix, `feedback.md`), `keep` (`about-you.md`); none: `approve` |
| `message` | a draft that leaves through a channel (Slack, a post) | `data-to="Slack · #launch"` (where it goes, shown above it), then `<p>`s and `<ul>` with `<strong> <em> <code> <a>`: `copy` turns them into Slack markup; optional `data-open` = the channel's `slack://` or `https://` link |
| `email` | one mail, or a newsletter | `data-to="Gmail · reply to Camille Roux"`, `data-verb="draft"` when it goes to his drafts (`leaving.md`), `<p data-field="to">`, optional `cc`, `<p data-field="subject">`, optional `preview` (a newsletter), then the body as `<p>`s and `<ul>` |

A `data-to` is a short label in the user's language, whatever language the draft itself is in:
`Slack · reply to Benjamin, on French`, never `on français` (cards and batch lines read it).
| `compare` | 2 to 4 options side by side (venues, drafts, tools) | `<ol>` of `<li data-id="moulin">`: an `<h3>` name, a `<ul>` of facts, a `<p>` with the why |
| `checklist` | what's left to do, who, by when | `<ol>` of `<li data-id="t1" data-who="Camille" data-due="2026-05-20">` the task `</li>`; `data-done` when done; the kit shows `Camille · friday` and what is late. `data-who="yours"`: a step only the user can do (a payment, a login): `you · friday`, and a card when every step before it is done; your own steps: `data-who="bise"` or none |

**A plan you work through with him** (buy the domain, set up the offsite): its checklist is
marked `data-plan` (`<section data-kit="checklist" data-id="steps" data-plan>`): only then do
his rows become cards, each in its turn. Any other checklist (a meeting's open items, promises)
never has it: his rows there reach him through the morning page. Take every step you
can yourself, and mark `data-who="yours"` only what needs his hands or his word (signing in, a
2FA code, a payment, a decision). Publish the rows first, before any draft or check: every step
in order, who does it, a short line each, up within about 30 seconds. A check that gates a
purchase or a step you can't undo (the trademark before paying) comes before it. Then take your
steps one by one and publish again as each is done: a mail or a message as a draft in its own
block (`email` / `message`, it waits for his word), a check run and its result.
Each of your rows says what you did, `data-did="drafted"` or `data-did="checked · it loads"`,
and where the draft is, `data-draft="<its block id>"`: the row shows `bise · drafted · in the
page` and takes him there. Someone else's step (`data-who="Lucas"`): draft the message that
asks them, the same way. See `examples/buy-domain.html`.
Never end with a step of yours open: what you can't finish becomes a `question` to him on the
page (`legal's address?`) or a draft waiting for his word (`data-did="drafted"`). While the plan
has open steps, stay on it: end each turn with `sb report progress "<one line>"`, never done
and never a plain last answer, so his answers and ticks come to you; then do the next step and
publish again. His `send` or `approve` on a row or on its draft is his word: send it now with
the tool, then publish the row `data-done` with `--went` (`leaving.md`). His `send all` (one
card for 2+ drafts) approves them all in one message: send each, then one publish with every row
`data-done` and a `--went` per draft.
A step that is a decision of his (how many years, which venue) is the `question` only, and its
row points at it: `<li data-id="s2" data-who="yours" data-question="years">`; never a row and a
question that both ask him. His answer ticks it: republish the row `data-done` with his pick
(`2 years`) in the turn his answer reaches you (once the question is answered, an open row of
his becomes his next card). When he says he'll do something ("i'll leave a note with legal's address"), never
ask it again: the row says it in one line (`waiting for your note: legal's address`). When his
note comes, republish in one version both the row (no more `waiting for your note`) and every
draft it fills (the mail's To), before the next step becomes his turn: his `send the draft?`
never asks him to send a draft that still says it's waiting.
Whatever you need from him to go on is ALWAYS a `question` (`who is legal?` · `i'll leave a
note` · `ask Marc` · `skip this step`), its row pointing at it with `data-question`: never only
row text or a callout, which give him no card and stall the plan. A draft missing what it needs
to leave (no address in its To) is not drafted: no `data-did`, and its row asks that question;
the lint refuses it otherwise. Once he answers, the row says what's next in one line (`waiting
for your note: legal's address`) until you republish the draft complete.

Also allowed inside blocks: `<br>`, `<kbd>`, `<del>`, `<ins>`, `<mark>`, `<small>`, `<sub>`,
`<img src="data:image/…" alt="…">`. Nothing else. Review and checklist items may carry
`data-agent="<agent name>"` (its live line, `feedback.md`); review items `data-reply` (`watch.md`).

## Writing them well

- **Sources:** every fact you took from somewhere gets a `<sup>n</sup>` right after it, n = its
  line in the `sources` block. A fact with no source says so in the text.
- **Numbers:** as they read (`412`, `4.1 s`, `€12,400`). One number per table cell: `11 (10
  new)` is two columns (`total`, `new today`). In prose, a key number in `<code>`.
- **Items** (review, compare, checklist) keep their `data-id` across versions. Each reaction
  comes back to you as a note with the item's id: review `approve` / `skip`, email `approve` /
  `skip`, compare `keep` / `drop` / `pick`, checklist `tick`. Sending, replying or booking is
  still your tool call, after the user's word.
- **Drafts** (a Slack message, a post, a reply) go in a `message` block, a mail in an `email`
  block: never in `<pre>` or `<code>`, those are for code.
- **Questions:** one per decision, options short enough for a button; the pick comes back as a
  note.
- **Words:** the user's language and voice; lowercase headings, short sentences, concrete.

## The lint

`sb page publish` refuses, one line per problem, and stores nothing: `style`, `class` or `id`,
`<script>`, `<iframe>`, `<link>`, `<div>`, `<span>` or any element not listed above, `on…=`
handlers, an unknown `data-kit` or `data-` attribute, a block without `data-id` or with one
already used, text outside a block, a block inside a block, `<!doctype>`/`<html>`/`<body>`,
images from the network (use a `data:` src), `javascript:` links, more than 1 MB. Each line
names the block and what to do: fix it and publish again.

## An example: the weekly update

```html
<section data-kit="heading" data-id="title">
  <h1>the week, in three lines</h1>
  <p>week 41 · for #team · draft 1</p>
</section>

<section data-kit="prose" data-id="p1">
  <p>signup is faster: <code>4.1 s → 0.9 s</code> on a mid-range phone<sup>1</sup>. dark mode ships on the settings page monday<sup>2</sup>.</p>
</section>

<section data-kit="heading" data-id="h-numbers"><h2>numbers</h2></section>

<section data-kit="table" data-id="t1">
  <table>
    <thead><tr><th>metric</th><th>last week</th><th>this week</th></tr></thead>
    <tbody>
      <tr><td>signups</td><td>412</td><td>468</td></tr>
      <tr><td>signup time (p50)</td><td>4.1 s</td><td>0.9 s</td></tr>
    </tbody>
  </table>
  <p>from the growth dashboard, monday 9:00<sup>3</sup></p>
</section>

<section data-kit="callout" data-id="c1" data-tone="risk">
  <h3>heads-up</h3>
  <p><strong>release 2.5 slips to wednesday.</strong> the checksum didn't match on friday<sup>4</sup>.</p>
</section>

<section data-kit="question" data-id="q1">
  <p>post it in #team now, or wait for Marc's check?</p>
  <ol><li>post now</li><li>wait for Marc</li></ol>
</section>

<section data-kit="sources" data-id="src">
  <ol>
    <li><a href="https://acme.slack.com/archives/C01PERF/p1712">#perf · Slack</a>, thursday</li>
    <li><a href="https://linear.app/acme/issue/DARK-12">DARK-12 · Linear</a></li>
    <li><a href="https://metabase.acme.dev/dashboard/7">growth dashboard · Metabase</a></li>
    <li><a href="https://acme.slack.com/archives/C01REL/p1713">#release · Slack</a>, friday</li>
  </ol>
</section>
```

More examples, each passing the lint, in the bise app's `kit/examples/`:
`offsite-venues.html` (a table to choose from), `pricing-question.html` (a decision),
`inbox-review.html` (replies to send and a mail), `inbox-writing.html` (the same at v2, while
writing), `launch-post.html` (a Slack post), `offsite-plan.html` (venues to compare, what's left),
`offsite-todo.html` (who does what by when), `launch-watch.html`, `feedback-bridge.html`,
`newsletter.html`, `morning.html`, `about-you.html`.
