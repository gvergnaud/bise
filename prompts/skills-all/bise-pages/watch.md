# bise pages: a standing order, one page kept up to date

Read this for "watch the launch for me", "tell me when the PR is reviewed", "a page every
morning": one page you keep current, not a new page each time.

- **On a timer, never by sleeping:** `sb every 10m "<what to check>" --page <id> --until <time>`,
  the line written as the instruction to your future self (`check the launch on HN, X, Reddit and
  Bluesky; republish launch-watch with what's new`). `--page` ties the timer to the page: its
  frame says `watching · checked 3 min ago · until tomorrow 18:00` and has `stop` (the user's
  stop ends the timer without you). It wakes you with `timer #3 (every 10m, …): <that line>`.
  `--until` or `--times 6` when the user gave an end. `sb every` lists the timers;
  `sb every --stop 3` stops one when the thing is over.
- **Each check republishes the same page id:** what is new on top (a `callout` saying `since
  14:10` and what came), the items still to handle under it, the handled ones gone or folded into
  one line (`12 answered earlier`). Keep each item's `data-id` (the comment's id, the PR's
  number) so the user's notes stay on it. Nothing new: don't publish.
- **A card**, once the item is published: `sb card` is main's, so send main the exact command,
  `sb send main "card: sb card --page <id>#<item id> \"<question>\""` (the card opens the page
  at that item), only for what the user asked to be told about, in their
  words ("tell me if Simon posts"); everything else waits on the page. Numbers never get a card.

## The launch: comments on Hacker News, Reddit, X, Bluesky

- The numbers in the heading's meta line, one number per fact (`HN #7 · 412 points · 1,204
  stars · 38 comments`).
- One `review` per group, each under an `h2` (`questions`, `bugs`, `praise`), with
  `data-verb="open-reply"`: these places are read-only for bise. One item per comment that needs
  an answer: `<li data-id="hn-4242" data-reply="<the comment's https link>">`, then
  `<p>who · where · when</p>`, the comment in a `<blockquote>`, your drafted reply in `<p>`s.
- The kit's button says where it goes (`open on X`), never `send`; it opens the thread with the
  reply ready and the item says what is left to do: `opened on X · your reply is in the box`
  or `copied · paste it in the thread`. bise never posts, the user does.
- See `examples/launch-watch.html`.

## The morning page

A daily order of its own (a new page each day, `morning-YYYY-MM-DD`): read `morning.md`.
