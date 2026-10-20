# core/edit.bend - the pure core of Vibe's `edit` and `write_file`
# tools, copied from Vibe 2.25.8 (vibe/core/tools/builtins/edit.py,
# write_file.py, prompts/edit.md, prompts/write_file.md): the same
# names, schemas, descriptions and errors, byte for byte (design
# docs/approvals-design.md §2.2). runtime/edit-tool.bend only moves
# bytes around this module.
#
# Where it leaves Vibe on purpose (tokens, Gabriel's call): no result
# or error echoes old_string, new_string or content, which the model's
# own call carries. The results say the file, the replacements and the
# lines (edit_result, write_result); not found and several matches end
# with lines in place of Vibe's "String: <old_string>". The clients
# draw the diff from the call's args (edit_view, write_view), never
# from the result.
#
# One edit toolset per request (the user's decision): the OpenAI
# provider gets `apply_patch`, every other provider `edit` and
# `write_file`, never both. The prompt line, the bash description's
# edit sentence and the deny-once hint of auto name the tools that are
# on (edit_tool_name, prompt_line, bash_line, deny_once_hint).
#
# Vibe reads a file into text with its newlines normalized to "\n" and
# writes it back with the file's own newline style (utils/io.py
# read_safe / atomic_replace): nl_style and normalize / restore below.
# What differs from Vibe, said plainly: a file that is not UTF-8 is
# refused (Vibe tries the locale's codec and charset_normalizer first);
# a file whose newlines are mostly lone "\r" is written back with "\n".
import Base
import ./text.bend as Tx
import ../vendor/json.bend as J

# ---- the error and result strings (Vibe's, byte for byte) ----

def err_empty_path() -> String:
  "File path cannot be empty"

def err_empty_old() -> String:
  "old_string cannot be empty. Use write_file to create new files."

def err_same() -> String:
  "No changes to make — old_string and new_string are identical"

def err_missing(+path: String) -> String:
  String.append("File does not exist: ", path)

def err_not_file(+path: String) -> String:
  String.append("Path is not a file: ", path)

# err_not_found, err_many and edit_result say lines: below, with the
# match walk (the edit itself)

# Python's UnicodeDecodeError words: the codec and the first bad byte
def err_not_text(+path: String, +byte: Nat) -> String:
  String.concat(["Cannot edit ", path, ": file is not valid text (utf-8, byte ",
    Nat.show(byte), ")"])

def err_permission(+path: String) -> String:
  String.append("Permission denied accessing file: ", path)

# a write that failed (Vibe: Python's OSError text; here the shell's)
def err_os(+path: String, +why: String) -> String:
  String.concat(["OS error accessing ", path, ": ", why])

def ok_one() -> String:
  "The file has been updated successfully."

def ok_all() -> String:
  "The file has been updated. All occurrences were successfully replaced"

def ok_msg(all: Bool) -> String:
  match all:
    case True{}:
      ok_all()
    case False{}:
      ok_one()

def werr_empty_path() -> String:
  "Path cannot be empty"

def WRITE_MAX() -> Nat:
  64000n

def werr_too_big() -> String:
  "Content exceeds 64000 bytes limit"

def werr_exists(+path: String) -> String:
  String.concat(["File '", path, "' already exists. Use edit to modify it."])

def werr_write(+path: String, +why: String) -> String:
  String.concat(["Error writing ", path, ": ", why])

# `lines`: text_lines of the content, never the content
def write_result(+path: String, +bytes: Nat, +lines: Nat) -> String:
  String.concat(["file_path: ", path, "\nbytes_written: ", Nat.show(bytes),
    "\nlines: ", Nat.show(lines)])

# ---- the descriptions (Vibe's prompts/*.md) ----

def edit_desc() -> String:
  "Exact string replacement in a file. You must `read_file` first. When editing text from `read_file` output, never include any part of the line-number prefix in `old_string` or `new_string`. If `old_string` is not found or matches multiple locations, provide more context to make it unique, or use `replace_all`. If an edit fails, re-read the file before retrying."

def write_desc() -> String:
  "Create a new file. Errors if the file already exists — use `edit` to modify existing files. Prefer editing existing files over creating new ones. Do not proactively create documentation or README files."

