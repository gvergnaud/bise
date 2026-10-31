# runtime/mcp-pure.bend - the pure half of runtime/mcp.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/types.bend as T
import ../core/text.bend as Tx
import ../core/wire.bend as W
import ./provider-pure.bend as Pvp

# The connectors' aliases (vibe's normalize and dedupe), the bootstrap
# URL and bise's image_generation note are Rust's now
# (bend_plugins::connectors): the session's bridge writes the index.

# whether a name is in a list (the codebase patterns: leaf helpers
# first, the recursive def calls itself by its own name, the recursive
# result rides as an argument)

def seen_pick(hit: Bool, rest_res: Bool) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      rest_res

def seen_has(seen: List<&2, String>, +alias: String) -> Bool:
  match seen:
    case Nil{}:
      False{}
    case Con{+h, rest}:
      seen_pick(String.eq(h, alias), seen_has(rest, alias))

# ---- views (docs/mcp-apps.md C1-C3) ----
# An index line of a tool with a view ends with ` | ui: <uri>` (the uri
# has no space). The runtime splits it off the description (the model
# reads neither the field nor the uri), asks the bridge to keep the
# call's full result (`x-bise-call: 1`, a local tool with a ui only) and
# puts the bridge's answer `x-bise-view: <ref>` on a `tool_view` line.

def ui_marker() -> String:
  " | ui: "

# the last place of the marker: `m` is the marker's next place found
# (Tx.find's third argument only offsets the index it answers, so the
# search goes on in the text after the last hit),
# `best` the last one before it; fuel bounds the steps (the text's length)
def ui_last(fuel: Nat, m: Maybe<&2, Nat>, +desc: String,
    +best: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match fuel:
    case 0n:
      best
    case 1n+k:
      match m:
        case None{}:
          best
        case Some{+p}:
          ui_last(k, Tx.find(String.drop(desc, Nat.add(p, 1n)), ui_marker(),
            Nat.add(p, 1n)), desc, Some{p})

# the text after the last marker is the field only when it is a `ui://`
# uri (MCP Apps' only scheme) with no space: a tool with no view whose
# schema ends with ` | ui: <word>` keeps its whole description
def desc_ui.uri(ok: Bool, +desc: String, +p: Nat, +rest: String) -> String & String:
  match ok:
    case True{}:
      (String.take(desc, p), rest)
    case False{}:
      (desc, "")

def desc_ui.rest(sp: Maybe<&2, Nat>, +desc: String, +p: Nat, +rest: String)
    -> String & String:
  match sp:
    case None{}:
      desc_ui.uri(String.starts_with(rest, "ui://"), desc, p, rest)
    case Some{_}:
      (desc, "")

def desc_ui.at(m: Maybe<&2, Nat>, +desc: String) -> String & String:
  match m:
    case None{}:
      (desc, "")
    case Some{+p}:
      desc_ui.rest(Tx.find(String.drop(desc, Nat.add(p, String.length(ui_marker()))), " ", 0n),
        desc, p, String.drop(desc, Nat.add(p, String.length(ui_marker()))))

# an index line's description part: (the part without its ui field, the
# view's uri or "")
def desc_ui(+desc: String) -> String & String:
  desc_ui.at(ui_last(String.length(desc), Tx.find(desc, ui_marker(), 0n), desc,
    None{}), desc)

def desc_only.of(sp: String & String) -> String:
  (d, u) = sp
  d

def ui_only.of(sp: String & String) -> String:
  (d, u) = sp
  u

def desc_only(+desc: String) -> String:
  desc_only.of(desc_ui(desc))

def ui_only(+desc: String) -> String:
  ui_only.of(desc_ui(desc))

# the call asks the bridge to keep its full result: a plugin tool (its
# loopback bridge) with a view; never a connector's
def asks_view(+cid: String, +ui: String) -> Bool:
  Bool.and(String.starts_with(cid, "http://127.0.0.1:"),
    Bool.not(String.is_empty(ui)))

# the runtime's two files of views, in its side folder (BEND_AGENT_RUN),
# one each per REPL: the tool running now (`<id>`, written at its start)
# and the refs kept since the last tool's end (`<id> <ref>` lines)
def tool_now_path(+side: String, +port: String) -> String:
  String.concat([side, "/bend-tool-", port, ".txt"])

def views_path(+side: String, +port: String) -> String:
  String.concat([side, "/bend-views-", port, ".txt"])

# one ref of the views file: `<tool id> <ref>`; "" when the bridge kept
# nothing (no header) or no tool is known
def view_entry(+id: String, +ref: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(String.trim(ref)), String.is_empty(String.trim(id))), "",
    String.concat([String.trim(id), " ", String.trim(ref), "\n"]))

