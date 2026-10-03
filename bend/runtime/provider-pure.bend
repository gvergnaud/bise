# runtime/provider-pure.bend - the pure half of runtime/provider.bend:
# the provider table and the retry policy the laws pin. No IO, no
# @unsafe and no foreign code in its closure (the PROOF gate refuses any
# in the import closure since Bend 2.0.32) - so no Http types either:
# runtime/provider.bend maps Http.Err onto NetErr and a response onto
# (status, Retry-After, mapped reply) before asking for a verdict.
import Base
import ../core/api.bend as Api
import ../core/wire.bend as W
import ../core/text.bend as Tx
import ../core/config.bend as Cf

# ---- the provider registry ----
# A model id is "provider/model" (the model may hold '/' itself:
# "openrouter/anthropic/claude-x"). A provider carries its family
# (`api`, core/api.bend: every per-family fact lives there), a base_url
# and the env var of its key (`key_env`, "" = no key: a local server).
#
# The entries come from the models file Rust writes before it spawns
# the REPL (BISE_MODELS_FILE: bise's built-in catalog merged with the
# user's config.toml, rust/catalog, BISE-142), over the entries built
# in below (so an old binary without the file keeps today's two
# setups). The file's shape (core/config.bend paths, quotes kept):
#   default_model = "<p>/<id>"   (none built in, BISE-266)
#   [aliases]            "opus-5.5" = "foundry/claude-opus-5-5"
#   [providers.<p>]      api, base_url, key_env, needs (+ limits)
#   [models."<p>/<id>"]  only the keys that differ from its provider
# A key of a model is models."p/id".<k>, else providers.p.<k>.
#
# The model of a call (the same order as the Rust side, rust/catalog's
# Setup; an empty env var counts as unset):
#   main:  BISE_MODEL > BEND_MODEL > [roles] main (config.toml) > model
#          > default_model > none: no call, the error says to pick a
#          provider (BISE-266)
#   agent: BISE_AGENT_MODEL > [roles] agents > agent_model > the main
#          order
# ([roles] main = "p/m", or [roles.main] model = "p/m": BISE-298)
# (agent: BISE_ROLE=agent, the hub's sub-agent REPLs). A name without
# '/' is an alias, else the legacy guess: claude* -> foundry/<name>,
# anything else -> mistral/<name>.

def builtin() -> List<&2, Cf.KV>:
  [Cf.KV{"aliases.\"opus-5.5\"", "foundry/claude-opus-5-5"},
    Cf.KV{"providers.foundry.api", "anthropic"},
    Cf.KV{"providers.foundry.key_env", "ANTHROPIC_FOUNDRY_API_KEY"},
    Cf.KV{"providers.foundry.reasoning", "true"},
    Cf.KV{"providers.foundry.context", "1000000"},
    Cf.KV{"providers.mistral.api", "openai-chat"},
    Cf.KV{"providers.mistral.base_url", "https://api.mistral.ai/v1"},
    Cf.KV{"providers.mistral.key_env", "MISTRAL_API_KEY"},
    Cf.KV{"providers.mistral.reasoning", "true"},
    Cf.KV{"providers.mistral.context", "128000"},
    Cf.KV{"providers.mistral.cache_key", "prompt_cache_key"}]

# the models file's entries win over the built-in ones (first match)
def registry(+file: String) -> List<&2, Cf.KV>:
  List.append(&2, Cf.KV, Cf.parse_toml(file), builtin())

# ---- the model name ----

def first_set.pick(empty: Bool, +x: String, +later: String) -> String:
  match empty:
    case True{}:
      later
    case False{}:
      x

# the first non-empty string of the list ("" when none is)
def first_set(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+x, +rest}:
      first_set.pick(String.is_empty(x), x, first_set(rest))

def cfg_str(kvs: List<&2, Cf.KV>, +key: String) -> String:
  Cf.cfg_or(Cf.cfg_find(kvs, key), "")

def line_model(is_model: Bool, +v: String) -> String:
  match is_model:
    case True{}:
      v
    case False{}:
      ""

# a role's setting in config.toml (BISE-298): `[roles.<r>] <k> = ...`,
# or for the model the line form `[roles] <r> = ...`
def role_str(+cfg: List<&2, Cf.KV>, +r: String, +k: String) -> String:
  +table = cfg_str(cfg, String.concat(["roles.", r, ".", k]))
  first_set([table, line_model(String.eq(k, "model"), cfg_str(cfg, String.append("roles.", r)))])

def agent_only(is_agent: Bool, +v: String) -> String:
  match is_agent:
    case True{}:
      v
    case False{}:
      ""

# the name the user asked for, before any alias or provider lookup
def model_name(+bise: String, +bend: String, +agent: String, +role: String,
    +cfg: List<&2, Cf.KV>, +reg: List<&2, Cf.KV>) -> String:
  +is_agent = String.eq(role, "agent")
  first_set([agent_only(is_agent, agent),
    agent_only(is_agent, role_str(cfg, "agents", "model")),
    agent_only(is_agent, cfg_str(cfg, "agent_model")), bise, bend,
    role_str(cfg, "main", "model"), cfg_str(cfg, "model"),
    cfg_str(reg, "default_model")])

# ---- the full id: provider/model ----

def has_slash(+s: String) -> Bool:
  String.contains(s, "/")

def legacy_id.pick(is_claude: Bool, +name: String) -> String:
  match is_claude:
    case True{}:
      String.append("foundry/", name)
    case False{}:
      String.append("mistral/", name)

# a bare name from before the registry: the family was guessed from it
def legacy_id(+name: String) -> String:
  legacy_id.pick(String.starts_with(name, "claude"), name)

def bare_id.pick(slash: Bool, +name: String) -> String:
  match slash:
    case True{}:
      name
    case False{}:
      legacy_id(name)

def bare_id(+name: String) -> String:
  bare_id.pick(has_slash(name), name)

def alias_of(m: Maybe<&2, String>, +name: String) -> String:
  match m:
    case Some{+a}:
      bare_id(a)
    case None{}:
      legacy_id(name)

def alias_key(+name: String) -> String:
  String.concat(["aliases.\"", name, "\""])

def full_id.pick(slash: Bool, +name: String, +reg: List<&2, Cf.KV>)
    -> String:
  match slash:
    case True{}:
      name
    case False{}:
      alias_of(Maybe.or(&2, String, Cf.cfg_find(reg, alias_key(name)),
        Cf.cfg_find(reg, String.append("aliases.", name))), name)