# ---- the schemas (Vibe's pydantic models, as the API sees them) ----

def str_prop(+name: String, +desc: String) -> Sigma<&2, &2, String, _ => J.Json>:
  J.kv(name, J.obj(Con{J.kv("type", J.str("string")),
    Con{J.kv("description", J.str(desc)), Nil{}}}))

def edit_schema() -> J.Json:
  J.obj(Con{J.kv("type", J.str("object")),
    Con{J.kv("properties", J.obj(
      Con{str_prop("file_path", "The absolute path to the file to modify"),
       Con{str_prop("old_string", "The text to replace"),
        Con{str_prop("new_string", "The text to replace it with (must be different from old_string)"),
         Con{J.kv("replace_all", J.obj(Con{J.kv("type", J.str("boolean")),
           Con{J.kv("default", J.JBool{False{}}),
            Con{J.kv("description", J.str("Replace all occurrences of old_string (default false)")),
             Nil{}}}})),
          Nil{}}}}})),
     Con{J.kv("required", J.arr([J.str("file_path"), J.str("old_string"),
       J.str("new_string")])), Nil{}}}})

def write_schema() -> J.Json:
  J.obj(Con{J.kv("type", J.str("object")),
    Con{J.kv("properties", J.obj(
      Con{str_prop("file_path", "The absolute path to the file to write (must be absolute, not relative)"),
       Con{str_prop("content", "The content to write to the file"), Nil{}}})),
     Con{J.kv("required", J.arr([J.str("file_path"), J.str("content")])),
       Nil{}}}})

# ---- one toolset per request, and the lines that name it ----

# the OpenAI provider (its id before the first '/' of the model id)
# gets apply_patch; every other provider, edit and write_file
def is_patch_provider(+prov: String) -> Bool:
  String.eq(prov, "openai")

def provider_of(+model: String) -> String:
  String.take(model, Maybe.default(&2, Nat, Tx.find(model, "/", 0n),
    String.length(model)))

def edit_tool_name(patch: Bool) -> String:
  match patch:
    case True{}:
      "apply_patch"
    case False{}:
      "edit"

# the system prompt line (design §2.2, steering)
def prompt_line(patch: Bool) -> String:
  match patch:
    case True{}:
      "Read files with `read_file`, edit and create them with `apply_patch`. Don't edit files through bash scripts (python or perl, `sed` on many files): in auto they need the user, and tool edits show as diffs."
    case False{}:
      "Read files with `read_file`, edit and create them with `edit` and `write_file`. Don't edit files through bash scripts (python or perl, `sed` on many files): in auto they need the user, and tool edits show as diffs."

# the bash description's sentence
def bash_line(patch: Bool) -> String:
  String.concat(["Not for editing files: use `", edit_tool_name(patch), "`."])

# auto's deny-once hint for a bash edit the parser cannot read (1a/1b)
def deny_once_hint(patch: Bool) -> String:
  String.concat(["auto: this bash call needs the user. Use `",
    edit_tool_name(patch),
    "`: it runs without asking. If bash is really needed, repeat the call and the user will be asked."])

# ---- substring count and replace (Python's str.count / str.replace:
# non-overlapping, left to right); fuel = the length, each hop eats at
# least one char. `pat` is never empty here (edit refuses it first).

def count.go(fuel: Nat, here: Bool, +s: String, +pat: String, +n: Nat) -> Nat:
  match fuel:
    case 0n:
      n
    case 1n+k:
      match here:
        case True{}:
          count.go(k,
            String.starts_with(String.drop(s, String.length(pat)), pat),
            String.drop(s, String.length(pat)), pat, Nat.add(n, 1n))
        case False{}:
          match s:
            case SNil{}:
              n
            case SCon{_, +t}:
              count.go(k, String.starts_with(t, pat), t, pat, n)

def count(+s: String, +pat: String) -> Nat:
  count.go(String.length(s),
    Bool.and(Bool.not(String.is_empty(pat)), String.starts_with(s, pat)),
    s, pat, 0n)

