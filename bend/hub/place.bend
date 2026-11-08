# hub/place.bend - a worktree task is placed off the hub's loop (hub-loop,
# docs/issues/32), the pure part.
#
# Before, sb-core asked the hub for `worktree_create` inside the spawn's
# step: the hub ran `git worktree add` (a checkout of the whole tree) and
# the config's setup command on its loop, 2-10 s during which no agent's
# `sb` was served. Now the spawn's step decides and commits: the task is
# created with a placing workspace ([`placing_ws`]: mode worktree, state
# placing, no path yet), and the fx `worktree_create` asks the shell to
# make it on a thread. The shell picks the path and branch (they depend
# on the disk and the refs) and sends the input `placed` with the whole
# workspace, or git's words; the journal keeps it (`worktree_placed`).
# No REPL starts before (core.bend's `spawn` skips a placing task): its
# messages wait for it as for a starting agent.
#
# What a `placed` does ([`placed_of`]): its task placing, the worktree
# made: the workspace, then its REPL; its task placing, the worktree not
# made: the task fails with git's words, its spawner told once; its task
# gone or no longer placing (dropped meanwhile, archived): the worktree
# goes again (fx `worktree_drop`, nothing to lose), nothing else; a
# `placed` with an error for a task not placing: nothing.
#
# A join while placing (`sb spawn B --place A`, A placing; rows233B):
# the daemon's ws for A's place carries `place`, so B makes no worktree
# of its own ([`makes`]) and waits; A's `placed` then places its
# [`joiners`] the same way: A's ws, or A's failure (A gone: theirs too).
#
# A /restore (3b) and an /isolate (3c) place a task the same way: its ws
# placing with where it came from ([`ws_back`]: BRestore, BIsolate), so
# its `placed` resumes it, or puts it back, instead of starting or
# failing a new one. `sb spawn --feature` is a new one whose fx carries
# the feature ([`ws_feature`]).
#
# core.bend's `placed` runs these decisions (it holds emit, spawn, send).
# Laws in bend/laws/place.bend.
import Base

import ../vendor/json.bend as J

import ./model.bend as M

import ./gen/state.bend as S

# the workspace of a task whose worktree the shell is making: no path, no
# branch yet (the shell picks them); `with_changes`: the shared folder's
# changes go into it (`sb spawn --with-changes`)
def placing_ws(+with_changes: Bool) -> J.Json:
  J.obj([J.kv("mode", J.JStr{"worktree"}), J.kv("state", J.JStr{"placing"}), J.kv("path", J.JStr{""}),
    J.kv("branch", J.JNull{}), J.kv("base_commit", J.JNull{}), J.kv("dropped", J.JBool{False{}}),
    J.kv("with_changes", J.JBool{with_changes})])

# ws.restore / ws.isolate / ws.restore_joined / ws.feature on a placing
# ws (issue 32 3b, 3c): untyped, c9 types them with ws. The only reads and
# writes of these keys (architect m_55528, m_55706).

# where a placing ws came from: a new worktree, a /restore's (the dropped
# ws it goes back to), an /isolate's (the shared ws it goes back to)
type Back is Data:
  BNew{}
  BRestore{from: J.Json}
  BIsolate{from: J.Json}

def ws_back.of(r: Maybe<&2, J.Json>, i: Maybe<&2, J.Json>) -> Back:
  match r i:
    case Some{+from} _:
      BRestore{from}
    case None{} Some{+from}:
      BIsolate{from}
    case None{} None{}:
      BNew{}

def ws_back(+ws: J.Json) -> Back:
  ws_back.of(J.get(ws, "restore"), J.get(ws, "isolate"))

def ws_with_restore(+ws: J.Json, +from: J.Json) -> J.Json:
  J.set(ws, "restore", from)

def ws_with_isolate(+ws: J.Json, +from: J.Json) -> J.Json:
  J.set(ws, "isolate", from)

# `sb spawn --feature <f>` (dev-flow §5.1): its worktree from f's tip
def ws_feature(+ws: J.Json) -> String:
  J.get_str_or(ws, "feature", "")

