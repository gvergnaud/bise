# 19 · a sandboxed agent can read every secret bise keeps

Status: done (sandbox-secrets, architect's plans m_13469 and m_13735): step A for files mode and ~/.ssh, step B for keychain mode. Found by architect while reviewing keychain-secrets' plan (m_13191, correction m_13201). Read from the code, not exploited. Label: security.

- **Step A (done):** `approvals/secrets.rs` is the one list (`auth.json` and `secrets/` from `bise_home`, `~/.ssh` but `config*`, `known_hosts*`, `authorized_keys*`, `*.pub`, `agent/`), read by the profile's `(deny file-read-data …)` and write deny, by `Denial::Secret` (a stopped read: one line, no card, no checker, no rerun without the sandbox) and by `allow_flags` (a command that names a secret never skips the sandbox). /approvals says in yolo that agents can read them. Measured under the real `sandbox-exec`: `cat`, `cp`, python, `ls`, `base64 <`, a link: refused; ssh with the key in ssh-agent works with the key file closed (a key only on disk: the line names `ssh-add`); gh's credential helper, `security` on a keychain, `~/.gitconfig`, `~/.ssh/config`, `known_hosts`, `*.pub`, `~/.config/gh` still read. Found on the way: docs/issues/20 (a sandboxed command can drive a process outside the sandbox, tmux measured).
- **Step B (done):** keychain mode's items live in bise's own keychain file, `~/.bise/secrets/bise.keychain-db`, inside step A's read deny (a sandboxed `security find-generic-password` on it answers "not found"), never in the login keychain, so gh, git and `/usr/bin/security` stay open. Its password is one login-keychain item. The stub's head says which keychain (`bise-secret bise-keychain`; v2026.10.2-28's `bise-secret keychain` is read in the login keychain and moves on its next write or on `bise secrets keychain on` again); -28 fails closed on the new head (measured on its binary). Before any `security` call both keychains' states are read with no window (Security.framework, `rust/secrets/src/status.rs`) and a pure `ready::next_step` decides: a locked keychain is "locked", never a dialog.

## The problem

The agents' sandbox (`rust/switchboard/src/approvals/sandbox.rs`) denies *writing* to `~/.bise/auth.json`, `~/.bise/hubs/`, `approvals.toml` and a few home files (`.ssh`, the shell rc files, `.config/git`). It never denies *reading*. So any agent's bash, sandboxed or not, can read:

- `~/.bise/auth.json`: every provider API key, and the ChatGPT sign-in (access token, id token, **refresh token**);
- `~/.bise/secrets/mcp-oauth/*.json`: every MCP server's OAuth tokens, refresh tokens included;
- `~/.ssh/*` too: `HOME_PROTECTED` is a write deny as well.

API keys already reach the REPLs through their environment, so reading them from the file adds little. The refresh tokens are different: an agent, or a prompt injection that drives one, can copy a token that keeps working after the session ends, and use the user's ChatGPT plan or MCP accounts from anywhere.

Keychain mode (option A of the secrets-keychain page) doesn't change this alone: items written by `/usr/bin/security` can be read back by any process that runs `/usr/bin/security`.

## The result (what is true after)

1. A sandboxed agent's command can't read `auth.json`, `secrets/` or `~/.ssh` private keys, in file mode or in keychain mode. It gets one refusal line it can understand.
2. Everything bise itself needs keeps working: the REPL and the hub (not sandboxed), `bise auth token chatgpt` run by the provider, MCP servers started by the runtime, and git and gh over ssh or https from an agent, with what they still need named.
3. Unsandboxed (yolo) agents are out of scope, and the approvals screen says so.

## Ways to get there (to decide in a plan)

- **A read deny in the profile** for `~/.bise/auth.json`, `~/.bise/secrets` and `~/.ssh/id_*`, with a test under the real `sandbox-exec`. Check what agents legitimately read there: git over ssh needs the private key through ssh-agent, not the file.
- **Keychain mode:** deny `process-exec` of `/usr/bin/security` and `mach-lookup` of the keychain services in the agents' profile, and measure what that breaks (git's osxkeychain credential helper, gh).
- **Where the secrets are read:** only the processes that need a secret read it, through `bise_secrets` (keychain-secrets' crate), and never an agent's own tool call.

Owner: to assign (keychain-secrets touches the same profile and readers). Reviewed by architect. On main.