# `on`: replacing is still on (False after the first one without
# replace_all), so `here` is only True while it is on
def replace.go(fuel: Nat, here: Bool, +on: Bool, +all: Bool, +s: String,
    +pat: String, +new: String) -> String:
  match fuel:
    case 0n:
      s
    case 1n+k:
      match here:
        case True{}:
          String.append(new, replace.go(k,
            Bool.and(all, String.starts_with(String.drop(s, String.length(pat)), pat)),
            all, all, String.drop(s, String.length(pat)), pat, new))
        case False{}:
          match s:
            case SNil{}:
              SNil{}
            case SCon{h, +t}:
              SCon{h, replace.go(k, Bool.and(on, String.starts_with(t, pat)),
                on, all, t, pat, new)}

def replace(+s: String, +pat: String, +new: String, +all: Bool) -> String:
  replace.go(String.length(s),
    Bool.and(Bool.not(String.is_empty(pat)), String.starts_with(s, pat)),
    True{}, all, s, pat, new)

# ---- newlines: read as "\n", written back in the file's style ----

def CR() -> String:
  SCon{Char.from_u32(13), SNil{}}

def CRLF() -> String:
  SCon{Char.from_u32(13), SCon{Char.from_u32(10), SNil{}}}

# Vibe's _detect_newline: the most frequent of "\r\n", "\n", "\r" (a
# tie goes to that order); none: "\n" (os.linesep on POSIX). A lone
# "\r" majority is kept as "\n" (said above).
def nl_style.pick(crlf_wins: Bool) -> String:
  match crlf_wins:
    case True{}:
      CRLF()
    case False{}:
      "\n"

def nl_style(+text: String) -> String:
  +crlf = count(text, CRLF())
  +lf = Nat.sub(count(text, "\n"), crlf)
  nl_style.pick(Bool.and(Nat.is_gt(crlf, 0n), Nat.is_ge(crlf, lf)))

def normalize(+text: String) -> String:
  replace(replace(text, CRLF(), "\n", True{}), CR(), "\n", True{})

def restore.pick(lf: Bool, +text: String, +nl: String) -> String:
  match lf:
    case True{}:
      text
    case False{}:
      replace(text, "\n", nl, True{})

def restore(+text: String, +nl: String) -> String:
  restore.pick(String.eq(nl, "\n"), text, nl)

# ---- UTF-8 ----

def char_bytes(c: Char) -> Nat:
  +u = Char.to_u32(c)
  Bool.pick(Nat, (u < 128 : U32), 1n,
    Bool.pick(Nat, (u < 2048 : U32), 2n,
      Bool.pick(Nat, (u < 65536 : U32), 3n, 4n)))