# a views file line as the wire line `tool_view #<id> : <ref>` (C3);
# a line without both: nothing
def view_line.at(m: Maybe<&2, Nat>, +l: String) -> List<&2, String>:
  match m:
    case None{}:
      Nil{}
    case Some{+p}:
      Bool.pick(List<&2, String>, Nat.is_eq(p, 0n), Nil{},
        Con{String.concat(["tool_view #", String.take(l, p), " : ",
          String.trim(String.drop(l, Nat.add(p, 1n)))]), Nil{}})

def view_line(+l: String) -> List<&2, String>:
  view_line.at(Tx.find(String.trim(l), " ", 0n), String.trim(l))

def view_lines.go(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      List.append(&2, String, view_line(l), view_lines.go(rest))

# the views file's text as its wire lines, in order
def view_lines(+content: String) -> List<&2, String>:
  view_lines.go(String.split(content, Tx.newline()))

# ---- the index line format, parsed ----
# line: "<cid> <cname> <tool> : #<desc>", the desc part ending
# " | input: <schema>" and, for a tool with a view, " | ui: <uri>"

def line_parts.n2(sp: Maybe<&2, Nat>, +cid: String, +cname: String,
    +rest: String) -> String & String & String & String:
  match sp:
    case Some{+q}:
      (cid, cname, String.take(rest, q),
        String.drop(rest, Nat.add(q, 3n)))
    case None{}:
      (cid, cname, "", "")

def line_parts.n(+cid: String, +cname: String, +rest: String)
    -> String & String & String & String:
  line_parts.n2(Tx.find(rest, " : ", 0n), cid, cname, rest)

def line_parts.c2(sp: Maybe<&2, Nat>, +cid: String, +rest: String)
    -> String & String & String & String:
  match sp:
    case Some{+q}:
      line_parts.n(cid, String.take(rest, q),
        String.drop(rest, Nat.add(q, 1n)))
    case None{}:
      (cid, "", "", "")

def line_parts.c(+cid: String, +rest: String)
    -> String & String & String & String:
  line_parts.c2(Tx.find(rest, " ", 0n), cid, rest)

def line_parts.d(sp: Maybe<&2, Nat>, +line: String)
    -> String & String & String & String:
  match sp:
    case Some{+p}:
      line_parts.c(String.take(line, p), String.drop(line, Nat.add(p, 1n)))
    case None{}:
      ("", "", "", "")

def line_parts(+line: String) -> String & String & String & String:
  line_parts.d(Tx.find(line, " ", 0n), line)

# the tool name of an index line ("" when the line is malformed)
def line_tool.of(sp: String & String & String & String) -> String:
  (a0, a1, tool, a3) = sp
  tool

def line_tool(+line: String) -> String:
  line_tool.of(line_parts(line))

# ---- the index file becomes tools ----

def pub_name(+cname: String, +tool: String) -> String:
  String.append(cname, String.append(".", tool))

# the model reads the description without its ui field (C1)
def index_tool.of(cid: String, cname: String, tool: String, +desc: String)
    -> T.ToolDef:
  T.ToolDef{pub_name(cname, tool), desc_only(desc)}

# skip a malformed line; cons its ToolDef onto the recursive result
# (the discovery.bend pattern: the unchosen branch is never forced)
def index_put.of(sp: String & String & String & String,
    +rest_res: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  (cid, cname, tool, desc) = sp
  Con{index_tool.of(cid, cname, tool, desc), rest_res}

def index_put(skip: Bool, +line: String,
    +rest_res: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match skip:
    case True{}:
      rest_res
    case False{}:
      index_put.of(line_parts(line), rest_res)

def index_go(lines: List<&2, String>) -> List<&2, T.ToolDef>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+line, rest}:
      index_put(String.is_empty(line_tool(line)), line, index_go(rest))

def index_of(+content: String) -> List<&2, T.ToolDef>:
  index_go(String.split(content, Tx.newline()))

# ---- the unified search: static catalog + MCP index ----
# search_tool_functions routes here; reading the index is an effect, so
# the search runs on the EExec path (bash and MCP calls live there too)

# the live surface searches the LIVE catalog (the parity tools, the
# skill loader, and the self.* harness tools) plus the MCP index; the
# scripted surface stays on catalog_short so the scenarios never read
# files (a search that could not see `skill` made the model guess wrong
# names from inside the sandbox)
def merged_tools.live(cat: List<&2, T.ToolDef>, m: Maybe<&2, String>)
    -> List<&2, T.ToolDef>:
  match m:
    case None{}:
      cat
    case Some{+content}:
      List.append(&2, T.ToolDef, cat, index_of(content))

# ---- the tool groups of a session, listed in the system prompt ----
# (vibe's "Searchable connector groups"): each `tools.<group>` namespace
# of the catalog plus the MCP index, in catalog order, with its number
# of functions. A top-level tool (bash, run_typescript...) has no group.

# a tool's group: its name before the first "." ("" for a top-level tool)
def group_of.pick(m: Maybe<&2, Nat>, +name: String) -> String:
  match m:
    case None{}:
      ""
    case Some{+p}:
      String.take(name, p)

def group_of(+name: String) -> String:
  group_of.pick(Tx.find(name, ".", 0n), name)

def tool_group(+td: T.ToolDef) -> String:
  T.ToolDef{+name, _} = td
  group_of(name)

def groups.put(skip: Bool, +g: String, +acc: List<&2, String>)
    -> List<&2, String>:
  match skip:
    case True{}:
      acc
    case False{}:
      Con{g, acc}

def groups.go(tools: List<&2, T.ToolDef>, +acc: List<&2, String>)
    -> List<&2, String>:
  match tools:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+t, rest}:
      groups.go(rest, groups.put(
        Bool.or(String.is_empty(tool_group(t)), seen_has(acc, tool_group(t))),
        tool_group(t), acc))

# the distinct groups, in catalog order
def groups_of(tools: List<&2, T.ToolDef>) -> List<&2, String>:
  groups.go(tools, Nil{})

def group_count.add(hit: Bool, +n: Nat) -> Nat:
  match hit:
    case True{}:
      1n+n
    case False{}:
      n

def group_count(tools: List<&2, T.ToolDef>, +g: String) -> Nat:
  match tools:
    case Nil{}:
      0n
    case Con{+t, rest}:
      group_count.add(String.eq(tool_group(t), g), group_count(rest, g))

def group_line.unit(one: Bool) -> String:
  match one:
    case True{}:
      " function)"
    case False{}:
      " functions)"

def group_line(+g: String, +n: Nat) -> String:
  String.append("- `", String.append(g, String.append("` (",
    String.append(Nat.show(n), group_line.unit(Nat.is_eq(n, 1n))))))

def group_lines(gs: List<&2, String>, +tools: List<&2, T.ToolDef>) -> String:
  match gs:
    case Nil{}:
      ""
    case Con{+g, rest}:
      String.append("\n", String.append(group_line(g, group_count(tools, g)),
        group_lines(rest, tools)))

def groups_section.pick(empty: Bool, +lines: String) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      String.append("### Tool groups\n\nThe `tools.<group>` namespaces of this session. To search one group, pass its name in the `connectors` argument of `search_tool_functions`.\n", lines)

# the prompt subsection ("" when no tool has a group)
def groups_section(+tools: List<&2, T.ToolDef>) -> String:
  groups_section.pick(String.is_empty(group_lines(groups_of(tools), tools)),
    group_lines(groups_of(tools), tools))

# ---- a call that failed: where, why, what bise did, is it safe ----
# A call is initialize, the initialized notification and tools/call on
# one connection (runtime/mcp.bend). It is tried again, on a fresh
# connection with a fresh initialize, only when it never reached the
# tool and a new try may work: initialize got no answer, a 5xx or a 429
# (the network right after the Mac wakes, a gateway hiccup), or
# tools/call could not go out (dns, connect, tls, write) or the server
# forgot the session (a 404, or a refusal that names the session). A
# tools/call the server may have run (no answer read, a timeout, a 5xx,
# an error it answered) is never sent twice: the model gets the cause
# and is told to check first. Every failure says, in one or two lines,
# the server and the phase, the cause, what bise tried, and whether
# calling again is safe (fail_text).

type Stage is Data:
  SInit{}
  SCall{}

# why a try stopped: no answer at all, an HTTP status (with its raw
# body), or a JSON-RPC error the server answered (its code as the JSON
# text, "" when absent; its message, with its data when it has some)
type Why is Data:
  WNet{e: Pvp.NetErr}
  WStatus{code: U32, body: String}
  WRpc{code: String, said: String}

# what kind of failure, so the model knows what to fix: its arguments,
# a login, the tool's name, a pause, or nothing it can fix (the server,
# the network)
type Cause is Data:
  KArgs{}
  KAuth{}
  KMissing{}
  KRate{}
  KServer{}
  KTimeout{}
  KNet{}
  KTool{}

# one try: the tool's answer, or where and why it stopped
# (`view`: the bridge's `x-bise-view` answer header, "" when none)
type Try is Data:
  TOk{ok: Bool, out: String, view: String}
  TFail{stage: Stage, why: Why}

# a connector call with no Mistral key: said at once, nothing sent (an
# empty bearer came back as the gateway's 'HTTP 401: Invalid API Key',
# which read like a revoked key)
def no_key_text() -> String:
  "no Mistral key for connectors: run bise login mistral"

# tries in total for one call (so at most 2 retries)
def max_tries() -> Nat:
  3n

# the pause before retry n (0-based): 1 s, then 2 s
def pause_ms(n: Nat) -> U32:
  Pvp.backoff_ms(n)

def session_words(+s: String) -> Bool:
  String.contains(String.to_lower(s), "session")

def is_call(st: Stage) -> Bool:
  match st:
    case SInit{}:
      False{}
    case SCall{}:
      True{}

# the server forgot the session between initialize and tools/call
def session_gone(w: Why, +st: Stage) -> Bool:
  match w:
    case WNet{_}:
      False{}
    case WStatus{+code, +body}:
      Bool.and(is_call(st), Bool.or(U32.is_eq(code, 404),
        Bool.and(U32.is_eq(code, 400), session_words(body))))
    case WRpc{_, +said}:
      session_words(said)

# a transport failure of tools/call: could the request have reached the
# server whole? not when the name, the connection, the handshake or the
# send failed
def net_sent(e: Pvp.NetErr) -> Bool:
  match e:
    case Pvp.NUrl{}:
      False{}
    case Pvp.NDns{_, _}:
      False{}
    case Pvp.NConnect{_, _}:
      False{}
    case Pvp.NTls{_, _}:
      False{}
    case Pvp.NWrite{_, _}:
      False{}
    case _:
      True{}

def reached.call(w: Why) -> Bool:
  match w:
    case WNet{e}:
      net_sent(e)
    case WStatus{+code, _}:
      U32.is_le(500, code)
    case WRpc{_, +said}:
      Bool.not(session_words(said))

# may the tool have run? never when initialize is what failed
def reached(st: Stage, w: Why) -> Bool:
  match st:
    case SInit{}:
      False{}
    case SCall{}:
      reached.call(w)

def net_transient(e: Pvp.NetErr) -> Bool:
  match e:
    case Pvp.NUrl{}:
      False{}
    case Pvp.NRedirect{}:
      False{}
    case _:
      True{}

# may a fresh try work? (an auth refusal or a bad URL will not)
def transient(w: Why, +st: Stage) -> Bool:
  match w:
    case WNet{e}:
      net_transient(e)
    case WStatus{+code, +body}:
      Bool.or(Pvp.retryable_status(code), session_gone(WStatus{code, body}, st))
    case WRpc{_, +said}:
      session_words(said)

def retry(+st: Stage, +w: Why) -> Bool:
  Bool.and(Bool.not(reached(st, w)), transient(w, st))

# the verdict on a try: go again (with a fresh initialize) or not
def again(t: Try) -> Bool:
  match t:
    case TOk{_, _, _}:
      False{}
    case TFail{+st, +w}:
      retry(st, w)

# ---- the words ----

# the server's own words reach the model whole up to this bound (a
# validation error lists every bad field; 200 cut them)
def said_max() -> Nat:
  1500n

def clip(+s: String) -> String:
  Bool.pick(String, Nat.is_lt(said_max(), String.length(s)),
    String.append(String.take(s, said_max()), "…"), s)

# ---- secrets out of the server's words ----
# A server may echo a header or a URL back: the word after "Bearer", the
# value of a key=, token=, secret= or password= pair, and a word that
# looks like a provider key (sk-, ghp_, xoxb-, ya29., AKIA...) become
# [redacted]. The words are already squeezed (one space between them).

def has_any(+s: String, ws: List<&2, String>) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, rest}:
      Bool.or(String.contains(s, w), has_any(s, rest))

