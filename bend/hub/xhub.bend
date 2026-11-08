# hub/xhub.bend — the cross-hub decisions (bise desktop S2), pure: what a
# forward of his message needs, the outbox's next step after an ack, a
# failure or an answer, the retry backoff, which inbox record an answer
# goes back for. core.bend's `---- cross-hub (S2) ----` section runs them
# through emit/send/out (it decides nothing itself); the state and its
# journal kinds are xhub-model.bend's. Laws: bend/laws/hub.bend `xhub_*`.
#
# Two kinds (architect's B): his own words, forwarded by reference only
# (`sb project send <p> --input m_<n>`: sb-core checks m_<n> is a message
# he sent this main and copies its text itself, so bise's model never
# retypes words carrying his authority), and bise's own question (`sb
# project ask`, the sender `@bise`, no authority).
#
# Trust: the ops `xin` and `xreply` on hub.sock trust their caller like
# the `input` op does (same uid): an agent could forge one as it could
# forge an input today; not new, not fixed here.
import Base

import ./model.bend as M
import ./gen/state.bend as S

import ./xhub-model.bend as XH

import ./timers.bend as TM

import ./lazy.bend as LZ

import ../vendor/json.bend as J
import ./gen/kit.bend as K

# the sender of bise's own question in a project hub: not a valid agent
# name, so no agent can be it
def FROM_BISE() -> String:
  "@bise"

# the view of his words routed by bise (`<user_message via="bise">`)
def VIA_BISE() -> String:
  "bise"

# ---- a forward of his message (home side) ----

type Fwd is Data:
  FwdErr{why: String}
  FwdOk{text: String}

def mid(+n: Nat) -> String:
  String.append("m_", Nat.show(n))

# `text`: his message's words, as the shell read them from the journal
# (the request's `orig`, c6: the state keeps no words)
def his.go(m: Maybe<&2, S.Msg>, +id: Nat, +text: String) -> Fwd:
  match m:
    case None{}:
      FwdErr{String.append("no message ", mid(id))}
    case Some{+msg}:
      Bool.pick(Fwd, String.eq(S.Msg.from(msg), M.USER()) && String.eq(S.Msg.to(msg), M.MAIN()),
        FwdOk{text},
        FwdErr{String.append(mid(id), " is not a message the user sent you: forward his own words only (your own question: sb project ask)")})

def forward.done(m: Maybe<&2, S.Out>, +s: S.St, +id: Nat, +text: String) -> Fwd:
  match m:
    case Some{+o}:
      FwdErr{String.append(String.append(mid(id), " was already forwarded to "), S.Out.project(o))}
    case None{}:
      his.go(M.msg_of(s, id), id, text)

# `sb project send --input m_<id>` by `from`: only main, only a message
# the user sent main, once per message (law xhub_forward_needs_his_message)
def forward(+s: S.St, +from: String, +id: Nat, +text: String) -> Fwd:
  Bool.pick(Fwd, String.eq(from, M.MAIN()),
    forward.done(XH.out_by_msg(S.Xs.outbox(S.St.x(s)), id), s, id, text),
    FwdErr{"only main forwards to the projects"})

# bise's own question (`sb project ask`): words, never a blank (law
# xhub_ask_needs_words); None: it may go
def ask_ok(+text: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, M.trim_is_empty(text), Some{"empty question"}, None{})

# the outbox entry of a new send to `project`: the next xid, pending,
# never tried; each site sets what its kind carries (msg, text) by name
def out_to(+x: S.Xs, +project: String, +k: S.XKind) -> S.Out:
  S.Out.with_kind(S.Out.with_project(S.Out.with_xid(S.Out.empty(), S.Xs.next_xid(x)), project), k)

# a fired route's outbox entry: his words (message `id`), their context
# and parts (R41 (6)), to the route's project, every field by name
def out_of_route(+x: S.Xs, +r: S.Route, +id: Nat) -> S.Out:
  +o = S.Out.with_text(S.Out.with_msg(out_to(x, S.Route.to(r), S.KInput{}), id), S.Route.text(r))
  S.Out.with_parts(S.Out.with_context(o, S.Route.context(r)), S.Route.parts(r))

# ---- the outbox's steps (home side) ----

def pending.go(+o: S.Out, s: S.OutSt) -> Maybe<&2, S.Out>:
  match s:
    case S.OPending{}:
      Some{o}
    case _:
      None{}