def utf8_len.go(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{h, t}:
      utf8_len.go(t, Nat.add(acc, char_bytes(h)))

def utf8_len(s: String) -> Nat:
  utf8_len.go(s, 0n)

# the offset of the first byte that is not valid UTF-8 (Python's
# UnicodeDecodeError.start: the start of the broken sequence), None
# when the bytes are UTF-8. The scan's state: `need` continuation bytes
# still owed by the sequence that started at `start`; `bad`: broken.
type U8 is Data:
  U8{need: Nat, start: Nat, bad: Bool}

# a lead byte: how many continuation bytes follow, None: not a lead
def utf8_lead(+b: U32) -> Maybe<&2, Nat>:
  Bool.pick(Maybe<&2, Nat>, (b < 128 : U32), Some{0n},
    Bool.pick(Maybe<&2, Nat>, (b < 194 : U32), None{},
      Bool.pick(Maybe<&2, Nat>, (b < 224 : U32), Some{1n},
        Bool.pick(Maybe<&2, Nat>, (b < 240 : U32), Some{2n},
          Bool.pick(Maybe<&2, Nat>, (b < 245 : U32), Some{3n}, None{})))))

def utf8_step.lead(m: Maybe<&2, Nat>, +i: Nat) -> U8:
  match m:
    case None{}:
      U8{0n, i, True{}}
    case Some{+n}:
      U8{n, i, False{}}

def utf8_step.cont(ok: Bool, +p: Nat, +start: Nat) -> U8:
  match ok:
    case True{}:
      U8{p, start, False{}}
    case False{}:
      U8{0n, start, True{}}

# one byte `b` at offset `i`
def utf8_step(+b: U32, +i: Nat, +need: Nat, +start: Nat) -> U8:
  match need:
    case 0n:
      utf8_step.lead(utf8_lead(b), i)
    case 1n+p:
      utf8_step.cont(Bool.and((b >= 128 : U32), (b < 192 : U32)), p, start)

def utf8_bad.end(+need: Nat, +start: Nat) -> Maybe<&2, Nat>:
  match need:
    case 0n:
      None{}
    case 1n+_:
      Some{start}

def utf8_bad.last(+bad: Bool, +need: Nat, +start: Nat) -> Maybe<&2, Nat>:
  match bad:
    case True{}:
      Some{start}
    case False{}:
      utf8_bad.end(need, start)

def utf8_bad.go(bs: List<&2, U32>, +i: Nat, st: U8) -> Maybe<&2, Nat>:
  match bs:
    case Nil{}:
      match st:
        case U8{+need, +start, +bad}:
          utf8_bad.last(bad, need, start)
    case Con{+b, rest}:
      match st:
        case U8{+need, +start, +bad}:
          match bad:
            case True{}:
              Some{start}
            case False{}:
              utf8_bad.go(rest, Nat.add(i, 1n), utf8_step(b, i, need, start))

def utf8_bad(bs: List<&2, U32>) -> Maybe<&2, Nat>:
  utf8_bad.go(bs, 0n, U8{0n, 0n, False{}})

# U+FFFD: what the runtime's text read puts in place of a bad byte
def has_replacement(+s: String) -> Bool:
  String.contains(s, SCon{Char.from_u32(65533), SNil{}})

# ---- the edit itself, on the normalized text ----

# ---- the line lists of the results and errors ----

# "12" or "12-14": a span of `h` lines from line `a` (h = 0: a deletion,
# shown at the line it was)
def span.pick(one: Bool, +a: Nat, +h: Nat) -> String:
  match one:
    case True{}:
      Nat.show(a)
    case False{}:
      String.concat([Nat.show(a), "-", Nat.show(Nat.sub(Nat.add(a, h), 1n))])

def span(+a: Nat, +h: Nat) -> String:
  span.pick(Nat.is_lt(h, 2n), a, h)

def uniq.keep(same: Bool, +x: Nat, +rest: List<&2, Nat>) -> List<&2, Nat>:
  match same:
    case True{}:
      rest
    case False{}:
      Con{x, rest}

# the same line twice in a row counts once (two matches on one line);
# lines start at 1, so `last` = 0 is no line
def uniq.go(xs: List<&2, Nat>, +last: Nat) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, rest}:
      uniq.keep(Nat.is_eq(x, last), x, uniq.go(rest, x))

def uniq(xs: List<&2, Nat>) -> List<&2, Nat>:
  uniq.go(xs, 0n)

# at most 3 spans, then " (+N more)"
def spans.more(+n: Nat) -> String:
  match n:
    case 0n:
      ""
    case 1n+_:
      String.concat([" (+", Nat.show(n), " more)"])

def spans.go(+i: Nat, xs: List<&2, Nat>, +h: Nat) -> List<&2, String>:
  match i:
    case 0n:
      Nil{}
    case 1n+k:
      match xs:
        case Nil{}:
          Nil{}
        case Con{+x, rest}:
          Con{span(x, h), spans.go(k, rest, h)}

def spans(xs: List<&2, Nat>, +h: Nat) -> String:
  +u = uniq(xs)
  String.append(String.join(spans.go(3n, u, h), ", "),
    spans.more(Nat.sub(List.length(&2, Nat, u), 3n)))

# "<lead>line 4." / "<lead>lines 4, 9."; no line: ""
def lines_hint.pick(+n: Nat, +lead: String, +at: List<&2, Nat>) -> String:
  match n:
    case 0n:
      ""
    case 1n+p:
      String.concat([lead, Bool.pick(String, Nat.is_eq(p, 0n), "line ", "lines "),
        spans(at, 1n), "."])

def lines_hint(+lead: String, +at: List<&2, Nat>) -> String:
  lines_hint.pick(List.length(&2, Nat, uniq(at)), lead, at)

