# hub/main.bend — sb-core: the Switchboard hub's decisions as a process.
#
#   bend hub/main.bend -o sb-core && SB_CORE_PORT=7790 ./sb-core
#
# The Rust daemon connects over TCP (127.0.0.1) and writes one JSON input
# per line; sb-core answers one JSON line per input:
#   {"fx": [...], "dirty": bool}   the effects of the input (committed)
#   {"need": {...}}                an env query (git): the daemon runs it and
#                                  sends the same input again with the answer
#                                  appended to "ans" (nothing was committed)
# Special inputs: {"t":"init","workspace":..} resets the hub, {"t":"replay",
# "ev":..} applies one journal event (boot; answers {"skipped":true} for a
# kind it does not know), {"t":"ping"} answers {}.
# The state lives across connections.
import Base

import ../vendor/json.bend as J

import ./model.bend as M
import ./cold.bend as CO

import ./gen/state.bend as S

import ./cx.bend as X

import ./core.bend as H

import ./view.bend as V

# his message a step made (A1: AStep.msg)
import ./said.bend as SD

# his answer to a card a step made (inbox picks (3): AStep.ans_*)
import ./picks.bend as PK

# the link's requests and answers, generated from bise_proto::core
import ./gen/link.bend as L

# the hub's photo (issue 36): photo / restore_photo
import ./photo.bend as PH

import ./gen/journal.bend as GJ

import ./gen/fx.bend as F

# `part`: the chars read after the last newline, newest first (Cut)
type Got is Type:
  Got{s: Socket, h: H.Hub, part: String}

type Cut is Data:
  Line{line: String, rest: String}
  More{part: String}

type Flow is Type:
  Go{h: H.Hub}

def fx_json(f: X.Fx) -> J.Json:
  match f:
    case X.FJ{+e}:
      J.obj(F.journal(GJ.json(e)))
    case X.FO{+o}:
      o

