# bend/laws/tools.bend: skills, /reload, self.reload, edit, write_file and
# read_file, the API mapping, call ids, intents. Open claims, each one
# closed by bend/proofs/tools.bend (moved from LAWS.bend). The gate
# checks every bend/proofs/*.bend: a law here with no proof there fails
# it.
import Base
import ../core/types.bend as T
import ../core/compaction.bend as C
import ../core/checkpoint.bend as K
import ../core/session.bend as S
import ../core/edit.bend as Ed
import ../core/read.bend as Rd
import ../core/commands.bend as Cm
import ../core/wire.bend as W
import ../core/oai-chat.bend as Oai
import ../core/anthropic.bend as Anth
import ../runtime/remote.bend as Rm
import ../runtime/tools-pure.bend as Xt
import ../runtime/main-pure.bend as Rt
import ../runtime/selftools-pure.bend as Sf
import ../runtime/mcp-pure.bend as Mc
import ../runtime/skills-pure.bend as Sk
import ../vendor/json.bend as J
import ../runtime/gate-pure.bend as Gt

# ---- Agent Skills (the Vibe CLI design: progressive disclosure) ----
# The prompt carries name+description+path only; the `skill` tool loads
# the full SKILL.md on exact-name demand.

# LAW: the rules section is the vibe_sdk contract: proactive loading,
# exact names, and no re-reading the SKILL.md root.
law skills_rules_contract:
  {Bool.and(
     String.contains(Sk.skills_rules(), "## Available Skills"),
     Bool.and(
       String.contains(Sk.skills_rules(),
         "proactively call the `skill` tool with its exact name"),
       String.contains(Sk.skills_rules(),
         "Do not read a root `SKILL.md` path with `bash`"))) == True{} : Bool}

# LAW: XML escaping per vibe_sdk prompt.rs: &, <, >, ", '.
law skills_escape_xml:
  {Sk.escape_xml("a<b>&c\"d'e")
    == "a&lt;b&gt;&amp;c&quot;d&apos;e" : String}

# LAW: one index line renders exactly one vibe_sdk catalog entry.
law skills_entry_format:
  {Sk.skill_entry(Sk.Skill{"review", "Review code.", "/skills/review/SKILL.md"})
    == "  <skill>\n    <name>review</name>\n    <description>Review code.</description>\n    <path>/skills/review/SKILL.md</path>\n  </skill>" : String}

# LAW: the catalog section wraps the entries, deduped by name, first
# occurrence winning.
law skills_catalog_dedupes:
  {Sk.skills_catalog("one\td1\t/p1\none\td2\t/p2\ntwo\td3\t/p3")
    == "<available-skills>\n  <skill>\n    <name>one</name>\n    <description>d1</description>\n    <path>/p1</path>\n  </skill>\n  <skill>\n    <name>two</name>\n    <description>d3</description>\n    <path>/p3</path>\n  </skill>\n</available-skills>" : String}

# LAW: an empty index yields no section at all (the prompt stays clean).
law skills_catalog_empty_is_empty:
  {Sk.skills_catalog("") == "" : String}

# LAW: the startup scan's note is what it printed, its status line
# dropped: a clean scan says nothing, a skipped file is named once.
law skills_scan_note_quiet:
  {Sk.scan_note("0
") == "" : String}

law skills_scan_note_names_the_skipped:
  {Sk.scan_note("0
skills scan: skipped /h/.vibe/skills/x/SKILL.md (no answer in 3 s)
")
    == "skills scan: skipped /h/.vibe/skills/x/SKILL.md (no answer in 3 s)
" : String}

# LAW: blank or malformed lines never become skills.
law skills_malformed_lines_skipped:
  {Sk.skill_ok_line("") == False{} : Bool}

# LAW: the args are the exact vibe_sdk schema: {"name": "..."}.
law skills_args_name_json:
  {Sk.args_name("{\"name\":\"code-quality\"}") == Some{"code-quality"}
    : Maybe<&2, String>}

# LAW: a bare name (not JSON) is the name, trimmed.
law skills_args_name_bare:
  {Sk.args_name(" bise-demo ") == Some{"bise-demo"} : Maybe<&2, String>}

# LAW: a missing or empty name is a usage error, never a lookup.
law skills_args_name_rejects_empty:
  {Sk.args_name("{}") == None{} : Maybe<&2, String>}

# LAW: a miss returns the vibe_sdk contract: the name, the guidance,
# and the available list.
law skills_miss_lists_available:
  {Bool.and(
     String.contains(Sk.not_available("bogus", "one\td1\t/p1"),
       "Skill \"bogus\" is not available"),
     String.contains(Sk.not_available("bogus", "one\td1\t/p1"),
       "Available skills: \"one\".")) == True{} : Bool}

# LAW: the live search surface carries the live catalog itself when
# the MCP index is absent (skill stays discoverable).
law merged_surface_carries_live_catalog:
  {Mc.merged_tools.live(Con{T.ToolDef{"skill", "d"}, Nil{}}, None{})
    == Con{T.ToolDef{"skill", "d"}, Nil{}} : List<&2, T.ToolDef>}

# LAW: an exact name finds the skill (first hit wins).
law skills_find_exact:
  {Sk.find_skill("one\td1\t/p1\ntwo\td2\t/p2", "two")
    == Some{Sk.Skill{"two", "d2", "/p2"}} : Maybe<&2, Sk.Skill>}

# LAW: the tool returns the CONTENT only — the YAML frontmatter
# never reaches the model (vibe_sdk ProjectSkill).
law skills_strip_frontmatter:
  {Sk.strip_frontmatter("---\nname: x\ndescription: y\n---\n\n# Body\nText")
    == "# Body\nText" : String}

# LAW: the prompt only gains the sections when skills exist.
law skills_prompt_gated:
  {Bool.and(
     String.eq(Sk.prompt_with("base", ""), "base"),
     String.contains(Sk.prompt_with("base", "one\td1\t/p1"),
       "<available-skills>")) == True{} : Bool}

# ---- /reload (the self-improvement loop: restart on the same session) ----

# LAW: /reload maps onto the reload protocol word, sent verbatim.
law slash_reload_maps:
  {Cm.interpret("/reload") == Cm.Send{"reload"} : Cm.Interpretation}

# LAW: the raw word "reload" passes through untouched.
law reload_raw_word:
  {Cm.interpret("reload") == Cm.Send{"reload"} : Cm.Interpretation}

# LAW: a fresh session is between turns: the reload is allowed.
law reload_allowed_when_fresh:
  for +cfg: T.Cfg
  {S.reload_allowed(T.new_session(cfg)) == True{} : Bool}

# LAW: a running turn blocks the reload — the checkpoint only
# serializes turn boundaries (ADR 0002).
law reload_refused_while_turn:
  for +cfg: T.Cfg
  for +text: String
  {S.reload_allowed(S.start_turn(text, T.new_session(cfg)))
    == False{} : Bool}

# LAW: a queued message no longer blocks the reload: it is serialized
# (QUEUE lines, v2) and the restarted process pops it into the turn it
# owes, by itself.
law reload_allows_with_queue:
  for +cfg: T.Cfg
  for +text: String
  {S.reload_allowed(S.queue_msg(T.new_session(cfg), text))
    == True{} : Bool}

# LAW: a held notification no longer blocks the reload either: it is
# serialized (NOTIF lines, v2).
law reload_allows_with_notif:
  for +cfg: T.Cfg
  for +text: String
  {S.reload_allowed(S.hold_notif(T.new_session(cfg), text))
    == True{} : Bool}

# ---- self.reload / self.compact: the agent files requests mid-turn ----
# Called inside a run_typescript program (no top-level tools since
# BISE-293), answered with an ack, executed at the turn boundary by
# the REPL.

# LAW: self.reload answers with the reload ack — visible to the model.
law self_reload_acks:
  {Sf.self_exec("self.reload") == Xt.Ex{True{}, Sf.RELOAD_ACK()}
    : Xt.ExecRes}

# LAW: self.compact answers with the compact ack.
law self_compact_acks:
  {Sf.self_exec("self.compact") == Xt.Ex{True{}, Sf.COMPACT_ACK()}
    : Xt.ExecRes}

# LAW: the routing recognizes exactly the two self.* tools; self.sleep
# is a programmatic tool (inside run_typescript programs), the
# ordinary tools stay themselves.
law self_tool_routing_table:
  {Bool.and(Sf.is_self_tool("self.reload"),
     Bool.and(Sf.is_self_tool("self.compact"),
       Bool.and(Bool.not(Sf.is_self_tool("self.sleep")),
         Bool.not(Sf.is_self_tool("bash"))))) == True{} : Bool}

# LAW: self.sleep is programmatic now: the bounds are 0 < s < 20
# (a longer sleep stalls the program round for no benefit).
law sleep_program_bounds:
  {Bool.and(Rt.prog_sleep.bounds(1),
     Bool.and(Bool.not(Rt.prog_sleep.bounds(0)),
       Bool.not(Rt.prog_sleep.bounds(20)))) == True{} : Bool}

# LAW: the seconds parse from the bare argument string (self.sleep(19)
# in a program) when the JSON parse finds nothing.
law sleep_program_secs:
  {Rt.prog_sleep.secs(None{}, "19") == 19 : U32}

# LAW: the acks carry their scan markers — what deferred_of finds is
# exactly what the model read.
law self_acks_carry_markers:
  {Bool.and(String.contains(Sf.RELOAD_ACK(), "reload: scheduled"),
     String.contains(Sf.COMPACT_ACK(), "compact: scheduled"))
    == True{} : Bool}

# LAW: the deferred scan reads the turn that just ended — the tool
# results after the last user message.
law deferred_reads_reload_request:
  {Sf.deferred_of(Con{T.user("hi"), Con{T.tool_res(Sf.RELOAD_ACK()), Nil{}}})
    == Sf.Def{False{}, True{}} : Sf.Def}

law deferred_reads_compact_request:
  {Sf.deferred_of(Con{T.user("hi"), Con{T.tool_res(Sf.COMPACT_ACK()), Nil{}}})
    == Sf.Def{True{}, False{}} : Sf.Def}

law deferred_reads_both_requests:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(Sf.RELOAD_ACK()),
        Con{T.tool_res(Sf.COMPACT_ACK()), Nil{}}}})
    == Sf.Def{True{}, True{}} : Sf.Def}