# the lines a text takes: "a\nb" 2, "a\n" 1, "" 0
def text_lines.pick(open: Bool, +text: String) -> Nat:
  match open:
    case True{}:
      Nat.add(count(text, "\n"), 1n)
    case False{}:
      count(text, "\n")

def text_lines(+text: String) -> Nat:
  text_lines.pick(Bool.and(Bool.not(String.is_empty(text)),
    Bool.not(String.ends_with(text, "\n"))), text)

# The results and errors say only what the model cannot know from its
# own call (Gabriel's call, tokens): never old_string, new_string or
# content, which its call carries already. These functions take none
# of them, so a result cannot depend on them (law
# edit_result_independent). Vibe's two errors ended with
# "String: <old_string>": here they say lines instead.

# `starts`: the line of each replacement in the file after the edit;
# `h`: the lines new_string takes (text_lines)
def edit_result(+path: String, +msg: String, +starts: List<&2, Nat>,
    +h: Nat) -> String:
  String.concat(["file: ", path, "\nmessage: ", msg, "\nreplacements: ",
    Nat.show(List.length(&2, Nat, starts)), "\nlines: ", spans(starts, h)])

# `at`: the lines where old_string's first line sits (first_line_at),
# a hint for the retry
def err_not_found(+at: List<&2, Nat>) -> String:
  String.append("String to replace not found in file.",
    lines_hint("\nIts first line is at ", at))

# `at`: the lines of the matches
def err_many(+n: Nat, +at: List<&2, Nat>) -> String:
  String.concat(["Found ", Nat.show(n),
    " matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.",
    lines_hint("\nAt ", at)])

# ---- where the matches sit: their lines, for the result and errors ----

def nl_of(+c: Char) -> Nat:
  Bool.pick(Nat, U32.is_eq(Char.to_u32(c), 10), 1n, 0n)

# the line of each match of `pat` in `s`, with replace's walk
# (non-overlapping, left to right; the first only unless `all`): `line`
# is where the walk is, and a match moves it by `step` lines, so step =
# the lines `pat` breaks gives the lines in `s`, and step = new's the
# lines in the text after the replace
def spots.go(fuel: Nat, here: Bool, +s: String, +pat: String, +step: Nat,
    +line: Nat, +all: Bool) -> List<&2, Nat>:
  match fuel:
    case 0n:
      Nil{}
    case 1n+k:
      match here:
        case True{}:
          match all:
            case True{}:
              Con{line, spots.go(k,
                String.starts_with(String.drop(s, String.length(pat)), pat),
                String.drop(s, String.length(pat)), pat, step,
                Nat.add(line, step), True{})}
            case False{}:
              Con{line, Nil{}}
        case False{}:
          match s:
            case SNil{}:
              Nil{}
            case SCon{h, +t}:
              spots.go(k, String.starts_with(t, pat), t, pat, step,
                Nat.add(line, nl_of(h)), all)

def spots(+s: String, +pat: String, +step: Nat, +all: Bool) -> List<&2, Nat>:
  spots.go(String.length(s),
    Bool.and(Bool.not(String.is_empty(pat)), String.starts_with(s, pat)),
    s, pat, step, 1n, all)

