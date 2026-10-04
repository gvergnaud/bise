# Glyph audit (BISE-03)

Checks every glyph of the bise book §6, plus `↪ ▸ ▾ ┃ │ ─`, for:

1. **Width 1**: with `unicode-width` (the crate the TUI uses) and measured in
   the terminals installed on this Mac.
2. **Font coverage**: does the font have its own glyph for the code point?

Date: 2026-09-28. Mac: macOS, Ghostty 1.3.1, Terminal.app, iTerm2.

## Summary

- **Width: no glyph fails.** Every glyph is 1 cell (`:*` is 2, as it should
  be) in `unicode-width` 0.2.0 (ratatui) and 0.1.14 (vt100, unicode-truncate).
  Ghostty and Terminal.app also give 1 cell for every glyph (cursor-position
  measurement, see Method).
- **Fonts: 17 of 33 glyphs are missing from at least one of the five spec
  fonts.** When a glyph is missing, the terminal draws it with a fallback font
  (Apple Symbols, STIX, Menlo, Apple Color Emoji...). It is still 1 cell wide,
  but the size, weight and baseline can differ from the rest of the text.
  `✉` and `↪` can also come out as color emoji.
- **Worst cases:** `∿` and `⧗` are in none of the six fonts checked. `⟳`
  and `⎇` are only in Fira Code. `✉`, `⇄`, `↻` and `♡` are only in Menlo
  and MesloLGS NF.
- **Ambiguous width:** 16 glyphs are East Asian *Ambiguous* (EAW = A, including
  the box-drawing lines); 15 of them become 2 cells in `width_cjk()` (`λ` stays 1). A terminal set to "ambiguous = wide" (option in
  iTerm2, Terminal.app, WezTerm; common with CJK locales) draws them 2 cells
  wide, but ratatui still counts 1, so the layout breaks. No glyph choice fixes
  this, because `│ ─ ┃` are ambiguous too. Rule: Switchboard needs
  ambiguous width = narrow (the default everywhere; the user's iTerm2 has
  `Ambiguous Double Width = 0`).

## Table: glyph × font / terminal

Legend: **EAW** East Asian Width (N neutral, Na narrow, A ambiguous);
**uw** `unicode-width` 0.2.0 `width()`; **uw cjk** `width_cjk()` (what an
"ambiguous = wide" terminal does); font columns: ✓ the font has the glyph,
**✗** missing, so a fallback font draws it; terminal columns: cells the
terminal actually moved the cursor by. `⚠emoji`: the code point has
`Emoji=Yes` (text by default), so a fallback may choose Apple Color Emoji.

