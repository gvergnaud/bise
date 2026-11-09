# runtime/mcp.bend — the Mistral MCP connectors, fully in Bend.
#
# At live startup the REPL fetches the connector bootstrap
# (api.mistral.ai/v1/connectors/bootstrap) and writes a local tool index:
# one line per remote tool: "<cid> <cname> <tool> : #<desc>". The tools
# never enter the model context (they are not in the API tools array);
# the agent reaches them through the ordinary mechanisms:
#   - search_tool_functions: one discovery protocol over the static
#     catalog PLUS this index (merged_search below)
#   - direct calls by name: any "<connector>.<tool>" the model calls —
#     top-level or from a run_program program — resolves against the
#     index here (mcp_call below)
# A call speaks MCP Streamable HTTP against
# /v1/connectors-gateway/<connector_id>/mcp: initialize,
# notifications/initialized, tools/call — three requests on ONE pooled
# connection, because the gateway state is per-connection (a server's
# Mcp-Session-Id and protocol version go back on the later two). A try
# that never reached the tool (initialize failed, no connection, the
# session lost) is tried again on a fresh connection, 3 tries in all; a
# tools/call that may have run is never sent twice. A failure for good
# names its kind, the server, the phase, the cause in the server's own
# words, the retries and whether calling again is safe (the policy and
# words: mcp-pure.bend; the retries and failures go to the debug log's
# events.jsonl). A tool that answers an error (isError), or a failure
# whose kind is bad arguments, also gets what was sent, the tool's
# parameters from its index line's input schema and a check of the
# arguments against it (mcp-args.bend).
#
# A plugin tool with a view (its index line's ` | ui: <uri>`, docs/
# mcp-apps.md C1-C3) asks its bridge to keep the call's full result
# (`x-bise-call: 1`); the bridge answers the view's ref in `x-bise-view`,
# which goes, with the tool's id read at the call's START, as one
# `<id> <ref>` line into the views file; the runtime (main.bend) takes
# that file at each tool's end and says each line as `tool_view #<id> :
# <ref>`. The runtime's two files of views, both in its side folder
# (BEND_AGENT_RUN), one each per REPL (Mcp.tool_now_path, Mcp.views_path):
#   bend-tool-<port>.txt   the tool running now (its id, written at its
#                          start): "the current tool" holds only while a
#                          REPL runs one tool at a time; tools run in
#                          parallel one day, the id moves to the call
#   bend-views-<port>.txt  the refs kept since the last tool's end
# Why files, though both sides run in this one process (one writer, the
# call; one reader, main.bend at a tool's end): a stopped call (main.bend's
# race) is abandoned, and its late ref still needs a place to land outside
# the call's return; run_typescript's calls run deep with no call id of
# their own. Their only reset is the boot (bash.bend views_cleanup, next
# to bash_cleanup): a crashed process's refs never reach the next REPL on
# that port.
# A late ref (a stopped call that still finishes) carries its own id and
# goes to its own row (the fold attaches by id, never by adjacency).
#
# Defs are ordered bottom-up: a def only calls defs defined ABOVE it.
# Recursive loops pass their recursive result to a pure "put" helper
# (Bend is lazy, so the unchosen branch is never forced).
import Base
import ../core/types.bend as T
import ../core/discovery.bend as D
import ../core/text.bend as Tx
import ./persist.bend as P
import ./tools.bend as X
import ./tools-pure.bend as Xp
import ../vendor/http/http.bend as Http
import ../vendor/http/encoding/encoding.bend as Enc
# request bodies are built with J; every PARSE goes through Jv, which keeps
# numbers as text: Bend's Nat is 48-bit, and the connector bootstrap
# carries 2^53-1 JSON-schema maximums that would fail to parse otherwise
import ../vendor/json.bend as J
import 0x16458a2db4f36577294543ec2a6d420c/json.bend as Jv
import ./mcp-pure.bend as Mcp
import ./mcp-args.bend as Ma
import ./plugins.bend as Pl
import ./provider.bend as Pv
import ./provider-pure.bend as Pvp
import ./debug.bend as Dbg
import ./debug-pure.bend as Dp

# what an MCP tool call answers (converted to ExecRes by the runtime)
type MR is Data:
  MR{ok: Bool, out: String}

# schema: the tool's input schema as JSON text ("" when its index line
# has none), for the words of a failure (runtime/mcp-args.bend)
# ui: the view's uri of its index line ("" when none)
type McpTool is Data:
  MT{cid: String, tool: String, schema: String, ui: String}

