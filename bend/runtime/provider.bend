# runtime/provider.bend — the live provider call.
#
# The HTTPS call through the hub HTTP client, the retry loop, and the
# reply rendered as the OK/CALL/END lines runtime/remote.bend parses.
# The provider table and the retry policy are pure and live in
# runtime/provider-pure.bend (the laws pin them there); this module
# only maps the Http values onto that policy and runs the effects.
import Base
import ../core/api.bend as Api
import ../core/wire.bend as W
import ./persist.bend as P
import ./remote.bend as Rem
import ../core/config.bend as Cf
import ../vendor/http/http.bend as Http
import ../vendor/http/dns/dns-pure.bend as Dnp
import ../vendor/http/encoding/encoding.bend as Enc
import ./settings.bend as St
import ./provider-pure.bend as Pvp
import ./plan-pure.bend as Plan
import ./plan.bend as Pl
import ../core/image.bend as Im
import ./debug.bend as Dbg
import ./debug-pure.bend as Dp
import ./usage-pure.bend as Us
import ./oneshot-pure.bend as Osp
import ../core/ev.bend as Ev
import ./race.bend as Race
import ./race-pure.bend as Rcp
import ./sse-read.bend as Sr
import ./live-pure.bend as Lp
import ./reqlog.bend as Rl
# ---- the live provider call, fully in Bend (hub HTTP client) ----
# The wire request (MODEL/TOOL/MSG/CALL lines) becomes the provider
# JSON in core/api.bend, is POSTed by the hub HTTP client, and the reply
# maps back through core/api.bend into the OK/CALL/END string remote.bend
# parses. No bridge process.

# the transport failure, as the pure policy sees it
def net_of(e: Http.Err) -> Pvp.NetErr:
  match e:
    case Http.ErrUrl{}:
      Pvp.NUrl{}
    case Http.ErrDns{+m}:
      Pvp.NDns{Dnp.why(m), Dnp.check(m)}
    case Http.ErrConnect{+c, +w}:
      Pvp.NConnect{c, w}
    case Http.ErrTls{+c, +w}:
      Pvp.NTls{c, w}
    case Http.ErrRead{+c, +w}:
      Pvp.NRead{c, w}
    case Http.ErrWrite{+c, +w}:
      Pvp.NWrite{c, w}
    case Http.ErrTimeout{}:
      Pvp.NTimeout{}
    case Http.ErrRedirect{}:
      Pvp.NRedirect{}
    case Http.ErrBad{}:
      Pvp.NBad{}

# the family's headers (core/api.bend), set in their order
def headers_of(hs: List<&2, Api.Hdr>, m: Map<&2, List<&2, String>>)
    -> Map<&2, List<&2, String>>:
  match hs:
    case Nil{}:
      m
    case Con{Api.Hdr{+name, +val}, +rest}:
      headers_of(rest, Http.set(m, name, val))

def model_headers(st: Api.Style, +betas: String, +key: String)
    -> Map<&2, List<&2, String>>:
  headers_of(Api.headers(st, betas, key), Http.empty())

# the ring also keeps the raw reply of a family that answers whole
# (<slot>.json.reply, runtime/reqlog.bend): the tool-call arguments
# exactly as the provider serialized them, before any harness parsing -
# the ground truth for wire-encoding bugs. rpath "": no ring.
def model_call.res.dumper(empty: Bool, +rpath: String, +res: Http.Res)
    -> IO(Unit):
  match empty:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      P.write_file(rpath, Http.text(res))

# a response, judged by the pure policy: its status, its Retry-After,
# and the reply the style mapper made of its body
def verdict_res(+res: Http.Res, st: Api.Style) -> Pvp.Attempt:
  match res:
    case Http.Res{+status, +h, _}:
      Pvp.attempt_status(status, Http.header(h, "retry-after"),
        Api.reply_of_for(st, status, Http.text(res)))

# one attempt as the loop and the debug log see it: the policy's
# verdict, the HTTP status (0: no response) and the raw body received
type Got is Data:
  Got{a: Pvp.Attempt, status: U32, raw: String}

# a transport failure names the host the call went to (Pvp.net_cause)
def got_err(+host: String, +e: Http.Err) -> Got:
  Got{Pvp.attempt_err(host, net_of(e)), 0, ""}

def got_res(+res: Http.Res, st: Api.Style) -> Got:
  match res:
    case Http.Res{+status, _, _}:
      Got{verdict_res(res, st), status, Http.text(res)}

def model_call.verdict(r: Result<&2, &2, Http.Err, Http.Res>, st: Api.Style,
    +host: String, +rpath: String) -> IO(Got):
  match r:
    case Done{+res}:
      do IO<Got>:
        d : Unit <- model_call.res.dumper(String.is_empty(rpath), rpath, res)
        IO.pure(Got, got_res(res, st))
    case Fail{+e}:
      IO.pure(Got, got_err(host, e))

# ---- the loop's output: stdout (scripted) or the client socket ----
# Defined here, below main, so the provider call carries it through (its
# retry notes reach the client while the call waits). Sockets are affine
# handles, so Out is a Type: it is threaded, never copied.

type Out is Type:
  OStd{}
  ONet{sock: Socket}

def out.pair(sm: Socket & Result<&1, &1, (U32 & String) & String, Unit>) -> Socket:
  match sm:
    case (s2, _):
      s2

# the wire log (BEND_WIRE_LOG, set by the Switchboard hub): every batch
# sent to the client is appended there FIRST. The hub reads the REPL's
# output from this file, not from the socket, so a hub that restarts
# (a version switch, a crash) resumes at its saved offset and loses no
# line, even one emitted while no hub was connected. BR-007: the notes
# of a call in flight (provider_retry) go there too, or the hub never
# sees them.
def wire_log.when(skip: Bool, +path: String, +text: String) -> IO(Unit):
  match skip:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        f : Result<&1, &1, U32 & String, File> <- File.open(path, "a")
        P.write.opened(f, text)