def starts_any(+s: String, ws: List<&2, String>) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, rest}:
      Bool.or(String.starts_with(s, w), starts_any(s, rest))

def redact.kv.at(m: Maybe<&2, Nat>, +w: String) -> String:
  match m:
    case None{}:
      "[redacted]"
    case Some{+i}:
      String.append(String.take(w, Nat.add(i, 1n)), "[redacted]")

def redact.key_like(+w: String) -> Bool:
  Bool.and(Nat.is_le(20n, String.length(w)),
    starts_any(w, ["sk-", "ghp_", "gho_", "ghs_", "github_pat_", "xoxb-",
      "xoxp-", "ya29.", "AKIA"]))

def redact.word.plain(+w: String) -> String:
  Bool.pick(String, has_any(String.to_lower(w), ["token=", "key=",
      "secret=", "password=", "signature="]),
    redact.kv.at(Tx.find(w, "=", 0n), w),
    Bool.pick(String, redact.key_like(w), "[redacted]", w))

def redact.word(after_bearer: Bool, +w: String) -> String:
  match after_bearer:
    case True{}:
      "[redacted]"
    case False{}:
      redact.word.plain(w)

def redact.go(ws: List<&2, String>, after_bearer: Bool) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, rest}:
      Con{redact.word(after_bearer, w),
        redact.go(rest, String.eq(String.to_lower(w), "bearer"))}