# ---- tolerant JSON accessors (numbers-as-text JSON) ----

def jdone(m: Maybe<&2, Jv.Val>) -> Jv.Val:
  match m:
    case Some{+w}:
      w
    case None{}:
      Jv.Null{}

def jstr.w(w: Jv.Val, +dflt: String) -> String:
  match w:
    case Jv.Str{+s}:
      s
    case _:
      dflt

def jstr.m(m: Maybe<&2, Jv.Val>, +dflt: String) -> String:
  match m:
    case Some{+w}:
      jstr.w(w, dflt)
    case None{}:
      dflt

def jstr(v: Jv.Val, +k: String, +dflt: String) -> String:
  jstr.m(Jv.get(v, k), dflt)

def jbool.w(w: Jv.Val, +dflt: Bool) -> Bool:
  match w:
    case Jv.Flag{on}:
      on
    case _:
      dflt

def jbool.m(m: Maybe<&2, Jv.Val>, +dflt: Bool) -> Bool:
  match m:
    case Some{+w}:
      jbool.w(w, dflt)
    case None{}:
      dflt

def jbool(v: Jv.Val, +k: String, +dflt: Bool) -> Bool:
  jbool.m(Jv.get(v, k), dflt)

def jarr.w(w: Jv.Val) -> List<&2, Jv.Val>:
  match w:
    case Jv.Arr{+xs}:
      xs
    case _:
      Nil{}

def jarr.m(m: Maybe<&2, Jv.Val>) -> List<&2, Jv.Val>:
  match m:
    case Some{+w}:
      jarr.w(w)
    case None{}:
      Nil{}

def jarr(v: Jv.Val, +k: String) -> List<&2, Jv.Val>:
  jarr.m(Jv.get(v, k))

# the index every lookup reads: this session's one tool index, written
# by its plugins bridge (runtime/plugins.bend): the plugins' tools and
# the Mistral connectors', minus the ones he turned off. Missing: no
# tool (no bridge, or it has not written it yet)
def index_content() -> IO(Maybe<&2, String>):
  do IO<Maybe<&2, String>>:
    +pp : String <- Pl.mcp_index_path()
    P.read_file(pp)

# ---- resolve "<cname>.<tool>" against the index lines ----

def find_line.hit.of(sp: String & String & String & String, +v: String)
    -> Bool:
  (a0, cname, tool, a3) = sp
  String.eq(Mcp.pub_name(cname, tool), v)

def find_line.hit(+line: String, +v: String) -> Bool:
  find_line.hit.of(Mcp.line_parts(line), v)

def find_put.mt(+cid: String, +tool: String, du: String & String) -> McpTool:
  (d, u) = du
  MT{cid, tool, Ma.schema_of(d), u}

def find_put.of(sp: String & String & String & String) -> McpTool:
  (cid, a1, tool, desc) = sp
  find_put.mt(cid, tool, Mcp.desc_ui(desc))

def find_put(hit: Bool, +line: String, +rest_res: Maybe<&2, McpTool>)
    -> Maybe<&2, McpTool>:
  match hit:
    case True{}:
      Some{find_put.of(Mcp.line_parts(line))}
    case False{}:
      rest_res

def find_line.go(lines: List<&2, String>, +v: String)
    -> Maybe<&2, McpTool>:
  match lines:
    case Nil{}:
      None{}
    case Con{+line, rest}:
      find_put(find_line.hit(line, v), line, find_line.go(rest, v))

def find_line(+content: String, +v: String) -> Maybe<&2, McpTool>:
  find_line.go(String.split(content, Tx.newline()), v)

def merged_search.pick(cat: List<&2, T.ToolDef>, +args: String,
    m: Maybe<&2, String>) -> MR:
  MR{True{}, Xp.search_reply(args, Mcp.merged_tools.live(cat, m))}

def merged_search.live(+args: String) -> IO(MR):
  do IO<MR>:
    +cat : List<&2, T.ToolDef> <- X.catalog()
    m : Maybe<&2, String> <- index_content()
    IO.pure(MR, merged_search.pick(cat, args, m))

def merged_search(+args: String, +live: Bool) -> IO(MR):
  match live:
    case True{}:
      merged_search.live(args)
    case False{}:
      IO.pure(MR, MR{True{}, Xp.search_reply(args, X.catalog_short())})

# ---- a call by name: three requests on one pooled connection ----

def init_body() -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"bend-harness\",\"version\":\"1.0\"}}}"

