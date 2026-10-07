# bise pages: what bise keeps about the user

Read this for "what do you know about me?", "show me my taste rules". Main publishes one page,
id `about-you`, from `sb taste` (the taste rules) and `sb people` (who is who), when they have
any:
- a `heading` (`what i keep about you`, meta line: `5 taste rules · 4 people · edit or strike any`);
- `## your taste`: a `review` with `data-verb="keep"`, one item per rule, its `data-id` the
  rule's slug (`no-emoji`): a first `<p>` saying where it came from (`from your notes on the
  launch recap · 12 may`), then the rule in a `<p>`, as the user would say it. The kit gives each
  `keep` / `strike`, the words editable in place; each change comes back as a note;
- `## who is who`: the same, one item per person (`data-id` their slug): `<p>Lélio Martin</p>`
  then `<p>your manager. the weekly update goes to him on fridays.</p>`;
- a `callout` (`note`): `nothing else is kept: no mail, no messages. strike a line and it's gone.`
- no `sources` (it is the user's own words) and no `question`.

The first publish is main's; after that the hub keeps the page fresh itself. Notes on it go to
main, who changes the files only through `sb taste` and `sb people`, never by editing them: a
struck rule `sb taste remove <n>` (numbers from `sb taste`), an edited one `sb taste remove <n>`
then `sb taste add "<the user's words>" --from "your note on about-you"`; a person
`sb people set <name> "<who>"` or `sb people remove <name>`. The hub redraws the page after each
change; mark the notes done with `sb page publish … --notes-done` only if you republish yourself,
else answer in one line. Nothing kept is ever shown elsewhere without the user asking. See
`examples/about-you.html`.