def redact(+s: String) -> String:
  String.join(redact.go(String.split(s, ' '), False{}), " ")

# what the model reads of the server's words: one line, no secret,
# bounded
def said_text(+s: String) -> String:
  clip(redact(W.squeeze(s)))

def said_part(+said: String) -> String:
  Bool.pick(String, String.is_empty(said), "",
    String.append(": ", said_text(said)))

# ---- the kind of a failure ----

def kind_name(k: Cause) -> String:
  match k:
    case KArgs{}:
      "bad arguments"
    case KAuth{}:
      "auth"
    case KMissing{}:
      "not found"
    case KRate{}:
      "rate limit"
    case KServer{}:
      "server error"
    case KTimeout{}:
      "timeout"
    case KNet{}:
      "network"
    case KTool{}:
      "tool error"

# a kind told by the server's words, else dflt (rate before auth: "quota
# exceeded for this token"; auth before arguments: "invalid_token")
def words_kind.l(+l: String, dflt: Cause) -> Cause:
  Bool.pick(Cause, has_any(l, ["rate limit", "rate_limit", "ratelimit",
      "too many requests", "quota"]), KRate{},
    Bool.pick(Cause, has_any(l, ["unauthorized", "unauthenticated",
        "forbidden", "permission", "invalid_token", "invalid token",
        "access denied", "not authorized", "token expired", "expired token",
        "credentials", "log in", "login"]), KAuth{},
      Bool.pick(Cause, has_any(l, ["timed out", "timeout",
          "deadline exceeded"]), KTimeout{},
        Bool.pick(Cause, has_any(l, ["not found", "no such", "does not exist",
            "unknown tool"]), KMissing{},
          Bool.pick(Cause, has_any(l, ["invalid", "validation", "required",
              "missing", "argument", "parameter", "unexpected keyword",
              "must be", "malformed", "bad request", "schema"]), KArgs{},
            dflt)))))