def notif_body() -> String:
  "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}"

def call_json(+tool: String, +args: String) -> String:
  J.stringify(J.obj(Con{J.kv("jsonrpc", J.str("2.0")), Con{J.kv("id", J.num(2n)), Con{J.kv("method", J.str("tools/call")), Con{J.kv("params", J.obj(Con{J.kv("name", J.str(tool)), Con{J.kv("arguments", J.raw(args)), Nil{}}})), Nil{}}}}}))

# the body goes out as UTF-8 bytes (Http counts Content-Length in the
# string's units: a raw "·" in the arguments made it short by one byte per
# non-ASCII char, the server read a cut JSON and answered 400, "mcp call
# failed: non-200 http status"; provider.bend encodes its body the same way)
def call_body(+tool: String, +args: String) -> String:
  Enc.utf8.encode(call_json(tool, args))

# a plugin tool's cid is already its loopback URL (the plugins bridge);
# the Mistral key never goes there
def is_local(+cid: String) -> Bool:
  String.starts_with(cid, "http://127.0.0.1:")

# base: Mistral's API (BISE_MISTRAL_URL names a fake one in tests)
def gateway_url(+base: String, +cid: String) -> String:
  Bool.pick(String, is_local(cid), cid,
    String.concat([base, "/v1/connectors-gateway/", cid, "/mcp"]))

def mistral_base.of(+s: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(s)), "https://api.mistral.ai",
    String.trim(s))

# Mistral's API base: BISE_MISTRAL_URL (a test setting), else Mistral's
def mistral_base() -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_MISTRAL_URL")
    IO.pure(String, mistral_base.of(P.env_or(r, "")))

def mcp_hdrs(+key: String) -> Map<&2, List<&2, String>>:
  Http.set(Http.set(Http.set(Http.empty(), "Authorization",
      String.append("Bearer ", key)),
    "Content-Type", "application/json"), "Accept",
    "application/json, text/event-stream")

def content_texts(blocks: List<&2, Jv.Val>) -> String:
  match blocks:
    case Nil{}:
      ""
    case Con{+b, rest}:
      String.append(jstr(b, "text", ""),
        String.append("\n", content_texts(rest)))

# a result's text blocks; a result with none (an error made of
# structuredContent, or of blocks that are not text) is shown as its JSON
def res_text(+texts: String, +r: Jv.Val) -> String:
  Bool.pick(String, String.is_empty(String.trim(texts)),
    Bool.pick(String, String.eq(Jv.encode(r), "null"), "", Jv.encode(r)), texts)

def call_res(+j: Jv.Val) -> String & Bool:
  (res_text(content_texts(jarr(jdone(Jv.get(j, "result")), "content")),
      jdone(Jv.get(j, "result"))),
    jbool(jdone(Jv.get(j, "result")), "isError", False{}))

def call_ok(sp: String & Bool, +view: String) -> Mcp.Try:
  (out, is_err) = sp
  Mcp.TOk{Bool.not(is_err), out, view}

# a JSON-RPC error the server answered instead of a result: its code
# (the JSON text, "" when absent) and its message with its data (a
# validation error's details are often there)
def rpc_code(m: Maybe<&2, Jv.Val>) -> String:
  match m:
    case Some{Jv.Num{+s}}:
      s
    case _:
      ""

def rpc_data(m: Maybe<&2, Jv.Val>) -> String:
  match m:
    case None{}:
      ""
    case Some{+d}:
      Bool.pick(String, String.eq(Jv.encode(d), "null"), "",
        String.append(" data: ", Jv.encode(d)))

def rpc_err.of(m: Maybe<&2, Jv.Val>) -> Maybe<&2, Mcp.Why>:
  match m:
    case None{}:
      None{}
    case Some{+e}:
      Some{Mcp.WRpc{rpc_code(Jv.get(e, "code")), String.append(
        jstr(e, "message", Jv.encode(e)), rpc_data(Jv.get(e, "data")))}}

def rpc_err(+j: Jv.Val) -> Maybe<&2, Mcp.Why>:
  rpc_err.of(Jv.get(j, "error"))

# a 200 body (JSON, or an event stream whose last data line is the answer)
def rpc_json(+body: String) -> Jv.Val:
  jdone(Jv.parse(Mcp.rpc_text(body)))

def call_rpc(m: Maybe<&2, Mcp.Why>, +j: Jv.Val, +view: String) -> Mcp.Try:
  match m:
    case Some{+w}:
      Mcp.TFail{Mcp.SCall{}, w}
    case None{}:
      call_ok(call_res(j), view)

