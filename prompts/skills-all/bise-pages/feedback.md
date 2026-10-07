# bise pages: feedback into work

Read this for bug reports or feedback from Slack, Linear or mail that agents can fix
("Benjamin's bug reports on Slack: handle them").

- Fast, like an inbox: publish each report as an item as soon as you have read it, before any
  diagnosis: who · where · when, the quote, its link, and `looking…` where the cause goes. The
  first items are up within about 45 seconds of the ask (callout `3 of 9 read`). Only then read
  the code, the logs or the repo, and fill in each item's cause and fix in later versions (same
  item ids). Never read every thread, or diagnose anything, before the first items.
- The reports are ALWAYS one `review` block, never headings with prose and messages: without
  it there is no `start an agent`, no answer, no skip. Even a report that needs no code change
  is an item (say so in it: he answers or skips it).
- A `review` with `data-verb="start"`, one item per report, its id the ticket's (`ENG-412`) or
  the message's (`s-101`): a first `<p>` `who · where · when`, what they said in a
  `<blockquote>` (linked in the sources), then what you think it is in one or two sentences.
- The kit gives each item `start an agent`, `answer Benjamin` (the reporter's first name) and
  `skip`. "start an agent" is a note to main, who starts the agent with the item as its brief.
- Then the item carries the agent's name: `<li data-id="s-101" data-agent="fix-install-path">`
  (review and checklist items). The kit shows its line live: `∿ fix install path working ·
  reading the installer`, `? … needs you`, `✓ … done`.
- When the agent is done, republish the item with its answer (`fixed in 0.4.2`, the PR) and its
  reply right under it: move the item into its own `review` under an `h2` (`fixed`), followed by
  a `message` block whose `data-to` says the place (`Slack · reply to Benjamin in
  #bise-feedback`); the open items stay in another `review` (`still open`). The message leaves
  only on the user's word: `data-verb="send"` when you can post it yourself (the buttons say
  `send` / `skip`), else `data-open` with the thread's link (`copy` and `open in Slack`).
  `answer Benjamin` on an item: draft that reply now, the same way.
- See `examples/feedback-bridge.html`.