# LAW: a request older than the last user message is spent — an old
# request can never fire twice.
law deferred_resets_at_user:
  {Sf.deferred_of(Con{T.tool_res(Sf.RELOAD_ACK()),
      Con{T.user("new turn"), Con{T.asst("hi", Nil{}), Nil{}}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: a message injected mid-turn (an agent's report steered in right
# after the ack) does not spend the request: the turn still compacts
# at its end. It once did: main's self.compact was lost that way.
law deferred_survives_injected_message:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(Sf.COMPACT_ACK()),
        Con{T.injected_user("<agent_message>report</agent_message>"),
          Con{T.asst("done", Nil{}), Nil{}}}}})
    == Sf.Def{True{}, False{}} : Sf.Def}

# LAW: a turn's final reply ends it: a later turn started by an
# injected message (an agent's report waking main) does not fire the
# earlier turn's request again.
law deferred_resets_after_final_reply:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(Sf.RELOAD_ACK()),
        Con{T.asst("done", Nil{}),
          Con{T.injected_user("<agent_message>report</agent_message>"),
            Con{T.asst("noted", Nil{}), Nil{}}}}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: an assistant message WITH tool calls does not end the turn: the
# request filed before it still stands.
law deferred_survives_tool_calls:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(Sf.COMPACT_ACK()),
        Con{T.asst("more", Con{T.Call{1, "bash", "ls"}, Nil{}}),
          Con{T.tool_res("ok"), Nil{}}}}})
    == Sf.Def{True{}, False{}} : Sf.Def}