# view: the bridge's x-bise-view header ("" when none)
def call_status(ok: Bool, +status: U32, +body: String, +view: String) -> Mcp.Try:
  match ok:
    case True{}:
      call_rpc(rpc_err(rpc_json(body)), rpc_json(body), view)
    case False{}:
      Mcp.TFail{Mcp.SCall{}, Mcp.WStatus{status, body}}

def call_got(r: Result<&2, &2, Http.Err, Http.Res>) -> Mcp.Try:
  match r:
    case Fail{+e}:
      Mcp.TFail{Mcp.SCall{}, Mcp.WNet{Pv.net_of(e)}}
    case Done{+res}:
      match res:
        case Http.Res{+status, +hs, _}:
          call_status(U32.is_eq(status, 200), status, Http.text(res),
            Http.header(hs, "x-bise-view"))

# close the try's connection, hand back its outcome
def try_end(p: Http.Pool, +t: Mcp.Try) -> IO(Mcp.Try):
  do IO<Mcp.Try>:
    u : Unit <- Http.pool.close(p)
    IO.pure(Mcp.Try, t)

def try_called(pr3: Http.Pool & Result<&2, &2, Http.Err, Http.Res>)
    -> IO(Mcp.Try):
  (p3, r3) = pr3
  try_end(p3, call_got(r3))

# a call's wait: a connector's 60 s; a plugin tool's bridge bounds its
# own calls (mcp.json's tool_timeout_sec, 300 s by default), so the
# REPL waits up to an hour for its answer
def call_ms(+url: String) -> U32:
  Bool.pick(U32, is_local(url), 3600000, 60000)

def try_call(p2: Http.Pool, +url: String, +h: Map<&2, List<&2, String>>,
    +tool: String, +args: String) -> IO(Mcp.Try):
  do IO<Mcp.Try>:
    pr3 : Http.Pool & Result<&2, &2, Http.Err, Http.Res> <- Http.pool.how(p2,
      "POST", url, h, call_body(tool, args), call_ms(url), Http.ModeFollow{})
    try_called(pr3)

# the notification's answer does not matter: tools/call says the rest
def try_notified(pr2: Http.Pool & Result<&2, &2, Http.Err, Http.Res>,
    +url: String, +h: Map<&2, List<&2, String>>, +tool: String,
    +args: String) -> IO(Mcp.Try):
  (p2, _) = pr2
  try_call(p2, url, h, tool, args)

def try_open(p1: Http.Pool, +url: String, +h: Map<&2, List<&2, String>>,
    +tool: String, +args: String) -> IO(Mcp.Try):
  do IO<Mcp.Try>:
    pr2 : Http.Pool & Result<&2, &2, Http.Err, Http.Res> <- Http.pool.how(p1,
      "POST", url, h, notif_body(), 20000, Http.ModeFollow{})
    try_notified(pr2, url, h, tool, args)

def hdr_if(empty: Bool, +h: Map<&2, List<&2, String>>, +k: String,
    +v: String) -> Map<&2, List<&2, String>>:
  match empty:
    case True{}:
      h
    case False{}:
      Http.set(h, k, v)

# MCP Streamable HTTP: the session the server gave at initialize, and
# the protocol version it chose, go on every later request (a server
# that keeps sessions answers 400/404 without them; the gateway gives
# none, and nothing changes there)
def with_session(+h: Map<&2, List<&2, String>>, +sid: String, +ver: String)
    -> Map<&2, List<&2, String>>:
  hdr_if(String.is_empty(ver),
    hdr_if(String.is_empty(sid), h, "Mcp-Session-Id", sid),
    "MCP-Protocol-Version", ver)

def try_init_rpc(m: Maybe<&2, Mcp.Why>, p1: Http.Pool, +url: String,
    +h: Map<&2, List<&2, String>>, +tool: String, +args: String)
    -> IO(Mcp.Try):
  match m:
    case Some{+w}:
      try_end(p1, Mcp.TFail{Mcp.SInit{}, w})
    case None{}:
      try_open(p1, url, h, tool, args)

def try_init_ok(p1: Http.Pool, +j: Jv.Val, +sid: String, +url: String,
    +h: Map<&2, List<&2, String>>, +tool: String, +args: String)
    -> IO(Mcp.Try):
  try_init_rpc(rpc_err(j), p1, url,
    with_session(h, sid, jstr(jdone(Jv.get(j, "result")), "protocolVersion",
      "")), tool, args)

