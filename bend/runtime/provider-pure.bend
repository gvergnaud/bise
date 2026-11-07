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
import ../core/image.bend as Im
import ../vendor/json.bend as J
import ./plan-pure.bend as Plan
import ../core/gen/obs.bend as OG

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

# No models file (it could not be written: every spawn sets
# BISE_MODELS_FILE): these two setups, with no efforts list (no effort
# chosen, the body default goes; an effort list never lives in bend/).
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

def cache_names(+reg: List<&2, Cf.KV>, +prov: String, +id: String)
    -> CNames:
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

# Rust writes each header as a quoted TOML value. Its escapes are also
# valid JSON escapes. Decode them before constructing the HTTP headers.
def header_value(+raw: String) -> String:
  Maybe.default(&2, String,
    J.as_str(W.parse_json(String.concat(["\"", raw, "\""]))), "")

def header_row(hit: Bool, +key: String, +value: String, +prefix: String) -> String:
  match hit:
    case True{}:
      String.concat([Cf.cfg_val(String.drop(key, String.length(prefix))),
        ": ", header_value(value), "\n"])
    case False{}:
      ""

def header_rows(kvs: List<&2, Cf.KV>, +prefix: String) -> String:
  match kvs:
    case Nil{}:
      ""
    case Con{Cf.KV{+key, +value}, +rest}:
      String.append(header_row(String.starts_with(key, prefix), key, value, prefix),
        header_rows(rest, prefix))

def headers(+file: String, +prov: String, +id: String) -> String:
  +reg = registry(file)
  String.append(header_rows(reg, String.concat(["providers.", prov, ".headers."])),
    header_rows(reg, String.concat(["models.\"", id, "\".headers."])))

def headers_missing(+name: String, +value: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(name)), String.is_empty(String.trim(value)))

def headers_why(+name: String) -> String:
  String.concat([name,
    " (headers_env in config.toml) is not set or is blank: set it before starting bise, or use headers in config.toml"])

def key_command(+file: String, +prov: String, +id: String) -> String:
  Cf.cfg_or(model_key(registry(file), prov, id, "key_command"), "")

# ---- the REPL's memory of its calls ----
# What one call leaves for the next ones of the same REPL process: the
# last good key each provider's key_command gave. runtime/provider.bend's
# Out carries it (the loop threads Out through every call, the REPL
# keeps it across its connections), so it lives in memory only: never
# in the checkpoint (P.save writes the Session alone), the reqlog ring
# or a log. A REPL restart starts empty.
type KeyMem is Data:
  KeyMem{prov: String, cmd: String, key: String}

type CallMem is Data:
  CallMem{keys: List<&2, KeyMem>}

def mem0() -> CallMem:
  CallMem{Nil{}}

# ---- a key_command's answer, and the key the call uses ----
# key_command runs before every call (a gateway's short-lived token, a
# keychain read). It fails at night when the login keychain is locked:
# then the last good key of the same provider and command goes on; only
# the provider refusing it (401/403, key_drop) forgets it.

# what the command did: exited with its streams, or never answered
# (could not start, ran past its limit: why)
type KeyRun is Data:
  KRan{code: U32, out: String, err: String}
  KFail{why: String}

# the key of this call: a fresh one (kept for the next calls), the last
# good one (the command failed: cause, logged), or none (cause, in the
# turn's error)
type KeyPick is Data:
  KFresh{key: String}
  KCached{key: String, cause: String}
  KNone{cause: String}

# the fresh key: the trimmed stdout of a command that exited 0, else ""
def key_out(r: KeyRun) -> String:
  match r:
    case KRan{+code, +out, _}:
      Bool.pick(String, U32.is_zero(code), String.trim(out), "")
    case KFail{_}:
      ""

def key_first(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+l, _}:
      l

def key_err_line(+err: String) -> String:
  String.take(String.trim(key_first(String.split(String.trim(err),
    Tx.newline()))), 200n)

def key_cause.err(empty: Bool, +head: String, +line: String) -> String:
  match empty:
    case True{}:
      head
    case False{}:
      String.concat([head, ": ", line])

