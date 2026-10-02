# core/compaction.bend — ADR 0011: core-owned context budgeting and compaction.
#
# - the pending projection is fitted to the resolved token threshold
# - the canonical history is never touched here; only the projection and
#   the accepted replacement are built
# - summary validation: <summary>..</summary> block, fallback trimmed text,
#   one format retry, then invalid_compaction_summary
import Base
import ./types.bend as T
import ./estimate.bend as E
import ./history.bend as H
import ./text.bend as Tx

# ---- fit result ----
type FitR is Data:
  FOk{ms: List<&2, T.Msg>}
  FBig{}

# middle-truncate the lone text-only latest user message to fit
def fit.lone.trunc(ok_budget: Bool, +m: T.Msg, +budget: Nat) -> FitR:
  match ok_budget:
    case True{}:
      match m:
        case T.Msg{role, +text, _, injected}:
          FOk{Con{T.Msg{role, H.truncate(text, budget), Nil{}, injected},
            Nil{}}}
    case False{}:
      FBig{}

def fit.lone.ok(fits: Bool, +m: T.Msg, +budget: Nat) -> FitR:
  match fits:
    case True{}:
      FOk{Con{m, Nil{}}}
    case False{}:
      fit.lone.trunc(Nat.is_le(3n, budget), m, budget)

def fit.lone(ms: List<&2, T.Msg>, +budget: Nat) -> FitR:
  match ms:
    case Nil{}:
      FBig{}
    case Con{+m, rest}:
      match rest:
        case Con{_, _}:
          FBig{}
        case Nil{}:
          fit.lone.ok(Nat.is_le(E.msg_tokens(m), budget), m, budget)

def fit.zero(fits: Bool, +ms: List<&2, T.Msg>, +budget: Nat) -> FitR:
  match fits:
    case True{}:
      FOk{ms}
    case False{}:
      fit.lone(ms, budget)

# fit `ms` into `budget` tokens: drop the oldest unprotected item (with
# tool-call pairing) until it fits; a lone latest user message is
# middle-truncated; a budget too small for the marker is too large.
# `k` is a drop fuel (the list length) that drives the termination check;
# `fits` and `d` are the precomputed check and drop for the current round.
def fit(+k: Nat, +ms: List<&2, T.Msg>, fits: Bool,
    d: Maybe<&2, List<&2, T.Msg>>, +budget: Nat) -> FitR:
  match k:
    case 0n:
      fit.zero(fits, ms, budget)
    case 1n+j:
      match fits:
        case True{}:
          FOk{ms}
        case False{}:
          match d:
            case None{}:
              fit.lone(ms, budget)
            case Some{+rest}:
              fit(j, rest, Nat.is_le(E.msgs_tokens(rest), budget),
                H.drop_one(rest), budget)

def fit_list(+budget: Nat, +ms: List<&2, T.Msg>) -> FitR:
  fit(List.length(&2, T.Msg, ms), ms,
    Nat.is_le(E.msgs_tokens(ms), budget), H.drop_one(ms), budget)

# ---- summary validation ----
# a complete <summary>..</summary> block with non-empty trimmed content,
# else (no delimiters) the trimmed text as a compatibility fallback,
# else malformed (None)

def parse.non_empty(is_e: Bool, +m: String) -> Maybe<&2, String>:
  match is_e:
    case True{}:
      None{}
    case False{}:
      Some{m}

# permissive: an unclosed <summary> takes the rest of the text as the
# summary (a truncated close tag is not a reason to fail compaction)
def parse.close(c: Maybe<&2, Nat>, +rest: String) -> Maybe<&2, String>:
  match c:
    case None{}:
      +trimmed = String.trim(rest)
      parse.non_empty(String.is_empty(trimmed), trimmed)
    case Some{q}:
      +mid = String.trim(String.take(rest, q))
      parse.non_empty(String.is_empty(mid), mid)

def parse.open(o: Maybe<&2, Nat>, +text: String) -> Maybe<&2, String>:
  match o:
    case None{}:
      +trimmed = String.trim(text)
      parse.non_empty(String.is_empty(trimmed), trimmed)
    case Some{p}:
      +rest = String.drop(text, Nat.add(p, 9n))
      parse.close(Tx.find(rest, "</summary>", 0n), rest)

def parse_summary(+text: String) -> Maybe<&2, String>:
  parse.open(Tx.find(text, "<summary>", 0n), text)

# a compaction reply is valid only without tool calls
def valid_summary(text: String, calls: List<&2, T.Call>) -> Maybe<&2, String>:
  match calls:
    case Nil{}:
      parse_summary(text)
    case Con{_, _}:
      None{}

# ---- the pending projection ----
type PRes is Data:
  POk{input: List<&2, T.Msg>}
  PBig{}