def wire_log(+text: String) -> IO(Unit):
  do IO<Unit>:
    fr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_LOG")
    +path : String <- IO.pure(String, P.env_or(fr, ""))
    wire_log.when(String.is_empty(path), path, text)

def note(n: Out, +line: String) -> IO(Out):
  match n:
    case OStd{}:
      do IO<Out>:
        IO.print(line)
        IO.pure(Out, OStd{})
    case ONet{sock}:
      do IO<Out>:
        wire_log(String.append(line, "\n"))
        sm : Socket & Result<&1, &1, (U32 & String) & String, Unit> <- TCP.send(sock,
          String.append(line, "\n"))
        s2 : Socket <- IO.pure(Socket, out.pair(sm))
        IO.pure(Out, ONet{s2})

# the token usage of a reply, as a feed line (runtime/usage-pure.bend);
# a reply without usage, or a failed call, prints nothing
def usage_note.pick(empty: Bool, n: Out, +line: String) -> IO(Out):
  match empty:
    case True{}:
      IO.pure(Out, n)
    case False{}:
      note(n, line)

# the attempt's body: a whole JSON reply, or the Anthropic stream's
# events (Us.usage_line reads both); no response (status 0) prints nothing
def usage_note(g: Got, n: Out, +st: Api.Style, +model: String, +req: U32)
    -> IO(Out):
  match g:
    case Got{_, +status, +raw}:
      +line = Us.usage_line(st, model, status, raw)
      +ev = Us.usage_ev(st, model, status, raw, req)
      do IO<Out>:
        n2 : Out <- usage_note.pick(String.is_empty(line), n, line)
        usage_note.pick(String.is_empty(ev), n2, ev)

def interrupt_path() -> IO(String):
  do IO<String>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    +d : String <- P.side_dir()
    IO.pure(String, Pvp.interrupt_path_of(d, p))

# BR-005: a Ctrl+C landing during a failed attempt must not wait for
# the remaining retries: the pause before a retry is slept in 1 s
# slices and the flag is read between them. BR-007: only a flag WITH
# content is an interrupt (Pvp.interrupt_flag) - the cleared flag
# is an empty file - and the stop keeps the error that made the call
# retry.
def model_call.wait(fuel: Nat, +ipath: String, +left: U32) -> IO(Bool):
  match fuel:
    case 0n:
      IO.pure(Bool, False{})
    case 1n+k:
      do IO<Bool>:
        m : Maybe<&2, String> <- P.read_file(ipath)
        +stop : Bool <- IO.pure(Bool, Pvp.interrupt_flag(m))
        IO.sleep(Pvp.slice_ms(stop, left))
        more : Bool <- model_call.wait(k, ipath, Pvp.left_after(stop, left))
        IO.pure(Bool, Bool.or(stop, more))

def model_call.step.retry.go(stop: Bool, n: Out, +ipath: String, +k: Nat,
    +tries: Nat, +why: String,
    next: Nat -> Out -> IO(Out & Rem.RemoteRes)) -> IO(Out & Rem.RemoteRes):
  match stop:
    case True{}:
      do IO<Out & Rem.RemoteRes>:
        # who asked (the flag's content), then consume the flag so the
        # boundary drains stay clean
        f : Maybe<&2, String> <- P.read_file(ipath)
        +who : String <- IO.pure(String, Pvp.interrupter_in(f))
        u : Unit <- P.write_file(ipath, "")
        IO.pure(Out & Rem.RemoteRes,
          (n, Rem.RRErr{Pvp.interrupted_text(who, Nat.add(tries, 1n), why)}))
    case False{}:
      next(k)(n)

def model_call.step.retry(can: Bool, n: Out, +k: Nat, +tries: Nat,
    +delay_ms: U32, +why: String, +req: U32,
    next: Nat -> Out -> IO(Out & Rem.RemoteRes)) -> IO(Out & Rem.RemoteRes):
  match can:
    case True{}:
      do IO<Out & Rem.RemoteRes>:
        n1 : Out <- note(n, Pvp.retry_line(tries, why, delay_ms))
        n2 : Out <- note(n1, Ev.retry(req, Nat.add(tries, 1n), why, delay_ms))
        +ipath : String <- interrupt_path()
        stop : Bool <- model_call.wait(Pvp.wait_slices(delay_ms), ipath,
          delay_ms)
        model_call.step.retry.go(stop, n2, ipath, k, tries, why, next)
    case False{}:
      IO.pure(Out & Rem.RemoteRes,
        (n, Rem.RRErr{Pvp.exhausted_for(Nat.add(tries, 1n), why)}))

def model_call.step(a: Pvp.Attempt, n: Out, +k: Nat, +tries: Nat, +req: U32,
    next: Nat -> Out -> IO(Out & Rem.RemoteRes)) -> IO(Out & Rem.RemoteRes):
  match a:
    case Pvp.AOk{+reply}:
      IO.pure(Out & Rem.RemoteRes, (n, Rem.parse_reply(reply)))
    case Pvp.AFail{+why}:
      IO.pure(Out & Rem.RemoteRes, (n, Rem.RRErr{why}))
    case Pvp.ARetry{+after_ms, +why}:
      model_call.step.retry(Nat.is_lt(0n, k), n, k, tries,
        Pvp.retry_delay_ms(after_ms, tries), why, req, next)

# the body text, decoded once from the raw pieces (newest first)
def sse.text(+parts: List<&2, String>) -> String:
  Enc.utf8.decode(String.join(List.reverse(&2, String, parts), ""))

def sse.dump(empty: Bool, +rpath: String, +text: String) -> IO(Unit):
  match empty:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      P.write_file(rpath, text)