def ws_with_feature(+ws: J.Json, +f: String) -> J.Json:
  J.set(ws, "feature", J.JStr{f})

# a mate that joined a restore (joined_ws): it waits for the same worktree
def ws_restore_joined(+ws: J.Json) -> Bool:
  J.get_bool_or(ws, "restore_joined", False{})

def ws_with_restore_joined(+ws: J.Json) -> J.Json:
  J.set(ws, "restore_joined", J.JBool{True{}})

def no_place(m: Maybe<&2, String>) -> Bool:
  match m:
    case None{}:
      True{}
    case Some{_}:
      False{}

# the shell makes this ws's worktree: placing, and its own (a ws with a
# `place` is a join, `sb spawn --place A` while A is placing: A's comes)
def makes(+ws: J.Json) -> Bool:
  no_place(J.get_str(ws, "place")) && M.ws_placing(ws)

def shared_ws(+path: String) -> J.Json:
  J.obj([J.kv("mode", J.JStr{"shared"}), J.kv("path", J.JStr{path}), J.kv("branch", J.JNull{}),
    J.kv("base_commit", J.JNull{}), J.kv("dropped", J.JBool{False{}})])

# a git answer of the shell (moved from core.bend, its line limit)
def ans_ok.go(m: Maybe<&2, J.Json>, +j: J.Json) -> Result<&2, &2, String, J.Json>:
  match m:
    case Some{+v}:
      Done{v}
    case None{}:
      Fail{J.get_str_or(j, "err", "git error")}

def ans_ok(+j: J.Json) -> Result<&2, &2, String, J.Json>:
  ans_ok.go(J.get(j, "ok"), j)

# what one `placed` input does
type Do is Data:
  # the worktree is made: the task's workspace, then its REPL (`back`:
  # a restore's resumes with its saved work used, an isolate's resumes)
  POk{name: String, ws: J.Json, back: Back}
  # it could not be made: a new task fails, `parent` told once; a
  # restore's goes back archived, an isolate's back to the shared folder
  PErr{name: String, err: String, parent: String, back: Back}
  # made for a task gone or no longer placing: it goes again
  POrphan{name: String, ws: J.Json}
  # an error for a task not placing: nothing to undo
  PNone{}

def parent_of(m: Maybe<&2, String>) -> String:
  match m:
    case Some{+p}:
      p
    case None{}:
      M.MAIN()

# a live placing task's worktree: made or not; `back`: where its placing
# ws came from (ws_back)
def placed_of.made(+back: Back, +a: S.Agent, +name: String, +ws: J.Json, +err: String) -> Do:
  Bool.pick(Do, String.is_empty(err), POk{name, ws, back}, PErr{name, err, parent_of(S.Agent.parent(a)), back})

# the task found, `live`: placing and not archived
def placed_of.live(live: Bool, +a: S.Agent, +name: String, +ws: J.Json, +err: String) -> Do:
  match live:
    case True{}:
      placed_of.made(ws_back(S.Agent.ws(a)), a, name, ws, err)
    case False{}:
      Bool.pick(Do, String.is_empty(err), POrphan{name, ws}, PNone{})

def is_archived(l: S.Life) -> Bool:
  match l:
    case S.LArchived{}:
      True{}
    case _:
      False{}

def placed_of.agent(m: Maybe<&2, S.Agent>, +name: String, +ws: J.Json, +err: String) -> Do:
  match m:
    case None{}:
      Bool.pick(Do, String.is_empty(err), POrphan{name, ws}, PNone{})
    case Some{+a}:
      placed_of.live(M.ws_placing(S.Agent.ws(a)) && Bool.not(is_archived(S.Agent.life(a))), a, name, ws, err)

# the decision of `placed` for task `name` (`err` "": `ws` is made)
def placed_of(+st: S.St, +name: String, +ws: J.Json, +err: String) -> Do:
  placed_of.agent(M.get_agent(st, name), name, ws, err)