def try_init_status(ok: Bool, p1: Http.Pool, +status: U32,
    +hs: Map<&2, List<&2, String>>, +body: String, +url: String,
    +h: Map<&2, List<&2, String>>, +tool: String, +args: String)
    -> IO(Mcp.Try):
  match ok:
    case True{}:
      try_init_ok(p1, rpc_json(body), Http.header(hs, "mcp-session-id"), url,
        h, tool, args)
    case False{}:
      try_end(p1, Mcp.TFail{Mcp.SInit{}, Mcp.WStatus{status, body}})

def try_init(r1: Result<&2, &2, Http.Err, Http.Res>, p1: Http.Pool,
    +url: String, +h: Map<&2, List<&2, String>>, +tool: String,
    +args: String) -> IO(Mcp.Try):
  match r1:
    case Fail{+e}:
      try_end(p1, Mcp.TFail{Mcp.SInit{}, Mcp.WNet{Pv.net_of(e)}})
    case Done{+res}:
      match res:
        case Http.Res{+status, +hs, _}:
          try_init_status(U32.is_eq(status, 200), p1, status, hs,
            Http.text(res), url, h, tool, args)

def try_started(pr1: Http.Pool & Result<&2, &2, Http.Err, Http.Res>,
    +url: String, +h: Map<&2, List<&2, String>>, +tool: String,
    +args: String) -> IO(Mcp.Try):
  (p1, r1) = pr1
  try_init(r1, p1, url, h, tool, args)

# one try on a fresh connection: initialize, notification, tools/call
def attempt(+url: String, +h: Map<&2, List<&2, String>>, +tool: String,
    +args: String) -> IO(Mcp.Try):
  do IO<Mcp.Try>:
    pr1 : Http.Pool & Result<&2, &2, Http.Err, Http.Res> <- Http.pool.how(
      Http.pool.new(), "POST", url, h, init_body(), 20000, Http.ModeFollow{})
    try_started(pr1, url, h, tool, args)

# ---- the tries of one call (the policy: runtime/mcp-pure.bend) ----

# one line in the session's debug log (events.jsonl; nothing without
# BEND_DEBUG_DIR): every retry and every call that failed for good
def log_ev(+kind: String, +fields: List<&2, Dp.Field>) -> IO(Unit):
  do IO<Unit>:
    +d : String <- Dbg.dir()
    +ts : Nat <- Dbg.now(d)
    Dbg.append(d, String.append(Dp.ev_line(ts, kind, fields), "\n"))

def try_why(+host: String, t: Mcp.Try) -> String:
  match t:
    case Mcp.TOk{_, _, _}:
      ""
    case Mcp.TFail{+st, +w}:
      Mcp.why_line(host, st, w)

# the answer the model reads: a result as it came; a tool that answered
# an error, or a call that failed for good, in words that say the kind,
# the server's own message, and (its arguments may be the cause) what was
# sent and the tool's parameters (runtime/mcp-args.bend)
def try_mr.ok(ok: Bool, +out: String, +name: String, +host: String,
    +schema: String, +args: String) -> MR:
  match ok:
    case True{}:
      MR{True{}, out}
    case False{}:
      MR{False{}, Ma.tool_failed(name, host, schema, args, out)}

def try_mr(t: Mcp.Try, +name: String, +host: String, +schema: String,
    +args: String, +n: Nat) -> MR:
  match t:
    case Mcp.TOk{ok, +out, _}:
      try_mr.ok(ok, out, name, host, schema, args)
    case Mcp.TFail{+st, +w}:
      MR{False{}, Ma.call_failed(name, host, schema, args, st, w, n)}

def try_done.log(ok: Bool, +name: String, +text: String, +n: Nat) -> IO(Unit):
  match ok:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      log_ev("mcp_failed", Con{Dp.FStr{"tool", name}, Con{Dp.FNat{"retries",
        n}, Con{Dp.FStr{"error", text}, Nil{}}}})

# the view's ref a try got from the bridge ("" when none)
def try_view(t: Mcp.Try) -> String:
  match t:
    case Mcp.TOk{_, _, +v}:
      v
    case Mcp.TFail{_, _}:
      ""

def note_view.opened(f: Result<&1, &1, U32 & String, File>, +line: String)
    -> IO(Unit):
  P.write.opened(f, line)