# a 200 streams events; any other status carries a plain error body
def sse.verdict.pick(ok: Bool, +status: U32, +retry_after: String,
    +text: String, st: Api.Style) -> Pvp.Attempt:
  match ok:
    case True{}:
      Pvp.attempt_status(200, retry_after, Api.stream_reply(st, text))
    case False{}:
      Pvp.attempt_status(status, retry_after,
        Api.reply_of_for(st, status, text))

def sse.body(+status: U32, +retry_after: String, +text: String,
    st: Api.Style, +rpath: String) -> IO(Got):
  do IO<Got>:
    d : Unit <- sse.dump(String.is_empty(rpath), rpath, text)
    IO.pure(Got,
      Got{sse.verdict.pick(U32.is_eq(status, 200), status, retry_after, text,
        st), status, text})

# a stream cut short keeps what it had received (the debug log shows
# how far the reply got)
def sse.done(rd: Sr.Rd, +status: U32, +retry_after: String, +ck: Sr.Ck,
    +host: String, +w: Pvp.ProvWait, fam: Api.Style, +rpath: String) -> IO(Got):
  match rd:
    case Sr.RdEnd{st, +parts}:
      do IO<Got>:
        c : Unit <- Http.stream.close(st)
        sse.body(status, retry_after, sse.text(parts), fam, rpath)
    case Sr.RdErr{st, +parts, +e}:
      do IO<Got>:
        c : Unit <- Http.stream.close(st)
        IO.pure(Got, Got{Pvp.attempt_err_in(host, w, net_of(e)), status,
          sse.text(parts)})
    case Sr.RdStop{st, +parts}:
      # cancelled: the race already answered the turn (call.raced) and
      # nobody reads this Got; the connection closes, the provider stops,
      # and the cancel file (its own token) is emptied
      match ck:
        case Sr.Ck{+cpath, _, _}:
          do IO<Got>:
            c : Unit <- Http.stream.close(st)
            u : Unit <- P.write_file(cpath, "")
            IO.pure(Got, Got{Pvp.AFail{"cancelled"}, status, sse.text(parts)})

@unsafe
def sse.head(sr: Http.Stream & Http.Res, +c: Chan(Race.Step<Got>), +lv: Lp.Lv,
    +ck: Sr.Ck, +host: String, +w: Pvp.ProvWait, fam: Api.Style,
    +rpath: String) -> IO(Got):
  match sr:
    case (st, res):
      match res:
        case Http.Res{+status, +h, _}:
          do IO<Got>:
            rd : Sr.Rd <- Sr.sse.check(Got, c, lv, st, Nil{}, ck, 0)
            sse.done(rd, status, Http.header(h, "retry-after"), ck, host,
              w, fam, rpath)

@unsafe
def sse.opened(r: Result<&1, &1, Http.Err, Http.Stream>,
    +c: Chan(Race.Step<Got>), +lv: Lp.Lv, +ck: Sr.Ck, +host: String,
    +w: Pvp.ProvWait, fam: Api.Style, +rpath: String) -> IO(Got):
  match r:
    case Fail{+e}:
      IO.pure(Got, Got{Pvp.attempt_err_in(host, w, net_of(e)), 0, ""})
    case Done{st}:
      sse.head(Http.stream.res(st), c, lv, ck, host, w, fam, rpath)

# the wait (Pvp.stream_wait): the provider's idle_timeout_sec, else 90 s;
# tok: the attempt's token in the cancel file at cpath; c, lv: the race
# channel its partials go to and its live reader (runtime/live-pure.bend)
@unsafe
def sse.call(+c: Chan(Race.Step<Got>), +lv: Lp.Lv, +tok: String,
    +cpath: String, +url: String, hdrs: Map<&2, List<&2, String>>,
    +body: String, +w: Pvp.ProvWait, fam: Api.Style, +rpath: String) -> IO(Got):
  do IO<Got>:
    r : Result<&1, &1, Http.Err, Http.Stream> <- Http.open.with("POST", url,
      hdrs, body, Pvp.wait_ms(w))
    sse.opened(r, c, lv, Sr.Ck{cpath, tok, Pvp.wait_ms(w)}, Pvp.url_host(url),
      w, fam, rpath)

# the whole reply at once (a family that does not stream)
def whole.call(+url: String, hdrs: Map<&2, List<&2, String>>, +body: String,
    st: Api.Style, +rpath: String) -> IO(Got):
  do IO<Got>:
    r : Result<&2, &2, Http.Err, Http.Res> <- Http.fetch.with("POST", url,
      hdrs, body, Pvp.step_timeout_ms())
    model_call.verdict(r, st, Pvp.url_host(url), rpath)

@unsafe
def call.by_style.pick(streams: Bool, +c: Chan(Race.Step<Got>), +lv: Lp.Lv,
    +tok: String, +cpath: String, st: Api.Style, +url: String,
    hdrs: Map<&2, List<&2, String>>, +body: String, +w: Pvp.ProvWait,
    +rpath: String) -> IO(Got):
  match streams:
    case True{}:
      sse.call(c, lv, tok, cpath, url, hdrs, body, w, st, rpath)
    case False{}:
      whole.call(url, hdrs, body, st, rpath)

# the family says whether it streams (core/api.bend); rpath: the ring's
# .reply for a whole answer ("" none)
@unsafe
def call.by_style(+c: Chan(Race.Step<Got>), +lv: Lp.Lv, +tok: String,
    +cpath: String, +st: Api.Style, +url: String,
    hdrs: Map<&2, List<&2, String>>, +body: String, +w: Pvp.ProvWait,
    +rpath: String) -> IO(Got):
  call.by_style.pick(Api.streams(st), c, lv, tok, cpath, st, url, hdrs, body,
    w, rpath)