# a placing task gets no REPL yet (core.bend's `spawn`)
def waits(m: Maybe<&2, S.Agent>) -> Bool:
  match m:
    case Some{+a}:
      M.ws_placing(S.Agent.ws(a))
    case None{}:
      False{}

# the spawner's words when the worktree could not be made
def err_text(+name: String, +err: String) -> String:
  String.append(String.append(String.append("@", name), " could not start: its worktree was not made: "), err)

# main's line when its worktree is made: `@fix's worktree is ready: sb/fix`
def ready_text(+name: String, +ws: J.Json) -> String:
  String.concat(["@", name, "'s worktree is ready: ", J.get_str_or(ws, "branch", ""), " (", J.get_str_or(ws, "path", ""), ")"])

# the spawn's reply: the task is placing until its worktree is made
def placing_note() -> String:
  "placing: it starts once its worktree is made"

# ---- a drop's words (moved from core.bend) ----

def plural(+n: Nat) -> String:
  Bool.pick(String, Nat.is_gt(n, 1n), "s", "")

def count(+n: Nat, +what: String) -> String:
  String.concat([Nat.show(n), " ", what, plural(n)])

def kept(+n: Nat, +what: String) -> String:
  String.concat(["its ", count(n, what), Bool.pick(String, Nat.is_gt(n, 1n), " are kept", " is kept")])

# what a drop keeps of its work (drop-card, designer m_35655): only the
# counts that aren't 0; both 0: nothing is lost
def kept_text(+dirty: Nat, +unpushed: Nat) -> String:
  Bool.pick(String, Nat.is_gt(dirty, 0n),
    Bool.pick(String, Nat.is_gt(unpushed, 0n),
      String.concat(["its ", count(dirty, "changed file"), " and ", count(unpushed, "unpushed commit"), " are kept; /restore brings it back"]),
      String.append(kept(dirty, "changed file"), "; /restore brings it back")),
    Bool.pick(String, Nat.is_gt(unpushed, 0n), String.append(kept(unpushed, "unpushed commit"), "; /restore brings it back"), "nothing of its work is lost"))

# why a drop asks first: mid-turn (designer m_37336), what it keeps
def drop_body(+busy: Bool, +dirty: Nat, +unpushed: Nat) -> String:
  String.concat([Bool.pick(String, busy, "it's working right now: archiving stops its turn. ", ""), kept_text(dirty, unpushed), "."])

# a drop card's options: 1 archives like `yes`, 2 keeps like `no`
# (core.bend answer.drop, picks.bend picks)
def drop_opts() -> String:
  "
1. archive it
2. keep it"

# main's drop card (its title says 'archive <agent>?')
def drop_card(+body: String) -> String:
  String.concat(["main suggests it. ", body, drop_opts()])

# his own /archive's card: he asked, the facts alone (architect m_37339)
def drop_asked(+body: String) -> String:
  String.append(body, drop_opts())

# ---- a drop off the loop (issue 32 3b): the archive is committed, its
# ws `dropping` while the shell saves and removes its worktree; the
# shell's `unplaced` ends it (core.bend's unplaced.in) ----

# the ws of a task whose worktree the shell is removing: what to save
# rides with it, so a stopped hub's `boot` asks again with the same
def dropping_ws(+ws: J.Json, +dirty: Nat, +unpushed: Nat) -> J.Json:
  J.set(J.set(J.set(ws, "state", J.JStr{"dropping"}), "drop_dirty", J.JNum{dirty}), "drop_unpushed", J.JNum{unpushed})

# the drop's line in main's feed while it runs
def removing_note() -> String:
  " · its worktree is being removed"

# main's feed when the shell could not remove it (git's words)
def undropped_text(+n: String, +err: String) -> String:
  String.concat(["@", n, "'s worktree was not removed: ", err, ". it stays: /restore brings @", n, " back in it"])

# a restore while the shell still removes its worktree
def dropping_text(+n: String) -> String:
  String.concat(["@", n, " is still being archived: restore it in a moment"])

