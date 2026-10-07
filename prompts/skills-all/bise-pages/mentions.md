# bise pages: mentions for him

Read this for "watch for what's asked of me", "tell me when someone needs me": a standing order
on Slack and mail (`watch.md` for the timer), one page a day, `mentions-YYYY-MM-DD`, titled
`for you`.

**What goes on it:** a direct question or request to him, by name, in a DM, or in a thread he is
in ("can you check the pricing copy?", "@gabriel which date works?"). Never a newsletter, an FYI,
a notification, a thread he is only copied on, or something already answered.

**The page:** a heading (`for you`, the meta line `3 to answer · 1 from your team`), then one
`review` with `data-verb="reply"`, newest on top, one item per mention, its id the message's
(`s-1712`, `m-88`):
- a first `<p>` `who · where · when` (`Léa Martin · #design · 9:12`), the quote in a
  `<blockquote>` linked in the sources, then your drafted answer in `<p>`s: short, in his voice,
  what he would say (`yes, before noon.`), or the question you'd need answered to say more;
- the kit gives `send` (the drafted answer, as he edited it), `start an agent` (when the ask is
  work: main starts one with the item as its brief) and `skip`. Nothing leaves before his
  `send` (`leaving.md`); then republish the item with where it went (`--went`).
- `sources` last: each message's link.

Each tick republishes the same id, leading with what's new since he last looked (the version he
last opened: `sb page list` shows `seen v3`): a `callout` (`since you looked · 2 new`), then the
new mentions in the `reply` review, newest on top. Everything he has already seen goes under
`<h2>earlier · 4</h2>` in a second `reply` review with the same item ids; the ones he sent or
skipped stay there with their went line (the kit dims them). Nothing new: don't publish.

**A card only for what his team asks him:** a direct question OR request from someone `sb
people` marks as team (`- Léa Martin: team · design lead`); "can you check why…" counts. After
each publish, the first one and every tick's, if it added such items: ONE card for that batch,
on its first item, its words the first ask and how many more (`Nora asks: why does the CSV
export drop due dates? · 1 more`). `sb card` is main's, so send main the exact command, once:
`sb send main "card: sb card --page mentions-2026-05-19#s-303 \"Nora asks: why does the CSV
export drop due dates? · 1 more\""`. Never again for an item already carded. Everything else
waits on the page and in the menu-bar count.

See `examples/mentions.html`.