def words_kind(+s: String, dflt: Cause) -> Cause:
  words_kind.l(String.to_lower(s), dflt)

# arguments are only the model's fault once they were sent (tools/call)
def args_or(+st: Stage, dflt: Cause) -> Cause:
  Bool.pick(Cause, is_call(st), KArgs{}, dflt)

def status_kind(+code: U32, +body: String, +st: Stage) -> Cause:
  Bool.pick(Cause, Bool.or(U32.is_eq(code, 401), U32.is_eq(code, 403)),
      KAuth{},
    Bool.pick(Cause, U32.is_eq(code, 429), KRate{},
      Bool.pick(Cause, Bool.or(U32.is_eq(code, 408), U32.is_eq(code, 504)),
          KTimeout{},
        Bool.pick(Cause, session_gone(WStatus{code, body}, st), KServer{},
          Bool.pick(Cause, U32.is_eq(code, 404), KMissing{},
            Bool.pick(Cause, Bool.or(U32.is_eq(code, 400),
                U32.is_eq(code, 422)),
              words_kind(body, args_or(st, KServer{})), KServer{}))))))

# JSON-RPC: -32602 invalid params (also MCP's answer to an unknown tool,
# so its words go first), -32700 a body that is not JSON (the arguments
# are put in raw), -32601 no such method or tool, -32001 the MCP SDKs'
# request timeout; else the words
def rpc_kind(+code: String, +said: String, +st: Stage) -> Cause:
  Bool.pick(Cause, Bool.or(String.eq(code, "-32602"),
      String.eq(code, "-32700")), words_kind(said, args_or(st, KServer{})),
    Bool.pick(Cause, String.eq(code, "-32601"), KMissing{},
      Bool.pick(Cause, String.eq(code, "-32001"), KTimeout{},
        words_kind(said, KServer{}))))