| glyph | use | code point | EAW | uw | uw cjk | SF Mono | Menlo | JetBrains Mono | Fira Code | Cascadia Code | MesloLGS NF | Ghostty | Terminal.app |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `›` | you, composer | U+203A | N | 1 | 1 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `:*` | main | U+003A U+002A | Na/Na | 2 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 2 | 2 |
| `◇` | brief | U+25C7 | A | 1 | 2 | **✗** | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `∴` | thinking | U+2234 | A | 1 | 2 | **✗** | **✗** | ✓ | ✓ | **✗** | **✗** | 1 | 1 |
| `$` | bash call | U+0024 | Na | 1 | 1 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `λ` | TS call | U+03BB | A | 1 | 1 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `↳` | sub-call | U+21B3 | N | 1 | 1 | **✗** | ✓ | **✗** | ✓ | **✗** | ✓ | 1 | 1 |
| `±` | file edit | U+00B1 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `✉` | message | U+2709 ⚠emoji | N | 1 | 1 | **✗** | ✓ | **✗** | **✗** | **✗** | ✓ | 1 | 1 |
| `▣` | image | U+25A3 | A | 1 | 2 | **✗** | ✓ | **✗** | ✓ | ✓ | ✓ | 1 | 1 |
| `?` | card / needs you | U+003F | Na | 1 | 1 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `⟳` | compaction running | U+27F3 | N | 1 | 1 | **✗** | **✗** | **✗** | ✓ | **✗** | **✗** | 1 | 1 |
| `≡` | compaction summary | U+2261 | A | 1 | 2 | **✗** | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `▲` | interrupted | U+25B2 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `·` | starting / sending | U+00B7 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `∿` | working | U+223F | N | 1 | 1 | **✗** | **✗** | **✗** | **✗** | **✗** | **✗** | 1 | 1 |
| `…` | waiting | U+2026 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `♡` | done | U+2661 | A | 1 | 2 | **✗** | ✓ | **✗** | **✗** | **✗** | ✓ | 1 | 1 |
| `✗` | failed | U+2717 | N | 1 | 1 | ✓ | ✓ | ✓ | **✗** | **✗** | ✓ | 1 | 1 |
| `○` | idle | U+25CB | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `–` | stopped | U+2013 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `✓` | got it | U+2713 | N | 1 | 1 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `•` | unread | U+2022 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `⎇` | worktree | U+2387 | N | 1 | 1 | **✗** | **✗** | **✗** | ✓ | **✗** | **✗** | 1 | 1 |
| `⇄` | overlap | U+21C4 | N | 1 | 1 | **✗** | ✓ | **✗** | **✗** | **✗** | ✓ | 1 | 1 |
| `↻` | restart failed | U+21BB | N | 1 | 1 | **✗** | ✓ | **✗** | **✗** | **✗** | ✓ | 1 | 1 |
| `⧗` | building / trial | U+29D7 | N | 1 | 1 | **✗** | **✗** | **✗** | **✗** | **✗** | **✗** | 1 | 1 |
| `▸` | closed | U+25B8 | N | 1 | 1 | ✓ | ✓ | ✓ | **✗** | ✓ | ✓ | 1 | 1 |
| `▾` | open | U+25BE | N | 1 | 1 | ✓ | ✓ | ✓ | **✗** | ✓ | ✓ | 1 | 1 |
| `↪` | wrap marker | U+21AA ⚠emoji | N | 1 | 1 | **✗** | ✓ | ✓ | ✓ | **✗** | ✓ | 1 | 1 |
| `┃` | rail | U+2503 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `│` | rule | U+2502 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |
| `─` | rule | U+2500 | A | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 1 | 1 |

Not in the table (no data on this Mac):

| terminal | status |
|---|---|
| iTerm2 | **not measured.** It launched but did not run the probe script (`open -a iTerm script.command` does nothing on a fresh start, and AppleScript would need an Automation permission). Config read: font MesloLGS NF 13 (column above), ambiguous width narrow. |
| kitty | **not checked**: not installed. |
| WezTerm | **not checked**: not installed. |
| Screenshots (all terminals) | **not possible**: `screencapture` fails ("could not create image from window/rect"), because this process has no Screen Recording permission. The widths come from the cursor measurement, not from pictures. The glyph look must be checked by hand (BISE-82). |

Notes on the fonts:

- **SF Mono**: `SF-Mono-Regular.otf` and `SFMono-Terminal.ttf` in
  Terminal.app's bundle; the same coverage. This is Terminal.app's default
  font, so Terminal.app draws 14 glyphs with a fallback font.
- **JetBrains Mono** 2.304 (official release, downloaded to /tmp). The
  installed *JetBrains Mono Nerd Font Mono* gives the same result.
- **Fira Code** 6.2 and **Cascadia Code** 2407.24 are not installed on this
  Mac. I checked the official release files, downloaded to /tmp.