def fxs_json(xs: List<&2, X.Fx>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      fx_json(h) <> fxs_json(t)

# `before`: the runtime and wait marks before the step (V.changed). The
# view carries only what the step changed: an idle tick answers a few
# hundred bytes, whatever the history (idle-cpu). `msg`: the id of his
# message the step made (SD.said), for the asker's send result (A1).
# `ans_*`: his answer to a card in the step (X.R.answered), for the
# answer's result (inbox picks (3), architect m_22636)
def answer.go(need: Maybe<&2, J.Json>, +h: H.Hub, +fx: List<&2, X.Fx>, +before: List<&2, V.Mark>) -> H.Hub & String:
  match need:
    case Some{+q}:
      (h, J.stringify(L.answer_need(q)))
    case None{}:
      (h, J.stringify(L.answer_step(fxs_json(fx), X.R.dirty(H.Hub.r(h)),
        V.view(H.Hub.d(h), X.R.rts(H.Hub.r(h)), X.R.waiters(H.Hub.r(h)), False{}, List.append(&2, Nat, V.touched(X.journal(fx)), V.moved_ids(X.journal(fx), S.St.hot(H.Hub.d(h)))),
          List.append(&2, String, V.changed(X.journal(fx), before, X.R.rts(H.Hub.r(h)), X.R.waiters(H.Hub.r(h))), V.fx_names(fx)),
          X.journal(fx)),
        SD.said_id(SD.said(X.journal(fx))),
        PK.card_of(X.R.answered(H.Hub.r(h))), PK.who_of(X.R.answered(H.Hub.r(h))), PK.said_of(X.R.answered(H.Hub.r(h))),
        PK.ask_of(X.R.answered(H.Hub.r(h))), PK.pick_of_ans(X.R.answered(H.Hub.r(h))))))

# the answer line of a committed step (H.commit, the law run_replays)
def answer(+before: List<&2, V.Mark>, +o: H.Turn) -> H.Hub & String:
  answer.go(H.Turn.need(o), H.Turn.hub(o), H.Turn.fx(o), before)

# an event this hub does not know (written by a newer hub, before a
# /version rollback), or a message_sent with a reused or older id, is
# not applied (CO.replay_fresh): the answer says so, the Rust side logs it
def replay(+h: H.Hub, m: Maybe<&2, M.Ev>) -> H.Hub & String:
  match m:
    case None{}:
      (h, J.stringify(L.answer_skipped(True{})))
    case Some{+e}:
      +ok = CO.replay_fresh(H.Hub.d(h), e)
      (H.Hub{CO.replay_one(ok, H.Hub.d(h), e), H.Hub.r(h), False{}}, J.stringify(Bool.pick(J.Json, ok, L.answer_done(), L.answer_skipped(True{}))))

# {"t":"replay_many","evs":[..]}: a batch of journal events at boot,
# answered {"skipped":[<index in the batch>..]} (CO.replay_batch).
def evs_of(evs: List<&2, J.Json>) -> List<&2, Maybe<&2, M.Ev>>:
  match evs:
    case Nil{}:
      Nil{}
    case Con{+e, +rest}:
      GJ.of(e) <> evs_of(rest)

def replay_many.done(p: S.St & List<&2, Nat>, +r: X.R) -> H.Hub & String:
  match p:
    case (+d, +sk):
      (H.Hub{d, r, False{}}, J.stringify(L.answer_skipped_at(sk)))

def replay_many(+h: H.Hub, +evs: List<&2, J.Json>) -> H.Hub & String:
  replay_many.done(CO.replay_batch(evs_of(evs), H.Hub.d(h), 0n, Nil{}), H.Hub.r(h))

# `photo`: the durable state's photo, or why not (an env query pending)
def photo.answer(m: Maybe<&2, J.Json>) -> J.Json:
  match m:
    case Some{+st}:
      L.answer_photo(st)
    case None{}:
      L.answer_error("photo: an env query is pending")

# `restore_photo`: the hub from the photo; not an object: unchanged
def restore_photo(m: Maybe<&2, H.Hub>, +h: H.Hub) -> H.Hub & String:
  match m:
    case Some{+h2}:
      (h2, J.stringify(L.answer_done()))
    case None{}:
      (h, J.stringify(L.answer_error("restore_photo: only a hub that applied nothing yet, from a photo object")))

# the link's own requests (gen/link.bend, from bise_proto::core::Link);
# any other tag is a step of sb-core
def handle.link(l: L.Link, +h: H.Hub, +i: J.Json) -> H.Hub & String:
  match l:
    case L.TInit{+workspace}:
      (H.hub_new(workspace), J.stringify(L.answer_done()))
    case L.TReplay{+ev}:
      replay(h, GJ.of(ev))
    case L.TReplayMany{+evs}:
      replay_many(h, evs)
    case L.TPing{}:
      (h, J.stringify(L.answer_done()))
    case L.TView{}:
      (h, J.stringify(L.answer_view(V.view(H.Hub.d(h), X.R.rts(H.Hub.r(h)), X.R.waiters(H.Hub.r(h)), True{}, Nil{}, Nil{}, Nil{}))))
    # view-all-perf: the bodies view_all left out, as a step's view of
    # those messages (whole, no agent, nothing else)
    case L.TMsgBodies{+ids}:
      (h, J.stringify(L.answer_view(V.view(H.Hub.d(h), X.R.rts(H.Hub.r(h)), X.R.waiters(H.Hub.r(h)), False{}, ids, Nil{}, Nil{}))))
    case L.TPhoto{}:
      (h, J.stringify(photo.answer(PH.photo(h))))
    case L.TRestorePhoto{+st}:
      restore_photo(PH.restore(h, st), h)
    case L.TStep{}:
      answer(V.marks(X.R.rts(H.Hub.r(h)), X.R.waiters(H.Hub.r(h))), H.commit(h, i))

def handle.parsed(r: Result<&2, &2, String, J.Json>, +h: H.Hub) -> H.Hub & String:
  match r:
    case Fail{+e}:
      (h, J.stringify(L.answer_error(String.append("json: ", e))))
    case Done{+i}:
      handle.link(L.of(i), h, i)

# one input line
def handle(+h: H.Hub, +line: String) -> H.Hub & String:
  handle.parsed(J.parse(line), h)

# ---- the line server ----

# a failed send's error carries the bytes the kernel did not take (bend
# 2.0.38: a List of U32, was a String): dropped, as the failed send itself
# always was
def send_line.after(r: Socket & Result<&1, &1, (U32 & String) & List<&2, U32>, Unit>) -> IO(Socket):
  match r:
    case (s, _):
      IO.pure(Socket, s)

# the text, then its newline: two sends; appending the newline copied
# the whole answer to add one char (view_all: 30 MB, ~0.2 s)
def send_line(s: Socket, +text: String) -> IO(Socket):
  do IO<Socket>:
    r0 : Socket & Result<&1, &1, (U32 & String) & List<&2, U32>, Unit> <- TCP.send(s, text)
    s2 : Socket <- send_line.after(r0)
    r : Socket & Result<&1, &1, (U32 & String) & List<&2, U32>, Unit> <- TCP.send(s2, "\n")
    send_line.after(r)

# the received `data` after `part` (the line so far, newest first): the
# first whole line and the data after it, or More: the line so far with
# `data` in it. Each char is read once: a line that comes in many recvs
# is never appended to nor scanned again (the old buffer was appended to
# and rescanned from its start at each recv: quadratic in a big line)
def cut(data: String, +part: String) -> Cut:
  match data:
    case SNil{}:
      More{part}
    case SCon{'\n', +t}:
      Line{String.reverse(part), t}
    case SCon{+c, +t}:
      # a pattern, not Bool.pick: pick computes both of its arguments, so
      # it reversed the whole prefix at every char (quadratic in the line)
      cut(t, SCon{c, part})

@unsafe
def lines(s: Socket, +h: H.Hub, +part: String, +data: String) -> IO(Got):
  lines.split(cut(data, part), s, h)

@unsafe
def lines.answer(p: H.Hub & String, s: Socket, +rest: String) -> IO(Got):
  match p:
    case (+h2, +out):
      do IO<Got>:
        s2 : Socket <- send_line(s, out)
        lines(s2, h2, "", rest)

@unsafe
def lines.split(m: Cut, s: Socket, +h: H.Hub) -> IO(Got):
  match m:
    case More{+part}:
      IO.pure(Got, Got{s, h, part})
    case Line{+l, +rest}:
      lines.answer(handle(h, l), s, rest)

@unsafe
def conn.after(rm: Socket & Result<&1, &1, U32 & String, Maybe<&1, String>>, +h: H.Hub, +part: String) -> IO(H.Hub):
  match rm:
    case (s, Fail{_}):
      do IO<H.Hub>:
        Socket.close(s)
        IO.pure(H.Hub, h)
    case (s, Done{m}):
      conn.got(s, h, part, recv_text(m))

# TCP.recv's None (bend 2.0.37: the peer sent everything) is the closed
# connection the empty text was before
def recv_text(m: Maybe<&1, String>) -> String:
  match m:
    case None{}:
      ""
    case Some{+d}:
      d

@unsafe
def conn.got(s: Socket, +h: H.Hub, +part: String, +data: String) -> IO(H.Hub):
  conn.data(String.is_empty(data), s, h, part, data)

@unsafe
def conn(s: Socket, +h: H.Hub, +part: String) -> IO(H.Hub):
  do IO<H.Hub>:
    # TCP.recv decodes each recv on its own: a multi-byte char a recv
    # boundary cuts would read as U+FFFD. Safe only because the hub writes
    # every line ASCII-only (bise_proto::ascii, CoreLink::call; its test
    # pins no byte >= 0x80, issue 36); the REPL's reader decodes bytes
    # across recvs instead (core/utf8.bend)
    rm : Socket & Result<&1, &1, U32 & String, Maybe<&1, String>> <- TCP.recv(s, 65536)
    conn.after(rm, h, part)

@unsafe
def conn.next(r: Got) -> IO(H.Hub):
  match r:
    case Got{s, +h, +part}:
      conn(s, h, part)

@unsafe
def conn.data(closed: Bool, s: Socket, +h: H.Hub, +part: String, +data: String) -> IO(H.Hub):
  match closed:
    case True{}:
      do IO<H.Hub>:
        Socket.close(s)
        IO.pure(H.Hub, h)
    case False{}:
      do IO<H.Hub>:
        r : Got <- lines(s, h, part, data)
        conn.next(r)

@unsafe
def serve.after(am: Listener & Result<&1, &1, U32 & String, Socket>, +h: H.Hub) -> IO(Unit):
  match am:
    case (l, Fail{_}):
      IO.print("sb-core: accept failed")
    case (l, Done{s}):
      # one connection: its end (the daemon exited or was killed) ends
      # sb-core, so no orphan survives the hub
      do IO<Unit>:
        h2 : H.Hub <- conn(s, h, "")
        IO.pure(Unit, Unit{})

@unsafe
def serve(l: Listener, +h: H.Hub) -> IO(Unit):
  do IO<Unit>:
    am : Listener & Result<&1, &1, U32 & String, Socket> <- TCP.accept(l)
    serve.after(am, h)

def port_of(r: Result<&1, &1, U32 & String, String>) -> U32:
  match r:
    case Fail{_}:
      7790
    case Done{+s}:
      Maybe.default(&2, U32, U32.read(s), 7790)

def main() -> IO(Unit):
  do IO<Unit>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("SB_CORE_PORT")
    +p : U32 <- IO.pure(U32, port_of(pr))
    l : Listener <- IO.try(Listener, TCP.listen("127.0.0.1", p))
    IO.print(String.append("sb-core on 127.0.0.1:", U32.show(p)))
    serve(l, H.hub_new(""))