def note_view.put(skip: Bool, +path: String, +line: String) -> IO(Unit):
  match skip:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        f : Result<&1, &1, U32 & String, File> <- File.open(path, "a")
        note_view.opened(f, line)

# one `<tool id> <ref>` line appended to the views file (none without a
# ref or an id)
def note_view(+tid: String, +ref: String) -> IO(Unit):
  do IO<Unit>:
    +side : String <- P.side_dir()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +line : String <- IO.pure(String, Mcp.view_entry(tid, ref))
    note_view.put(String.is_empty(line), Mcp.views_path(side, P.env_or(pr, "0")), line)

def try_done(+m: MR, +name: String, +n: Nat) -> IO(MR):
  match m:
    case MR{+ok, +out}:
      do IO<MR>:
        u : Unit <- try_done.log(ok, name, out, n)
        IO.pure(MR, m)

# go again (after a pause, with a fresh initialize) or answer; `more`
# is the next try, built lazily and run only when chosen
def try_next(go: Bool, +t: Mcp.Try, +name: String, +host: String,
    +schema: String, +args: String, +n: Nat, +tid: String, more: IO(MR)) -> IO(MR):
  match go:
    case True{}:
      do IO<MR>:
        u : Unit <- log_ev("mcp_retry", Con{Dp.FStr{"tool", name},
          Con{Dp.FNat{"retry", Nat.add(n, 1n)}, Con{Dp.FStr{"why",
            try_why(host, t)}, Nil{}}}})
        w : Unit <- IO.sleep(Mcp.pause_ms(n))
        more
    case False{}:
      do IO<MR>:
        u : Unit <- note_view(tid, try_view(t))
        try_done(try_mr(t, name, host, schema, args, n), name, n)

# tid: the tool's id read at the call's start ("" when no view is asked)
def tries(+fuel: Nat, +n: Nat, +name: String, +host: String, +url: String,
    +h: Map<&2, List<&2, String>>, +tool: String, +schema: String,
    +args: String, +tid: String) -> IO(MR):
  match fuel:
    case 0n:
      IO.pure(MR, MR{False{}, "mcp call failed: no try left"})
    case 1n+p:
      do IO<MR>:
        +t : Mcp.Try <- attempt(url, h, tool, args)
        try_next(Bool.and(Mcp.again(t), Bool.not(Nat.is_eq(p, 0n))), t, name,
          host, schema, args, n, tid, tries(p, Nat.add(n, 1n), name, host, url, h,
            tool, schema, args, tid))

# the tool running now (its id; "" when unknown or not asked)
def tool_now.read(asks: Bool) -> IO(String):
  match asks:
    case False{}:
      IO.pure(String, "")
    case True{}:
      do IO<String>:
        +side : String <- P.side_dir()
        pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
        +now : String <- P.read_or(Mcp.tool_now_path(side, P.env_or(pr, "0")), "")
        IO.pure(String, String.trim(now))

# a tool with a view asks its bridge to keep the full result (C2); acct:
# the account the call names (its connection's name, sent as
# x-credentials-name), "" for his default
def mcp_call.send(+name: String, +cid: String, +tool: String,
    +schema: String, +args: String, +key: String, +ui: String, +acct: String)
    -> IO(MR):
  do IO<MR>:
    +tid : String <- tool_now.read(Mcp.asks_view(cid, ui))
    +base : String <- mistral_base()
    tries(Mcp.max_tries(), 0n, name, Pvp.url_host(gateway_url(base, cid)),
      gateway_url(base, cid), hdr_if(String.is_empty(acct),
        hdr_if(Bool.not(Mcp.asks_view(cid, ui)),
          mcp_hdrs(Bool.pick(String, is_local(cid), "", key)), "x-bise-call", "1"),
        "x-credentials-name", acct),
      tool, schema, args, tid)

# a gateway call needs the Mistral key: without one, say so and send nothing
def mcp_call.keyed(no_key: Bool, +name: String, +cid: String, +tool: String,
    +schema: String, +args: String, +key: String, +ui: String, +acct: String)
    -> IO(MR):
  match no_key:
    case True{}:
      IO.pure(MR, MR{False{}, Mcp.no_key_text()})
    case False{}:
      mcp_call.send(name, cid, tool, schema, args, key, ui, acct)

def mcp_call.found(+name: String, +cid: String, +tool: String,
    +schema: String, +args: String, +key: String, +ui: String, +acct: String)
    -> IO(MR):
  mcp_call.keyed(Bool.and(String.is_empty(key), Bool.not(is_local(cid))),
    name, cid, tool, schema, args, key, ui, acct)

