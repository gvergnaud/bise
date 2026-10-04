# the bise book

Brand book, product spec and implementation plan for **bise**, the terminal
UI of Switchboard. One document, three parts:

- **Part I, brand:** who we are, how we sound, how we look.
- **Part II, product spec:** what the terminal UI does, screen by screen.
- **Part III, implementation plan:** how main agents split the work into
  non-overlapping tasks. The issues themselves live in
  [bise-issues.md](bise-issues.md), one section per issue, with status and
  notes.

Status: design decided with Gabriel on 2026-09-28. Nothing implemented yet.
Claims that are not true yet are marked **⚠**. When this book and an older
doc disagree, this book wins.

Visual references (open them in a browser):

| File | What |
|---|---|
| [site/book/screens.html](../../site/book/screens.html) | every screen of the product, 32 mockups + the levels and symbol legends |
| [site/book/live.html](../../site/book/live.html) | an 80-second live simulation; you can type and send messages |
| [site/book/onboarding.html](../../site/book/onboarding.html) | the first launch, 6 steps (validated as is) |
| [tui-mockup.html](tui-mockup.html) | the first single-screen mockup (superseded by tui-screens) |
| [site/index.html](../../site/index.html) | the one static site: the landing page, and the book under `site/book/` (old `tui-*.html` paths redirect) |
| [board.html](board.html) | the brand exploration (3 art directions + the chosen one) |
| [tui-spec.md](tui-spec.md) | the design log (how we got here) |

---

# Part I — Brand

## 1. Name and story

- **Name: `bise`.** Always lowercase, like a command you type. Four letters,
  checked free on npm, crates.io, PyPI and Homebrew (**⚠** `bise.sh` and
  `bise.ai` are taken; domain, GitHub org and trademarks not checked).
- **The story, told once:** *bise* is French for a little kiss, and the name
  of a cold north wind. Light, quick, friendly. Working with bise should feel
  like that: a light touch, not a weight.
- **The mark: `:*`**, the ASCII kiss. The logo is `bise :*`. It works in any
  terminal and any font, and looks typed by a human.
- **Standalone brand.** bise is its own brand, unrelated to Mistral: no
  Mistral colors, no Mistral mention in the copy. (The code runs on Mistral's
  Unified Harness.)

## 2. What we promise

**The problem, said plainly.** A day coding with agents feels empty. You
shipped more than ever and feel like you did nothing: progress bars, "v1 or
v2?", tab switching, reading what an agent did while you looked away. You feel
slow, useless, out of control, and got nothing back for the fun you gave away.
It's not you, it's the interruptions: every agent pulls you out of your head
every few minutes, and flow needs quiet to start. You never get there.

**bise preserves your flow.** Built for humans piloting hundreds of robots,
from a distance. (**⚠** "hundreds" is the image, not a tested number: no limit
in the product, but tokens, cost and provider speed are the real limit.)

- **Audience:** people with lots of ideas who move fast, and want the tech to
  disappear and keep up with them. Developers who already use a coding agent
  every day and have hit the wall at 2 or 3 agents.
- **The value is human:** flow, control, lightness. You talk to one agent;
  it runs as many as the work needs; you never wait, and you never have to
  manage them. Only the decisions that are really yours reach you.
- **Honesty rules:**
  - no number as a promise ("10x"): it sounds like a cap, and the product has
    none;
  - the real limits are tokens, cost and provider speed: say so;
  - never claim a feature that isn't built; mark it **⚠** in internal docs.

## 3. Taglines

| Where | Line | Status |
|---|---|---|
| site, headline | **kiss your backlog goodbye.** | picked (Gabriel) |
| site, subline | *ramble. interrupt. change your mind. i run the agents. you stay in flow.* | picked (replaces *built for engineers who think faster than they type.*, 2913f0a) |
| inside the product (onboarding) | **ideas in. little kisses out. also pull requests.** | picked (Gabriel; replaces "your ideas. my hands. lots of them.") |
| in reserve | you, but with way more hands. · your team is as big as your ideas. | former headlines |

**The name gloss**: site hero, README, launch posts, and the welcome
screen of the TUI (§15, the meanings without final periods there):

> **bise** /beez/ · french, n. 1. a quick kiss on the cheek. 2. a brisk north
> wind. 3. a terminal where multi-agent coding is painless.

Rejected, don't bring them back: "all the agents. none of the overhead."
("overhead" is unclear), "all the agents. stay in flow.", "code like a team
of ten." (a number), "all your ideas. none of the juggling." (sounds like an
LLM).

## 4. Voice

- **No capital letter at the start of a word**, on every marketing piece, all
  website text and all UI chrome. It reads like a human typing.
  - Exceptions: proper nouns keep their capitals (Mistral, Claude Code,
    GitHub, Ghostty); acronyms stay in caps (API, MCP, PR), though "cli" in
    lowercase is fine; ALL CAPS to shout is allowed, extremely rarely.
  - Our own name stays lowercase: `bise`.