def pending.of(m: Maybe<&2, S.Out>) -> Maybe<&2, S.Out>:
  match m:
    case None{}:
      None{}
    case Some{+o}:
      pending.go(o, S.Out.st(o))

# an ack or a failure counts only for a send still pending (a late ack
# or failure of a settled send changes nothing)
def pending(+x: S.Xs, +xid: Nat) -> Maybe<&2, S.Out>:
  pending.of(XH.out_of(S.Xs.outbox(x), xid))

def open.go(+o: S.Out, s: S.OutSt) -> Maybe<&2, S.Out>:
  match s:
    case S.OSettled{}:
      None{}
    case _:
      Some{o}

def open.of(m: Maybe<&2, S.Out>) -> Maybe<&2, S.Out>:
  match m:
    case None{}:
      None{}
    case Some{+o}:
      open.go(o, S.Out.st(o))

# the send an answer settles: known and not settled yet (law
# xhub_outbox_settles_once: after its settle, none)
def open(+x: S.Xs, +xid: Nat) -> Maybe<&2, S.Out>:
  open.of(XH.out_of(S.Xs.outbox(x), xid))

def is_pending(s: S.OutSt) -> Bool:
  match s:
    case S.OPending{}:
      True{}
    case _:
      False{}

def pendings(xs: List<&2, S.Out>) -> List<&2, S.Out>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      +rest = pendings(t)
      Bool.pick(List<&2, S.Out>, is_pending(S.Out.st(h)), h <> rest, rest)

# the next try of a failed delivery: 1 s, 2 s, 4 s ... at most 60 s
def backoff(+tries: Nat) -> Nat:
  Nat.min(Nat.mul(1000n, Nat.pow(2n, Nat.min(tries, 6n))), 60000n)

# ---- the inbox (project side) ----