def full_id(+name: String, +reg: List<&2, Cf.KV>) -> String:
  full_id.pick(has_slash(name), name, reg)

# "p/a/b" -> ("p", "a/b"): the provider is the text before the FIRST '/'
def split_id.parts(parts: List<&2, String>) -> String & String:
  match parts:
    case Nil{}:
      ("", "")
    case Con{+p, +rest}:
      (p, String.join(rest, "/"))

def split_id(+id: String) -> String & String:
  split_id.parts(String.split(id, '/'))

# ---- a key of a model: its own, else its provider's ----

def model_key(+reg: List<&2, Cf.KV>, +prov: String, +id: String,
    +k: String) -> Maybe<&2, String>:
  Maybe.or(&2, String,
    Cf.cfg_find(reg, String.concat(["models.\"", id, "\".", k])),
    Cf.cfg_find(reg, String.concat(["providers.", prov, ".", k])))

# the prompt cache's routing key of a model (BISE-268, core/api.bend
# session_key): (the body field, the header) the catalog names for it,
# "" = none; `id` is the full "provider/model"
type CNames is Data:
  CN{field: String, header: String}

def cache_names(+file: String, +prov: String, +id: String) -> CNames:
  +reg = registry(file)
  CN{Cf.cfg_or(model_key(reg, prov, id, "cache_key"), ""),
    Cf.cfg_or(model_key(reg, prov, id, "cache_header"), "")}

def cache_field(+c: CNames) -> String:
  match c:
    case CN{+f, _}:
      f

def cache_header(+c: CNames) -> String:
  match c:
    case CN{_, +h}:
      h

# a gateway (Claude Code's ANTHROPIC_CUSTOM_HEADERS and apiKeyHelper):
# the variable holding extra headers (headers_env) and the command that
# prints the key (key_command), "" = none; `id` is the full
# "provider/model"
def headers_env(+file: String, +prov: String, +id: String) -> String:
  Cf.cfg_or(model_key(registry(file), prov, id, "headers_env"), "")

def key_command(+file: String, +prov: String, +id: String) -> String:
  Cf.cfg_or(model_key(registry(file), prov, id, "key_command"), "")

# ---- the resolution ----

# what a call needs: the provider id, the model id sent on the wire,
# the family, the URL, the env var holding the key ("": none) and what
# the catalog says of the model (Api.MFacts)
type Prov is Data:
  Prov{provider: String, model: String, st: Api.Style, url: String,
    key_env: String, facts: Api.MFacts}

# an error names the full id it was about (what the clients show)
type Resolved is Data:
  ROk{p: Prov}
  RErr{id: String, msg: String}

def strip_slash.pick(ends: Bool, +s: String) -> String:
  match ends:
    case True{}:
      String.take(s, Nat.sub(String.length(s), 1n))
    case False{}:
      s

def strip_slash(+s: String) -> String:
  strip_slash.pick(String.ends_with(s, "/"), s)

# BEND_PROVIDER_URL=<url> points the call elsewhere (a local fault
# injector in the tests); unset, base_url + the family's endpoint
def call_url.pick(empty: Bool, +env: String, +base: String, st: Api.Style)
    -> String:
  match empty:
    case True{}:
      String.append(strip_slash(base), Api.endpoint(st))
    case False{}:
      env

def call_url(+env: String, +base: String, st: Api.Style) -> String:
  call_url.pick(String.is_empty(env), env, base, st)

def quoted(+s: String) -> String:
  String.concat(["\"", s, "\""])

def unknown_provider(+prov: String, +id: String) -> Resolved:
  RErr{id, String.concat(["unknown provider ", quoted(prov), " (model ",
    quoted(id), "): add a [providers.", prov,
    "] table (base_url, key_env, api) to config.toml"])}

# a provider bise knows but ships without a URL (foundry: a private
# proxy, its users set base_url in their config.toml, or the variable
# the models file names, base_url_env: ANTHROPIC_FOUNDRY_BASE_URL)
def no_base_url.how(none: Bool, +env: String, +prov: String) -> String:
  match none:
    case True{}:
      String.concat(["add it under [providers.", prov,
        "] in ~/.bise/config.toml"])
    case False{}:
      String.concat(["set ", env, ", or add it under [providers.", prov,
        "] in ~/.bise/config.toml"])

def no_base_url.pick(unknown: Bool, +prov: String, +id: String,
    +how: String) -> Resolved:
  match unknown:
    case True{}:
      unknown_provider(prov, id)
    case False{}:
      RErr{id, String.concat(["provider ", quoted(prov), " (model ",
        quoted(id), ") has no base_url: ", how])}

def no_base_url(+reg: List<&2, Cf.KV>, +prov: String, +id: String)
    -> Resolved:
  +env = Cf.cfg_or(model_key(reg, prov, id, "base_url_env"), "")
  no_base_url.pick(
    String.is_empty(Cf.cfg_or(model_key(reg, prov, id, "api"), "")), prov,
    id, no_base_url.how(String.is_empty(env), env, prov))

# ---- the model's facts (core/api.bend MFacts) ----

def fact_nat.read(m: Maybe<&2, Nat>) -> Nat:
  match m:
    case Some{+n}:
      n
    case None{}:
      0n

def fact_nat(m: Maybe<&2, String>) -> Nat:
  match m:
    case Some{+s}:
      fact_nat.read(Nat.read(s))
    case None{}:
      0n

def max_key.pick(is_openai: Bool) -> String:
  match is_openai:
    case True{}:
      "max_completion_tokens"
    case False{}:
      "max_tokens"

# ---- the reasoning effort (BISE-135) ----
# The words a model takes: its `efforts` key (comma-separated, the
# models file has them normalized by Rust), else its family's; none
# when it does not reason (an Anthropic one with thinking = "none"
# included). The same rule as rust/catalog's Resolved::efforts.

def anth_efforts() -> List<&2, String>:
  ["none", "low", "medium", "high", "max"]

def chat_efforts() -> List<&2, String>:
  ["none", "low", "medium", "high"]

def efforts.family(is_anth: Bool) -> List<&2, String>:
  match is_anth:
    case True{}:
      anth_efforts()
    case False{}:
      chat_efforts()

def efforts.own(empty: Bool, +own: List<&2, String>, is_anth: Bool)
    -> List<&2, String>:
  match empty:
    case True{}:
      efforts.family(is_anth)
    case False{}:
      own