# fixed implementation-owned compaction prompt; manual compaction appends
# the caller's instructions under <extra_compaction_instructions>
def prompt.with_extra(has_extra: Bool, +base: String, +extra: String) -> String:
  match has_extra:
    case True{}:
      String.append(base, String.append(
        "\n<extra_compaction_instructions>", String.append(extra,
          "</extra_compaction_instructions>")))
    case False{}:
      base

def compaction_prompt(+extra: String) -> String:
  prompt.with_extra(Bool.not(String.is_empty(extra)),
    "Summarize the conversation so far for the agent that continues it. Reply with a single <summary>...</summary> block containing a non-empty summary. Do not call tools.\nAfter this compaction the agent sees only your summary and the user's few most recent messages, verbatim; every older user message, and any earlier summary, is gone. So the summary must carry, in order: every request the user made (its key wording, constraints and preferences) and whether it is done, in progress, pending or dropped; the decisions taken and why; the work in progress and its next step; the names it needs to go on (files, commands, branches, ids, people); what an earlier summary said that still matters. Drop what is finished and no longer useful.", extra)

def project.build(f: FitR, +sysmsg: T.Msg, +prompt: T.Msg) -> PRes:
  match f:
    case FOk{ms}:
      POk{Con{sysmsg, List.append(&2, T.Msg, ms, Con{prompt, Nil{}})}}
    case FBig{}:
      PBig{}

# [system] ++ fitted history ++ [compaction prompt], within the threshold.
# The prompt goes LAST: the history of an idle session ends with an
# assistant message, and providers without assistant prefill reject a
# request that does not end with a user message (provider 400).
def project(cfg: T.Cfg, history: List<&2, T.Msg>, +extra: String) -> PRes:
  match cfg:
    case T.Cfg{+threshold, _, _, +system, +tools}:
      +prompt = T.injected_user(compaction_prompt(extra))
      +sysmsg = T.sys(system)
      overhead = Nat.add(E.msg_tokens(sysmsg),
        Nat.add(E.msg_tokens(prompt), E.tool_tokens(tools)))
      budget = Nat.sub(threshold, overhead)
      project.build(fit_list(budget, history), sysmsg, prompt)

# ---- the replacement context ----
type RRes is Data:
  ROk{history: List<&2, T.Msg>}
  RBig{}

# the one injected message that carries the accepted summary. It opens
# the replacement, so the session log records its whole text as
# compaction_done's summary (/log shows the real summary). Its prefix is
# the one --resume (runtime/main-pure.bend replay) and the session
# migration recognise as a summary.
def summary_message(+summary: String) -> T.Msg:
  T.injected_user(String.append(
    "Summary of the earlier conversation:\n<summary>", String.append(summary,
      "</summary>\nThe conversation was compacted: this summary replaces it. The user messages after this one, if any, are the user's most recent ones, kept verbatim for their exact wording; older ones are only in the summary. They were sent before the summary was written: it already says what was done about each, so continue the work from where it stops and do not redo what it reports as done.")))

# 1. the current system message (projected, not stored in canonical history)
# 2. the summary message (summary_message)
# 3. the most recent non-injected user messages, verbatim, that fit the
#    selection budget counted from the newest (H.preserve_recent: never
#    cut in half, the latest one always kept). An earlier summary is
#    injected, so it is never kept: the summarizer folds it into the new
#    one. A message kept by an earlier compaction is kept again only
#    while it is still among the newest within the budget: nothing piles
#    up across compactions.
def replace.build(f: FitR, +final: T.Msg) -> RRes:
  match f:
    case FOk{sel}:
      ROk{Con{final, sel}}
    case FBig{}:
      RBig{}

def replace(cfg: T.Cfg, history: List<&2, T.Msg>, summary: String) -> RRes:
  match cfg:
    case T.Cfg{+threshold, +select_budget, _, +system, +tools}:
      +sysmsg = T.sys(system)
      +final = summary_message(summary)
      overhead = Nat.add(E.msg_tokens(sysmsg),
        Nat.add(E.msg_tokens(final), E.tool_tokens(tools)))
      budget = Nat.sub(threshold, overhead)
      replace.build(fit_list(budget, H.preserve_recent(select_budget, history)),
        final)

# the non-injected user messages of a replacement (what it keeps
# verbatim), for the laws and tests
def kept_users(r: RRes) -> List<&2, T.Msg>:
  match r:
    case ROk{h}:
      H.users_of(h)
    case RBig{}:
      Nil{}

# the replacement's history (Nil when too large), to chain compactions
# in the laws
def replaced(r: RRes) -> List<&2, T.Msg>:
  match r:
    case ROk{h}:
      h
    case RBig{}:
      Nil{}

# ---- session-creation validation ----
