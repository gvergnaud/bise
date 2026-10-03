---
title: instructions and skills
description: AGENTS.md tells every agent how your repo works. skills are instructions an agent loads only when a task needs them.
---

## AGENTS.md

every agent reads the instructions files when it starts, the way Codex does:

1. yours, for every repo: `~/.bise/AGENTS.override.md`, else `~/.bise/AGENTS.md`.
2. the repo's: in each folder from the repo's root down to the folder you work in, the first of `AGENTS.override.md` and `AGENTS.md`.

a deeper file wins over a higher one, the repo's over yours, and what you say in the chat over all of them. the repo's files count up to 32 KiB together.

### using CLAUDE.md

bise reads no CLAUDE.md unless you ask. to read it where a folder has no AGENTS.md:

```sh
bise config set project_doc_fallback_filenames CLAUDE.md
```

```toml title="~/.bise/config.toml"
project_doc_fallback_filenames = ["CLAUDE.md"]
project_doc_max_bytes = 32768    # the repo's files, together
```

## skills

a skill is a folder with a `SKILL.md`: a name, a one-line description, then instructions. the agents see the list of names and descriptions, and read a skill in full only when a task needs it.

```markdown title="~/.agents/skills/review-pr/SKILL.md"
---
name: review-pr
description: review a pull request the way this team does. use when asked to review a PR.
---

read the diff with `gh pr diff`. check the tests first...
```

bise looks for skills in:

| folder | for |
|---|---|
| `~/.agents/skills/` | you, in every repo |
| `~/.vibe/skills/` | you, shared with Vibe |
| `<repo>/.agents/skills/` | this repo |
| a plugin's `skills/` | the plugin's, named `<plugin>:<skill>`. see [plugins and MCP](plugins) |

type `$` in the composer to see them; `$review-pr` in your message points the agent at that skill. a skill you add or edit shows up without a restart.

Claude Code's skills in `~/.claude/skills` are the same format: link or copy them into `~/.agents/skills`. the [setup prompt](https://bise.dev/setup.md) offers to do it.
