---
title: landing work
description: how the agents' work reaches your repo: small commits straight on main, a feature branch you try first, or pull requests.
---

## two flows, one per repo

| flow | for | what the agents do |
|---|---|---|
| **trunk** | a repo only you push to | land small commits on your default branch, one at a time, each one checked, with no pull request |
| **PR** | a repo you share | each task gets its own branch in a worktree, pushes it, opens a pull request, and follows it until it is merged |

bise suggests a flow from the repo itself:

| what bise sees | flow |
|---|---|
| no remote | trunk (a PR is impossible) |
| the default branch is protected, or a ruleset requires a PR | PR, forced |
| someone else committed on the default branch in the last 90 days | PR, suggested |
| the repo's AGENTS.md or CONTRIBUTING asks for PRs | PR, suggested |
| none of these: you alone | trunk, suggested |

when it's a suggestion, main asks you once, at the first task that changes code, and remembers your answer. your words win for one task: "open a PR for this", or "just commit it".

## trunk flow

an agent never commits someone else's half-done files. each agent commits only the files it changed, and bise moves the branch only if nobody moved it meanwhile. a file two agents both changed is refused, and main asks you who takes it.

an agent in its own worktree commits on the worktree's branch first. when its work is done, bise rebases the branch on your default branch, runs the check again if needed, and moves the default branch to it. one land at a time, so two agents never race.

by default, bise pushes the default branch after every land. to keep the lands local until you push yourself, set `push = false` in the repo's `[flow]`.

### feature branches

experimental, risky or big work (several agents, several days) goes on a local feature branch: its agents land there, never on main. when they're done, an inbox item lets you try a build of the branch. main merges it into your default branch on your go. say "in a branch" or "I want to try it first" to get one; "just land it" puts a task back on main.

## PR flow

1. the task's agent works in its own worktree, on a branch made from your remote's default branch.
2. it commits small and runs the check.
3. it pushes the branch and opens the pull request with `gh pr create`, using the repo's template when there is one.
4. it stays alive until the merge: it answers review comments, fixes red checks, and rebases when the base moves.
5. it never merges, approves, closes, or comments on GitHub unless you allowed it.

when the PR is merged, bise archives its agents and removes the worktree. `↑` in the panel marks an agent with a PR.

PR flow needs the GitHub CLI logged in: `gh auth login`. `bise doctor` checks it.

## the check

every land runs the repo's check command first, the one in `[flow] check`. main asks for it once when it isn't set, or reads it from the repo.
