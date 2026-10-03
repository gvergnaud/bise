---
title: subscriptions
description: use a plan you already pay for, ChatGPT Plus or Pro, or a coding plan from Z.ai, Kimi or MiniMax, in place of an API key.
---

<!-- draft: goes live with the release that ships subscriptions. at the release, move it to site/docs/ and run python3 site/docs/build.py (docs/site-drafts/README.md) -->

an API key bills each call. a plan is a monthly price you may already pay. bise can use a few plans, signed in with the provider's own login or with the plan's key.

| plan | how bise uses it | set it up |
|---|---|---|
| ChatGPT Plus or Pro | "Sign in with ChatGPT", in your browser | `bise login chatgpt` |
| OpenRouter | sign in in your browser: it makes an OpenRouter key for bise | `bise login openrouter` |
| GLM Coding Plan (Z.ai) | the plan's API key | `bise login zai-coding` |
| Kimi Code | the plan's API key | `bise login kimi-code` |
| MiniMax M plan | the plan's API key | `bise login minimax` |

the first run asks how you want to pay for the models: Continue with ChatGPT, OpenRouter, an API key, or a coding plan key. later, `/provider` adds or changes any of them.

## ChatGPT Plus or Pro

bise signs in with OpenAI's "Sign in with ChatGPT": your ChatGPT plan pays for the calls, not an API account.

### sign in

in bise, the first run's **Continue with ChatGPT**, or `/provider`, then ChatGPT. in a terminal:

```sh
bise login chatgpt
```

```text
opening your browser to sign in to ChatGPT…
or open this link:
  https://auth.openai.com/...
waiting… ctrl+c cancels.
✓ signed in as you@example.com · ChatGPT Plus.
```

sign in to ChatGPT in the browser and allow bise to use your plan. bise checks the plan with one tiny call, then main and your agents use a ChatGPT model on your plan.

### over SSH, or from an agent

```sh
bise login chatgpt --no-browser
```

it prints the link and waits. the sign-in comes back to `127.0.0.1` on the machine where bise runs, so over SSH, forward that port first; the command prints the exact line, like `ssh -L <port>:127.0.0.1:<port> <host>`.

a coding agent setting bise up for you can run this command, but only you can sign in: the browser asks for your consent.

### the models

ChatGPT models are named `chatgpt/<model>`, for example `chatgpt/gpt-6.1-sol`. they are OpenAI's models, billed to your plan. `openai/<model>` stays your API key: you can have both and mix them across roles.

`/models` lists the models your plan offers, and says `your ChatGPT plan` where the others show a price.

```sh
bise config set main chatgpt/gpt-6.1-sol
bise config set small chatgpt/gpt-6-luna
```

### limits and errors

your plan has a usage limit for apps like bise. when it's reached, the turn ends with:

```text
▲ your ChatGPT plan's limit for bise is reached. your usage is at chatgpt.com/settings/usage, or switch model with /model.
```

your usage is at [chatgpt.com/settings/usage](https://chatgpt.com/settings/usage); `/provider` links it too. the other lines you may see:

| line | what to do |
|---|---|
| `▲ ChatGPT plan use is off for bise. turn it on in your ChatGPT settings, or pick another provider in /provider.` | your account or workspace doesn't allow plan use in other apps |
| `▲ ChatGPT couldn't check your plan's usage. try again in a moment, or switch model with /model.` | OpenAI couldn't check your usage; bise already retried |
| `▲ your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt.` | sign in again |

### sign out

```sh
bise logout chatgpt
```

bise tells ChatGPT to end its sign-in. if ChatGPT doesn't confirm, the line says so: then remove bise from the connected apps in your ChatGPT settings.

## OpenRouter

```sh
bise login openrouter            # asks: sign in with your browser, or paste a key
bise login openrouter --browser
bise login openrouter --key
bise login openrouter --no-browser   # prints the link (SSH: forward its port, as for ChatGPT)
```

the browser sign-in makes a normal OpenRouter API key for bise and saves it, so nothing else changes: the models are `openrouter/<vendor>/<model>`, paid with your OpenRouter credit.

## coding plans

Z.ai, Kimi and MiniMax sell coding plans for tools like bise. each plan gives you an API key for its coding endpoint; bise has a provider for each.

| plan | provider | key variable | starts with | get the key |
|---|---|---|---|---|
| GLM Coding Plan | `zai-coding` | `ZAI_API_KEY` | `zai-coding/glm-5.3` | [z.ai](https://z.ai/manage-apikey/apikey-list) |
| Kimi Code | `kimi-code` | `KIMI_API_KEY` | `kimi-code/kimi-for-coding` | [kimi.com](https://www.kimi.com/code/console) |
| MiniMax M plan | `minimax` | `MINIMAX_API_KEY` | `minimax/MiniMax-M3` | [platform.minimax.io](https://platform.minimax.io/user-center/basic-information/interface-key) |

```sh
bise login zai-coding
```

or the first run's **a coding plan key**. the key is checked with one tiny call, then saved like any key (see [providers and keys](providers#where-a-key-comes-from)).

## where it's saved

| file | what |
|---|---|
| `~/.bise/auth.json` | the keys, and the ChatGPT sign-in: its tokens, your email and plan. only you can read it (0600) |
| `~/.bise/host-id` | an id for this machine, made once before the first ChatGPT sign-in. not a secret |

the ChatGPT token renews by itself while bise runs. a token never goes in the environment, a log, or a command line.

```sh
bise auth status        # every provider: how it logs in, its state. never a secret
bise auth status --json # the same, for a script or an agent
bise doctor             # also says how long the ChatGPT sign-in is good for
```

## what bise sees from your other tools

bise looks whether Codex is signed in with ChatGPT, and whether Claude Code is signed in with a Claude plan. it looks only at whether their login is there (a file, or on a Mac a keychain entry), never at the tokens, to tell you the right next step:

- Codex with ChatGPT: the first run marks **Continue with ChatGPT**, "you use it in Codex already". bise still signs in on its own.
- Claude Code with a Claude plan: one line says that plan works only in Claude Code.

## not supported

| plan | why |
|---|---|
| Claude Pro or Max | Anthropic's terms allow these plans only in Claude Code and Anthropic's apps. for Claude in bise, use an Anthropic API key, or Claude through OpenRouter |
| GitHub Copilot | Copilot's sign-in for other tools comes from partnerships with those tools (OpenCode has one). bise has none |
| SuperGrok | the same: no partnership with xAI. an xAI API key works |
| Gemini CLI, Antigravity | Google's terms keep these logins to Google's tools. a Google AI Studio key works |