def non_empty(s: String) -> Bool:
  Bool.not(String.is_empty(s))

def efforts.words(+s: String) -> List<&2, String>:
  List.filter(~String, ~non_empty, String.split(s, ','))

def efforts.on(on: Bool, +own: String, is_anth: Bool) -> List<&2, String>:
  match on:
    case True{}:
      +ws = efforts.words(own)
      efforts.own(List.is_empty(&2, String, ws), ws, is_anth)
    case False{}:
      Nil{}

def is_anthropic(+reg: List<&2, Cf.KV>, +prov: String, +id: String) -> Bool:
  String.eq(Cf.cfg_or(model_key(reg, prov, id, "api"), "openai-chat"),
    "anthropic")

def efforts(+reg: List<&2, Cf.KV>, +prov: String, +id: String)
    -> List<&2, String>:
  +is_anth = is_anthropic(reg, prov, id)
  +reasons = String.eq(Cf.cfg_or(model_key(reg, prov, id, "reasoning"), ""),
    "true")
  +off = Bool.and(is_anth,
    String.eq(Cf.cfg_or(model_key(reg, prov, id, "thinking"), ""), "none"))
  efforts.on(Bool.and(reasons, Bool.not(off)),
    Cf.cfg_or(model_key(reg, prov, id, "efforts"), ""), is_anth)

def has_word(+xs: List<&2, String>, +w: String) -> Bool:
  List.contains(~String, ~String.eq, xs, w)

def last_word(xs: List<&2, String>) -> String:
  Maybe.default(&2, String, List.last(&2, String, xs), "")

def effort_default.high(has_high: Bool, +xs: List<&2, String>) -> String:
  match has_high:
    case True{}:
      "high"
    case False{}:
      last_word(xs)

def effort_default.pick(has_own: Bool, +own: String, +xs: List<&2, String>)
    -> String:
  match has_own:
    case True{}:
      own
    case False{}:
      effort_default.high(has_word(xs, "high"), xs)

# the effort nobody picked: the model's `effort` key when it takes it,
# else high, else the last word of its list ("" when it takes none)
def effort_default(+xs: List<&2, String>, +own: String) -> String:
  effort_default.pick(has_word(xs, own), own, xs)

def effort_pick(has: Bool, +asked: String, +dflt: String) -> String:
  match has:
    case True{}:
      asked
    case False{}:
      dflt

# the effort of a call: the word asked when the model takes it, else
# its default
def effort_of(+reg: List<&2, Cf.KV>, +prov: String, +id: String,
    +asked: String) -> String:
  +xs = efforts(reg, prov, id)
  effort_pick(has_word(xs, asked), asked,
    effort_default(xs, Cf.cfg_or(model_key(reg, prov, id, "effort"), "")))

# the request rules of a provider (`shape`, the catalog's: subs-auth
# writes providers.chatgpt.shape = "chatgpt-plan"): only that exact
# word is the ChatGPT plan route; absent or anything else, today's body
def is_plan_shape(+shape: String) -> Bool:
  String.eq(shape, "chatgpt-plan")

def facts(+reg: List<&2, Cf.KV>, +prov: String, +id: String, +asked: String)
    -> Api.MFacts:
  Api.MFacts{String.eq(Cf.cfg_or(model_key(reg, prov, id, "reasoning"), ""),
      "true"),
    fact_nat(model_key(reg, prov, id, "max_output")),
    max_key.pick(String.eq(prov, "openai")),
    Cf.cfg_or(model_key(reg, prov, id, "thinking"), ""),
    Cf.cfg_or(model_key(reg, prov, id, "betas"), ""),
    effort_of(reg, prov, id, asked),
    is_plan_shape(Cf.cfg_or(model_key(reg, prov, id, "shape"), ""))}

def resolve.family(m: Maybe<&2, Api.Style>, +api: String, +prov: String,
    +model: String, +id: String, +base: String, +key_env: String,
    +url_env: String, +fs: Api.MFacts) -> Resolved:
  match m:
    case Some{+st}:
      ROk{Prov{prov, model, st, call_url(url_env, base, st), key_env, fs}}
    case None{}:
      RErr{id, String.concat(["model ", quoted(id), ": api ", quoted(api),
        " of provider ", quoted(prov), " is not supported yet"])}

def resolve.needs(ok: Bool, +needs: String, +api: String, +prov: String,
    +model: String, +id: String, +base: String, +key_env: String,
    +url_env: String, +fs: Api.MFacts) -> Resolved:
  match ok:
    case True{}:
      resolve.family(Api.style_of_api(api), api, prov, model, id, base,
        key_env, url_env, fs)
    case False{}:
      RErr{id, String.concat(["model ", quoted(id), ": provider ", quoted(prov),
        " is not supported yet (needs ", needs, ")"])}

# a base_url = "" is no base_url (an older models file wrote it): the
# error that says where to set it, never a call to "/messages"
def resolve.base.set(empty: Bool, +base: String, +reg: List<&2, Cf.KV>,
    +prov: String, +model: String, +id: String, +url_env: String,
    +asked: String) -> Resolved:
  match empty:
    case True{}:
      no_base_url(reg, prov, id)
    case False{}:
      +needs = Cf.cfg_or(model_key(reg, prov, id, "needs"), "")
      resolve.needs(String.is_empty(needs), needs,
        Cf.cfg_or(model_key(reg, prov, id, "api"), "openai-chat"), prov,
        model, id, base, Cf.cfg_or(model_key(reg, prov, id, "key_env"), ""),
        url_env, facts(reg, prov, id, asked))

def resolve.base(m: Maybe<&2, String>, +reg: List<&2, Cf.KV>, +prov: String,
    +model: String, +id: String, +url_env: String, +asked: String)
    -> Resolved:
  match m:
    case Some{+base}:
      resolve.base.set(String.is_empty(base), base, reg, prov, model, id,
        url_env, asked)
    case None{}:
      no_base_url(reg, prov, id)

def resolve.split(pm: String & String, +reg: List<&2, Cf.KV>, +id: String,
    +url_env: String, +asked: String) -> Resolved:
  match pm:
    case (+prov, +model):
      resolve.base(model_key(reg, prov, id, "base_url"), reg, prov, model,
        id, url_env, asked)

# a full id against the registry, with the effort asked ("": none)
def resolve_id_effort(+id: String, +reg: List<&2, Cf.KV>, +url_env: String,
    +asked: String) -> Resolved:
  resolve.split(split_id(id), reg, id, url_env, asked)

