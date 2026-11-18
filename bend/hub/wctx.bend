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
import ./cx.bend as X
import ./keyed.bend as KY
import ./lazy.bend as LZ
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

# the works of `ids` not in `seen` (the ones whose context the agent got
# already, X.Rt.ctx): a steered agent is not their member yet, so its
# membership alone would give it the context at every delivery (architect
# m_67149; law work_context_once_per_agent_without_membership)
def unseen.pick(seen: Bool, +h: String, +rest: List<&2, String>) -> List<&2, String>:
  match seen:
    case True{}:
      rest
    case False{}:
      h <> rest

def unseen(ids: List<&2, String>, +seen: List<&2, String>) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      unseen.pick(WO.has(seen, h), h, unseen(t, seen))

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

# lines under their head; none: not even the head
def headed(+head: String, xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      head <> Con{h, t}

# one line, or none when `none`
def opt(none: Bool, +line: String) -> List<&2, String>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [line]

# the lists one after the other
def joined(xss: List<&2, List<&2, String>>) -> List<&2, String>:
  match xss:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      List.append(&2, String, h, joined(t))

# its done-when, under its head; none: no line
def goals(gs: List<&2, S.Goal>) -> List<&2, String>:
  headed(core.ctx_done_when(), goals.go(gs))

def why(+why: String) -> List<&2, String>:
  opt(String.is_empty(why), core.ctx_why(why))

# his words that started it (Work.said, stamped at birth); an older work: none
def said(+said: String) -> List<&2, String>:
  opt(String.is_empty(said), core.ctx_said(said))

# the most chars of a waiting item's or an agent's line
def LINE_MAX() -> Nat:
  120n

def short(+s: String) -> String:
  T.clip(String.trim(T.one_line(s)), LINE_MAX())

# its open cards (questions, picks), one line each
def waits.go(cs: List<&2, S.Card>, +id: String) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      +rest = waits.go(t, id)
      LZ.pick(List<&2, String>, WO.has(S.Card.works(c), id), u => core.ctx_wait(S.Card.kind(c), short(S.Card.text(c))) <> rest, u => rest)

# what waits on him now: its open cards, then the count of its items on
# his pages (R.pw, architect m_65544); none: no line
def waits(+cs: List<&2, S.Card>, +pw: List<&2, X.Kn>, +id: String) -> List<&2, String>:
  +n = KY.kn_get(pw, id)
  headed(core.ctx_waits(), List.append(&2, String, waits.go(cs, id), opt(Nat.is_eq(n, 0n), core.ctx_page_waits(Nat.show(n)))))

# one of its agents and its `sb status` note
def agent.go(m: Maybe<&2, S.Agent>, +who: String) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+a}:
      +note = short(S.Agent.note(a))
      [Bool.pick(String, String.is_empty(note), core.ctx_agent_quiet(who), core.ctx_agent(who, note))]

def agents.go(ns: List<&2, String>, +st: S.St) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      List.append(&2, String, agent.go(M.get_agent(st, h), h), agents.go(t, st))

# who does what: its agents, one line each
def agents(ns: List<&2, String>, +st: S.St) -> List<&2, String>:
  headed(core.ctx_agents(), agents.go(ns, st))

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

# an agent's newest report that says something: not a turn's end (auto),
# not an empty one
def said_report(+r: J.Json) -> Bool:
  Bool.not(J.get_bool_or(r, "auto", False{}) || String.is_empty(String.trim(J.get_str_or(r, "summary", ""))))

def report.go(ok: Bool, +who: String, +r: J.Json) -> List<&2, String>:
  opt(Bool.not(ok), core.ctx_report(who, J.get_str_or(r, "kind", ""), String.trim(J.get_str_or(r, "summary", ""))))

def decisions.go(ok: Bool, +r: J.Json) -> List<&2, String>:
  match ok:
    case False{}:
      Nil{}
    case True{}:
      decided(M.jstr_list(Maybe.default(&2, List<&2, J.Json>, J.get_arr(r, "decisions"), Nil{})))

# its newest report's line (`decisions`: its decisions' lines instead)
def report(m: Maybe<&2, S.Agent>, +who: String, decisions: Bool) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+a}:
      +r = S.Agent.report(a)
      Bool.pick(List<&2, String>, decisions, decisions.go(said_report(r), r), report.go(said_report(r), who, r))

def reports.go(ns: List<&2, String>, +st: S.St, +decisions: Bool) -> List<&2, String>:
  match ns:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      List.append(&2, String, report(M.get_agent(st, h), h, decisions), reports.go(t, st, decisions))

# its agents' newest reports, under their head; none said: no line
def reports(ns: List<&2, String>, +st: S.St) -> List<&2, String>:
  headed(core.ctx_reports(), reports.go(ns, st, False{}))

# the decisions of its agents' newest reports
def decisions(ns: List<&2, String>, +st: S.St) -> List<&2, String>:
  reports.go(ns, st, True{})

# work `w`'s block, in the order a cut keeps (tasks-ux m_65542): what it
# is and what he asked first, its agents' reports last
def block(+w: S.Work, +st: S.St, +pw: List<&2, X.Kn>) -> String:
  +ns = S.Work.agents(w)
  +head = {[core.ctx_head(S.Work.id(w), S.Work.name(w)), core.ctx_intro()] : List<&2, String>}
  lines(joined([head, said(S.Work.said(w)), why(S.Work.why(w)), paused(S.Work.paused(w)), goals(S.Work.goals(w)), waits(S.St.cards(st), pw, S.Work.id(w)), decisions(ns, st), agents(ns, st), reports(ns, st)]))

def blocks.one(m: Maybe<&2, S.Work>, +st: S.St, +pw: List<&2, X.Kn>) -> String:
  match m:
    case None{}:
      ""
    case Some{+w}:
      block(w, st, pw)

def blocks(ids: List<&2, String>, +st: S.St, +pw: List<&2, X.Kn>) -> String:
  match ids:
    case Nil{}:
      ""
    case Con{+h, +t}:
      String.append(blocks.one(WO.find(S.St.works(st), h), st, pw), blocks(t, st, pw))

def more.one(m: Maybe<&2, S.Work>) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+w}:
      [core.ctx_more(S.Work.id(w))]

# where the rest is (its Log), one line per live work
def more(ids: List<&2, String>, +st: S.St) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      List.append(&2, String, more.one(WO.find(S.St.works(st), h)), more(t, st))

# ---- the context ----

def raw.go(none: Bool, +body: String, +more: String) -> String:
  match none:
    case True{}:
      body
    case False{}:
      String.concat([T.clip(body, Nat.sub(CTX_MAX(), Nat.add(1n, String.length(more)))), core.nl(), more])

# the context of works `ids` at `st` (`pw` his pages' waits by work):
# their blocks cut from the end to leave room for the pointer to the Log,
# which stays the last line
def raw(+ids: List<&2, String>, +st: S.St, +pw: List<&2, X.Kn>) -> String:
  +more = String.trim(lines(more(ids, st)))
  raw.go(String.is_empty(more), String.trim(blocks(ids, st, pw)), more)

# the context of works `ids` at `st`: clipped at CTX_MAX (law
# work_context_is_capped); "" for none
def ctx(ids: List<&2, String>, +st: S.St, +pw: List<&2, X.Kn>) -> String:
  T.clip(raw(ids, st, pw), CTX_MAX())