# why the command gave no key, in the user's words: its exit code and
# the first line of its stderr ("exit 36: security: ... keychain")
def key_cause(r: KeyRun) -> String:
  match r:
    case KRan{+code, _, +err}:
      key_cause.err(String.is_empty(key_err_line(err)),
        Bool.pick(String, U32.is_zero(code), "it printed nothing",
          String.append("exit ", U32.show(code))),
        key_err_line(err))
    case KFail{+why}:
      why

def key_is(+prov: String, +cmd: String, +k: KeyMem) -> Bool:
  match k:
    case KeyMem{+p, +c, _}:
      Bool.and(String.eq(p, prov), String.eq(c, cmd))

def key_of_mem(+k: KeyMem) -> String:
  match k:
    case KeyMem{_, _, +key}:
      key

def key_prov(+k: KeyMem) -> String:
  match k:
    case KeyMem{+p, _, _}:
      p

# whether the list's head is the provider's key for that command
def key_at(+keys: List<&2, KeyMem>, +prov: String, +cmd: String) -> Bool:
  match keys:
    case Nil{}:
      False{}
    case Con{+k, _}:
      key_is(prov, cmd, k)

# the walk stops at its hit (the hit is a parameter, computed on the tail)
def key_find.go(keys: List<&2, KeyMem>, +hit: Bool, +prov: String,
    +cmd: String) -> Maybe<&2, String>:
  match keys hit:
    case Nil{} _:
      None{}
    case Con{+k, _} True{}:
      Some{key_of_mem(k)}
    case Con{_, +rest} False{}:
      key_find.go(rest, key_at(rest, prov, cmd), prov, cmd)

def key_find(+keys: List<&2, KeyMem>, +prov: String, +cmd: String)
    -> Maybe<&2, String>:
  key_find.go(keys, key_at(keys, prov, cmd), prov, cmd)

def key_without.at(hit: Bool, +k: KeyMem, +rest: List<&2, KeyMem>)
    -> List<&2, KeyMem>:
  match hit:
    case True{}:
      rest
    case False{}:
      Con{k, rest}

# every key but the provider's (whatever its command)
def key_without(+keys: List<&2, KeyMem>, +prov: String) -> List<&2, KeyMem> :
  match keys:
    case Nil{}:
      Nil{}
    case Con{+k, +rest}:
      key_without.at(String.eq(key_prov(k), prov), k,
        key_without(rest, prov))

def key_pick.old(m: Maybe<&2, String>, +cause: String) -> KeyPick:
  match m:
    case None{}:
      KNone{cause}
    case Some{+key}:
      KCached{key, cause}

def key_pick.of(empty: Bool, +fresh: String, +mem: CallMem, +prov: String,
    +cmd: String, +cause: String) -> KeyPick:
  match empty:
    case False{}:
      KFresh{fresh}
    case True{}:
      match mem:
        case CallMem{+keys}:
          key_pick.old(key_find(keys, prov, cmd), cause)

# the key a call uses, from what key_command did and the REPL's memory
def key_pick(+mem: CallMem, +prov: String, +cmd: String, +r: KeyRun)
    -> KeyPick:
  key_pick.of(String.is_empty(key_out(r)), key_out(r), mem, prov, cmd,
    key_cause(r))

# why the pick has no key ("" when it has one)
def key_missing_cause(+p: KeyPick) -> String:
  match p:
    case KNone{+cause}:
      cause
    case _:
      ""

def key_used(+p: KeyPick) -> String:
  match p:
    case KFresh{+key}:
      key
    case KCached{+key, _}:
      key
    case KNone{_}:
      ""

# the memory after the pick: a fresh key replaces the provider's entry
# (a changed command drops the old one); a cached or no key leaves it
def key_keep(+mem: CallMem, +prov: String, +cmd: String, +key: String)
    -> CallMem:
  match mem:
    case CallMem{+keys}:
      CallMem{Con{KeyMem{prov, cmd, key}, key_without(keys, prov)}}

def key_mem(+mem: CallMem, +prov: String, +cmd: String, +p: KeyPick)
    -> CallMem:
  match p:
    case KFresh{+key}:
      key_keep(mem, prov, cmd, key)
    case _:
      mem

