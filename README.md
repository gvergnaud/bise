<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/hero-dark.svg">
    <img src="docs/brand/readme/hero-light.svg" width="860" alt="bise :* · a multi-agent harness, made for humans.">
  </picture>
</p>

<p align="center">
  <b>bise</b> /beez/ · french, n.<br>
  1. a quick kiss on the cheek :*<br>
  2. a brisk north wind<br>
  3. a terminal where multi-agent coding is painless
</p>

<p align="center">
  <a href="https://bise.dev">bise.dev</a> ·
  <a href="#install">install</a> ·
  <a href="#features">features</a> ·
  <a href="https://bise.dev/docs/">docs</a> ·
  <a href="https://bise.dev/book/">brand book</a>
</p>

<br>

**meet your team lead. you stay in flow, it runs the agents.**

bise is a terminal app for multi-agent coding. there's one thread per repo. in it you talk to **main**, an agent that acts as your team lead: it splits your requests into jobs, starts an agent when a job needs one, answers their routine questions, and only comes back to you with the decisions that are yours.

no sessions to juggle and no workflow to set up: the orchestration is built in, and it has opinions.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/demo-b-dark.svg">
    <img src="docs/brand/readme/demo-b-light.svg" width="860" alt="a bise session in the terminal: you send ideas one after the other, main starts an agent for each, they ship.">
  </picture>
</p>