# the cancel file of this REPL (race-pure.bend), next to its interrupt flag
def cancel_path() -> IO(String):
  do IO<String>:
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +p : String <- IO.pure(String, P.env_or(pr, "0"))
    +d : String <- P.side_dir()
    IO.pure(String, Rcp.cancel_path_of(d, p))

# a stop wins the race: the attempt is left behind with its token in the
# cancel file (it closes its connection at its next piece or quiet
# slice), and the call ends as an interrupted attempt (AFail: no retry);
# the boundary drain after model_call then applies the interrupt
def call.raced.got(n: Out, r: Race.Raced<Got>, +tok: String, +cpath: String)
    -> IO(Out & Got):
  match r:
    case Race.Won{g}:
      IO.pure(Out & Got, (n, g))
    case Race.Stopped{+who}:
      do IO<Out & Got>:
        u : Unit <- P.write_file(cpath, tok)
        IO.pure(Out & Got, (n, Got{Pvp.AFail{Pvp.interrupted_by(who)}, 0, ""}))

# a partial line: the hub reads the REPL's lines from the wire log only
# (switchboard daemon/repl.rs pump_wire; the socket is only drained), so
# a partial goes there and nowhere else: no TCP send, and the log's path
# read once per call (wpath), not per partial (R40's CPU bar)
def note.partial(n: Out, +wpath: String, +line: String) -> IO(Out):
  match n:
    case OStd{}:
      do IO<Out>:
        IO.print(line)
        IO.pure(Out, OStd{})
    case ONet{sock}:
      do IO<Out>:
        wire_log.when(String.is_empty(wpath), wpath, String.append(line, "
"))
        IO.pure(Out, ONet{sock})

# the race read until its end; each partial the attempt sends is written
# here, by the one owner of the wire (Out)
@unsafe
def call.raced.read(x: Race.Next<Got>, n: Out, +c: Chan(Race.Step<Got>),
    +ipath: String, +tok: String, +cpath: String, +wpath: String)
    -> IO(Out & Got):
  match x:
    case Race.NPart{+line}:
      do IO<Out & Got>:
        n2 : Out <- note.partial(n, wpath, line)
        y : Race.Next<Got> <- Race.next(Got, c, ipath)
        call.raced.read(y, n2, c, ipath, tok, cpath, wpath)
    case Race.NDone{r}:
      call.raced.got(n, r, tok, cpath)

# every model attempt goes through here (the one call.by_style caller):
# raced against the interrupt flag, so a stop ends it within a tick.
# id: the attempt's partial id (<call>.<attempt>); live: its text is the
# agent's answer, so it streams in (Lp.lv0)
@unsafe
def call.raced(n: Out, +tok: String, +id: String, live: Bool,
    +st: Api.Style, +url: String, hdrs: Map<&2, List<&2, String>>,
    +body: String, +w: Pvp.ProvWait, +rpath: String) -> IO(Out & Got):
  do IO<Out & Got>:
    +ipath : String <- interrupt_path()
    +cpath : String <- cancel_path()
    +c : Chan(Race.Step<Got>) <- Race.open(Got, +c2 =>
      call.by_style(c2, Lp.lv0(live, id, st), tok, cpath, st, url, hdrs, body,
        w, rpath))
    fr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_WIRE_LOG")
    +wpath : String <- IO.pure(String, P.env_or(fr, ""))
    x : Race.Next<Got> <- Race.next(Got, c, ipath)
    call.raced.read(x, n, c, ipath, tok, cpath, wpath)

# the attempt, into the session debug log when it is an incident
def model_call.log(+call: Nat, +tries: Nat, +model: String, +url: String,
    +json: String, g: Got) -> IO(Unit):
  match g:
    case Got{+a, +status, +raw}:
      Dbg.attempt(call, tries, model, url, status, json, raw, a)

# the attempt the Core reads (Pvp.attempt_for, BISE-293); on a ChatGPT
# plan provider, its failures read as the plan's lines (Pvp.attempt_plan)
# (a model the plan refuses: Plan.refused_line, model = the id called)
def got_for(+tools: List<&2, W.WTool>, +name: String, +key: String,
    +plan: Bool, +model: String, +st: Api.Style, g: Got) -> Pvp.Attempt:
  match g:
    case Got{+a, +status, +raw}:
      Pvp.attempt_plan.line(Plan.refused_line(plan, status, raw, model),
        Pvp.attempt_plan(plan, status, Api.error_code(st, raw),
          Pvp.attempt_for(tools, name, key, status, raw, a)))

def got_attempt(g: Got) -> Pvp.Attempt:
  match g:
    case Got{+a, _, _}:
      a

# ---- images (docs/images.md): check each image part of the printed body ----
# The adapters print each image as a part placeholder (Im.img_part_ph). Here,
# before every send, each one is checked: a .b64 of the image store, there,
# non-empty, a multiple of 4 bytes, base64 in its first bytes. A good one
# becomes the image object with a FILE PART (Http.file_part: the path and
# the size), not the data: the HTTP client sends the file's bytes from a C
# buffer at write time. The base64 never enters the Bend heap: as a String
# it cost ~47 bytes of heap per char, so 20 screenshots made a request of
# ~1.5 GB, and the heap never shrinks (repl-live at 1.8 GB on 2026-10-01).
# The 20 most recent good ones go (Im.res_cap); any other becomes a text
# part "[image unavailable: <name> (<reason>)]", for every family: an image
# never leaves without its data (2026-10-02: one did, as its placeholder,
# and Anthropic refused every turn of three agents).

def img_cap() -> Nat:
  20n

def img_dir() -> IO(String):
  do IO<String>:
    +d : String <- P.env_path("BEND_IMAGE_DIR", "/.bise/images")
    IO.pure(String, String.append(d, "/"))

def head_of(r: Result<&1, &1, U32 & String, String>) -> String:
  match r:
    case Done{+s}:
      s
    case Fail{_}:
      ""

def img_check.why(+why: String, +path: String, +n: U32) -> Im.IRes:
  Bool.pick(Im.IRes, String.is_empty(why),
    Im.IROk{Http.file_part(path, n), U32.to_nat(n)}, Im.IRBad{why})

def img_check.head(fr: File & Result<&1, &1, U32 & String, String>,
    +path: String, +n: U32) -> IO(Im.IRes):
  match fr:
    case (f, r):
      do IO<Im.IRes>:
        c : Unit <- File.close(f)
        IO.pure(Im.IRes, img_check.why(Im.img_why(n, head_of(r)), path, n))

def img_check.size(fs: File & Result<&1, &1, U32 & String, U32>,
    +path: String) -> IO(Im.IRes):
  match fs:
    case (f, r):
      match r:
        case Fail{_}:
          do IO<Im.IRes>:
            c : Unit <- File.close(f)
            IO.pure(Im.IRes, Im.IRBad{"unreadable file"})
        case Done{+n}:
          do IO<Im.IRes>:
            fr : File & Result<&1, &1, U32 & String, String> <- File.read(f, 64)
            img_check.head(fr, path, n)

def img_check.opened(o: Result<&1, &1, U32 & String, File>, +path: String)
    -> IO(Im.IRes):
  match o:
    case Fail{_}:
      IO.pure(Im.IRes, Im.IRBad{"file missing"})
    case Done{f}:
      do IO<Im.IRes>:
        fs : File & Result<&1, &1, U32 & String, U32> <- File.size(f)
        img_check.size(fs, path)

def img_check.pick(ok: Bool, +path: String) -> IO(Im.IRes):
  match ok:
    case False{}:
      IO.pure(Im.IRes, Im.IRBad{"not in the image store"})
    case True{}:
      do IO<Im.IRes>:
        o : Result<&1, &1, U32 & String, File> <- File.open(path, "r")
        img_check.opened(o, path)

# one image's .b64: its file part, or why it cannot go
def img_check(+path: String, +dir: String) -> IO(Im.IRes):
  img_check.pick(Im.img_store_ok(path, dir), path)

@unsafe
def img_checks(phs: List<&2, Im.IPh>, +dir: String,
    +acc: List<&2, Im.IRes>) -> IO(List<&2, Im.IRes>):
  match phs:
    case Nil{}:
      IO.pure(List<&2, Im.IRes>, List.reverse(&2, Im.IRes, acc))
    case Con{+ph, rest}:
      do IO<List<&2, Im.IRes>>:
        r : Im.IRes <- img_check(Im.ph_b64(ph), dir)
        img_checks(rest, dir, Con{r, acc})

@unsafe
def img_body.fill(+segs: List<&2, Im.IBSeg>, +limit: Nat) -> IO(Im.IFill):
  do IO<Im.IFill>:
    +dir : String <- img_dir()
    rs : List<&2, Im.IRes> <- img_checks(Im.body_phs(segs), dir, Nil{})
    IO.pure(Im.IFill, Im.fill_fit(segs, rs, img_cap(), limit))

# a body without image part costs one scan
@unsafe
def img_body.pick(has: Bool, +json: String, +limit: Nat) -> IO(Im.IFill):
  match has:
    case True{}:
      img_body.fill(Im.body_segs(json), limit)
    case False{}:
      IO.pure(Im.IFill, Im.IFill{json, String.length(json), 0n})

# the printed body, each image part checked and filled (or made text),
# the images within the byte limit (Im.res_fit); with its weight
@unsafe
def img_body(+json: String, +limit: Nat) -> IO(Im.IFill):
  img_body.pick(String.contains(json, Im.part_open()), json, limit)

# the byte limit of a request (big-request, 2026-10-04): what bise lets
# the images make a body weigh. $BISE_REQUEST_MAX_BYTES, else 24 MiB:
# under Anthropic's 32 MB (and the foundry proxy's), OpenAI's and
# Mistral's, with room for the text. A provider that still refuses the
# size gets one retry with half the weight (model_call.sized).
def req_limit.default() -> Nat:
  25165824n

def req_limit.of(m: Maybe<&2, U32>) -> Nat:
  match m:
    case None{}:
      req_limit.default()
    case Some{+v}:
      Bool.pick(Nat, U32.is_eq(v, 0), req_limit.default(), U32.to_nat(v))

def req_limit() -> IO(Nat):
  do IO<Nat>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_REQUEST_MAX_BYTES")
    IO.pure(Nat, req_limit.of(U32.read(String.trim(P.env_or(r, "")))))

def fill_body(+f: Im.IFill) -> String:
  match f:
    case Im.IFill{+b, _, _}:
      b

def fill_bytes(+f: Im.IFill) -> Nat:
  match f:
    case Im.IFill{_, +n, _}:
      n

def fill_img_bytes(+f: Im.IFill) -> Nat:
  match f:
    case Im.IFill{_, _, +n}:
      n

def got_status(g: Got) -> U32:
  match g:
    case Got{_, +status, _}:
      status

def got_raw(g: Got) -> String:
  match g:
    case Got{_, _, +raw}:
      raw

# a request whose tool calls were not all answered (a history cut by a
# restart): the guard answered them; one line in the debug log and one
# on stderr, so the repair is never silent
def unpaired_log(bad: Bool, +call: Nat, +req: U32) -> IO(Unit):
  match bad:
    case False{}:
      IO.pure(Unit, Unit{})
    case True{}:
      do IO<Unit>:
        IO.print_err(String.append("bise: request ", String.append(U32.show(req),
          ": a tool call had no result; it got a failed one (no result: bise restarted while this ran)")))
        +d : String <- Dbg.dir()
        +ts : Nat <- Dbg.now(d)
        Dbg.append(d, Dp.ev_line(ts, "unpaired_calls_answered",
          Con{Dp.FNat{"call", call}, Con{Dp.FNat{"req", U32.to_nat(req)}, Nil{}}}) ++ "\n")

# the attempt's result, once its partials are written
def got_then(ng: Out & Got, f: Out -> Got -> IO(Out & Rem.RemoteRes))
    -> IO(Out & Rem.RemoteRes):
  match ng:
    case (n, g):
      f(n)(g)

# THE body of a request: what model_call.sent sends and what the render
# mode prints (BISE_RENDER, runtime/render.bend: /debug completion), one
# function, so the body /debug shows is the body sent. Beside it, what
# the call needs: the parsed request, the models file, the cache names
# and key (BISE-268), the JSON before its images, the images' fill.
type Built is Data:
  Built{wreq: W.WReq, mfile: String, names: Pvp.CNames, ckey: String,
    json: String, fill: Im.IFill, body: String}

@unsafe
def request_body(+request: String, +pv: Pvp.Prov, +limit: Nat) -> IO(Built):
  match pv:
    case Pvp.Prov{+prov, +model, +st, _, _, +facts}:
      do IO<Built>:
        # a one-shot call (BISE_ONESHOT, BISE-126) goes out light:
        # no thinking / effort keys (runtime/oneshot-pure.bend)
        osr : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_ONESHOT")
        +wreq : W.WReq <- IO.pure(W.WReq, W.parse_wire(request))
        +mfile : String <- models_file()
        +names : Pvp.CNames <- IO.pure(Pvp.CNames,
          Pvp.cache_names(mfile, prov, String.concat([prov, "/", model])))
        +ckey : String <- IO.pure(String, Api.session_key(wreq))
        +json : String <- IO.pure(String,
          Osp.light_if(Bool.not(String.is_empty(P.env_or(osr, ""))),
            Api.wire_body(st, Api.with_cache_key(Pvp.cache_field(names), ckey,
              Api.api_body_for(st, model, facts, W.pair_wire(wreq))))))
        +fill : Im.IFill <- img_body(json, limit)
        IO.pure(Built, Built{wreq, mfile, names, ckey, json, fill,
          Enc.utf8.encode(fill_body(fill))})

# one call = up to max_attempts attempts; `call` (its start time, ms)
# names the call in the debug log
@unsafe
def model_call.try(fuel: Nat, +tries: Nat, n: Out, +request: String,
    +key: String, +pv: Pvp.Prov, +call: Nat, +req: U32, +limit: Nat,
    +shrunk: Bool, +live: Bool) -> IO(Out & Rem.RemoteRes):
  match fuel:
    case 0n:
      IO.pure(Out & Rem.RemoteRes,
        (n, Rem.RRErr{"provider unreachable: retries exhausted"}))
    case 1n+k:
      do IO<Out & Rem.RemoteRes>:
        +b : Built <- request_body(request, pv, limit)
        model_call.sent(b, k, tries, n, request, key, pv, call, req, limit,
          shrunk, live)

# one attempt with its body built: logged in the ring (/debug
# completion, runtime/reqlog.bend) before it goes out, then sent
@unsafe
def model_call.sent(b: Built, +k: Nat, +tries: Nat, n: Out,
    +request: String, +key: String, +pv: Pvp.Prov, +call: Nat, +req: U32,
    +limit: Nat, +shrunk: Bool, +live: Bool) -> IO(Out & Rem.RemoteRes):
  match b:
    case Built{+wreq, +mfile, +names, +ckey, +json, +fill, +body}:
      match pv:
        case Pvp.Prov{+prov, +model, +st, +url, _, +facts}:
          do IO<Out & Rem.RemoteRes>:
            # the last guard (BISE-242): every call answered, logged
            # when it had to answer one
            og : Unit <- unpaired_log(W.wire_unpaired(wreq), call, req)
            rl : Unit <- Rl.write(req, live, String.concat([prov, "/", model]),
              tries, call, fill_body(fill), request)
            # a gateway (headers_env, key_command), read on each call:
            # its extra headers, its key also as a bearer token
            +hvar : String <- IO.pure(String,
              Pvp.headers_env(mfile, prov, String.concat([prov, "/", model])))
            hr : Result<&1, &1, U32 & String, String> <- IO.get_env(hvar)
            +bearer : Bool <- IO.pure(Bool, Bool.not(String.is_empty(
              Pvp.key_command(mfile, prov, String.concat([prov, "/", model])))))
            hdrs : Map<&2, List<&2, String>> <- IO.pure(Map<&2, List<&2, String>>,
              headers_of(Api.with_extra_headers(String.append(
                Pvp.headers(mfile, prov, String.concat([prov, "/", model])), P.env_or(hr, "")),
                Api.with_bearer(bearer, key,
                  Api.with_cache_header(Pvp.cache_header(names), ckey,
                    Api.headers(st, Api.facts_betas(facts), key)))), Http.empty()))
            +rpath : String <- Rl.reply_path(req)
            # a slow gateway or local model: its idle_timeout_sec
            ng : Out & Got <- call.raced(n,
              String.concat([Nat.show(call), "-", Nat.show(tries)]),
              String.concat([Nat.show(call), ".", Nat.show(tries)]), live,
              st, url, hdrs, body,
              Pvp.stream_wait(mfile, prov, String.concat([prov, "/", model])), rpath)
            got_then(ng, n0 => +g => do IO<Out & Rem.RemoteRes>:
              lg : Unit <- model_call.log(call, tries, model, url, json, g)
              # the usage line names the full id (the TUI's catalog key)
              nu : Out <- usage_note(g, n0, st, String.concat([prov, "/", model]), req)
              # big-request: a body over the provider's size limit goes
              # again once, its images within half its weight
              # (model_call.sized); else BISE-293: the calls named back
              # through the request's tools, a refusal in bise's words and
              # the provider's
              model_call.sized(Bool.and(Bool.not(shrunk),
                  Bool.and(W.size_refused(got_status(g), got_raw(g)),
                    Nat.is_lt(0n, fill_img_bytes(fill)))),
                nu, k, tries, req,
                Pvp.size_retry_why(Pvp.prov_label(mfile, prov, url), got_status(g),
                  Nat.div(fill_bytes(fill), 2n)),
                k4 => n3 => model_call.step(got_for(W.req_tools(wreq),
                    Pvp.prov_label(mfile, prov, url), key, Api.facts_plan(facts), model, st, g),
                  n3, k4, tries, req,
                  k2 => n2 => model_call.try(k2, Nat.add(tries, 1n), n2, request,
                    key, pv, call, req, limit, shrunk, live)),
                k3 => n4 => model_call.try(k3, Nat.add(tries, 1n), n4, request,
                  key, pv, call, req, Nat.div(fill_bytes(fill), 2n), True{}, live)))

# a request the provider refused for its size, that had images: one line
# (a retry the TUI shows), then the same request at once with half the
# weight (the oldest images go). Once per call: a second refusal is the
# turn's failure, and the Core compacts (core/session.bend).
def model_call.sized(big: Bool, nu: Out, +k: Nat, +tries: Nat, +req: U32,
    +why: String, step: Nat -> Out -> IO(Out & Rem.RemoteRes),
    again: Nat -> Out -> IO(Out & Rem.RemoteRes)) -> IO(Out & Rem.RemoteRes):
  match big:
    case False{}:
      step(k)(nu)
    case True{}:
      do IO<Out & Rem.RemoteRes>:
        n1 : Out <- note(nu, Pvp.retry_line(tries, why, 0))
        n2 : Out <- note(n1, Ev.retry(req, Nat.add(tries, 1n), why, 0))
        again(k)(n2)

@unsafe
def model_call.go(n: Out, +request: String, +key: String, +pv: Pvp.Prov,
    +req: U32, +live: Bool) -> IO(Out & Rem.RemoteRes):
  do IO<Out & Rem.RemoteRes>:
    +call : Nat <- IO.now()
    +limit : Nat <- req_limit()
    model_call.try(Pvp.max_attempts(), 0n, n, request, key, pv, call, req,
      limit, False{}, live)

# a provider with a key_env (or a key_command) needs its key; one with
# key_env = "" and no key_command (a local server) calls without a key.
# No key: one line that says what to do (BISE-294: the TUI names the
# provider and points at /provider; main's thread reads it as is)
def model_call.key(missing: Bool, n: Out, +request: String, +key: String,
    +pv: Pvp.Prov, +why: String, +req: U32, +live: Bool)
    -> IO(Out & Rem.RemoteRes):
  match missing:
    case True{}:
      IO.pure(Out & Rem.RemoteRes, (n, Rem.RRErr{why}))
    case False{}:
      model_call.go(n, request, key, pv, req, live)

def key_cmd.max_output() -> U32:
  65536

def key_cmd.timeout_ms() -> U32:
  60000

# key_command's answer: its stdout, trimmed, when it exits 0; else ""
def key_cmd.out(ok: Bool, +out: String) -> String:
  match ok:
    case True{}:
      String.trim(out)
    case False{}:
      ""

def key_cmd.streams(+code: U32, oe: String & String) -> String:
  match oe:
    case (out, _):
      key_cmd.out(U32.is_zero(code), out)

def key_cmd.done(res: U32 & (String & String)) -> String:
  match res:
    case (code, oe):
      key_cmd.streams(code, oe)

def key_cmd.answer(r: Result<&1, &1, U32 & String, U32 & (String & String)>)
    -> String:
  match r:
    case Done{res}:
      key_cmd.done(res)
    case Fail{_}:
      ""

# the key of a call: key_command's output when the catalog names one,
# run by /bin/sh before every call like Claude Code's apiKeyHelper (a
# gateway's short-lived token); else the key_env variable's value
@unsafe
def key_of.pick(none: Bool, +cmd: String, +env_key: String) -> IO(String):
  match none:
    case True{}:
      IO.pure(String, env_key)
    case False{}:
      do IO<String>:
        r : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run(
          "/bin/sh", Con{"-c", Con{cmd, Nil{}}}, "", key_cmd.max_output(),
          key_cmd.timeout_ms())
        IO.pure(String, key_cmd.answer(r))

# the missing key's line: Pvp.key_why (a ChatGPT plan's: its sign-in
# expired; the key_command's, else the key_env's)
@unsafe
def model_call.auth(n: Out, +request: String, +pv: Pvp.Prov, +req: U32,
    +live: Bool) -> IO(Out & Rem.RemoteRes):
  match pv:
    case Pvp.Prov{+prov, +model, _, _, +key_env, +facts}:
      do IO<Out & Rem.RemoteRes>:
        +mfile : String <- models_file()
        +cmd : String <- IO.pure(String,
          Pvp.key_command(mfile, prov, String.concat([prov, "/", model])))
        kr : Result<&1, &1, U32 & String, String> <- IO.get_env(key_env)
        +key : String <- key_of.pick(String.is_empty(cmd), cmd, P.env_or(kr, ""))
        model_call.key(Bool.and(Bool.not(Bool.and(String.is_empty(key_env),
            String.is_empty(cmd))), String.is_empty(key)), n, request, key, pv,
          Pvp.key_why(Bool.not(String.is_empty(cmd)), Api.facts_plan(facts), prov,
            key_env), req, live)

@unsafe
def model_call.headers_ready(missing: Bool, +hvar: String, n: Out,
    +request: String, +pv: Pvp.Prov, +req: U32, +live: Bool)
    -> IO(Out & Rem.RemoteRes):
  match missing:
    case True{}:
      IO.pure(Out & Rem.RemoteRes, (n, Rem.RRErr{Pvp.headers_why(hvar)}))
    case False{}:
      model_call.auth(n, request, pv, req, live)

@unsafe
def model_call.env(n: Out, +request: String, +pv: Pvp.Prov, +req: U32,
    +live: Bool) -> IO(Out & Rem.RemoteRes):
  match pv:
    case Pvp.Prov{+prov, +model, _, _, +key_env, +facts}:
      do IO<Out & Rem.RemoteRes>:
        +mfile : String <- models_file()
        +hvar : String <- IO.pure(String,
          Pvp.headers_env(mfile, prov, String.concat([prov, "/", model])))
        hr : Result<&1, &1, U32 & String, String> <- IO.get_env(hvar)
        model_call.headers_ready(Pvp.headers_missing(hvar, P.env_or(hr, "")),
          hvar, n, request, pv, req, live)

def model_call.resolved(r: Pvp.Resolved, n: Out, +request: String,
    +req: U32, +live: Bool) -> IO(Out & Rem.RemoteRes):
  match r:
    case Pvp.ROk{+pv}:
      model_call.env(n, request, pv, req, live)
    case Pvp.RErr{_, +msg}:
      IO.pure(Out & Rem.RemoteRes, (n, Rem.RRErr{msg}))

def models_file.read(none: Bool, +path: String) -> IO(Maybe<&2, String>):
  match none:
    case True{}:
      IO.pure(Maybe<&2, String>, None{})
    case False{}:
      P.read_file(path)

# the models file (BISE_MODELS_FILE, written by Rust before the spawn);
# unset or unreadable: "" (the built-in entries alone)
def models_file() -> IO(String):
  do IO<String>:
    fr : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_MODELS_FILE")
    +path : String <- IO.pure(String, P.env_or(fr, ""))
    m : Maybe<&2, String> <- models_file.read(String.is_empty(path), path)
    IO.pure(String, Maybe.default(&2, String, m, ""))

# the session's choice (BISE-135: /model, /reasoning): the file the hub
# writes (BISE_SESSION_CHOICE, rust/catalog Choice), re-read before
# each call; unset or unreadable: "" (nothing picked)
def choice_file() -> IO(String):
  do IO<String>:
    cr : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_SESSION_CHOICE")
    +path : String <- IO.pure(String, P.env_or(cr, ""))
    m : Maybe<&2, String> <- models_file.read(String.is_empty(path), path)
    IO.pure(String, Maybe.default(&2, String, m, ""))

# THE model resolution (runtime/provider-pure.bend, Pvp.resolve) - the
# one function every surface uses: the provider call below AND the
# harness-info line the clients (TUI, bend_client) display. Re-read on
# every call: a config.toml edit (or /model) takes effect at the next
# call.
def resolution() -> IO(Pvp.Resolved):
  do IO<Pvp.Resolved>:
    br : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_MODEL")
    mr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_MODEL")
    ar : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_AGENT_MODEL")
    rr : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_ROLE")
    ur : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_PROVIDER_URL")
    +cfgt : String <- St.read_cfg()
    +file : String <- models_file()
    +choice : String <- choice_file()
    IO.pure(Pvp.Resolved,
      Pvp.resolve_chosen(choice, P.env_or(br, ""), P.env_or(mr, ""),
        P.env_or(ar, ""), P.env_or(rr, ""), cfgt, file, P.env_or(ur, "")))

# the model the clients show: its full id, provider/model
def resolved_model() -> IO(String):
  do IO<String>:
    r : Pvp.Resolved <- resolution()
    IO.pure(String, Pvp.shown_model(r))

# the reasoning effort the next call runs with ("": none, BISE-135)
def resolved_effort() -> IO(String):
  do IO<String>:
    r : Pvp.Resolved <- resolution()
    IO.pure(String, Pvp.shown_effort(r))

# the context window of the model this REPL starts with (BISE-150): the
# same inputs as resolution(); the compaction threshold's default
def resolved_window() -> IO(U32):
  do IO<U32>:
    br : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_MODEL")
    mr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_MODEL")
    ar : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_AGENT_MODEL")
    rr : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_ROLE")
    +cfgt : String <- St.read_cfg()
    +file : String <- models_file()
    +choice : String <- choice_file()
    IO.pure(U32, Pvp.window_chosen(choice, P.env_or(br, ""),
      P.env_or(mr, ""), P.env_or(ar, ""), P.env_or(rr, ""), cfgt, file))

@unsafe
# req: the Core's action id of the request (the session log's req,
# core/ev.bend); a call outside a session (one-shot) passes 0. live: the
# reply is the agent's answer, so its text streams in as partial lines
# (runtime/live-pure.bend); a compaction summary or a one-shot: False
# the id the session asked for, before a plan's swap (Pvp.asked_chosen)
def asked_model() -> IO(String):
  do IO<String>:
    br : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_MODEL")
    mr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_MODEL")
    ar : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_AGENT_MODEL")
    rr : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_ROLE")
    +cfgt : String <- St.read_cfg()
    +file : String <- models_file()
    +choice : String <- choice_file()
    IO.pure(String, Pvp.asked_chosen(choice, P.env_or(br, ""), P.env_or(mr, ""),
      P.env_or(ar, ""), P.env_or(rr, ""), cfgt, file))

def model_call.req(n: Out, +request: String, +req: U32, +live: Bool)
    -> IO(Out & Rem.RemoteRes):
  do IO<Out & Rem.RemoteRes>:
    +r : Pvp.Resolved <- resolution()
    +asked : String <- asked_model()
    +said : String <- Pl.swap_said(live, asked, Pvp.shown_model(r))
    n1 : Out <- usage_note.pick(String.is_empty(said), n, said)
    model_call.resolved(r, n1, request, req, live)

def model_call(n: Out, +request: String) -> IO(Out & Rem.RemoteRes):
  model_call.req(n, request, 0, False{})