def net_kind(e: Pvp.NetErr) -> Cause:
  match e:
    case Pvp.NTimeout{}:
      KTimeout{}
    case _:
      KNet{}

def kind_of(+st: Stage, w: Why) -> Cause:
  match w:
    case WNet{e}:
      net_kind(e)
    case WStatus{+code, +body}:
      status_kind(code, body, st)
    case WRpc{+code, +said}:
      rpc_kind(code, said, st)

# the kinds where the model's arguments may be the cause: it gets the
# tool's parameters and a check of what it sent (runtime/mcp-args.bend)
def wants_schema(k: Cause) -> Bool:
  match k:
    case KArgs{}:
      True{}
    case KTool{}:
      True{}
    case _:
      False{}

def rpc_code_part(+code: String) -> String:
  Bool.pick(String, String.is_empty(code), "",
    String.concat([" (code ", code, ")"]))

def is_url_err(e: Pvp.NetErr) -> Bool:
  match e:
    case Pvp.NUrl{}:
      True{}
    case _:
      False{}

def net_text.pick(bad_url: Bool, +host: String, +e: Pvp.NetErr) -> String:
  match bad_url:
    case True{}:
      String.concat(["bad server URL (", host, ")"])
    case False{}:
      Pvp.net_cause(host, e)

def net_text(+host: String, +e: Pvp.NetErr) -> String:
  net_text.pick(is_url_err(e), host, e)