- **Who speaks.** Inside the product, bise speaks as **"i"**; **"you"** is
  always the user. ("say it and keep talking. the work runs in the
  background, i'm always here.") The site talks to the reader as "you", so
  "kiss your backlog goodbye." is a site line, never an in-product line.
  Why it works: an English idiom everyone knows, turned: the backlog goes,
  and the kiss gives the name its meaning for English speakers.
- **Human, short, concrete.** Casual, warm, a bit cheeky. Short sentences.
  Show the moment instead of naming the feeling. Avoid polished LLM patterns
  ("X. None of the Y.", "seamless", "unleash", "supercharge").
- **Few words to learn.** The user meets four things: **you, main, agents,
  the inbox** (what waits in it keeps its own noun: a question, an
  approval; BISE-248). Never say "task", "hands", "hub", "sub-agent", "orchestrator" in
  the UI. (Internal docs and code may keep "task".)

## 5. Visual identity

**Two places, two looks: the site is paper, the product is the terminal.**
The user picked "paper" for every brand surface (bise.dev, the book, the
design system, the README images, og.png and the X header); the product
keeps its own look, below, unchanged.

**The site: "paper"** (direction B + the "figure" screens; reference
`site/content/landing-paper.html`, tokens in `site/book/paper.css` and
`docs/brand/readme/ink.py`).
- Paper `#f2ede2` with a light grain; ink `#1d1a17`, dim `#5a5349`, kick
  `#8a8174`; the pen `#c8264a` is the one accent; graphite `#6b645a`
  is for doodles and tallies (no blue, user). Dark paper: `#171513`, ink
  `#ece6da`, pen `#f4a6b0`, graphite `#b8b0a3`. Highlighter: `#f4a6b0` at 60% (dark 28%).
- Type: Newsreader for text and titles (600, tight tracking); Caveat for
  the pen only (the `:*`, group labels, short notes in the margin);
  JetBrains Mono only inside screens, for code and commands. Where no web
  font loads (GitHub SVGs, og.png), the display words are outlined to paths.
- The kiss in Caveat: the `*` comes down level with the colon (a mouth).
- Pen marks drawn in SVG that draw themselves once: underline, ring, arrow,
  tally, highlighter. One or two per screen.
- The doodle: the north wind as a pencil cloud (graphite: stroke
  `#6b645a`, hatch and gusts `#8a8174`, face `#1d1a17`; dark `#b8b0a3`,
  `#8a8276`, `#ece6da`; pink cheeks), blowing next to the name (the gusts start at its
  right edge, never across its face), asleep at the end of a page.
- Screens on the site are figures: a card a bit lighter than the paper
  (`#faf7f0`, dark `#1f1c19`), a pencil frame, a flat offset shadow kept very
  light (`3px 3px 0 #1d1a170c`), the TUI's own colors inside (light theme on
  paper, dark theme on dark paper), an italic caption under it ("fig. 1 · a
  bise session, playing live"). No tape, no blurry shadows.
- Lowercase everywhere, no final period on titles, no § before them.

**The product: "terminal brut, with the soul of the bise."** Raw terminal
credibility (monospace, calm, the product is the visual) plus one warm human
touch (the pale pink accent, the `:*`, a heart for "done").

**Background.** bise paints its own background (BISE-92, Gabriel's call;
it replaces "the terminal's background shows through"): every cell gets the
theme's ground (dark `#141211`, light `#fdfbf7`), so the text reads whatever
the terminal's colors, its transparency, or a wrong theme pick. On that
ground, only two tints: the selection and the card box. Where the terminal
supports it (OSC 11), its own default background is set to the same ground
so the padding around the grid matches; it is always given back (OSC 111,
then the color read at start) on exit, on a crash and when a shell takes
the terminal. A terminal that ignores OSC 11 just keeps its padding color.

**Two themes, light and dark,** chosen automatically from the terminal's
background (OSC 11 query), with a setting to force one. Every readable text
is ≥ 4.5:1 (WCAG AA) on white, our cream, black and a typical dark grey
`#282c34`.

| Role | Dark | Light | Use |
|---|---|---|---|
| text | `#ece6da` (15:1) | `#1b1917` (17.5:1) | everything you read |
| dim | `#a39c90` (6.9:1) | `#6b645a` (5.8:1) | secondary text, level 3, durations |
| faint | `#857d72` (4.6:1; 4.2 on raised) | `#7d766c` (4.3:1; 3.9 on raised) | the quietest text: hints, keys, timestamps, numbers, ` · ` between them (BISE-279, user: « le text super muted … est un peu trop muted ça le rend difficile à lire »; was `#4a4540` 1.97:1 / `#cfc8bd` 1.61:1, now the site's grays) |
| rule | `#4a4540` (1.97:1) | `#cfc8bd` (1.61:1) | the lines: the frame, the panel's rule, rails, borders, table rules, the empty composer's bar; **never** text (BISE-279: the old faint, so the lines stay quiet) |
| accent | `#f4a6b0` pale pink (9.7:1) | `#b8416b` raspberry (5.2:1) | the `:*`, "needs you", the agent you talk to, `✓✓` read, the bar on your messages |
| error | `#ff5a52` | `#b3261e` | failures only |
| ok | `#b9d99a` | `#3f7a2a` | diff additions only |
| ground (`bg`) | `#141211` | `#fdfbf7` | every cell (BISE-92); tints on it: selection `#33292c` / `#fdeef2`, card `#211d1b` / `#f1eee6` |
| raised | `#1f1c1a` (1.10:1 on the ground; text 13.6, dim 6.2, accent 8.8, error 5.5) | `#f4f0e8` (1.10:1; text 15.4, dim 5.1, accent 4.6, error 5.8) | the composer pane: the inside of the frame under the divider, edge to edge between the side edges; the frame's lines stay on the ground, outside it (BISE-102, BISE-212, user requests). Ground not ours (`BISE_TERM_BG=0`, or OSC 11 answers another color): the ground mixed 5% toward the text color. Not truecolor: 234 / 255. 16 colors or `NO_COLOR`: no tint, the bar alone |
| chip | `#231f1d` (text 13:1, dim 5.9:1) | `#efe9df` (dim ≈ 4.8:1) | the level-3 message chip only (BISE-106). 16 colors / `NO_COLOR`: no tint |
| pill | `#3a2530` (text 11.4:1, accent 7.3:1; 1.20:1 on raised) | `#f0d3dc` (text 12.6:1, accent 3.8:1, a glyph: ≥ 3:1; 1.23:1 on raised) | a quote or image chip in the composer and the strip: ` ❝ 1 ` (BISE-205). 16 colors / `NO_COLOR`: no tint, `[❝ 1]` |

- **Color means attention.** Only "needs you" and errors get a hue. Everything
  else is text, dim or faint (lines: rule). The accent is pink, not red, so "needs you"
  never looks like an error.
- **Syntax colors** (scripts, diffs): soft versions of the usual palette. Dark:
  keyword `#d7a6f0`, string `#b9d99a`, comment `#857e74`, number `#f0b27a`,
  call `#8fc4f0`. **⚠** Light syntax colors: to pick (issue BISE-01).
- **Marketing type:** the site is paper (Newsreader, Caveat, above);
  JetBrains Mono (free) for screenshots and everything inside a screen. In
  the terminal the font is the user's.
- **Motion:** in the product, only the working pulse `∿`, the blinking
  cursor, the typed welcome, the `:*` pop. Nothing bounces, nothing slides.
  On the site: the pen marks draw once when they come in, the cloud bobs
  and blows, a few seeds drift.

## 6. Symbol language

One glyph per entity and per status. Color only for attention.

**Entities**

| Glyph | Meaning |
|---|---|
| `›` | the composer prompt, and queued messages above it |
| `│` (accent) | your message in the history: a thin bar in column 1, on every wrapped line (heavy `┃` is for cards). ASCII: `|` |
| `:*` (accent) | main, the agent you talk to by default; it never moves. In the panel it follows main's name, and main's status sits in the glyph column like every agent's: the breathing gust while main works, `○` idle (` 0 ≈ main :*`, BISE-119) |
| `◇` | an agent's brief |
| `∴` (dim) | thinking |
| `$` | a bash call |
| `ƒ` | a TypeScript call |
| `↳` | a sub-call inside a TypeScript run |
| `±` | a file edit (patch) |
| `@` | a message between agents, or an agent writing to you (was `✉`: can turn into a color emoji) |
| `▣` (accent chip) | an image |
| `?` (accent) | a card: a decision that needs you |
| `≡` (dim; pulsing while running) | compaction running / its summary (was `⟳`: in almost no font) |
| `▲` (dim) | turn interrupted |

**Agent status**

| Glyph | Status |
|---|---|
| `·` (dim, pulsing) | starting |
| `∿` → a gust blowing by (`≈∿~·`), animated; one breathing cell in the panel | working: a breeze (user pick, site/book/working.html variant I; BISE-107) |
| `…` | waiting on another agent |
| `?` (accent) | needs you (question or blocked) |
| `✓` (accent) | done (Gabriel, 2026-09-29: the `✓` was not clear; a small pink check reads "finished"). It leads a panel row or a report; your read marks `✓` / `✓✓` sit at the end of your own lines, so the place tells them apart |
| `✗` (error) | failed |
| `○` (dim) | idle |
| `–` (dim) | stopped |

**Marks**

| Glyph | Meaning |
|---|---|
| `·` → `✓` → `✓✓` | your message: sending → the agent got it → the model read it (`✓✓` in accent) |
| `•` (accent) | unread activity in an agent |
| `ψ` (picked in BISE-84: width 1, in every installed audited font) | the agent works in its own worktree: a hub one (its branch) or a private one (`gate.sh new`, BISE-136); no mark: the shared checkout; was `⎇`, in almost no font |
| `⇄` | overlap: two agents changed the same file |
| `↻` (error) | a restart failed |
| `Δ` | a version is building or on trial (was `⧗`: in no font) |
| `▸` / `▾` | closed / open (progressive disclosure) |

**Decided by Gabriel (2026-09-28), after the glyph audit (BISE-03,
[glyph-audit.md](glyph-audit.md)):** keep the brand glyphs (`∿`, `:*`; `♡` only outside the product since BISE-100) and
every glyph a fallback font draws at width 1; replace only the ones that break:
`✉` → `@` and `↪` → `»` (color-emoji risk), `⟳` → `≡` pulsing, `⧗` → `Δ`,
`⎇` → `ψ` (BISE-84). `BISE_ASCII=1` switches every glyph to plain
ASCII (`~` working, `<3` done, `>` you, …) for terminals that draw them badly
(BISE-84). Terminals must use ambiguous width = narrow (the default).

**ASCII forms** (`BISE_ASCII=1`, BISE-84, made distinct in BISE-91): one
cell each, and no two entities share one.

| Glyph | ASCII | | Glyph | ASCII | | Glyph | ASCII |
|---|---|---|---|---|---|---|---|
| `›` you | `>` | | `▲` interrupted | `^` | | `✓` got it | `v` |
| `:*` main | `:*` | | `»` wrap | `}` | | `✓✓` read | `vv` |
| `◇` brief | `&` | | `·` starting, sending | `.` | | `•` unread | `!` |
| `∴` thinking | `:` | | `∿` working | `~` | | `ψ` worktree | `Y` |
| `ƒ` TypeScript | `f` | | `…` waiting | `;` | | `⇄` overlap | `/` |
| `↳` sub-call | `L` | | `✓` done | `*` | | `↻` restart failed | `(` |
| `±` edit | `%` | | `✗` failed | `x` | | `Δ` building | `A` |
| `▣` image | `#` | | `○` idle | `o` | | `▸` / `▾` | `+` / `-` |
| `≡` compaction | `=` | | `–` stopped | `_` | | `$` `@` `?` | themselves |

A cut text ends with `...` (not the one-cell `;`), and the panel title and
the help keys say `alt + number` instead of `⌥ + number`. Chrome glyphs
outside §6 (`⏎ ← → ↑ ↓ ⇧ ● ◉ ◆ ✚ ◀ ▪ ×`) keep their table forms through
`theme::asciify`. Box drawing is drawn like the frame: `-` and `|` for the
lines, `+` for corners and joins (the card box and bar, the rails; QA E).
Block elements (`▁ █`) stay.

**The legend** (BISE-137, user: ψ was unclear): /help and /shortcuts end
with a `symbols` section, one row per glyph: the glyph in its screen
color (its ASCII form under `BISE_ASCII=1`), then a few plain words, in
three groups with a faint title: **agents** (`:*` `∿` `·` `…` `?` `✓` `✗`
`○` `–` `•` `ψ` `opus·hi` `⇄` `↻` `Δ` `# 3` `+ 2 more`), **messages** (`›`
`│` `· ✓ ✓✓` `┃` `@` `✉︎ a → b` `▣` `❝` `●`) and **history** (`◇` `∴` `$`
`ƒ` `↳` `±` `·` `≡` `▲` `»` `▸ ▾` `▸ 3 more lines`). The filter finds a
row by its glyph, its ASCII form or its words (`worktree` → `ψ`). The
rows are one table next to the glyphs (`theme::LEGEND`); a test reads
every `G_*` glyph constant declared in the sources and fails when one has
no row. Zen is a behavior, not a glyph: it has a key row (`typing`), not
a symbol row (designer).

---

# Part II — Product spec (the terminal UI)

## 7. Principles

1. **You stay in control, the screen stays calm.** Agents work quietly. Only
   what needs you gets color. A finished agent arrives like a bise: a light
   touch, not an alarm.
2. **Everything is readable.** Prose never wraps wider than a novel line.
   Contrast ≥ 4.5:1 for all text you read.
3. **Progressive disclosure.** One line per step by default, details one key
   away. Exception: bash and TypeScript scripts are always shown in full.
4. **One symbol per thing** (§6).
5. **The history never lies.** Append-only, in arrival order (§10).
6. **No new vocabulary** (§4): you, main, agents, the inbox.
7. **Honest.** No silent failure, no hidden limit, nothing we pretend to
   have. (Don't talk about undo in user-facing copy: it doesn't exist for the
   user, so mentioning it is an artifact. §13 keeps it for implementers.)

## 8. Layout

```
 bise :*                                          ~/acme · # 2 in the inbox    ← header
                                                    │ agents
  │  the login breaks on safari                     │ 0 ○ main :*          4%
  :* on it: auth-fix takes it.                      │ 1 ∿ auth-fix •  12m  21%
    │ @ docs      → main      v1 or v2?             │ 2 ∿ release      3m   8%  ψ
    │ @ main      → docs      v2, the brief says so │ 3 ? docs             18%
  :* docs asked v1 or v2; the brief says v2,        │ 4 … api-v2            6%
     so i answered. ▸ why                           │ 5 ✓ bench
  ┃ ? docs needs you                                │
  ┃ the brief says "keep old clients working"…      │ inbox
                                                    │ 1 ? docs  keep old clien…
                                                    │ 2 ✓ bench  p95 down 12%…
 ┌ the inbox's strip (while something waits) ───────┐
 you → main · opus 5.5 · high · yolo ─────────────────────── 210k · 21%          ← divider
 › _                                                                              ← composer
   @ file   $ skills   / commands   ctrl+1 inbox                                  ← key bar
```

**Less on screen (BISE-303, designer's spec, the user's pick):** at rest
the screen shows what changes and what needs you. The glyphs already say
the state, so the words go: no `idle`, `done`, `you`, `waiting` in the
panel or on the divider, no model tag in the panel, no agent counts in the
header beside the panel. Hold ctrl alone (§16) and every word comes back
in its own place, nothing moves: the header's counts, the panel's state
words, `working · 1m` and the long context on the divider, the key bar's
ctrl keys.

- **Header:** `bise :*` on the left; on the right, beside the panel, the
  folder and the inbox count: `~/acme · # 2 in the inbox` (the agents'
  counts are the panel's job). No panel (narrow): the short counts,
  `∿ 3 · ? 1 · ✓ 1 · # 2`. Ctrl held: every non-zero count,
  `~/acme · ∿ 3 working · … 1 waiting · ? 1 needs you · ✓ 1 done · # 2 in the inbox`.
  **⚠** No cost in $ until the usage work lands.
- **Feed** on the left, prose ≤ 88 columns; extra width goes to the margin
  and the panel, never to longer lines.
- **Scrollbar:** only while you are scrolled up from the bottom: faint, one
  column, no arrows. Never at the tail (BISE-90).
- **Agents panel** on the right (hidden under 70 columns; the header keeps the
  counts). Title `agents` alone (BISE-303: holding ⌥ writes the numbers'
  key, `⌥1`, and `· ⌥↑↓ select` after the title). One
  row per agent: its number (faint; 0 main, 1–9 the first nine agents, blank
  after), status glyph, name, marks (main's `:*` and `✉ 2`, the unread
  `•`, `· 2 queued`), then three columns right-aligned, one column of
  margin (BISE-303): ` N G name …… TTT  PPP  ψ `. TTT the turn's time
  (dim, only while it works), PPP the context % (dim), each 3 columns
  wide, 2 between; `ψ` (dim) when it has a worktree (BISE-136: a hub
  worktree, `sb spawn --worktree` / `/isolate`, or a private one the
  agent told the hub about, `gate.sh new`; the shared checkout: blank).
  A blank column keeps its place, so the rows line up; a long name may
  take a blank time column's cells, never the % or ψ. No state word and
  no model tag at rest (the glyph says the state, the divider the
  model). Ctrl held: the state word (`working`, `done`, `idle`,
  `waiting`, `starting`, `failed`, `you` in accent) takes the time and %
  columns, 8 wide, right-aligned; ψ stays. Numbers never change
  while an agent lives (creation order). With more agents than rows, it
  scrolls and ends with `+ 21 more`. Archived agents: keep what landed in
  85160ab (a dim folded `▸ {n} archived` row at the bottom, click / `A` /
  `/archived` to open, read-only history, `/restore`); restyle only.
- **Cards section** (BISE-125, user request: main says `card #153`, you find it): under the live agents, above the archived row, while a card is open: a blank row, `inbox` (`inbox · ctrl+g` before BISE-302, `cards · ctrl+g` before BISE-248), then one row per open card in the strip's order with the strip's number (BISE-302, designer: ctrl+1 opens the row that says 1 everywhere; the hub's `#153` was shown before): ` 1 ✓ debt-solo  the debt list is cl…` = number (dim; ctrl held: accent), the kind's glyph in its color (`?` needs you / blocked, `✓` done, `✗` failed), the agent, 2 spaces, the first line of the card (dim), cut with `…` at the panel's edge (the agent is cut only when fewer than 7 columns are left for the text). The card in the box is on the selection color. Click a row: the box shows that card; click the title: the top card, again: back to the thread. It shares the panel's `+ n more`. The header counts them last, `# 3 in the inbox` (short `# 3`), kept right after needs you when room runs out, so under 90 columns (no panel) the count stays and ctrl+1-9 open them. (BISE-20 had removed the pre-bise list, `◆ cards (n) · Ctrl+G`.)
- **Card box** above the status row when a card is open (§12).
- **Status row:** the name of the agent you talk to **in accent** (`main`,
  `auth-fix`), then dim: state, context (`210k / 1M tokens · 21%`), `ψ
  branch` (a hub worktree) or `ψ fix-wt` (a private worktree's folder
  name) when it does not work in the shared checkout (nothing then:
  quiet is normal, BISE-136), and transient notes (`preview of auth-fix`).
- **Composer:** `› ` prompt; key hints on the right, dim, lowercase.
- **First run** (no agents yet), in the feed, dim, at the feed's indent, at 2/5 of the history's free rows (under 12 rows: on top, after one blank row; BISE-245):
  ```
  what's on your mind?

  say it and keep talking. the work runs in the background, i'm always here.

  try: "show me what you can do"
  ```
  The quoted sentence is a link (BISE-284): under the mouse, the hand and an accent underline; a click puts it in the composer, selected (typing replaces it, ⏎ sends it, an arrow or esc keeps it), and sends nothing. While the composer holds it, the last line says `⏎ try it · or just type your own` (`⏎` in the accent, bold; the rest dim).

**The reading column** (user request, marketing 82f1742). The history is a 91-column column (3 for the lead + 88 of text; widened from 79 by Gabriel, 2026-09-29: +15%), centered in the space left of the panel: F = terminal width − panel (30) − 1; x0 = floor((F − 91) / 2) when F ≥ 95, else column 1. Tables and code start at x0 and may run to 103 columns (capped at F − 1), extending right, never re-centered. The status row, the queue, the images strip, the composer block and its hints use the same x0 and width (hints right-aligned to x0 + 91). The agents panel stays flush right. Under 70 columns the panel hides and F = width.

**Spacing, in cells** (user request, marketing 0e6e803). The rule is the landing demo, translated to whole cells.
- **Outer margins:** 2 columns left and right, 1 row top and bottom; under 30 rows the top and bottom rows go.
- **Header:** its own row: `bise :*` bold at the left margin, the summary (`∿ 3 working · ✓ 4 done`) flush right; then 1 blank row.
- **History:** the reading column above (91 wide, centered in the feed area = everything left of the panel).
- **Between feed and panel:** 3 blank columns. No vertical rule: whitespace and alignment do the job.
- **Scrollbar:** no arrows, no track: only a faint `┃` thumb in the last column of the feed area, and only while you are away from the bottom (the status row says `↓ back to the bottom`).
- **Agents panel:** 28 columns (wider on wide screens, below), flush right at the right margin, first row level with the history's first row; title `agents`, then 1 blank row; rows `N glyph name` with the time, % and ψ columns right-aligned (BISE-303); a name takes all the room its row leaves and is cut with `…` only there (no fixed cap; BISE-109, user request).
- **Between history blocks:** 1 blank row (§10), and 1 above and below level 2 (§9).
- **Card box:** the reading column's x and width, 1 blank row above; heavy bar `┃` in the column's first cell, text from its 4th; title, body, 1 blank row, then the choices and keys row.
- **Bottom stack:** 1 blank row under the history, then the status row, queue, strip, composer block (§13), hints row, 1 bottom margin row, all on the reading column's x and width (the hints end at its right edge).
- **Wide terminals** (BISE-260, user: « sur les grands écrans quand il y a de la place on pourrait rendre la sidebar un poil plus large »): from 165 columns the panel takes 1 more column for every 5 past 160, the feed area the other 4, up to 44 columns at 240 (where the model tags show): 180 → 32, 200 → 36, ≥ 240 → 44. The feed area never shrinks as the screen grows (122 at 160, 138 at 180, 186 at 240, framed).
- **Narrow terminals:** ≥ 100 columns as above; 90–99: panel 24 wide, gap 2; < 90: no panel (the header keeps the short counts, `∿ 3 · # 1`), margins 2, column = min(91, width − 4) (at 80: 76 wide, 73 of text); at 60: margins 1, column 58 (55 of text). The column is centered only when the feed area has at least 95 columns, else it starts at the left margin.
- **What the demo does that a terminal can't:** its own font (the terminal's is the user's), line height (1.2 vs 1.6), sub-cell gaps (the site's 10–18 px become 0 or 1 whole row: we take 1), 1 px rules (a terminal rule is a full cell), fade and slide motion. bise does paint its theme background (§5, BISE-92), but no rounded panels. The demo is the reference for rhythm and proportions, not for exact pixels.

**The frame** (user request on 805e538, marketing 9f000c8; replaces, in "Spacing, in cells" above, the outer margins, the header row, "no vertical rule" and the bottom stack's status and hints rows; the reading column, the panel widths and the narrow tiers stay). bise draws itself like an app: a thin faint frame on the edge of the terminal, with "bise :*" and the summary in its top border. Inside, 2 blank columns on each side. The agents panel sits behind a faint rule that joins the frame. A full-width divider separates the history from the composer pane. The lines are faint so the text stays in front: the frame is a shape, not a decoration. Under 60 columns or 16 rows the frame goes, the divider stays.

Exact layout (terminal F columns × H rows, 0-based; all lines faint; ASCII: `+ - |`):

Frame (when F ≥ 60 and H ≥ 16):
- rounded frame on the terminal edge: row 0, row H−1, column 0, column F−1 (╭ ╮ ╰ ╯ ─ │).
- row 0 = the header. `╭─ bise :* ───…─── ~/acme · ∿ 3 working · ? 1 needs you ─╮`: title from column 3 ("bise" bold text, ":*" accent), 1 space around it and the summary; the summary dim (its glyphs keep their colors), ending at column F−4. Not enough room: drop the path first, then use the short counts (∿ 3 · ? 1 · ✓ 2).
- viewing a task, its **role line** follows the title, dim: `╭─ bise :* · fixing the safari login redirect ──── ∿ 3 working ─╮` (BISE-126, user request): what the task is doing now, ≤ 60 characters, lowercase, written by a small model after each of its turns that changed something (its first value: the objective's first sentence). Room: the counts come first, then the line (the path goes before the line is cut under 27 columns), cut with `…`, gone under 12. Main's view: no line. The unframed header row shows it the same way.
- row 1 blank. The history starts on row 2 and ends 1 blank row above the divider.
- inside the frame: 2 blank columns each side. Text starts at column 3.
- panel, F ≥ 100: a rule │ at column F−33, joined with ┬ on row 0 and ┴ on the divider; panel text from F−31 to F−4 (28 columns); the history ends at column F−36. From F = 165 the panel is P = min(44, 28 + (F − 160) / 5) columns: text F−P−3 .. F−4, rule at F−P−5, history ending at F−P−8. F 90–99: panel 24 wide (rule at F−29). F < 90: no panel, no rule.
- scrollbar: a dim ┃ thumb drawn on the panel rule (on the frame's right border if there is no panel), only while you are scrolled away from the bottom.
Composer pane (bottom up):
- H−1 frame bottom. H−2 the key bar. The composer: 1 bar row (none under the attachments box), the text, no bar row under it (BISE-210). [the attachments box] [1 blank row] [queued lines, BISE-89]. The divider.
- divider: a full-width rule `├─ you → main · opus 5.5 · high · yolo ─────…───── 18k · 2% ─┤` joining the frame. Label from column 3: "you →" dim, the agent name accent, then the model, effort, approvals mode (and `ψ place`). The right side is dim and ends at F−4: at rest the context, short (`18k · 2%`, BISE-303: no `idle`, no `working`); ctrl held, the long form after the state when it doesn't work (`idle · 18k / 1M tokens · 2%`); notes (preview, read-only, the hub's version, the hub disconnected) follow. This replaces the status row. When the agent you're viewing works, the gust (today's working animation) follows the mode behind a dim ` · `: `you → marketing · opus 5.5 · high · yolo · ≈∿~·` (BISE-105, BISE-303); ctrl held, ` working · 42s` (dim, the current turn) after the gust; idle: nothing after the mode.
- composer: bar │ at column 3 on every row of the composer (the blank bar row above the text too; faint while empty, accent with text, an image or recording), text from column 7 (x0 + 4, one column right of the history's text column: the composer is its own raised pane; BISE-108, BISE-228, user requests), wrapped at word boundaries 2 columns before the pane's right edge. Under 60 columns: text at x0 + 3, no right margin. At least 1 row between 2 blank bar rows from 20 rows (BISE-111; BISE-210 dropped the one under it, BISE-219 put it back), growing to min(12, 40% of H), then scrolling. Empty: cursor then a dim placeholder "what's on your mind?" (to an agent: "talk to auth-fix directly").
- composer markdown (BISE-276, user: « Les listes à puces automatiques comme dans un rich text editor ; les ```ts et autres code blocks avec le syntax highlighting. Je ne veux pas de caractères cachés, je veux voir tout le markdown, mais je veux que ça formate / colorise tout seul. »): the composer colors markdown as you type and hides nothing: every mark stays, so the message sent is the text typed and the cursor and the wrap never shift. Marks dim, never accent (designer: the bar is already pink): `- * + 1. 1)` markers, `[ ]` `[x]`, `#`, `>`, `**` `*` `_` `~~`, backticks, `[`, `](url)`, a fence and its tag. Content: a heading bold, **bold** bold, *italic* italic, ~~strike~~ crossed out, a checked item's text dim, inline `code` as the history's inline code (ok, bold), a link's label underlined. A fenced block whose tag names a language (ts/js, rust, python, go, the C family, ruby, lua, sql, bash/sh, json, toml, yaml, css, html/xml, diff) is colored with the six `syntax_*` roles (the history's fenced blocks too); no tag or an unknown one: plain text; no tint. `NO_COLOR`: the modifiers only. Keys: a newline (shift+⏎, alt+⏎, ctrl+j) on a list item continues the list (the next number, an unchecked box for a task; the items below renumber), on an empty item it steps out a level, then ends the list; tab / shift+tab on a list item (or the items of a selection) indent under the item above / outdent to its parent, numbers restart at 1 in a new sublist; in a code block (its lines or its opening fence) a newline keeps the line's indentation, never a list, and plain ⏎ is a newline too: the closing fence makes ⏎ send again. Each of these is one undo step with its newline. Plain ⏎ on a list item still sends (`mdlive::ENTER_CONTINUES_LISTS`: the Slack way, one switch).
- key bar, from the composer's text column (column 7; x0 under 60 columns, BISE-228): keys in text color, what they do dim, 3 spaces between pairs; default `@ file   $ skills   / commands   ctrl+1 inbox` (BISE-303, designer: the three characters that start something; `ctrl+1 inbox` only while the inbox holds something, `/inbox` without ctrl+digits; `⏎ send`, `? help` (? on an empty composer still opens it), `⌥0-9 switch` (the panel's numbers say it) and `ctrl+s find agent` (the held ctrl bar lists it) are gone; pairs drop from the right when the row is short); with images, `ctrl+v paste image` first; while the agent works, `tab queue   ⏎ steer   ctrl+1 inbox   ctrl+c interrupt`; per-mode sets as today. On the right, ending at F−4: a dim tip (e.g. "tip · ctrl+o opens everything folded") that changes every 5 minutes, going through the tips in order, so you meet more of them over time (user request, 2026-09-29); it never changes while you type, hidden while you type or when there are fewer than 3 columns between it and the keys.
- height: divider + blank + 1 + key bar + frame = 5 rows at rest (BISE-210, user request: less padding at the bottom; was 6 with BISE-111's blank row under the text), the same as today's block + bottom margin. The frame gives back 1 row at the top (border+blank instead of margin+header+blank). It costs 2 columns (3 each side instead of 2).
Small terminals: H < 24: drop the blank row under the text. H < 20: also the one above. H < 16 or F < 60: no frame. Then a header row on row 0, the divider is a plain ─ rule, margins of 1, the key bar stays.
The frame and the rules paint no background of their own: the theme ground (§5, BISE-92) stays everywhere; light theme = same tokens.

## 9. Three levels: what's for you, what isn't

The same three levels everywhere, in main and inside an agent.

| Level | What | Look |
|---|---|---|
| **1 · needs you** | a question or a blocker addressed to you | accent bar `┃` on the left, bold accent title `? docs needs you`, normal body; stays until answered; also in the card box |
| **2 · for you** | what main or an agent says to you: replies, summaries, reports on your requests, main answering on your behalf | normal text, with `:*`, a status glyph (`✓` `✗`) or `@ name to you:` in front |
| **3 · between agents** | messages agents send each other and to main | dim text under a faint rail: `@ from → to  text`, names padded to 10 columns, one line each, `▸` when long |

- **Your messages** carry a thin accent bar `│` on the left, on every wrapped line, text at column 3; marks `·` `✓` `✓✓` at the end. Thin bar = you, heavy bar `┃` = needs you, so you can tell at a glance what you said from what the agents said (decided by Gabriel, 2026-09-28). A long one folds (BISE-239, user: « Le user message devrait avoir un max line count dans l'historique avec ctrl+o pour l'afficher en entier. », designer's picks; 12 since BISE-261, user: « je la trouve un peu trop limitée ... x 1.5 »; 20 since BISE-262, user: « on peut faire 20 lignes? »; was 8): more than 20 lines (a quote, an image chip: one line each; or as many rows at 80 columns) shows its first 20 rows, then its own dim row `▸ n more lines` at the text column, the bar through it, the mark after it (`▸ 12 more lines ✓✓`); n = every line not shown whole (a line cut to fit counts as hidden). A click on that row, or `space` on the message, opens it whole in place, `▾` after its last line (a click on that row folds it back); `ctrl+o` opens and folds every one with the rest. Every view: main's and each agent's. Its other rows stay for reading and selecting (a click there does nothing). Search (ctrl+f) finds text in the hidden part and opens it. ASCII: `> 12 more lines`.

- Traffic between agents is **always in the history** (including between two
  agents that aren't main), so what happened stays understandable. Main's
  level-2 line after a burst is the summary for you.
- Levels 2 and 3 differ by brightness and the rail, never by hue.
- Main deciding for you is level 2 and says why on demand:
  `:* docs asked v1 or v2; the brief says v2, so i answered. ▸ why`.
- No "quiet" mode: the levels and the folding (§10) already keep it calm.

**Emphasis** (user request, marketing 82f1742). A terminal has one font size, so what's for you reads bigger through contrast and room: level 2 in text color with its speaker in bold (`:*` accent bold, `@ name to you:` text bold) and a blank row above and below, even between two level-2 blocks; the agent's own work (thinking `∴`, the one-line tool calls, level 3) is dim; cards (level 1) unchanged. OSC 66 text sizing is never used in the history (§15 may use it for the welcome line only, where detected).

**Level 3 is an envelope chip** (user pick, site/book/messages.html variant C, marketing 346dacb; BISE-106). Between agents, a message looks like a message: a small tinted chip with an envelope says who writes to whom (✉ auth-fix → release, the sender in bold), and the text goes under it, dim. You can follow the conversation at a glance, and it never shouts.
- One message = one line group at x0, flush with the text (the chip's first tinted cell at x0, `✉︎` at x0+1, the sender at x0+3; BISE-109, marketing e567233): the chip alone on its row, the text under it (BISE-127). Chip = tinted cells ` ✉︎ sender → receiver ` (1 tinted column each side; palette role `chip`, §5). `✉︎` = U+2709 U+FE0E (text presentation, 1 column; if a terminal still draws it 2 wide, fall back to `@`). Envelope dim, sender bold text, `→` faint, receiver dim. Names cut at 24 with `…` (BISE-109, user request: room for real names; was 12). `main` is a plain name here (no `:*`, no accent): it's level 3.
- 16 colors / `NO_COLOR`: no tint, the chip reads `[✉ sender → receiver]`. ASCII: `[@ sender > receiver]`.
- Text: dim, on the rows under the chip at x0+2, at every width (BISE-127, user: « on devrait mettre le contenu textuel en dessous plutôt qu'à la droite, ça pose un peu des problèmes d'alignement »: beside chips of different widths, the texts started on different columns); wrapped to the reading width; at most 2 rows, then `… ▸` opens it whole (`▾` at its end).
- Spacing: messages of the same pair stack with no blank row; a new pair after a blank row. Messages from the same sender to the same receiver in a row share one chip: each one's text starts on its own row under it, at x0+2 (the way back, `release → auth-fix`, gets its own chip, still with no blank row). The fold line `▸ n messages between k agents` stays dim at x0, no chip. Levels 1 and 2 unchanged.
- **Short on room (the chip)** (W = reading width at x0): a name is cut only when the row really lacks room (BISE-109). W ≥ 60: names up to 24, then `…`. 40 ≤ W < 60: names up to 16. W < 40: the chip without inner padding and spaces, `✉auth-fix→release`, still tinted, at x0, names up to 10. At every width the text stays under the chip at x0+2, ≤ 2 rows then `… ▸`. Never cut the arrow or the envelope; cut the receiver before the sender. 16 colors / `NO_COLOR` and ASCII forms follow the same cut rules. The fold line is cut from the right with `…` when narrow. Level 3 stays the quietest thing on screen: the chip tint is its only background, no accent anywhere in it (main included).

**A box that only sends** (user request, BISE-110). A bash box whose whole script is one `sb send`, `sb ask` or `sb report` (any flags, after at most one `cd <dir> &&`) says nothing the chip under it doesn't: it hides, if and only if the command succeeded and the message it sent (matched by the id in its output: `sent m_12`, `reported (m_12)`, an ask's `reply from docs (m_13, answers m_12)`, the question and the reply both) is drawn below it in the same feed. Anything else in the script (`;`, a pipe, a redirection, `$( )`, a second command), a failure, or no such message drawn (a task's own sends: only main's feed draws them): the box stays. A hidden box takes no row and does not split a run of level 3 (main's sends to one agent stack, and fold after 3 like any run, §10); ctrl+o shows it again with everything folded. The hub's `msg` line carries the id for this (`msg : main → docs m_12 : text`).

**Providers (`/provider`, BISE-294, designer's layout).** Full screen, the first run's column: `providers` (bold), `the keys i can use. enter sets one up or changes it.` (dim), the filter line `› type to filter`, one row per provider: its name, then `✓ ready · saved in bise` / `✓ ready · from OPENAI_API_KEY` (`✓` accent, `ready` text, the source dim), `not set up` (dim), `✓ no key needed` for a local one, ` · main uses it` on main's; the hidden providers with a key follow, the others behind `more providers…` (their names, dim). Enter on one not set up: the first run's key step with all its states; on one set up: keys and accounts only (BISE-301): its name, `✓ ready · saved in bise` / `✓ ready · from X`, one dim line naming the roles on it (`main, small jobs and voice use it. /models changes that.`, none: `no role uses it yet. /models picks one.`), then a numbered menu (`paste a new key`, `open the keys page`, `open billing`, `remove the key`); a local provider has no menu rows (`{esc} back`). The roles on each row, dim (`main · small jobs · voice`), line up after the widest state when every row fits, else 3 spaces after its own state, else under it. Keys `{↑↓} choose · {enter} open · {esc} back`. A turn on a provider with no key: `✗ turn stopped: no OpenRouter key yet. /provider sets it up.` The terminal's `bise providers` prints the same rows under `your providers`.

**Model roles (`/models`, BISE-298, designer's layout; user: the model choice must scale).** Each job bise gives a model is a role: `main` (talks with you and starts the agents), `agents` (the work main hands out), `small jobs (titles, summaries)` (the words `titles, summaries` go with the name wherever it shows, tags included), `voice` (listens to you), and `auto-confirm` (declared, no screen yet). config.toml `[roles]`: `main = "provider/model"` or `main = { model = "…", effort = "high" }`; the old keys (`model`, `agent_model`, `small_model`, `[voice] model`) are still read, bise writes `[roles]` only; env > `[roles]` > old key > fallback. `/models` (also `/roles`, and `/model`'s last row `every role…`) is the one home of the roles; roles first, everywhere (BISE-301, the user: "one provider must obviously serve several roles"; designer's option A, site/content/roles-menu.html): `which model does what?`, dim `each role picks a provider, then a model. one provider can serve several.`, one row per role: its provider then its model, the providers in one column (`main   Mistral  mistral-large-latest · high`), a fallback dim `same as main · Mistral · <id> · high` / `auto · Mistral · <id>` (narrow: the id goes, then the effort, never the provider); a role on a provider with no key: `Anthropic  claude-opus-5-5  ✗ no key · enter fixes it`; rows never wrap, a dim hint line for the row under the cursor. Enter on a role, the same steps for every role: 1. `agents: which provider?`, dim `now: OpenAI · gpt-6-luna · medium`: `same as main` (agents) / `auto` (small jobs) first with what it resolves to, the ready providers (`✓ ready`, dim `· now` on the role's, `· main, small jobs use it` for the other roles on it), the others `not set up`, `more providers…`; voice lists only the providers that listen (`only the providers that can listen. ctrl+r starts, any key stops.`, `· voice only`). One not set up: the first run's key step (dim `for the agents. then you pick the model.`, keys `{enter} check it · {esc} back to the providers`), then step 2. 2. `agents · Anthropic: which model?`, dim `type to filter, or a model id that isn't listed.` (voice: `you talk, it types in the composer.`): the filter, that provider's ids without it, `✓ now` and `recommended` (the provider's pick; small jobs: its small model; voice: its voice pick) in accent, a typed id `+ use anthropic/<id>   not in my list: i'll try it with one tiny call` (checked); one model: skipped. 3. main and agents: `how hard should it think?`, dim `claude-sonnet-5-5 for the agents`. Then `/models`, the row flashing ✓. esc goes one step back. `/model` stays the fast list for the agent in view, grouped by provider (a header row per provider that picks nothing, the ids without the provider, their windows in one column), then `+ another provider…` and `every role…`. 80 columns when the terminal has 90. `/provider` is keys and accounts (above); removing a key names the roles that stop (`main, small jobs and voice use Mistral. without the key they stop.`). `bise doctor` prints one line per role (`✓ main  anthropic/claude-opus-5-5 · high`, `✓ agents  same as main`, `✓ small jobs  auto · <id> · titles, summaries`, `✓ voice  <id> · language auto · listens when you talk`), the origin only when it is not config.toml; `bise config get|set main|agents|small|voice` (old names as aliases; unset: `small (small jobs) is not set: auto · <id>`).

**Voice setup (BISE-298, designer's layout).** Voice is a second pick next to the chat model: same providers, same keys, same column. Offered: Mistral (voxtral, recommended, the default), OpenAI (its transcribe models), ElevenLabs (scribe, voice only); Groq and Deepgram stay in the catalog, hidden. Turning voice on with no setup that works (`/voice`, or ctrl+r while voice is off) opens voice's steps (BISE-301, as every role's above): `voice: which provider?` (the ready ones first; none ready: Mistral preselected), then `voice · Mistral: which model?`; a provider without a key goes through its key step first, checked with its voice pick, and the model just checked is taken without a second call. Esc: voice stays off, one dim line `voice is off. /voice when you want it.` A provider without a key: `/provider`'s key step with all its states; the check transcribes half a second of silence with the voice model (`checking your key with one tiny call…`), the key saved is a normal provider key. Then `✓ voice is on: mistral/voxtral-mini-latest.` + dim `press ctrl+r and talk, any key stops. /voice turns it off.`; `[roles] voice` in config.toml. Later: `/voice` is one row in the `/` list (`dictation and voice mode: the model, the voice, the language`, no argument popup) and one ⏎ opens its one screen, the model on its speech-to-text row (voice-menu, designer); `/voice setup` typed still opens it on that row, but no line names it any more. `/models`' voice row stays (`voice: writes down what you say. enter opens /voice.`): ⏎ opens `/voice` on speech to text, esc there comes back to `/models` on that row. A failed transcription says why in one line, the provider's words dim under it, and keeps the clip (`your recording is kept: ctrl+r retry`: the next ctrl+r sends it again): `✗ voice needs a key. /voice setup picks one.`, `✗ Mistral says the voice key is wrong. /provider fixes it.`, `? your Mistral account has no credit yet. add some here: <billing url>`, `✗ i couldn't reach Mistral to transcribe. try again, or /voice setup for another provider.`, `✗ Mistral can't transcribe with <model>. /voice setup picks another model.` The mic refused: `✗ i can't hear you. allow the microphone for your terminal: System Settings › Privacy & Security › Microphone.`

**Working = a gust blowing by** (user pick, site/book/working.html variant I; BISE-107).
- Header and divider: 5 cells. A gust crosses left to right, 110 ms a frame, 9-frame cycle: head `≈` (text), tail `∿` (text) `~` (dim) `·` (faint), then 5 empty frames. Cell k at frame i = ramp[(i − k) mod 9], ramp = `≈ ∿ ~ · _ _ _ _ _` (`_` = space).
- Panel status (1 cell): the gust breathes in place: `· ~ ∿ ≈ ∿ ~`, 110 ms a frame, same colors. main's row lines up with the others (BISE-119): its status in the same glyph column (the breathing cell while it works, `○` idle), its name in the name column, its `:*` (accent, still) right after the name: ` 0 ≈ main :*   42s`, idle ` 0 ○ main :*`.
- Divider (BISE-105, BISE-303): `you → marketing · opus 5.5 · high · yolo · <5-cell gust>` (a dim ` · ` before the gust); ctrl held, ` working · 42s` (dim) after it. Idle: nothing after the mode.
- **Model and effort (BISE-135, user: « c'est très important de montrer le nom du modèle qui est utilisé et le reasoning effort … à côté du nom de l'agent … c'est pour la transparence »):** the divider says the model the agent in view runs and its reasoning effort right after its name, then `ψ place` when it works outside the shared checkout (BISE-136): `you → auth-fix · opus 5.5 · high · ψ fix-login` (model, effort and place dim, ` · ` faint; the shared checkout: nothing after the effort; a model with no effort: the model alone). Short on room, before the state loses anything: (1) the place's name goes, ψ stays; (2) the long form becomes the tag `opus·hi`; (3) the tag goes; then the gust's order above. Each live row of the panel carries its tag in one aligned column before its state (`opus·hi`, `sonnet·lo`, `gpt-4.1`; family, plus its version when two agents run two models of one family; efforts lo, med, hi, max, off; ASCII `opus.hi`), faint, dim when it differs from main's; a panel under 44 columns shows no tag. `/model [<model>] [default]` and `/reasoning [<effort>]` switch the agent in view from its next call (the popup: the catalog's models and aliases, or the efforts its model takes; a note row says which agent; ✓ on the current one; BISE-289: a typed id the list does not have is its last row, `+ use <provider>/<id>` (no provider typed: the current model's), after a note row `no listed model matches.` when nothing else does: any id of a known provider switches, the provider answers at the next call); `default` also writes config.toml (`[roles] main` for main, `[roles] agents` for an agent, BISE-298). BISE-294: the list holds only the models of the providers set up (a key found, or none needed), then `+ another provider…` (the providers not set up, dim), which opens `/provider`; a model of a provider without a key opens `/provider` on its key step and switches once the key works. The hub stores the choice in the agent's state dir (`choice.toml`, BISE_SESSION_CHOICE): it survives reloads and restarts; a model with another context window reloads the REPL at its next idle, same session, so the compaction threshold follows. Header: `<gust> 3 working · ? 1 needs you · ✓ 2 done`.
- ASCII: ramp `. - ~ =` (head `=`), same motion.
- Cost: redraw only those cells; ≤ 10 fps; stop when no agent works or the terminal loses focus.
- **Short on room (the gust, BISE-303):** divider label, full: `you → marketing · opus 5.5 · high · yolo · ≈∿~·` (ctrl held: `  working · 42s` after it) on the left, the context on the right. Not enough room: drop in this order, one step at a time, until it fits with ≥ 3 columns between left and right: (1) ctrl held, the long context becomes the short one, then `working · 42s` goes; (2) the tail shrinks (the place's name, then the long model becomes the tag, then the tag, then ψ); (3) the context goes; (4) the gust 5 cells → 3 cells (same ramp, cycle 7: `≈ ∿ ~ · _ _ _`); (5) the gust → the 1-cell breathing form (`· ~ ∿ ≈ ∿ ~`); (6) cut the agent name at 20, then at 12, with `…` (BISE-109; was 12 then 8); (7) last, the approvals mode. The gust never disappears while the agent works: it's the last thing kept after `you → name`. Header: F ≥ 90: `<5-cell gust> 3 working · ? 1 needs you · ✓ 2 done`; 70–89: 3-cell gust + short counts `3 · ? 1 · ✓ 2`; < 70: 1-cell breathing + short counts. Panel: always the 1-cell breathing form. No motion (the terminal loses focus, the redraw budget is hit, or a reduce-motion env is set): a static `∿` in text color everywhere (`BISE_ASCII=1` keeps the motion with `. - ~ =`).

**Zen while you type** (user request, BISE-121: « If I typed less than 8s ago and I didn't move my cursor, I want UI elements to fade out a bit, and animations to get more subtle »).
- **In:** a key that changes the composer (a character with any modifier that types, `⌥` accents and dead keys: `⌥`` then `e` = `è`, an accent the terminal or an input method composed; backspace, delete, a new line: shift+⏎, alt+⏎, ctrl+j; a paste), no popup open.
- **Holds** (BISE-124, user: « quand je tape un accent genre ` ou les arrow keys, etc le zen mode s'enlève »): every key that edits or moves inside the composer starts the 5 s again: the above, the arrows, home/end, the word moves (`⌥`/ctrl + arrows, `⌥b`/`⌥f`), ctrl+a/e/…, undo, select all. A move alone never starts zen. A key the app has no use for (a lone modifier, caps lock) changes nothing.
- **Out** (BISE-128, user: « Quand je fais enter ça devrait enlever le zen mode direct. pareil si je lance un shortcut pour switcher ou naviguer dans la UI. Aussi le zen mode ne devrait rester que 5s en fait. »): 5 s after the last composer key, or at once on: ⏎ (send, enter the selected agent, run a command); a mouse move, click or scroll (the mouse cursor moved); the terminal losing the focus; esc (every job: back to main, close the selection, the card, the draft away), tab, page up/down, end back to the bottom; every key an app shortcut takes before the composer: switching agents `⌥0-9`, the panel keys (alt+↑↓ on an empty composer, ⏎, space, `D`, `A`), the card keys (ctrl+1-9, ctrl+a on an empty composer, ctrl+n/ctrl+p, alt+r, ctrl+f, ctrl+x, pgup/pgdn), ctrl+o, ctrl+l, ctrl+c, ctrl+r, ctrl+v, ctrl+`/ctrl+space, copy, any key while the help or the terminal pane is up; a popup (`/`, `@`, `$`); anything that needs you (a new card, a message to you, a confirm, an error).
- **Look** (BISE-132, user: « En mode focus, je pense qu'on devrait quand même garder l'historique principal visible. Seulement la sidebar et les animations et tout devraient être un peu dimées, parce que parfois j'ai quand même besoin de lire pour pouvoir écrire. Mais j'ai pas besoin de voir tout ce qui se passe ailleurs, j'ai pas besoin de voir toutes les animations. »): the chrome's text goes 45 % of the way toward its own background: the header (frame title, role, counts), the frame's lines, the agents panel (agents, cards list, its rule and scrollbar), the divider's rule and its right side, the queued messages and attachments, the key bar. Backgrounds, glyphs and layout never move. Kept as they are: the history you read (the feed area, from its first row down to the divider, the pinned line included; new lines keep coming in their normal colors), the composer's text and cursor, the divider's label (`you → name`, its gust included), the card box, and every cell in the accent or the error color (level 1, what needs you, errors). The fade is 250 ms in 4 steps (≤ 4 repaints in, ≤ 4 out), from where it is if you type again while it fades out.
- **Motion:** what happens elsewhere stands still: the panel's gusts and the header's are one `∿` that keeps a slow color pulse, dim then faint, one step every ~1.3 s (user: « en zen mode, je pense qu'on veut toujours une animation de couleur du symbole wave pour les agents, c'est assez subtil, mais on comprend que ça travaille toujours »; was one still `∿`, BISE-132). The glyph never changes shape; only that cell repaints, once a step. Still with reduce motion, when the terminal loses the focus or a draw is over budget, and under `NO_COLOR` (the pulse is a color); `BISE_ASCII`: the same pulse on `~`. The gust of the agent in view (the divider's label) at half speed (220 ms a frame), each tone one step down (text → dim → faint); the tick pulses (`∿` of a running tool, `·` starting) hold still.
- **Fallbacks:** reduce motion (`BISE_REDUCE_MOTION`): no ramp, one step in and out (the gust is still anyway). `NO_COLOR`: no mixed colors: the terminal's dim attribute on the faded cells, one step. `BISE_ASCII`: nothing changes (colors only). No extra redraws: the loop's 80 ms frames carry the fade; a steady zen rewrites no cell but the working agents' pulsing `∿` (one cell each, every ~1.3 s).

**When a conversation compacts** (BISE-300, user: make the setting clear and safe). config.toml `compaction_threshold`: a number of estimated tokens (`compaction_threshold = 450000`) or a share of the model's context window (`compaction_threshold = "45%"`). Whatever is written, a conversation compacts at 80 % of its model's window at the latest, so a number set for a 1M model never overflows an agent on a 200k one. Unset: 80 % of the window. `BEND_THRESHOLD` wins over the file and reads the same way. `bise config set compaction_threshold 45%` writes it; `bise doctor`'s `compaction` line shows the figure each model gets (`main 450000 tokens · agents 160000 tokens (capped: 80% of its 200k window)`). The old key `threshold` is no longer read: `bise doctor`'s config line and `bise models` say to rename it.

## 10. The history (invariant)

- **Append-only, in arrival order.** No section per agent that gets updated
  later, no reordering, no line that moves. What changes over time (status,
  age, context, open cards) lives outside the history: panel, header, card
  box.
- **Content is frozen; small status marks are not.** The only in-place
  change on an existing line is its status mark (`✓` → `✓✓` on your message;
  a card's answered state, see §12).
- **Only the tail can grow.** A run of level-3 lines longer than 3 folds into
  one dim line `▸ 47 messages between 30 agents`; `▸` opens it in place, in
  order. Only the last run, at the bottom, can still grow (a small pulsing
  `∿` shows it's live). As soon as a level-1 or level-2 line is appended, the
  run is closed and frozen.
- **Time marks** after a pause: a faint `· 14:31 ·` after 5 minutes without a
  line (**⚠** threshold to tune). The pause is the hub's time of the lines,
  so a replayed feed (the TUI opened later, a page of older history) has its
  marks too; another day says it: `· yesterday 18:02 ·`, `· sep 28 18:02 ·`
  (BISE-271).
- **When a turn ended**, on hover (BISE-271): the mouse over a reply (or the
  row that ends its turn) shows the time, dim, right-aligned in the column on
  that row (else the nearest row of the reply with room): `12:41 · 1h ago`
  (`12:41 · now` under a minute, `5m ago`, `23h ago`), then from a day on
  `yesterday 18:02`, `sep 28 18:02`. Drawn over the frame: no text moves; no
  blank room at the row's end, nothing. Gone on a click, a key, a move away.
  Dim, not faint: you asked to read it (designer). No separator between
  turns: the pause marks do that.
- Views are filters of the same stream (entering an agent shows its thread),
  never a regrouping.

## 11. Reading

- **Measure.** Prose wraps at `min(width − margins, 88)`. Code (scripts,
  diffs, outputs) up to 100 columns; longer lines wrap with a hanging indent
  and a faint `↪`.
- **Scripts in full.** bash and TypeScript scripts are always shown whole,
  with syntax colors. (Today `render.rs` folds code over 60 lines to 40:
  that goes for scripts.)
- **Progressive disclosure**

| Item | Default | Disclosed |
|---|---|---|
| thinking | `∴ thought for 14s ▸` | the full text |
| bash / TypeScript script | **always in full** | — |
| bash / TypeScript script and output | inside its box, 15 rows in all, `▸ n more lines` on the last (see *Scripts: a box*) | the whole script and output |
| other tool results | `▸ output · 42 lines · 1 failed` | the full output |
| sub-calls | `↳ github.search_issues ✓` | — |
| file edit | `± edit web/src/auth/session.ts ✓ +3 −1 ▸` | the diff |
| a run of edits (BISE-304) | `± ▸ 3 files · session.ts, token.ts, README.md  ✓ +7 −2` | the edits' rows, in order |
| report in main | `✓ bench is done. p95 at 180 ms ▸ report` | the report |
| brief (inside an agent) | `◇ brief ▸` | the brief |
| a run of level 3 | `▸ 12 messages between 8 agents` | the messages, in order |
| a long message of yours (BISE-239, BISE-262) | its first 20 rows, `▸ 10 more lines` | the whole message |
| a card | open while it needs you | see §12 |

  Keys: click or `space` on the selected item toggles it; `ctrl+o` opens or
  closes everything folded, one state (like Claude Code; `ctrl+t` is gone).
- **Calls: one row** (BISE-223, user request, designer's reco; agents' views too since BISE-227). In every feed (main's and each agent's) each bash or typescript call is one dim row: `$` or `ƒ` in the lead column (the error color when failed), then the model's own description of the call (the optional `description` parameter of bash and run_typescript: one short line in your language), then its state right-aligned: `∿ 12s` running (the description in the text color), `✓ 0.3s` done (all dim, never green), `✗ exit 1 · 0.8s` failed (error color) plus one row under it, from column 3, with the first error line of the output, cut with `…`. No description: the script's first line in its code colors. A long description is cut with `…` before the state. Calls in a row stack with no blank row. 4 done calls or more in a run (the thinking between them included) fold into one row at the first one's place, `$ ▸ 6 commands · <first description>  ✓ 3.2s` (the total time); failed and running calls keep their rows, nothing moves; a click on the fold brings the rows back (`▾`). A click or `space` on a row opens its box (15 rows), a click on a box that hides lines opens it whole, then back to the row; `ctrl+o` opens every call as its whole box (folds too), again: every row. A failure never opens by itself. An agent's view draws the same rows, the same fold, the same keys (BISE-227, user request: designer's all-screens #s6 'the same view in rows'). An open box's title is the kind and the description (`╭─ $ weighing the hero image ✓ 0.1s ─╮`; without one `$ bash`, `ƒ typescript`). A box that only sends a message (BISE-110) stays hidden, ctrl+o shows it as a box. NO_COLOR: same glyphs, no pulse, the error row starts `✗ `. ASCII: `$` / `f`, `~ 12s`, `ok 0.3s`, `x exit 1`, `> 6 commands`.
- **Edits: one row per run** (BISE-304, main's task, designer's calls). 3 done edits or more in a run (edit, write_file, apply_patch; only thinking between them: a bash call, text or a message ends the run, so edits and commands never fold together) fold into one dim row at the first one's place, like the commands fold: `± ▸ 3 files · session.ts, token.ts, README.md` with `✓ +7 −2` right-aligned (the lines of every done edit; no `−0`). Each file counted once; one file edited several times counts the calls: `± ▸ 4 edits · theme.ts  ✓ +8 −3`. Names: basenames in run order, a basename twice gets its parent (`auth/index.ts, ui/index.ts`); whole names that fit, then `+3 more`; a name is cut with `…` only when not even one fits. Failed and running edits keep their own rows (never counted), nothing moves; the current run folds as soon as its 3rd edit is done. A click or `space` on the fold: `▾` and today's edit rows under it (each opens its diff); `ctrl+o` opens every fold and diff, again closes them; ctrl held: what the commands fold shows. ASCII: `% > 3 files`, `ok +7 -2`. NO_COLOR: same glyphs. Every view.
- **Skill calls: a sentence** (BISE-283, the user's onboarding test: `skill ✓ 0.0s · {"name":"bise-demo"}` then `▸ output` looked like plumbing; words by designer). A `skill` call is one row, blank lead column, never the tool's JSON nor `▸ output`: `reading skill bise-demo` while it runs (the verb dim, the name in the text color, `∿ 1s` on the right), `read skill bise-demo` once read (all dim, nothing on the right: it takes no time worth a `✓ 0.0s`), `read skill bise-dmeo  ✗ unknown skill` when it failed (the runtime's reason up to its first `:`, error color, no error row under it). A click or `ctrl+o` opens its SKILL.md in a box titled with the sentence (`╭─ read skill bise-demo ✓ 0.0s ─╮`, no script, no rule), like a bash call; skill calls never fold into `▸ n commands`. Every view, main's and each agent's.
- **Scripts: a box** (user request, marketing 82f1742). Each bash or typescript call is a box, rounded (`╭─╮ │ ╰─╯`), the width of the reading column, content at 2 columns of inner margin, its title in the top border (`╭─ $ bash ∿ 12s ─…╮`, `ƒ typescript ✓ 1.1s`, `$ bash ✗ exit 1 · 0.8s`). Border: text color while running (with the pulsing `∿`; not the accent), faint when done, error when failed. Inside: the script, a faint rule `├──┤`, then the output, dim. Closed, the inside (script + rule + output, in rows after wrapping) is 15 rows at most (BISE-123, user: « il faudrait un max line de 15 lignes, sauf si tu es en mode ctrl+o dans ce cas tu vois tout. Il faut montrer qu'il y a des lignes cachées en bas du block »). When it does not fit, its last row is a dim `▸ n more lines` (`▸ 1 more line`), n = every hidden line, script and output together, and the 14 others split: the script gets its first rows, up to 5 (more when the output is short: the output always shows whole when script + rule + output fit), the rule 1, the output the rest (8 when both are long); a script with no output yet gets 14. Done ok: the output's first lines (the rest is below); running (as it streams) and failed (the error is at the end): its latest lines. A line cut to fit counts as hidden. 15 rows or fewer: all, no marker, no fold. `▸` (click, `space`) or `ctrl+o` opens the whole box, script and output, in place and closes back to the same 15. TypeScript sub-calls `↳ github.search_issues ✓ 0.8s` are output lines inside the box. ASCII: `+- $ bash ok 0.9s ---+`, `|`, `+---+`, `> 27 more lines`. The box replaces the separate folded `▸ output · n lines` line and the code/output left rails.
- **Failures**: a failing bash/typescript call is a box with an error border (above); other failing tools (edit, read, web) stay one line in error color with the reason, `▸` for the full error.
- **Markdown** in messages: headers, lists, quotes, code fences, inline
  bold/italic/code, and GFM tables (BISE-87): no frame, columns 2 spaces
  apart, bold header over one faint `─` per column, aligned by display
  width (`---:`, `:---:`); up to the code measure, then the widest column
  shrinks and wraps inside it (a blank line between rows once one wraps);
  too many columns: one `title` + `  key  value` block per row.
- **Links** in messages (BISE-211): `[label](url)` shows the label,
  `<https://…>` and a bare `http(s)://…` the url (trailing `.,;:!?` and
  an unbalanced `)` stay text); http, https, mailto, file only; never
  inside `code`. Look: the label in the text color (a bare url dim),
  underlined, the underline in the accent (SGR 58; light theme: the
  light accent); `NO_COLOR` and ASCII: a plain underline. Each cell is an
  OSC 8 hyperlink (Ghostty, kitty, iTerm2, WezTerm, tmux 3.4+ with the
  `hyperlinks` feature), a wrapped link is one link on each of its rows.
  A plain click (press and release, no drag) opens it (`open`,
  `xdg-open`, or `BISE_OPEN`) and says `opening <url>`; the terminal's
  own cmd+click works too. The copy of a selection writes
  `label (url)`. `BISE_HYPERLINKS=0`: no OSC 8, and a label shows
  `label (url)`, the url dim.
- **File links** (BISE-264, user: « que les liens dans l'historique vers
  des fichiers locaux soient cliquables […] ça les ouvrirait dans ton
  éditeur par défaut »): a local path is a link like a url, same look
  (a bare path dim, a code span in its code color, underlined in the
  accent), OSC 8 `file://…`. Which paths: a markdown link to a file
  (`[the guide](docs/guide.md#L12)`), an inline code span that is a path
  (`` `rust/tui/src/links.rs:42` ``), a bare path in prose and in a done
  tool row (a call's description, its error row, a tool's args, the file
  of a one-file edit). A path has a `/` (`./x`, `../x`, `/abs`, `~/x`) or
  is a `name.ext`; `:line`, `:line:col`, `#L12` give the line. Only a
  file that exists: a relative path resolves against the feed's agent
  (its private worktree, its folder), then the workspace, then bise's
  cwd; the stat is cached 5 s. A plain click opens it in the editor:
  `BISE_EDITOR`, else `$VISUAL`, else `$EDITOR`, else the file's default
  app (`open`, `xdg-open`, `BISE_OPEN`; no line). A GUI editor runs
  detached, at the line in its own syntax: code, cursor, windsurf,
  codium `-g file:line:col`; zed, subl `file:line:col`; idea, webstorm
  and the other JetBrains `--line N --column C file`; mate `-l N`; gvim,
  mvim `+N`. A terminal editor (vim, nvim, vi, nano `+N,C`, emacs, micro,
  kak `+N:C`, hx `file:line:col`, and any editor bise does not know, given
  the file alone) runs in the terminal panel, never by suspending the
  TUI: the panel shows with the keys, titled `terminal · vim · ctrl+` hide`;
  the shell waits behind it and the panel goes back as it was when the
  editor exits; one editor at a time. The status row says `opening
  links.rs:42 in code` or `vim opens links.rs:42 in the terminal panel ·
  ctrl+` hides it`. The copy keeps a file link's label alone when it
  names the file.
- **The mouse pointer** (BISE-272, user: « que les éléments qui ont une
  interaction au hover changent le cursor? genre pointer sur les liens, drag
  ou resize sur les parties resizable »): a hand (`pointer`) over what a
  click does: links and file links, the history's rows a click opens or
  closes (a thinking section, a tool row, `▸ n more lines` of your
  message, a fold), `↓ back to the bottom`, the panel's rows, the inbox's
  rows, tabs, choices and `×`, the palette's entries; the text cursor
  (`text`) over the composer; `ns-resize` on the terminal panel's top
  border (the one part a drag resizes: the panel's rule and zen have no
  drag). The default elsewhere, under a popup, the help or a hint, over
  the terminal panel's inside, and while the terminal is out of focus. A
  drag keeps the shape it started with (the border's `ns-resize`, the
  composer's `text`). Each frame says the shape of what it draws, the last
  drawn wins; the terminal gets OSC 22 (`ESC ]22;pointer ESC \`, kitty's
  CSS names) only on a change, and `default` on exit. Only in Ghostty and
  kitty, outside tmux (it does not pass OSC 22); `BISE_POINTER=0` off,
  `=1` on in another terminal that has it (WezTerm, iTerm2 and
  Terminal.app do not: nothing is written there).
- **The terminal's tab title (term-title, designer's pick):** while bise
  runs, the tab says what waits for you, then the project:
  `#2 ↗1 ●3 · harness`. `#N` the inbox's cards (the header's `#`), `↗N`
  the new artifacts (the header's `↗`), `●N` the agents at work (working
  or waiting on another agent; main left out), then the repo's folder
  name (cut at 32 with `…`). A count at 0 is left out; all at 0: the
  folder alone (`harness`). No `bise` word, no `:*`: the tab is the
  user's. The counts come first: a narrow tab (`#2 ↗1 ●3 · h… ⌘1`) cuts
  the end, and the folder is the part you can guess. `●`, not the TUI's
  `∿`: the tab's system font draws `∿` as a tick. `BISE_ASCII=1`: the
  TUI's ASCII forms, `#2 +1 *3 . harness` (`↗` has none: `+`). Written
  with OSC 0 once the text held still 300 ms; the title you had is
  pushed first (`CSI 22;0 t`) and popped on exit or crash (`CSI 23;0 t`,
  after an empty title for the terminals without the stack). Never when
  stdout is not a terminal; `BISE_TERM_TITLE=0` off. Ghostty, iTerm2,
  kitty, WezTerm, Terminal.app, tmux (`#{pane_title}`, the outer tab
  with `set-titles on`).

## 12. The inbox

The inbox (BISE-236 cards v2, BISE-248 its name and its arrows; designer's
round 3, mocks `inbox · 1-7` and `inbox · the keys, checked` in the
screens page, approved by the user: « Inbox c'est beaucoup mieux,
implémentons la proposal »). The place is the **inbox**; what waits in it
keeps its own noun (a question, an approval), never "card" on screen (the
code keeps `Card`). One rule for the arrows: **an empty composer, they
belong to the inbox; text in the composer, they belong to your text.** In
the thread they never touch the inbox: `ctrl+1`…`ctrl+9` and a click open
an item (BISE-302).

**Opening an item (BISE-302, the user: « utiliser ctrl+1 2 3 4 pour ouvrir
la card directement, plutot que devoir faire ctrl+g […] clicker sur une
notification l'ouvre directement »; designer's calls).** `ctrl+N` opens
row N of the strip (1 = the most blocking, the rows under `+ n more`
too) in the item view, from the thread or from another item; past the
last row: nothing. A click on a row opens it. `ctrl+g` and the inbox
selected (`▸`, `↑↓` on the rows, digits answering a row) are gone. On a
French layout the kitty protocol reports the top row's own characters
(`&é"'(-è_çà`, a Mac's `§` `!`): they count as their digits. Where
ctrl+digits reach bise: a terminal speaking the kitty keyboard protocol
(Ghostty, kitty, WezTerm, foot…: it answers `CSI ? u`), tmux with
`set -s extended-keys always` (its `CSI 27;5;49~`, read by our crossterm
patch). Elsewhere (Apple Terminal, tmux by default, an old xterm) ctrl+1
types `1`, ctrl+2 is ctrl+space, ctrl+3 esc, ctrl+8 backspace: **never a
key that doesn't work on screen** (designer): the strip's label says
`click to open` (no clicks either, tmux with `mouse off`: `/inbox opens
it`), the key bar `/inbox`, the help `/inbox   open the inbox (or click a
row)`, the onboarding `… in your inbox · click it`, the first-item hint
`… click it, or type /inbox.`, the demo's tip `{click} the question`;
the rows keep their numbers (they still match the panel). ctrl+4-7 come
from any terminal (0x1c-0x1f) but are never offered alone. A ctrl+1,
2, 3, 8 or 9 that arrives proves the terminal sends them: the words switch
to ctrl. `BISE_CTRL_DIGITS=0|1`, `BISE_CLICKS=0|1` override what is
detected. `bise doctor` inside tmux without `extended-keys always`: `?
tmux  tmux eats ctrl+1-9`, fix `add "set -s extended-keys always" to
~/.tmux.conf`. ctrl+k / ctrl+j no longer move between agents (too many
shortcuts, the user): `⌥↑↓` select, `⌥0-9` go; ctrl+k is the line end,
ctrl+j a new line again.

**Whose inbox (BISE-299, the user: « il faut des inbox séparées pour les
agents et pour l'utilisateur »).** The inbox is yours and holds only what
needs you: main's escalations (`sb card`, `sb card --for <msg>`: your
answer goes to the task that asked; main's `sb drop` of a task you must
confirm) and, once the gate exists, tool-call confirmations (kind
`confirm`, straight to you whoever runs the tool). An item's inbox is set
by its kind when it opens, never by who answers first. **Only you answer
or close it**: main's or a task's reply to an escalated question is
refused, `sb close` too; main may take back its own question when it is
moot (`sb card --withdraw N "why"`, a dim `#N withdrawn by main: why` in
main's feed). The agents' traffic (their questions to main, reports,
blocked, file overlaps, crashes) is **main's inbox**: it never opens an
item here. You still see it, never asked: the `@`/report lines of main's
feed, the agent views, a dim `@ 2` on main's row of the panel while two
questions wait for main, a dim `?` on a blocked task's row (the accent
`?` and "needs you" come only from an item in your inbox). An older
journal's items of the other kinds (done, blocked, failed, overlap) close
at the hub's start, "moved to main's inbox".

The inbox's box and its keys (designer's round 2, variant A, approved by
the user: « franchement, c'est parfait »; mock `inbox-redesign.html`). One
rule: **the inbox never blocks the thread or a normal message.** An item
opens where it is, a digit answers it and the next one opens, esc gives
your draft back, the thread stays in sight the whole time.

- **The box.** Right above the divider (1 blank row above it from 24
  rows), while something waits; none when nothing does. Its own block: a
  rounded border in the line color, from the gutter (a column left of the
  composer's bar, 2 from the frame at least) to the panel, on the ground
  (no tint). Its title in the top border, faint: `╭─ inbox · 4 waiting
  for you ───── ctrl+1-3 open ─╮` (`inbox · 2 waiting` when short); the
  right part says what opens a row (BISE-302): `ctrl+1 open` with one
  row, `ctrl+1-2 open`, `ctrl+1-3 open` (the key in text color, its
  words dim; without ctrl+digits `click to open` or `/inbox opens it`).
  Inside, a row per item, most blocking first (approvals, questions, the
  rest): `1 ? t3 · $ npm publish --access public   2m`: the number faint
  (accent while ctrl is held, bold under NO_COLOR), the glyph in its
  color, who (the agent), ` · ` dim, what (an approval's `$` in accent
  and its command's first line, a question's first line, a patch's first
  file and `+2 files · +42 −7`) in text color, the age faint on the right.
  At most 3 rows, then `+ n more · ? release · …` faint (a click opens the
  first of them). Approvals that open together (5 s) share a row (`3
  agents`). Under 24 rows: one row, `+ n` before its age. A click on a row
  opens it, on the title the top item.
- **Open in place.** `ctrl+N` or a click on a row opens that item inside
  the box, where its row stood; the other items stay rows above and below
  it (an item past the third takes the third's place), the thread above
  the box. The open item is on the item tint (one step above the
  composer's: `#26221f`, light `#f2ede6`; none under NO_COLOR) with the
  accent `┃` (ASCII `|`) on every row, a blank bar row at the top and the
  bottom: its head `? t3 wants to run` (the title bold) with `1 of 4 · 2m`
  dim on the right (a question: `? cookies asks`); what it asks (the
  command in the code colors, a question's text); a blank line; dim, why
  it asks, the hub's remark and where the agent is (`t3 is shipping 2.5.0
  · its last step: ✓ npm run build · 12s`: its role and its last finished
  tool row while its feed is loaded; left out when unknown, and for
  main, whose question is its own `sb card`); a blank
  line; the options on one line (`1 allow   2 always allow npm publish *
  here   3 no`, digits accent, labels dim; one per line when they don't
  fit); the hint, faint: `or type why not, ⏎ says no` (an approval), `or
  type your answer, ⏎ sends it` (a question). It takes at most half the
  feed area: past that its first lines, then `… n more lines · ctrl+o
  full screen`. Opening puts the composer's draft aside (kept, its
  cursor too); the divider reads `you → ? t3 · your answer`; esc closes
  the item back to its row and brings the draft back.
- **Keys with an item open.** `1-9` pick an option (empty composer only;
  once you type, digits are text). `←→` highlight an option (no wrap; the
  first `→` is option 1, the first `←` the last; nothing highlighted on
  open: a reflex `⏎` never answers); the highlighted one is accent on the
  ground (inverse; reverse video under NO_COLOR). `↑↓` the previous / next
  item (no wrap). `⏎` picks the highlighted option; with text typed it
  sends the answer (an approval: a no with the text as the note); nothing
  highlighted and no text: nothing (a done or overlap item: `⏎`
  acknowledges it). `ctrl+N` jumps to item N. `ctrl+o` full screen. `esc`
  back to your message. Typing: the arrows are your text's, the options'
  digits dim and the highlight hidden (kept). `ctrl+↑↓` and `ctrl+←→` are
  silent aliases, never shown (macOS keeps them for Mission Control and
  Spaces); `ctrl+n` / `ctrl+p` (around) and `ctrl+x` (close without
  answering) stay, unshown. Each item keeps its own draft. `⌥0-9` goes to
  an agent (the item closes); `ctrl+f` is find.
- **Key bar with an item open:** `1-3 answer   ←→ choose   ↑↓ other
  items   ctrl+o full screen   esc back to your message` (a hard rule
  `1 3 answer`; an option highlighted: `⏎ always allow` first, the
  option cut at 32 columns; one item: no `↑↓`); text typed `⏎ says no,
  with your note` (a question `⏎ sends your answer`), `ctrl+o`, `esc`.
  Under 100 columns: `1-3 answer   ↑↓ other items   esc back`; then the
  pairs drop from the right, what answers last.
- **After an answer.** The item folds into one line at the top of the box
  for 2 s: `✓ you allowed t3: npm publish --access public` (✓ accent, the
  words dim), `✗ you said no to api-v2: git push … · "use a branch"` (all
  dim), `✓ you answered cookies: both`; the next item opens by itself, so 1,
  1, 3, 1 clears four. The agent's `?` in the panel turns back into its
  gust at once. The same line lands in the thread (a gate's from the hub,
  the others the same sentence, no leading `·`). The last one answered: the box
  goes, your draft comes back, the divider says `✓ inbox clear` for 2 s
  (✓ accent, the words in text color, like `✓ copied`).
- **Full screen** (`ctrl+o` on an open item; the old item view): the item
  takes the history's place, the others as tabs on top: `1 ? t3   2 ?
  api-v2   3 ? cookies` (the box's numbers; the current one accent, `[ ]`
  under NO_COLOR, the others dim; `↑↓` faint on the right); its head, the
  whole text at the reading width (88), the options and the hint. `pgup`
  / `pgdn` and the wheel scroll (`▾ 12 more lines · pgdn` on its last
  row); a highlighted option scrolls into view. `ctrl+o` again puts it
  back in place, `esc` back to your message. Key bar `1-3 answer   ↑↓
  other items   pgup pgdn scroll   ctrl+o back in place   esc back to
  your message`.
- **Kinds' options:** an approval `1 allow / 2 always allow … here / 3
  no`; a hard rule `1 allow / 3 no` (no 2: 3 is no everywhere); a sandbox
  rerun `1 run it again without the sandbox / 2 always run it outside the
  sandbox here / 3 no`; a question its choices, or free text; ready to
  merge (when PRs land) `1 merge it / 2 not yet`. Reports, done and FYI
  stay out of the inbox: they are main's thread.
- **80 columns:** the same box from column 2, the options one per line
  when they don't fit.
- **Removed:** the strip's raised top row and its label row (the title is
  in the border now); the item view as the way in (it is full screen,
  `ctrl+o`); `↑↓` choosing options and `←→` switching items (swapped,
  round 2); before, `ctrl+g` and the inbox selected (BISE-302: `ctrl+1-9`
  and a click open an item at once), `ctrl+g` opening an item directly
  (BISE-248), `↑↓` scrolling an item; earlier (BISE-236): `ctrl+f` full
  screen, `alt+r`, `ctrl+a` on an empty composer, `ctrl+x` / `ctrl+n` /
  `ctrl+p` from the thread, the divider's `? n cards · ctrl+g`.
- A new item never takes the focus: a row in the box.
- **Kinds** reuse the glyphs: `?` approval, question and blocked (accent),
  `✗` failed, `↻` restart failed (error), `–` drop confirmation, `⇄`
  overlap, `✓` done.
- **Approvals** (docs/approvals.md §7) plug in as the kind `approval`: the
  text is the command (or a unified diff), a blank line, the reason; an
  option answers `/answer N allow once|always here|deny`, `⏎` with text
  `/answer N deny: <note>`. The gate itself is not built yet.
- **Answered cards fade in place** (decided by Gabriel, 2026-09-28): once
  answered, the card in the history turns grey (dim bar, `answered`), your
  answer follows as a normal line. BISE-31.

## 13. Talking to agents

- **To main:** `⏎`. Main says who takes what: `:* on it: auth-fix takes the
  safari bug, release takes the note.` New agents appear in the panel.
- **To an agent directly from main:** `@name text` (popup with the agents).
  Main is not in the loop; the agent's reply to you is level 2:
  `@ auth-fix to you: got it, i'll check logout after the login fix.`
- **Files outside the workspace** (BISE-206). `@../`, `@~/`, `@/`: the
  popup lists the folder typed so far, only it (no index, nothing read
  ahead), so macOS asks for Desktop, Documents, Downloads, iCloud or a
  volume only once you enter it (their rows say `protected`). A folder
  you cannot read lists nothing (`this folder · no access`). A pick
  inserts `~/` expanded to the home folder; `../` and `/` as typed.
- **Inside an agent:** `⏎` on a selected agent or `⌥ + number`. One dim line
  says: `you're talking to auth-fix directly. main isn't in the loop. esc back
  to main.` `@main` goes back up.
- **Steering and marks.** While an agent works, `⏎` steers its turn. Your
  line ends with a mark: `·` sending, `✓` the agent got it
  (`steering_received`), `✓✓` in accent the model read it (`steered`). This
  replaces today's info lines "steering received: …" and "steering passed to
  the model: …". A message at idle goes straight to `✓✓`. If the agent is
  gone: `✗ not delivered: auth-fix stopped. ⏎ send again · esc drop` (**⚠**
  new).
- **Nothing typed is lost** (BISE-120a). Each agent's draft (its text,
  its cursor, the images it names while their stored copy exists) and the
  sent prompts (`↑`/`↓`, the newest 50) are kept per workspace on disk,
  in the Switchboard state root (`drafts/<folder>-<hash>.json`, mode 600).
  A draft is written once it has not moved for 300 ms, and when the UI
  ends (quit, `/restart`'s reload, a version switch); a crash loses at most those
  300 ms. At the next launch every draft is back in its composer and `↑`
  recalls the prompts sent before. Sending a draft takes it off the disk
  at once. No message about it: it just works.
- **Ask about a selection** (BISE-134, after Vibe Work's "Ask about
  this"; user: « si je sélectionne du texte dans le thread et que je me
  mets à taper … ça met le texte que j'ai sélectionné en contexte pour
  l'agent »). Select text in the history (the release still copies it);
  the key bar says `type ask about it   cmd+c copy   esc drop`, `type ask
  about it` in the accent, `type` bold, so you notice you can just type
  (the only accent of the bar; `NO_COLOR`: the pair bold). Once the
  drag ends, a one-row pill says it where the eyes are too (BISE-229):
  ` type ask about it · cmd+c copy ` on the pink pill, on the row right
  above the selection, at its first column (pushed left to stay in the
  history's column; never over the panel, the divider or the composer).
  No room above: under the last row; no room either: none. Narrow: only
  ` type ask about it `. A press, a scroll, typing or esc puts it away;
  `NO_COLOR`: `[ type ask about it · cmd+c copy ]`, no tint. The first
  key you type puts the selection in the composer as a quote chip `❝ 1`
  at the cursor (BISE-207), and ends the selection; the key types right
  after it. A chip, quote or image (paste, drop, `@`), always goes in at
  the cursor, like a paste: a space before it when it would touch a word
  or another chip, one after it. On send the quotes still go in front of
  your text, in text order: where a chip stands only sets that order. Selecting alone adds nothing: a selection is often only a
  copy, and a quote you did not ask for would ride along with your next
  message. The attachments section lists it: `❝ 1 “the login breaks on
  safari…”  main · 3 lines`; a backspace on the chip removes it, like an
  image.
- **A long paste is a chip** (BISE-240; user: « Paste un texte long
  devrait faire une petite chip et un attachment pour éviter de saturer
  l'input »; designer's spec). A bracketed paste of 12 lines or more, or
  1200 characters or more, goes in as the chip `▤ 1` at the cursor (it
  replaces the selection, like an image); its text is an attachment,
  the box lists it: `▤ 1 “first words of the paste…”  240 lines · 9.8
  kB` (the right part faint, cut first). A shorter paste stays inline;
  typed text never becomes a chip. The first undo right after the paste
  puts the text inline, a second one takes it away; backspace on the
  chip removes it. On send the chip becomes the whole text where it
  stood, in one tag the model reads as pasted: `<pasted n="1"
  lines="240">…</pasted>`. In the history the chip stays in your line
  (`▤ 1`, accent) and one dim row under your message says what it is,
  `▤ 1 “first words…” · 240 lines`; opening your message (ctrl+o, a
  click or space on its fold, the user-message fold) shows the full
  text there. Never the full text by default. Images, quotes and pastes
  count in one sequence (`❝ 1`, `▣ 2`, `▤ 3`): a number always points
  at one row of the box. `NO_COLOR`: `[▤ 1]`; ASCII: `[T 1]` (`=` is
  the compaction's).
- **A chip is a pill** (BISE-205; user: « elles ne sont pas assez
  clairement définies … on dirait trop du texte »; designer's pick). In
  the composer and at the start of each strip row, a quote or image chip
  is drawn ` ❝ 1 ` / ` ▣ 2 `: 5 tinted cells on the pink `pill` tint
  (§5), one padding cell each side, the glyph accent, the number in the
  text color. No space is added around it: your spaces stay yours. The
  cursor on it reverses the whole pill; a selection covers the whole
  pill. 16 colors / `NO_COLOR`: no tint, `[❝ 1]` (brackets dim); ASCII:
  `[" 1]`, `[# 2]`. Same width in every form, so nothing moves. Zen keeps
  it as is (the typed text is never faded). Half-block caps (`▐…▌`) were
  turned down: they leave seams in many fonts. Several selections, several quotes (4 at most, 8 000 characters
  each). A `/` typed in an empty composer stays a command (no quote). The
  draft keeps its quotes like its images. On send, each quote leaves the
  text and goes in front as a tag, then your words:
  `<selection from="main">` newline, the selected text, newline,
  `</selection>`, newline, your text. `from` names who wrote the selected
  lines (the agent in view, `you`, the sender of an agent message; several
  joined with `, `). Your line in the history shows each quote as one dim
  line above your words: `❝ the login breaks on safari… · main · 3 lines`.
- **Queued messages** (BISE-89, after Codex). During a turn, `tab` keeps
  the composer text for after the turn instead of steering: it stays in
  the TUI, **nothing goes to the hub** until it leaves the queue. The queue
  shows just above the composer, one dim line each (` › text…`, cut to the
  width), newest last, then a faint `queued · sent when this turn ends · ↑
  edit`. `↑` in an empty composer pops the newest back to edit (the
  history comes after the queue); `tab` queues it again, `⏎` steers it
  now, clearing the composer drops it. When the turn ends, the oldest goes
  out as a normal message (marks `·` → `✓✓`) and starts the next turn; the
  next one waits for that turn to end. One queue per agent (main and each
  agent), kept even out of view; the panel row shows `· {n} queued`. A
  reload or a version switch (the TUI re-executes itself) keeps the queue:
  it is saved with the drafts and comes back when the hub's replay ends;
  an agent idle by then gets the oldest at once (BISE-131). A later
  restart of the TUI (over 60 s) drops it: old lines never fire at an idle
  agent after a restart.
- **No undo.** Agents may already have acted, so an undo promises too much.
  To change something, you say it ("no, v1 for docs"). Main sends the agent an
  explicit correction (`the user changed their mind: use v1, not v2.`) and
  confirms in one line (`:* told docs: v1, you changed your mind.`). Today's
  `ctrl+z` / `/cancel` go away.
- `ctrl+c` interrupts the turn of the agent in view; again (or at idle)
  quits; the agents keep running.

### The composer block (layout; user request, marketing 393dbd3)

Bottom of the screen, top to bottom:

1. the status row (1 row);
2. the queued messages (BISE-89) and their faint hint, if any: they are the composer's pending texts, so they sit right above it;
3. the attachments strip, if any (the composer's quotes, §13 "Ask about a selection", then its images, §14);
4. the composer: a bar `│` in column 1 on every row of the block, faint while the composer is empty, accent as soon as there is text (the same bar your message keeps in the history, so a sent message just moves up unchanged); 1 blank row (bar only) above the text and 1 below; the text from column 3, at least 2 rows, growing one row per wrapped row up to min(12, 40% of the terminal height), then scrolling with the cursor row in view; right margin 2 columns; the text wraps at the same width as your message in the history (§11), so the composer shows how it will read. Empty: the cursor at column 3 and the dim placeholder. Voice (BISE-222): one atomic chip in the text at the cursor, the pill of the quote chips: recording ` ● ▂▅▃▆▂▃ 0:07 ` (the `●` blinks accent/dim every 600 ms, the last 6 levels in accent scrolling left, in decibels so speech fills the bars and a quiet room stays flat (BISE-246), the timer in the text color), transcribing ` ∿ ▂▃▅▆▅▃ 0:07 ` (same width, a dim wave rolling one cell every 120 ms, the timer held); the transcript replaces it in place; `[● ▂▅▃▆▂▃ 0:07]` under NO_COLOR (the blink bold on/off), `[* .=-#.- 0:07]` / `[~ .-=#=- 0:07]` in ASCII; under 40 text columns no timer, under 20 three bars. `›` leaves the composer (it stays for the queued lines).
5. the key hints: their own last row, dim, flush right with 2 columns of margin (never on the text row).

Rows: 1 status + 1 + 2 text + 1 + 1 hints = 6 at minimum (+ the blank row under the feed). Small terminals: height < 24 drops the bottom blank row; < 18 also the top one, and the minimum goes to 1 text row; < 14 the hints go back on the status row.

**The composer pane** (framed, raised; user request, marketing e71ec74; supersedes the list above for the status row and the hints, see §8 "The frame" for the frame rows). The bottom of the frame belongs to you. The divider says who you talk to (you → main, the name in blush) and, on the right, what they're doing. Everything under the divider is slightly raised, like the bottom of an app: a tinted row, your text behind the same thin bar your messages keep in the history (faint while empty, blush once you type), at least 2 rows that grow with you, then a tinted row and the key bar: keys in the text color, what they do dim, a tip on the right when you're idle. When you talk to an agent directly, the key bar starts with `esc back to main`, so the way home is always in sight. Empty, the composer asks: what's on your mind?

Exact (BISE-102, BISE-103):
- **The raised pane** (palette role `raised`, §5; user request on marketing 685220f, replaces the raised block of e71ec74): every cell from the row under the divider to the row above the frame's bottom border (H−2), columns 1..F−2 (the whole inside of the frame, the margin columns included, no ground between the grey and the side edges). The frame's lines stay on the ground, outside the grey, on all four sides: the divider row with its label and state, the side edges, the bottom edge (BISE-212, user request: BISE-210's grey over the lines looked bad). No frame (H < 16 or F < 60): full width, from the row under the divider down to H−1; the divider stays on the ground.
- **On the tint, top down** (H ≥ 20; BISE-108 two sections, BISE-111 symmetric padding, user requests): [queued lines, BISE-224 (user request): 1 blank tinted row between the divider and the first one, the body's own blank bar row under their hint] · [the attachments box, BISE-209: 1 blank tinted row, then the box, see "The attachments box" below; no blank row between it and the body, whose blank bar row above the text goes (the bar starts at the first text row)] · the body: the bar `│` at x0 on each of its rows (faint empty, accent with text, an image or recording): 1 blank bar row (none under the box: the box gives the top space), the text from x0+4 (BISE-228, user request: ~8 px more room; one column right of the history's text column, the pane's padding), wrapped at word boundaries 2 columns before the pane's right edge; under 60 columns x0+3 and no right margin as before, ≥ 1 row, grows to min(12, 40% of H), then scrolls, 1 blank bar row (BISE-219, user request: room around what you type; replaces BISE-210's none). The two bar rows are part of the typing area: fixed while the text grows and scrolls between them, tinted like the text rows (zen keeps the tint) · the key bar (from x0+4, the text's column; x0 under 60 columns; in an agent's view `esc back to main` first) · frame bottom. No blank ground row under the divider (the blank bar row does that job). Height at rest: divider + 1 + 1 + 1 + key bar + frame = 6 rows. H 16–19: drop both blank bar rows, the blank row above the queued lines and the one above the attachments box (4); never one bar row alone. **The tint is one grey block inside the lines** (BISE-212, user request; replaces BISE-210's grey over the frame's cells): from the row under the divider to the row above the bottom edge, from the left edge to the right edge exclusive; the divider, the side edges and the bottom edge keep the history's ground, and the panel's rule stops at its `┴` on the divider. `NO_COLOR`: no tint. H < 16 or F < 60: no frame, same as 16–19. H < 14: the key bar goes into the divider's right side instead of the state.
- **The attachments box** (BISE-209; the user's pick "d", designer's spec; the attachments must stop reading as part of your message). Under the divider, inside the composer pane, above your message: a thin rounded frame `╭╮╰╯─│` in dim from the composer's text column x0+4 (BISE-228, designer; from the bar's column x0 under 60 columns), ending at the text's wrap at the latest; `attached` in its top border (`╭─ attached ───╮`, dim); `backspace on a chip removes it` in its bottom border on the right (`╰──── backspace on a chip removes it ─╯`, faint), dropped first when the box is short. As wide as its longest row plus its frame (2 blank columns each side, the rows inset from the text's column; under 60 columns at x0+3 like your text), at least 44 columns, at most the reading width (the composer's, ≤ 91); full width when narrower. One row per attachment, in number order (quotes and images alike): the pill (§13 "A chip is a pill"), a blank, the preview dim (a quote's words in `“”`, an image's file name), the source faint flush right (`main · 1 line`, `1600×900 · 240 kB`), at least 4 blanks between them. Short on room the preview is cut with `…` down to 16 columns, then the source goes and the preview takes the row. The box has no bar; your message keeps its pink bar, and the chips stay inline where you put them (same number as in the box). A long paste's row: its first words in `“”`, `240 lines · 9.8 kB` (BISE-240). `NO_COLOR`: the same frame (glyphs), pills in brackets. ASCII: `+ - |` corners and edges, `[" 1]` / `[# 3]` / `[T 2]`.
```
╭─ attached ──────────────────────────────────────────╮
│   ❝ 1  “la licence du repo,”         main · 1 line  │
│   ❝ 2  “le README n'est pas prêt”    main · 1 line  │
│   ▣ 3  readme-dark.png           1600×900 · 240 kB  │
╰──────────────────── backspace on a chip removes it ─╯
│  pour  ❝ 1  tu recommande quoi? et pour  ❝ 2 , voilà à quoi il ressemble :
│   ▣ 3  c'est trop long non?
```
- **Placeholder:** dim, `what's on your mind?` (to an agent: `talk to auth-fix directly`).
- **Key bar in an agent's view:** `esc back to main` is always the first pair, from x0, on every key set of that view (idle; working: `esc back to main   ⏎ steer   ctrl+c interrupt`). `esc` in the text color like every key, `back to main` dim. Then the thread's pairs (`esc back to main   @ file   $ skills   / commands`, BISE-303). Never dropped for lack of room: `/ commands` drops first, then the pairs from the right. The right-side tip is hidden in an agent's view. The divider says `you → auth-fix`.

## 14. Images

Built on the technical work of the `screenshots` task
([../images.md](../images.md)): sources are a dragged file (its path is
pasted), `ctrl+v` (clipboard image) and an image picked in the `@` popup.

- **Chip:** an image is one atomic accent chip in the text: the pill
  ` ▣ 1 ` in the composer (§13 "A chip is a pill"), `▣ login.png` in the history. Deleting the chip drops the image.
  (The label underneath can stay `[Image #1]`.)
- **Attachments box above your message** (§13 "The attachments box", BISE-209) while
  images are attached, the file name only (never the path; cut at its end
  with `…`; BISE-108; the divider's flash too: `✓ attached ▣ 1 login-mobile.png`): `▣ 1 login-mobile.png · 1170×2532 · 310 kB`, `▣ 2 clipboard · 2048×1536 ·
  1.1 MB → resized to fit 2048`; `backspace on a chip removes it` in the box's bottom border.
- **History:** your line keeps the chips; one dim line under it gives each
  image's size. No picture drawn in the terminal for now (**later**: kitty /
  iTerm2 image protocols).
- **Routing:** main says it passes the images on (`layout-fix takes it, with
  both images.`). **⚠** To check: the image marker travels in the brief.
- **Tool results:** `result · ▣ screenshot.png 390×844`.
- **Model without vision:** `✗ glm-5 can't read images. pick a model that can
  (/model), or describe the screen in words.` (**⚠** today the provider's raw
  error shows.)

## 15. Onboarding (first launch)

Onboarding v3 (screens `onboarding v3 · …`, the user's pick with one change:
the theme and "how it works" stay). Before the thread, three or four short
steps; no folder step (bise works where you typed `bise`, the header says
where). Step dots: one per step shown (a key found: 3), faint, the current
one in accent.

1. **Welcome**, centered. Typed at ~70 ms per character: `hi, i'm bise`, then
   `:*` pops in accent (scale 0.4 → 1.5 → 1, 0.9 s), then, right under the
   name and a blank row, the name gloss (§1) comes line by line, 300 ms
   apart: `bise /beez/ · french, n.` / `1. a quick kiss on the cheek :*` /
   `2. a brisk north wind` / `3. a terminal where multi-agent coding is
   painless`, then `ideas in. little kisses out. also pull requests.` is typed,
   then `any key ↵`. Any key while it types shows it all at once; then any
   key goes on (no wait for enter).
2. **Theme.** `your terminal looks dark, so i picked dark.` / `you can change
   it any time with /theme.` When no detection was needed, it says why:
   `BISE_THEME is set to light, so i picked it.` or `you picked light last
   time, so i kept it.` (the saved choice); no answer from the terminal:
   `i couldn't read your terminal's background, so i picked dark.` Two live previews side by side (the same four
   lines of a bise feed); `←→` switches, `enter` keeps. Right after the
   welcome, so the next steps already wear the chosen colors. `esc`: the
   thread with the theme the launch had (the terminal's, or the saved one).
3. **A key, when the model in use can't run** (BISE-266). bise has no
   built-in model: with no `model` (config.toml, `BISE_MODEL`) or a model
   whose provider has no key, it sends nothing, and this step shows, at
   the first run and at any later launch (then alone: `keys only`).
   `which model should do the work?` / `i found no key in your
   environment.` (or `i found a key`) / the found keys (`use
   OPENAI_API_KEY found`, sub-line `OpenAI. i'll use gpt-6-astra.`) / `set up
   a provider` (sub-line: every provider offered). Then `which provider?`
   (`1 · Anthropic  Claude, by Anthropic`; five providers, in this order:
   Anthropic, OpenAI, Google AI Studio, Mistral, OpenRouter, the user's
   pick of 2026-09-30, BISE-288; `hidden = true` in the catalog is never
   offered: the foundry proxy, xAI, DeepSeek, Groq, Together, Fireworks,
   Cerebras, whose keys in the env or auth.json and `bise login <id>`
   still work) → `which model?` (`you can change it any time with
   /model.`, then the filter line `› ` + faint `type to filter, or any
   model id` (BISE-289; `› gpt-6-ast▏` once typed: the rows whose id
   holds it, `esc` empties it first), the catalog's pick first,
   `recommended`, then the provider's current models, newest first; a
   typed id the list does not have is the last row, no number: `+ use
   openai/gpt-6-astra   not in my list: i'll try it with one tiny call`
   (`+` accent, the id in the text color, the rest dim; the picked
   provider in front unless typed), after a dim `no listed model
   matches.` when it is the only one, selected. The live check runs with
   it: an id the provider doesn't know gets `<Provider> doesn't know
   <model>.` with its words, and `enter` brings it back typed) → `paste your Mistral key` / `get one:
   <keys page>` (an OSC 8 link; as in the feed, a plain click opens it
   and says `opening <url>`, a drag or a double click copies it and says
   `copied N chars`, under the step dots, BISE-281) / `no account yet?
   <sign-up page>` when it differs / the masked field / `saved in ~/.bise/auth.json. only you
   can read it.` → `checking your key with one tiny call…`: one real
   request to that model (16 output tokens). Failed: `Mistral says this
   key is wrong. copy it again from <link>` (a found key: `the key in
   MISTRAL_API_KEY doesn't work. Mistral says it's wrong.`, `enter`
   pastes another) · `this key can't use <model>.` (a permission error:
   `enter` picks another model) · `Mistral doesn't know <model>. pick
   another model.` · `i couldn't reach Mistral: <why>.`; under it, dim,
   the provider's own words (BISE-282: `Mistral said: "…"`, one line of
   200 chars at most, the key masked as `sk-…abcd`). A refused key is
   tried once more after 3 s, quietly (a new key may take a moment).
   `enter` tries again, `tab` another provider, `esc` back; nothing is
   saved. No credit is not a failure (`? the key works, but your
   Anthropic account has no credit yet.` / `add some here: <billing
   page>` / `i saved the key. add credit, then enter checks again.`):
   the key is saved, not the model, and `enter` checks it again. A url
   in the provider's words stays plain text: the one link is bise's
   line under them (BISE-287: `add some here:` and the url on one row
   when both fit, else the url on a row of its own). Passed: the key
   in `auth.json` like `login` (asks before replacing one), the model as
   `model` in config.toml, then `it works: <model> answered.` / `main
   uses <model>.` and the optional keys not set yet (`web search and
   other tools: a Mistral key · /setup`, `voice input (ctrl+r): a
   Mistral, OpenAI or ElevenLabs key · /voice`), and a dim
   `agents, voice and the rest: /models` (BISE-298). A
   found key is checked the same way (nothing to save). A saved key wins
   over the environment's (BISE-269: what you paste is what runs;
   `bise logout <provider>` goes back to the env); when the env holds
   another key, one dim line says so (`MISTRAL_API_KEY in your
   environment holds another key: i use this one.`), and `bise doctor`,
   `bise auth list`, `bise models` show it. API keys only: no
   browser sign-in. The hub gives the new key to every agent before its
   next message: a REPL spawned with other keys relaunches at its next
   idle, same session.
4. **How it works**, the three lines appearing one by one, the numbers dim,
   `me` and `i` (bise) in accent, no final periods:
   `1  you talk to me: main, your team lead. any time, keep typing`
   `2  i start an agent when a job needs one. they sync on their own`
   `3  only the real decisions reach you, in your inbox · ctrl+1`
   (without ctrl+digits, BISE-302: `… in your inbox · click it`)
   (each fits the 64-column column; narrower, a line wraps at the words
   with a 3-column hanging indent), then, faint, `ctrl+o opens everything folded · ⌥0-9 talk to an agent`,
   and `any key ↵`: any key opens the thread.
5. **The thread** opens on the empty state (§8, §17): `what's on your
   mind?` / `say it and keep talking. the work runs in the background, i'm
   always here.` / `try: "show me what you can do"`, dim,
   at the feed's indent, at 2/5 of the history's free rows (under 12 rows:
   on top, after one blank row); it goes with the first message. No
   first-task suggestion. The first open after the first run (not after
   `/welcome`, not the key step alone) starts with `show me what you can
   do` in the composer, selected: one ⏎ starts the demo (BISE-284).
   **Tune bise: one quiet item** (BISE-245; its words take you by the hand,
   BISE-249, designer's copy a2b7eea, screens `setup, by the hand · 1-6`,
   user: « les cards d'onboarding devraient plus expliquer le concept et
   prendre l'utilisateur par la main »). In the inbox (§12), never opened
   for you, nothing checked yet: `? main · can i set bise up for your
   terminal and this repo?` + faint ` 1 min · i ask before changing
   anything` (when it fits) · `1 yes, check` · `2 not now`. It waits: no
   timeout, no nudge; being the first item, the first-item hint teaches the
   inbox (§15 6). Once per user (`setup` in prefs.json); a user who
   answered gets it again in a new repo, for the repo part only (`new repo:
   can i set bise up for it?`: git and AGENTS.md). `SB_SETUP=off` or
   `SB_ONBOARDING=off`: never. These items are the TUI's own: the hub never
   sees them, they carry no `#` number. Each has its own tab (`set up bise`,
   `Ghostty keys`, `AGENTS.md`, `connectors`) and meta (`about a minute`,
   `Ghostty config · +2 lines`, `new file · 14 lines`, `optional`).
   - **opened**, before any yes: `i'll check 8 things: your terminal, its
     keys, colors, glyphs, git, gh, an AGENTS.md, and the connectors key.`
     (the real checks, named and counted by `tune::subjects`: 7 off macOS,
     no keys check; a new repo: `i'll check this repo: git and an
     AGENTS.md.`), `checking changes nothing. each fix i find comes back
     here as its own item, with the exact change, and you say yes or no to
     each one.`, dim `your agents keep working meanwhile. not now? type
     /setup whenever you want.` Key bar `↑↓ choose · 1-2 pick · esc back`
     (setup items never say `type to answer`).
   - **not now** (`2`, `×`, ctrl+x): the item goes, one dim row `– not now ·
     type /setup whenever you want`, never asked again.
   - **yes** (any option starting with `yes`): the checks run in code, each
     with its own timeout, 3 s for all: the terminal (name, version), cmd
     keys reaching bise (macOS; §16 "cmd+f": Ghostty's config is read; kitty
     passes them; WezTerm, iTerm2, Terminal.app, tmux get a note), truecolor
     (`COLORTERM`), the glyph widths (trusted for the terminals bise is
     tried in, and `BISE_ASCII`; not measured: a cursor probe would race the
     input reader), git and whether bise runs in a repo, `gh auth status`,
     AGENTS.md at the repo's root (BISE-232), the connectors' key
     (`MISTRAL_API_KEY`, found like the harness finds keys). They fold into
     one dim row, `▸ checked 8 things · 5 fine · 2 i can fix · 1 note`
     (nothing else: `checked 8 things · all fine`; a click or ctrl+o opens
     it: `✓` fine, `?` a fix, `–` a note, one line each). main says one
     line: `2 small fixes would help. each one waits in your inbox with the
     exact change. yes or no to each, whenever you want.` (one: `1 small fix
     would help. it waits in your inbox with the exact change. yes or no,
     whenever you want.`; none: `all good here. nothing to change.`).
   - **the fixes**, one item each, in the inbox, not opened, three at most:
     - `let cmd+v, cmd+f, cmd+k, cmd+a and cmd+↑↓ reach bise` · faint `Ghostty config · +8
       lines`: `right now Ghostty keeps these keys for itself. with these
       lines, cmd+v can paste a screenshot into your message (text still
       pastes as usual), cmd+f searches your history, cmd+k finds an
       agent by name, cmd+a selects all your message, and cmd+↑↓ jump to
       your message's start or end (with shift, they select to there).`
       (two keys: `these two keys`, the two reasons joined by `, and`; the
       four arrow lines count as one key, cmd+↑↓, and one reason) / `i'd add 8 lines
       to {path}:` (a new file: `i'd create {path} with 8 lines:`), the diff
       (`keybind = performable:super+v=paste_from_clipboard`, `keybind =
       super+f=unbind`, `keybind = super+k=unbind` (BISE-265), `keybind =
       super+a=unbind` (BISE-267), `keybind = super+arrow_up=unbind`,
       `super+arrow_down`, `super+shift+arrow_up`, `super+shift+arrow_down`
       (the same `=unbind`), only the
       missing ones; the title names only their keys; in the strip the long title is cut with `…` before the options), dim `i copy the file to
       config.bise-backup first. to undo: delete the 8 lines. Ghostty uses
       them after a reload (cmd+shift+,) or in a new window.` Options `yes,
       add them` (one line: `yes, add it`) / `no`.
     - `write a starter AGENTS.md` · faint `new file · 14 lines`, titled
       `write a starter AGENTS.md for this repo`: what AGENTS.md is for, `i'd
       create it at the root of the repo. nothing else changes:`, the whole
       file as a diff, dim `it's a plain file: edit it whenever, delete it to
       undo, commit it so your team's agents read it too.` Options `yes,
       write it` / `no`. Written by the model from package.json, Cargo.toml,
       the Makefile, pyproject/go.mod, the CI workflows and the last 12
       commit subjects (the only model call, Mistral with the connectors'
       key, 20 s; without one, a plain draft from the same facts); its item
       comes when the text is ready.
     - `turn on web search and the other connectors` · faint `optional ·
       needs a Mistral key`, the strip's right side `⏎ paste it`: what the
       connectors are, that everything else works without them, `paste your
       key below and press ⏎. it goes in ~/.bise/auth.json, and only you can
       read it. to remove it later, delete it from that file.`, dim `no key
       yet? console.mistral.ai. not now: ctrl+x.` No options: the composer is
       a masked paste field (`•` per character, never in the history); key
       bar `paste your key · ⏎ save · ctrl+x not now · esc back`; `⏎` saves
       the key in auth.json like `login`.
     Nothing is written without a yes, and only these files: the terminal's
     config (a backup `<file>.bise-backup` first; an older backup is kept),
     a new AGENTS.md (never over one), auth.json. Each answer leaves one dim
     row: `✓ Ghostty config · 2 lines added, the old one in
     ~/…/config.bise-backup · reload Ghostty (cmd+shift+,) to use them`,
     `✓ AGENTS.md written · 14 lines · your agents read it from their next
     turn`, `✓ MISTRAL_API_KEY saved · the agents you start from now on can
     search the web`, `– Ghostty config unchanged · type /setup whenever you
     want`, `– no AGENTS.md · type /setup whenever you want`, `– no
     connectors key · type /setup whenever you want`; a paste that is not a
     key: `that doesn't look like a key: nothing saved` (the item stays).
   - `/setup` runs the checks and the offers again, any time (the offers
     still in the strip are replaced).
   - NO_COLOR: the glyphs carry it (`✓ ? –`); `BISE_ASCII=1`: `ok ? -`.

6. **The real first run, with one-time hints.** No tour. Each hint shows once,
   next to the thing, the first time it happens, and goes away when used:
   - first agent: `new: your agents. they work in the background. ⌥ 1 to look
     inside, esc to come back. →`
   - first run of level 3: `agents talk to each other. it stays dim: you can
     ignore it, or ▸ to read.`
   - first item in the inbox: `? this is your inbox. when an agent needs you,
     it waits here instead of interrupting you. ctrl+1 opens it, or click
     it. ↓` (without ctrl+digits: `… click it, or type /inbox. ↓`;
     `this is your inbox.` bold, the keys in the text color, the rest
     dim; BISE-249)
   - (**⚠** proposed) first steer: `✓ the agent got it · ✓✓ it read it.`

The onboarding runs once per user (a flag in the state directory); `/welcome`
replays it (**⚠** proposed command).

**Layout.** One content column for all steps: 64 columns (terminal width − 8 when narrower), horizontally centered. Welcome and theme center their lines inside it; the key and how-it-works are left-aligned inside it. Vertically, the block sits a bit above the middle: 2/5 of the free rows above it, 3/5 below; the step dots stay 2 rows above the bottom. **Emphasis** (a terminal has one font size, so "size" is weight, color and space): each step's first line is its title, bold, text color; then 2 blank rows; the body in text color, notes dim, 1 blank row between options or lines; then 2 blank rows and the key line. Key lines are read, so they are dim, never faint (§5), with the keys themselves in text color: `←→ switch · enter keep`. Options: the selected one `›` accent + name bold, the others indented 2, their sub-line dim and indented 2 more. Welcome: `hi, i'm bise` bold + `:*` accent bold; 1 blank row; the gloss, a block centered as a whole with its lines left-aligned inside (a meaning too wide wraps with a 3-column hanging indent): `bise` bold, `/beez/ · french, n.` and meanings 1-2 dim (not faint: they are read), `:*` accent, meaning 3 (what bise is) in text color; 1 blank row; the tagline in text color; 2 blank rows; `any key ↵` dim with `any key` in text. Where the terminal supports text sizing (kitty ≥ 0.40, OSC 66), `hi, i'm bise :*` is drawn at scale 2; elsewhere bold. Small terminals: height < 22 turns every 2 blank rows into 1; width < 50 makes the column width − 4.

## 16. Keys (final)

| Keys | Action | Change |
|---|---|---|
| `⏎` | send to the agent in view; during a turn, steer | — |
| `tab` | during a turn: queue the message for after it | shown above the composer (BISE-89) |
| `↑` in an empty composer | edit the newest queued message (then the history) | new (BISE-89) |
| `↑` / `↓` at the composer's first / last row | the sent prompts, newest first (50 per workspace, kept across launches) | kept on disk (BISE-120a) |
| `@name …` | direct message from main | — |
| `/` | the commands, then each argument of a command (`/theme` light · dark · auto, the agents of `/archive` `/rename` `/isolate` (the agent in view first), the archived ones of `/restore` (the one in view first), the inbox items of `/close` `/answer`, the versions of `/version` `/restart`, `/plugins` and its plugins): tab completes, ⏎ runs once nothing required is left | arguments new (BISE-117) |
| `ctrl+c` | interrupt; again (or idle) quit, agents keep running | — |
| `⌥ + 0…9` | go to main / agent N | now shown in the panel |
| `alt+↓` / `alt+↑` | select next / previous agent | `ctrl+k` / `ctrl+j` removed (BISE-302) |
| `cmd+k`, `ctrl+s`, `/switch [name]` | find an agent by name and open it (see "Switch agents" below) | new (BISE-265) |
| `cmd+a` | select the whole composer text; typing or a paste replaces it, backspace clears it. Only the composer: in the help, find or the agent palette it does nothing (no `a` typed), in the terminal panel it goes nowhere. Ghostty keeps cmd+a (its screen's select all) unless `keybind = super+a=unbind`, which the setup offers with the cmd+v, cmd+f and cmd+k lines | reaches the composer (BISE-267, user: « j'aimerais bien que Command A dans le composer, ça sélectionne tout le texte du composer. Actuellement, ça ne fait rien du tout. ») |
| `cmd+↑` / `cmd+↓` (and `ctrl+home` / `ctrl+end`) | the very start / end of the composer text, multi-line included, wherever the cursor is (no history recall at the edge); with shift, select from the cursor to there (typing replaces it). Ghostty keeps all four by default (`jump_to_prompt`, the shell's previous or next prompt; always performed, so `performable:` does not help) unless `keybind = super+arrow_up=unbind`, `super+arrow_down`, `super+shift+arrow_up`, `super+shift+arrow_down` (the same `=unbind`), which the setup offers with the cmd+v, cmd+f, cmd+k and cmd+a lines (the shell then loses Ghostty's cmd+↑↓ prompt jumps). kitty passes them; WezTerm and iTerm2 only when they speak the kitty keyboard protocol and have no binding of their own on them (not checked); Terminal.app and tmux never send cmd keys: ctrl+home / ctrl+end, or ↑ / ↓ row by row | reaches the composer (user: « si je fais commande + flèche du haut et commande + flèche du bas, mon curseur devrait aller tout au début ou tout à la fin du texte. Et si je fais shift en même temps, ça devrait sélectionner jusqu'au tout début ou tout à la fin du texte. ») |
| `⏎` on a selected agent | enter it | — |
| `space` | preview the selected agent; in the feed, toggle the selected item | feed toggle new |
| `D` | drop the selected agent (asks first) | — |
| `esc` | close selection; in an agent, back to main | — |
| `ctrl+1`…`ctrl+9`, a click on a row | open inbox item N (the strip's and the panel's numbers) in the item view, from the thread or another item (§12); without ctrl+digits from the terminal: a click, `/inbox` | new (BISE-302); `ctrl+g` and the inbox selected (`↑↓`, `1-9`, `⏎` on the rows) removed |
| `↑↓`, `⏎`, `1-9`, `←→`, `ctrl+n` / `ctrl+p`, `ctrl+x`, `esc`, `pgup` / `pgdn` | an item open: choose an option (empty composer), pick it, pick at once, previous / next item, close, back, scroll (§12) | `↑↓` chose to scroll before (BISE-248); `alt+r`, `ctrl+f` full screen, `ctrl+a` on an empty composer: removed (BISE-236) |
| `ctrl+o` | open or close everything folded (thinking, outputs, diffs, reports, runs, `▸ why`, your long messages) | was `ctrl+t` (removed, no alias); the `ctrl+o` shell is gone: the terminal panel is the one shell |
| `ctrl+f`, `cmd+f` | find in the history (main or the agent in view) in a small box over the history's top-right (BISE-297, designer; like an editor's or a browser's find): 3 rows, its top border on the history's first row, 1 column in from the history's right edge (left of the panel's rule), 40 columns (at least 24, never more than the history less 2), a rounded dim border on the raised grey (NO_COLOR: the border only), no title; inside ` ⌕ query▏ … 3 of 12 ` (`⌕` is `/` under `BISE_ASCII=1`), `find in {agent}` dim when empty, the counter right-aligned and dim (`3 of 12`, `12+` while older lines are not loaded, `no match` in the error red); the composer stays with its draft and no caret (the box has the keys; a click in the composer closes the box and takes them back), the divider stays `you → {agent}`; the box's row selects and copies (BISE-290), a paste goes to the query (one line); every edit searches again and goes to the newest match (your messages and the replies first, then the calls), 4 rows of context above it so it lands under the box (a match on screen but under the box counts as not seen; at the history's top, where the view cannot go up, the box goes to the history's bottom-right when it would cover the match); `⏎` / `↑` / `ctrl+f` older, `shift+⏎` / `↓` newer, wrapping with `back to the newest` / `back to the oldest` for 1.5 s; matches on the pill tint, the current one on the accent, bold (NO_COLOR: underlined / reversed); smart-case; a match hidden in a call's box, a `▸ n commands` fold, a report opens it while current, closes it after; thinking is not searched; `esc` closes, the view stays on the match, the keys go back to where they were; key bar `⏎ older   shift+⏎ newer   esc close`; held ctrl (BISE-277) shows the box's keys (`ctrl+f older`, `ctrl+w delete a word`, `ctrl+u clear`), not the composer's. `cmd+f` does what `ctrl+f` does when the terminal passes it (the terminal's own find takes it by default, see "cmd+f" below); once any cmd key has reached the app in the session, the help says `cmd+f ctrl+f` and holding cmd shows `cmd+f find` (the ctrl hints keep `ctrl+f find`, BISE-277) | new (BISE-237); was the emacs forward char (`→` does it) and the card full screen; cmd+f: BISE-241 |
| `ctrl+r` | voice: record, any key stops, then the clip is transcribed at once; the voice chip at the cursor meanwhile, you keep typing while it is transcribed (BISE-222) | batch, not live (BISE-130): the voice role's model, `[roles] voice` in config.toml; voice off with no setup that works: the voice picker (BISE-298) |
| ``ctrl+` `` | terminal panel | — |
| `ctrl+v` | paste an image | from the images work |
| hold `ctrl` alone (~150 ms, BISE-231) | the ctrl keys show where they act: a fold's `▸ 12 more lines` reads `▸ ctrl+o expand`, the inbox rows' numbers in accent (BISE-302; was the panel title `agents · ctrl+k/j select`), the key bar every ctrl key of the moment (`ctrl+c interrupt` first while the agent works; the divider's `ctrl+c interrupt` hint is gone, BISE-303), and every word the screen leaves out at rest in its own place (BISE-303: the header's counts, the panel's state words, the divider's ` working · 42s` and long context); key in accent, what it does dim; over cells already drawn, nothing moves; released or any other key: gone at once. Only in a terminal that confirms the kitty keyboard protocol's flags 8 + 16 (Ghostty, kitty, WezTerm…); tmux and the others: off | new (BISE-203) |
| hold `⌥` (option) alone (~150 ms, BISE-277) | the same for the ⌥ keys (user: « Est-ce qu'on pourrait le faire pour la touche option aussi? »): the panel's numbers read `⌥0` `⌥1`… over their ` 0` ` 1` (accent; ASCII: the number alone in accent), the panel title `agents · ⌥↑↓ select` on an empty composer, the key bar `⌥0-9 go to an agent   ⌥↑↓ select an agent` (more than one agent; ⌥↑↓ on an empty composer), `⌥←→ word   ⌥⌫ delete a word` (a draft), `⌥⏎ newline`; in the thread, an agent's view, the card view and the inbox selected. Option typing characters (a layout, `macos-option-as-alt = false`): the hints show only while ⌥ is held with no other key; `⌥c` = `ç` arrives without alt, the hints go at once and it types; option as alt: `⌥c` is a combo, the hints go and the composer's option layer types `ç`. ⌥ with another modifier: none | new (BISE-277) |
| hold `cmd` alone (~150 ms, BISE-277) | once a cmd key has reached the app in the session (the terminal passes them; cmd alone says nothing, e.g. cmd+tab), the same for the cmd keys: the panel title `agents · cmd+k find`, the key bar `cmd+k find an agent` (more than one agent), `cmd+f find` (a history), `cmd+c copy` (a selection; `cmd+x cut` in the composer), `cmd+a select all   cmd+←→ line start/end   cmd+⌫ delete to line start` (a draft), `cmd+v paste`; the keys Ghostty keeps by default (cmd+z, cmd+↑↓) only in the help. Before any cmd key: nothing | new (BISE-277) |
| `ctrl+z` | ~~cancel the last route~~ | **removed** |
| `typing` (composer) | zen: the edges fade while you type, back 5 s after your last key (or at once on ⏎, esc, a shortcut) | a row in /shortcuts (BISE-137) |
| `/help`, `/shortcuts` | both end with the `symbols` legend (§6): every glyph, its ASCII form under `BISE_ASCII=1`, a few words | new (BISE-137) |

**cmd+f** (BISE-241, user: « pour la recherche ça devrait être CMD+F et pas ctrl+F, c'est plus naturel »). Every macOS terminal keeps cmd+f for its own find, and there is no per-app binding: freeing it frees it in every tab. It reaches the app as SUPER+f under the kitty keyboard protocol. Per terminal:

- **Ghostty** (1.3; `~/Library/Application Support/com.mitchellh.ghostty/config`): `keybind = super+f=unbind`. `performable:` does not help here: `start_search` is always performable, so Ghostty would keep taking cmd+f. Without the app, cmd+f then does nothing (Ghostty sends no text for a cmd key on macOS); Ghostty's find stays in its menu (Edit › Find), and `keybind = super+alt+f=start_search` gives it a key back if you want one.
- **kitty**: cmd+f is not bound by default: it reaches the app as is. If your `kitty.conf` maps it, `map cmd+f` (mapped to nothing) passes it on.
- **WezTerm**: `enable_kitty_keyboard = true` and, in `config.keys`, `{ key = 'f', mods = 'SUPER', action = wezterm.action.DisableDefaultAssignment }`; WezTerm's search stays on ctrl+shift+f. Not tried.
- **iTerm2**: its Find menu owns cmd+f. Settings › Keys › Key Bindings, `+`: shortcut cmd+f, action "Send Escape Sequence", `[102;9u` (SUPER+f in the kitty encoding). It is sent in every session, a shell included (it shows as junk there); iTerm2's find stays in the menu. Not tried.
- **Terminal.app**: never sends cmd keys to the app: ctrl+f only.

**Switch agents** (BISE-265, user: « ce serait cool d'avoir un moyen de switcher en tapant le nom de l'agent, un genre de commande K […] qui me permet de chercher mes agents, tous mes agents? Et quand je sélectionne, ça m'ouvre l'agent. Ce serait en plus du option 1, 2, 3, 4 qu'on a déjà. »; designer's look). `cmd+k` where the terminal passes it (Ghostty: `keybind = super+k=unbind`, its own cmd+k clears the screen; the setup offers the line with the cmd+v and cmd+f ones), `ctrl+s` in any terminal (ctrl+k is taken: kill to the line end, next agent), `/switch [name]` (the rest of the line is the query). The palette takes the composer pane like find: the divider reads `you → find an agent`, the query row sits where the composer's text sits (`type part of a name` dim when empty, `3 agents` dim on the right), the list grows the pane upward above it (1 blank row between), at most 12 rows, then it scrolls with the selection; the history keeps its 3 rows. The draft waits and comes back on esc. Rows: `▸` (accent) on the selected one, its name bold; the status mark (`:*` main, the gust while it works, `?` needs you, `✓` done…), `ψ` when it has its own worktree, the name (the matched chars bold in accent; NO_COLOR bold underlined), what it does now (else its objective; main: `your team lead`) dim and cut with `…`, its `⌥n` faint on the right (`alt+n` in ASCII; the palette teaches the direct key). Order: an empty query lists the live agents in the panel's order (main first) and no archived ones; a query lists the live ones it names, best first (the name's start, a word of it, anywhere in it, its initials `ap` agent-palette, its letters in order `dkmd` from 2 letters; then the query in its objective, note or last report, no highlight), an agent waiting on you first on a tie, then the panel's order; then a blank row, faint `earlier · read-only` and the archived ones it names (dim, their last report or objective, the most recent first within a rank). No match: `no agent called “zz” · esc closes`. Keys: typing, backspace, ctrl+w / option+backspace, ctrl+u edit the query (the selection back on the first row); `↑↓` (ctrl+p / ctrl+n, tab / shift+tab) choose, looping; `⏎` opens the agent's view as `⌥n` does (an archived one: its read-only history); `esc`, ctrl+c or the opening key again close; a click on a row opens it, a click elsewhere closes and does its job; pgup/pgdn still scroll the feed. Key bar `↑↓ choose   ⏎ open   esc close`. Help: `ctrl+s /switch` (`cmd+k ctrl+s /switch` once a cmd key reached the app), a tip `ctrl+s finds an agent by name, ⏎ opens it`; the ctrl hints show `ctrl+s find an agent` when there is more than one agent, the cmd hints `cmd+k find an agent` once a cmd key was seen (BISE-277).

## 17. Copy deck

Every string the UI shows, lowercase. Issues must use these exact strings.

| Where | Text |
|---|---|
| header, no agents | `no agents yet` |
| header counts | beside the panel `{folder} · # {n} in the inbox`; ctrl held or no panel `∿ {n} working · … {n} waiting · ? {n} needs you · ✓ {n} done` (BISE-303) |
| panel title | `agents` |
| panel, more rows | `+ {n} more` |
| panel, archived | `▸ {n} archived` |
| first run | `what's on your mind?` / `say it and keep talking. the work runs in the background, i'm always here.` / `try: "show me what you can do"` |
| inside an agent | `you're talking to {name} directly. main isn't in the loop. esc back to main.` |
| composer hints, main | `@ file · $ skills · / commands` (something in the inbox: `… · ctrl+1 inbox`; without ctrl+digits `… · /inbox`) (BISE-303) |
| composer hints, during a turn | `tab queue · ⏎ steer · ctrl+c interrupt` |
| composer placeholder (BISE-98) | `what's on your mind?` (to main) · `talk to {name} directly` (inside an agent) · `{name} is archived · /restore to talk to it` |
| divider (BISE-98, BISE-135, BISE-303) | `you → {name} · {model} · {effort} · {mode}` (working: ` · <gust>`; ctrl held ` working · 42s`) · on the right the context `18k · 2%` (ctrl held: `idle · 18k / 1M tokens · 2%`), or `↓ back to the bottom · end · {n} new lines` while scrolled up |
| run of level 3 | `▸ {n} messages between {k} agents` |
| thinking | `∴ thought for {s}s` |
| output | `▸ output · {n} lines` (+ ` · {k} failed` when known) |
| edit | `± edit {path} ✓ +{a} −{d}` |
| run of edits (BISE-304) | `▸ {n} files · {names} ✓ +{a} −{d}` · one file: `▸ {n} edits · {name}` · `+{k} more` |
| turn done (inside an agent) | `✓ turn done · {duration}` |
| card title | `? {name} needs you` |
| strip label | `inbox · 3 waiting for you · ctrl+1-3 open` (`ctrl+1 open`, `ctrl+1-2 open`; `click to open`; `/inbox opens it`) · rows ` 1 ? perf · …` |
| item view keys | `↑↓ choose · 1-2 pick · ←→ other items · type to answer in your words · esc back` · `↑↓ choose · ⏎ pick “{option}” · ←→ other items · esc back` · approval `… 1-3 pick · type a note to deny …` · typing `⏎ send as your answer · ctrl+n next item · esc back, draft kept` (§12) |
| item view divider | `you → ? {name} · your answer` |
| panel section, header | `inbox` · rows ` 1 ? perf  …` · `# 3 in the inbox` |
| /inbox | `the inbox is empty: nothing waits for you` |
| answered | `✓ {name} · you said {answer}` |
| direct reply | `@ {name} to you: {text}` |
| not delivered | `✗ not delivered: {name} stopped. ⏎ send again · esc drop` |
| no vision | `✗ {model} can't read images. pick a model that can (/model), or describe the screen in words.` |
| provider down | `✗ the model provider answered {code}. retrying in {s}s ({i} of {n}).` / `{names} are waiting on it; nothing is lost.` |
| hub lost | `○ hub disconnected · reconnecting…` |
| images strip | `attached · backspace on a chip removes it` |
| no undo (ctrl+z, or /cancel typed) | `no undo: an agent may already have acted. say the change to main instead ("no, v1 for docs").` |

| archive asks first (D) | `archive {name}? /restore brings it back. y / n` · hint `y archive · n or esc keep` |
| yes/no confirm | `answer y (yes) or n (no), then ⏎` · hint `y yes · n no · esc cancel` |
| archived agent in view | ⏎ on a plain message (it is not sent, it stays in the composer): `{name} is archived. /restore brings it back · esc → main` |
| /archive, /restore pickers | title `archive which agent?` / `restore which agent?` (dim, above the rows); rows `{name} · {status} · {objective}`, the agent in view first, `{name} · in view · …` (⏎ takes it); in an archived agent's view `/restore` shows once, in the placeholder: the divider's right says `archived` (dim), the key bar `esc back to main` |
| /theme | `theme: {mode}.` / `theme: {mode}. /theme auto, light or dark to change it.` / `theme: {mode}, for now: i couldn't save it ({err}).` / `/theme takes auto, light or dark.` |
| /clear, ctrl+l | `display cleared — scroll up to see the earlier lines again` |
| interrupt | `… · ctrl+c again to quit` |
| terminal panel | title ``terminal · ctrl+` hide`` (an editor opened by a file link, BISE-264: ``terminal · vim · ctrl+` hide``) · hint ``terminal: keys go to the shell · ctrl+` hide · wheel/shift+pgup scroll · drag select · drag the border resize`` · a drag selects (the history's tint), the release copies; cmd+c / ctrl+shift+c copy; a program that takes the mouse gets it, shift+drag selects |
| help footer | `type to filter · tab switch · esc close` |
| steer with nothing | `nothing to steer with: type the text after steer` |
| queued messages | ` › {text}…` (one per line, dim) · hint `queued · sent when this turn ends · ↑ edit` · panel row `· {n} queued` |
| connect failed | `couldn't connect: {err}` |
| voice | `✓ voice is on: {model}.` + `press ctrl+r and talk, any key stops. /voice turns it off.` / `voice is off. /voice turns it back on.` / `voice is off. /voice when you want it.` / `no speech detected` / the failure lines of BISE-298 (`✗ voice needs a key. /voice setup picks one.` …, the provider's words dim, `your recording is kept: ctrl+r retry`) / `voice model {model}: unknown provider '{p}' ('bise models' lists the voice ones)` / key bar `any key stop   esc cancel` while recording, `esc cancel` while the clip is sent (BISE-222) / `no audio input device found.` / `audio backend is unavailable: {err}` / `no audio detected from the microphone — check your terminal has mic access.` (+ ` grant access in System Settings → Privacy & Security → Microphone.`) |
| command descriptions | as in `/help` (lowercase, "agent"); `/agents`: `list the agents and what they do` |

Not built yet (a feature, not wording): `✓ turn done · {duration}` and the two `provider down` lines.

Onboarding strings: §15.

---

# Part III — Implementation plan

## 18. How main agents use this plan

1. Read Part I and II once, then [bise-issues.md](bise-issues.md).
2. Work **wave by wave**. Inside a wave, issues on different **tracks** can
   run in parallel: their files don't overlap (§20). Issues on the same track
   run one after the other, by one implementer.
3. Spawn **one implementer per track**, not per issue. Give it the list of its
   issues for the wave, this book and the tracker. It works them in order.
4. Before starting a track, check `sb list`: a task already working in the
   same files must land first. Don't interrupt it. (On 2026-09-28 the two
   known ones have landed: `archived-sidebar` in 85160ab, `screenshots` in
   4282501.)
5. The implementer updates **only its issue sections** in the tracker:
   `status`, `owner`, `commits`, `notes` (what was done, what was learned,
   what the next issue should know).
6. An issue is done when its "done when" list is true, the gates pass (§19),
   and its tracker section says `done` with the commits.
7. When an issue changes a contract (§21), stop and tell main: other tracks
   depend on it.

## 19. Rules for every implementer

- **Shared folder.** Never `git stash`, `git clean`, `git reset --hard`,
  `git checkout -- <paths>`, `git restore` on paths that aren't yours. Commit
  only your paths (`git add <paths>`, check `git diff --cached`). No push.
- **Stay in your files** (the "owns" list of the issue). If you need a change
  elsewhere, write it in your issue's notes and tell main; don't edit.
- **Gates** before `done`: `cargo build`, `cargo test --workspace`,
  `cargo clippy` without new warnings, `tests/run_all.sh`;
  `bend PROOF.bend` if a Bend file changed.
- **Visual check:** run the TUI in Ghostty (dark) and once in a light
  terminal; compare with the mockup named in the issue. Put what differs in
  the notes.
- **Strings** come from the copy deck (§17). A new string: add it to your
  notes, main updates the book.

## 20. Tracks and file ownership

| Track | Owns | Issues |
|---|---|---|
| **T · theme** | `rust/tui/src/theme.rs`; then `term.rs` + new `theme_detect.rs` | BISE-01, BISE-02 |
| **G · glyph audit** | no code; writes `docs/brand/glyph-audit.md` | BISE-03 |
| **H · hub protocol** | `rust/switchboard/src/core.rs`, `daemon.rs`, `transcript.rs`, `core_tests.rs`; in the TUI `wire.rs` (the `Ev` enum) and `sb.rs::parse_hub_line` | BISE-04 |
| **F · feed** | `render.rs`, `feed.rs`, `code.rs`, `markdown.rs`, `feedsel.rs`, `feed_render_tests.rs`; later `wire.rs` (steering lines) | BISE-10 … BISE-15 |
| **P · chrome** | `sb/panel.rs`, `ui.rs` | BISE-20, BISE-21, BISE-22 |
| **C · cards** | `sb/cards.rs` | BISE-30, BISE-31 |
| **K · keys & help** | `help.rs`, `commands.rs`, `input.rs`, the key arms of `sb.rs` | BISE-40, BISE-41, BISE-42 |
| **M · main's behavior** | `rust/switchboard/src/prompts.rs`, `router.rs` | BISE-50, BISE-51 |
| **O · onboarding** | new `onboarding.rs`, `hints.rs`; the entry in `run.rs`; hint hooks in `sb.rs` (event handling only) | BISE-60, BISE-61 |
| **I · images UI** | `attach.rs`, the composer chip drawing (`editor.rs` render path), the strip in `ui.rs` | BISE-70 |
| **S · sweeps** | any file, one sweep at a time, when no track is active in it | BISE-80 … BISE-83 |

Files touched by two tracks are never touched in the same wave: `wire.rs`
(H in wave 0, F in wave 2), `sb.rs` (H wave 0, K wave 1 key arm, O wave 2
hooks), `ui.rs` (P wave 1, I wave 2).

## 21. Contracts (frozen in wave 0)

**C1 · theme tokens** (BISE-01). `theme.rs` exposes roles, not colors:
`text()`, `dim()`, `faint()`, `accent()`, `error()`, `ok()`, `selection_bg()`,
`card_tint()`, `bg()` (the painted ground, BISE-92; every cell left at
`Color::Reset` gets it through the frame pass `theme::paint`), syntax roles,
and `set_mode(Mode::Light | Mode::Dark)`. The old
constants (`BRAND`, `ACCENT`, `INFO`, `WARN`, `HEAD`, `PANEL`, …) stay as
deprecated aliases until BISE-83, so no track breaks. Glyph constants for §6
(`G_YOU`, `G_MAIN`, `G_WORKING`, …) live there too.
Amendment (BISE-84, accepted by main): `G_*` stay `&'static str` constants
(Unicode, with the §6 fallbacks applied); `theme::glyph(G_X)` returns the
ASCII form when `BISE_ASCII=1` (`theme::ascii_mode()`); as a safety net,
`theme::asciify(buf)` rewrites the drawn buffer's cells that hold a table
glyph (only those) after each draw, only in ASCII mode. ASCII forms are one
cell wide (`+` done, `:*` stays). Hard-coded glyph literals migrate to
`glyph()` in BISE-83.

**C2 · hub line protocol v2** (BISE-04). Today the hub writes synthetic lines
`sb <kind> : <text>` into an agent's feed (`you`, `msg-in`, `card`,
`card-closed`, `route`, `spawn`, `direct`, `warn`). v2 keeps the same shape
and adds structured kinds; the TUI maps each to one level:

| kind | text | level | Ev |
|---|---|---|---|
| `msg-in` | `{from} {m_id} : {text}` (what the feed owner receives; v1) | 3 | `Ev::AgentMsg { from, to: "", text, level: 3, id: "m_3" }` |
| `msg` | `{from} → {to} : {text}` | 3 | `Ev::AgentMsg { from, to, text, level: 3, id: "" }` |
| `msg-you` | `{from} : {text}` (an agent writing to the user) | 2 | `Ev::AgentMsg { from, to: "you", text, level: 2, id: "" }` |
| `answered` | `{agent} : {question} : {answer} : {why}` (main answered for you) | 2 | `Ev::Answered { … }` |
| `route` / `spawn` | as today | 2 | as today |
| `card` | as today | 1 | as today |

- The hub also feeds `msg` lines for **messages between two other agents**
  into main's feed (today main only sees messages to main).
- Old kinds keep working (a v1 transcript still renders).
- Amendment (BISE-04, accepted by main): `Ev::AgentMsg` carries `id` (the message id, `m_3` for `msg-in`, empty otherwise). A ` : ` inside a field of `answered` is escaped as ` \: `. `sb send --why <text>` fills the `why` of `answered`.
- C2 amendment: history timestamp (BISE-85, accepted by main). A `history`
  page line is `{pos, line, ts?}`: `ts` is when the hub's transcript wrote
  the line (ms since the epoch), optional; a line without it still parses.
  The TUI reads it into `wire::HistLine { pos, line, ts: Option<u64> }`
  (`wire::parse_history`) and puts `Ev::TimeMark("hh:mm")` (local time)
  before a replayed line that comes 5 minutes or more after the one before
  it, as for live lines (§10).
- C2 amendment: line timestamp (BISE-271). A live `line` event carries
  `ts` too (the transcript's time of the line, ms since the epoch;
  optional, absent from an older hub): the lines a new TUI gets at hello
  are replayed at once, their pauses are in `ts`. The TUI marks pauses by
  `ts` (else by arrival) and keeps the end of each turn (`turn_done`) as
  `Ev::Ended(ts)`, never drawn: the hover of §10. A line the REPL replays
  (`history …`) has the replay's time: no mark, no end.
- C2 amendment: `undelivered` (BISE-86, accepted by main). When a message
  from the user cannot reach its agent (stopped, dropped, archived: the
  send fails with `recipient_unavailable`, or it was still queued when the
  agent stopped), the hub writes `sb undelivered : {name} : {text}` in the
  feed where the user wrote it (the agent's own, or the `via` view; fields
  escaped like `answered`). The `recipient_unavailable` notice stays. TUI:
  `Ev::Undelivered { name, text, open }`; your matching line (the text, or
  `@name text`) gets `Mark::Failed` (`✗`, error color), or comes back
  marked when the feed does not have it; the line reads
  `✗ not delivered: {name} stopped. ⏎ send again · esc drop` (§13, §17)
  while `open`: on an empty composer ⏎ sends it again, esc drops it.
- **⚠** Volume: with 30 agents this is many lines; the TUI folds them (§10),
  the hub must not drop them.

**C3 · steering marks** (BISE-15). The REPL's wire lines
`steering_received: <text>` and `steered: <text>` stop being `Ev::Info`; they
set a mark on the last `Ev::You` with the same text: `Mark::Received`,
`Mark::Read`.

**C4 · one-time hints** (BISE-61). A small store in the state directory
(`hints.json`: `{ "first_agent": true, … }`) and one call
`hints::once(app, Hint::FirstAgent)`.

## 22. Waves

```
wave 0  foundations      BISE-01 theme tokens ─┐   BISE-03 glyph audit   BISE-04 hub protocol v2
                                                │                              │
wave 1  parallel tracks  T: BISE-02 detection  F: BISE-10 → 11 → 12 → 13    P: BISE-20 → 21 → 22
                         C: BISE-30            K: BISE-40                    M: BISE-50 → 51
wave 2  builds on 1      F: BISE-14 (needs 04, 13) → BISE-15 (needs 04)
                         O: BISE-60 → 61 (61 needs 14, 20, 30)
                         I: BISE-70 (needs 22)
                         K: BISE-41 → 42 (needs 12 for the new keys)
                         C: BISE-31 (needs the open question answered)
wave 3  sweeps           BISE-80 vocabulary · BISE-81 lowercase + copy deck
                         BISE-82 visual QA · BISE-83 remove deprecated theme aliases
```

A good first day: wave 0 with three implementers (T, G, H), then wave 1 with
up to six (T, F, P, C, K, M).

## 23. Later (not scheduled)

- Draw images in the terminal (kitty / iTerm2 protocols).
- An approval mode for risky commands (then onboarding step 4 changes, §15).
- Cost in $ in the header.
- `✓` / `✓✓` on level-3 notifications (`notification_received` /
  `notification_delivered`).
- Renaming the binary and commands to `bise` (a product decision for main).
- A "filter this agent" view that keeps arrival order.
