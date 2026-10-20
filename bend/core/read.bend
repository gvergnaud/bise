# core/read.bend - the pure core of the `read_file` tool, copied from
# Vibe 2.25.8 (vibe/core/tools/builtins/read_file.py,
# prompts/read_file.md): the same name, schema, description, errors,
# warnings, line numbering and paging, byte for byte (issue #21: the
# edit description names `read_file`, and the tool did not exist, so a
# model obeying it landed in the remote-MCP branch and stalled).
# runtime/read-tool.bend only moves bytes around this module.
#
# Where it leaves Vibe on purpose, said plainly:
# - the cap is 20000 chars, not Vibe's 50 KiB: every tool result is
#   capped there (runtime/tools-pure.bend cap_out), so a read that
#   passed a 50 KiB check would arrive cut by the wire cap anyway — the
#   read's own cap is the wire's, so a result that passes is delivered
#   whole. The error still names the way out (offset and limit).
# - a file that is not UTF-8 is refused (the repo's own stance, like
#   edit's; Vibe's fallback chain would return mangled text).
# - offset < 1 reads from 1 and limit < 1 the default, instead of a
#   pydantic validation error: a small model's malformed call still
#   reads something (the skills loader's tolerance).
# - the result says file, the window and the total, then the content
#   (the token call of 1b061c1a), not Vibe's pydantic field dump: the
#   content is the payload.
# - a read the disk refuses says Vibe's "Error reading <path>: <why>"
#   with the shell's words (no errno text here).
import Base
import ./text.bend as Tx
import ./edit.bend as Ed
import ../vendor/json.bend as J

# ---- the strings (Vibe's, byte for byte) ----

def read_desc() -> String:
  "Read a file. Use absolute paths. For large files, use `offset` and `limit` to page through sections. Use `grep` to find specific content instead of reading sequentially. Do not read binary or model weight files (.bin, .safetensors, .pt, .gguf)."

def err_empty_path() -> String:
  "file_path cannot be empty"

def err_not_found(+path: String) -> String:
  String.append("File not found at: ", path)

def err_is_dir(+path: String) -> String:
  String.append("Path is a directory, not a file: ", path)

def err_not_text(+path: String, +byte: Nat) -> String:
  String.concat(["Cannot read ", path, ": file is not valid text (utf-8, byte ",
    Nat.show(byte), ")"])

def err_reading(+path: String, +why: String) -> String:
  String.concat(["Error reading ", path, ": ", why])

# a read the char cap refuses (Vibe's error shape, bise's cap)
def err_too_big(+n: Nat, +max: Nat) -> String:
  String.concat(["Output (", Nat.show(n), " chars) exceeds maximum allowed size (",
    Nat.show(max), " chars). Use offset and limit to read a smaller portion of the file."])

# ---- the warnings (Vibe's, wrapped in its tag) ----

def warn_tag(+msg: String) -> String:
  String.concat(["<vibe_warning>", msg, "</vibe_warning>"])

def warn_empty() -> String:
  warn_tag("Warning: the file exists but the contents are empty.")

def warn_short(+start: Nat, +total: Nat) -> String:
  warn_tag(String.concat(["Warning: the file exists but is shorter than the provided offset (",
    Nat.show(start), "). The file has ", Nat.show(total), " lines."]))

# ---- the schema (Vibe's pydantic fields) ----

def DEFAULT_LIMIT() -> Nat:
  2000n

def read_schema() -> J.Json:
  J.obj(Con{J.kv("type", J.str("object")),
    Con{J.kv("properties", J.obj(
      Con{Ed.str_prop("file_path", "The absolute path to the file to read"),
       Con{J.kv("offset", J.obj(Con{J.kv("type", J.str("integer")),
         Con{J.kv("minimum", J.num(1n)),
          Con{J.kv("description", J.str("The line number to start reading from (1-indexed)")),
           Nil{}}}})),
        Con{J.kv("limit", J.obj(Con{J.kv("type", J.str("integer")),
          Con{J.kv("default", J.num(DEFAULT_LIMIT())),
           Con{J.kv("exclusiveMinimum", J.num(0n)),
            Con{J.kv("description", J.str("The maximum number of lines to read")),
             Nil{}}}}})),
          Nil{}}}})),
     Con{J.kv("required", J.arr(Con{J.str("file_path"), Nil{}})), Nil{}}}})

# ---- the args ----