# LAW: no history, no requests.
law deferred_empty_history:
  {Sf.deferred_of(Nil{}) == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: no requests, no suffix on the program result.
law def_acks_suffix_empty_when_none:
  {Sf.def_acks.suffix(Sf.Def{False{}, False{}}) == "" : String}

# LAW: a reload filed inside a program rides its result — even when
# the program returns something else.
law def_acks_suffix_carries_reload:
  {String.contains(Sf.def_acks.suffix(Sf.Def{False{}, True{}}),
     "reload: scheduled") == True{} : Bool}

# LAW: both requests filed in one program both ride the result.
law def_acks_suffix_carries_both:
  {Bool.and(
     String.contains(Sf.def_acks.suffix(Sf.Def{True{}, True{}}),
       "compact: scheduled"),
     String.contains(Sf.def_acks.suffix(Sf.Def{True{}, True{}}),
       "reload: scheduled")) == True{} : Bool}

# LAW: the live catalog carries ONE edit toolset (design §2.2, the
# user's decision): Vibe's edit and write_file for every provider but
# OpenAI, apply_patch for OpenAI, never both. They come last in the
# catalog: the wire reverses it, so a request lists them first, before
# bash (steering). Nothing after them: the self.* requests are
# programmatic only (BISE-293). catalog_short (the scripted scenarios)
# stays untouched.
law live_catalog_edit_toolset_has_seven_tools:
  {Xt.catalog_len(Xt.catalog_live("s", "r", "b", "k", "p", False{})) == 7n : Nat}

law live_catalog_patch_toolset_has_six_tools:
  {Xt.catalog_len(Xt.catalog_live("s", "r", "b", "k", "p", True{})) == 6n : Nat}

law parity_catalog_keeps_three:
  {Xt.catalog_len(Xt.catalog_pure("s", "r", "b")) == 3n : Nat}

law live_catalog_edit_names:
  {String.join([Xt.tool_name_at(0n, Xt.catalog_live("s", "r", "b", "k", "p", False{})),
     Xt.tool_name_at(1n, Xt.catalog_live("s", "r", "b", "k", "p", False{})),
     Xt.tool_name_at(2n, Xt.catalog_live("s", "r", "b", "k", "p", False{})),
     Xt.tool_name_at(3n, Xt.catalog_live("s", "r", "b", "k", "p", False{})),
     Xt.tool_name_at(4n, Xt.catalog_live("s", "r", "b", "k", "p", False{})),
     Xt.tool_name_at(5n, Xt.catalog_live("s", "r", "b", "k", "p", False{})),
     Xt.tool_name_at(6n, Xt.catalog_live("s", "r", "b", "k", "p", False{}))], ",")
    == "search_tool_functions,bash,run_typescript,skill,write_file,edit,read_file" : String}

law live_catalog_patch_names:
  {String.join([Xt.tool_name_at(0n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(1n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(2n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(3n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(4n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(5n, Xt.catalog_live("s", "r", "b", "k", "p", True{}))], ",")
    == "search_tool_functions,bash,run_typescript,skill,apply_patch,read_file" : String}

# LAW: apply_patch rides with its description argument, and the
# fallback description names the format and the atomicity.
law apply_patch_desc_names_the_format:
  {Bool.and(
     String.eq(Xt.tool_desc_at(4n, Xt.catalog_live("s", "r", "b", "k", "p", True{})), "p"),
     Bool.and(
       String.contains(Xt.patch_desc.read(None{}), "Codex V4A"),
       String.contains(Xt.patch_desc.read(None{}),
         "validated before anything is written"))) == True{} : Bool}

# LAW: edit and write_file carry Vibe 2.25.8's descriptions verbatim
# (prompts/edit.md, prompts/write_file.md).
law edit_tools_carry_vibe_descriptions:
  {String.join([Xt.tool_desc_at(5n, Xt.catalog_live("s", "r", "b", "k", "p", False{})),
     Xt.tool_desc_at(4n, Xt.catalog_live("s", "r", "b", "k", "p", False{}))], "|")
    == "Exact string replacement in a file. You must `read_file` first. When editing text from `read_file` output, never include any part of the line-number prefix in `old_string` or `new_string`. If `old_string` is not found or matches multiple locations, provide more context to make it unique, or use `replace_all`. If an edit fails, re-read the file before retrying.|Create a new file. Errors if the file already exists — use `edit` to modify existing files. Prefer editing existing files over creating new ones. Do not proactively create documentation or README files." : String}

# LAW: bash's description names the edit tool that is on, in place of
# its {edit_line} marker (and nothing when the text has none).
law bash_desc_names_the_edit_tool:
  {String.join([Xt.tool_desc_at(1n, Xt.catalog_live("s", "r", "x {edit_line} y", "k", "p", False{})),
     Xt.tool_desc_at(1n, Xt.catalog_live("s", "r", "x {edit_line} y", "k", "p", True{})),
     Xt.bash_for("plain", False{})], "|")
    == "x Not for editing files: use `edit`. y|x Not for editing files: use `apply_patch`. y|plain" : String}

# LAW: which toolset a catalog carries; the scripted one carries none.
law catalog_toolset_read_back:
  {(Xt.toolset(Xt.catalog_live("s", "r", "b", "k", "p", True{})),
    (Xt.toolset(Xt.catalog_live("s", "r", "b", "k", "p", False{})),
     Xt.toolset(Xt.catalog_pure("s", "r", "b"))))
    == (Some{True{}}, (Some{False{}}, None{}))
    : Maybe<&2, Bool> & (Maybe<&2, Bool> & Maybe<&2, Bool>)}

# LAW: the OpenAI provider (the id before the model id's first '/')
# is the only one that gets apply_patch; OpenRouter's openai/ models
# ride OpenRouter, so they get edit.
law patch_provider_is_openai_only:
  {(Ed.is_patch_provider(Ed.provider_of("openai/gpt-5")),
    (Ed.is_patch_provider(Ed.provider_of("openrouter/openai/gpt-5")),
     (Ed.is_patch_provider(Ed.provider_of("anthropic/claude-x")),
      (Ed.is_patch_provider(Ed.provider_of("mistral/devstral")),
       Ed.is_patch_provider(Ed.provider_of(""))))))
    == (True{}, (False{}, (False{}, (False{}, False{}))))
    : Bool & (Bool & (Bool & (Bool & Bool)))}

# LAW: the prompt line, the bash line and auto's deny-once hint name the
# tool that is on, never the other one (design §2.2).
law edit_steering_lines:
  {String.join([Ed.prompt_line(False{}), Ed.prompt_line(True{}),
     Ed.deny_once_hint(False{}), Ed.deny_once_hint(True{})], "|")
    == "Read files with `read_file`, edit and create them with `edit` and `write_file`. Don't edit files through bash scripts (python or perl, `sed` on many files): in auto they need the user, and tool edits show as diffs.|Read files with `read_file`, edit and create them with `apply_patch`. Don't edit files through bash scripts (python or perl, `sed` on many files): in auto they need the user, and tool edits show as diffs.|auto: this bash call needs the user. Use `edit`: it runs without asking. If bash is really needed, repeat the call and the user will be asked.|auto: this bash call needs the user. Use `apply_patch`: it runs without asking. If bash is really needed, repeat the call and the user will be asked." : String}

# LAW: a provider switch re-tools the request: the tool lines and the
# prompt's edit line swapped, the history untouched.
law retool_swaps_tools_and_line:
  {Rt.retool("MODEL default\nTOOL a : x\nTOOL apply_patch : p\n\nMSG system : hi L1 end\nMSG assistant : L1\nEND\n",
      [T.ToolDef{"a", "x"}, T.ToolDef{"apply_patch", "p"}],
      [T.ToolDef{"a", "x"}, T.ToolDef{"edit", "e"}, T.ToolDef{"write_file", "w"}],
      "L1", "L2")
    == "MODEL default\nTOOL a : x\nTOOL edit : e\nTOOL write_file : w\n\nMSG system : hi L2 end\nMSG assistant : L1\nEND\n" : String}

# ---- Vibe's edit and write_file (core/edit.bend) ----

# LAW: every error and result string is Vibe 2.25.8's, byte for byte
# (vibe/core/tools/builtins/edit.py, write_file.py).
law edit_err_empty_path:
  {Ed.err_empty_path() == "File path cannot be empty" : String}

law edit_err_empty_old:
  {Ed.err_empty_old() == "old_string cannot be empty. Use write_file to create new files." : String}

law edit_err_same:
  {Ed.err_same() == "No changes to make — old_string and new_string are identical" : String}

law edit_err_missing:
  {Ed.err_missing("/w/a.txt") == "File does not exist: /w/a.txt" : String}

law edit_err_not_file:
  {Ed.err_not_file("/w") == "Path is not a file: /w" : String}

# LAW: the two errors that ended with Vibe's "String: <old_string>" say
# lines instead (the model's call has old_string): not found, where its
# first line sits, if anywhere; several matches, where they are.
law edit_err_not_found:
  {(Ed.err_not_found([]), Ed.err_not_found([12n, 40n]))
    == ("String to replace not found in file.",
        "String to replace not found in file.\nIts first line is at lines 12, 40.")
    : String & String}

law edit_err_many:
  {Ed.err_many(3n, [2n, 2n, 9n]) == "Found 3 matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.\nAt lines 2, 9." : String}

law edit_err_not_text:
  {Ed.err_not_text("/w/b.bin", 2n) == "Cannot edit /w/b.bin: file is not valid text (utf-8, byte 2)" : String}

law edit_err_permission:
  {Ed.err_permission("/w/a") == "Permission denied accessing file: /w/a" : String}

law edit_ok_messages:
  {String.join([Ed.ok_msg(False{}), Ed.ok_msg(True{})], "|")
    == "The file has been updated successfully.|The file has been updated. All occurrences were successfully replaced" : String}

# LAW: an edit's result is the file, Vibe's message, the replacements
# and the lines the new text takes; never old_string or new_string (the
# model's call has them; edit_result takes neither, so no result can
# depend on them). Many replacements: 3 spans at most, then
# "(+N more)"; two on one line count as one span.
law edit_result_text:
  {(Ed.edit_result("/w/a", Ed.ok_one(), [3n], 2n),
    (Ed.edit_result("/w/a", Ed.ok_all(), [1n, 1n, 4n, 9n, 12n, 20n], 1n),
     Ed.edit_result("/w/a", Ed.ok_one(), [7n], 0n)))
    == ("file: /w/a\nmessage: The file has been updated successfully.\nreplacements: 1\nlines: 3-4",
        ("file: /w/a\nmessage: The file has been updated. All occurrences were successfully replaced\nreplacements: 6\nlines: 1, 4, 9 (+2 more)",
         "file: /w/a\nmessage: The file has been updated successfully.\nreplacements: 1\nlines: 7"))
    : String & (String & String)}

law write_err_empty_path:
  {Ed.werr_empty_path() == "Path cannot be empty" : String}

law write_err_too_big:
  {Ed.werr_too_big() == "Content exceeds 64000 bytes limit" : String}

law write_err_exists:
  {Ed.werr_exists("/w/a") == "File '/w/a' already exists. Use edit to modify it." : String}

# LAW: write_file's result is the path, the bytes and the lines; never
# the content (write_result does not take it).
law write_result_text:
  {Ed.write_result("/w/n.txt", Ed.utf8_len("hé\n"), Ed.text_lines("hé\n"))
    == "file_path: /w/n.txt\nbytes_written: 4\nlines: 1" : String}

law text_lines_counts:
  {(Ed.text_lines(""), (Ed.text_lines("a"), (Ed.text_lines("a\n"), Ed.text_lines("a\nb"))))
    == (0n, (1n, (1n, 2n))) : Nat & (Nat & (Nat & Nat))}

# LAW: the argument checks run in Vibe's order: the path, an empty
# old_string, then identical strings; a fine call passes.
law edit_check_args_order:
  {(Ed.check_args("  ", "", "a"),
    (Ed.check_args("/a", "", "a"),
     (Ed.check_args("/a", "x", "x"), Ed.check_args("/a", "x", "y"))))
    == (Some{"File path cannot be empty"},
        (Some{"old_string cannot be empty. Use write_file to create new files."},
         (Some{"No changes to make — old_string and new_string are identical"}, None{})))
    : Maybe<&2, String> & (Maybe<&2, String> & (Maybe<&2, String> & Maybe<&2, String>))}

law write_check_order:
  {(Ed.check_write(" ", "x"), Ed.check_write("/a", "x"))
    == (Some{"Path cannot be empty"}, None{}) : Maybe<&2, String> & Maybe<&2, String>}

# LAW: the replacement: not found, several matches without replace_all,
# the first match only, or all of them (Python's str.replace).
law edit_apply_cases:
  {(Ed.apply("a b a", "c", "d", False{}),
    (Ed.apply("a b a", "a", "z", False{}),
     (Ed.apply("a b c", "b", "zz", False{}),
      Ed.apply("aaa b aa", "aa", "x", True{}))))
    == (Ed.EdFail{"String to replace not found in file."},
        (Ed.EdFail{"Found 2 matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.\nAt line 1."},
         (Ed.EdOk{"a zz c", "The file has been updated successfully.", [1n]},
          Ed.EdOk{"xa b x", "The file has been updated. All occurrences were successfully replaced", [1n, 1n]})))
    : Ed.Edited & (Ed.Edited & (Ed.Edited & Ed.Edited))}

# LAW: the lines: a replacement's start in the text after the edit (a
# multi-line new_string pushes the later ones down); the matches' lines
# in the text for several matches; not found, the lines of old_string's
# first line, unless it is under 4 non-blank characters ("}").
law edit_apply_lines:
  {(Ed.apply("one\ntwo\nthree\nfour\n", "two\nthree", "2", False{}),
    (Ed.apply("x\ny\nx\ny\nx\n", "x", "a\nb", True{}),
     (Ed.apply("a\nb\na\n", "a", "z", False{}),
      (Ed.apply("fn main() {\n  go();\n}\n", "fn main() {\n  run();", "z", False{}),
       Ed.apply("}\n}\n", "}\n  x", "z", False{})))))
    == (Ed.EdOk{"one\n2\nfour\n", "The file has been updated successfully.", [2n]},
        (Ed.EdOk{"a\nb\ny\na\nb\ny\na\nb\n", "The file has been updated. All occurrences were successfully replaced", [1n, 4n, 7n]},
         (Ed.EdFail{"Found 2 matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.\nAt lines 1, 3."},
          (Ed.EdFail{"String to replace not found in file.\nIts first line is at line 1."},
           Ed.EdFail{"String to replace not found in file."}))))
    : Ed.Edited & (Ed.Edited & (Ed.Edited & (Ed.Edited & Ed.Edited)))}

# LAW: a CRLF file is edited as "\n" text and written back in CRLF; an
# LF file stays LF.
law edit_keeps_crlf:
  {(Ed.nl_style("a\r\nb\r\n"), (Ed.nl_style("a\nb"),
    (Ed.normalize("a\r\nb\r\n"), Ed.restore("x\ny\n", Ed.nl_style("a\r\nb\r\n")))))
    == ("\r\n", ("\n", ("a\nb\n", "x\r\ny\r\n"))) : String & (String & (String & String))}

# LAW: the first byte that is not UTF-8 (Python's start offset).
law utf8_bad_offsets:
  {(Ed.utf8_bad([97, 98, 255, 99]), (Ed.utf8_bad([97, 226, 130]),
    (Ed.utf8_bad([104, 195, 169, 226, 130, 172]), Ed.utf8_bad([226, 40, 161]))))
    == (Some{2n}, (Some{1n}, (None{}, Some{0n})))
    : Maybe<&2, Nat> & (Maybe<&2, Nat> & (Maybe<&2, Nat> & Maybe<&2, Nat>))}

# LAW: a path resolves like Vibe's: relative in the working folder,
# `~/` in the home folder, "." and ".." gone.
law edit_path_resolves:
  {String.join([Ed.resolve("/w", "/h", "a/../b.txt"), Ed.resolve("/w", "/h", "~/x"),
     Ed.resolve("/w", "/h", "/abs/./y"), Ed.resolve("/w/sub", "", " ../z ")], "|")
    == "/w/b.txt|/h/x|/abs/y|/w/z" : String}

# LAW: the approvals gate (approvals-design.md §3, §10): the wrappers are
# never gated (run_typescript's own calls are, one by one); every other
# tool is: bash, the edit tools, a connector.
law gate_what_is_gated:
  {(Gt.gated("run_typescript"), Gt.gated("search_tool_functions"), Gt.gated("skill"),
    Gt.gated("self.sleep"), Gt.gated("bash"), Gt.gated("edit"), Gt.gated("gmail.send_email"))
    == (False{}, False{}, False{}, False{}, True{}, True{}, True{})
    : Bool & Bool & Bool & Bool & Bool & Bool & Bool}

# LAW: only `auto` in the mode file asks; `yolo`, no file (no hub) or
# anything else never does (yolo costs no round trip).
law gate_only_auto_asks:
  {(Gt.mode_asks(Some{"auto\n"}), Gt.mode_asks(Some{"yolo\n"}), Gt.mode_asks(None{}), Gt.mode_asks(Some{""}))
    == (True{}, False{}, False{}, False{}) : Bool & Bool & Bool & Bool}

# LAW: the runtime takes the hub's line for its call only: its number and
# its nonce; `allow` may carry flags (the sandbox's `net`), `deny` a reason.
law gate_reads_its_own_verdict:
  {(Gt.verdict("6 x allow\n7 b allow\n", "7", "a"), Gt.verdict("7 a allow\n", "7", "a"),
    Gt.verdict("7 a allow sandbox net\n", "7", "a"), Gt.verdict("7 a deny the user said no: use a branch\n", "7", "a"),
    Gt.verdict("7 a deny\n", "7", "a"))
    == (None{}, Some{Gt.VAllow{""}}, Some{Gt.VAllow{"sandbox net"}}, Some{Gt.VDeny{"the user said no: use a branch"}},
        Some{Gt.VDeny{"the user said no."}})
    : Maybe<&2, Gt.Verdict> & Maybe<&2, Gt.Verdict> & Maybe<&2, Gt.Verdict> & Maybe<&2, Gt.Verdict> & Maybe<&2, Gt.Verdict>}

# LAW: a poll finds the verdict first, else an interrupt (a flag with
# content: the cleared flag is empty), else nothing yet.
law gate_poll_order:
  {(Gt.found(Some{"7 a allow"}, Some{"1"}, "7", "a"), Gt.found(Some{""}, Some{"1"}, "7", "a"),
    Gt.found(None{}, Some{""}, "7", "a"))
    == (Some{Gt.VAllow{""}}, Some{Gt.VStop{"the user"}}, None{})
    : Maybe<&2, Gt.Verdict> & Maybe<&2, Gt.Verdict> & Maybe<&2, Gt.Verdict>}

# LAW: what the call returns: an allow runs, a denial is its reason, an
# interrupt says so; the wire lines.
law gate_results_and_lines:
  {(Gt.denial(Gt.VAllow{"net"}), Gt.denial(Gt.VDeny{"no"}), Gt.denial(Gt.VStop{"the user"}),
    Gt.ask_line("7", "bash", "ls \"x\"", "a"), Gt.done_line("7", Gt.how_of(Gt.VStop{"main"})),
    Gt.poll_ms(0n), Gt.poll_ms(10n))
    == (None{}, Some{"no"}, Some{"interrupted by the user"},
        "gate 7 {\"tool\":\"bash\",\"args\":\"ls \\\"x\\\"\",\"nonce\":\"a\"}", "gate-done 7 interrupted",
        20, 100)
    : Maybe<&2, String> & Maybe<&2, String> & Maybe<&2, String> & String & String & U32 & U32}

# LAW: a bash call the sandbox stopped asks again with its output (the
# last 4000 chars) as "denied"; a no keeps the first output, then why.
law gate_sandbox_rerun_line:
  {(Gt.ask_line_x("8", "bash", "cp a ~/x", "b", Gt.denied_extra("cp: /h/x: Operation not permitted")),
    Gt.stopped_result("cp: /h/x: Operation not permitted\n", "stopped by the sandbox."))
    == ("gate 8 {\"tool\":\"bash\",\"args\":\"cp a ~/x\",\"nonce\":\"b\",\"denied\":\"cp: /h/x: Operation not permitted\"}",
        "cp: /h/x: Operation not permitted\n\nstopped by the sandbox.")
    : String & String}

law gate_sandbox_denied_tail:
  {String.length(Gt.denied_tail(String.concat(["a", "b"]))) == 2n : Nat}

# LAW: the gate (1a/1b) reads {"file_path": <absolute>}.
law edit_gate_args:
  {Ed.gate_args("/w", "/h", Ed.gate_path("edit", "{\"old_string\":\"a\",\"file_path\":\"src/x.rs\",\"new_string\":\"b\"}"))
    == "{\"file_path\":\"/w/src/x.rs\"}" : String}

# LAW: the TUI sees an edit as the V4A patch it amounts to, and a
# write_file as an Add File.
law edit_views_are_patches:
  {(Rt.code_src("edit", "{\"file_path\":\"a.txt\",\"old_string\":\"x\\ny\",\"new_string\":\"z\"}"),
    Rt.code_src("write_file", "{\"file_path\":\"n.txt\",\"content\":\"one\\ntwo\"}"))
    == ("*** Begin Patch\n*** Update File: a.txt\n@@\n-x\n-y\n+z\n*** End Patch",
        "*** Begin Patch\n*** Add File: n.txt\n+one\n+two\n*** End Patch") : String & String}

# LAW: the schemas the providers get are Vibe's.
law edit_schemas:
  {(W.tool_schema("edit"), (W.tool_schema("write_file"), W.tool_schema("apply_patch")))
    == (Ed.edit_schema(), (Ed.write_schema(), W.arg_schema())) : J.Json & (J.Json & J.Json)}

law edit_calls_ride_structured:
  {(W.is_structured("edit"), (W.is_structured("write_file"),
    (W.is_structured("read_file"), W.is_structured("apply_patch"))))
    == (True{}, (True{}, (True{}, False{}))) : Bool & (Bool & (Bool & Bool))}

# LAW: the schema the providers get for read_file is Vibe's pydantic
# shape (file_path, offset, limit).
law read_schema_is_vibes:
  {W.tool_schema("read_file") == Rd.read_schema() : J.Json}

# ---- read_file (core/read.bend, Vibe 2.25.8's read_file.py) ----

# LAW: the description is Vibe's prompts/read_file.md, byte for byte.
law read_desc_is_vibes:
  {Rd.read_desc()
    == "Read a file. Use absolute paths. For large files, use `offset` and `limit` to page through sections. Use `grep` to find specific content instead of reading sequentially. Do not read binary or model weight files (.bin, .safetensors, .pt, .gguf)." : String}

# LAW: Vibe's errors and warnings, byte for byte (its words, bise's
# shown path).
law read_errors:
  {String.join([Rd.err_empty_path(), Rd.err_not_found("/w/a.txt"),
    Rd.err_is_dir("/w"), Rd.err_not_text("/w/b.bin", 2n)], "|")
    == "file_path cannot be empty|File not found at: /w/a.txt|Path is a directory, not a file: /w|Cannot read /w/b.bin: file is not valid text (utf-8, byte 2)" : String}

law read_warnings:
  {String.join([Rd.warn_empty(), Rd.warn_short(5n, 4n)], "|")
    == "<vibe_warning>Warning: the file exists but the contents are empty.</vibe_warning>|<vibe_warning>Warning: the file exists but is shorter than the provided offset (5). The file has 4 lines.</vibe_warning>" : String}

# LAW: the content is Vibe's numbered lines: the number right-justed
# to 9, then U+2192; a window numbers from its start line.
law read_numbers_like_vibe:
  {(Rd.numbered(1n, Con{"one", Con{"two", Nil{}}}),
    Rd.numbered(4n, Con{"four", Nil{}}))
    == ("        1→one\n        2→two", "        4→four") : String & String}

# LAW: the result: the file, the window (first-last of the total), a
# blank line, then the content; a read past the end, of an empty file,
# and the trailing-newline count (4 lines, not 5).
law read_result_windows:
  {(Rd.read_out("l1\nl2\nl3\n", "/w/a.txt", 1n, 2000n),
    Rd.read_out("l1\nl2\nl3\nl4\n", "/w/a.txt", 5n, 2000n),
    Rd.read_out("", "/w/e.txt", 1n, 2000n))
    == (Rd.ROk{"file: /w/a.txt\nlines: 1-3 of 3\n\n        1→l1\n        2→l2\n        3→l3"},
        Rd.ROk{"file: /w/a.txt\nlines: 0 of 4\n\n<vibe_warning>Warning: the file exists but is shorter than the provided offset (5). The file has 4 lines.</vibe_warning>"},
        Rd.ROk{"file: /w/e.txt\nlines: 0 of 0\n\n<vibe_warning>Warning: the file exists but the contents are empty.</vibe_warning>"})
    : Rd.ReadOut & Rd.ReadOut & Rd.ReadOut}

# LAW: offset and limit page the read; a window starts at its offset.
law read_pages:
  {Rd.read_out("a\nb\nc\nd\n", "/w/a.txt", 3n, 1n)
    == Rd.ROk{"file: /w/a.txt\nlines: 3-3 of 4\n\n        3→c"} : Rd.ReadOut}

# LAW: a read over the result cap fails with the size and the way out;
# the cap is the wire's (cap_out), so a read that passes is delivered
# whole.
law read_cap:
  {(Rd.result_cap(), Rd.read_out(String.concat(["x", String.concat(["y", "z"])]), "/w/a.txt", 1n, 2000n))
    == (20000n, Rd.ROk{"file: /w/a.txt\nlines: 1-1 of 1\n\n        1→xyz"}) : Nat & Rd.ReadOut}

# LAW: the args: JSON (Vibe's schema), or a bare path; offset 0 is no
# offset, limit 0 the default 2000; a file_path key wins over path and
# arg.
law read_args_parse:
  {(Rd.read_path(Rd.read_args("{\"file_path\":\"/a\",\"offset\":3,\"limit\":2}")),
    Rd.read_args("/bare"))
    == ("/a", Rd.ReadArgs{"/bare", None{}, 2000n}) : String & Rd.ReadArgs}

law read_args_offset_zero_is_none:
  {(Rd.read_args("{\"file_path\":\"/a\",\"offset\":0}"),
    Rd.read_args("{\"file_path\":\"/a\",\"limit\":0}"))
    == (Rd.ReadArgs{"/a", None{}, 2000n}, Rd.ReadArgs{"/a", None{}, 2000n})
    : Rd.ReadArgs & Rd.ReadArgs}

# LAW: an empty path is refused before the disk is asked.
law read_empty_path_refused:
  {Rd.check_args(" ") == Some{"file_path cannot be empty"} : Maybe<&2, String>}

# LAW: the call line the feed shows: the path, then the window when
# one was asked (Vibe's display).
law read_ann_names_the_window:
  {String.join([Rd.ann_args("{\"file_path\":\"a.rs\"}"),
    Rd.ann_args("{\"file_path\":\"a.rs\",\"offset\":4}"),
    Rd.ann_args("{\"file_path\":\"a.rs\",\"offset\":4,\"limit\":9}")], "|")
    == "a.rs|a.rs from line 4 limit 2000 lines|a.rs from line 4 limit 9 lines" : String}

# ---- the provider API mapping (core/api.bend, formerly the Rust bridge) ----

# LAW: the wire request parses into exactly the tools and messages it
# was built from (oldest-first, calls attached to their message).
law api_wire_parses:
  {W.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n")
    == W.WReq{Con{W.WTool{"echo", "Returns text."}, Nil{}},
      Con{W.WMsg{"user", "bonjour", Nil{}}, Nil{}}, None{}}
    : W.WReq}

# LAW: the API body carries the model, the messages, the tools with the
# flat arg schema, and tool_choice auto — byte-for-byte.
law api_body_maps:
  {Oai.api_body("zai-glm-5-3",
      W.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n"))
    == "{\"model\":\"zai-glm-5-3\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"bonjour\"}],\"tools\":[{\"type\":\"function\",\"function\":{\"name\":\"echo\",\"description\":\"Returns text.\",\"parameters\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]}}}],\"tool_choice\":\"auto\"}"
    : String}

# LAW: GLM 5.3 runs at HIGH reasoning effort on every call — the vibe
# CLI parity (model thinking "high" maps to reasoning_effort "high"
# on the Mistral API, mistral.py _THINKING_TO_REASONING_EFFORT), and
# the compaction call carries it too.
law api_body_high_reasoning:
  {String.contains(Oai.api_body("zai-glm-5-3",
      W.parse_wire("MODEL default\nMSG user : bonjour\nEND\n")),
    "\"reasoning_effort\":\"high\"") == True{} : Bool}

# LAW: the way back — an assistant message whose history text carries
# marked reasoning is re-sent as the vibe CLI ThinkChunk shape: the
# thinking block first, the text block second.
law asst_think_roundtrip:
  {J.stringify(Oai.asst_content_json(
      W.unescape_nl("<think>2+2 = 4\\n</think>\\n4")))
    == "[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}]},{\"type\":\"text\",\"text\":\"4\"}]"
    : String}

# LAW: a thinking-only assistant message carries no text block.
law asst_think_only_no_text_block:
  {J.stringify(Oai.asst_content_json(
      W.unescape_nl("<think>R\\n</think>")))
    == "[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"R\"}]}]"
    : String}

# LAW: a plain assistant message is sent as a plain string, exactly as
# before — the conversion only fires on marked reasoning.
law asst_plain_content_stays_plain:
  {J.stringify(Oai.asst_content_json("hello")) == "\"hello\"" : String}

# LAW: end to end on the request path — a wire carrying a marked
# assistant message produces an API body with the thinking block.
law api_body_returns_thinking_block:
  {String.contains(Oai.api_body("m",
      W.parse_wire("MODEL default\nMSG assistant : <think>2+2 = 4\\n</think>\\n4\nEND\n")),
    "\"type\":\"thinking\"") == True{} : Bool}

# LAW: GLM content blocks become separate lines; the wire carries them
# escaped, the client sees real newlines after unescape.
law glm_blocks_join_lines:
  {W.unescape_nl(Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"a\"},{\"type\":\"text\",\"text\":\"b\"}]}}]}"))
    == "OK a\nb\nEND\n" : String}

# LAW: GLM thinking blocks surface in the reply too -- the parts of the
# nested thinking array wrapped in think markers, ahead of the text
# blocks. The history the model sees back then carries its own
# reasoning; the common harness bug dropped it here, so every later
# hop of a multi-tool turn reasoned blind.
law glm_thinking_surfaces:
  {W.unescape_nl(Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}],\"closed\":true},{\"type\":\"text\",\"text\":\"4\"}]}}]}"))
    == "OK <think>2+2 = 4\n</think>\n4\nEND\n" : String}

# LAW: a GLM thinking block with a signature carries it on the span's
# last line - the same signed transport the Anthropic adapter uses, so
# one extraction serves both providers.
law glm_thinking_signature_surfaces:
  {W.unescape_nl(Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}],\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}]}}]}"))
    == "OK <think>2+2 = 4