def key_refused(+status: U32) -> Bool:
  Bool.or(U32.is_eq(status, 401), U32.is_eq(status, 403))

# the provider refused the key (401/403): its entry goes, the next call
# asks key_command again and uses nothing older; any other answer keeps it
def key_drop(+mem: CallMem, +prov: String, +status: U32) -> CallMem:
  match mem:
    case CallMem{+keys}:
      Bool.pick(CallMem, key_refused(status),
        CallMem{key_without(keys, prov)}, CallMem{keys})

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
# The words a model takes: its `efforts` key, else its provider's
# (comma-separated). Rust is the one owner of the lists
# (rust/catalog/src/efforts.rs): the models file carries each model's
# list and each provider's family list; no list lives here. None when it
# does not reason (an Anthropic one with thinking = "none" included), or
# when no file says (no models file: builtin() below has none, the body
# default goes).

def non_empty(s: String) -> Bool:
  Bool.not(String.is_empty(s))

def efforts.words(+s: String) -> List<&2, String>:
  List.filter(~String, ~non_empty, String.split(s, ','))

def efforts.on(on: Bool, +own: String) -> List<&2, String>:
  match on:
    case True{}:
      efforts.words(own)
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
    Cf.cfg_or(model_key(reg, prov, id, "efforts"), ""))

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

# ---- a model listed without vision ----
# The models file writes every listed model its own `vision = true|false`
# (Rust handoff_toml, pinned by rust/catalog tests.rs
# the_handoff_says_each_listed_models_vision): a model whose own line
# says false never gets an image part (runtime/provider.bend img_body),
# because the provider behind a gateway refuses the whole request for
# one (the Datadog one: "Image input is not supported for this
# endpoint"). The provider's line is never read here: a model the file
# does not list has no line and is the provider's to decide, nothing is
# stripped on a guess. `reg` is the registry the request already read.
def model_blind(+reg: List<&2, Cf.KV>, +id: String) -> Bool:
  String.eq(Cf.cfg_or(Cf.cfg_find(reg,
    String.concat(["models.\"", id, "\".vision"])), ""), "false")

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

# the full id a session asked for, before the swap (the note's <from>):
# the same inputs as resolve_chosen; "" when there is no model
def asked_chosen(+choice: String, +bise: String, +bend: String,
    +agent: String, +role: String, +cfg: String, +file: String) -> String:
  +reg = registry(file)
  +name = chosen_name(Cf.parse_toml(choice),
    model_name(bise, bend, agent, role, Cf.parse_toml(cfg), reg))
  Bool.pick(String, String.is_empty(name), "", full_id(name, reg))

def resolve_named(empty: Bool, +name: String, +reg: List<&2, Cf.KV>,
    +url_env: String, +asked: String) -> Resolved:
  match empty:
    case True{}:
      RErr{"", NO_MODEL()}
    case False{}:
      resolve_id_effort(Plan.swapped(full_id(name, reg), reg), reg, url_env, asked)

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
# DEFAULT_CONTEXT (rust/catalog's DEFAULT_CAPS). The compaction
# threshold it gives: runtime/threshold-pure.bend.

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

# the in-flight mark: "request" while an agent's model call runs, "" else
# (runtime/main.bend). A REPL restored after a death reads it once: a
# turn cut during its model call resumes (core/session.bend apply.resume)
def inflight_path_of(+dir: String, +port: String) -> String:
  String.append(dir, String.append("/bend-inflight-", String.append(port, ".txt")))

def inflight_cut(m: Maybe<&2, String>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+t}:
      String.eq(String.trim(t), "request")

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

# NDns: why the name gave no address to connect to and what the user can
# check ("" when nothing on his side would help), vendor/http/dns's words.
type NetErr is Data:
  NUrl{}
  NDns{why: String, check: String}
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
    case NDns{_, _}:
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

# what a failed TLS handshake asks to check: code 2 (ENOENT) is wire.c's "no
# TLS library", which its own words explain; the network has no part in it
def tls_check(+c: U32) -> String:
  Bool.pick(String, U32.is_eq(c, 2), "", check_network())

