# runtime/tools-pure.bend - the pure half of runtime/tools.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../core/discovery.bend as D
import ../vendor/json.bend as J
import ../core/types.bend as T
import ../core/edit.bend as Ed
import ../core/read.bend as Rd

def patch_desc.read(m: Maybe<&2, String>) -> String:
  Maybe.default(&2, String, m, "Create, update or delete files with a patch in the Codex V4A format. The whole patch is validated before anything is written; every failure is loud.")

# pure: build the catalog from given descriptions
# self.sleep is NOT a model tool anymore: it is a programmatic tool,
# called inside a run_typescript program (await self.sleep(20)) -
# see exec_program.call.sleep in runtime/main.bend
def catalog_pure(+sd: String, +rd: String, +bd: String) -> List<&2, T.ToolDef>:
  Con{T.ToolDef{"search_tool_functions", sd},
   Con{T.ToolDef{"bash", bd},
    Con{T.ToolDef{"run_typescript", rd},
     Nil{}}}}

# ---- self.* harness requests: programmatic only (BISE-293) ----
# self.compact and self.reload are no top-level tools: a run_typescript
# program files them (await self.compact()), see
# exec_program.call.self_req in runtime/main.bend and the deferred scan
# in runtime/selftools.bend. They left the catalog on the user's call:
# they are harness controls, not model tools, and their dotted names
# were refused by OpenAI's tool-name pattern.

# pure helpers the laws pin: catalog size and positional extraction
def catalog_len(tools: List<&2, T.ToolDef>) -> Nat:
  List.length(&2, T.ToolDef, tools)

def tool_name_at(n: Nat, tools: List<&2, T.ToolDef>) -> String:
  match n:
    case 0n:
      match tools:
        case Nil{}:
          ""
        case Con{T.ToolDef{+name, _}, _}:
          name
    case 1n+k:
      match tools:
        case Nil{}:
          ""
        case Con{_, +rest}:
          tool_name_at(k, rest)

def tool_desc_at(n: Nat, tools: List<&2, T.ToolDef>) -> String:
  match n:
    case 0n:
      match tools:
        case Nil{}:
          ""
        case Con{T.ToolDef{_, +desc}, _}:
          desc
    case 1n+k:
      match tools:
        case Nil{}:
          ""
        case Con{_, +rest}:
          tool_desc_at(k, rest)

# the live catalog: the three parity tools, the skill loader, then ONE
# edit toolset (the user's decision, design §2.2: the OpenAI provider
# gets apply_patch, every other one Vibe's edit and write_file, never
# both) and Vibe's read_file beside it. The wire reverses the catalog
# (core/wire.bend add_tool pushes each TOOL line on the front), so a
# request lists the file tools FIRST, before bash (steering): read_file,
# edit, write_file, skill, run_typescript, bash, search_tool_functions.
# The OpenAI catalog is today's plus read_file (the prompt cache: one
# miss). bash's description names the edit tool that is on (its
# {edit_line}). catalog_short keeps the scripted scenarios
# byte-identical.
def edit_tools(patch: Bool, +pd: String) -> List<&2, T.ToolDef>:
  match patch:
    case True{}:
      Con{T.ToolDef{"apply_patch", pd},
        Con{T.ToolDef{"read_file", Rd.read_desc()}, Nil{}}}
    case False{}:
      Con{T.ToolDef{"write_file", Ed.write_desc()},
        Con{T.ToolDef{"edit", Ed.edit_desc()},
          Con{T.ToolDef{"read_file", Rd.read_desc()}, Nil{}}}}

def bash_for(+bd: String, +patch: Bool) -> String:
  Ed.replace(bd, "{edit_line}", Ed.bash_line(patch), False{})

def catalog_live(+sd: String, +rd: String, +bd: String, +kd: String,
    +pd: String, +patch: Bool) -> List<&2, T.ToolDef>:
  List.append(&2, T.ToolDef, catalog_pure(sd, rd, bash_for(bd, patch)),
    Con{T.ToolDef{"skill", kd}, edit_tools(patch, pd)})

# which edit toolset a catalog carries: Some(True) apply_patch,
# Some(False) edit + write_file, None: none (the scripted catalog)
def has_tool(tools: List<&2, T.ToolDef>, +name: String) -> Bool:
  match tools:
    case Nil{}:
      False{}
    case Con{T.ToolDef{+n, _}, rest}:
      Bool.or(String.eq(n, name), has_tool(rest, name))

def toolset.edit(has_edit: Bool) -> Maybe<&2, Bool>:
  match has_edit:
    case True{}:
      Some{False{}}
    case False{}:
      None{}

def toolset.pick(has_patch: Bool, +tools: List<&2, T.ToolDef>) -> Maybe<&2, Bool>:
  match has_patch:
    case True{}:
      Some{True{}}
    case False{}:
      toolset.edit(has_tool(tools, "edit"))