def seen.of(m: Maybe<&2, S.XIn>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{_}:
      True{}

# a delivery of an origin already received adds nothing (law
# xhub_xin_delivered_once)
def seen(+x: S.Xs, +hub: String, +xid: Nat) -> Bool:
  seen.of(XH.xin_of(S.Xs.xin(x), hub, xid))

def reply.go(+i: S.XIn, r: Bool) -> Maybe<&2, S.XIn>:
  match r:
    case True{}:
      None{}
    case False{}:
      Some{i}

def reply.of(m: Maybe<&2, S.XIn>) -> Maybe<&2, S.XIn>:
  match m:
    case None{}:
      None{}
    case Some{+i}:
      reply.go(i, S.XIn.replied(i))

# the inbox record an answer to message `msg` goes back for: one not
# answered yet (law xhub_xin_replied_once)
def reply_due(+x: S.Xs, +msg: Nat) -> Maybe<&2, S.XIn>:
  reply.of(XH.xin_by_msg(S.Xs.xin(x), msg))

# ---- the acts ----

# the act of a retry of `xid` after `tries` failures, due from `now`
def retry_act(+tm: S.Tms, +xid: Nat, +tries: Nat, +now: Nat) -> S.Act:
  TM.act_of(S.Tms.next_act(tm), Nat.add(now, backoff(tries)), S.Retry{xid})

# ---- the routing hold (home side, step 2) ----

# how long his words wait before they go to the project the shell picked:
# his time to correct or cancel them (`route_correct`, `route_cancel`)
def HOLD_MS() -> Nat:
  2000n

# a new held route from his hold `d` (its words, target and parts set by
# name by XB.route_for): the next rid, due HOLD_MS from `now`,
# its act the clock's next one
def route_of_hold(+x: S.Xs, +tm: S.Tms, +d: S.Route, +now: Nat) -> S.Route:
  S.Route.with_act(S.Route.with_due(S.Route.with_rid(d, S.Xs.next_rid(x)), Nat.add(now, HOLD_MS())), S.Tms.next_act(tm))

# the act that sends a held route at its due time (law
# route_fires_at_due_only: timers.bend's acts_due)
def route_act(+r: S.Route) -> S.Act:
  TM.act_of(S.Route.act(r), S.Route.due(r), S.RouteSend{S.Route.rid(r)})

def is_held(s: S.RSt) -> Bool:
  match s:
    case S.RHeld{}:
      True{}
    case _:
      False{}

def held.go(+r: S.Route, s: S.RSt) -> Maybe<&2, S.Route>:
  match s:
    case S.RHeld{}:
      Some{r}
    case _:
      None{}

def held.of(m: Maybe<&2, S.Route>) -> Maybe<&2, S.Route>:
  match m:
    case None{}:
      None{}
    case Some{+r}:
      held.go(r, S.Route.st(r))

# the route a fire, a correction or a cancel acts on: one still held
# (laws route_one_outcome, route_correct_only_held)
def held(+x: S.Xs, +rid: Nat) -> Maybe<&2, S.Route>:
  held.of(XH.route_of(S.Xs.routes(x), rid))

# the view a routed message carries ("" none)
def via_of(+v: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, String.is_empty(v), None{}, Some{v})

# his routed words as his plain message to main: id `id` (its own thread),
# its text, via and parts from the route (core.bend route_said)
def msg_of_route(+r: S.Route, +id: Nat, +now: Nat) -> S.Sent:
  +m = S.Sent.with_to(S.Sent.with_from(S.Sent.with_thread(S.Sent.with_id(S.Sent.empty(), id), id), M.USER()), M.MAIN())
  +m2 = S.Sent.with_plain(S.Sent.with_created(S.Sent.with_text(m, S.Route.text(r)), now), True{})
  S.Sent.with_parts(S.Sent.with_via(m2, via_of(S.Route.via(r))), S.Route.parts(r))

# ---- a held route with no target ----
# Only an older hub's route model held one (S2 step 5, gone: only his
# explicit `@<project>` routes now); one left in a journal fires plain to
# bise's main (law route_unclear_goes_plain).

# held, with no target yet
def is_unclear(+r: S.Route) -> Bool:
  is_held(S.Route.st(r)) && String.is_empty(S.Route.to(r))

# ---- his words' route (home side, step 1; issue 37 #1) ----
# His words to the agent in view go to a project only when he addressed
# it: `@<project> words`, a registered project's name (ASCII case aside)
# as their first word, words after it, no agent of this hub by that name
# (an agent wins), sent to main now, not a command, not fn space's 'start
# an agent'. Never because a name, a path or a URL in his words points
# there (his m_75). core.bend's route_ask holds what `addressed` picks.
# Laws: bend/laws/hub.bend `route_ask_*`, `addressed_*`, `address_*`.

# a registered project his words may be addressed to: (its hub id, what
# a route names; its name in the projects list)
def Proj() -> Data:
  Sigma<&2, &2, String, _ => String>

def proj.id(p: Proj()) -> String:
  (id, _) = p
  id

def proj.name(p: Proj()) -> String:
  (_, name) = p
  name

# the registered projects of input route_ask (`{id, name}` each)
def projs(js: List<&2, J.Json>) -> List<&2, Proj()>:
  match js:
    case Nil{}:
      Nil{}
    case Con{+j, +t}:
      (K.get_s(j, "id"), K.get_s(j, "name")) <> projs(t)

# Rust's char::is_whitespace, as route.rs read his words: ASCII's 9-13
# and 32, then U+0085, U+00A0, U+1680, U+2000-U+200A, U+2028, U+2029,
# U+202F, U+205F, U+3000
def is_ws(c: Char) -> Bool:
  match c:
    case Chr{+x}:
      (U32.is_ge(x, 9) && U32.is_le(x, 13)) || U32.is_eq(x, 32) || U32.is_eq(x, 133) || U32.is_eq(x, 160) || U32.is_eq(x, 5760) || (U32.is_ge(x, 8192) && U32.is_le(x, 8202)) || U32.is_eq(x, 8232) || U32.is_eq(x, 8233) || U32.is_eq(x, 8239) || U32.is_eq(x, 8287) || U32.is_eq(x, 12288)

def is_code(c: Char, +n: U32) -> Bool:
  match c:
    case Chr{+x}:
      U32.is_eq(x, n)

# the text from its first non-blank char: walks the leading blanks only
def skip_ws(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{+h, +t}:
      LZ.pick(String, is_ws(h), u => skip_ws(t), u => SCon{h, t})

# a name's trailing ':' and ',' cut (`@harness: fix it`)
def drop_marks(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{+h, +t}:
      LZ.pick(String, is_code(h, 58) || is_code(h, 44), u => drop_marks(t), u => SCon{h, t})

def cut_marks(+s: String) -> String:
  String.reverse(drop_marks(String.reverse(s)))

# (the first word, what follows its first blank); None: no blank
def head.cons(+c: Char, m: Maybe<&2, Proj()>) -> Maybe<&2, Proj()>:
  match m:
    case None{}:
      None{}
    case Some{(+name, +words)}:
      Some{(SCon{c, name}, words)}

def head(s: String) -> Maybe<&2, Proj()>:
  match s:
    case SNil{}:
      None{}
    case SCon{+h, +t}:
      LZ.pick(Maybe<&2, Proj()>, is_ws(h), u => Some{("", t)}, u => head.cons(h, head(t)))

def address.ok(+name: String, +words: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, String.is_empty(name) || String.is_empty(skip_ws(words)), None{}, Some{name})

def address.of(m: Maybe<&2, Proj()>) -> Maybe<&2, String>:
  match m:
    case None{}:
      None{}
    case Some{(+name, +words)}:
      address.ok(cut_marks(name), words)

def address.at(s: String) -> Maybe<&2, String>:
  match s:
    case SNil{}:
      None{}
    case SCon{+h, +t}:
      LZ.pick(Maybe<&2, String>, is_code(h, 64), u => address.of(head(t)), u => None{})

# the `@name` his words start with (None: no address, or no words after
# it). Reads the head of his text only: its leading blanks, the name up
# to the first blank, then the words' leading blanks up to their first
# char; a pasted 200 KB text is never walked (core_tests pins it).
def address(+text: String) -> Maybe<&2, String>:
  address.at(skip_ws(text))

# his words, if they may go to a project: sent to main (or the agent in
# view unset) now, not empty, not a command (an `/answer` included), not
# fn space's 'start an agent' (bise's main starts it)
def words_start(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{+h, _}:
      Bool.not(is_code(h, 47))

def routable(queued: Bool, +focus: String, +text: String, +via: String) -> Bool:
  Bool.not(queued) && (String.is_empty(focus) || String.eq(focus, M.MAIN())) && words_start(skip_ws(text)) && Bool.not(String.eq(via, "capsule-start"))

def same_name(+a: String, +b: String) -> Bool:
  String.eq(String.to_lower(a), String.to_lower(b))

# the project named `n`, ASCII case aside: the list's first
def project_named(ps: List<&2, Proj()>, +n: String) -> Maybe<&2, Proj()>:
  match ps:
    case Nil{}:
      None{}
    case Con{+p, +t}:
      LZ.pick(Maybe<&2, Proj()>, same_name(proj.name(p), n), u => Some{p}, u => project_named(t, n))

# an agent of this hub by that name (or alias): his `@name` is for it
def addressed.free(m: Maybe<&2, String>, +n: String, +ps: List<&2, Proj()>) -> Maybe<&2, Proj()>:
  match m:
    case Some{_}:
      None{}
    case None{}:
      project_named(ps, n)

def addressed.to(m: Maybe<&2, String>, +st: S.St, +ps: List<&2, Proj()>) -> Maybe<&2, Proj()>:
  match m:
    case None{}:
      None{}
    case Some{+n}:
      addressed.free(M.resolve(st, n), n, ps)

# the project his words are addressed to; None: they stay with the agent
# in view. No project (a hub other than bise's home): None, his text
# unread.
def addressed(ps: List<&2, Proj()>, +st: S.St, queued: Bool, +focus: String, +text: String, +via: String) -> Maybe<&2, Proj()>:
  match ps:
    case Nil{}:
      None{}
    case Con{+p, +t}:
      LZ.pick(Maybe<&2, Proj()>, routable(queued, focus, text, via), u => addressed.to(address(text), st, Con{p, t}), u => None{})

# the route his words make for project `p`: his words (whole), his view,
# his fn context and parts; the why he reads
def route_for(+p: Proj(), +text: String, +via: String, +context: String, +parts: String) -> S.Route:
  +r = S.Route.with_to(S.Route.with_via(S.Route.with_text(S.Route.empty(), text), via), proj.id(p))
  +r2 = S.Route.with_why(S.Route.with_name(r, proj.name(p)), String.append("you wrote @", proj.name(p)))
  S.Route.with_parts(S.Route.with_context(r2, context), parts)