# a full id against the registry
def resolve_id(+id: String, +reg: List<&2, Cf.KV>, +url_env: String)
    -> Resolved:
  resolve_id_effort(id, reg, url_env, "")

# ---- the session's choice (BISE-135) ----
# /model and /reasoning write the session's choice file (its path in
# BISE_SESSION_CHOICE, rust/catalog Choice): `model = "..."`,
# `reasoning_effort = "..."`. It comes before every other source:
#   model:  choice > the role's order above
#   effort: choice > agent_reasoning_effort (agent) > reasoning_effort
#           (config.toml) > the model's default; a word the model does
#           not take falls to its default (effort_of)

def chosen_name(+choice: List<&2, Cf.KV>, +role_name: String) -> String:
  first_set([cfg_str(choice, "model"), role_name])

def effort_asked(+choice: List<&2, Cf.KV>, +role: String,
    +cfg: List<&2, Cf.KV>) -> String:
  +is_agent = String.eq(role, "agent")
  first_set([cfg_str(choice, "reasoning_effort"),
    agent_only(is_agent, role_str(cfg, "agents", "effort")),
    agent_only(is_agent, cfg_str(cfg, "agent_reasoning_effort")),
    role_str(cfg, "main", "effort"),
    cfg_str(cfg, "reasoning_effort")])

# no model anywhere (BISE-266): what the turn says, no call made
def NO_MODEL() -> String:
  "no model yet: run bise and pick a provider (it checks your key), or set model in ~/.bise/config.toml"

def resolve_named(empty: Bool, +name: String, +reg: List<&2, Cf.KV>,
    +url_env: String, +asked: String) -> Resolved:
  match empty:
    case True{}:
      RErr{"", NO_MODEL()}
    case False{}:
      resolve_id_effort(full_id(name, reg), reg, url_env, asked)

# THE resolution with the session's choice (its file's text, "" when
# none), pure
def resolve_chosen(+choice: String, +bise: String, +bend: String,
    +agent: String, +role: String, +cfg: String, +file: String,
    +url_env: String) -> Resolved:
  +reg = registry(file)
  +ch = Cf.parse_toml(choice)
  +ckv = Cf.parse_toml(cfg)
  +name = chosen_name(ch, model_name(bise, bend, agent, role, ckv, reg))
  resolve_named(String.is_empty(name), name, reg, url_env,
    effort_asked(ch, role, ckv))

# THE resolution, pure: the env values (BISE_MODEL, BEND_MODEL,
# BISE_AGENT_MODEL, BISE_ROLE, BEND_PROVIDER_URL), the user's
# config.toml text and the models file text ("" when absent); no
# session choice
def resolve(+bise: String, +bend: String, +agent: String, +role: String,
    +cfg: String, +file: String, +url_env: String) -> Resolved:
  resolve_chosen("", bise, bend, agent, role, cfg, file, url_env)

# the full id of a resolved call, "provider/model" (BISE-150: the name
# the clients look up in bise's catalog)
def prov_name(p: Prov) -> String:
  match p:
    case Prov{+provider, +model, _, _, _, _}:
      String.concat([provider, "/", model])

# the model the clients show (the harness-info line): the full id,
# resolved or not
def shown_model(r: Resolved) -> String:
  match r:
    case ROk{+p}:
      prov_name(p)
    case RErr{+id, _}:
      id

# the reasoning effort of a resolved call ("": none, BISE-135)
def shown_effort(r: Resolved) -> String:
  match r:
    case ROk{+p}:
      match p:
        case Prov{_, _, _, _, _, +fs}:
          Api.facts_effort(fs)
    case RErr{_, _}:
      ""

# ---- the context window and the compaction threshold (BISE-150) ----
# The window of the model a REPL starts with: its `context` in the
# models file, else its provider's; a model nothing describes gets
# DEFAULT_CONTEXT (rust/catalog's DEFAULT_CAPS). The compaction starts
# at 80 % of it unless BEND_THRESHOLD or config `compaction_threshold`
# says, and never later (BISE-300).

def DEFAULT_CONTEXT() -> U32:
  128000

def u32_or.read(m: Maybe<&2, U32>) -> U32:
  match m:
    case Some{+n}:
      n
    case None{}:
      0

# a number key of the registry; missing or not a number: 0
def u32_or(m: Maybe<&2, String>) -> U32:
  match m:
    case Some{+s}:
      u32_or.read(U32.read(s))
    case None{}:
      0

def window.known(zero: Bool, +n: U32) -> U32:
  match zero:
    case True{}:
      DEFAULT_CONTEXT()
    case False{}:
      n

def window.split(pm: String & String, +reg: List<&2, Cf.KV>, +id: String)
    -> U32:
  match pm:
    case (+prov, _):
      +n = u32_or(model_key(reg, prov, id, "context"))
      window.known(U32.is_eq(n, 0), n)

# the context window, in tokens, of the model the same inputs as
# `resolve` pick (never 0)
def window_chosen(+choice: String, +bise: String, +bend: String,
    +agent: String, +role: String, +cfg: String, +file: String) -> U32:
  +reg = registry(file)
  +id = full_id(chosen_name(Cf.parse_toml(choice),
    model_name(bise, bend, agent, role, Cf.parse_toml(cfg), reg)), reg)
  window.split(split_id(id), reg, id)

def window(+bise: String, +bend: String, +agent: String, +role: String,
    +cfg: String, +file: String) -> U32:
  window_chosen("", bise, bend, agent, role, cfg, file)

# 80 % of a window, rounded down (no overflow: n / 5 first)
def threshold_of(+n: U32) -> U32:
  ((n / 5) * 4 + ((n % 5) * 4) / 5 : U32)

# ---- the threshold as written (BISE-300) ----
# a number of tokens ("450000") or a share of the window ("45%"); 0,
# empty or anything else: None (the default applies)
def thr_value.some(zero: Bool, +n: U32) -> Maybe<&2, U32>:
  match zero:
    case True{}:
      None{}
    case False{}:
      Some{n}