def toolset(+tools: List<&2, T.ToolDef>) -> Maybe<&2, Bool>:
  toolset.pick(has_tool(tools, "apply_patch"), tools)

# ---- the unified search: the vibe_sdk SearchRequest schema ----
# The live model sends {"mode": ..., "query": ...} (the exact vibe_sdk
# schema pinned by search_schema in core/api.bend): best_match returns up
# to 20 ranked exact function names, details returns declarations for
# exact names, all_connector_capabilities lists connector inventories.
# The scripted suite keeps the flat 'best_match: query' syntax below.

def SR_USAGE() -> String:
  "Usage: mode=best_match with query; mode=details with functions=[names]; mode=all_connector_capabilities with connectors=[names]"

# strings of a JSON array field
def jstrs_of(vals: List<&2, J.Json>, +acc: List<&2, String>) -> List<&2, String>:
  match vals:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+v, rest}:
      match v:
        case J.JStr{+s}:
          jstrs_of(rest, Con{s, acc})
        case _:
          jstrs_of(rest, acc)

def jstrs.arr(v: J.Json) -> List<&2, String>:
  match v:
    case J.JArr{+vals}:
      jstrs_of(vals, Nil{})
    case _:
      Nil{}

def jstrs.found(m: Maybe<&2, J.Json>) -> List<&2, String>:
  match m:
    case Some{+v}:
      jstrs.arr(v)
    case None{}:
      Nil{}

def jstrs(j: J.Json, +key: String) -> List<&2, String>:
  jstrs.found(J.get(j, key))

# ---- tool field accessors ----

def tool_name(td: T.ToolDef) -> String:
  match td:
    case T.ToolDef{+name, _}:
      name

def tool_desc(td: T.ToolDef) -> String:
  match td:
    case T.ToolDef{_, +desc}:
      desc

def word_hit(+w: String, +td: T.ToolDef) -> Bool:
  Bool.or(String.contains(tool_name(td), w), String.contains(tool_desc(td), w))

def bool_nat(b: Bool) -> Nat:
  match b:
    case True{}:
      1n
    case False{}:
      0n

# a tool's score = the number of query words it matches anywhere
def word_score(words: List<&2, String>, +td: T.ToolDef) -> Nat:
  match words:
    case Nil{}:
      0n
    case Con{+w, rest}:
      Nat.add(bool_nat(word_hit(w, td)), word_score(rest, td))

