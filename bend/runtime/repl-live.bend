# runtime/repl-live.bend — the live REPL entry (real provider, real tools).
#
#   bend runtime/repl-live.bend -o repl-live && ./repl-live
#   nc 127.0.0.1 7702
#
# The provider call and the bash tool run INSIDE this process: the hub
# HTTP client does the TLS call to api.mistral.ai (core/api.bend maps
# the wire request to the API JSON and the reply back), and Base's
# Process.run (runtime/proc.bend) runs /bin/sh. The line server itself is
# runtime/repl-core.bend; this file builds the live session config.
import Base
import ../core/types.bend as T
import ./persist.bend as P
import ./mcp.bend as Mcp
import ./mcp-pure.bend as Mcpp
import ./skills.bend as Sk
import ./plugins.bend as Pl
import ./skills-pure.bend as Skp
import ./tools.bend as X
import ./tools-pure.bend as Xp
import ../core/edit.bend as Ed
import ./main.bend as R
import ../runtime/bash.bend as Sh
import ../runtime/provider.bend as Pv
import ../runtime/provider-pure.bend as Pvp
import ../runtime/settings.bend as St
import ./repl-core.bend as Rc
import ./debug.bend as Dbg
import ./repl-core-pure.bend as Rcp
import ./oneshot.bend as Os

# how many tokens of the user's most recent messages a compaction keeps
# verbatim after the summary, counted from the newest (core/history.bend
# preserve_recent): BEND_COMPACT_KEEP env > config.toml
# compaction_keep_tokens > 6000. It was 20000: after nine compactions
# main's context still opened with 30+ user messages from days ago.
def keep_of_cfg(+envv: String, +cfgt: String) -> Nat:
  U32.to_nat(St.env_cfg_u32(envv, cfgt, "compaction_keep_tokens", 6000))

# the compaction threshold for real conversations, in approximate
# tokens. The same precedence as everything else: BEND_THRESHOLD env
# > config.toml compaction_threshold (tokens or "45%" of the window,
# BISE-300) > 80 % of the model's context window (from bise's catalog,
# BISE-150), never above that 80 %: runtime/provider-pure.bend
# `threshold`
def thr_of_cfg(+envv: String, +cfgt: String, +window: U32) -> Nat:
  U32.to_nat(Pvp.threshold(envv, cfgt, window))

# the live system prompt = identity + the exact vibe_sdk sections
# (tool_use_prompt + current_time_prompt), loaded from prompt-*.txt;
# a solo session also gets the tone block (prompt-tone.txt, Rcp.solo_tone)
# right after the identity
def prompt_read(+path: String) -> IO(String):
  P.read_or(path, "")

# the identity names the product (bise), not the runtime under it
def identity() -> String:
  "You are a precise assistant in bise. Discover additional tools with `search_tool_functions`, and call them using `run_typescript`."

# what bise is and what it is for, right after the identity: every
# agent (main, tasks) reads the same paragraph; a solo session (no role)
# is also told it works alone
def about() -> String:
  "## What bise is\n\nbise is a multi-agent harness, made for humans: a terminal app with one thread per repo. The user talks to `main`; main turns their ideas into tasks and runs one agent per task, in parallel; the agents coordinate with the `sb` command. What the user gets: they stay in flow, they are interrupted only when a decision needs them, and no work gets lost."

def about.pick(solo: Bool) -> String:
  match solo:
    case True{}:
      String.append(about(), "\n\nThis session is a solo one: you work alone with the user, without main or other agents.")
    case False{}:
      about()

def about_for(+extra: String) -> String:
  about.pick(String.is_empty(String.trim(extra)))

# ab: the bise paragraph (about_for); gs: the tool groups subsection
# (Mcpp.groups_section), right after the tool-use section it belongs to
# ("" adds nothing)
def prompt_join(+ab: String, +tone: String, +tu: String, +gs: String, +ct: String) -> String:
  String.append(
    Rcp.prompt_extra(Rcp.prompt_extra(identity(), ab), tone),
    String.append("\n\n", String.append(Rcp.prompt_extra(tu, gs), String.append("\n\n", ct))))

# the live system prompt = identity + the exact vibe_sdk sections
# (tool_use_prompt + current_time_prompt), then the skills sections
# (rules before the <available-skills> catalog, the vibe_sdk order);
# both only when the skills index is non-empty
def live_prompt.go(+base: String, +content: String) -> String:
  Skp.prompt_with(base, content)

# BEND_EXTRA_PROMPT: a file whose content ends the system prompt (a
# switchboard agent's role); unset or missing: nothing is added
def extra_prompt.read(empty: Bool, +path: String) -> IO(String):
  match empty:
    case True{}:
      IO.pure(String, "")
    case False{}:
      prompt_read(path)

def extra_prompt() -> IO(String):
  do IO<String>:
    er : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_EXTRA_PROMPT")
    +path : String <- IO.pure(String, P.env_or(er, ""))
    extra_prompt.read(String.is_empty(path), path)

# BEND_AGENTS_MD: a file with the AGENTS.md section of the agent's
# working folder (BISE-232, written by the hub at each start:
# rust/switchboard/src/agents_md.rs); unset, missing or empty: nothing
def agents_md() -> IO(String):
  do IO<String>:
    ar : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_AGENTS_MD")
    +path : String <- IO.pure(String, P.env_or(ar, ""))
    extra_prompt.read(String.is_empty(path), path)

