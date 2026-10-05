# runtime/skills-pure.bend - the pure half of runtime/skills.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base
import ../vendor/json.bend as J
import ../core/text.bend as Tx

def tab() -> Char:
  Char.from_u32(9)

# ---- the index line format, parsed ----

type Skill is Data:
  Skill{name: String, desc: String, path: String}

def line_fields.name(fields: List<&2, String>) -> String:
  match fields:
    case Nil{}:
      ""
    case Con{+n, _}:
      n

def line_fields.desc(fields: List<&2, String>) -> String:
  match fields:
    case Con{_, Con{+d, _}}:
      d
    case _:
      ""

# the path is everything after the second tab, tabs rejoined (a path
# can never contain one, but the rejoin keeps the parse total)
def line_fields.tail_join(rest: List<&2, String>, +s: String) -> String:
  match rest:
    case Nil{}:
      s
    case Con{+x, +r2}:
      String.append(s, String.append("\t", line_fields.tail_join(r2, x)))

def line_fields.tail(rest: List<&2, String>) -> String:
  match rest:
    case Nil{}:
      ""
    case Con{+s, +r2}:
      line_fields.tail_join(r2, s)

def line_fields.path(fields: List<&2, String>) -> String:
  match fields:
    case Con{_, Con{_, +rest}}:
      line_fields.tail(rest)
    case _:
      ""

def skill_of_line.of(+fields: List<&2, String>) -> Skill:
  Skill{line_fields.name(fields), line_fields.desc(fields),
    line_fields.path(fields)}

def skill_of_line(line: String) -> Skill:
  skill_of_line.of(String.split(line, tab()))


def skill_name(+sk: Skill) -> String:
  match sk:
    case Skill{+name, _, _}:
      name

# a well-formed line: non-empty name and description, absolute path
def skill_ok(+sk: Skill) -> Bool:
  match sk:
    case Skill{+name, +desc, +path}:
      Bool.and(Bool.not(String.is_empty(name)),
        Bool.and(Bool.not(String.is_empty(desc)),
          String.starts_with(path, "/")))

def skill_ok_line(line: String) -> Bool:
  skill_ok(skill_of_line(line))

# ---- find a skill by exact name (first hit wins) ----

def find_skill.hit(+sk: Skill, +name: String) -> Bool:
  Bool.and(skill_ok(sk), String.eq(skill_name(sk), name))

def find_skill.put(hit: Bool, +sk: Skill,
    +rest_res: Maybe<&2, Skill>) -> Maybe<&2, Skill>:
  match hit:
    case True{}:
      Some{sk}
    case False{}:
      rest_res

def find_skill.go(lines: List<&2, String>, +name: String)
    -> Maybe<&2, Skill>:
  match lines:
    case Nil{}:
      None{}
    case Con{+l, rest}:
      find_skill.put(find_skill.hit(skill_of_line(l), name), skill_of_line(l),
        find_skill.go(rest, name))

def find_skill(+content: String, +name: String) -> Maybe<&2, Skill>:
  find_skill.go(String.split(content, Tx.newline()), name)

# ---- the prompt sections (pure; the laws pin the exact contracts) ----

# XML escaping per vibe_sdk prompt.rs: &, <, >, ", '
def esc_apos(is_a: Bool, +c: U32) -> String:
  match is_a:
    case True{}:
      "&apos;"
    case False{}:
      SCon{Chr{c}, SNil{}}

def esc_quot(is_q: Bool, +c: U32) -> String:
  match is_q:
    case True{}:
      "&quot;"
    case False{}:
      esc_apos(U32.is_eq(c, 39), c)

def esc_gt(is_gt: Bool, +c: U32) -> String:
  match is_gt:
    case True{}:
      "&gt;"
    case False{}:
      esc_quot(U32.is_eq(c, 34), c)

def esc_lt(is_lt: Bool, +c: U32) -> String:
  match is_lt:
    case True{}:
      "&lt;"
    case False{}:
      esc_gt(U32.is_eq(c, 62), c)

