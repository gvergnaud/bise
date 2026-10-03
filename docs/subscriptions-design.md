# Subscriptions and API keys: design (phase 2)

Research: docs/subscriptions-research.md. Scope of this release:

1. **ChatGPT plan** (Plus/Pro) through OpenAI's open-source "Sign in with
   ChatGPT" flow: a new provider `chatgpt`.
2. **OpenRouter sign-in**: a browser login that mints a normal OpenRouter
   API key (the provider `openrouter` does not change).
3. **Coding plans that are API keys**: new providers `zai-coding` (GLM
   Coding Plan), `kimi-code` (Kimi Code), `minimax` (MiniMax token plan).
4. **Detection** of logins bise cannot or must not use, to tell the user
   the right next step: Codex signed in with ChatGPT, Claude Code signed in
   with a Claude plan. Presence only: bise never reads their tokens.

Not in scope (terms): Claude Pro/Max, Gemini CLI/Antigravity, Copilot,
SuperGrok logins. Main asks Gabriel; the design leaves room (an `oauth`
entry is per provider) but nothing is built.

## Why `chatgpt` is its own provider (not `openai` with another key)

- The two bill differently and both can be set up at once: a model id says
  which one pays (`chatgpt/gpt-6.1-sol` = the plan, `openai/gpt-6.1-sol` =
  the API key). Roles can mix them; `/models` shows both.
- The plan route has its own request rules (below): the provider carries
  them, the runtime does not guess from the key.
- No prices for `chatgpt` models: the cost line says "ChatGPT plan".

## Files (everything a prompt can read or write)

### `~/.bise/auth.json` (0600, OpenCode layout, one entry per provider)

```json
{
  "anthropic": {"type": "api", "key": "sk-ant-..."},
  "openrouter": {"type": "api", "key": "sk-or-...", "via": "openrouter-login"},
  "chatgpt": {
    "type": "oauth",
    "client_id": "oaiapp_...",
    "email": "you@example.com",
    "subject": "<id token sub>",
    "plan": "plus",
    "access": "<access token>",
    "refresh": "<refresh token>",
    "expires": 1790000000000,
    "id_token": "<id token>",
    "scopes": ["chatgpt.tokens.use.direct", "email", "offline_access", "openid", "profile", "resource.invoke"],
    "saved_at": "2026-10-03T13:00:00Z"
  }
}
```

