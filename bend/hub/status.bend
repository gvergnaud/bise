# hub/status.bend - an agent's status words and a sender's relation: the
# pure part, out of core.bend (its line ratchet). The status every view
# shows (model.rs Agent::status) and the relation a recipient reads
# (prompts.rs relation); sb-core's core.bend calls them, nothing else
# here decides.
import Base

import ./model.bend as M

import ./cx.bend as X

import ./gen/state.bend as S

def status_of_decl(d: Maybe<&2, S.Decl>) -> String:
  match d:
    case Some{S.DDone{}}:
      "done"
    case Some{S.DBlocked{}}:
      "blocked"
    case _:
      "idle"

def status_of_run(+a: S.Agent, x: X.Rt) -> String:
  match x:
    case X.Rt{_, run, +w, _, _, _}:
      match run:
        case S.RDown{}:
          "starting"
        case S.RStarting{}:
          "starting"
        case S.RBusy{}:
          Bool.pick(String, w, "waiting", "working")
        case S.RIdle{}:
          status_of_decl(S.Agent.declared(a))

def status_of_life(l: S.Life, +a: S.Agent, +x: X.Rt) -> String:
  match l:
    case S.LArchived{}:
      "archived"
    case S.LStopped{}:
      "stopped"
    case S.LFailed{}:
      "failed"
    case S.LActive{}:
      status_of_run(a, x)

# the status every view shows (model.rs Agent::status)
def status_str(+a: S.Agent, +x: X.Rt) -> String:
  status_of_life(S.Agent.life(a), a, x)

# the relation of the sender to the recipient, as the recipient reads it
# (prompts.rs relation)
def relation(+from: String, +to: String, +fp: Maybe<&2, String>, +tp: Maybe<&2, String>) -> String:
  Bool.pick(String, String.eq(from, M.USER()), "user",
    Bool.pick(String, String.eq(from, M.HUB()), "hub",
      Bool.pick(String, String.eq(from, M.GITHUB()), "peer",
      Bool.pick(String, M.maybe_str_is(tp, from) || (String.eq(from, M.MAIN()) && Bool.not(String.eq(to, M.MAIN()))), "parent",
        Bool.pick(String, M.maybe_str_is(fp, to) || String.eq(to, M.MAIN()), "child", "peer")))))

def parent_of(m: Maybe<&2, S.Agent>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{+a}:
      S.Agent.parent(a)