- **MesloLGS NF**: the user's iTerm2 font, added because it is a real case.
- **The user's Ghostty** asks for `font-family = FiraCode`, but Fira Code is
  not installed (`ghostty +list-fonts` does not list it). So Ghostty uses its
  built-in font, JetBrains Mono (from Ghostty's defaults, not seen on screen).
  In practice the JetBrains Mono column is what the user sees today.
- Box drawing `┃ │ ─` extends past the advance width in SF Mono and Cascadia.
  This is normal (the lines join across cells). Ghostty, iTerm2, kitty and
  WezTerm draw box-drawing glyphs themselves anyway. Fira Code's `⟳` extends
  slightly past its cell (-47..1355 for a 1200 advance).

## Fallback list

One proposal per failing glyph. "Replace with" is a single-cell character
that all six fonts have (or all but one, marked), so no fallback font is
needed. "ASCII" is the last resort for a future plain-ASCII mode.

| glyph | use | missing in | replace with | ASCII | why |
|---|---|---|---|---|---|
| `∿` | working | all six | `~` | `~` | a breeze stays a breeze; ASCII, so it works everywhere, and it can still pulse |
| `⧗` | building / on trial | all six | `Δ` | `^` | "a change being tried"; in all fonts |
| `⟳` | compaction running | all but Fira | `Σ` (dim, pulsing) | `=` | a summary being made; in all fonts |
| `≡` | compaction summary | SF Mono | `Σ` (dim, still) | `=` | same glyph as running; the state shows in the pulse, like the status glyphs. (Or keep `≡`: only SF Mono lacks it.) |
| `⎇` | worktree | all but Fira | `⌥` (all but Cascadia) | `Y` | it looks like a branch; SF Mono, Menlo, JetBrains, Fira, Meslo have it |
| `✉` | message | SF, JetBrains, Fira, Cascadia; can become emoji | `@` | `@` | ASCII, never emoji, and it reads as "addressed to" |
| `⇄` | overlap | SF, JetBrains, Fira, Cascadia | `↔` | `=` | in all fonts (it has Emoji=Yes, but no font lacks it, so no emoji fallback) |
| `↻` | restart failed | SF, JetBrains, Fira, Cascadia | `!` (error color) | `!` | the error color already shows "failed"; `↺` has the same coverage problem |
| `♡` | done | SF, JetBrains, Fira, Cascadia | `✓` (alt. `●`) | `+` | `✓` is in all fonts. It is also the "got it" message mark, but that mark sits on your message, not in the status column. `♡` is a brand choice: keeping it means a fallback-font heart in 4 of 6 fonts. **Gabriel decides.** |
| `∴` | thinking | SF, Menlo, Cascadia, Meslo | `≈` (dim) | `.` | in all fonts; "turning it over". (JetBrains and Fira have `∴`, so today's Ghostty draws it fine.) |
| `↳` | sub-call | SF, JetBrains, Cascadia | `└` | `-` | box drawing: all fonts have it, and terminals draw it to fit the cell exactly |
| `▣` | image | SF, JetBrains | `■` | `#` | in all fonts; it is an accent chip anyway |
| `◇` | brief | SF | `◊` | `+` | in all fonts; almost the same shape |
| `✗` | failed | Fira, Cascadia | `×` | `x` | in all fonts; pairs with `✓` |
| `↪` | wrap marker | SF, Cascadia; can become emoji | `»` (faint) | `>` | in all fonts, never emoji |
| `◷` | a scheduled task (sb every): its ◷ lines, the next run on an agent's row, /scheduled (site/m/timers) | new, not audited in the six fonts | keep (one cell; `⏱` is an emoji in most fonts and takes two) | `@` | a clock face, faint; `@` reads "at a time" |
| `▸` / `▾` | closed / open | Fira | keep | `>` / `v` | only Fira lacks them; `▶ ▼` are in all fonts but look too heavy. Keep them. |

No change needed: `› :* $ λ ± ? ▲ · … ○ – ✓ ✓✓ • ┃ │ ─`.

Check before adopting the replacements: `≈ └ ■ Σ ● × ↔ Δ` are also EAW = A
(like the current glyphs); `◊ ✓ ⌥ » @ ~ !` are not.

## Method (to redo the audit)

Everything is in `/tmp/g-glyphs` (throwaway):

- **Width:** a small Rust program with `unicode-width = "=0.2.0"` and
  `"=0.1.14"` (the two versions in `rust/Cargo.lock`) that prints
  `width()` and `width_cjk()` for each glyph.
- **Fonts:** Python + fontTools: the glyph is in `getBestCmap()`, its advance
  equals the advance of `0`, and its bounds stay inside the cell. Missing
  glyphs: `fc-list ":charset=<hex>" family` lists the fonts macOS can fall
  back to.
- **Terminals:** `probe.py` switches the tty to raw mode, prints each glyph
  between two `ESC[6n` cursor-position reports, and writes the column
  difference to a file. Ghostty: run `ghostty -e python3 probe.py` (my own
  process, killed after). Terminal.app: `open -a Terminal probe.command`
  (it was not running; I killed its PID after).