type ReadArgs is Data:
  ReadArgs{path: String, offset: Maybe<&2, Nat>, limit: Nat}

# the path of a JSON arg: file_path, else path, else arg (the form a
# run_typescript program's call takes); a bare JSON string is the path
# itself (a small model's one-argument call)
def path_of.pick2(+p: String, +q: String, +j: J.Json) -> String:
  Bool.pick(String, String.is_empty(p), q, p)

def path_of.pick1(+p: String, +j: J.Json) -> String:
  path_of.pick2(p, J.get_str_or(j, "arg", ""), j)

def path_of.obj(+j: J.Json) -> String:
  path_of.pick1(J.get_str_or(j, "file_path", ""), j)

def path_of.str_or(+j: J.Json, +from_obj: String) -> String:
  match j:
    case J.JStr{+s}:
      s
    case _:
      from_obj

def path_of(+j: J.Json) -> String:
  path_of.str_or(j, path_of.obj(j))

# offset 0 is no offset, limit 0 the default (a malformed window still
# reads something, like the skills loader's bare-name tolerance)
def limit_of.pick(zero: Bool, +n: Nat) -> Nat:
  match zero:
    case True{}:
      DEFAULT_LIMIT()
    case False{}:
      n

def limit_of(m: Maybe<&2, Nat>) -> Nat:
  match m:
    case Some{+n}:
      limit_of.pick(Nat.is_eq(n, 0n), n)
    case None{}:
      DEFAULT_LIMIT()

def offset_of.pick(zero: Bool, +n: Nat) -> Maybe<&2, Nat>:
  match zero:
    case True{}:
      None{}
    case False{}:
      Some{n}

def offset_of(m: Maybe<&2, Nat>) -> Maybe<&2, Nat>:
  match m:
    case Some{+n}:
      offset_of.pick(Nat.is_eq(n, 0n), n)
    case None{}:
      None{}

def read_args.json(+j: J.Json) -> ReadArgs:
  ReadArgs{path_of(j), offset_of(J.get_num(j, "offset")),
    limit_of(J.get_num(j, "limit"))}

# tolerant: JSON, or the bare path a program or a small model sends
def read_args.of(r: Result<&2, &2, String, J.Json>, +args: String) -> ReadArgs:
  match r:
    case Done{+j}:
      read_args.json(j)
    case Fail{_}:
      ReadArgs{String.trim(args), None{}, DEFAULT_LIMIT()}

def read_args(+args: String) -> ReadArgs:
  read_args.of(J.parse(String.trim(args)), args)

def read_path(a: ReadArgs) -> String:
  match a:
    case ReadArgs{+p, _, _}:
      p

def check_args(+path: String) -> Maybe<&2, String>:
  Bool.pick(Maybe<&2, String>, String.is_empty(String.trim(path)),
    Some{err_empty_path()}, None{})

# the start line: 1 without an offset
# the start line: 1 without an offset
def start_of.m(m: Maybe<&2, Nat>) -> Nat:
  match m:
    case None{}:
      1n
    case Some{+n}:
      n

def start_of(a: ReadArgs) -> Nat:
  match a:
    case ReadArgs{_, +o, _}:
      start_of.m(o)

# the call line the feed shows (Vibe's display): the path, then the
# window when one was asked
def ann_of.suffix(+offset: Maybe<&2, Nat>, +limit: Nat) -> String:
  match offset:
    case Some{+n}:
      String.concat([" from line ", Nat.show(n), " limit ", Nat.show(limit), " lines"])
    case None{}:
      Bool.pick(String, Nat.is_eq(limit, DEFAULT_LIMIT()), "",
        String.concat([" limit ", Nat.show(limit), " lines"]))

def ann_args.a(a: ReadArgs) -> String:
  match a:
    case ReadArgs{+p, +o, +l}:
      String.append(p, ann_of.suffix(o, l))

def ann_args(+args: String) -> String:
  ann_args.a(read_args(args))

# ---- the read itself, on the whole text ----

# the file's lines: String.lines keeps the empty after a trailing
# newline ("a\n" -> ["a", ""]), a file's lines end at its last one
def lines_of.drop(+ls: List<&2, String>) -> List<&2, String>:
  List.reverse(&2, String, List.tail(&2, String, List.reverse(&2, String, ls)))

def lines_of.ends(ends_nl: Bool, +ls: List<&2, String>) -> List<&2, String>:
  match ends_nl:
    case True{}:
      lines_of.drop(ls)
    case False{}:
      ls

