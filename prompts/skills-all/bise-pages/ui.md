# bise pages: draw it (the ui block)

Read this when the page is about something to **see**: a feature proposal, a TUI screen, a
flow, the states of a thing, two looks side by side, options that are designs. Words alone make
the user imagine the screen; draw it instead, the way designer's mocks did (bise.dev/m/timers,
bise.dev/m/artifacts): real terminal cells, dark and light, 150 and 80 columns, every state.

Stay in text for mails, plans, lists to review, weekly updates, numbers: the text blocks are
better there.

## When to draw

| the page is… | draw |
|---|---|
| a TUI feature or change | the screen in a `k-term`, tabs for 150 / 80 columns or for the steps |
| a thing with states (empty, working, failed, done) | a `k-states` grid, one small frame per state |
| a change to something that exists | a `k-ba`: today, proposed |
| a change to code, a prompt, a config: lines that go and come | a `k-diff`: a diff on a page is a k-diff, never drawn by hand, so readers can note its lines |
| a choice between designs | a `k-pick`: each option with its picture and its trade-off; the pick is a note |
| a plan with steps and owners | a `k-timeline` |

A text heading, a callout `in short`, then the drawings, each under its own `h2` heading block.

## The block

```html
<section data-kit="ui" data-id="screen">
<figure class="k-term" data-ui="term">
<div data-ui="tabs">
<div class="k-tabs"><button type="button" data-tab="w150">150 cols</button><button type="button" data-tab="w80">80 cols</button></div>
<div class="k-pane" data-tab="w150"><div class="k-cells"><div class="k-ln"><span class="t-acc t-b">/scheduled</span>  <span class="t-dim">3 scheduled tasks</span></div>
<div class="k-ln t-sel"><span class="t-acc">›</span> launch-watch   every 10 min</div></div></div>
<div class="k-pane" data-tab="w80"><div class="k-cells"><div class="k-ln"><span class="t-acc t-b">/scheduled</span> <span class="t-dim">· 3</span></div></div></div>
</div>
<figcaption>the list at 150 and 80 columns</figcaption>
</figure>
</section>
```

A ui block is a block like the others: its `data-id` stays across versions, the user's notes
and quotes anchor on it, a new version swaps it in place. A note on the lines of a `k-diff` or a
`k-term` comes to you with those lines: `on diff main, old 34–36 → new 34: …` and, under it,
each line's numbers and text. Inside it you may use `div span figure
figcaption button details summary`, `svg` (`path rect circle line polyline polygon text g`),
`class`, any `data-` name of yours (not `data-id`, `data-kit`), `aria-*`, `role`. Behaviour comes
from the kit, never from you: `data-ui="tabs"`, `"term"`, `"pick"`. Without JS everything still
reads (tabs show every pane).

## The components (use them first)

- `k-term` (`data-ui="term"` adds its dark/light switch): a terminal. `k-cells` holds lines
  `k-ln`, one character per cell: a column is padded to its widest cell, like the TUI (pad
  inside the span: `<span class="t-b">morning        </span>`), so every column starts at the
  same cell on every row. Colors: `t-dim t-faint t-acc t-b t-ok t-err t-sel t-raised t-chip t-hl
  t-u`. A `figcaption` says what it shows. Use the TUI's real words and marks (`✓` done, `–`
  stopped, `◷` scheduled), never invented ones and never emoji (two cells wide); a key bar puts
  three spaces between pairs (`⏎ open   x stop   esc back`), never ` · `.
- `data-ui="tabs"`: a `k-tabs` bar of `<button type="button" data-tab="x">` and `k-pane`s with
  the same `data-tab`: widths, steps, before / after.
- `k-diff`: a diff, drawn by the kit like a review on GitHub. One `div class="k-dl"` per line
  with its numbers: `data-old="34" data-new="34"` unchanged, only `data-old` a line that goes,
  only `data-new` a line that comes; the kit writes the numbers and the `−` `+` marks, so you
  write only the line's text (escaped: `&lt;`). `<div class="k-hunk">@@ -1,40 +1,36 @@ what</div>`
  heads a hunk; a long unchanged run goes in `<details class="k-fold"><summary>30 unchanged
  lines</summary>…rows…</details>`. Tabs for several files. The user picks lines by their
  numbers (click, shift-click) and leaves a note; you get the block, the tab, the numbers and the
  lines' text. Keep the real numbers: they are how the user's note finds the line.
  ```html
  <div class="k-diff">
  <div class="k-hunk">@@ -4,4 +4,3 @@ main's prompt</div>
  <div class="k-dl" data-old="4" data-new="4">## Skills</div>
  <div class="k-dl" data-old="5">Call `skill` before the task.</div>
  <div class="k-dl" data-new="5">Call `skill` with a name listed below.</div>
  </div>
  ```
- `k-ba`: two `figure`s, each a first `figcaption` (`<b>today</b> · …`) and its picture.
- `k-states`: `figure`s, each a `figcaption` with the state's name and a small `k-term`.
- `k-pick` with `data-ui="pick"`: `k-option`s with `data-pick="a"`, each an `h3`, a `p`, a
  `ul` of trade-offs; on your pick, first, `<span class="k-mark">my pick</span>`. A click is a
  pick note, like a compare's pick; one per block.
- `k-timeline` (an `ol`): `<li data-state="done|now">` with `<b>step</b><span class="k-who">who</span><p>…</p>`;
  a step that waits on the user takes `data-who="yours"` (it gets the accent).
- `k-grid` of `k-card`s, `k-kicker`, `k-tag`, `k-big` (a number).

The full example, every component: `kit/examples/feature-proposal.html` in the
bise repo (copy it, keep its shape, change the words and the cells).

## Your own style, when a component is not enough

One `<style>` per ui block, first in it. The server scopes every rule to the block and serves it
as a file. The lint refuses, one line each:
- colors other than the tokens: write `var(--ink)`, `var(--dim)`, `var(--faint)`,
  `var(--line)`, `var(--surface-card)`, `var(--term-…)`; never `#hex`, `rgb()`, `white`;
- fonts other than `var(--font-read|ui|mono|display)`, font sizes in px (use `var(--text-ui)`,
  `var(--text-small)`, `var(--text-kicker)`, or `em`);
- `:root`, `html`, `body`, `#ids`, the kit's own classes (`bise-…`, `bn-…`), `url()`, `@import`,
  `@font-face`, `position: fixed` or `sticky`, a `z-index` over 9, nested rules.
Spaces `var(--space-1…6)`, radii `var(--radius)`, `:scope` is the block itself.

## The look

- Light and dark both: the tokens follow the system; a `k-term` has its own switch.
- The accent (pink) is only for what waits on the user. A selected tab, a picked option, a
  chart use ink and dim. Inside a `k-term`, `t-acc` is the TUI's own accent, as on screen.
- The reading type is the sans; the serif is only the page's `h1`.
- What looks clickable is clickable (tabs, options); nothing else.
- No emoji, plain words, lowercase headings.