- `type: "api"` is today's entry; `via` is optional (where it came from).
- `type: "oauth"`: `expires` in ms since epoch (OpenCode's field). Signed
  out = the entry keeps `client_id`, `email`, `subject` and drops the
  tokens (OpenAI asks to reuse the issued client for the next sign-in).
- One ChatGPT account per machine in this release (the store has one
  `chatgpt` entry). Several accounts later: `chatgpt@<label>` keys.
- A prompt can write an `api` entry (that is `bise login <p>` piped).
  It cannot make an `oauth` entry: that needs the user's consent in a
  browser. It runs `bise login chatgpt --no-browser`, which prints the URL
  and waits for the browser to come back.

### `~/.bise/host-id` (0600, not a secret)

`urn:uuid:<v4>`, made once before the first ChatGPT sign-in, never
changed (OpenAI's `ext_agent_host_id`). Kept on logout.

### `~/.bise/config.toml`

Nothing new is required: `model = "chatgpt/gpt-6.1-sol"`, roles, and the
usual `[providers.chatgpt]` overrides work. New optional keys:

```toml
[providers.chatgpt]
auth = "chatgpt"          # built in; how the provider logs in: "api" (default) | "chatgpt"
shape = "chatgpt-plan"    # built in; the request rules (runtime)
```

`auth` and `shape` are catalog fields like `key_command`; users never
need to write them. Endpoints can be moved for tests:
`BISE_CHATGPT_ISSUER` (default `https://auth.openai.com`) and the
provider's `base_url`; `BISE_OPENROUTER_AUTH` (default
`https://openrouter.ai`).

## How a call gets its token

The `chatgpt` provider has `key_env = ""` and
`key_command = '<abs path of bise> auth token chatgpt'` (the catalog
writes the running binary's path into the models file, BISE_MODELS_FILE).
The runtime already runs `key_command` before every call
(bend/runtime/provider.bend `key_of.pick`).

`bise auth token chatgpt`: read auth.json; access valid for 5+ more
minutes -> print it, exit 0. Else take the lock
(`~/.bise/auth.json.lock`, flock), read again (another process may have
refreshed), refresh if still needed (POST token endpoint,
`grant_type=refresh_token`, issued client_id, `resource`), write the new
access/refresh/expires atomically, print. Signed out, refresh refused
(`invalid_grant`), or no network: print nothing on stdout, one line on
stderr, exit 1 -> the runtime's existing "key_command failed" path, with a
chatgpt-specific line (below). A token is never in the environment, the
log, or an argument.

## Flows

Words below are drafts: the designer signs off the final ones.

### First run (onboarding key step)

Today: a key step only when no key is found. New: that step is "how do you
want to pay for the models", a list:

```
  Continue with ChatGPT        use your Plus or Pro plan
  OpenRouter                   sign in, or paste a key
  Paste an API key             Anthropic, OpenAI, Google, Mistral, …
  Coding plan key              GLM, Kimi, MiniMax
```

- When Codex is signed in with ChatGPT (detected), the first row is
  marked: "you use ChatGPT in Codex: one click here too".
- When Claude Code is signed in with a Claude plan (detected) and there
  is no Anthropic key: under the list, one dim line: "Claude Code's plan
  only runs in Claude Code: Anthropic needs an API key here."
- Continue with ChatGPT: open the browser, show "waiting for your browser…
  (esc cancels · c copies the link)"; on return: "signed in as
  you@example.com · ChatGPT Plus", then one tiny check call (like the key
  check today), then the roles default to chatgpt models.
- Declined plan use (`access_denied`) or no plan scope: "ChatGPT signed you
  in but did not allow plan use. Try again and allow it, or pick another
  way." Never a crash, never a raw OAuth error.
- A key is found already: no step (as today). The setup card in the
  thread offers "use your ChatGPT plan too" only when Codex is detected and
  `chatgpt` is not set up.

### `/provider` (TUI) and `bise providers` (CLI)

`chatgpt` row states: `✓ signed in · you@example.com · Plus`,
`signed out` (client kept), `not set up`, `✗ sign-in expired · enter to
sign in again`. Enter on a set-up row: its menu: sign in again / switch
account, sign out, ChatGPT usage settings (link). `openrouter` gets
"sign in with OpenRouter" next to "paste a key".

### CLI

- `bise login chatgpt [--no-browser]`: the flow above in the terminal.
  `--no-browser`: print the URL, wait (an agent or SSH session; the
  callback is 127.0.0.1, so SSH users forward the port: said in the line).
- `bise login openrouter [--browser|--key]`: on a terminal, asks which;
  piped stdin = a key (as today).
- `bise logout chatgpt`: revoke the refresh token (RFC 7009 endpoint from
  the OpenID configuration), clear the tokens, keep the client. "signed
  out (ChatGPT confirmed)" or "signed out here; ChatGPT did not confirm:
  disconnect bise in ChatGPT settings".
- `bise auth token chatgpt`: internal (the key_command). Refuses on a
  terminal ("this prints a secret: it is for bise's runtime").
- `bise auth status [--json]`: every provider, how it logs in, its state,
  where its key comes from, the detected logins; never a secret. For
  agents configuring bise.

### `/models`

`chatgpt` models are listed when signed in. The list comes from
`GET /v1/models` with the token (`visibility == "list"`, server order,
`display_name`), fetched at sign-in and when `/models` opens (cached in
`~/.bise/cache/chatgpt-models.json`, 1 day), falling back to the built-in
entries. Each row says what pays: `ChatGPT plan` vs the price.

### `bise doctor` (no network, never a secret)

- `chatgpt`: `signed in as you@example.com (Plus) · token renews by
  itself · sign-in good until <refresh saved_at + 30 days>`; warn under 3
  days left ("run `bise login chatgpt`"); warn on loose auth.json mode
  (exists today).
- Detected logins: `codex: signed in with ChatGPT (bise has its own
  sign-in: bise login chatgpt)`, `claude code: signed in with a Claude
  plan (bise needs an Anthropic API key)`. Info lines, not warnings.
- `keys` check passes with only a subscription.

### Errors in a turn (one line each, the runtime's)

- usage limit (`subscription_sharing_usage_limit_exceeded`): "your ChatGPT
  plan's limit for bise is reached: it resets on its own, or switch model
  (/models)".
- `subscription_sharing_usage_unavailable`: "ChatGPT plan use is off for
  bise: turn it on in ChatGPT settings, or /provider".
- 401 / token command failed: "ChatGPT sign-in expired: /provider (or
  `bise login chatgpt`)".

## The runtime's `chatgpt-plan` request shape (Bend, with laws)

On `shape = "chatgpt-plan"` (openai-responses family), the body:
- `store: false`, `stream: true` (always streamed);
- no `max_output_tokens`, `temperature`, `top_p`, `metadata`, `user`,
  `prompt_cache_retention`, `safety_identifier`, `truncation`,
  `previous_response_id`; whole history in `input` (already the case);
- system text as a developer message (already the case: check), never a
  `role: "system"` item;
- function tools grouped in one namespace:
  `{"type":"namespace","name":"bise","description":"...","tools":[...]}`,
  and a namespaced call back (`namespace` field on the call) maps to the
  tool's own name; verify the exact shape against OpenAI's function-calling
  docs and Codex (`~/lab/codex`) before writing laws;
- the two usage-limit errors and 401 become the lines above.

## Detection (presence only, `rust/catalog/src/detect.rs`)

- Codex: `$CODEX_HOME/auth.json` or `~/.codex/auth.json` exists and parses
  with `tokens` present (or `auth_mode == "chatgpt"`): "codex-chatgpt".
  Only the JSON keys' presence is looked at; values are not kept. Codex's
  keyring store: not probed.
- Claude Code: `~/.claude/.credentials.json` exists, or on macOS
  `security find-generic-password -s "Claude Code-credentials"` (no `-w`:
  attributes only, exit code only) succeeds: "claude-plan".
- Tests: a temp HOME with fake files; never the real ones.

## Fake servers for tests (tests/fake_openai_auth.py, extend fake_provider.py)

- auth: `/.well-known/openid-configuration`, `/api/accounts/authorize`
  (auto-redirects to the loopback with code, state, issued client_id;
  knobs: deny, no plan scope, wrong state), `/api/accounts/oauth/token`
  (code + PKCE check, refresh with rotation, `invalid_grant` knob, expiry
  knob), JWKS + RS256-signed ID tokens, revocation endpoint.
- responses: `/v1/responses` rejects anything the plan route rejects
  (store true, no stream, max_output_tokens, temperature, system item,
  un-namespaced tools, a bad token) with OpenAI's error shapes; usage-limit
  knobs mid-stream; `/v1/models` with `visibility`.
- OpenRouter: `/auth` redirect + `/api/v1/auth/keys` with PKCE check.