# ---- a restore off the loop (issue 32 3b): the task is active at once,
# its ws placing while the shell makes its dropped worktree again (fx
# worktree_restore); the shell's `placed` ends it (POk, PErr: BRestore)

# the ws of a task whose dropped worktree the shell makes again: placing
# (no REPL yet, its messages wait), `restore` the dropped ws to go back to
def restoring_ws(+ws: J.Json) -> J.Json:
  ws_with_restore(J.set(J.set(ws, "state", J.JStr{"placing"}), "dropped", J.JBool{False{}}), ws)

# a mate's /restore while its place is being made again (restore.join):
# it waits for the same worktree, marked so a boot asks for it once
def joined_ws.of(b: Back, +ws: J.Json) -> J.Json:
  match b:
    case BRestore{_}:
      ws_with_restore_joined(ws)
    case _:
      ws

def joined_ws(+ws: J.Json) -> J.Json:
  joined_ws.of(ws_back(ws), ws)

def boot_back.go(joined: Bool, +ws: J.Json) -> Back:
  match joined:
    case False{}:
      ws_back(ws)
    case True{}:
      BNew{}

# what a boot asks again for a placing ws: its own restore, never a
# joiner's (it waits for the same worktree), else as a new one (place.ask)
def boot_back(+ws: J.Json) -> Back:
  boot_back.go(ws_restore_joined(ws), ws)

# the restore's line in main's feed while it runs
def restoring_note() -> String:
  " · its worktree is being made again"

# main's feed when the shell could not make it again (git's words)
def unrestored_text(+n: String, +err: String) -> String:
  String.concat(["@", n, " could not be restored: its worktree was not made again: ", err, ". it stays archived, its saved work kept"])

# ---- an isolate off the loop (issue 32 3c): `/isolate x` stops x's
# REPL, its ws placing while the shell makes its worktree (fx
# worktree_create); the shell's `placed` ends it (POk, PErr: BIsolate) ----

# the ws of a task the shell isolates: a new worktree's placing ws,
# `isolate` the shared ws it goes back to if git fails
def isolating_ws(+ws: J.Json) -> J.Json:
  ws_with_isolate(placing_ws(False{}), ws)

# main's line now, and when its worktree is made
def isolating_text(+n: String) -> String:
  String.concat(["@", n, " is being isolated: its worktree is being made"])

def isolated_text(+n: String, +ws: J.Json) -> String:
  String.concat(["@", n, " now works in the worktree ", J.get_str_or(ws, "branch", "")])

# the isolated agent's own word on its new folder
def isolated_note(+ws: J.Json) -> String:
  String.concat(["[bise] Your working directory is now the isolated git worktree `", M.ws_path(ws), "` (branch `",
    J.get_str_or(ws, "branch", ""), "`). Work only there from now on."])

# main's feed when the shell could not make it (git's words)
def unisolated_text(+n: String, +err: String) -> String:
  String.concat(["@", n, " could not be isolated: its worktree was not made: ", err, ". it goes on in the shared folder"])

# a `placed` that starts the task (laws: placed_create_once)
def is_ok(d: Do) -> Bool:
  match d:
    case POk{_, _, _}:
      True{}
    case _:
      False{}

# dev-flow §3.1: the place of a ws: "shared", or its worktree's id (the
# Rust Workspace::place_id; an older journal's worktree has none: its
# agent's own place, wt:<dir>). Moved from model.bend (its line limit).
def ws_place(+ws: J.Json, +dir: String) -> String:
  Bool.pick(String, M.ws_worktree(ws), J.get_str_or(ws, "place", String.append("wt:", dir)), "shared")

def Agent.place(+a: S.Agent) -> String:
  ws_place(S.Agent.ws(a), S.Agent.dir(a))

# ---- a place's agents (moved from core.bend, its line limit) ----

def is_gone(l: S.Life) -> Bool:
  match l:
    case S.LArchived{}:
      True{}
    case _:
      False{}

