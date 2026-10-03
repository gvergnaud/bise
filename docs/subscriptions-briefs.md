# Subscriptions: implementer briefs (phase 3)

Read first: docs/subscriptions-research.md and docs/subscriptions-design.md
(on main once subs-lead lands; until then on branch sb/subs-lead).
Lead: `subs-lead` (questions, reviews, hand-offs between you). Every test
runs on fake servers and a temp HOME: never a real ChatGPT, OpenRouter,
Codex or Claude Code login, never the real ~/.bise or ~/.codex.
sb-core only through scripts/bins.sh. Commits signed, long subject.

## A. `subs-auth`: the Rust side of logins (catalog + CLI + doctor)

Objective: auth.json `oauth` entries, the ChatGPT sign-in (dynamic client
registration, loopback PKCE on 127.0.0.1, ID token checked against JWKS,
plan scope checked), refresh under a lock, revoke, OpenRouter sign-in,
the new providers, the presence-only detection, and their CLI and doctor
lines.

Owned files: rust/catalog/src/{auth.rs, auth_cli.rs, lib.rs, roles.rs,
models.toml, cli.rs} and new rust/catalog/src/{chatgpt.rs,
openrouter_login.rs, detect.rs} with their tests;
rust/harness/src/{main.rs (dispatch only), doctor.rs};
docs/custom-providers.md. Reuse rust/plugins/src/oauth.rs (query parsing,
loopback server, the lock, safe URLs): if the catalog cannot depend on
the plugins crate, move the shared pieces to a small module both use
(say so to subs-lead first).

Work:
- `Store`: `oauth(provider)` read/write, sign-out keeps client_id/email/
  subject; unknown entries kept as today. `Keys::ready`/`source` know a
  provider with `auth = "chatgpt"` (signed in = ready).
- Catalog: fields `auth` and `shape` (parse, merge, models-file output);
  providers `chatgpt` (openai-responses, https://api.openai.com/v1,
  key_env "", auth chatgpt, shape chatgpt-plan, key_command = the running
  binary's absolute path + ` auth token chatgpt`, single-quoted, no
  prices), `zai-coding` (openai-chat, https://api.z.ai/api/coding/paas/v4),
  `kimi-code` (https://api.kimi.com/coding/v1), `minimax` (verify its
  plan's endpoint): check each base URL and model id on the provider's
  docs; the coding plans in the hidden list.
- roles.rs: one-login defaults (only chatgpt signed in -> every role on a
  chatgpt model, the small model on the cheapest one listed).
- chatgpt.rs: host id file (`~/.bise/host-id`, urn:uuid v4), authorize URL,
  callback (state, issued client_id, access_denied), token exchange,
  ID token validation (RS256 via ring, iss/aud/exp/nonce), refresh with
  rotation under `~/.bise/auth.json.lock` (re-read after the lock),
  revoke via the OpenID configuration's revocation_endpoint, `GET
  /v1/models` into `~/.bise/cache/chatgpt-models.json`. Issuer from
  `BISE_CHATGPT_ISSUER` (default https://auth.openai.com).
- CLI: `bise login chatgpt [--no-browser]`, `bise logout chatgpt`,
  `bise auth token chatgpt` (stdout = token only, refuses a terminal),
  `bise auth status [--json]` (no secret), `bise login openrouter
  [--browser|--key]` (`BISE_OPENROUTER_AUTH` for tests); `bise providers`
  rows for chatgpt.
- detect.rs: Codex ChatGPT login and Claude Code plan login, presence
  only (design §Detection); doctor info lines and the chatgpt line.
- Expose for the TUI (C): a non-blocking sign-in handle (start -> URL;
  poll -> waiting/done/denied/failed; cancel), status of a provider,
  detection results. Tell subs-tui the API as soon as it lands.

Done when: unit tests on fake HTTP servers in-process (sign-in happy path,
denied, no plan scope, bad state, bad ID token signature/nonce, refresh
with rotation, two processes refreshing at once give one refresh and both
print the same token, invalid_grant -> signed out with the client kept,
revoke confirmed / not confirmed, OpenRouter PKCE), detection on a temp
HOME, `cargo test -p bise-catalog`, clippy, quick gate green
(tests/gate.sh); landed on main.

## B. `subs-runtime`: the `chatgpt-plan` request shape (Bend, laws)

Objective: on a provider with `shape = "chatgpt-plan"`, the Responses body
obeys OpenAI's plan route, and its errors read as one clear line.

Owned files: bend/core/oai-resp.bend, bend/core/api.bend,
bend/runtime/provider.bend, bend/runtime/provider-pure.bend (the `shape`
read from the models file), bend/LAWS.bend and the proofs.

Work: `store:false`, always streamed; no `max_output_tokens` (and none of
the fields the design lists); system text as a developer message only;
function tools inside one namespace and namespaced calls mapped back to
the tool's name (check the exact JSON in OpenAI's function-calling docs
and ~/lab/codex before the laws); `response.failed` with
`subscription_sharing_usage_limit_exceeded` /
`subscription_sharing_usage_unavailable` and a 401 or failed key_command on
a chatgpt provider -> the design's lines. Today's openai-responses body
is unchanged without the shape (a law says so).

Done when: laws for the plan body (each field present/absent), the
namespace round trip, the old body unchanged, the error lines; proofs
green (PROOF shards), the runtime tests, quick gate; landed on main.
Coordinates the models-file field name with subs-auth.

## C. `subs-tui`: first run, /provider, /models

Objective: the screens and words the designer signs off: the first-run
"how do you pay" step, ChatGPT sign-in waiting state, the detection hints,
the chatgpt rows and menu in /provider, OpenRouter "sign in or paste",
/models rows that say what pays.

Owned files: rust/tui/src/onboarding.rs, rust/tui/src/onboarding/
{provider.rs, provider_tests.rs, roles.rs, roles_tests.rs},
rust/tui/src/models.rs, rust/tui/src/keycheck.rs, and a new tmux test
under tests/.

Work: get the final words from `designer` (subs-lead asked; ask again if
none came) and build against subs-auth's API (until it lands, the screens
and their tests with a fake sign-in). esc cancels a sign-in and leaves no
listener; `c` copies the link; no secret ever drawn.

Done when: render tests for every state (not set up, waiting, signed in,
denied, expired, Codex detected, Claude Code detected), a tmux test of
the first run signing in against the fake auth server on a temp HOME,
quick gate green; landed on main.

## D. `subs-e2e`: fake servers and end-to-end tests

Objective: fake OpenAI auth, fake plan Responses and fake OpenRouter auth
servers, and e2e tests of the whole path, from sign-in to a turn to
refresh to logout.

Owned files: tests/fake_openai_auth.py (new), tests/fake_provider.py (the
plan route and /v1/models), tests/subscriptions_e2e.py (new),
tests/gate.sh (adding it).

Work: the servers of the design (§Fake servers), strict like the real
route (they reject what OpenAI rejects, with OpenAI's error shapes); e2e:
`bise login chatgpt --no-browser` with the fake authorize page, a turn on
`chatgpt/<model>` through the hub on a temp HOME, the token expiring
mid-session and refreshing, two agents refreshing at once, a usage limit
mid-stream, `invalid_grant` -> the expired line, logout with revoke;
OpenRouter sign-in -> a key in auth.json -> a turn; the coding-plan
providers against fake_provider; detection on fake ~/.codex and
~/.claude files.

Done when: the e2e passes against main with A, B, C landed, runs in the
gate's e2e group, and never touches a real account or the real HOME.
Starts now on the servers (the contract is in the design).