# a call with no arguments still needs a JSON object body
def call_args.of(is_empty: Bool, +args: String) -> String:
  match is_empty:
    case True{}:
      "{}"
    case False{}:
      args

def call_args(+args: String) -> String:
  call_args.of(String.is_empty(args), args)

# ---- the account a call names (Mcp.account_pick) ----
# A gateway connector's call may carry `account`; a plugin's own tool, or
# a tool whose schema takes its own `account`, keeps it as an argument.

def jsome(m: Maybe<&2, Jv.Val>) -> Bool:
  match m:
    case Some{_}:
      True{}
    case None{}:
      False{}

def schema_takes.props(m: Maybe<&2, Jv.Val>) -> Bool:
  match m:
    case Some{+p}:
      jsome(Jv.get(p, "account"))
    case None{}:
      False{}

def schema_takes.of(m: Maybe<&2, Jv.Val>) -> Bool:
  match m:
    case Some{+v}:
      schema_takes.props(Jv.get(v, "properties"))
    case None{}:
      False{}

# the tool's input schema has its own `account` property
def schema_takes_account(+schema: String) -> Bool:
  schema_takes.of(Jv.parse(schema))

# whether `account` is bise's to read for this tool
def account_ours(+cid: String, +schema: String) -> Bool:
  Bool.and(Bool.not(is_local(cid)), Bool.not(schema_takes_account(schema)))

def arg_account.w(m: Maybe<&2, Jv.Val>) -> Maybe<&2, String>:
  match m:
    case Some{Jv.Str{+s}}:
      Some{s}
    case _:
      None{}

def arg_account.p(m: Maybe<&2, Jv.Val>) -> Maybe<&2, String>:
  match m:
    case Some{+v}:
      arg_account.w(Jv.get(v, "account"))
    case None{}:
      None{}

# the call's `account` argument (a string), if any
def arg_account(+args: String) -> Maybe<&2, String>:
  arg_account.p(Jv.parse(args))

def strip_account.v(v: Jv.Val, +args: String) -> String:
  match v:
    case Jv.Obj{m}:
      Jv.encode(Jv.Obj{Map.del(&2, Jv.Val, m, "account")})
    case _:
      args

def strip_account.p(m: Maybe<&2, Jv.Val>, +args: String) -> String:
  match m:
    case Some{v}:
      strip_account.v(v, args)
    case None{}:
      args

# the arguments without `account` (the tool never sees it)
def strip_account(+args: String) -> String:
  strip_account.p(Jv.parse(args), args)

def with_account.v(v: Jv.Val, +args: String, +n: String) -> String:
  match v:
    case Jv.Obj{m}:
      Jv.encode(Jv.Obj{Map.set(&2, Jv.Val, m, "account", Jv.Str{n})})
    case _:
      args

def with_account.p(m: Maybe<&2, Jv.Val>, +args: String, +n: String) -> String:
  match m:
    case Some{v}:
      with_account.v(v, args, n)
    case None{}:
      args

# the arguments with `account` set to the connection's name the gate asked
# about: the call resolves a name first, so it sends that account or, if
# it is gone since, refuses (never another one)
def with_account(+args: String, +n: String) -> String:
  with_account.p(Jv.parse(args), args, n)

def acct_of(+v: Jv.Val) -> Mcp.Acct:
  Mcp.Acct{jstr(v, "name", ""), jstr(v, "title", ""), jbool(v, "default", False{})}

def accts_list(xs: List<&2, Jv.Val>) -> List<&2, Mcp.Acct>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, rest}:
      Con{acct_of(x), accts_list(rest)}

def conn_of(+v: Jv.Val) -> Mcp.ConnAccts:
  Mcp.ConnAccts{jstr(v, "cid", ""), jstr(v, "group", ""), accts_list(jarr(v, "accounts"))}

def conns_list(xs: List<&2, Jv.Val>) -> List<&2, Mcp.ConnAccts>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, rest}:
      Con{conn_of(x), conns_list(rest)}

# the accounts file's text, typed (none when missing or not a list)
def conns_of(m: Maybe<&2, String>) -> List<&2, Mcp.ConnAccts>:
  match m:
    case None{}:
      Nil{}
    case Some{+t}:
      conns_list(jarr.w(jdone(Jv.parse(t))))