# " — check <what>", or nothing when there is nothing to check
def check_what(+what: String) -> String:
  Bool.pick(String, String.is_empty(what), "", String.append(" — check ", what))

# the host of "host:port" (a DNS miss names the name, not the port)
def host_name(+hp: String) -> String:
  url_host.head(String.split(hp, ':'))

def net_cause(+host: String, e: NetErr) -> String:
  match e:
    case NUrl{}:
      String.concat(["bad provider URL (", host,
        ") — check BEND_PROVIDER_URL or the provider table"])
    case NDns{+why, +what}:
      String.concat(["cannot resolve ", host_name(host), ": ", why, check_what(what)])
    case NConnect{+c, +w}:
      String.concat(["cannot reach ", host, " (", coded("connect ", c, w),
        ")", check_network()])
    case NTls{+c, +w}:
      String.concat(["TLS handshake with ", host, " failed (",
        coded("tls ", c, w), ")", tls_check(c)])
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
# The network gone (no name, no connection: AWait) spends no attempt:
# the call waits for it, up to 30 min (the network wait, next_of).

# which limit of a streamed attempt ran out (ProvWait): the response
# head, or a gap in the body
type Limit is Data:
  LHead{}
  LGap{}

# the verdict on one attempt: the mapped reply, a final failure, or a
# retry after a pause (0 = no server hint, the backoff decides); a
# timeout is a retry with the backoff too, and says which limit ran out
# and its ms (the request_failed event carries them, typed)
type Attempt is Data:
  AOk{reply: String}
  AFail{why: String}
  ARetry{after_ms: U32, why: String}
  AWait{why: String}
  ATimeout{limit: Limit, ms: U32, why: String}

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
    case NDns{_, _}:
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

# who a refusal names: the provider's display name, or, for a provider
# whose URL the user brings (base_url_env: foundry, any endpoint behind
# it), the host the call went to. 2026-10-30: foundry's URL pointed to a
# gateway, and its 400 read "Anthropic (foundry proxy) refused...".
def prov_label.pick(own: Bool, +name: String, +host: String) -> String:
  Bool.pick(String, Bool.and(own, Bool.not(String.is_empty(host))), host, name)

def prov_label(+file: String, +prov: String, +url: String) -> String:
  prov_label.pick(
    Bool.not(String.is_empty(Cf.cfg_or(Cf.cfg_find(registry(file),
      String.concat(["providers.", prov, ".base_url_env"])), ""))),
    prov_display(file, prov), url_host(url))

def refusal.who(+name: String) -> String:
  Bool.pick(String, String.is_empty(name), "the provider", name)

# bise's line of a refused request: who refused, the status, what to
# check (the designer's words, BISE-282's pattern)
# (big: a refusal for size, W.size_refused: a 413 or a 400 that says so;
# its line carries W.too_large_mark, which the Core reads to compact)
# (hist: a refusal of the history itself, W.history_refused; its line
# carries W.history_mark, which the Core reads to send it again once
# without its reasoning)
def refusal.head(+who: String, +status: U32, big: Bool, hist: Bool)
    -> String:
  +s = U32.show(status)
  Bool.pick(String, Bool.or(U32.is_eq(status, 401), U32.is_eq(status, 403)),
    String.concat([who, " refused the key (", s, "). check it with /setup."]),
    Bool.pick(String, U32.is_eq(status, 404),
      String.concat([who, " refused the request (", s,
        "). check the model name and the provider URL."]),
      Bool.pick(String, big,
        String.concat([who, " refused the request (", s, "). ",
          W.too_large_mark(), "."]),
        Bool.pick(String, hist,
          String.concat([who, " ", W.history_mark(), " (", s, ")."]),
          String.concat([who, " refused the request (", s, ")."])))))