# `b` is another agent of `place`'s live folder: still in it (`live`), or
# archived with its ws still on that folder (it left before the others)
def mate.hit(m: Maybe<&2, S.Agent>, +name: String, +place: String, +live: Bool) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{+b}:
      +gone = is_gone(S.Agent.life(b))
      Bool.not(String.eq(S.Agent.name(b), name)) && M.ws_worktree(S.Agent.ws(b)) && Bool.not(M.ws_dropped(S.Agent.ws(b)))
        && String.eq(Agent.place(b), place) && Bool.pick(Bool, live, Bool.not(gone), gone)

def mates(xs: List<&2, S.Agent>, +name: String, +place: String, +live: Bool) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+b, +t}:
      +rest = mates(t, name, place, live)
      Bool.pick(List<&2, String>, mate.hit(Some{b}, name, place, live), S.Agent.name(b) <> rest, rest)

# the names of `ord` (the creation order) that are in `hs`
def in_order(ord: List<&2, String>, +hs: List<&2, String>) -> List<&2, String>:
  match ord:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = in_order(t, hs)
      Bool.pick(List<&2, String>, M.str_in(hs, n), n <> rest, rest)

def in_order.if(hs: List<&2, String>, +ord: List<&2, String>) -> List<&2, String>:
  match hs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      in_order(ord, h <> t)

# the other agents of a's worktree (none for the shared folder), in
# creation order: the live ones are in St.agents, the archived ones in
# arch (the agents split), so one walk of that list, not St.order with a
# lookup per name (O(N^2) in the archived agents: c4b's leftover, every
# sb drop paid it)
def mates_of(+s: S.St, +a: S.Agent, +live: Bool) -> List<&2, String>:
  +xs = Bool.pick(List<&2, S.Agent>, live, S.St.agents(s), S.St.arch(s))
  in_order.if(mates(xs, S.Agent.name(a), Agent.place(a), live), S.St.order(s))

def first_agent(+s: S.St, ns: List<&2, String>) -> Maybe<&2, S.Agent>:
  match ns:
    case Nil{}:
      None{}
    case Con{+n, _}:
      M.get_agent(s, n)

def drop.stays(ms: List<&2, String>) -> String:
  match ms:
    case Nil{}:
      ""
    case Con{+h, +t}:
      String.append(" — its worktree stays with @", String.join({h <> t : List<&2, String>}, ", @"))

def drop.left.ws(m: Maybe<&2, S.Agent>) -> J.Json:
  match m:
    case None{}:
      J.JNull{}
    case Some{+b}:
      J.set(S.Agent.ws(b), "dropped", J.JBool{True{}})

# ---- a join while placing (`sb spawn B --place A`, A placing) ----
# B is created with the daemon's ws for A's place (state placing, place
# wt:<A's dir>), no worktree of its own (makes: False), no REPL (waits).
# A's `placed` then places B the same way: A's worktree, or A's failure.

def placing_of(xs: List<&2, String>, +s: S.St) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      +rest = placing_of(t, s)
      Bool.pick(List<&2, String>, waits(M.get_agent(s, n)), n <> rest, rest)

# the joiners of `name`'s place: exactly the live agents whose ws.place
# is name's place (wt:<its dir>) and whose state is placing; never any
# other placing agent (one placing its own worktree has no ws.place)
def joiners.of(m: Maybe<&2, S.Agent>, +s: S.St) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+a}:
      placing_of(mates_of(s, a, True{}), s)

def joiners(+s: S.St, +name: String) -> List<&2, String>:
  joiners.of(M.get_agent(s, name), s)

# a joiner's `placed`: the ws its place's got (none when it failed) ...
def join.ws(d: Do) -> J.Json:
  match d:
    case POk{_, +ws, _}:
      ws
    case _:
      J.JNull{}

# ... and the error: "" when made, else its place's words (`err`, git's)
def join.err(d: Do, +err: String) -> String:
  match d:
    case POk{_, _, _}:
      ""
    case PErr{_, +e, _, _}:
      e
    case POrphan{+n, _}:
      String.concat(["@", n, " left before its worktree was made"])
    case PNone{}:
      err