# the session's accounts, as its plugins bridge wrote them beside the index
def accounts_read() -> IO(List<&2, Mcp.ConnAccts>):
  do IO<List<&2, Mcp.ConnAccts>>:
    +pp : String <- Pl.mcp_accounts_path()
    m : Maybe<&2, String> <- P.read_file(pp)
    IO.pure(List<&2, Mcp.ConnAccts>, conns_of(m))

def mt_pick.ours(ours: Bool, +cid: String, want: Maybe<&2, String>, +name: String)
    -> IO(Mcp.CallPick):
  match ours:
    case False{}:
      IO.pure(Mcp.CallPick, Mcp.CPGo{"", ""})
    case True{}:
      do IO<Mcp.CallPick>:
        cs : List<&2, Mcp.ConnAccts> <- accounts_read()
        IO.pure(Mcp.CallPick, Mcp.call_pick(Mcp.accts_of(cs, cid), want,
          Mcp.group_of(name)))

def call_account.found(m: Maybe<&2, McpTool>, +name: String, +args: String)
    -> IO(Mcp.CallPick):
  match m:
    case Some{MT{+cid, _, +schema, _}}:
      mt_pick.ours(account_ours(cid, schema), cid, arg_account(args), name)
    case _:
      IO.pure(Mcp.CallPick, Mcp.CPGo{"", ""})

def call_account.read(m: Maybe<&2, String>, +name: String, +args: String)
    -> IO(Mcp.CallPick):
  match m:
    case Some{+content}:
      call_account.found(find_line(content, name), name, args)
    case None{}:
      IO.pure(Mcp.CallPick, Mcp.CPGo{"", ""})

# the account a gateway call runs with and the label its card shows,
# resolved ONCE above the call (main.bend, every mode): the index for its
# connector, then its accounts (read at each call: a renamed or new
# account is never stale). Not a gateway connector's call: nothing
def call_account(+name: String, +args: String) -> IO(Mcp.CallPick):
  do IO<Mcp.CallPick>:
    m : Maybe<&2, String> <- index_content()
    call_account.read(m, name, args)

# the arguments a resolved call runs with: `account` set to the picked
# connection's name ("" for the default: the arguments as they are)
def pick_args(p: Mcp.CallPick, +args: String) -> String:
  match p:
    case Mcp.CPGo{+n, _}:
      Bool.pick(String, String.is_empty(n), args, with_account(args, n))
    case Mcp.CPRefuse{_}:
      args

# the account the call carries, already resolved above it: it leaves the
# arguments and goes as x-credentials-name; mcp_call looks nothing up
def mcp_call.acct(m: Maybe<&2, String>, ours: Bool, +name: String, +cid: String,
    +tool: String, +schema: String, +args: String, +key: String, +ui: String)
    -> IO(MR):
  match m:
    case Some{+n}:
      Bool.pick(IO(MR), ours,
        mcp_call.found(name, cid, tool, schema, call_args(strip_account(args)), key, ui, n),
        mcp_call.found(name, cid, tool, schema, call_args(args), key, ui, ""))
    case None{}:
      mcp_call.found(name, cid, tool, schema, call_args(args), key, ui, "")

def mcp_call.mt(mt: McpTool, +name: String, +args: String, +key: String)
    -> IO(MR):
  match mt:
    case MT{+cid, +tool, +schema, +ui}:
      mcp_call.acct(arg_account(args), account_ours(cid, schema), name, cid,
        tool, schema, args, key, ui)

# no index: the call is still named (issue 21: a nameless answer read
# like an empty result)
def index_missing(+name: String) -> MR:
  MR{False{}, Xp.unknown_text(name,
    "the tool index is missing: remote MCP tools are unavailable in this session")}

def mcp_call.resolved(m: Maybe<&2, McpTool>, +name: String, +args: String,
    +key: String) -> IO(MR):
  match m:
    case None{}:
      IO.pure(MR, MR{False{}, Xp.unknown_text(name,
        "discover tools with search_tool_functions")})
    case Some{+mt}:
      mcp_call.mt(mt, name, args, key)

def mcp_call.read(m: Maybe<&2, String>, +name: String, +args: String,
    +key: String) -> IO(MR):
  match m:
    case None{}:
      IO.pure(MR, index_missing(name))
    case Some{+content}:
      mcp_call.resolved(find_line(content, name), name, args, key)

def mcp_call(+name: String, +args: String, +key: String) -> IO(MR):
  do IO<MR>:
    m : Maybe<&2, String> <- index_content()
    mcp_call.read(m, name, args, key)
