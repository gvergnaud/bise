# runtime/mcp-pure.bend - the pure half of runtime/mcp.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/types.bend as T
import ../core/text.bend as Tx
import ../core/wire.bend as W
import ./provider-pure.bend as Pvp

# ---- the published alias: a valid identifier (vibe_sdk requires it) ----
# vibe's _normalize_name: anything outside [a-zA-Z0-9_-] flattens to
# "_", the edges strip, the result caps at 256 (an empty name becomes
# "unnamed"). tools.<alias>.<function> in the sandbox needs a clean
# identifier; the connector id (used for the gateway URL) stays raw.

def norm_chr.ok(ok: Bool, +c: U32) -> String:
  match ok:
    case True{}:
      SCon{Chr{c}, SNil{}}
    case False{}:
      "_"

def norm_chr.digit(+c: U32) -> Bool:
  Bool.and(U32.is_le(48, c), U32.is_le(c, 57))

def norm_chr.alpha(+c: U32) -> Bool:
  Bool.or(Bool.and(U32.is_le(65, c), U32.is_le(c, 90)),
    Bool.and(U32.is_le(97, c), U32.is_le(c, 122)))

def norm_chr.sym(+c: U32) -> Bool:
  Bool.or(U32.is_eq(c, 95), U32.is_eq(c, 45))

def norm_chr.ok2(d: Bool, a: Bool, s: Bool) -> Bool:
  Bool.or(d, Bool.or(a, s))

def norm_chr(+c: U32) -> String:
  norm_chr.ok(norm_chr.ok2(norm_chr.digit(c), norm_chr.alpha(c),
    norm_chr.sym(c)), c)

def norm_go(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{Chr{+c}, +t}:
      String.append(norm_chr(c), norm_go(t))

def trim_go.next(head_edge: Bool, tail_edge: Bool, +s: String) -> String:
  match head_edge:
    case True{}:
      String.drop(s, 1n)
    case False{}:
      match tail_edge:
        case True{}:
          String.take(s, Nat.sub(String.length(s), 1n))
        case False{}:
          s

# fuel-bound edge trim: one character per iteration, so 2*len fuel is
# always enough (+s: the edges and the recursion each read the string)
def trim_go(fuel: Nat, +s: String) -> String:
  match fuel:
    case 0n:
      s
    case 1n+k:
      trim_go(k, trim_go.next(
        Bool.or(String.starts_with(s, "_"), String.starts_with(s, "-")),
        Bool.or(String.ends_with(s, "_"), String.ends_with(s, "-")), s))

def norm_trim(+s: String) -> String:
  trim_go(Nat.add(String.length(s), String.length(s)), s)

def norm_pick(empty: Bool, +s: String) -> String:
  match empty:
    case True{}:
      "unnamed"
    case False{}:
      s

def norm_take.fits(fits: Bool, +s: String) -> String:
  match fits:
    case True{}:
      s
    case False{}:
      String.take(s, 256n)

def normalize_name(+s: String) -> String:
  norm_pick(String.is_empty(s),
    norm_take.fits(Nat.is_le(String.length(s), 256n),
      norm_trim(norm_go(s))))

# alias dedupe (vibe): colliding normalized aliases get _2, _3, ...
# (the codebase patterns: leaf helpers first, the recursive def calls
# itself by its own name, the recursive result rides as an argument)

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

def suffix.base(+base: String, +n: Nat) -> String:
  String.append(base, String.append("_", Nat.show(n)))

# fuel = len(seen) + 2 tries: one suffix per seen alias, plus slack;
# each try checks base_n against the seen list
def suffix_go(fuel: Nat, +base: String, +n: Nat,
    +seen: List<&2, String>, taken: Bool) -> String:
  match fuel:
    case 0n:
      suffix.base(base, n)
    case 1n+k:
      match taken:
        case True{}:
          suffix_go(k, base, Nat.add(n, 1n), seen,
            seen_has(seen, suffix.base(base, Nat.add(n, 1n))))
        case False{}:
          suffix.base(base, n)

def dedupe.pick(taken: Bool, +alias: String, +seen: List<&2, String>)
    -> String:
  match taken:
    case True{}:
      # the loop starts by trying <alias>_2 (not the alias itself)
      suffix_go(Nat.add(List.length(&2, String, seen), 2n), alias, 2n, seen,
        seen_has(seen, suffix.base(alias, 2n)))
    case False{}:
      alias

# +seen: both the collision check and the suffix search read the list
def dedupe(+alias: String, +seen: List<&2, String>) -> String:
  dedupe.pick(seen_has(seen, alias), alias, seen)



# the bootstrap endpoint, exactly as the vibe CLI calls it: the
# web_search builtin rides along as an MCP-callable connector, and
# auth-actionable (not yet connected) connectors are included (they
# stay out of the index: only ready connectors index their tools)
def bootstrap_url() -> String:
  "https://api.mistral.ai/v1/connectors/bootstrap?supports_mcp=true&include_auth_actionable_connectors=true&builtin_connectors=web_search"

# ---- the index line format, parsed ----
# line: "<cid> <cname> <tool> : #<desc>"

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

def index_tool.of(cid: String, cname: String, tool: String, +desc: String)
    -> T.ToolDef:
  T.ToolDef{pub_name(cname, tool), desc}

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
# body), or a JSON-RPC error the server answered
type Why is Data:
  WNet{e: Pvp.NetErr}
  WStatus{code: U32, body: String}
  WRpc{said: String}

# one try: the tool's answer, or where and why it stopped
type Try is Data:
  TOk{ok: Bool, out: String}
  TFail{stage: Stage, why: Why}

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
    case WRpc{+said}:
      session_words(said)

# a transport failure of tools/call: could the request have reached the
# server whole? not when the name, the connection, the handshake or the
# send failed
def net_sent(e: Pvp.NetErr) -> Bool:
  match e:
    case Pvp.NUrl{}:
      False{}
    case Pvp.NDns{}:
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
    case WRpc{+said}:
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
    case WRpc{+said}:
      session_words(said)

def retry(+st: Stage, +w: Why) -> Bool:
  Bool.and(Bool.not(reached(st, w)), transient(w, st))

# the verdict on a try: go again (with a fresh initialize) or not
def again(t: Try) -> Bool:
  match t:
    case TOk{_, _}:
      False{}
    case TFail{+st, +w}:
      retry(st, w)

# ---- the words ----

def said_max() -> Nat:
  200n

def clip(+s: String) -> String:
  Bool.pick(String, Nat.is_lt(said_max(), String.length(s)),
    String.append(String.take(s, said_max()), "…"), s)

def said_part(+said: String) -> String:
  Bool.pick(String, String.is_empty(said), "", String.append(": ", clip(said)))

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
        said_part(W.err_said(body))])
    case WRpc{+said}:
      String.concat([host, " answered an error",
        said_part(W.squeeze(said))])

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

# what the model reads when a call fails for good: the tool and its
# server, the phase, the cause, the retries, and whether it is safe to
# call again. n = the retries already made.
def fail_text(+name: String, +host: String, +st: Stage, +w: Why, +n: Nat)
    -> String:
  String.concat(["mcp call failed: ", name, " (", host, "), ",
    why_line(host, st, w), ".", retried_text(n), safety(st, w)])

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
