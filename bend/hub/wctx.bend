# hub/wctx.bend — a task's context for an agent that joins it (roma
# idea 5; architect m_65317, m_65509): its name, why, done-when (ticked
# or not), whether he paused it, and its agents' newest reports with
# their decisions, in sb-core's words (gen/words.bend, the catalog's
# `core` area), clipped at CTX_MAX. A pure function of the state: it
# replaces reading main's thread, never copies it. core.bend's pump puts
# it in front of a delivery (Fx deliver's `ctx`, a hub part) for the
# works the agent never served (`fresh`, read before the step's
# work_turn); an agent that served them already gets none.
import Base

import ../vendor/json.bend as J

import ./gen/state.bend as S
# sb-core reads its words by their area's name (tests/words_read_check.py)
import ./gen/words.bend as core
import ./model.bend as M
import ./text.bend as T
import ./work.bend as WO

# the context's size cap, in chars
def CTX_MAX() -> Nat:
  2000n

# ---- the works an agent joins ----

def fresh.one(m: Maybe<&2, S.Work>, +agent: String) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+w}:
      Bool.not(WO.has(S.Work.agents(w), agent))

# the works of `ids` (live ones) whose agents don't hold `agent` yet: the
# ones its delivery makes it join
def fresh(ids: List<&2, String>, +xs: List<&2, S.Work>, +agent: String) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = fresh(t, xs, agent)
      Bool.pick(List<&2, String>, fresh.one(WO.find(xs, h), agent), h <> rest, rest)

# ---- one task's block ----

# lines, each ended by a line's end
def lines(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, +t}:
      String.concat([h, core.nl(), lines(t)])

def goal(d: Bool, +text: String) -> String:
  match d:
    case True{}:
      core.ctx_goal_done(text)
    case False{}:
      core.ctx_goal_open(text)

def goals.go(gs: List<&2, S.Goal>) -> List<&2, String>:
  match gs:
    case Nil{}:
      Nil{}
    case Con{+g, +t}:
      goal(S.Goal.done(g), S.Goal.text(g)) <> goals.go(t)

# its done-when, under its head; none: no line
def goals(gs: List<&2, S.Goal>) -> List<&2, String>:
  match gs:
    case Nil{}:
      Nil{}
    case Con{+g, +t}:
      core.ctx_done_when() <> goals.go(Con{g, t})

def why.go(empty: Bool, +why: String) -> List<&2, String>:
  match empty:
    case True{}:
      Nil{}
    case False{}:
      [core.ctx_why(why)]

def why(+why: String) -> List<&2, String>:
  why.go(String.is_empty(why), why)

def paused(m: Maybe<&2, Nat>) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{_}:
      [core.ctx_paused()]

def decided(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      core.ctx_decided(h) <> decided(t)

# an agent's newest report and its decisions; a turn's end (auto) or an
# empty one: none
def report.go(skip: Bool, +who: String, +r: J.Json) -> List<&2, String>:
  match skip:
    case True{}:
      Nil{}
    case False{}:
      +ds = M.jstr_list(Maybe.default(&2, List<&2, J.Json>, J.get_arr(r, "decisions"), Nil{}))
      core.ctx_report(who, J.get_str_or(r, "kind", ""), String.trim(J.get_str_or(r, "summary", ""))) <> decided(ds)

def report(m: Maybe<&2, S.Agent>, +who: String) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+a}:
      +r = S.Agent.report(a)
      report.go(J.get_bool_or(r, "auto", False{}) || String.is_empty(String.trim(J.get_str_or(r, "summary", ""))), who, r)

def reports.go(ns: List<&2, String>, +st: S.St) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      List.append(&2, String, report(M.get_agent(st, h), h), reports.go(t, st))

# its agents' newest reports, under their head; none said: no line
def reports.of(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      core.ctx_reports() <> Con{h, t}

def reports(ns: List<&2, String>, +st: S.St) -> List<&2, String>:
  reports.of(reports.go(ns, st))

# work `w`'s block
def block(+w: S.Work, +st: S.St) -> String:
  +head = {[core.ctx_head(S.Work.id(w), S.Work.name(w)), core.ctx_intro()] : List<&2, String>}
  +body = List.append(&2, String, why(S.Work.why(w)), List.append(&2, String, paused(S.Work.paused(w)), List.append(&2, String, goals(S.Work.goals(w)), reports(S.Work.agents(w), st))))
  lines(List.append(&2, String, head, body))

def blocks.one(m: Maybe<&2, S.Work>, +st: S.St) -> String:
  match m:
    case None{}:
      ""
    case Some{+w}:
      block(w, st)

def blocks(ids: List<&2, String>, +st: S.St) -> String:
  match ids:
    case Nil{}:
      ""
    case Con{+h, +t}:
      String.append(blocks.one(WO.find(S.St.works(st), h), st), blocks(t, st))

# ---- the context ----

# the context of works `ids` at `st`, whole (before its cap)
def raw(ids: List<&2, String>, +st: S.St) -> String:
  String.trim(blocks(ids, st))

# the context of works `ids` at `st`: clipped at CTX_MAX (law
# work_context_is_capped); "" for none
def ctx(ids: List<&2, String>, +st: S.St) -> String:
  T.clip(raw(ids, st), CTX_MAX())