def thr_value.num(m: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match m:
    case Some{+n}:
      thr_value.some(U32.is_eq(n, 0), n)
    case None{}:
      None{}

# p % of a window, rounded down; p above 100 counts as 100 (no
# overflow: window / 100 first)
def pct_of(+p: U32, +window: U32) -> U32:
  +q = U32.min(p, 100)
  ((window / 100) * q + ((window % 100) * q) / 100 : U32)

def thr_value.pct(m: Maybe<&2, U32>, +window: U32) -> Maybe<&2, U32>:
  match m:
    case Some{+p}:
      thr_value.num(Some{pct_of(p, window)})
    case None{}:
      None{}

def thr_value.split(pct: Bool, +t: String, +window: U32) -> Maybe<&2, U32>:
  match pct:
    case True{}:
      thr_value.pct(U32.read(String.trim(String.take(t,
        Nat.sub(String.length(t), 1n)))), window)
    case False{}:
      thr_value.num(U32.read(t))

def thr_value(+s: String, +window: U32) -> Maybe<&2, U32>:
  +t = String.trim(s)
  thr_value.split(String.ends_with(t, "%"), t, window)

# a value, capped at 80 % of the window; none: that 80 %. A number
# written for a 1M model never overflows a 200k one (an agent on a
# smaller model, BISE-298's roles).
def thr_capped(m: Maybe<&2, U32>, +cap: U32) -> U32:
  match m:
    case Some{+n}:
      U32.min(n, cap)
    case None{}:
      cap

def threshold.pick(empty: Bool, +envv: String, +cfgt: String,
    +window: U32) -> U32:
  match empty:
    case True{}:
      thr_capped(thr_value(Cf.cfg_or(Cf.cfg_get(cfgt,
        "compaction_threshold"), ""), window), threshold_of(window))
    case False{}:
      thr_capped(thr_value(envv, window), threshold_of(window))

# the compaction threshold: BEND_THRESHOLD (envv) > config.toml
# `compaction_threshold` > 80 % of the model's window; either a number
# of tokens or "45%" of the window, and never above 80 % of it
def threshold(+envv: String, +cfgt: String, +window: U32) -> U32:
  threshold.pick(String.is_empty(envv), envv, cfgt, window)

# ---- the threshold in the Core's units ----
# The threshold the user writes (and the 80 % cap) counts the
# provider's tokens; the Core compares it with its own estimate, four
# bytes a token (core/estimate.bend), and providers count about three:
# on main's history Anthropic counted 1.28x the estimate (507700 real
# input tokens at an estimate of 396485; 450607 real at 343760). Given
# as is, a 450000 threshold never fired and main ran past 500k. The
# Core gets three quarters of it: 337500 for 450000, crossed at about
# 450k real. On lighter text it compacts a little early, never late.
def est_units(+n: U32) -> U32:
  ((n / 4) * 3 + ((n % 4) * 3) / 4 : U32)

def interrupt_path_of(+dir: String, +port: String) -> String:
  String.append(dir, String.append("/bend-interrupt-", String.append(port, ".txt")))

# the interrupt flag file: the TUI writes "1" on Ctrl+C, the runtime
# clears it to "" at every turn end. Only a non-empty flag is a request
# (an empty one read as a Ctrl+C turned every retry after the first
# turn into "interrupted by the user").
def interrupt_requested(+c: String) -> Bool:
  Bool.not(String.is_empty(String.trim(c)))

# who an interrupt flag names: the hub writes who asked for the stop
# ("user": Ctrl+C or esc in the TUI; "main" or a task's name: `sb
# interrupt`; "bise": a stop of the hub's own). "1" (an older hub) and
# an unreadable flag read as the user.
def interrupter.of(+t: String) -> String:
  Bool.pick(String, String.is_empty(t) || String.eq(t, "1") || String.eq(t, "user"),
    "the user", t)

def interrupter(+c: String) -> String:
  interrupter.of(String.trim(c))

# the same, from the flag file as read (no file: the user)
def interrupter_in(m: Maybe<&2, String>) -> String:
  match m:
    case None{}:
      "the user"
    case Some{+c}:
      interrupter(c)

# the error of a call an interrupt stopped: "interrupted by main"
def interrupted_by(+who: String) -> String:
  String.append("interrupted by ", who)

# ---- a transport failure, without the Http types ----

type NetErr is Data:
  NUrl{}
  NDns{}
  NConnect{code: U32, what: String}
  NTls{code: U32, what: String}
  NRead{code: U32, what: String}
  NWrite{code: U32, what: String}
  NTimeout{}
  NRedirect{}
  NBad{}

def coded(+label: String, +c: U32, +w: String) -> String:
  String.append(label, String.append(U32.show(c), String.append(" ", w)))

def api_err(e: NetErr) -> String:
  match e:
    case NUrl{}:
      "bad url"
    case NDns{}:
      "dns"
    case NConnect{+c, +w}:
      coded("connect ", c, w)
    case NTls{+c, +w}:
      coded("tls ", c, w)
    case NRead{+c, +w}:
      coded("read ", c, w)
    case NWrite{+c, +w}:
      coded("write ", c, w)
    case NTimeout{}:
      "timeout"
    case NRedirect{}:
      "too many redirects"
    case NBad{}:
      "bad response"

# ---- what a failure means for the user ----
# The short code (api_err) is kept inside the text: it is what a bug
# report needs. The text around it names the host the call went to and
# the one thing the user can check. English, one line.

# "https://host:port/path" -> "host:port"; anything else is itself
def url_host.head(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, _}:
      h

def url_host.rest(+rest: String) -> String:
  url_host.head(String.split(rest, '/'))

def url_host.http(is_http: Bool, +url: String) -> String:
  match is_http:
    case True{}:
      url_host.rest(String.drop(url, 7n))
    case False{}:
      url_host.rest(url)

def url_host.pick(is_https: Bool, +url: String) -> String:
  match is_https:
    case True{}:
      url_host.rest(String.drop(url, 8n))
    case False{}:
      url_host.http(String.starts_with(url, "http://"), url)

def url_host(+url: String) -> String:
  url_host.pick(String.starts_with(url, "https://"), url)

def check_network() -> String:
  " — check your network or VPN"

def net_cause(+host: String, e: NetErr) -> String:
  match e:
    case NUrl{}:
      String.concat(["bad provider URL (", host,
        ") — check BEND_PROVIDER_URL or the provider table"])
    case NDns{}:
      String.concat(["cannot resolve ", host, " (dns)", check_network()])
    case NConnect{+c, +w}:
      String.concat(["cannot reach ", host, " (", coded("connect ", c, w),
        ")", check_network()])
    case NTls{+c, +w}:
      String.concat(["TLS handshake with ", host, " failed (",
        coded("tls ", c, w), ")", check_network()])
    case NRead{+c, +w}:
      String.concat(["connection to ", host, " lost while reading (",
        coded("read ", c, w), ")", check_network()])
    case NWrite{+c, +w}:
      String.concat(["connection to ", host, " lost while sending (",
        coded("write ", c, w), ")", check_network()])
    case NTimeout{}:
      # the limit depends on the call: stream_step_ms between two
      # events of a stream, step_timeout_ms for a whole reply
      String.concat(["no answer from ", host, " (timeout)"])
    case NRedirect{}:
      String.concat(["too many redirects from ", host])
    case NBad{}:
      String.concat(["garbled response from ", host])

# a final HTTP error: the status and the body the style mapper kept,
# plus what to check when the status says it
def status_hint(+s: U32) -> String:
  Bool.pick(String, Bool.or(U32.is_eq(s, 401), U32.is_eq(s, 403)),
    " — check the API key",
    Bool.pick(String, U32.is_eq(s, 404),
      " — check the model name and the provider URL",
      Bool.pick(String, U32.is_eq(s, 413), " — the request is too large",
        "")))

# ---- the interrupt flag, as the retry loop reads it ----
# The flag file is CLEARED by writing "" (every turn end does it), so an
# existing empty file is the normal state: only content means "stop"
# (interrupt_requested, the one predicate). No file: no request.
def interrupt_flag(m: Maybe<&2, String>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+c}:
      interrupt_requested(c)

# ---- the retry policy ----
# Nothing a provider answers may kill the turn at the first blow:
# connection loss (connect, DNS, timeout, a read/write cut mid-response,
# a garbled response), the transient statuses (408, 409, 425, 429 and
# every 5xx - 529 is Anthropic's "overloaded") and a 200 that carries an
# error body (the proxy answers overloads that way) all retry, up to
# max_attempts in total, with an exponential backoff (1s, 2s, 4s, 8s,
# 16s, then 30s) - or the server's Retry-After when it sends one
# (capped at 60s). Everything else (auth, validation, a bad URL, TLS)
# fails the turn immediately: retrying it only wastes the user's time.

# the verdict on one attempt: the mapped reply, a final failure, or a
# retry after a pause (0 = no server hint, the backoff decides)
type Attempt is Data:
  AOk{reply: String}
  AFail{why: String}
  ARetry{after_ms: U32, why: String}

def max_attempts() -> Nat:
  10n

# how long one read may wait for data. A non-streaming reply sends
# NOTHING until the whole generation is done: measured on opus-5.5, a
# 32k-token reply arrives after 255 s (first byte = last byte). At
# 120 s every long reply timed out, and the retries replayed the same
# doomed request. 600 s is Anthropic's own non-streaming ceiling.
def step_timeout_ms() -> U32:
  600000

# a streamed reply keeps data coming (the first event in ~1 s, the
# largest gap measured over a 100 s opus-5.5 reply: 5 s), so a read
# that waits 90 s is a dead connection, not a long answer
def stream_step_ms() -> U32:
  90000

def retryable_err(e: NetErr) -> Bool:
  match e:
    case NConnect{_, _}:
      True{}
    case NDns{}:
      True{}
    case NTimeout{}:
      True{}
    case NRead{_, _}:
      True{}
    case NWrite{_, _}:
      True{}
    case NBad{}:
      True{}
    case _:
      False{}

def retryable_status(+s: U32) -> Bool:
  Bool.or(U32.is_le(500, s),
    Bool.or(U32.is_eq(s, 429),
      Bool.or(U32.is_eq(s, 408),
        Bool.or(U32.is_eq(s, 409), U32.is_eq(s, 425)))))

# the Retry-After header in seconds -> ms, capped at 60s; absent or
# not a number -> 0 (the backoff decides)
def retry_after.of(m: Maybe<&2, U32>) -> U32:
  match m:
    case None{}:
      0
    case Some{+secs}:
      U32.min(U32.mul(secs, 1000), 60000)

def retry_after_ms(+h: String) -> U32:
  retry_after.of(U32.read(String.trim(h)))

# the pause before retry number n (0-based): 1s doubling up to 30s
def backoff_ms(n: Nat) -> U32:
  match n:
    case 0n:
      1000
    case 1n:
      2000
    case 2n:
      4000
    case 3n:
      8000
    case 4n:
      16000
    case _:
      30000

def retry_delay_ms.pick(no_hint: Bool, +hint: U32, n: Nat) -> U32:
  match no_hint:
    case True{}:
      backoff_ms(n)
    case False{}:
      hint

# the server's hint when it gave one, else the backoff
def retry_delay_ms(+hint: U32, n: Nat) -> U32:
  retry_delay_ms.pick(U32.is_eq(hint, 0), hint, n)

def first_line(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+l, _}:
      l

# a 200 whose body is an error ("ERROR provider 200: Overloaded") is a
# transient failure too
def attempt_body(is_err_body: Bool, +hint: U32, +reply: String) -> Attempt:
  match is_err_body:
    case True{}:
      ARetry{hint,
        String.drop(first_line(String.split(String.trim(reply), Tx.newline())),
          6n)}
    case False{}:
      AOk{reply}

# a final HTTP error (not 200, not transient): the turn fails on its
# first line, with the hint for the status
def attempt_final(is_err: Bool, +status: U32, +hint: U32, +reply: String)
    -> Attempt:
  match is_err:
    case True{}:
      AFail{String.append(
        String.drop(first_line(String.split(String.trim(reply), Tx.newline())),
          6n),
        status_hint(status))}
    case False{}:
      attempt_body(String.starts_with(reply, "ERROR provider 200:"), hint,
        reply)

def attempt_status.pick(retryable: Bool, +status: U32, +hint: U32,
    +reply: String) -> Attempt:
  match retryable:
    case True{}:
      ARetry{hint, String.append("provider ",
        String.append(U32.show(status), " (transient)"))}
    case False{}:
      attempt_final(Bool.and(Bool.not(U32.is_eq(status, 200)),
          String.starts_with(reply, "ERROR provider ")),
        status, hint, reply)

# a response: its status, its Retry-After header, and the reply the
# style mapper made of its body
def attempt_status(+status: U32, +retry_after: String, +reply: String)
    -> Attempt:
  attempt_status.pick(retryable_status(status), status,
    retry_after_ms(retry_after), reply)

# ---- the attempt as the Core and the user read it (BISE-293) ----
# The provider's display name (providers.<p>.name of the catalog),
# "" when it has none.
def prov_display(+file: String, +prov: String) -> String:
  Cf.cfg_or(Cf.cfg_find(registry(file),
    String.concat(["providers.", prov, ".name"])), "")

def refusal.who(+name: String) -> String:
  Bool.pick(String, String.is_empty(name), "the provider", name)

# bise's line of a refused request: who refused, the status, what to
# check (the designer's words, BISE-282's pattern)
def refusal.head(+who: String, +status: U32) -> String:
  +s = U32.show(status)
  Bool.pick(String, Bool.or(U32.is_eq(status, 401), U32.is_eq(status, 403)),
    String.concat([who, " refused the key (", s, "). check it with /setup."]),
    Bool.pick(String, U32.is_eq(status, 404),
      String.concat([who, " refused the request (", s,
        "). check the model name and the provider URL."]),
      Bool.pick(String, U32.is_eq(status, 413),
        String.concat([who, " refused the request (", s,
          "). the request is too large."]),
        String.concat([who, " refused the request (", s, ")."]))))

# the key, if the provider echoed it, as ...<last 4>
def mask.at(fuel: Nat, m: Maybe<&2, Nat>, +s: String, +key: String)
    -> String:
  match fuel:
    case 0n:
      s
    case 1n+k:
      match m:
        case None{}:
          s
        case Some{+p}:
          String.concat([String.take(s, p), "…",
            String.drop(key, Nat.sub(String.length(key), 4n)),
            mask.at(k,
              Tx.find(String.drop(s, Nat.add(p, String.length(key))), key, 0n),
              String.drop(s, Nat.add(p, String.length(key))), key)])

def mask.pick(short: Bool, +s: String, +key: String) -> String:
  match short:
    case True{}:
      s
    case False{}:
      mask.at(8n, Tx.find(s, key, 0n), s, key)

def mask(+s: String, +key: String) -> String:
  mask.pick(Nat.is_lt(String.length(key), 12n), s, key)

# the provider's words, ~200 chars at most
def said_cut(+s: String) -> String:
  Bool.pick(String, Nat.is_gt(String.length(s), 200n),
    String.append(String.trim(String.take(s, 200n)), "…"), s)

def refusal.said(empty: Bool, +who: String, +said: String) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      String.concat([" ", who, " said: \"", said, "\""])

# a final HTTP error on one line: bise's words, then the provider's
# quoted ("OpenAI refused the request (400). OpenAI said: "...""); the
# TUI shows the quote dim under bise's line, the whole body stays in
# the debug log
def refusal(+name: String, +status: U32, +body: String, +key: String)
    -> String:
  +who = refusal.who(name)
  +said = said_cut(mask(W.err_said(body), key))
  String.append(refusal.head(who, status),
    refusal.said(String.is_empty(said), who, said))

def attempt_for.fail(refused: Bool, +why: String, +name: String,
    +status: U32, +body: String, +key: String) -> Attempt:
  match refused:
    case True{}:
      AFail{refusal(name, status, body, key)}
    case False{}:
      AFail{why}

# a reply's calls get their harness names back through the request's
# tools; a refusal (an HTTP status, not 200) reads as bise's line and
# the provider's words; a transport failure and a retry stay
def attempt_for(+tools: List<&2, W.WTool>, +name: String, +key: String,
    +status: U32, +body: String, a: Attempt) -> Attempt:
  match a:
    case AOk{+reply}:
      AOk{W.reply_names_in(tools, reply)}
    case AFail{+why}:
      attempt_for.fail(Bool.and(Bool.not(U32.is_eq(status, 0)),
          Bool.not(U32.is_eq(status, 200))), why, name, status, body, key)
    case ARetry{+ms, +why}:
      ARetry{ms, why}

# ---- the ChatGPT plan's failures (shape = "chatgpt-plan") ----
# The designer's lines (docs/subscriptions-design.md, final words §5).
# OpenAI's plan route says a usage limit with a code, in a non-200 JSON
# body (429, 503) or mid-stream in response.failed (a 200): either way
# the turn stops on bise's line, never a retry loop on a limit nor an
# "OpenAI refused the key" on a sign-in that expired.

# subscription_sharing_usage_limit_exceeded: OpenAI says not to infer a
# reset time from it, so the line points at the usage page
def PLAN_LIMIT() -> String:
  "your ChatGPT plan's limit for bise is reached. your usage is at chatgpt.com/settings/usage, or switch model with /model."

# subscription_sharing_user_not_eligible (403): final, no retry
def PLAN_OFF() -> String:
  "ChatGPT plan use is off for bise. turn it on in your ChatGPT settings, or pick another provider in /provider."

# subscription_sharing_usage_unavailable, once the retries are spent
def PLAN_UNCHECKED_LINE() -> String:
  "ChatGPT couldn't check your plan's usage. try again in a moment, or switch model with /model."

def PLAN_EXPIRED() -> String:
  "your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt."

# the retries' cause while ChatGPT cannot check the plan's usage
# (subscription_sharing_usage_unavailable: OpenAI's errors-and-recovery
# says retry later with a bounded backoff); PLAN_UNCHECKED_LINE is said
# only once the retries are spent (exhausted_for)
def PLAN_UNCHECKED() -> String:
  "ChatGPT could not check your plan's usage (subscription_sharing_usage_unavailable)"

# the final line for a reply's status and error code, None: not one of
# the plan's final failures (a usage limit, plan use not allowed, a
# sign-in refused)
def plan_line(+status: U32, +code: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>,
    String.eq(code, "subscription_sharing_usage_limit_exceeded"),
    Some{PLAN_LIMIT()},
    Bool.pick(Maybe<&2, String>,
      String.eq(code, "subscription_sharing_user_not_eligible"),
      Some{PLAN_OFF()},
      Bool.pick(Maybe<&2, String>, U32.is_eq(status, 401),
        Some{PLAN_EXPIRED()}, None{})))

def attempt_plan.line(m: Maybe<&2, String>, a: Attempt) -> Attempt:
  match m:
    case Some{+l}:
      AFail{l}
    case None{}:
      a

# usage unavailable: a transient failure, retried with the usual policy
# (the server's Retry-After when the attempt had one, else the backoff)
def plan_retry(a: Attempt) -> Attempt:
  match a:
    case ARetry{+ms, _}:
      ARetry{ms, PLAN_UNCHECKED()}
    case _:
      ARetry{0, PLAN_UNCHECKED()}

def attempt_plan.pick(unchecked: Bool, +status: U32, +code: String,
    a: Attempt) -> Attempt:
  match unchecked:
    case True{}:
      plan_retry(a)
    case False{}:
      attempt_plan.line(plan_line(status, code), a)

# a call's attempt on a plan provider: a usage limit or a refused
# sign-in ends the turn on its line; usage unavailable is retried; any
# other attempt (or provider) unchanged
def attempt_plan(plan: Bool, +status: U32, +code: String, a: Attempt)
    -> Attempt:
  match plan:
    case True{}:
      attempt_plan.pick(String.eq(code, "subscription_sharing_usage_unavailable"),
        status, code, a)
    case False{}:
      a

# the missing key's line: a plan's token command failed (signed out, the
# refresh refused): its sign-in expired; else the key_command's, else
# the key_env's
def key_why.cmd(plan: Bool, +prov: String) -> String:
  match plan:
    case True{}:
      PLAN_EXPIRED()
    case False{}:
      String.concat(["no ", prov,
        " key: its key_command failed or printed nothing (config.toml)"])

def key_why(has_cmd: Bool, plan: Bool, +prov: String, +key_env: String)
    -> String:
  match has_cmd:
    case True{}:
      key_why.cmd(plan, prov)
    case False{}:
      String.concat(["no ", prov, " key yet (", key_env,
        " is not set): /provider sets it up"])

def attempt_err.pick(retryable: Bool, +host: String, +e: NetErr) -> Attempt:
  match retryable:
    case True{}:
      ARetry{0, net_cause(host, e)}
    case False{}:
      AFail{net_cause(host, e)}

# a transport failure: no response at all. The cause names the host and
# says what the user can do about it (the text every retry line, the
# final error and an interrupted retry loop carry)
def attempt_err(+host: String, +e: NetErr) -> Attempt:
  attempt_err.pick(retryable_err(e), host, e)

# ---- a provider's own wait (slow gateways, local models) ----
# idle_timeout_sec, under [providers.<id>] or a model's table: how long
# one read of the streamed reply may wait for data (the response head,
# then each gap between two events). A local model on a slow machine
# can take minutes to read a long prompt before its first byte; a
# gateway can hold the head while it queues. Unset, 0 or not a number:
# stream_step_ms (90 s), as before. Capped at a day (the ms fit a U32).
# The wait carries the provider id: the timeout text names the key.
type Wait is Data:
  Wait{prov: String, ms: U32}

def idle_max_secs() -> U32:
  86400

def idle_ms.pick(zero: Bool, +secs: U32) -> U32:
  match zero:
    case True{}:
      stream_step_ms()
    case False{}:
      U32.mul(U32.min(secs, idle_max_secs()), 1000)

def idle_ms(+secs: U32) -> U32:
  idle_ms.pick(U32.is_eq(secs, 0), secs)

# `id` is the full "provider/model"
def stream_wait(+file: String, +prov: String, +id: String) -> Wait:
  Wait{prov, idle_ms(u32_or(model_key(registry(file), prov, id,
    "idle_timeout_sec")))}

def wait_ms(+w: Wait) -> U32:
  match w:
    case Wait{_, +ms}:
      ms

# "90 s"; under a second (never from the catalog): "0 s"
def wait_secs(+ms: U32) -> String:
  String.append(U32.show(U32.div(ms, 1000)), " s")

# a read that waited the whole limit: the host, the limit, and the key
# that raises it (the user can't tell a slow model from a dead link;
# the text says what to change when it is the model)
def timeout_cause(+host: String, +w: Wait) -> String:
  match w:
    case Wait{+prov, +ms}:
      String.concat(["no answer from ", host, " in ", wait_secs(ms),
        " (timeout) — a slow model or gateway? raise idle_timeout_sec under [providers.",
        prov, "] in ~/.bise/config.toml"])

def is_timeout(e: NetErr) -> Bool:
  match e:
    case NTimeout{}:
      True{}
    case _:
      False{}

def attempt_err_in.pick(timeout: Bool, +host: String, +w: Wait, +e: NetErr)
    -> Attempt:
  match timeout:
    case True{}:
      ARetry{0, timeout_cause(host, w)}
    case False{}:
      attempt_err(host, e)

# a transport failure of a streamed call: a timeout names its limit and
# idle_timeout_sec (still a retry, as before); the others as attempt_err
def attempt_err_in(+host: String, +w: Wait, +e: NetErr) -> Attempt:
  attempt_err_in.pick(is_timeout(e), host, w, e)

# the line the client shows while the call waits (the TUI renders it
# as a warning): "provider_retry: 2/10 · <why> · retry in 4s"
def retry_line(+n: Nat, +why: String, +delay_ms: U32) -> String:
  String.append("  obs: provider_retry: ",
    String.append(Nat.show(Nat.add(n, 1n)),
      String.append("/",
        String.append(Nat.show(max_attempts()),
          String.append(" · ",
            String.append(String.take(why, 200n),
              String.append(" · retry in ",
                String.append(U32.show(U32.div(delay_ms, 1000)), "s"))))))))

# the final error once every attempt failed: the count and the last cause
def exhausted_text(+tries: Nat, +why: String) -> String:
  String.append(String.concat(["provider failed after ", Nat.show(tries),
    " attempts: "]), why)

# the same, but a ChatGPT plan whose usage could never be checked ends
# on the designer's line alone
def exhausted_for(+tries: Nat, +why: String) -> String:
  Bool.pick(String, String.eq(why, PLAN_UNCHECKED()), PLAN_UNCHECKED_LINE(),
    exhausted_text(tries, why))

# an interrupt during the retries stops them, and keeps the error that
# made them necessary: the user sees both. who: interrupter's answer
# ("the user", "main")
def interrupted_text(+who: String, +tries: Nat, +why: String) -> String:
  String.append(String.concat(["stopped retrying (interrupted by ", who, ") after ",
    Nat.show(tries), " failed attempts; last error: "]), why)

# the pause before a retry is slept in 1 s slices, the flag read between
# them: how many slices a pause takes
def wait_slices(+ms: U32) -> Nat:
  U32.to_nat(U32.add(U32.div(ms, 1000), 1))

# one slice of the pause: nothing left or an interrupt -> 0
def slice_ms(stop: Bool, +left: U32) -> U32:
  Bool.pick(U32, stop, 0, U32.min(left, 1000))

def left_after(stop: Bool, +left: U32) -> U32:
  Bool.pick(U32, stop, 0, U32.sub(left, U32.min(left, 1000)))