# the characters that are not blanks
def solid.go(s: String, +n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{h, t}:
      solid.go(t, Nat.add(n, Bool.pick(Nat, Char.is_space(h), 0n, 1n)))

def solid(s: String) -> Nat:
  solid.go(s, 0n)

def first_line.of(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+l, _}:
      String.trim(l)

def first_line_at.pick(ok: Bool, +text: String, +first: String) -> List<&2, Nat>:
  match ok:
    case True{}:
      spots(text, first, 0n, True{})
    case False{}:
      Nil{}

# where old_string's first line sits in the text, for not-found's hint;
# a first line under 4 non-blank characters ("}", ");") says nothing
def first_line_at(+text: String, +old: String) -> List<&2, Nat>:
  +first = first_line.of(String.lines(old))
  first_line_at.pick(Nat.is_ge(solid(first), 4n), text, first)

type Edited is Data:
  EdFail{msg: String}
  EdOk{text: String, msg: String, starts: List<&2, Nat>}

def apply.many(many: Bool, +n: Nat, +text: String, +old: String,
    +new: String, +all: Bool) -> Edited:
  match many:
    case True{}:
      EdFail{err_many(n, spots(text, old, count(old, "\n"), True{}))}
    case False{}:
      EdOk{replace(text, old, new, all), ok_msg(all),
        spots(text, old, count(new, "\n"), all)}

def apply.found(+n: Nat, +text: String, +old: String, +new: String,
    +all: Bool) -> Edited:
  match n:
    case 0n:
      EdFail{err_not_found(first_line_at(text, old))}
    case 1n+p:
      apply.many(Bool.and(Nat.is_gt(p, 0n), Bool.not(all)), 1n+p, text, old,
        new, all)

# Vibe's run(): not found, then several matches without replace_all,
# then the replacement (the first one, or all)
def apply(+text: String, +old: String, +new: String, +all: Bool) -> Edited:
  apply.found(count(text, old), text, old, new, all)

# the checks on the arguments alone, in Vibe's order (_validate_args);
# the path's own checks (exists, is a file) need the disk
def check_args.same(same: Bool) -> Maybe<&2, String>:
  match same:
    case True{}:
      Some{err_same()}
    case False{}:
      None{}

def check_args.old(empty: Bool, +old: String, +new: String) -> Maybe<&2, String>:
  match empty:
    case True{}:
      Some{err_empty_old()}
    case False{}:
      check_args.same(String.eq(old, new))

def check_args.path(empty: Bool, +old: String, +new: String) -> Maybe<&2, String>:
  match empty:
    case True{}:
      Some{err_empty_path()}
    case False{}:
      check_args.old(String.is_empty(old), old, new)

def check_args(+path: String, +old: String, +new: String) -> Maybe<&2, String>:
  check_args.path(String.is_empty(String.trim(path)), old, new)

# write_file's checks before the disk: the path, then the size (UTF-8
# bytes, Vibe's max_write_bytes)
def check_write.size(big: Bool) -> Maybe<&2, String>:
  match big:
    case True{}:
      Some{werr_too_big()}
    case False{}:
      None{}

def check_write.path(empty: Bool, +content: String) -> Maybe<&2, String>:
  match empty:
    case True{}:
      Some{werr_empty_path()}
    case False{}:
      check_write.size(Nat.is_gt(utf8_len(content), WRITE_MAX()))

def check_write(+path: String, +content: String) -> Maybe<&2, String>:
  check_write.path(String.is_empty(String.trim(path)), content)

# ---- the arguments ----

type EditArgs is Data:
  EditArgs{path: String, old: String, new: String, all: Bool}

def jbool.of(m: Maybe<&2, J.Json>) -> Bool:
  match m:
    case Some{J.JBool{+b}}:
      b
    case _:
      False{}

def edit_args.of(r: Result<&2, &2, String, J.Json>) -> EditArgs:
  match r:
    case Done{+j}:
      EditArgs{J.get_str_or(j, "file_path", ""), J.get_str_or(j, "old_string", ""),
        J.get_str_or(j, "new_string", ""), jbool.of(J.get(j, "replace_all"))}
    case Fail{_}:
      EditArgs{"", "", "", False{}}

def edit_args(+args: String) -> EditArgs:
  edit_args.of(J.parse(args))

type WriteArgs is Data:
  WriteArgs{path: String, content: String}

def write_args.of(r: Result<&2, &2, String, J.Json>) -> WriteArgs:
  match r:
    case Done{+j}:
      WriteArgs{J.get_str_or(j, "file_path", ""), J.get_str_or(j, "content", "")}
    case Fail{_}:
      WriteArgs{"", ""}

def write_args(+args: String) -> WriteArgs:
  write_args.of(J.parse(args))

# ---- paths: Vibe's resolve_tool_path, without the symlink resolve ----
# `~/` is the home folder; a relative path lives in the agent's working
# folder (BEND_WORKDIR, like apply_patch's); "." and ".." segments go.

def seg_push.up(acc: List<&2, String>) -> List<&2, String>:
  match acc:
    case Nil{}:
      Nil{}
    case Con{_, +rest}:
      rest

def seg_push.keep(skip: Bool, +seg: String, +acc: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      acc
    case False{}:
      Con{seg, acc}

def seg_push.pick(up: Bool, +seg: String, +acc: List<&2, String>) -> List<&2, String>:
  match up:
    case True{}:
      seg_push.up(acc)
    case False{}:
      seg_push.keep(Bool.or(String.is_empty(seg), String.eq(seg, ".")), seg, acc)

def seg_push(+seg: String, +acc: List<&2, String>) -> List<&2, String>:
  seg_push.pick(String.eq(seg, ".."), seg, acc)

def segs_norm(segs: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match segs:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+s, rest}:
      segs_norm(rest, seg_push(s, acc))

def norm_abs(+path: String) -> String:
  String.append("/", String.join(segs_norm(String.split(path, '/'), Nil{}), "/"))

def join.pick(abs: Bool, +dir: String, +path: String) -> String:
  match abs:
    case True{}:
      path
    case False{}:
      String.concat([dir, "/", path])

def expand_home.pick(tilde: Bool, +home: String, +path: String) -> String:
  match tilde:
    case True{}:
      String.append(home, String.drop(path, 1n))
    case False{}:
      path

def expand_home(+home: String, +path: String) -> String:
  expand_home.pick(Bool.and(Bool.not(String.is_empty(home)),
    Bool.or(String.eq(path, "~"), String.starts_with(path, "~/"))), home, path)

# the absolute path a tool call acts on; `dir` = the working folder
def resolve.norm(abs: Bool, +p: String) -> String:
  match abs:
    case True{}:
      norm_abs(p)
    case False{}:
      p

def resolve(+dir: String, +home: String, +path: String) -> String:
  +p = join.pick(String.starts_with(expand_home(home, String.trim(path)), "/"),
    dir, expand_home(home, String.trim(path)))
  resolve.norm(String.starts_with(p, "/"), p)

# ---- what the gate reads (1a / 1b): {"file_path": <absolute>} ----
def gate_args(+dir: String, +home: String, +path: String) -> String:
  J.stringify(J.obj(Con{J.kv("file_path", J.str(resolve(dir, home, path))), Nil{}}))

def edit_path(a: EditArgs) -> String:
  match a:
    case EditArgs{+p, _, _, _}:
      p

def write_path(a: WriteArgs) -> String:
  match a:
    case WriteArgs{+p, _}:
      p

def gate_path.pick(write: Bool, +args: String) -> String:
  match write:
    case True{}:
      write_path(write_args(args))
    case False{}:
      edit_path(edit_args(args))

def gate_path(+name: String, +args: String) -> String:
  gate_path.pick(String.eq(name, "write_file"), args)

# ---- how the TUI shows a call: as a V4A patch (rendered as a diff
# like apply_patch's) ----

def prefix_lines(+mark: String, lines: List<&2, String>) -> List<&2, String>:
  match lines:
    case Nil{}:
      Nil{}
    case Con{+l, rest}:
      Con{String.append(mark, l), prefix_lines(mark, rest)}

def body_lines(+mark: String, +text: String) -> String:
  String.join(prefix_lines(mark, String.lines(text)), "\n")

def edit_view.of(a: EditArgs) -> String:
  match a:
    case EditArgs{+p, +old, +new, _}:
      String.concat(["*** Begin Patch\n*** Update File: ", p, "\n@@\n",
        body_lines("-", old), "\n", body_lines("+", new), "\n*** End Patch"])

def edit_view(+args: String) -> String:
  edit_view.of(edit_args(args))

def write_view.of(a: WriteArgs) -> String:
  match a:
    case WriteArgs{+p, +content}:
      String.concat(["*** Begin Patch\n*** Add File: ", p, "\n",
        body_lines("+", content), "\n*** End Patch"])

def write_view(+args: String) -> String:
  write_view.of(write_args(args))

# the one-line args of the `tool #` feed line: the path first (the hub
# reads it for the activity line: "edit <file>" / "write <file>")
def ann_args(+name: String, +args: String) -> String:
  J.stringify(J.obj(Con{J.kv("file_path", J.str(gate_path(name, args))), Nil{}}))
