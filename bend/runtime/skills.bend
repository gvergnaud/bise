# runtime/skills.bend — the Agent Skills feature (Vibe CLI design).
#
# Progressive disclosure, exactly as vibe_sdk/harness/core
# features/skills: the system prompt carries ONLY name + description +
# path per skill (an <available-skills> section), and the `skill` tool
# loads the full SKILL.md instructions on demand. Callers must use an
# exact name from the section; a miss returns the error contract with
# the available list.
#
# The catalog is a local index file written at live startup by scanning
# the standard skill folders (~/.agents/skills, ~/.vibe/skills, and the
# repo's .agents/skills) for SKILL.md files with YAML frontmatter
# (name:, description:) — the Agent Skills format. Scripted runs never
# scan and never read the index: the scenarios stay hermetic.
#
# index line: "name\tdescription\tpath" (tab-separated; the scan strips
# tabs and quotes from the fields, so the split is unambiguous)
import Base
import ../core/types.bend as T
import ../core/text.bend as Tx
import ./persist.bend as P
import ./mcp.bend as Mc
import ../vendor/json.bend as J
import ./proc.bend as Proc
import ./skills-pure.bend as Skp
import ./plugins.bend as Pl

# ---- the tool contract ----

def SKILL_USAGE() -> String:
  "skill: call exactly with {\"name\":\"<exact name>\"}; no other argument keys are accepted"

# ---- the index file (mirror of the MCP index pattern) ----

def index_path() -> IO(String):
  P.env_path("BEND_SKILLS_INDEX", "/.bend-harness/skills-index.txt")

# ---- the startup scan (live only) ----
# One POSIX shell script writes two indexes: every SKILL.md under a
# skill folder with a name: and a single-line description: in its
# frontmatter.
#   $1 the shared index: the user folders (~/.agents/skills,
#      ~/.vibe/skills); the TUI `$` popup reads it
#   $2 this session's index: the workspace folder
#      ($BEND_WORKDIR/.agents/skills - the hub runs a task's REPL in the
#      app root, so $PWD would miss the task's workspace - else
#      $PWD/.agents/skills), then the plugin skills ($3, written by the
#      plugins bridge, runtime/plugins.bend), then every agent's
#      built-in skills, the app root's prompts/skills-all (bise-pages),
#      only where the desktop is on (BISE_DESKTOP=1, set by the hub with
#      the prompt's own flag),
#      then, for main only (BISE_ROLE=main, set by the hub), main's
#      built-in skills: the app
#      root's prompts/skills (the hub runs the REPL in the app root, and
#      versions.sh and build-dist.sh ship prompts/ whole), e.g. bise-demo
# Each index is written to a temp file then renamed: every REPL start
# rescans, and a skill call or the TUI reading the shared index mid-scan
# must see the old index or the new one, never a truncated file.
# The group ends with `true`: its status is its last command's, and a
# missing $3 (no plugins bridge, or one not ready within 15 s) or a last
# SKILL.md without a description made it fail, so the rename never came
# (no session index, a temp file left behind).
# skill_ok keeps absolute paths only, so a relative one would drop
# every local skill. Tabs and quotes are stripped from the fields so the
# tab-separated line stays unambiguous; folded multi-line YAML
# descriptions are out of scope (the Agent Skills format is one line).
# The scan never blocks the REPL's start (the first model call waits on
# it): a SKILL.md that links into ~/Documents made `sed` wait ~40 min on
# macOS's privacy check, and main never made its first call. So no
# command reads a user path in the foreground: each folder's listing,
# then each SKILL.md (its first 64 KiB, `head`), is read by a background
# job of its own into a temp folder, all at once; the script waits at
# most $BEND_SKILLS_SCAN_SECS (default 3) for the listings, then as long
# for the files, kills what is left (its children: the job of a file
# stuck in the kernel just stays, with no hold on our output), parses the
# copies, and prints one line per skipped path ("skills scan: skipped
# <path> (no answer in 3 s)"), which the REPL puts on its stderr
# (repl.err). A SKILL.md that is not a regular file (a FIFO) is read the
# same way: a writer-less FIFO is skipped at the deadline. Proc.run_for
# kills the script itself after 20 s (a shell that hangs anyway).

def scan_script() -> String:
  "lim=\"${BEND_SKILLS_SCAN_SECS:-3}\"
