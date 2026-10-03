# Set up bise for me

You are an AI coding agent (Claude Code, Codex, or another). The user wants
to try bise (https://bise.dev), a terminal app that runs one agent per task
in parallel. Install it and set it up from what this machine already has,
so its first run asks almost nothing. Work in the user's shell, step by step.

## Rules

- Never show a key or a token, not even part of one. Do NOT open, `cat`,
  `grep`, `jq` or read shell rc files, `.env` files, `~/.claude.json` or
  `~/.codex/config.toml`: they hold secrets. `bise setup scan` reads them for
  you and prints names and places only. Keys move only through the
  commands below (`--from FILE`, a pipe).
- No sudo. Touch only: bise's install (`~/.local/share/bise`,
  `~/.local/bin`), `~/.bise/`, `~/.agents/plugins/`, `~/.agents/skills/`, and
  Ghostty's config (bise backs it up first).
- Read first, write after: show the user ONE short plan and wait for their
  yes. Then do only what they approved.
- Safe to run twice: skip what is already done.
- A ChatGPT Plus or Pro plan works: `bise login chatgpt` (or the first
  run's "Continue with ChatGPT"); the user signs in in the browser, you
  can't do it for them (`--no-browser` prints the link). OpenRouter can
  sign in too (`bise login openrouter`), and the GLM, Kimi and MiniMax
  coding plans are keys (`bise login zai-coding`, `kimi-code`,
  `minimax`). A Claude Pro/Max login can't be used (Anthropic's terms:
  bise needs an Anthropic API key); Copilot and SuperGrok are not
  supported. bise.dev/docs/subscriptions says more.

## 1. Install

`command -v bise || curl -fsSL https://bise.dev/install | sh`

If `bise` is still not found, use `~/.local/bin/bise` and tell the user to
open a new terminal later. Check with `bise --version`.

## 2. Look around

Run `bise setup scan` in the user's repo. It lists: the API keys found and
where (the environment, a shell file, a repo's `.env`), what bise has
already, Claude Code's and Codex's model (with the closest bise model),
instructions file, skills and MCP servers, the repos, the terminal.

## 3. Plan, then ask

Pick the model the user runs today: Claude Code's → `anthropic/…`, Codex's
→ `openai/…` (the scan's `→ bise:` hint; `bise models <word>` lists more).
It needs a key of that provider among the keys found; else take a provider
that has a key, with its default model.

Show a short plan, only the lines with something to do, e.g.:

    bise setup, nothing written yet
    key        anthropic, from ~/.zshrc (one tiny test call before saving)
    model      anthropic/claude-opus-5-5 (you use opus in Claude Code)
    AGENTS.md  ~/.bise/AGENTS.md ← a copy of ~/.claude/CLAUDE.md
    repos      read CLAUDE.md where there is no AGENTS.md
    skills     review-pr, linked into ~/.agents/skills
    MCP        github, linear (remote, http) from Claude Code; fs (Codex)
    terminal   4 Ghostty lines for cmd+v/f/k/a (backup kept)
    ok?

Then stop and wait for the user's answer.

## 4. Apply (only what was approved)

- **Key** (from a file, the key never leaves the pipe):
  `bise login anthropic --check --model anthropic/claude-opus-5-5 --from ~/.zshrc`
  A key found in the environment only: `printenv ANTHROPIC_API_KEY | bise login anthropic --check --model …`.
  `--check` makes one tiny call and saves the key only if it answers. If it
  fails, tell the user why (the command says it in plain words, never the
  key) and try the next place found; none works: leave it, bise's first
  run asks for a key.
- **Model:** `bise config set model anthropic/claude-opus-5-5`
- **Global instructions:** if ~/.bise/AGENTS.md is missing:
  `mkdir -p ~/.bise && cp -n ~/.claude/CLAUDE.md ~/.bise/AGENTS.md` (or ~/.codex/AGENTS.md).
- **Repo instructions:** `bise config set project_doc_fallback_filenames CLAUDE.md`
  (a folder without AGENTS.md then gives its CLAUDE.md to the agents).
- **Skills:** for each skill folder not in ~/.agents/skills yet:
  `mkdir -p ~/.agents/skills && ln -s ~/.claude/skills/NAME ~/.agents/skills/NAME`.
- **MCP servers**, local and remote (http/sse with their headers; the tokens go
  through the pipe, never shown; a `${VAR}` reference stays one, filled from
  bise's environment when the server connects):
  `jq '{mcpServers: (.mcpServers // {})}' ~/.claude.json | bise plugins import-mcp from-claude-code`
  `python3 -c 'import tomllib,json,os; c=tomllib.load(open(os.path.expanduser("~/.codex/config.toml"),"rb")); print(json.dumps({"mcpServers": c.get("mcp_servers", {})}))' | bise plugins import-mcp from-codex`
  Then `bise plugins list` must show them without errors.
- **Ghostty:** `bise setup ghostty`

## 5. Check and hand over

Run `bise auth check` (the key works with the model in use) and
`bise doctor`. Then tell the user, in three lines at most: what was set,
what was skipped and why, and the next step:

    cd your-repo && bise

Its first run then skips the key step. Don't start bise yourself: it is a
full-screen app for the user's terminal.