def esc_code(is_amp: Bool, +c: U32) -> String:
  match is_amp:
    case True{}:
      "&amp;"
    case False{}:
      esc_lt(U32.is_eq(c, 60), c)

def escape_xml(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{Chr{+c}, +t}:
      String.append(esc_code(U32.is_eq(c, 38), c), escape_xml(t))

# one <skill> entry, the exact vibe_sdk format
def skill_entry(+sk: Skill) -> String:
  match sk:
    case Skill{+name, +desc, +path}:
      String.append("  <skill>\n    <name>",
        String.append(escape_xml(name),
          String.append("</name>\n    <description>",
            String.append(escape_xml(desc),
              String.append("</description>\n    <path>",
                String.append(escape_xml(path),
                  "</path>\n  </skill>"))))))

# dedupe by name, first occurrence wins (the conn_names.go pattern)
def names_has(names: List<&2, String>, +v: String) -> Bool:
  match names:
    case Nil{}:
      False{}
    case Con{+n, rest}:
      Bool.or(String.eq(n, v), names_has(rest, v))

def catalog_put(ok: Bool, +line: String, +rest_out: String) -> String:
  match ok:
    case True{}:
      String.append(skill_entry(skill_of_line(line)),
        String.append("\n", rest_out))
    case False{}:
      rest_out

def catalog_go(lines: List<&2, String>, +seen: List<&2, String>,
    +rest_out: String) -> String:
  match lines:
    case Nil{}:
      rest_out
    case Con{+l, rest}:
      catalog_put(
        Bool.and(skill_ok_line(l),
          Bool.not(names_has(seen, skill_name(skill_of_line(l))))),
        l, catalog_go(rest, Con{skill_name(skill_of_line(l)), seen}, rest_out))

def skills_catalog.pick(empty: Bool, +content: String) -> String:
  match empty:
    case True{}:
      ""
    case False{}:
      String.append("<available-skills>\n",
        String.append(catalog_go(String.split(content, Tx.newline()), Nil{}, ""),
          "</available-skills>"))

def skills_catalog(+content: String) -> String:
  skills_catalog.pick(
    String.is_empty(catalog_go(String.split(content, Tx.newline()), Nil{}, "")),
    content)

# the rules section, the exact vibe_sdk text (read_file -> bash: the
# Bend harness reaches files through the shell)
def skills_rules() -> String:
  "## Available Skills\n\nSkills contain task-specific instructions. When a request matches a skill description, proactively call the `skill` tool with its exact name before starting the task, then follow the loaded instructions. If that skill's full instructions are already present in the current context, follow them without calling `skill` again. Load every skill that materially applies.\n\nCall `skill` only with a name listed in `<available-skills>`. Connector names and unlisted names in skill instructions are not skills.\n\nDo not read a root `SKILL.md` path with `bash` after loading a skill. Use `bash` only for support or reference files mentioned by the loaded skill."

# args are the exact vibe_sdk schema: {"name": "..."}
def args_name.pick(empty: Bool, +name: String) -> Maybe<&2, String>:
  match empty:
    case True{}:
      None{}
    case False{}:
      Some{name}

def args_name.w(+w: J.Json) -> Maybe<&2, String>:
  args_name.pick(String.is_empty(J.get_str_or(w, "name", "")),
    J.get_str_or(w, "name", ""))

# not JSON: the bare name (a small model sends the tool's one string
# argument as the name itself, `bise-demo`, and retried the same call
# seven times against the usage error)
def args_name.parse(r: Result<&2, &2, String, J.Json>, +args: String)
    -> Maybe<&2, String>:
  match r:
    case Fail{_}:
      args_name.pick(String.is_empty(String.trim(args)), String.trim(args))
    case Done{+w}:
      args_name.w(w)

def args_name(+args: String) -> Maybe<&2, String>:
  args_name.parse(J.parse(args), args)

# ---- the miss error (the vibe_sdk contract, suggestions omitted:
# the available list is the operative part) ----

def avail_step(new: Bool, +name: String, +acc: List<&2, String>)
    -> List<&2, String>:
  match new:
    case True{}:
      Con{name, acc}
    case False{}:
      acc

def avail_names.go(lines: List<&2, String>, +seen: List<&2, String>,
    +acc: List<&2, String>) -> List<&2, String>:
  match lines:
    case Nil{}:
      List.reverse(&2, String, acc)
    case Con{+l, rest}:
      avail_names.go(rest, Con{skill_name(skill_of_line(l)), seen},
        avail_step(Bool.and(skill_ok_line(l),
            Bool.not(names_has(seen, skill_name(skill_of_line(l))))),
          skill_name(skill_of_line(l)), acc))

def quote_join(names: List<&2, String>) -> String:
  match names:
    case Nil{}:
      ""
    case Con{+n, rest}:
      String.append(String.append("\"", String.append(n, "\"")),
        quote_join(rest))

def avail_text(names: List<&2, String>) -> String:
  match names:
    case Nil{}:
      " No skills are available."
    case Con{_, _}:
      String.append(" Available skills: ",
        String.append(quote_join(names), "."))

def not_available(+name: String, +content: String) -> String:
  String.append("Skill \"",
    String.append(name,
      String.append("\" is not available. Use an exact name from the available skills below. Connector names and unlisted names in skill instructions are not skills.",
        avail_text(avail_names.go(String.split(content, Tx.newline()), Nil{},
          Nil{})))))

# ---- frontmatter stripping (vibe_sdk ProjectSkill: the tool returns
# the CONTENT, never the YAML header) ----
# The fold pattern of checkpoint.bend: a pure per-line step over a
# state record, one self-recursive driver.

type FmMode is Data:
  FmBefore{}
  FmInside{}
  FmContent{}

type Fm is Data:
  Fm{mode: FmMode, acc: List<&2, String>}

def fm_open(+line: String) -> Bool:
  String.eq(String.trim(line), "---")

def fm_acc(+st: Fm) -> List<&2, String>:
  match st:
    case Fm{_, +acc}:
      acc

def fm_step.before(open: Bool, +st: Fm, +line: String) -> Fm:
  match open:
    case True{}:
      Fm{FmInside{}, fm_acc(st)}
    case False{}:
      # no frontmatter at all: everything is content
      Fm{FmContent{}, Con{line, fm_acc(st)}}

def fm_step.inside(open: Bool, +st: Fm) -> Fm:
  match open:
    case True{}:
      Fm{FmContent{}, fm_acc(st)}
    case False{}:
      st

def fm_step.content(+st: Fm, +line: String) -> Fm:
  Fm{FmContent{}, Con{line, fm_acc(st)}}

def fm_step.dispatch(mode: FmMode, +st: Fm, +line: String) -> Fm:
  match mode:
    case FmBefore{}:
      fm_step.before(fm_open(line), st, line)
    case FmInside{}:
      fm_step.inside(fm_open(line), st)
    case FmContent{}:
      fm_step.content(st, line)

def fm_step(+st: Fm, +line: String) -> Fm:
  match st:
    case Fm{+mode, _}:
      fm_step.dispatch(mode, st, line)

def fm_go(lines: List<&2, String>, st: Fm) -> Fm:
  match lines:
    case Nil{}:
      st
    case Con{+l, rest}:
      fm_go(rest, fm_step(st, l))

def strip_frontmatter.acc(st: Fm) -> String:
  match st:
    case Fm{_, +acc}:
      String.trim(String.join(List.reverse(&2, String, acc), "\n"))

def strip_frontmatter(+content: String) -> String:
  strip_frontmatter.acc(fm_go(String.split(content, Tx.newline()),
    Fm{FmBefore{}, Nil{}}))

# ---- the live prompt section assembly (repl-live uses these) ----

def prompt_with.pick(empty: Bool, +base: String, +content: String) -> String:
  # rules then catalog; both only when skills exist
  match empty:
    case True{}:
      base
    case False{}:
      String.append(base,
        String.append("\n\n",
          String.append(skills_rules(),
            String.append("\n\n", skills_catalog(content)))))

def prompt_with(+base: String, +content: String) -> String:
  prompt_with.pick(String.is_empty(skills_catalog(content)), base, content)