# BEND_TOOLS_NOTE: whether rg and git are on this machine (BISE-166,
# written by bend-harness / the hub), after the current-time section;
# unset or blank: nothing is added. The AGENTS.md section comes after
# the skills, before the role (the role stays last)
def live_prompt(+cat: List<&2, T.ToolDef>, +patch: Bool) -> IO(String):
  do IO<String>:
    +tu0 : String <- prompt_read("prompts/prompt-tool-use.txt")
    # the line naming the edit tools that are on (core/edit.bend)
    +tu : String <- IO.pure(String, Rcp.prompt_extra(tu0, Ed.prompt_line(patch)))
    +ct : String <- prompt_read("prompts/prompt-current-time.txt")
    +tone : String <- prompt_read("prompts/prompt-tone.txt")
    tn : Result<&1, &1, U32 & String, String> <- IO.get_env("BEND_TOOLS_NOTE")
    +note : String <- IO.pure(String, P.env_or(tn, ""))
    +content : String <- Sk.prompt_content()
    +extra : String <- extra_prompt()
    +amd : String <- agents_md()
    mi : Maybe<&2, String> <- Mcp.index_content()
    +gs : String <- IO.pure(String, Mcpp.groups_section(Mcpp.merged_tools.live(cat, mi)))
    IO.pure(String, Rcp.prompt_extra(Rcp.prompt_extra(live_prompt.go(prompt_join(about_for(extra), Rcp.solo_tone(extra, tone), tu, gs, Rcp.prompt_extra(ct, note)), content), amd), extra))

def live_cfg(+threshold: Nat, +keep: Nat) -> IO(T.Cfg):
  do IO<T.Cfg>:
    # one edit toolset, by the provider of the model this start runs
    # (a later /model switch re-tools each request: main.bend retool)
    +model : String <- Pv.resolved_model()
    +patch : Bool <- IO.pure(Bool, Ed.is_patch_provider(Ed.provider_of(model)))
    +cat : List<&2, T.ToolDef> <- X.catalog_for(patch)
    +p : String <- live_prompt(cat, patch)
    IO.pure(T.Cfg, T.Cfg{threshold, keep, 3n, p, cat})

def live_main() -> IO(Unit):
  do IO<Unit>:
    # the config template lands on disk on first run, before anything
    # reads it
    dbg : Unit <- Dbg.start()
    ut : Unit <- St.write_cfg_template()
    env : Result<&1, &1, U32 & String, String> <- IO.get_env(
      "BEND_THRESHOLD")
    +cfgt : String <- St.read_cfg()
    +window : U32 <- Pv.resolved_window()
    +threshold : Nat <- IO.pure(Nat, thr_of_cfg(P.env_or(env, ""), cfgt, window))
    kenv : Result<&1, &1, U32 & String, String> <- IO.get_env(
      "BEND_COMPACT_KEEP")
    +keep : Nat <- IO.pure(Nat, keep_of_cfg(P.env_or(kenv, ""), cfgt))
    +p : U32 <- Rc.repl_port(7702)
    l : Listener <- IO.try(Listener, TCP.listen("127.0.0.1", p))
    # the single source of truth for every client: printed BEFORE the
    # banner the parent waits for
    +model : String <- Pv.resolved_model()
    +info : String <- R.harness_info(model, Nat.show(threshold))
    ui : Unit <- IO.print(info)
    IO.print(String.append("bend-harness LIVE REPL on 127.0.0.1:",
      String.append(U32.show(p),
        String.append(" (real model, FS tools, bash) — compaction threshold: ",
          String.append(Nat.show(threshold), " tokens (at most 80% of the model's context; BEND_THRESHOLD or config.toml compaction_threshold to change)")))))
    # the plugins bridge, then the skills scan (it reads the plugin
    # skills), then the prompt (it lists the skills)
    u5 : Unit <- Pl.start()
    u4 : Unit <- Sk.skills_scan()
    +cfg : T.Cfg <- live_cfg(threshold, keep)
    +ld : P.Load <- P.load(T.new_session(cfg))
    kr : Result<&1, &1, U32 & String, String> <- IO.get_env("MISTRAL_API_KEY")
    +key : String <- IO.pure(String, P.env_or(kr, ""))
    u : Unit <- Mcp.bootstrap(key)
    u3 : Unit <- Sh.bash_cleanup(U32.show(p))
    f : Rc.Flow <- Rc.serve(True{}, l, Rc.ld_greet(ld), Rc.ld_sess(ld))
    # Stop (a /reload) lands here: main returns, the process exits 0,
    # and the parent respawns the latest code on the same session.
    # The marker line lets the parent tell a reload from a crash.
    u2 : Unit <- Rc.flow_note(f)
    IO.pure(Unit, Unit{})

# BISE_ONESHOT=<request file> (BISE-126): one provider call, its answer on
# stdout, exit (runtime/oneshot.bend); else the live REPL
@unsafe
def main.pick(empty: Bool, +path: String) -> IO(Unit):
  match empty:
    case True{}:
      live_main()
    case False{}:
      Os.run(path)

@unsafe
def main() -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env("BISE_ONESHOT")
    +path : String <- IO.pure(String, P.env_or(r, ""))
    main.pick(String.is_empty(path), path)
