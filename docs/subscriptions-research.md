# Subscriptions for model providers: research (phase 1)

Date: 2026-10-03. Sources: provider docs and terms (links below), opencode
`~/lab/opencode` @ 9b4882db (packages/opencode/src/plugin), Codex
`~/lab/codex` @ 69f71405 (codex-rs/login), bise main @ 27ad71d7.

## Verdict per provider

| Provider | Subscription in a third-party harness | Terms today | Build? |
|---|---|---|---|
| OpenAI (ChatGPT Plus/Pro) | "Sign in with ChatGPT", ChatGPT plan usage, open-source flow | **Allowed and documented** for open-source, locally run tools (DevDay 2026-09-29) | yes |
| Anthropic (Claude Pro/Max/Team) | Claude Code's OAuth (`claude.ai` login) | **Forbidden**: "Using OAuth tokens obtained through Claude Free, Pro, or Max accounts in any other product, tool, or service — including the Agent SDK — is not permitted"; server-side blocks since Jan 2026, enforced 2026-04-04, accounts banned. The June 15 "Agent SDK credit" plan is paused. "Unless previously approved, Anthropic does not allow third party developers to offer claude.ai login." | no (decision for the user) |
| GitHub Copilot | GitHub device flow, `api.githubcopilot.com` | Officially supported **for OpenCode only**, "through a formal partnership" (github.blog 2026-01-16), with opencode's own OAuth app. GitHub staff: other uses of the Copilot API outside its clients break the ToS and risk suspension. | no, unless GitHub agrees (decision for the user) |
| Google (Gemini CLI / Antigravity OAuth) | Gemini CLI's OAuth | **Forbidden**: "harvesting or piggybacking on Gemini CLI's OAuth ... is a direct violation"; Antigravity bans in 2026 | no |
| xAI (SuperGrok / X Premium) | Grok-CLI public OAuth client, device code | Announced as an **OpenCode** integration (x.ai/news/grok-opencode, 2026-05-21); nothing says other tools may use the Grok-CLI client | no, unless xAI agrees (decision for the user) |
| OpenRouter | no subscription (credits; Free/Standard/Business plans) | "Sign in with OpenRouter": OAuth PKCE that mints a normal API key for the user, documented for any app | yes, as a login that yields an API key |
| Z.ai GLM Coding Plan, Kimi Code (Kimi membership), MiniMax coding plan | the plan gives an **API key** + a coding base URL | **Allowed**: the plans name OpenCode, Cline, Kilo, OpenClaw... as targets | yes, as API-key providers |
| Mistral | API key | n/a | already supported |

## OpenAI: how "Sign in with ChatGPT" works for an open-source tool

Docs: developers.openai.com/siwc/token-sharing-open-source (+ `/sign-in`,
`/profiles-and-sessions`, `/models-and-inference`, `/token-reference`,
`/preview-limitations`, `/errors-and-recovery`; append `.md`).

- **Host id**: before the first sign-in, make and keep one opaque
  `ext_agent_host_id` per machine (`urn:uuid:<v4>` is accepted).
- **First sign-in = dynamic client registration**: open the system browser at
  `https://auth.openai.com/api/accounts/authorize` with
  `client_id=dynamic_agent_client`, `agent_name_hint=bise`, `ext_agent_host_id`,
  `response_type=code`, `redirect_uri=http://127.0.0.1:<port>/auth/callback`
  (127.0.0.1, never `localhost`; path fixed, port may vary),
  `scope=openid profile email offline_access resource.invoke chatgpt.tokens.use.direct`,
  `resource=https://api.openai.com/v1`, `state`, `nonce`, PKCE S256.
  The callback gives `code`, `state` and the **issued `client_id`**
  (`oaiapp_...`): keep it per account; reuse it (with `id_token_hint`,
  `login_hint`, no `agent_name_hint`) for later sign-ins.
- **Token exchange**: POST form to `https://auth.openai.com/api/accounts/oauth/token`
  (`grant_type=authorization_code`, issued `client_id`, `code`,
  `code_verifier`, same `redirect_uri`, same `resource`). No secret.
  Validate the ID token (JWKS, iss, aud = issued client_id, exp, nonce) and
  that the granted scopes include `chatgpt.tokens.use.direct`.
- **Lifetimes**: access token 1 h; refresh token 30 days, rotating (each
  refresh returns a new one). Refresh: POST `grant_type=refresh_token`,
  issued `client_id`, `refresh_token`, `resource`; serialize refreshes
  across processes. `access_denied` = user declined plan use.