def keep_word(keep: Bool, +w: String, +acc: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      Con{w, acc}
    case False{}:
      acc

def words_of.clean(ws: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+w, rest}:
      words_of.clean(rest, keep_word(Bool.not(String.is_empty(w)), w, acc))

def words_of(+q: String) -> List<&2, String>:
  words_of.clean(String.split(String.trim(q), ' '), Nil{})

def is_nil_strs(ws: List<&2, String>) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case Con{_, _}:
      False{}

# does any connector match the tool name?
def conn_hit(conns: List<&2, String>, +td: T.ToolDef) -> Bool:
  match conns:
    case Nil{}:
      False{}
    case Con{+c, rest}:
      Bool.or(String.starts_with(tool_name(td), String.append(c, ".")),
        conn_hit(rest, td))

# tools of the given connectors: an EMPTY connector list keeps
# everything (no filter); a non-empty list keeps only matches
def conn_keep(conns: List<&2, String>, +td: T.ToolDef) -> Bool:
  match conns:
    case Nil{}:
      True{}
    case Con{_, _}:
      conn_hit(conns, td)

def tools_of(tools: List<&2, T.ToolDef>, +conns: List<&2, String>,
    +acc: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match tools:
    case Nil{}:
      List.reverse(&2, T.ToolDef, acc)
    case Con{+t, rest}:
      tools_of(rest, conns, D.search.put(conn_keep(conns, t), t, acc))

# tools scoring exactly s, in catalog order (the put/reverse idiom)
def rank_at(tools: List<&2, T.ToolDef>, +words: List<&2, String>, +s: Nat,
    +level: List<&2, T.ToolDef>) -> List<&2, T.ToolDef>:
  match tools:
    case Nil{}:
      List.reverse(&2, T.ToolDef, level)
    case Con{+t, rest}:
      rank_at(rest, words, s,
        D.search.put(Nat.is_eq(word_score(words, t), s), t, level))

# descending scores [n .. 1] driven by the words list (one per word)
def scores_go(words: List<&2, String>, +cur: Nat,
    +acc: List<&2, Nat>) -> List<&2, Nat>:
  match words:
    case Nil{}:
      acc
    case Con{_, rest}:
      scores_go(rest, Nat.add(cur, 1n), Con{cur, acc})

def scores_of(words: List<&2, String>) -> List<&2, Nat>:
  scores_go(words, 1n, Nil{})

# ranked by descending score (equal scores keep catalog order)
def rank_desc(scores: List<&2, Nat>, +words: List<&2, String>,
    +tools: List<&2, T.ToolDef>, +acc: List<&2, T.ToolDef>)
    -> List<&2, T.ToolDef>:
  match scores:
    case Nil{}:
      acc
    case Con{+s, rest}:
      rank_desc(rest, words, tools,
        List.append(&2, T.ToolDef, acc, rank_at(tools, words, s, Nil{})))

def ranked(+words: List<&2, String>, +tools: List<&2, T.ToolDef>)
    -> List<&2, T.ToolDef>:
  rank_desc(scores_of(words), words, tools, Nil{})

# ---- renderers ----

def names_of(ts: List<&2, T.ToolDef>, +acc: List<&2, String>) -> List<&2, String>:
  match ts:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+t, rest}:
      names_of(rest, Con{tool_name(t), acc})

def show_names(ts: List<&2, T.ToolDef>) -> String:
  String.join(names_of(ts, Nil{}), "\n")

def more_flag.pick(too_many: Bool) -> String:
  match too_many:
    case True{}:
      "moreCandidatesAvailable: true\n"
    case False{}:
      ""

def more_flag(+count: Nat) -> String:
  more_flag.pick(Nat.is_le(21n, count))

# ---- the structured modes ----

def is_nil_defs(ts: List<&2, T.ToolDef>) -> Bool:
  match ts:
    case Nil{}:
      True{}
    case Con{_, _}:
      False{}

def sr_best.none(none: Bool, +words: List<&2, String>,
    +tools: List<&2, T.ToolDef>) -> String:
  match none:
    case True{}:
      "No matching tool candidates for the query."
    case False{}:
      String.append(more_flag(catalog_len(ranked(words, tools))),
        show_names(List.take(&2, T.ToolDef, ranked(words, tools), 20n)))

def sr_best.has_any(+words: List<&2, String>, +tools: List<&2, T.ToolDef>) -> String:
  sr_best.none(is_nil_defs(ranked(words, tools)), words, tools)

def sr_best.pick(empty: Bool, +query: String, +j: J.Json,
    tools: List<&2, T.ToolDef>) -> String:
  match empty:
    case True{}:
      SR_USAGE()
    case False{}:
      sr_best.has_any(words_of(query),
        tools_of(tools, jstrs(j, "connectors"), Nil{}))

def sr_best.go(+query: String, +j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_best.pick(String.is_empty(query), query, j, tools)

def sr_best(+j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_best.go(J.get_str_or(j, "query", ""), j, tools)

# ---- details-mode missing-name reporting (the vibe_sdk contract:
# a reply never comes back silently empty; unfound names are listed) ----

def has_name(+name: String, tools: List<&2, T.ToolDef>) -> Bool:
  match tools:
    case Nil{}:
      False{}
    case Con{+t, rest}:
      Bool.or(String.eq(tool_name(t), name), has_name(name, rest))

def keep_str(keep: Bool, +s: String, +rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      Con{s, rest}
    case False{}:
      rest

# requested names absent from the catalog, in request order
def missing_names(names: List<&2, String>, +tools: List<&2, T.ToolDef>)
    -> List<&2, String>:
  match names:
    case Nil{}:
      Nil{}
    case Con{+n, rest}:
      keep_str(Bool.not(has_name(n, tools)), n, missing_names(rest, tools))

def missing_lines(names: List<&2, String>, +acc: List<&2, String>)
    -> List<&2, String>:
  match names:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+n, rest}:
      missing_lines(rest,
        Con{String.append("- No exact tool function found for: ", n), acc})

def show_missing.some(empty: Bool, +names: List<&2, String>) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      String.append("\n# Missing Exact Names\n",
        String.join(missing_lines(names, Nil{}), "\n"))

def show_missing(+names: List<&2, String>) -> String:
  show_missing.some(is_nil_strs(names), names)

# the details reply: declarations for the names that resolve, the
# vibe_sdk empty message when none do, and the missing list either way
def det_reply.none(none: Bool, +names: List<&2, String>,
    +found: List<&2, T.ToolDef>, +tools: List<&2, T.ToolDef>) -> String:
  match none:
    case True{}:
      String.append(
        "No exact tool function details loaded. Call best_match/all_connector_capabilities first, then request exact returned function names.",
        show_missing(missing_names(names, tools)))
    case False{}:
      String.append(D.show_tools(found),
        show_missing(missing_names(names, tools)))

def det_reply(+names: List<&2, String>, +tools: List<&2, T.ToolDef>) -> String:
  det_reply.none(is_nil_defs(D.search_details(names, tools)), names,
    D.search_details(names, tools), tools)

def sr_det(names: List<&2, String>, tools: List<&2, T.ToolDef>) -> String:
  det_reply(names, tools)

def sr_det.pick(empty: Bool, names: List<&2, String>, tools: List<&2, T.ToolDef>) -> String:
  match empty:
    case True{}:
      "Details mode requires at least one exact function name returned by best_match or all_connector_capabilities mode."
    case False{}:
      sr_det(names, tools)

def sr_det.entry(+j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_det.pick(is_nil_strs(List.append(&2, String, jstrs(j, "functions"),
    jstrs(j, "functionNames"))),
    List.append(&2, String, jstrs(j, "functions"), jstrs(j, "functionNames")),
    tools)

def sr_caps(conns: List<&2, String>, tools: List<&2, T.ToolDef>) -> String:
  D.show_tools(tools_of(tools, conns, Nil{}))

def sr_caps.pick(empty: Bool, conns: List<&2, String>, tools: List<&2, T.ToolDef>) -> String:
  match empty:
    case True{}:
      "all_connector_capabilities requires at least one connector name."
    case False{}:
      sr_caps(conns, tools)

def sr_caps.entry(+j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_caps.pick(is_nil_strs(jstrs(j, "connectors")), jstrs(j, "connectors"), tools)

def sr_mode.caps(is_caps: Bool, +j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  match is_caps:
    case True{}:
      sr_caps.entry(j, tools)
    case False{}:
      SR_USAGE()

def sr_mode.det(is_det: Bool, +mode: String, +j: J.Json,
    tools: List<&2, T.ToolDef>) -> String:
  match is_det:
    case True{}:
      sr_det.entry(j, tools)
    case False{}:
      sr_mode.caps(String.eq(mode, "all_connector_capabilities"), j, tools)

def sr_mode.best(is_best: Bool, +mode: String, +j: J.Json,
    tools: List<&2, T.ToolDef>) -> String:
  match is_best:
    case True{}:
      sr_best(j, tools)
    case False{}:
      sr_mode.det(String.eq(mode, "details"), mode, j, tools)

def sr_dispatch(+j: J.Json, tools: List<&2, T.ToolDef>) -> String:
  sr_mode.best(String.eq(J.get_str_or(j, "mode", "best_match"), "best_match"),
    J.get_str_or(j, "mode", "best_match"), j, tools)

# ---- the flat syntax of the scripted suite ----

def flat_mode(details: Bool, +args: String, tools: List<&2, T.ToolDef>) -> String:
  match details:
    case True{}:
      det_reply(String.split(String.trim(String.drop(args, 8n)), ' '), tools)
    case False{}:
      D.show_tools(D.best_match(
        String.trim(String.drop(args, 11n)), tools, 5n))

def flat_pick(known: Bool, +args: String, tools: List<&2, T.ToolDef>) -> String:
  match known:
    case False{}:
      SR_USAGE()
    case True{}:
      flat_mode(String.starts_with(args, "details:"), args, tools)

def flat_reply(+args: String, tools: List<&2, T.ToolDef>) -> String:
  flat_pick(Bool.or(String.starts_with(args, "details:"),
    String.starts_with(args, "best_match:")), args, tools)

def sr_parse(r: Result<&2, &2, String, J.Json>, +args: String,
    tools: List<&2, T.ToolDef>) -> String:
  match r:
    case Done{+j}:
      sr_dispatch(j, tools)
    case Fail{_}:
      flat_reply(args, tools)

def search_reply(+args: String, tools: List<&2, T.ToolDef>) -> String:
  sr_parse(J.parse(String.trim(args)), args, tools)

# ---- the tool execution result ----

# the unified tool-execution result: ok carries the output, failure
# carries the reason (exit status, stderr) so the model can see it
type ExecRes is Data:
  Ex{ok: Bool, out: String}

# a giant output (a catted binary, a full log) must never flow into the
# history and blow up the next provider request. The limit is
# parameterized so the law can prove the logic at a small scale (a
# 20001-char literal would overflow the proof checker's stack).
def cap_out_limit() -> Nat:
  20000n

def cap_out_at.pick(fits: Bool, +out: String, +n: Nat) -> String:
  match fits:
    case True{}:
      out
    case False{}:
      String.append(String.take(out, n),
        String.append("  [output truncated at 20000 chars; ",
          String.append(Nat.show(Nat.sub(String.length(out), n)),
            " more chars discarded]")))

def cap_out_at(+n: Nat, +out: String) -> String:
  cap_out_at.pick(Nat.is_le(String.length(out), n), out, n)

def cap_out(+out: String) -> String:
  cap_out_at(cap_out_limit(), out)
