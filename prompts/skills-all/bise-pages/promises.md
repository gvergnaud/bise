# bise pages: his promises

Read this when the morning wake says "refresh his promises", or the user asks bise to keep track
of what he said he'd do. One page a week, `promises-YYYY-wNN` (`promises-2026-w41`), titled
`your promises`; no ask needed once it's on.

**What a promise is:** something he said he would do, with or without a date, in his own words:
a Granola note of a meeting (`Gabriel: i'll send the SSO timeline by friday`), his sent mail, his
Slack messages (`i'll look at it tomorrow`), and the answers he sent from a mentions page (an item
with its went line: `i'll send an invite` is a promise, its source that item). Never what others
promised him, never a vague "let's talk".

**The page:** a heading (`your promises`, meta line `3 open · 1 overdue · 1 kept this week`),
then one `checklist` (never `data-plan`: these rows never make cards), one row per open promise:
`<li data-id="p-sso" data-who="yours" data-due="2026-05-16">send Hélène the SSO timeline in
writing<sup>1</sup></li>`, the kit showing `you · friday` or `late · 2 days`. A promise without
a date gets the date it implies (`tomorrow`, `this week`: friday), else none. Under the list, the
source of each in `sources`, quoted in its link's name (`"i'll send it friday" · Granola, Q3
renewal`).

**A draft ready when it's due:** a promise due today or overdue gets its draft in its own `email`
or `message` block (it waits for his word, `leaving.md`), and its row points at it
(`data-draft`). Read `sb people` and the thread before you ask him for an address or a
name. A doubt left (the address, a cc) is a page-only `question`, `data-card="none"`, its row
pointing at it (`data-question`): it waits on the page and in the morning, never a card (the
lint refuses a question without it on this page).

**Each refresh** (same id all week): read what is new since the last one; add new promises; tick
a row (`data-done`, its text ending `· kept: your mail of 14:02`) when his own sent mail or
message shows it done, or he ticked it; fold kept ones into one line at the end (`kept this
week: 2`). Nothing new: don't publish. A new week: a new page, the open rows carried over with
their ids.

**No card, ever:** an overdue promise leads the morning page's `needs you` with its draft
(`sb page waiting` lists overdue rows first). See `examples/promises.html`.