def lines_of(+text: String) -> List<&2, String>:
  lines_of.ends(String.ends_with(text, "
"), String.lines(text))

# lines [start, start+limit), 1-indexed
def window(ls: List<&2, String>, +start: Nat, +limit: Nat) -> List<&2, String>:
  List.take(&2, String, List.drop(&2, String, ls, Nat.sub(start, 1n)), limit)

# "{n:>9}→{line}" (Vibe's _add_line_numbers): the number right-justed
# to 9, then U+2192
def pad9(+s: String) -> String:
  +n = String.length(s)
  Bool.pick(String, Nat.is_ge(n, 9n), s,
    String.append(String.take("         ", Nat.sub(9n, n)), s))

def numbered_line(+n: Nat, +line: String) -> String:
  String.append(pad9(Nat.show(n)), String.append("→", line))

def numbered.go(+ls: List<&2, String>, +start: Nat, +acc: List<&2, String>)
    -> List<&2, String>:
  match ls:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+l, rest}:
      numbered.go(rest, Nat.add(start, 1n), Con{numbered_line(start, l), acc})

def numbered(+start: Nat, +ls: List<&2, String>) -> String:
  String.join(numbered.go(ls, start, Nil{}), "
")

# ---- the result ----

# the cap every tool result lives under (cap_out), so a read that
# passes is delivered whole
def result_cap() -> Nat:
  20000n

# the window's header line: "lines: 0 of N" with nothing shown, else
# "lines: <first>-<last> of N"
def lines_head.none(none: Bool, +start: Nat, +n: Nat, +total: Nat) -> String:
  match none:
    case True{}:
      String.concat(["lines: 0 of ", Nat.show(total)])
    case False{}:
      String.concat(["lines: ", Nat.show(start), "-",
        Nat.show(Nat.sub(Nat.add(start, n), 1n)), " of ", Nat.show(total)])

def lines_head(+start: Nat, +n: Nat, +total: Nat) -> String:
  lines_head.none(Nat.is_eq(n, 0n), start, n, total)

# ok: the header (file, the window, the total), the content under it
def result(+path: String, +start: Nat, +n: Nat, +total: Nat, +what: String) -> String:
  String.concat(["file: ", path, "
", lines_head(start, n, total), "
", "
", what])

# the window's result: the lines shown and their numbered text
def read_of.window(+path: String, +start: Nat, +limit: Nat, +total: Nat,
    +ls: List<&2, String>) -> String:
  +w = window(ls, start, limit)
  result(path, start, List.length(&2, String, w), total, numbered(start, w))

# a non-empty file's read: the short-file warning when the offset is
# past its end, else the window
def read_of.win(+path: String, +start: Nat, +limit: Nat, +total: Nat,
    +ls: List<&2, String>) -> String:
  Bool.pick(String, Nat.is_lt(total, start),
    result(path, start, 0n, total, warn_short(start, total)),
    read_of.window(path, start, limit, total, ls))

# the whole read of `text` as the model reads it: the header, then the
# numbered window or Vibe's warnings (an empty file; an offset past
# its end)
def read_of.body.empty(empty: Bool, +path: String, +start: Nat, +limit: Nat,
    +text: String) -> String:
  match empty:
    case True{}:
      result(path, start, 0n, Ed.text_lines(text), warn_empty())
    case False{}:
      +total = Ed.text_lines(text)
      read_of.win(path, start, limit, total, lines_of(text))

def read_of.body(+path: String, +start: Nat, +limit: Nat,
    +text: String) -> String:
  read_of.body.empty(String.is_empty(text), path, start, limit, text)

# the read as the model sees it: ok with the result, failed with the
# cap error (Vibe raises ToolError there)
type ReadOut is Data:
  ROk{text: String}
  RFail{why: String}

# past the cap: Vibe's error shape naming the size and the way out
def read_out.pick(cap_ok: Bool, +r: String, +n: Nat) -> ReadOut:
  match cap_ok:
    case True{}:
      ROk{r}
    case False{}:
      RFail{err_too_big(n, result_cap())}

def read_out(+text: String, +path: String, +start: Nat, +limit: Nat) -> ReadOut:
  +r = read_of.body(path, start, limit, text)
  read_out.pick(Nat.is_le(String.length(r), result_cap()), r, String.length(r))
