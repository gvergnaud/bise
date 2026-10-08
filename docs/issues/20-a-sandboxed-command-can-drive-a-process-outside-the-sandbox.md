# 20 · a sandboxed command can drive a process outside the sandbox

Status: open. Found by sandbox-secrets while measuring issue 19 (architect m_13469: write it as the class). Label: security.

## The problem

The agents' sandbox (`rust/switchboard/src/approvals/sandbox.rs`) applies to the process `sandbox-exec` starts and to its children. A process that already runs outside the sandbox, and that a sandboxed command can talk to, does what it is asked with the user's full rights: every deny of the profile (writes outside the roots, the network, and since issue 19 the reads of `auth.json`, `secrets/` and the ssh keys) is gone for that request.

The profile keeps unix sockets open (the hub's `agent.sock`, ssh-agent, tmux in tests), so the doors are many.

## Measured (issue 19, real `sandbox-exec`, throwaway files)

- **tmux**: a tmux server started outside the sandbox (a throwaway one, `tmux -L x`): `tmux -L x new-window "cat ~/.bise/auth.json > f"` run under the profile wrote the secret to `f`. A server the user started (his own terminal's tmux, `default` socket in `/private/tmp/tmux-<uid>/`) is the same door.
- **launchctl submit** under the profile: the job did not run (nothing written within 1 s). Not checked further.

## Not tested (same class, to measure)

- `screen` sessions the user started;
- `open` / LaunchServices (`open -a Terminal x.command`, `open x.app`): the app starts outside the sandbox;
- AppleEvents: `osascript -e 'tell app "Terminal" to do script "…"'`, iTerm's scripting;
- `ssh localhost` (needs a key in the agent and Remote Login on);
- `docker run -v ~/.bise:/x` (the daemon runs outside);
- any long-running helper of the user's with a socket or a port on loopback (editors' servers, language servers, dev servers with an eval endpoint).

## The result (what is true after)

1. A sandboxed command can't ask a process outside the sandbox to run a command or read a file for it, through the doors above, or the ones that stay are named on the approvals screen.
2. What agents use legitimately keeps working: their own tmux servers (tests start them inside the sandbox), the hub's `agent.sock`, ssh-agent.

## Ways to get there (to decide in a plan)

- Deny `network-outbound` to the tmux and screen sockets of servers the sandbox did not start (the user's `default` socket; agents' tests use `-L <name>` under `$TMUX_TMPDIR`, which the gate sets to the agent's tmp).
- Deny `appleevent-send` and `lsopen` in the profile, and measure what that breaks (`open` of a URL by an agent, `osascript` in tests).
- Deny `process-exec` of `/usr/bin/ssh` to loopback hosts? (no host filter in SBPL: likely a parser tier instead).

Owner: to assign. Reviewed by architect. On main.