BENDSIG::sig</think>
4
END
" : String}

# LAW: a signed span replays as the ThinkChunk shape WITH the signature
# at block level (mistral.py _build_mistral_think_chunk: the metadata
# rides the block, the thinking array stays the nested text parts).
law glm_signed_think_signature_replays:
  {J.stringify(Oai.asst_content_json(
      W.unescape_nl("<think>2+2 = 4
BENDSIG::sig</think>
4")))
    == "[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}],\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}]"
    : String}

# LAW: tool calls map onto the CALL reply lines the Core parses (1-based
# ids, the flat arg rehydrated from the arguments JSON string).
law api_tool_calls_map:
  {Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"arg\\\": \\\"hi\\\"}\"}}]}}]}")
    == "OK \nCALL 1 echo : hi\nEND\n" : String}

# LAW: a model that renames the flat arg (Mistral Large 4 calls bash
# with {"command": ...} whatever the schema says) still gets its command
# run: the call's one string field other than its description.
law api_tool_call_renamed_arg:
  {Oai.reply_of(200,
      "{\"choices\":[{\"message\":{\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"command\\\": \\\"echo hi\\\"}\"}}]}}]}")
    == "OK 
CALL 1 bash : echo hi
END
" : String}

# LAW: a renamed arg with a description keeps the canonical JSON args.
law flat_in_renamed_arg_with_description:
  {W.flat_in("bash", J.obj(Con{J.kv("command", J.str("ls")),
      Con{J.kv("description", J.str("list")), Nil{}}}), "x")
    == "{\"arg\":\"ls\",\"description\":\"list\"}" : String}

# LAW: two string fields and no arg: no guess, the raw args.
law flat_in_two_fields_no_guess:
  {W.flat_in("bash", J.obj(Con{J.kv("a", J.str("ls")),
      Con{J.kv("b", J.str("pwd")), Nil{}}}), "raw") == "raw" : String}

# LAW: a provider error becomes the ERROR reply line, flattened and
# capped at 300 chars, with the status.
law api_error_maps:
  {Oai.reply_of(401, "{\"error\":\"bad key\"}")
    == "ERROR provider 401: bad key\nEND\n" : String}

# ---- permissive compaction + newline transport ----

# LAW: an unclosed <summary> block still yields the summary (a
# truncated close tag no longer fails compaction).
law unclosed_summary_falls_back:
  {C.parse_summary("<summary>texte sans fermeture")
    == Some{"texte sans fermeture"} : Maybe<&2, String>}

# LAW: escaping keeps every wire line single-line, whatever the text.
law escape_keeps_lines_single:
  {String.contains(W.escape_nl("x\ny\nz"), "\n") == False{} : Bool}

# LAW: escaping then unescaping is the identity (concrete).
law escape_roundtrip:
  {W.unescape_nl(W.escape_nl("a\nb\n\nc")) == "a\nb\n\nc" : String}

# LAW: a compaction request (no TOOL lines) produces a body WITHOUT
# tools or tool_choice — the model cannot even offer a tool call.
law api_bare_body_omits_tools:
  {Oai.api_body("m", W.parse_wire("MODEL default\nMSG user : salut\nEND\n"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":\"salut\"}]}"
    : String}

# ---- the on-disk text form (ADR 0002) ----

# LAW: the text form of a checkpoint parses back into exactly that
# checkpoint — roles, injected flags, tool calls, counters and all.
law checkpoint_text_roundtrip:
  {K.from_text(K.to_text(K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.", Con{T.ToolDef{"echo", "Returns text."}, Nil{}}}, Con{T.user("bonjour"), Con{T.asst("réponse", Con{T.Call{1, "echo", "hi"}, Nil{}}), Con{T.tool_res("hi"), Con{T.injected_user("résumé"), Nil{}}}}}, 3n, 7, None{}, None{}, Nil{}, Nil{}})))
    == Some{K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.", Con{T.ToolDef{"echo", "Returns text."}, Nil{}}}, Con{T.user("bonjour"), Con{T.asst("réponse", Con{T.Call{1, "echo", "hi"}, Nil{}}), Con{T.tool_res("hi"), Con{T.injected_user("résumé"), Nil{}}}}}, 3n, 7, None{}, None{}, Nil{}, Nil{}})} : Maybe<&2, K.Checkpoint>}

# LAW: the queue and the held notifications survive the text form too
# (QUEUE/NOTIF lines, wire-encoded) - the message the user queued
# mid-turn is there after a reload, so its turn starts by itself.
law checkpoint_text_roundtrip_queued:
  {K.from_text(K.to_text(K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.",
      Con{T.ToolDef{"echo", "Returns text."}, Nil{}}},
    Con{T.user("bonjour"), Nil{}}, 1n, 0, None{}, None{},
    Con{"next question", Nil{}}, Con{"note one", Nil{}}})))
    == Some{K.export(T.Sess{T.Cfg{400n, 40n, 3n, "You are precise.",
      Con{T.ToolDef{"echo", "Returns text."}, Nil{}}},
    Con{T.user("bonjour"), Nil{}}, 1n, 0, None{}, None{},
    Con{"next question", Nil{}}, Con{"note one", Nil{}}})}
    : Maybe<&2, K.Checkpoint>}

# ---- call-id renumbering (core/session.bend) ----
# The wire pairs tool results with calls by id; provider replies number
# calls per completion, so sequential turns once echoed two "call_1" and
# the provider answered 400 invalid_args.

# LAW: calls renumber to sequential ids from the base, name and args
# untouched — ToolAct, PTools and history all agree on ids.
law renumber_is_sequential:
  {S.renumber(Con{T.Call{5, "echo", "hi"}, Con{T.Call{9, "bash", "ls"}, Nil{}}}, 42)
    == Con{T.Call{42, "echo", "hi"}, Con{T.Call{43, "bash", "ls"}, Nil{}}}
    : List<&2, T.Call>}

# LAW: renumbering drops and duplicates nothing.
law renumber_keeps_length:
  for +c1: T.Call
  for +c2: T.Call
  for +base: U32
  {S.calls_count(S.renumber(Con{c1, Con{c2, Nil{}}}, base)) == 2n : Nat}

# ---- tool results carry the name the model called ----

# LAW: the wire echo of a past call carries its name as the model wrote it.
law wire_echo_keeps_names:
  {Rm.ser_call(T.Call{7, "bash", "ls -la"}) == "\n  CALL 7 bash : ls -la"
    : String}

# LAW: tool results are attributed to the name the model called.
law result_text_keeps_names:
  {S.tool_result_msg("bash", "out", False{})
    == T.tool_res("tool bash failed: out") : T.Msg}

# ---- structured tool arguments (core/api.bend) ----
# The schemas promise the model exact fields; args must pass through
# un-wrapped for the structured tools and wrapped for the flat ones.

# LAW: run_typescript args pass through as the JSON the model wrote.
law args_json_passes_structured:
  {Oai.args_json("run_typescript", "{\"code\": \"x\"}") == "{\"code\": \"x\"}"
    : String}

# LAW: search args pass through too.
law args_json_passes_search:
  {Oai.args_json("search_tool_functions", "{\"mode\": \"details\"}")
    == "{\"mode\": \"details\"}" : String}

# LAW: ordinary tools keep the flat {"arg": ...} wrapper.
law args_json_wraps_plain:
  {Oai.args_json("bash", "ls -la") == "{\"arg\":\"ls -la\"}" : String}

# LAW: call_code extracts the code field from run_typescript args.
law call_code_extracts:
  {W.call_code("{\"code\": \"async function main() { return 1; }\"}")
    == "async function main() { return 1; }" : String}

# LAW: args with no code field pass through — the engine fails
# readably instead of the runtime guessing.
law call_code_passthrough:
  {W.call_code("just text") == "just text" : String}

# ---- the intent of a call (BISE-223) ----
# bash and run_typescript take an optional one-line "description" the
# TUI shows in place of the code. A bash call with one keeps its JSON
# args; everything else about flat args stays as it was.

# LAW: a bash call's command is the "arg" of its JSON args.
law flat_cmd_json:
  {W.flat_cmd("{\"arg\":\"ls\",\"description\":\"lister\"}") == "ls" : String}

# LAW: bare bash args are the command itself.
law flat_cmd_raw:
  {W.flat_cmd("ls -la") == "ls -la" : String}

# LAW: the intent is the description of the JSON args.
law call_intent_reads:
  {W.call_intent("{\"code\":\"x\",\"description\":\"je lance les tests\"}")
    == "je lance les tests" : String}

# LAW: args with no description have no intent.
law call_intent_none:
  {W.call_intent("ls -la") == "" : String}

# LAW: the intent is one line.
law call_intent_one_line:
  {W.call_intent("{\"code\":\"x\",\"description\":\" a\\nb \"}") == "a b"
    : String}

# LAW: the OpenAI adapter keeps a bash call's description in its args.
law arg_of_keeps_bash_intent:
  {Oai.arg_of("bash", "{\"arg\":\"ls\",\"description\":\"lister\"}")
    == "{\"arg\":\"ls\",\"description\":\"lister\"}" : String}

# LAW: a bash call without a description stays the bare command.
law arg_of_bash_plain:
  {Oai.arg_of("bash", "{\"arg\":\"ls\"}") == "ls" : String}

# LAW: other flat tools drop a stray description.
law arg_of_other_drops_intent:
  {Oai.arg_of("skill", "{\"arg\":\"x\",\"description\":\"d\"}") == "x"
    : String}

# LAW: bash JSON args go back to the provider as they are.
law args_json_passes_bash_intent:
  {Oai.args_json("bash", "{\"arg\":\"ls\",\"description\":\"lister\"}")
    == "{\"arg\":\"ls\",\"description\":\"lister\"}" : String}

# LAW: the Anthropic input of such a call is its JSON object.
law input_json_bash_intent:
  {Anth.input_json("bash", "{\"arg\":\"ls\",\"description\":\"lister\"}")
    == J.obj(Con{J.kv("arg", J.str("ls")),
      Con{J.kv("description", J.str("lister")), Nil{}}}) : J.Json}

# LAW: the TUI gets the command, not the JSON, in the tool and code lines.
law ann_bash_intent_shows_cmd:
  {(Rt.call_ann(T.Call{3, "bash", "{\"arg\":\"ls\",\"description\":\"lister\"}"}),
    Rt.code_ann(T.Call{3, "bash", "{\"arg\":\"ls\",\"description\":\"lister\"}"}))
    == ("tool #3 bash : ls", "tool_code #3 : ls") : String & String}

# LAW: a described bash call gets its intent line.
law intent_anns_bash:
  {Rt.intent_anns(T.Call{3, "bash", "{\"arg\":\"ls\",\"description\":\"lister\"}"})
    == Con{"tool_intent #3 : lister", Nil{}} : List<&2, String>}

# LAW: a run_typescript call gets its intent line.
law intent_anns_ts:
  {Rt.intent_anns(T.Call{4, "run_typescript", "{\"code\":\"x\",\"description\":\"je cherche\"}"})
    == Con{"tool_intent #4 : je cherche", Nil{}} : List<&2, String>}

# LAW: no description, no intent line (old sessions replay unchanged).
law intent_anns_none:
  {Rt.intent_anns(T.Call{5, "bash", "ls"}) == Nil{} : List<&2, String>}

# LAW: another tool's "description" field is not an intent.
law intent_anns_other_tool:
  {Rt.intent_anns(T.Call{6, "mcp_x", "{\"description\":\"d\"}"}) == Nil{}
    : List<&2, String>}
