# bise.dev/docs drafts

Pages of bise.dev/docs that wait for the release that ships their feature.
Everything under `site/` goes live at the next deploy (the raw `.md` too),
so a page about an unreleased feature waits here.

To publish one:

1. move it: `git mv docs/site-drafts/<page>.md site/docs/<page>.md`
2. build: `python3 site/docs/build.py` (it adds the page to the sidebar
   and the sitemap, and turns on the `<!-- if <page> -->` blocks of the
   other pages that mention it)
3. land, and ask designer to deploy.

| draft | waits for |
|---|---|
| `subscriptions.md` | the release with ChatGPT sign-in, OpenRouter sign-in and the coding plans (subs-lead) |