more demos and the docs on [bise.dev](https://bise.dev).

## install

```sh
curl -fsSL bise.dev/install | sh
```

then start it in the repo you want to work on:

```sh
cd ~/your-repo
bise
```

the first run walks you through a theme and a model key. `bise doctor` checks your install, your keys and the running hubs.

switching from Claude Code or Codex? paste this into it:

```text
read https://bise.dev/setup.md and set bise up for me
```

your agent installs bise and brings over what you already have: your API key, your model, your `CLAUDE.md`, skills and MCP servers. it shows you one plan and waits for your yes. it never prints a key. if your agent can't open links, paste [the full prompt](https://bise.dev/setup) instead. a Claude Pro/Max or ChatGPT subscription isn't an API key: bise needs a key.

using LiteLLM or another gateway, or any OpenAI-compatible base URL? see [custom providers](docs/custom-providers.md).

> [!NOTE]
> bise is pre-release. **macOS only for now** (Apple silicon and Intel). linux is next.
> bring your own key: Anthropic, OpenAI, Mistral and more.

## how it works

- **one thread per repo.** you always talk to the same thread. it never ends: when it gets long, bise compacts it and keeps going.
- **main is the team lead.** it answers what it can, starts an agent when a job needs one, follows up, and picks up work that stopped half-way.
- **agents work in the background,** in your checkout. they message each other before they touch the same files. one makes a git worktree only when it needs its own copy, and cleans it up after.
- **the inbox holds the decisions that need you.** only the real ones reach you. they wait above your message (`ctrl+1`, or a click), never in the middle of your sentence.

four words to learn: you, main, agents, inbox.

## features

### orchestration

#### long-running goals

give main a large goal. it splits it into jobs, runs agents in parallel, restarts any that stop on an error, and keeps going until the goal is done.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/resume-dark.svg"><img src="docs/brand/readme/feat/resume-light.svg" width="680" alt="you give main one big goal. it runs three agents at a time, starts again the one that stops on an error, and keeps going until all 12 pages are done."></picture>

#### automatic worktrees

agents share your checkout by default. when one needs an isolated copy (a clean build, a risky change), it creates a git worktree, works there, and removes it when it's done. no branches to name, no folders to clean up.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/worktree-dark.svg"><img src="docs/brand/readme/feat/worktree-light.svg" width="680" alt="dark-mode and cookies share your folder; perf gets its own worktree for a clean build, finishes, and the worktree is cleaned up."></picture>

### focus

#### main is always available

main hands the heavy work to agents, so it's never busy. ask it anything while five agents run: it listens and answers right away, and nothing you send interrupts a job.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/talk-dark.svg"><img src="docs/brand/readme/feat/talk-light.svg" width="680" alt="three agents are working. you ask main what perf is doing and it answers right away; you add a job, it starts one more agent and is still there."></picture>

#### zen mode

while you type, the agents panel, the counters and the agents' chatter dim. they come back when you send. messages between agents stay folded; `ctrl+o` expands them.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/zen-dark.svg"><img src="docs/brand/readme/feat/zen-light.svg" width="680" alt="you start typing and everything else fades: the agents, the counts. perf finishes meanwhile. you send, and it all comes back."></picture>

### voice

#### voice to voice

press `ctrl+r` twice for voice mode and just talk to main. it answers out loud while the agents keep working. `space` sends right away, `esc` leaves, and `/voice` picks the model, the voice and the language. the face is drawn in the terminal: it smiles while it listens, turns a * while it thinks, and blows you a kiss when it's done.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/voicemode-dark.svg"><img src="docs/brand/readme/feat/voicemode-light.svg" width="680" alt="you press ctrl+r twice and ask main to make the pricing page less busy. the face listens, thinks, then talks: main says pricing-page will cut it to three plans. it ends with a kiss."></picture>

#### dictation

set it up with `/voice`, then press `ctrl+r` and talk. your words land in the composer.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/voice-dark.svg"><img src="docs/brand/readme/feat/voice-light.svg" width="680" alt="you press ctrl+r and say it; a level meter moves while you talk; your words land in the composer as text, and you send them."></picture>

### coordination

#### main answers routine questions

agents ask main, not you. main answers what it can, the way you would, and says why. only the decisions that are yours reach your inbox.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/card-dark.svg"><img src="docs/brand/readme/feat/card-light.svg" width="680" alt="three agents ask main a question. main answers two of them itself, the way you would, and passes you the one decision that is yours."></picture>

#### agent-to-agent messages

agents message each other directly: questions, hand-offs, who edits which file. these messages are folded in your thread.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/sync-dark.svg"><img src="docs/brand/readme/feat/sync-light.svg" width="680" alt="release asks emoji-csv and dark-mode what it needs; they answer; the four messages fold into one line, and main says there is nothing for you."></picture>

### tools

#### every MCP server, always enabled

connect as many MCP servers as you want (GitHub, Linear, Sentry, Slack, your database) and keep them all on. agents call tools from code, so a hundred servers don't fill the context.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/tools-dark.svg"><img src="docs/brand/readme/feat/tools-light.svg" width="680" alt="42 MCP servers are on. you ask why signup is slow; main writes a few lines of code that call Sentry, GitHub and Slack, and answers in two lines."></picture>

#### computer use

your agents can use your browser: they open pages in their own tab group, in the background, with your logins, and read, click, type, fill forms and take screenshots. they never take your screen or your active tab. on macOS they can drive apps like Notes or Figma too. it's off by default: `/computer-use` sets it up (the Chrome extension, the permissions, a live test). for now it acts without asking first, so give it the jobs you'd give someone at your desk.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/computer-dark.svg"><img src="docs/brand/readme/feat/computer-light.svg" width="680" alt="you ask main if buy is visible on the mobile pricing page. an agent opens it in a background tab in its own group, reads it, takes a screenshot, and main answers: buy shows on all three plans. your own tab never moved."></picture>

#### Agent Plugins

skills, MCP servers and hooks in the [Agent Plugins](https://agent-plugins.org) format load as they are, from `~/.agents/plugins` or your repo. Vibe plugins too.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/plugins-dark.svg"><img src="docs/brand/readme/feat/plugins-light.svg" width="680" alt="on its first run, bise finds your skills, plugins and MCP servers; you ask for release notes and the agent uses your own skill."></picture>

### control

#### talk to any agent

`⌥` + a number, or `@name`, talks to one agent directly. ask it why, redirect it, then `⌥0` takes you back to main. the other agents keep running.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/direct-dark.svg"><img src="docs/brand/readme/feat/direct-light.svg" width="680" alt="you press alt+1 and talk to perf directly: why is signup slow, then add a check. alt+0 takes you back to main. nobody stopped."></picture>

#### pull requests

in a repo that takes pull requests, each change gets its own branch and PR, and goes through your CI, review bots and teammates. the panel shows each PR's state: checks, review, merged.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/prs-dark.svg"><img src="docs/brand/readme/feat/prs-light.svg" width="680" alt="cookies opens pull request #409. CI fails, it fixes the test; a review bot asks for a bigger button, you say do it; #409 merges. the panel shows where it stands."></picture>

## details

small things, done carefully.

#### agents only when needed

main does small jobs itself and starts an agent only when a job needs one. no agents reviewing each other in loops, so fewer tokens.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/tokens-dark.svg"><img src="docs/brand/readme/feat/tokens-light.svg" width="680" alt="you ask for a one-word typo fix. main does it itself: too small for an agent. 0 agents started, the count stays at 2."></picture>

#### images

paste a screenshot with `ctrl+v` or drag it in. it becomes one chip in your message.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/screenshot-dark.svg"><img src="docs/brand/readme/feat/screenshot-light.svg" width="680" alt="you type a message, paste a screenshot with ctrl+v: it lands as one chip in your text, and the agent gets the image."></picture>

#### quotes

select lines in the history and start typing: they're attached to your message as a quote.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/quote-dark.svg"><img src="docs/brand/readme/feat/quote-light.svg" width="680" alt="you select &#x27;4.1 s to 0.9 s&#x27; in perf&#x27;s answer and start typing: the lines come along as a quote chip, and perf answers about them."></picture>

#### a model per agent

a large model for the hard job, a fast one for chores. `/model` and `/reasoning` set them per agent.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/model-dark.svg"><img src="docs/brand/readme/feat/model-light.svg" width="680" alt="each agent shows its model in the panel. you switch to release and type /model opus, then /reasoning hi: its line goes from haiku·lo to opus·hi."></picture>

#### built-in shell

``ctrl+` `` opens a terminal in your repo. it keeps running while hidden.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/shell-dark.svg"><img src="docs/brand/readme/feat/shell-light.svg" width="680" alt="ctrl+` opens a terminal in your repo; you start the dev server and hide it; it keeps running; ctrl+` again and the new requests are there."></picture>

#### restart-safe

update or quit mid-work. agents resume where they stopped, and your thread, your draft and your queued messages come back.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/restart-dark.svg"><img src="docs/brand/readme/feat/restart-light.svg" width="680" alt="you are typing a draft while three agents work. bise restarts. it comes back at once: the agents pick up where they were, your draft is still in the composer."></picture>

#### proven message passing

the hub that carries messages between agents is written in [Bend](https://github.com/HigherOrderCO/Bend), with checked proofs that no message is lost or delivered twice (`bend/PROOF.bend`).

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/proof-dark.svg"><img src="docs/brand/readme/feat/proof-light.svg" width="680" alt="in bise&#x27;s terminal you run bend PROOF.bend: no message lost, none delivered twice, a restart is the same state, nothing waits forever. all proofs check."></picture>

#### your terminal's theme

bise uses your terminal's colors, light or dark, and keeps text at a reading width.

<picture><source media="(prefers-color-scheme: dark)" srcset="docs/brand/readme/feat/theme-dark.svg"><img src="docs/brand/readme/feat/theme-light.svg" width="680" alt="the same bise screen in your terminal&#x27;s dark colors, then light, then dark again; the text stops at a reading width."></picture>

## from source

```sh
git clone https://github.com/gvergnaud/bise && cd bise
./run.sh    # builds and starts bise in the current folder
```

## what's in here

- [`rust/`](rust/) · the app: the terminal UI, the agent harness, plugins, sessions
- [`bend/`](bend/) · the agent runtime and the Switchboard hub, written in [Bend](https://github.com/HigherOrderCO/Bend), and their laws (`LAWS.bend`, `PROOF.bend`)
- [`prompts/`](prompts/) · the system prompts and tool descriptions the agents read
- [`scripts/`](scripts/) · dev scripts: build the Bend binaries, build and switch versions
- [`docs/`](docs/) · design docs, RFCs, the implementation notes
- [`site/`](site/) · [bise.dev](https://bise.dev), a static site (the installer too)
- [`tests/`](tests/) · the gate (`gate.sh`), e2e and TUI tests
- [`packaging/`](packaging/) · build, install and release scripts
- [`docs/brand/`](docs/brand/) · the brand book, the issue list, these images

## license

Apache-2.0, see [LICENSE](LICENSE). third-party components: [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES).

<p align="center"><br>ideas in. little kisses out. also pull requests. :*</p>