# the cause: the transport error, the status with the server's own
# words, or its JSON-RPC error
def why_text(+host: String, w: Why) -> String:
  match w:
    case WNet{+e}:
      net_text(host, e)
    case WStatus{+code, +body}:
      String.concat([host, " answered HTTP ", U32.show(code),
        said_part(W.err_text(body))])
    case WRpc{+code, +said}:
      String.concat([host, " answered an error", rpc_code_part(code),
        said_part(said)])

def phase.pick(st: Stage, gone: Bool) -> String:
  match st:
    case SInit{}:
      "initialize"
    case SCall{}:
      Bool.pick(String, gone, "tools/call (the server lost the session)",
        "tools/call")

def phase(+st: Stage, +w: Why) -> String:
  phase.pick(st, session_gone(w, st))

def retried_text(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "",
    String.concat([" bise reconnected and retried ", Nat.show(n),
      Bool.pick(String, Nat.is_eq(n, 1n), " time.", " times.")]))

def safety.pick(sent: Bool, fresh: Bool) -> String:
  match sent:
    case True{}:
      " not retried: the call may have run on the server, check before calling it again (safe if the tool only reads)."
    case False{}:
      Bool.pick(String, fresh,
        " the call never reached the server: safe to call again.",
        " the call never reached the server, but calling again will fail the same way until that is fixed.")

def safety(+st: Stage, +w: Why) -> String:
  safety.pick(reached(st, w), transient(w, st))

# where and why, for the debug log: "initialize: cannot reach …"
def why_line(+host: String, +st: Stage, +w: Why) -> String:
  String.concat([phase(st, w), ": ", why_text(host, w)])

# what the model reads when a call fails for good: the kind, the tool
# and its server, the phase, the cause in the server's own words, the
# retries, and whether it is safe to call again. n = the retries already
# made. (A bad-arguments failure gets the tool's parameters after it:
# runtime/mcp-args.bend.)
def fail_text(+name: String, +host: String, +st: Stage, +w: Why, +n: Nat)
    -> String:
  String.concat(["mcp call failed (", kind_name(kind_of(st, w)), "): ", name,
    " (", host, "), ", why_line(host, st, w), ".", retried_text(n),
    safety(st, w)])

# what the model reads when the tool ran and answered an error
# (isError): the kind, the tool and its server, and its words
def tool_text(+name: String, +host: String, k: Cause, +said: String)
    -> String:
  String.concat(["mcp tool error (", kind_name(k), "): ", name, " (", host,
    ") answered: ", Bool.pick(String, String.is_empty(String.trim(said)),
      "(no text)", said_text(said)), "."])

# a JSON-RPC answer may come as an event stream (Accept lists both):
# then its last "data:" line is the answer
def rpc_text.keep(is_data: Bool, +l: String, +acc: Maybe<&2, String>)
    -> Maybe<&2, String>:
  match is_data:
    case True{}:
      Some{String.trim(String.drop(l, 5n))}
    case False{}:
      acc

def rpc_text.go(ls: List<&2, String>, +acc: Maybe<&2, String>)
    -> Maybe<&2, String>:
  match ls:
    case Nil{}:
      acc
    case Con{+l, rest}:
      rpc_text.go(rest, rpc_text.keep(String.starts_with(l, "data:"), l, acc))

def rpc_text.pick(m: Maybe<&2, String>, +body: String) -> String:
  match m:
    case None{}:
      body
    case Some{+d}:
      d

def rpc_text(+body: String) -> String:
  rpc_text.pick(rpc_text.go(String.split(body, Tx.newline()), None{}), body)