# the retry line of a request refused for its size (model_call.sized)
def size_retry_why(+name: String, +status: U32, +half: Nat) -> String:
  String.concat([refusal.who(name), " refused the request for its size (",
    U32.show(status), "): sending it again with fewer images, under ",
    Im.bytes_text(half)])

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
  String.append(refusal.head(who, status, W.size_refused(status, body),
      W.history_refused(status, body)),
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
    case AWait{+why}:
      AWait{why}
    case ATimeout{+l, +ms, +why}:
      ATimeout{l, ms, why}

# ---- the ChatGPT plan's failures (shape = "chatgpt-plan") ----
# The designer's lines (docs/subscriptions-design.md, final words §5).
# OpenAI's plan route says a usage limit with a code, in a non-200 JSON
# body (429, 503) or mid-stream in response.failed (a 200): either way
# the turn stops on bise's line, never a retry loop on a limit nor an
# "OpenAI refused the key" on a sign-in that expired.

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
      ARetry{ms, Plan.PLAN_UNCHECKED()}
    case _:
      ARetry{0, Plan.PLAN_UNCHECKED()}

def attempt_plan.pick(unchecked: Bool, +status: U32, +code: String,
    a: Attempt) -> Attempt:
  match unchecked:
    case True{}:
      plan_retry(a)
    case False{}:
      attempt_plan.line(Plan.plan_line(status, code), a)

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
# the key_env's. cause: why the key_command gave no key (key_cause:
# its exit code and the first line of its stderr; "" unknown)
def key_why.said(empty: Bool, +prov: String, +cause: String) -> String:
  match empty:
    case True{}:
      String.concat(["no ", prov,
        " key: its key_command failed or printed nothing (config.toml)"])
    case False{}:
      String.concat(["no ", prov, " key: its key_command failed (", cause,
        "); check it in config.toml"])

def key_why.cmd(plan: Bool, +prov: String, +cause: String) -> String:
  match plan:
    case True{}:
      Plan.PLAN_EXPIRED()
    case False{}:
      key_why.said(String.is_empty(cause), prov, cause)

def key_why(has_cmd: Bool, plan: Bool, +prov: String, +key_env: String,
    +cause: String) -> String:
  match has_cmd:
    case True{}:
      key_why.cmd(plan, prov, cause)
    case False{}:
      String.concat(["no ", prov, " key yet (", key_env,
        " is not set): /provider sets it up"])

# the network itself is gone (no name, no route, no connection): the
# call waits for it (AWait) instead of spending its attempts
def net_down(e: NetErr) -> Bool:
  match e:
    case NDns{_, _}:
      True{}
    case NConnect{_, _}:
      True{}
    case _:
      False{}

def attempt_err.retry(down: Bool, +host: String, +e: NetErr) -> Attempt:
  match down:
    case True{}:
      AWait{net_cause(host, e)}
    case False{}:
      ARetry{0, net_cause(host, e)}

def attempt_err.pick(retryable: Bool, +host: String, +e: NetErr) -> Attempt:
  match retryable:
    case True{}:
      attempt_err.retry(net_down(e), host, e)
    case False{}:
      AFail{net_cause(host, e)}

# a transport failure: no response at all. The cause names the host and
# says what the user can do about it (the text every retry line, the
# final error and an interrupted retry loop carry)
def attempt_err(+host: String, +e: NetErr) -> Attempt:
  attempt_err.pick(retryable_err(e), host, e)

# ---- a provider's own wait (slow gateways, local models) ----
# Two limits (chantier c8-gateway-hang). The head: how long the attempt
# waits for the HTTP response head (status + headers), which a streaming
# endpoint sends before any token, so a thinking model's long think
# does not count; c2's usage ttfb_ms measures the same moment. The gap:
# how long one read of the body may then wait for data, between two
# events. A hung gateway holds the head: 703 x 90 s waits in 2 weeks
# (10-24 -> 11-07), so the head limit is shorter than the gap.
# idle_timeout_sec, under [providers.<id>] or a model's table, sets
# both (a local model on a slow machine can take minutes to read a long
# prompt before its head). Unset, 0 or not a number: the gap is
# stream_step_ms (90 s), as before, and the head is head_ms (30 s).
# Capped at a day (the ms fit a U32). The wait carries the provider id:
# the timeout text names the key.
type ProvWait is Data:
  ProvWait{prov: String, head: U32, gap: U32}

# the head limit of a call's first attempt, without idle_timeout_sec.
# Not sized on a measured ttfb (none in his events before c2's fields
# reach his hubs): the wall time of a one-attempt call with a short
# reply is p50 4.9 s, p99 48.6 s, an upper bound that holds the
# thinking. Read call_times.py's ttfb p99.9 again around 2026-11-15 and
# move it there. Each retry doubles it, up to the gap (head_for_try):
# a slow but alive head still gets today's 90 s by the third attempt.
def head_ms() -> U32:
  30000

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

# the head of the first attempt: head_ms unset, else the provider's own
def idle_head.pick(zero: Bool, +secs: U32) -> U32:
  match zero:
    case True{}:
      head_ms()
    case False{}:
      idle_ms(secs)

def idle_head(+secs: U32) -> U32:
  idle_head.pick(U32.is_eq(secs, 0), secs)

def stream_wait.of(+prov: String, +secs: U32) -> ProvWait:
  ProvWait{prov, idle_head(secs), idle_ms(secs)}

# `id` is the full "provider/model"
def stream_wait(+file: String, +prov: String, +id: String) -> ProvWait:
  stream_wait.of(prov, u32_or(model_key(registry(file), prov, id,
    "idle_timeout_sec")))

def gap_ms(+w: ProvWait) -> U32:
  match w:
    case ProvWait{_, _, +gap}:
      gap

def head_of(+w: ProvWait) -> U32:
  match w:
    case ProvWait{_, +head, _}:
      head

# the head doubled once per earlier attempt, never past the gap (each
# step capped, so a day-long wait never overflows)
def head_grow(tries: Nat, +head: U32, +gap: U32) -> U32:
  match tries:
    case 0n:
      U32.min(head, gap)
    case 1n+p:
      head_grow(p, U32.min(U32.mul(head, 2), gap), gap)

# the wait of attempt #tries (0-based): its head limit grown, its gap
# as it is
def wait_for_try(+w: ProvWait, tries: Nat) -> ProvWait:
  match w:
    case ProvWait{+prov, +head, +gap}:
      ProvWait{prov, head_grow(tries, head, gap), gap}

# "90 s"; under a second (never from the catalog): "0 s"
def wait_secs(+ms: U32) -> String:
  String.append(U32.show(U32.div(ms, 1000)), " s")

def limit_ms(+w: ProvWait, l: Limit) -> U32:
  match l:
    case LHead{}:
      head_of(w)
    case LGap{}:
      gap_ms(w)

def limit_words(l: Limit) -> String:
  match l:
    case LHead{}:
      "no response head"
    case LGap{}:
      "the reply stalled"

# a read that waited the whole limit: the host, the limit (and which
# one), and the key that raises it (the user can't tell a slow model
# from a dead link; the text says what to change when it is the model)
def timeout_cause(+host: String, +w: ProvWait, +l: Limit) -> String:
  match w:
    case ProvWait{+prov, _, _}:
      String.concat(["no answer from ", host, " in ", wait_secs(limit_ms(w, l)),
        " (timeout, ", limit_words(l),
        ") — a slow model or gateway? raise idle_timeout_sec under [providers.",
        prov, "] in ~/.bise/config.toml"])

def is_timeout(e: NetErr) -> Bool:
  match e:
    case NTimeout{}:
      True{}
    case _:
      False{}

def attempt_err_in.pick(timeout: Bool, +host: String, +w: ProvWait,
    +l: Limit, +e: NetErr) -> Attempt:
  match timeout:
    case True{}:
      ATimeout{l, limit_ms(w, l), timeout_cause(host, w, l)}
    case False{}:
      attempt_err(host, e)

# a transport failure of a streamed call: a timeout names its limit and
# idle_timeout_sec (still a retry, as before); the others as attempt_err.
# l: where it waited (LHead: before the response head, LGap: in the body)
def attempt_err_in(+host: String, +w: ProvWait, +l: Limit, +e: NetErr)
    -> Attempt:
  attempt_err_in.pick(is_timeout(e), host, w, l, e)

# what a retry's request_failed event says of its cause (bend/core/ev.bend
# retry): "timeout" and the limit that ran out ("head" / "gap") with its
# ms, else "other" and no limit ("")
def retry_kind(+a: Attempt) -> String:
  match a:
    case ATimeout{_, _, _}:
      "timeout"
    case _:
      "other"

def retry_limit(+a: Attempt) -> String:
  match a:
    case ATimeout{+l, _, _}:
      match l:
        case LHead{}:
          "head"
        case LGap{}:
          "gap"
    case _:
      ""

def retry_waited(+a: Attempt) -> U32:
  match a:
    case ATimeout{_, +ms, _}:
      ms
    case _:
      0

# the line the client shows while the call waits (the TUI renders it
# as a warning): "provider_retry: 2/10 · <why> · retry in 4s"
def retry_line(+n: Nat, +why: String, +delay_ms: U32) -> String:
  OG.provider_retry(Nat.add(n, 1n), max_attempts(), String.take(why, 200n), U32.to_nat(delay_ms))

# the final error once every attempt failed: the count and the last cause
def exhausted_text(+tries: Nat, +why: String) -> String:
  String.append(String.concat(["provider failed after ", Nat.show(tries),
    " attempts: "]), why)

# the same, but a ChatGPT plan whose usage could never be checked ends
# on the designer's line alone
def exhausted_for(+tries: Nat, +why: String) -> String:
  Bool.pick(String, String.eq(why, Plan.PLAN_UNCHECKED()), Plan.PLAN_UNCHECKED_LINE(),
    exhausted_text(tries, why))

# an interrupt during the retries stops them, and keeps the error that
# made them necessary: the user sees both. who: interrupter's answer
# ("the user", "main")
def interrupted_text(+who: String, +tries: Nat, +why: String) -> String:
  String.append(String.concat(["stopped retrying (interrupted by ", who, ") after ",
    Nat.show(tries), " failed attempts; last error: "]), why)

# the pause before a retry is slept in 100 ms slices, the flag read
# between them, so a stop lands within one slice (issue 13: under 1 s;
# they were 1 s): how many slices a pause takes
def wait_slices(+ms: U32) -> Nat:
  U32.to_nat(U32.add(U32.div(ms, 100), 1))

# one slice of the pause: nothing left or an interrupt -> 0
def slice_ms(stop: Bool, +left: U32) -> U32:
  Bool.pick(U32, stop, 0, U32.min(left, 100))

def left_after(stop: Bool, +left: U32) -> U32:
  Bool.pick(U32, stop, 0, U32.sub(left, U32.min(left, 100)))

# ---- the call's context ----
# What one model call needs beside its request, resolved once per call
# (not per attempt) by prepare: the provider, its full id, the models
# file, the key (key_pick's), whether it also goes as a bearer token (a
# key_command gateway), the extra headers (headers in config.toml, then
# the headers_env value), the stream's wait, the name errors give the
# provider; req: the Core's action id; live: the reply streams in;
# save_ms: the checkpoint's time before the call (its usage says it).
type CallCtx is Data:
  CallCtx{pv: Prov, id: String, mfile: String, key: String, bearer: Bool,
    extra: String, wait: ProvWait, label: String, req: U32, live: Bool,
    save_ms: Nat}

# hval: the headers_env variable's value ("" none)
def prepare(+pv: Prov, +mfile: String, +key: String, +hval: String,
    +req: U32, +live: Bool, +save_ms: Nat) -> CallCtx:
  match pv:
    case Prov{+prov, +model, _, +url, _, _}:
      CallCtx{pv, String.concat([prov, "/", model]), mfile, key,
        Bool.not(String.is_empty(key_command(mfile, prov,
          String.concat([prov, "/", model])))),
        String.append(headers(mfile, prov, String.concat([prov, "/", model])),
          hval),
        stream_wait(mfile, prov, String.concat([prov, "/", model])),
        prov_label(mfile, prov, url), req, live, save_ms}

# the provider id of a call (the key memory's key)
def ctx_prov(+c: CallCtx) -> String:
  match c:
    case CallCtx{+pv, _, _, _, _, _, _, _, _, _, _}:
      match pv:
        case Prov{+provider, _, _, _, _, _}:
          provider

# ---- the network wait (c2-turn-survives b) ----
# A call has two budgets. A status failure (a 5xx, a 429, a timeout, a
# cut stream) spends one of its max_attempts, with the backoff, as
# before. A network failure (AWait: no name, no connection) spends
# none: the call waits for the network, the pause doubling from 1 s to
# 30 s, until net_wait_max_ms (30 min) of waiting; a status failure in
# between goes back to the status budget (its waiting starts over at
# the next outage). Every pause is interruptible (model_call.wait).

# left: the status retries the call still has; waited: the ms it has
# waited for the network in the current outage
type Budget is Data:
  Budget{left: Nat, waited: U32}

# a call's first budget: max_attempts in all, the first one running
def budget0() -> Budget:
  Budget{Nat.sub(max_attempts(), 1n), 0}

def net_wait_max_ms() -> U32:
  1800000

def net_every_ms() -> U32:
  30000

# the next pause of an outage: as long as it has lasted (1 s at first),
# at most 30 s, never past the 30 min
def net_delay(+waited: U32) -> U32:
  U32.min(U32.min(U32.max(waited, 1000), net_every_ms()),
    U32.sub(net_wait_max_ms(), U32.min(waited, net_wait_max_ms())))

# what the loop does after a failed attempt: retry after a pause with
# the budget that is left (wait: a network wait; says: it is the first
# pause of the outage or the first one past a 5 min mark - the one that
# writes a line, so 30 min of waiting is 7 lines, not 60), or stop with
# the turn's error
type Next is Data:
  XRetry{delay_ms: U32, why: String, b: Budget}
  XWait{delay_ms: U32, why: String, b: Budget, says: Bool}
  XStop{why: String}

def net_gave_up(+waited: U32, +why: String) -> String:
  String.concat(["no network for ", U32.show(U32.div(waited, 60000)),
    " min; last error: ", why])

# a wait line every 5 min of an outage, and at its start
def wait_says(+waited: U32, +delay: U32) -> Bool:
  Bool.or(U32.is_zero(waited),
    Bool.not(U32.is_eq(U32.div(waited, 300000),
      U32.div(U32.add(waited, delay), 300000))))

def next_status(left: Nat, +waited: U32, +after_ms: U32, +why: String,
    +tries: Nat) -> Next:
  match left:
    case 0n:
      XStop{exhausted_for(Nat.add(tries, 1n), why)}
    case 1n+p:
      XRetry{retry_delay_ms(after_ms, tries), why, Budget{p, 0}}

def next_net(over: Bool, +left: Nat, +waited: U32, +why: String) -> Next:
  match over:
    case True{}:
      XStop{net_gave_up(waited, why)}
    case False{}:
      XWait{net_delay(waited), why,
        Budget{left, U32.add(waited, net_delay(waited))},
        wait_says(waited, net_delay(waited))}

# the decision after attempt #tries (0-based) failed with a: a status
# retry (ARetry) or a network wait (AWait); AOk and AFail end the call
# before it is asked (they stop here too)
def next_of(+a: Attempt, +b: Budget, +tries: Nat) -> Next:
  match a:
    case ARetry{+after_ms, +why}:
      match b:
        case Budget{+left, +waited}:
          next_status(left, waited, after_ms, why, tries)
    case ATimeout{_, _, +why}:
      # no server hint: the backoff grows as for any status retry, so a
      # shorter head limit never hammers a gateway in an episode
      match b:
        case Budget{+left, +waited}:
          next_status(left, waited, 0, why, tries)
    case AWait{+why}:
      match b:
        case Budget{+left, +waited}:
          next_net(U32.is_le(net_wait_max_ms(), waited), left, waited, why)
    case AFail{+why}:
      XStop{why}
    case AOk{_}:
      XStop{""}

# a status attempt spent outside next_of (a body refused for its size
# goes again once, smaller): one retry less
def budget_spent(+b: Budget) -> Budget:
  match b:
    case Budget{+left, _}:
      Budget{Nat.sub(left, 1n), 0}

def budget_waited(+b: Budget) -> U32:
  match b:
    case Budget{_, +waited}:
      waited

# the guard of a call's loop (model_call.try's fuel): every status
# attempt, and every pause of a network wait for each of them
def call_fuel() -> Nat:
  U32.to_nat(1000)
