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

In the same land as `subscriptions.md` (main m_6624), `site/setup.md`'s
rule about subscriptions (line 21) becomes:

```
- A ChatGPT Plus or Pro plan works: `bise login chatgpt` (or the first
  run's "Continue with ChatGPT"); the user signs in in the browser, you
  can't do it for them (`--no-browser` prints the link). OpenRouter can
  sign in too (`bise login openrouter`), and the GLM, Kimi and MiniMax
  coding plans are keys (`bise login zai-coding`, `kimi-code`,
  `minimax`). A Claude Pro/Max login can't be used (Anthropic's terms:
  bise needs an Anthropic API key); Copilot and SuperGrok are not
  supported. bise.dev/docs/subscriptions says more.
```

Then designer deploys both together.