t=$(mktemp -d \"${TMPDIR:-/tmp}/bend-skills.XXXXXX\") || exit 0
trap 'rm -rf \"$t\"' EXIT
job() {
  id=\"$1\"; shift
  ( \"$@\" > \"$t/$id\" 2>/dev/null; : > \"$t/$id.ok\" ) < /dev/null > /dev/null 2>&1 &
  echo \"$!\" > \"$t/$id.pid\"
}
settle() {
  n=$((n + 1))
  ( sleep \"$lim\"; : > \"$t/late$n\" ) < /dev/null > /dev/null 2>&1 &
  kills=\"$kills,$!\"
  while :; do
    all=1
    for id in \"$@\"; do [ -e \"$t/$id.ok\" ] || { all=0; break; }; done
    [ \"$all\" = 1 ] && break
    [ -e \"$t/late$n\" ] && break
    sleep 0.05
  done
}
ok() {
  [ -e \"$t/$1.ok\" ] && return 0
  read -r p < \"$t/$1.pid\" && kills=\"$kills,$p\"
  printf 'skills scan: skipped %s (no answer in %s s)\\n' \"$2\" \"$lim\"
  return 1
}
entries() {
  [ -d \"$1\" ] || return 0
  for d in \"$1\"/*; do [ -e \"$d\" ] && printf '%s\\n' \"$d\"; done
}
index() {
  for r; do shift; [ -f \"$t/r$r.files\" ] && set -- \"$@\" \"$t/r$r.files\"; done
  [ $# -gt 0 ] || return 0
  awk -v t=\"$t\" '{
    id = $1; p = substr($0, length($1) + 2); ok = t \"/\" id \".ok\"
    if ((getline x < ok) < 0) next
    close(ok); f = t \"/\" id; n = \"\"; s = \"\"; hn = 0; hs = 0
    while ((getline l < f) > 0) {
      if (!hn && l ~ /^name:/) { hn = 1; sub(/^name:[ \\t]*/, \"\", l); n = l; gsub(/[\\t\\r\"]/, \"\", n) }
      else if (!hs && l ~ /^description:/) { hs = 1; sub(/^description:[ \\t]*/, \"\", l); s = l; gsub(/[\\t\\r\"]/, \"\", s) }
    }
    close(f)
    if (n != \"\" && s != \"\") printf \"%s\\t%s\\t%s\\n\", n, s, p
  }' \"$@\"
}
w=\"${BEND_WORKDIR:-$PWD}\"
b=\"\"; [ \"${BISE_ROLE:-}\" = main ] && b=\"$PWD/prompts/skills\"
a=\"\"; [ \"${BISE_DESKTOP:-}\" = 1 ] && a=\"$PWD/prompts/skills-all\"
set -- \"$1\" \"$2\" \"$3\" \"$HOME/.agents/skills\" \"$HOME/.vibe/skills\" \"$w/.agents/skills\" \"$a\" \"$b\"
r=0; rs=\"\"
for root in \"$4\" \"$5\" \"$6\" \"$7\" \"$8\"; do
  r=$((r + 1)); [ -n \"$root\" ] && { job \"r$r\" entries \"$root\"; rs=\"$rs r$r\"; }
done
settle $rs
k=0; fs=\"\"; r=0
for root in \"$4\" \"$5\" \"$6\" \"$7\" \"$8\"; do
  r=$((r + 1)); [ -n \"$root\" ] || continue
  ok \"r$r\" \"$root\" || continue
  while IFS= read -r d; do
    k=$((k + 1)); f=\"$d/SKILL.md\"
    job \"f$k\" head -c 65536 \"$f\"
    printf 'f%s %s\\n' \"$k\" \"$f\" >> \"$t/r$r.files\"
    fs=\"$fs f$k\"
  done < \"$t/r$r\"
done
settle $fs
for r in 1 2 3 4 5; do
  [ -f \"$t/r$r.files\" ] || continue
  while IFS=' ' read -r id f; do ok \"$id\" \"$f\" || true; done < \"$t/r$r.files\"
done
[ -n \"$kills\" ] && pkill -9 -P \"${kills#,}\" 2>/dev/null
mkdir -p \"$(dirname \"$1\")\" \"$(dirname \"$2\")\"
{ index 1 2; true; } > \"$1.$$\" && mv -f \"$1.$$\" \"$1\"
{ index 3; cat \"$3\" 2>/dev/null; index 4 5; true; } > \"$2.$$\" && mv -f \"$2.$$\" \"$2\""

# the backstop: the script bounds itself (two waits of $lim s), so this
# only fires if the shell itself hangs; the REPL then starts on the
# indexes of its last start
def scan_timeout_ms() -> U32:
  20000

def scan_path(+dir: String, +port: String) -> String:
  String.append(dir, String.append("/bend-skills-scan-", String.append(port, ".sh")))

def session_index_path() -> IO(String):
  do IO<String>:
    +r : String <- Pl.run_dir()
    IO.pure(String, String.append(r, "/skills-index.txt"))

# the scan's skipped paths, on the REPL's stderr (repl.err); nothing
# when it skipped none
def scan_note(quiet: Bool, +note: String) -> IO(Unit):
  match quiet:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      IO.print_err(note)

def skills_scan() -> IO(Unit):
  do IO<Unit>:
    +idx : String <- index_path()
    +sidx : String <- session_index_path()
    +psk : String <- Pl.skills_index_path()
    pr : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_REPL_PORT")
    +port : String <- IO.pure(String, P.env_or(pr, "0"))
    +side : String <- P.side_dir()
    +path : String <- IO.pure(String, scan_path(side, port))
    u : Unit <- P.write_file(path, scan_script())
    +out : String <- Proc.run_for(Con{"/bin/sh", Con{path, Con{idx, Con{sidx, Con{psk, Nil{}}}}}},
      scan_timeout_ms())
    +note : String <- IO.pure(String, Skp.scan_note(out))
    scan_note(String.is_empty(note), note)

# the index the catalog and the skill tool read: this session's index
# first (workspace and plugin skills win a name clash: the first
# occurrence of a name is kept), then the shared one
def index_content.join(ms: Maybe<&2, String>, mu: Maybe<&2, String>)
    -> Maybe<&2, String>:
  match ms:
    case None{}:
      mu
    case Some{+s}:
      Some{String.append(s, String.append("
", Maybe.default(&2, String, mu, "")))}

def index_content() -> IO(Maybe<&2, String>):
  do IO<Maybe<&2, String>>:
    +sidx : String <- session_index_path()
    +idx : String <- index_path()
    ms : Maybe<&2, String> <- P.read_file(sidx)
    mu : Maybe<&2, String> <- P.read_file(idx)
    IO.pure(Maybe<&2, String>, index_content.join(ms, mu))

# ---- execution ----

def skill_path(+sk: Skp.Skill) -> String:
  match sk:
    case Skp.Skill{_, _, +path}:
      path

def read_skill.go(m: Maybe<&2, String>, +sk: Skp.Skill) -> Mc.MR:
  match m:
    case None{}:
      Mc.MR{False{},
        String.append("skill file unreadable: ", skill_path(sk))}
    case Some{+content}:
      Mc.MR{True{}, Skp.strip_frontmatter(content)}

def read_skill(sk: Skp.Skill) -> IO(Mc.MR):
  match sk:
    case Skp.Skill{_, _, +path}:
      do IO<Mc.MR>:
        m : Maybe<&2, String> <- P.read_file(path)
        IO.pure(Mc.MR, read_skill.go(m, sk))

# a name the index lacks is not an unreadable index (an empty index said
# "unreadable" and sent the search to the wrong place)
def exec_skill.live.found(fs: Maybe<&2, Skp.Skill>, +name: String) -> IO(Mc.MR):
  match fs:
    case None{}:
      IO.pure(Mc.MR, Mc.MR{False{},
        String.append("unknown skill: ", String.append(name,
          ": call it with an exact name from <available-skills>"))})
    case Some{+sk}:
      read_skill(sk)

def exec_skill.live.go(m: Maybe<&2, String>, +name: String) -> IO(Mc.MR):
  match m:
    case None{}:
      IO.pure(Mc.MR,
        Mc.MR{False{}, "skills index unreadable: no skills are available"})
    case Some{+content}:
      exec_skill.live.found(Skp.find_skill(content, name), name)

def exec_skill.live.pick(n: Maybe<&2, String>) -> IO(Mc.MR):
  match n:
    case None{}:
      IO.pure(Mc.MR, Mc.MR{False{}, SKILL_USAGE()})
    case Some{+name}:
      do IO<Mc.MR>:
        m : Maybe<&2, String> <- index_content()
        exec_skill.live.go(m, name)

def exec_skill.live(+args: String) -> IO(Mc.MR):
  do IO<Mc.MR>:
    n : Maybe<&2, String> <- IO.pure(Maybe<&2, String>, Skp.args_name(args))
    exec_skill.live.pick(n)

def exec_skill.scripted() -> Mc.MR:
  Mc.MR{False{}, "skills need a live session"}

def exec_skill(+args: String, live: Bool) -> IO(Mc.MR):
  match live:
    case True{}:
      exec_skill.live(args)
    case False{}:
      IO.pure(Mc.MR, exec_skill.scripted())

def prompt_content() -> IO(String):
  do IO<String>:
    m : Maybe<&2, String> <- index_content()
    IO.pure(String, Maybe.default(&2, String, m, ""))