- **Logout**: revoke the refresh token at the `revocation_endpoint` from
  `https://auth.openai.com/.well-known/openid-configuration`, then clear
  the tokens; keep the client_id and host id for the next sign-in.
- **Inference**: the public `POST https://api.openai.com/v1/responses` with
  `Authorization: Bearer <access>`. Models: `GET /v1/models` with the same
  token (`models[]`, keep `visibility == "list"`, show `display_name`,
  send `slug`).
- **Request limits (preview)**: `store: false` and `stream: true` always;
  `input` carries the whole history (no `previous_response_id` over HTTP);
  system text as `instructions` or developer messages (a
  `role: "system"` item is rejected); omit `background`, `conversation`,
  `max_output_tokens`, `max_tool_calls`, `metadata`, `moderation`,
  `prompt`, `prompt_cache_retention`, `safety_identifier`, `temperature`,
  `top_logprobs`, `top_p`, `truncation`, `user`; function tools must be
  grouped in namespaces (or `additional_tools`); no hosted tools.
  Usage limit mid-stream: `response.failed` with
  `subscription_sharing_usage_limit_exceeded` /
  `subscription_sharing_usage_unavailable`.
- **UI rules**: the button says "Continue with ChatGPT"; show which
  account is active; say what the plan covers.

What opencode and Codex do instead (older path, not for us): opencode's
`plugin/openai/codex.ts` borrows **Codex's** client id
`app_EMoamEEZ73f0CkXaXp7hrann` (loopback :1455 or the device code at
`/api/accounts/deviceauth/usercode`) and calls the private
`https://chatgpt.com/backend-api/codex/responses` with `ChatGPT-Account-Id`.
OpenAI now says not to point at `backend-api`. Codex stores
`~/.codex/auth.json` (`auth_mode`, `OPENAI_API_KEY`, `tokens{id_token,
access_token, refresh_token, account_id}`, `last_refresh`; or the keyring).
Borrowing those tokens would also race Codex's rotating refresh token and
log Codex out: bise must only **detect** that file, never use it.

## Anthropic, for the record

Claude Code keeps its login in the macOS Keychain (`Claude Code-credentials`)
or `~/.claude/.credentials.json`. bise can detect it to say "your Claude
subscription only runs in Claude Code; bise needs an API key"; it must
never read the token. The only allowed paths are an API key or a cloud
(Bedrock, Vertex, Foundry: bise already has `foundry`).

## Copilot and xAI, for the record

Copilot (opencode `plugin/github-copilot/copilot.ts`): GitHub device flow
on `github.com/login/device/code` with opencode's own OAuth app
(`Ov23li8tweQw6odWQebz`, scope `read:user`), the GitHub token used as
Bearer on `api.githubcopilot.com` (enterprise: `copilot-api.<domain>`),
headers `x-initiator`, `Openai-Intent`, `X-GitHub-Api-Version`. bise would
need its own GitHub OAuth app and GitHub's agreement.
xAI (opencode `plugin/xai.ts`): Grok-CLI public client, device code on
`auth.x.ai/oauth2/device/code`, scope `... grok-cli:access api:access`.

## OpenRouter login

Browser to `https://openrouter.ai/auth?callback_url=<loopback>&code_challenge=<S256>&code_challenge_method=S256`,
then POST `https://openrouter.ai/api/v1/auth/keys` `{code, code_verifier,
code_challenge_method}` -> `{key}`: a normal OpenRouter API key, stored as
`{"type":"api"}` in auth.json. Nothing to refresh.

## What bise has that fits

- `auth.json` already uses the OpenCode layout and keeps unknown entries:
  an `{"type":"oauth", ...}` entry slots in.
- The runtime already runs a provider's `key_command` before every call
  (bend/runtime/provider.bend `key_of.pick`): a subscription can be
  `key_command = "bise auth token openai"` (refresh under a lock, print the
  access token), so no long-lived token sits in the environment.
- The MCP OAuth code (rust/plugins/src/oauth.rs, login.rs): PKCE, loopback
  redirect, refresh under a lock, `~/.bise/secrets`: reuse it.
- To build in the runtime: an OpenAI "ChatGPT plan" request shape (the
  limits above: no `max_output_tokens`, no system item, namespaced tools,
  `store:false`), and the usage-limit errors as clear lines.
