# LAWS.bend — the harness invariants.
# Open claims; PROOF.bend must close every one. `bend PROOF.bend` is the
# gate: it fails while any law is open or false.
import Base
import ./core/types.bend as T
import ./core/history.bend as H
import ./core/compaction.bend as C
import ./core/checkpoint.bend as K
import ./core/session.bend as S
import ./core/patch.bend as Pa
import ./core/edit.bend as Ed
import ./core/config.bend as Cf
import ./core/commands.bend as Cm
import ./core/api.bend as Api
import ./core/wire.bend as W
import ./core/oai-chat.bend as Oai
import ./core/oai-resp.bend as Rs
import ./core/anthropic.bend as Anth
import ./core/image.bend as Im
import ./core/text.bend as Tx
import ./core/anth-stream.bend as As
import ./core/oai-stream.bend as Os
import ./core/obs.bend as O
import ./runtime/remote.bend as Rm
import ./runtime/tools-pure.bend as Xt
import ./runtime/main-pure.bend as Rt
import ./runtime/bash-pure.bend as Sh
import ./runtime/provider-pure.bend as Pv
import ./runtime/selftools-pure.bend as Sf
import ./runtime/settings-pure.bend as St
import ./runtime/mcp-pure.bend as Mc
import ./runtime/skills-pure.bend as Sk
import ./runtime/repl-core-pure.bend as Rp
import ./vendor/json.bend as J
import ./runtime/debug-pure.bend as Dg
import ./runtime/usage-pure.bend as Us
import ./runtime/gate-pure.bend as Gt
import ./runtime/race-pure.bend as Rc
import ./hub/model.bend as HM
import ./hub/cx.bend as HX
import ./hub/core.bend as HC
import ./hub/inv.bend as HI
import ./hub/timers.bend as HTM
import ./hub/xhub-model.bend as HXH
import ./hub/xhub.bend as HXB
import ./hub/codec.bend as HCo

# LAW: a checkpoint restores to exactly the session it exported.
law checkpoint_roundtrip:
  for +s: T.Session
  {K.restore(K.export(s)) == s : T.Session}

# LAW: a compaction reply that carries a tool call is malformed, whatever
# its text says (ADR 0011: "any tool call makes the response malformed").
law summary_rejects_tool_calls:
  for +t: String
  for +c: T.Call
  {C.valid_summary(t, Con{c, Nil{}}) == None{} : Maybe<&2, String>}

# LAW: an empty <summary></summary> block is malformed, not accepted.
law empty_block_malformed:
  {C.parse_summary("<summary></summary>") == None{} : Maybe<&2, String>}

# LAW: undelimited non-empty text is accepted as the provider
# compatibility fallback (ADR 0011).
law fallback_accepts_plain_text:
  {C.parse_summary("plain text") == Some{"plain text"} : Maybe<&2, String>}

# LAW: nothing can be dropped from an empty history.
law drop_one_empty:
  {H.drop_one(Nil{}) == None{} : Maybe<&2, List<&2, T.Msg>>}

# LAW: manual compaction instructions travel under their tag.
law manual_instructions_tagged:
  {String.contains(C.compaction_prompt("keep names"),
    "<extra_compaction_instructions>keep names</extra_compaction_instructions>")
    == True{} : Bool}

# LAW: the projected model input always leads with the system message,
# so every provider request keeps the prompt prefix.
law model_input_leads_with_system:
  for +cfg: T.Cfg
  for +h: List<&2, T.Msg>
  {S.head_role(S.model_input(cfg, h)) == Some{T.RSys{}} : Maybe<&2, T.Role>}

# LAW: the compaction request is [system] ++ history ++ [prompt]: it
# ENDS with the user-role compaction prompt. An idle history ends with
# an assistant message, and providers without assistant prefill reject
# a request that does not end with a user message.
law compaction_request_ends_with_prompt:
  {C.project(T.Cfg{400n, 40n, 3n, "You are precise.", Nil{}},
      Con{T.user("bonjour"), Con{T.asst("réponse", Nil{}), Nil{}}}, "")
    == C.POk{Con{T.sys("You are precise."),
      Con{T.user("bonjour"), Con{T.asst("réponse", Nil{}),
      Con{T.injected_user(C.compaction_prompt("")), Nil{}}}}}}
    : C.PRes}

# ---- the commit convention ----

# LAW: a commit lands at the tail of the history (canonical order is
# oldest-first). A prepend regression — which a live run once exposed
# and the scripted scenarios masked — breaks this immediately.
law commits_append_at_tail:
  {H.last(H.commit(Con{T.user("one"), Con{T.user("two"), Nil{}}},
      T.user("three")))
    == Some{T.user("three")} : Maybe<&2, T.Msg>}

# ---- the wire protocol (runtime/remote.bend, spoken by the bridge) ----

# LAW: a serialized call parses back into the same call — the " : "
# separator and the id format are contract, not convention.
law wire_call_roundtrip:
  {Rm.parse_call("7 echo : hi there") == T.Call{7, "echo", "hi there"}
    : T.Call}

# LAW: a reply parses bottom-up into the completion the Core expects:
# OK text, CALL lines, END.
law wire_reply_roundtrip:
  {Rm.parse_reply("OK hello\nCALL 1 echo : hi\nEND\n")
    == Rm.RROk{"hello", Con{T.Call{1, "echo", "hi"}, Nil{}}}
    : Rm.RemoteRes}

# ---- multi-line message text (paste) ----
# The client escapes real newlines to keep every transport line
# single-line (socket, wire, checkpoint); the text restores at each
# parse, and the MODEL input carries the real newlines (ser_msg escapes
# instead of flattening - flattening turned a pasted block into one
# space-joined line).

# LAW: the say command line unescapes into the message text.
law say_multiline_unescapes:
  {Rp.plan_say(True{}, "say", "a\\nb")
    == Rp.PRun{Con{T.UserMsg{T.Queue{}, "a
b"}, Nil{}},
      Rp.script_for_entry("a
b")}
    : Rp.Plan}

# LAW: the request wire carries message text on one line, escaped.
law ser_msg_escapes_newlines:
  {Rm.ser_msg(T.Msg{T.RUser{}, "a
b", Nil{}, False{}})
    == "MSG user : a\\nb"
    : String}

# LAW: a multi-line message survives the checkpoint roundtrip.
def ml_flow.started(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(T.new_session(cfg), T.UserMsg{T.Queue{}, "a
b"}),
    T.new_session(cfg))

def ml_flow.done(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(ml_flow.started(cfg), T.CompletionOk{0, "done", Nil{}}),
    ml_flow.started(cfg))

law checkpoint_multiline_roundtrip:
  {K.restore(K.export(ml_flow.done(law_cfg()))) == ml_flow.done(law_cfg())
    : T.Session}

# ---- REPL chunk coalescing (the stub in PROOF) ----

# LAW: a chunk carrying several lines splits into separate commands,
# empty lines dropped - kernel write coalescing must not merge
# messages or swallow the commands after the first.
law chunk_lines_split:
  {Rp.cmd_lines("say a

say b")
    == Con{"say a", Con{"say b", Nil{}}}
    : List<&2, String>}

# LAW: a chunk of only blank lines yields no commands.
law chunk_lines_empty:
  {Rp.cmd_lines("\n\n") == Nil{} : List<&2, String>}

# LAW: the interrupt flag file is per-process (the Ctrl+C path), in the
# side dir (the agent's run/ folder, approvals-design.md §7.1).
law interrupt_path_is_per_port:
  {Pv.interrupt_path_of("/h/agents/a/run", "7702")
    == "/h/agents/a/run/bend-interrupt-7702.txt" : String}

# ---- a call raced against the interrupt flag (issue 13, race-pure) ----

# LAW: a result that is in wins, whatever the flag says (the boundary
# drain after it applies the interrupt).
law race_result_wins:
  for +f: Maybe<&2, String>
  {Rc.decide(True{}, f) == Rc.VWon{} : Rc.Verdict}

# LAW: a tick with no flag, or a cleared one (an empty file), waits.
law race_no_flag_waits:
  {Con{Rc.decide(False{}, None{}), Con{Rc.decide(False{}, Some{""}),
    Con{Rc.decide(False{}, Some{" \n"}), Nil{}}}}
    == Con{Rc.VWait{}, Con{Rc.VWait{}, Con{Rc.VWait{}, Nil{}}}} : List<&2, Rc.Verdict>}

# LAW: a set flag stops, naming who asked ("1" and "user": the user).
law race_flag_stops_naming_who:
  {Con{Rc.decide(False{}, Some{"main"}), Con{Rc.decide(False{}, Some{"1"}), Nil{}}}
    == Con{Rc.VStop{"main"}, Con{Rc.VStop{"the user"}, Nil{}}} : List<&2, Rc.Verdict>}

# LAW: the ticker ends once its tick is refused (the channel closed).
law race_ticker_ends_on_close:
  {Rc.tick_again(False{}) == False{} : Bool}

# LAW: an attempt is cancelled only by its own token, never by a stale
# file or none; the cancel file is per process, next to the flag.
law race_cancel_is_its_own:
  {Con{Rc.cancelled(Some{"1700-0\n"}, "1700-0"), Con{Rc.cancelled(Some{"1600-0"}, "1700-0"),
    Con{Rc.cancelled(None{}, "1700-0"), Con{Rc.cancelled(Some{""}, "1700-0"), Nil{}}}}}
    == Con{True{}, Con{False{}, Con{False{}, Con{False{}, Nil{}}}}} : List<&2, Bool>}

law race_cancel_path_is_per_port:
  {Rc.cancel_path_of("/h/agents/a/run", "7702")
    == "/h/agents/a/run/bend-cancel-7702.txt" : String}

# LAW: the bash wrapper reads the flag in its poll loop and stops only
# the sync command's own group (-$C), TERM before KILL; never the
# runner or the wrapper (a background handoff has no poll loop left).
law bash_stop_cuts_its_own_group:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "IF=/i"),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_stop_check()),
   Bool.and(String.contains(Sh.bg_stop_check(), "kill -TERM -$C"),
   Bool.and(String.contains(Sh.bg_stop_check(), "kill -KILL -$C"),
   Bool.and(Bool.not(String.contains(Sh.bg_stop_check(), "kill -TERM $r")),
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_stop_note()))))))
    == True{} : Bool}

# ---- multi-line tool args on the wire ----

# LAW: the wire arg encoding is reversible on newlines - tool args are
# commands and JSON text, and a lossy escape would corrupt them
# (escape_nl cannot tell an encoded newline from a literal backslash-n).
law wire_args_reversible:
  {W.wire_decode(W.wire_encode("one
two")) == "one
two" : String}

# LAW: a literal backslash-n in the arg survives the round-trip (the
# python -c one-liners are full of them).
law wire_args_keep_backslash_n:
  {W.wire_decode(W.wire_encode("a\\nb")) == "a\\nb" : String}

# LAW: a literal capital-N sentinel sequence survives too - encoding
# doubles its backslash, decoding restores it exactly.
law wire_args_keep_sentinel:
  {W.wire_decode(W.wire_encode("a\\Nb")) == "a\\Nb" : String}

# LAW: the reply wire carries a multi-line arg encoded, on one line.
law reply_encodes_multiline_args:
  {Oai.reply_of(200, "{\"choices\":[{\"message\":{\"content\":\"\",\"tool_calls\":[{\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\": \\\"one\\\\ntwo\\\"}\"}}]}}]}")
    == "OK 
CALL 1 bash : one\\Ntwo
END
" : String}

# LAW: the reply parse gives the arg its real newlines back - the
# heredoc the model wrote is the heredoc bash runs.
law parse_reply_decodes_multiline_args:
  {Rm.parse_reply("OK 
CALL 1 bash : one\\Ntwo
END
")
    == Rm.RROk{"", Con{T.Call{1, "bash", "one
two"}, Nil{}}}
    : Rm.RemoteRes}

# LAW: the request wire encodes multi-line args the same way (the
# session calls ride the wire to every provider round).
law wire_echo_encodes_multiline_args:
  {Rm.ser_call(T.Call{7, "bash", "a
b"})
    == "
  CALL 7 bash : a\\Nb" : String}

# LAW: the request parse decodes the args the history carries.
law api_wire_decodes_args:
  {W.parse_wire("MODEL default
MSG assistant : 
  CALL 1 bash : one\\Ntwo
END
")
    == W.WReq{Nil{}, Con{W.WMsg{"assistant", "",
      Con{W.WCall{"1", "bash", "one
two"}, Nil{}}}, Nil{}}, None{}}
    : W.WReq}
# ---- session-level rejections (the Core accepts nothing uninvited) ----

# LAW: a tool result with no tools pending is rejected — a late or
# orphan result can never touch the session.
law orphan_tool_result_rejected:
  for +cfg: T.Cfg
  {S.apply(T.new_session(cfg), T.ToolOk{0, "out"})
    == T.Rejected{"no pending tool result"} : T.Res}

# LAW: interrupting an idle session is an accepted no-op: the session
# is untouched (only the turn counter moves).
law idle_interrupt_noop:
  for +cfg: T.Cfg
  {S.apply(T.new_session(cfg), T.Interrupt{})
    == T.Accepted{Nil{}, Nil{}, S.bump_inputs(T.new_session(cfg))} : T.Res}

# ---- ADR 0011 selection budget ----

# LAW: the middle-truncation keeps the head, marks the cut, and stays
# within the token budget (4 chars per token).
law truncate_marks_the_cut:
  {H.truncate("abcdefghij", 2n) == "ab[...]" : String}

# LAW: a message that already fits the budget passes through untouched.
law truncate_keeps_small_messages:
  {H.truncate("abc", 2n) == "abc" : String}

# LAW: the compaction selection keeps only non-injected user messages.
law preserve_recent_keeps_users:
  {H.all_user(H.preserve_recent(100n,
      Con{T.asst("hi", Nil{}),
        Con{T.user("u1"),
          Con{T.tool_res("res"),
            Con{T.user("u2"), Nil{}}}}}))
    == True{} : Bool}

# ---- compaction keeps the summary, then only the newest user messages ----
# (user: "il faut garder que les messages récents, avec un nombre de
# tokens max en partant du plus récent"). Each kc.u* is 40 or 41 chars,
# 11 estimated tokens; kc.cfg keeps 25 tokens: two of them.

def kc.cfg() -> T.Cfg:
  T.Cfg{4000n, 25n, 3n, "sys", Nil{}}

def kc.u1() -> T.Msg:
  T.user("u1: fix the login page, it hangs on load")

def kc.u2() -> T.Msg:
  T.user("u2: then add a test for the signup flow!")

def kc.u3() -> T.Msg:
  T.user("u3: and rename the field to email_addr..")

def kc.u4() -> T.Msg:
  T.user("u4: now bump the version and tag v0.4.0.")

def kc.u5() -> T.Msg:
  T.user("u5: write the changelog entry for v0.4.0")

def kc.u6() -> T.Msg:
  T.user("u6: and open the PR on the feature branch")

def kc.ok() -> T.Msg:
  T.asst("ok", Nil{})

# compaction 1 of a fresh conversation, then two more turns and a
# compaction, then one more turn and a compaction, then a compaction of
# an idle context with no new message
def kc.r1() -> List<&2, T.Msg>:
  C.replaced(C.replace(kc.cfg(),
    [kc.u1(), kc.ok(), kc.u2(), kc.ok(), kc.u3(), kc.ok()], "s1"))

def kc.r2() -> List<&2, T.Msg>:
  C.replaced(C.replace(kc.cfg(), List.append(&2, T.Msg, kc.r1(),
    [kc.u4(), kc.ok(), kc.u5(), kc.ok()]), "s2"))

def kc.r3() -> List<&2, T.Msg>:
  C.replaced(C.replace(kc.cfg(), List.append(&2, T.Msg, kc.r2(),
    [kc.u6(), kc.ok()]), "s3"))

def kc.r4() -> List<&2, T.Msg>:
  C.replaced(C.replace(kc.cfg(), List.append(&2, T.Msg, kc.r3(),
    [T.asst("done", Nil{})]), "s4"))

# LAW: a replacement is the summary message first, then the kept user
# messages, verbatim: no separate preamble, so the session log's
# compaction_done carries the real summary (what /log shows).
law compaction_replacement_opens_with_summary:
  {C.replace(T.Cfg{400n, 40n, 3n, "You are precise.", Nil{}},
      [T.user("bonjour"), T.asst("réponse", Nil{})], "court")
    == C.ROk{[C.summary_message("court"), T.user("bonjour")]} : C.RRes}

# LAW: the kept messages are the NEWEST that fit the budget counted from
# the latest backwards: of three 11-token messages, a 25-token budget
# keeps the last two.
law compaction_keeps_newest_within_budget:
  {kc.r1() == [C.summary_message("s1"), kc.u2(), kc.u3()]
    : List<&2, T.Msg>}

# LAW: a message is never cut in half: the walk stops at the first one
# that does not fit, even when an older, smaller one would.
law compaction_stops_at_first_misfit:
  {C.kept_users(C.replace(kc.cfg(),
      [T.user("hi"), kc.ok(), T.user(String.append(
        "a message far too long for what remains of the budget ",
        "once the latest one is in")), kc.ok(), kc.u6(), kc.ok()], "s"))
    == [kc.u6()] : List<&2, T.Msg>}

# LAW: the latest user message is always kept, whole, even over budget.
law compaction_keeps_latest_over_budget:
  {C.kept_users(C.replace(T.Cfg{4000n, 2n, 3n, "sys", Nil{}},
      [kc.u1(), kc.ok(), kc.u2(), kc.ok()], "s"))
    == [kc.u2()] : List<&2, T.Msg>}

# LAW: across compactions nothing piles up: the context holds ONE
# summary (an earlier summary is injected, never kept: the summarizer
# folds it in) and the newest messages within the budget; the messages
# kept by earlier compactions leave as newer ones come.
law compaction_second_drops_old_kept:
  {kc.r2() == [C.summary_message("s2"), kc.u4(), kc.u5()]
    : List<&2, T.Msg>}

law compaction_third_drops_old_kept:
  {kc.r3() == [C.summary_message("s3"), kc.u5(), kc.u6()]
    : List<&2, T.Msg>}

# LAW: compacting a compacted context with no new message keeps the
# same newest messages (within the budget), not more.
law compaction_idle_recompaction_stable:
  {kc.r4() == [C.summary_message("s4"), kc.u5(), kc.u6()]
    : List<&2, T.Msg>}

# LAW: after each of the four compactions the kept messages fit the
# budget.
law compaction_kept_within_budget:
  {Bool.and(H.within_budget(25n, H.users_of(kc.r1())),
    Bool.and(H.within_budget(25n, H.users_of(kc.r2())),
      Bool.and(H.within_budget(25n, H.users_of(kc.r3())),
        H.within_budget(25n, H.users_of(kc.r4())))))
    == True{} : Bool}

# LAW: the summarizer is told that older user messages are gone, so its
# summary carries every request and its state.
law compaction_prompt_carries_requests:
  {Bool.and(String.contains(C.compaction_prompt(""),
      "every request the user made"),
    String.contains(C.compaction_prompt(""), "any earlier summary, is gone"))
    == True{} : Bool}

# LAW: the summary message keeps the prefix --resume and the session
# migration recognise, and its <summary> block parses back to the
# summary.
def kc.text(m: T.Msg) -> String:
  match m:
    case T.Msg{_, t, _, _}:
      t

law summary_message_parses_back:
  {Bool.and(String.starts_with(kc.text(C.summary_message("court")),
      "Summary of the earlier conversation:"),
    Maybe.is_some(&2, String, C.parse_summary(kc.text(C.summary_message("court")))))
    == True{} : Bool}

law summary_message_summary_is_the_block:
  {C.parse_summary(kc.text(C.summary_message("court")))
    == Some{"court"} : Maybe<&2, String>}

# ---- the observation wire format (every client parses this) ----

# LAW: an observation renders as exactly one line with the obs prefix.
law obs_wire_format:
  {O.show_obs(T.Assistant{"bonjour"}) == "  obs: assistant: bonjour"
    : String}

# LAW: the compaction preview stays on one wire line: a summary that
# starts with (or contains) newlines is trimmed and flattened.
law compaction_preview_one_line:
  {O.show_obs(T.CompactionDone{"
Le user
veut ça"})
    == "  obs: compaction_done: Le user veut ça..." : String}

# LAW: the same holds for the in-session form, where a newline travels
# escaped as backslash-n (chars 92, 110).
law compaction_preview_unescapes:
  {O.show_obs(T.CompactionDone{SCon{Chr{92}, SCon{Chr{110}, "Le user"}}})
    == "  obs: compaction_done: Le user..." : String}

# ---- the command language (core/commands.bend, server-side) ----

# LAW: plain text is an implicit "say" — the reason no client ever
# needs a command prefix.
law implicit_say:
  {Cm.interpret("bonjour le harness") == Cm.Send{"say bonjour le harness"}
    : Cm.Interpretation}

# LAW: raw protocol words pass through verbatim, never re-interpreted.
law raw_words_pass_through:
  {Cm.interpret("say tools demo") == Cm.Send{"say tools demo"}
    : Cm.Interpretation}

# LAW: /compact maps onto the manual compaction protocol word.
law slash_compact_maps:
  {Cm.interpret("/compact") == Cm.Send{"compact"} : Cm.Interpretation}

# LAW: /steer is not a user command — steering is the Enter key while
# a turn runs (the client writes the steer side-channel).
law slash_steer_is_not_a_command:
  {Cm.interpret("/steer vas-y")
    == Cm.Note{"unknown command: /steer — try /help"}
    : Cm.Interpretation}

# LAW: /notify is not a user command — notifications come from the
# system and from background processes, never from the user.
law slash_notify_is_not_a_command:
  {Cm.interpret("/notify salut")
    == Cm.Note{"unknown command: /notify — try /help"}
    : Cm.Interpretation}

# LAW: /quit (and /exit) map onto the connection-closing word.
law slash_quit_maps:
  {Cm.interpret("/exit") == Cm.Send{"quit"} : Cm.Interpretation}

# LAW: an unknown /command is refused with the name it was given.
law unknown_slash_refused:
  {Cm.interpret("/frobnicate")
    == Cm.Note{"unknown command: /frobnicate — try /help"}
    : Cm.Interpretation}

# ---- steering commits into the durable history (vibe_sdk parity) ----
# vibe_sdk: defer_steer holds the message; state.context.extend(steering)
# commits it at the next agent call, and finish_turn commits it when a
# turn dies first. The Bend core now does both.

# a concrete cfg: the flows go through dispatch, whose fits comparison
# needs a concrete threshold to normalize
def law_cfg() -> T.Cfg:
  T.Cfg{600n, 20000n, 3n, "system prompt", Nil{}}

# the law flows: say hi (turn + first AgentCall pending), steer, then
# complete the call with a tool round -> the dispatch commits the steer
def steer_flow.started(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(T.new_session(cfg), T.UserMsg{T.Queue{}, "hi"}),
    T.new_session(cfg))

def steer_flow.steered(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.started(cfg),
      T.UserMsg{T.Steer{}, "penguins"}), steer_flow.started(cfg))

def steer_flow.called(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.steered(cfg),
      T.CompletionOk{0, "thinking", Con{T.Call{1, "bash", "ls"}, Nil{}}}),
    steer_flow.steered(cfg))

def steer_flow(+cfg: T.Cfg) -> T.Session:
  # the completion renumbers the calls (base = the consumed action ids),
  # so Call{1,...} waits as call id 2
  S.res_next(S.apply(steer_flow.called(cfg), T.ToolOk{2, "out"}),
    steer_flow.called(cfg))

# LAW: a steered user message enters the durable history at the next
# model-safe boundary -- every later model call and the checkpoint
# carry it.
law steering_lands_in_history:
  {S.history_has_user(S.sess_history(steer_flow(law_cfg())), "penguins")
    == True{} : Bool}

# LAW: a turn that dies before its next model call (an interrupt) still
# commits the steering: the client already showed the message, the next
# turn's model input must carry it.
law steering_survives_interrupt:
  {S.history_has_user(
      S.sess_history(S.res_next(
        S.apply(steer_flow.steered(law_cfg()), T.Interrupt{}),
        steer_flow.steered(law_cfg()))), "penguins") == True{} : Bool}

# LAW: a failed turn commits its pending steering the same way.
law steering_survives_fail:
  {S.history_has_user(
      S.sess_history(S.res_next(
        S.apply(steer_flow.steered(law_cfg()), T.FailTurn{"boom"}),
        steer_flow.steered(law_cfg()))), "penguins") == True{} : Bool}

# big-request: what an apply says, in short: "compact" when it starts a
# compaction, "fail" when it ends the turn, "other" else
def big_flow.verdict(obs: List<&2, T.Obs>) -> String:
  match obs:
    case Nil{}:
      "other"
    case Con{+o, rest}:
      match o:
        case T.CompactionStarted{_, _}:
          "compact"
        case T.TurnDone{_}:
          "fail"
        case _:
          big_flow.verdict(rest)

def big_flow.of(r: T.Res) -> String:
  match r:
    case T.Accepted{obs, _, _}:
      big_flow.verdict(obs)
    case T.Rejected{+why}:
      String.append("rejected: ", why)

def big_flow.err() -> String:
  "Anthropic refused the request (400). the request is too large. Anthropic said: \"Request content length exceeded 32 MB limit.\""

# LAW (big-request): a call refused for its size, in a history that has
# an assistant message, starts a compaction instead of failing the turn
# (the agent recovers by itself); right after a compaction (no assistant
# message left: nothing to shrink) or for another failure, the turn fails.
law size_refusal_compacts:
  {Con{big_flow.of(S.apply(steer_flow(law_cfg()), T.CompletionFail{3, big_flow.err()})),
    Con{big_flow.of(S.apply(steer_flow.started(law_cfg()), T.CompletionFail{0, big_flow.err()})),
      Con{big_flow.of(S.apply(steer_flow(law_cfg()), T.CompletionFail{3, "boom"})), Nil{}}}}
    == Con{"compact", Con{"fail", Con{"fail", Nil{}}}} : List<&2, String>}

# arrival order: two steers, committed in the order they arrived
def steer_order.one(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.started(cfg),
      T.UserMsg{T.Steer{}, "one"}), steer_flow.started(cfg))

def steer_order.two(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_order.one(cfg),
      T.UserMsg{T.Steer{}, "two"}), steer_order.one(cfg))

def steer_order.called(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_order.two(cfg),
      T.CompletionOk{0, "thinking", Con{T.Call{1, "bash", "ls"}, Nil{}}}),
    steer_order.two(cfg))

def steer_order(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_order.called(cfg), T.ToolOk{2, "out"}),
    steer_order.called(cfg))

# LAW: several steers commit in arrival order, not reversed.
law steering_arrival_order:
  {S.injected_user_texts(S.sess_history(steer_order(law_cfg())))
    == Con{"one", Con{"two", Nil{}}} : List<&2, String>}

# ---- mid-turn steering side-channel (ADR 0005, live) ----
# The client appends steering lines to /tmp/bend-steer-<port>.txt; the
# Runtime drains the file at every model/tool safe boundary (the socket
# is only read between turns). The pure pieces are pinned here.

# LAW: the side-channel path is per-process (the REPL port), so two
# parallel terminals never read each other's steering.
law steer_path_is_per_port:
  {Rt.steer_path_of("/h/agents/a/run", "7702")
    == "/h/agents/a/run/bend-steer-7702.txt" : String}

# LAW: blank lines in the side-channel never steer.
law steer_channel_skips_blank_lines:
  {Rt.drain_steer.skip_go(Con{"a", Con{"", Con{"b", Nil{}}}})
    == Con{"a", Con{"b", Nil{}}} : List<&2, String>}

# LAW: a drained mid-turn steer emits exactly the wire line the client
# shows (steering_received) — never a turn_started: draining must not
# start a new turn, it steers the running one.
law drain_emits_steering_received:
  {Rt.drain_steer.emit_of(
      S.apply(steer_flow.started(law_cfg()), T.UserMsg{T.Steer{}, "penguins"}))
    == Con{"  obs: steering_received: penguins", Nil{}} : List<&2, String>}

def drain_flow.acts(res: T.Res) -> List<&2, T.Act>:
  match res:
    case T.Accepted{_, acts, _}:
      acts
    case T.Rejected{_}:
      Nil{}

# LAW: mid-turn steering produces no acts: the drained line never
# reroutes the running turn's continuation (the drain may drop acts
# safely).
law drain_steering_is_act_free:
  {drain_flow.acts(S.apply(steer_flow.started(law_cfg()),
      T.UserMsg{T.Steer{}, "penguins"})) == Nil{} : List<&2, T.Act>}

# LAW: a rejected apply keeps the session (the drain's defensive arm).
law drain_next_of_rejected_keeps_session:
  for +s: T.Session
  {Rt.drain_steer.next_of(s, T.Rejected{"nope"}) == s : T.Session}

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

# ---- queued messages (ADR 0005's sibling: Queue never supersedes) ----
# A Queue message applied mid-turn is HELD (queue_msg), and the idle
# boundary pops it into the next turn. Before the pop existed, the
# held list was a black hole: nothing consumed it.

def queue_flow.res(+cfg: T.Cfg) -> T.Res:
  S.apply(steer_flow.started(cfg), T.UserMsg{T.Queue{}, "penguins"})

def queue_flow.held(+cfg: T.Cfg) -> T.Session:
  S.res_next(queue_flow.res(cfg), steer_flow.started(cfg))

# LAW: a queued message mid-turn produces no acts — it never
# supersedes the running turn.
law queue_hold_is_act_free:
  {drain_flow.acts(queue_flow.res(law_cfg())) == Nil{} : List<&2, T.Act>}

# LAW: ... and lands in the held list (never in the history yet).
law queue_holds_mid_turn:
  {S.sess_queued(queue_flow.held(law_cfg()))
    == Con{"penguins", Nil{}} : List<&2, String>}

def queue_flow.done(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(queue_flow.held(cfg),
      T.CompletionOk{0, "done", Nil{}}), queue_flow.held(cfg))

# LAW: the turn's end keeps the queue intact.
law queue_survives_turn_end:
  {S.sess_queued(queue_flow.done(law_cfg()))
    == Con{"penguins", Nil{}} : List<&2, String>}

def queue_flow.tr(+cfg: T.Cfg) -> Rt.Trans:
  Rt.transition(Rt.Rt{queue_flow.done(cfg), Nil{}, Nil{}, False{}},
    Rt.JIdle{})

def queue_flow.job_of(t: Rt.Trans) -> Rt.Job:
  match t:
    case Rt.Tr{_, _, job}:
      job

def queue_flow.is_queue_cmd(job: Rt.Job) -> Bool:
  match job:
    case Rt.JCmd{T.UserMsg{T.Queue{}, "penguins"}, Nil{}}:
      True{}
    case _:
      False{}

def queue_flow.tail_of(t: Rt.Trans) -> List<&2, String>:
  match t:
    case Rt.Tr{_, +rt2, _}:
      match rt2:
        case Rt.Rt{+sess, _, _, _}:
          S.sess_queued(sess)

def queue_flow.next_of(t: Rt.Trans) -> T.Session:
  match t:
    case Rt.Tr{_, +rt2, _}:
      match rt2:
        case Rt.Rt{+sess, _, _, _}:
          sess

# LAW: the idle boundary pops the held head into the next turn's user
# message ...
law queue_pops_at_idle:
  {queue_flow.is_queue_cmd(queue_flow.job_of(queue_flow.tr(law_cfg())))
    == True{} : Bool}

# LAW: ... and the popped head is REMOVED from the held list (the tail
# stays queued for later turns; no re-delivery loop).
law queue_pop_removes_head:
  {queue_flow.tail_of(queue_flow.tr(law_cfg())) == Nil{}
    : List<&2, String>}

def queue_flow.delivered(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(queue_flow.next_of(queue_flow.tr(cfg)),
      T.UserMsg{T.Queue{}, "penguins"}),
    queue_flow.next_of(queue_flow.tr(cfg)))

# a PLAIN user text in the history (history_has_user matches only
# injected messages; a queued message starts a turn as a plain one)
def queue_flow.has_user(msgs: List<&2, T.Msg>, +text: String) -> Bool:
  match msgs:
    case Nil{}:
      False{}
    case Con{+m, +rest}:
      match m:
        case T.Msg{T.RUser{}, +t, _, False{}}:
          Bool.or(String.eq(t, text), queue_flow.has_user(rest, text))
        case _:
          queue_flow.has_user(rest, text)

# LAW: the popped message starts the next turn and reaches the
# durable history — the queue is a delivery mechanism, not a hold.
law queue_delivered_in_history:
  {queue_flow.has_user(S.sess_history(queue_flow.delivered(law_cfg())),
    "penguins") == True{} : Bool}

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
law live_catalog_edit_toolset_has_six_tools:
  {Xt.catalog_len(Xt.catalog_live("s", "r", "b", "k", "p", False{})) == 6n : Nat}

law live_catalog_patch_toolset_has_five_tools:
  {Xt.catalog_len(Xt.catalog_live("s", "r", "b", "k", "p", True{})) == 5n : Nat}

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
    == "search_tool_functions,bash,run_typescript,skill,write_file,edit," : String}

law live_catalog_patch_names:
  {String.join([Xt.tool_name_at(0n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(1n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(2n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(3n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(4n, Xt.catalog_live("s", "r", "b", "k", "p", True{})),
     Xt.tool_name_at(5n, Xt.catalog_live("s", "r", "b", "k", "p", True{}))], ",")
    == "search_tool_functions,bash,run_typescript,skill,apply_patch," : String}

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
    == "Edit files with `edit`, create them with `write_file`. Don't edit files through bash scripts (python or perl, `sed` on many files): in auto they need the user, and tool edits show as diffs.|Edit and create files with `apply_patch`. Don't edit files through bash scripts (python or perl, `sed` on many files): in auto they need the user, and tool edits show as diffs.|auto: this bash call needs the user. Use `edit`: it runs without asking. If bash is really needed, repeat the call and the user will be asked.|auto: this bash call needs the user. Use `apply_patch`: it runs without asking. If bash is really needed, repeat the call and the user will be asked." : String}

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

law edit_err_not_found:
  {Ed.err_not_found("foo") == "String to replace not found in file.\nString: foo" : String}

law edit_err_many:
  {Ed.err_many(3n, "x") == "Found 3 matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.\nString: x" : String}

law edit_err_not_text:
  {Ed.err_not_text("/w/b.bin", 2n) == "Cannot edit /w/b.bin: file is not valid text (utf-8, byte 2)" : String}

law edit_err_permission:
  {Ed.err_permission("/w/a") == "Permission denied accessing file: /w/a" : String}

law edit_ok_messages:
  {String.join([Ed.ok_msg(False{}), Ed.ok_msg(True{})], "|")
    == "The file has been updated successfully.|The file has been updated. All occurrences were successfully replaced" : String}

law edit_result_text:
  {Ed.edit_result("/w/a", Ed.ok_one(), "x", "y")
    == "file: /w/a\nmessage: The file has been updated successfully.\nold_string: x\nnew_string: y" : String}

law write_err_empty_path:
  {Ed.werr_empty_path() == "Path cannot be empty" : String}

law write_err_too_big:
  {Ed.werr_too_big() == "Content exceeds 64000 bytes limit" : String}

law write_err_exists:
  {Ed.werr_exists("/w/a") == "File '/w/a' already exists. Use edit to modify it." : String}

law write_result_text:
  {Ed.write_result("/w/n.txt", Ed.utf8_len("hé\n"), "hé\n")
    == "file_path: /w/n.txt\nbytes_written: 4\ncontent: hé\n" : String}

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
    == (Ed.EdFail{"String to replace not found in file.\nString: c"},
        (Ed.EdFail{"Found 2 matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.\nString: a"},
         (Ed.EdOk{"a zz c", "The file has been updated successfully."},
          Ed.EdOk{"xa b x", "The file has been updated. All occurrences were successfully replaced"})))
    : Ed.Edited & (Ed.Edited & (Ed.Edited & Ed.Edited))}

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
  {(W.is_structured("edit"), (W.is_structured("write_file"), W.is_structured("apply_patch")))
    == (True{}, (True{}, False{})) : Bool & (Bool & Bool)}

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

# ---- the structured search (runtime/tools.bend) ----
# best_match returns ranked names without declarations, details returns
# declarations and NEVER a silent empty, the flat syntax stays for the
# scripted suite.

# LAW: best_match returns the exact function NAME, not its declaration.
law best_match_names_only:
  {Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"echo thing\"}",
      Con{T.ToolDef{"echo.thing", "a loud description"}, Nil{}})
    == "echo.thing" : String}

# LAW: a two-word match ranks above a one-word match.
law best_match_ranks_words:
  {Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"echo thing\"}",
      Con{T.ToolDef{"other.thing", "d"},
        Con{T.ToolDef{"echo.thing", "d"}, Nil{}}})
    == "echo.thing\nother.thing" : String}

# LAW: best_match caps at 20 names and flags that more exist (the
# a21.zz tool must NOT appear).
law best_match_caps_and_flags:
  {Bool.and(
      String.contains(
        Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"zz\"}",
          Con{T.ToolDef{"a1.zz", "d"},
      Con{T.ToolDef{"a2.zz", "d"},
      Con{T.ToolDef{"a3.zz", "d"},
      Con{T.ToolDef{"a4.zz", "d"},
      Con{T.ToolDef{"a5.zz", "d"},
      Con{T.ToolDef{"a6.zz", "d"},
      Con{T.ToolDef{"a7.zz", "d"},
      Con{T.ToolDef{"a8.zz", "d"},
      Con{T.ToolDef{"a9.zz", "d"},
      Con{T.ToolDef{"a10.zz", "d"},
      Con{T.ToolDef{"a11.zz", "d"},
      Con{T.ToolDef{"a12.zz", "d"},
      Con{T.ToolDef{"a13.zz", "d"},
      Con{T.ToolDef{"a14.zz", "d"},
      Con{T.ToolDef{"a15.zz", "d"},
      Con{T.ToolDef{"a16.zz", "d"},
      Con{T.ToolDef{"a17.zz", "d"},
      Con{T.ToolDef{"a18.zz", "d"},
      Con{T.ToolDef{"a19.zz", "d"},
      Con{T.ToolDef{"a20.zz", "d"},
      Con{T.ToolDef{"a21.zz", "d"},
      Nil{}}}}}}}}}}}}}}}}}}}}}}),
        "moreCandidatesAvailable: true"),
      Bool.not(String.contains(
        Xt.search_reply("{\"mode\": \"best_match\", \"query\": \"zz\"}",
          Con{T.ToolDef{"a1.zz", "d"},
      Con{T.ToolDef{"a2.zz", "d"},
      Con{T.ToolDef{"a3.zz", "d"},
      Con{T.ToolDef{"a4.zz", "d"},
      Con{T.ToolDef{"a5.zz", "d"},
      Con{T.ToolDef{"a6.zz", "d"},
      Con{T.ToolDef{"a7.zz", "d"},
      Con{T.ToolDef{"a8.zz", "d"},
      Con{T.ToolDef{"a9.zz", "d"},
      Con{T.ToolDef{"a10.zz", "d"},
      Con{T.ToolDef{"a11.zz", "d"},
      Con{T.ToolDef{"a12.zz", "d"},
      Con{T.ToolDef{"a13.zz", "d"},
      Con{T.ToolDef{"a14.zz", "d"},
      Con{T.ToolDef{"a15.zz", "d"},
      Con{T.ToolDef{"a16.zz", "d"},
      Con{T.ToolDef{"a17.zz", "d"},
      Con{T.ToolDef{"a18.zz", "d"},
      Con{T.ToolDef{"a19.zz", "d"},
      Con{T.ToolDef{"a20.zz", "d"},
      Con{T.ToolDef{"a21.zz", "d"},
      Nil{}}}}}}}}}}}}}}}}}}}}}}),
        "a21.zz")))
    == True{} : Bool}

# LAW: details on a missing name never comes back silently empty — the
# empty message and the missing list (a silent "" once left the model
# retrying blind).
law details_missing_is_loud:
  {Xt.search_reply("{\"mode\": \"details\", \"functions\": [\"no.such\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "No exact tool function details loaded. Call best_match/all_connector_capabilities first, then request exact returned function names.\n# Missing Exact Names\n- No exact tool function found for: no.such"
    : String}

# LAW: details with a found and a missing name shows both parts.
law details_show_found_and_missing:
  {Xt.search_reply(
      "{\"mode\": \"details\", \"functions\": [\"echo.thing\", \"no.such\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n\n# Missing Exact Names\n- No exact tool function found for: no.such"
    : String}

# LAW: the deprecated functionNames alias resolves like functions.
law details_alias_resolves:
  {Xt.search_reply("{\"mode\": \"details\", \"functionNames\": [\"echo.thing\"]}",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n" : String}

# LAW: the flat scripted syntax still resolves (the suite's hermetic
# path — non-JSON args fall back).
law flat_details_still_works:
  {Xt.search_reply("details: echo.thing",
      Con{T.ToolDef{"echo.thing", "desc"}, Nil{}})
    == "echo.thing: desc\n" : String}

# LAW: a connectors filter restricts best_match to that connector.
law best_match_connectors_filter:
  {Xt.search_reply(
      "{\"mode\": \"best_match\", \"query\": \"thing\", \"connectors\": [\"echo\"]}",
      Con{T.ToolDef{"other.thing", "d"},
        Con{T.ToolDef{"echo.thing", "d"}, Nil{}}})
    == "echo.thing" : String}

# ---- the TUI feed annotations (runtime/main.bend) ----
# One line each, single-line, capped — every client parses these.

# LAW: the tool annotation carries the id and the model-facing name.
law tool_ann_format:
  {Rt.call_ann(T.Call{12, "bash", "ls -la"}) == "tool #12 bash : ls -la"
    : String}

# LAW: results annotate with the outcome and a preview.
law tool_res_ann_ok:
  {Rt.res_ann(12, Xt.Ex{True{}, "hello"}) == "tool_result #12 ok : hello"
    : String}

law tool_res_ann_fail:
  {Rt.res_ann(12, Xt.Ex{False{}, "boom"}) == "tool_result #12 fail : boom"
    : String}

# LAW: program sub-calls annotate with name and outcome.
law subtool_ann_format:
  {Rt.sub_ann("github_app.get_me", Xt.Ex{True{}, "{}"})
    == "subtool github_app.get_me ok : {}" : String}

# LAW: annotations stay single-line — one wire line each.
law ann_flat_single_line:
  {Bool.not(String.contains(Rt.ann_flat("a\nb"), "\n")) == True{} : Bool}

# LAW: annotations cap at 200 chars (a 210-char payload trims to 200).
law ann_flat_caps_at_200:
  {String.length(Rt.ann_flat("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")) == 200n : Nat}

# LAW: an engine error payload hands its message back — the model
# reads exit-43 errors and retries.
law node_err_extracts_message:
  {Rt.node_err("{\"error\": \"missing required parameter: owner\"}")
    == "missing required parameter: owner" : String}



# ---- MCP connector parity (vibe connector_registry) ----

# LAW: the published alias is a clean identifier (vibe_sdk requires a
# TypeScript identifier for tools.<alias>.<function>): everything
# outside [a-zA-Z0-9_-] flattens to "_", edges strip, "" is "unnamed".
law mcp_alias_normalizes:
  {Bool.and(String.eq(Mc.normalize_name("github app!"), "github_app"),
     Bool.and(String.eq(Mc.normalize_name("--x.y--"), "x_y"),
       Bool.and(String.eq(Mc.normalize_name("ok-name_1"), "ok-name_1"),
         String.eq(Mc.normalize_name(""), "unnamed")))) == True{} : Bool}

# LAW: colliding normalized aliases get _2, _3... (vibe dedupe).
law mcp_alias_dedupes:
  {Bool.and(
     String.eq(Mc.dedupe("github", Nil{}), "github"),
     Bool.and(
       String.eq(Mc.dedupe("github", Con{"github", Nil{}}), "github_2"),
       String.eq(Mc.dedupe("github",
         Con{"github", Con{"github_2", Nil{}}}), "github_3")))
    == True{} : Bool}

# LAW: the bootstrap hits the same endpoint as the vibe CLI, with the
# same params: the web_search builtin is exposed as an MCP tool and
# auth-actionable connectors are included (ready ones index only).
law mcp_bootstrap_url_parity:
  {Bool.and(
     String.contains(Mc.bootstrap_url(),
       "/v1/connectors/bootstrap?supports_mcp=true"),
     Bool.and(
       String.contains(Mc.bootstrap_url(),
         "include_auth_actionable_connectors=true"),
       String.contains(Mc.bootstrap_url(), "builtin_connectors=web_search")))
    == True{} : Bool}

# ---- bash background handoff (runtime/bash.bend) ----
# A command that outlives the sync window must never block the turn: it
# detaches under a per-id directory the agent drives with ordinary bash.
# The wrapper is pinned byte for byte — output to a FILE (never a pipe,
# so no orphan can hold the read), the fifo held RDWR (no EOF), the
# reaper writing .rc, the exit-199 contract, the slot cleanup.

# LAW: the wrapper script, byte for byte.
law bg_wrapper_script:
  {Sh.bg_wrap("whoami", "/tmp/bend-bg-0", 30, "/i")
    == "BG=/tmp/bend-bg-0
IF=/i
unset BEND_SESSION_FILE BEND_CONTEXT_FILE BEND_WIRE_LOG BEND_REPL_PORT BEND_EXTRA_PROMPT BEND_CONTINUE BEND_CRASH_NOTE BEND_DEBUG_DIR
mkdir -p $BG
i=0
f=0
while [ -e $BG/$i.out ] || [ -e $BG/$i.rc ] || ! mkdir $BG/$i.slot 2>/dev/null; do
  if [ -e $BG/$i.slot ] || [ -e $BG/$i.out ] || [ -e $BG/$i.rc ]; then i=$((i+1)); else f=$((f+1)); fi
  if [ $f -ge 20 ]; then echo \"background: cannot create a slot in /tmp/bend-bg-0 (disk full or not writable?)\"; rm -f \"$0\"; exit 1; fi
done
mkfifo $BG/$i.in 2>/dev/null
exec 9<>$BG/$i.in
cat > $BG/$i.cmd <<'XQ-3131618372-EOF'
whoami
XQ-3131618372-EOF
if [ $? -ne 0 ]; then echo \"background: could not write /tmp/bend-bg-0/$i.cmd (disk full?)\"; rm -f $BG/$i.in $BG/$i.cmd; rmdir $BG/$i.slot 2>/dev/null; rm -f \"$0\"; exit 1; fi
L=${BEND_MEM_LIMIT_MB:-}; if [ -z \"$L\" ]; then m=$(sysctl -n hw.memsize 2>/dev/null); m=$((${m:-0}/4194304)); if [ $m -eq 0 ] || [ $m -gt 8192 ]; then m=8192; fi; if [ $m -lt 2048 ]; then m=2048; fi; L=$m; fi
( ulimit -f 102400; set -m; ( BEND_STDIN=$BG/$i.in /bin/sh $BG/$i.cmd ) > $BG/$i.out 2>&1 </dev/null 9<&- & C=$!; set +m; echo $C > $BG/$i.pid; if [ \"$L\" -gt 0 ] 2>/dev/null; then ( while kill -0 -$C 2>/dev/null; do sleep 2; k=$(ps -axo pgid=,pid=,rss=,ucomm= 2>/dev/null | awk -v g=$C -v l=$((L*1024)) '$1==g && $3>l {print \"pid \" $2 \" (\" $4 \") at \" int($3/1024) \" MB\"; exit}'); if [ -n \"$k\" ]; then kill -9 -$C 2>/dev/null; if { [ -d $BG/$i.slot ] && [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; } || { [ ! -d $BG/$i.slot ] && [ -e $BG/$i.rc ]; }; then echo \"[bash: killed $k of RAM, over the limit of $L MB per process (BEND_MEM_LIMIT_MB, 0 = no limit). A process that keeps growing is a leak or a command that never ends: e.g. its output piped into grep -m1 or head, or a CLI left running in the background]\" >> $BG/$i.out; fi; break; fi; done ) >/dev/null 2>&1 & fi; wait $C; R=$?; if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then echo $R > $BG/$i.rc; rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in; rmdir $BG/$i.slot 2>/dev/null; fi; exit $R ) >/dev/null 2>&1 &
r=$!
sleep 30 >/dev/null 2>&1 </dev/null 9<&- & t=$!
trap 'kill $t 2>/dev/null; exit 143' HUP INT TERM
e=0
x=
while kill -0 $r 2>/dev/null; do
  if [ $e -lt 5 ]; then sleep 0.02; else sleep 0.1; fi
  e=$((e+1))
  if [ -s \"$IF\" ] && [ -s $BG/$i.pid ] && grep -q '[^[:space:]]' \"$IF\" 2>/dev/null; then x=$(tr -d '\\n\\r' < \"$IF\"); case \"$x\" in \"\"|1|user) x=\"the user\";; esac; C=$(cat $BG/$i.pid 2>/dev/null); kill -TERM -$C 2>/dev/null; n=0; while kill -0 -$C 2>/dev/null && [ $n -lt 5 ]; do sleep 0.1; n=$((n+1)); done; kill -KILL -$C 2>/dev/null; break; fi
  if ! kill -0 $t 2>/dev/null; then
    printf '%s\\n' \"BEND-BG-HANDOFF-6f2c9e5b\"
    printf '%s\\n' \"background: still running after 30s - id $i (dir /tmp/bend-bg-0)\"
    printf '%s\\n' \"  output: tail -n 50 /tmp/bend-bg-0/$i.out\"
    printf '%s\\n' \"  stdin (only if the command reads the fifo in BEND_STDIN): printf '%s\\n' 'text' > /tmp/bend-bg-0/$i.in  (otherwise its stdin is /dev/null)\"
    printf '%s\\n' \"  status: kill -0 $(cat /tmp/bend-bg-0/$i.pid) && echo RUNNING || cat /tmp/bend-bg-0/$i.rc 2>/dev/null\"
    printf '%s\\n' \"  kill: kill $(cat /tmp/bend-bg-0/$i.pid)\"
    if [ ! -s /tmp/bend-bg-0/$i.pid ]; then echo \"background: could not write /tmp/bend-bg-0/$i.pid (disk full?) - the command runs under the runner pid $r\"; fi
    rm -f \"$0\"
    exit 0
  fi
done
kill $t 2>/dev/null; wait $t 2>/dev/null
wait $r
s=$?
if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished before the exit code was recorded: /tmp/bend-bg-0\"; rm -f \"$0\"; exit 1; fi
if [ ! -f /tmp/bend-bg-0/$i.out ]; then echo \"background: could not write /tmp/bend-bg-0/$i.out (disk full?)\"; fi
n=$(wc -c < $BG/$i.out 2>/dev/null); n=$((${n:-0}+0)); if [ $n -gt 1048576 ]; then echo \"[bash: the output is $n bytes; only its first 1048576 follow. The rest is lost: redirect a big output to a file and read it in parts]\"; head -c 1048576 $BG/$i.out; echo; else cat $BG/$i.out 2>/dev/null; fi
if [ ! -s /tmp/bend-bg-0/$i.rc ]; then echo \"background: could not write /tmp/bend-bg-0/$i.rc (disk full?) - the output above may be incomplete\"; fi
if [ -n \"$x\" ]; then echo \"[bash: interrupted by $x: the command was stopped (TERM to its process group, then KILL); its output so far is above. Check what it changed before you run it again]\"; s=130; fi
rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc 2>/dev/null
rmdir $BG/$i.slot 2>/dev/null
rm -f \"$0\"
exit $s"
    : String}

# LAW: the sync window defaults# LAW: the sync window defaults to 30 seconds and clamps into [0, 3600].
law bg_after_defaults:
  {Sh.bg_after.read(None{}, 30) == 30 : U32}

law bg_after_clamps_high:
  {Sh.bg_after.clamp(4000) == 3600 : U32}

law bg_after_keeps_low:
  {Sh.bg_after.clamp(5) == 5 : U32}

# LAW: the handoff maps to a SUCCESSFUL result: the wrapper prints the
# handoff marker as its first line, exec_of strips it and keeps the
# contract — the turn continues and the agent decides.
law exec_of_background_maps_ok:
  {Sh.exec_of(String.append("0\n", String.append(Sh.HANDOFF_MARKER(),
      "\nbackground: still running")))
    == Xt.Ex{True{}, "background: still running"} : Xt.ExecRes}

# LAW: a command that REALLY exits 199 reports as a failure with its
# code — the handoff is detected by the marker, never by the code
# (once: exit 199 was silently swallowed as a handoff).
law exec_of_real_199_fails:
  {Sh.exec_of("199\nreal output")
    == Xt.Ex{False{}, "exit 199: real output"} : Xt.ExecRes}

# LAW: ordinary failures still report their exit code.
law exec_of_exit_still_fails:
  {Sh.exec_of.pick.zero(False{}, 1, "boom")
    == Xt.Ex{False{}, "exit 1: boom"} : Xt.ExecRes}


# ---- the background battery: solidity and agent ergonomics ----

# LAW: the bg dir is per-REPL, keyed by port — parallel terminals
# never share slots, and a /reload (same port) finds its slots again.
law bg_dir_default_root:
  {Sh.bg_dir_of("/tmp", "7702") == "/tmp/bend-bg-7702" : String}

law bg_dir_custom_root:
  {Sh.bg_dir_of("/var/bg", "3") == "/var/bg/bend-bg-3" : String}

# ---- the agent's own folders (approvals-tmp, approvals-design.md §7.1) ----

# LAW: the harness's own files go to the agent's run/ folder when the hub
# names it (BEND_AGENT_RUN), else to $TMPDIR (a headless run), and only
# with neither to /tmp; the trailing "/" of a macOS $TMPDIR is dropped.
law side_dir_is_the_agent_run:
  {Sh.side_dir_of("/h/agents/a/run", "/var/T/") == "/h/agents/a/run" : String}

law side_dir_falls_back_to_tmpdir:
  {Sh.side_dir_of("", "/var/folders/x/T/") == "/var/folders/x/T" : String}

law side_dir_last_resort_tmp:
  {Sh.side_dir_of("", "") == "/tmp" : String}

law dir_trim_keeps_root:
  {Sh.dir_trim("/") == "/" : String}

# LAW: the bg dir is the one the hub names (the agent's tmp/bg, so the
# agent reads its jobs' output in its own temp folder), else per port
# under BEND_BG_ROOT, else under $TMPDIR, else under /tmp.
law bg_dir_is_the_agent_tmp_bg:
  {Sh.bg_dir_pick("/h/agents/a/tmp/bg", "/r", "/var/T/", "7")
    == "/h/agents/a/tmp/bg" : String}

law bg_dir_root_then_tmpdir:
  {Bool.and(String.eq(Sh.bg_dir_pick("", "/r", "/var/T/", "7"), "/r/bend-bg-7"),
     Bool.and(String.eq(Sh.bg_dir_pick("", "", "/var/T/", "7"), "/var/T/bend-bg-7"),
       String.eq(Sh.bg_dir_pick("", "", "", "7"), "/tmp/bend-bg-7")))
    == True{} : Bool}

# LAW: the boot cleanup removes this port's wrappers in the side dir,
# the folder quoted as one shell word.
law bash_cleanup_cmd_quotes_the_dir:
  {Sh.bash_cleanup_cmd("/a b/run", "7") == "rm -f '/a b/run'/bend-sh-7-*.sh"
    : String}

# LAW: one slot = five files in one dir under one id.
law bg_slot_paths:
  {Bool.and(String.eq(Sh.bg_slot("/d", "4", ".out"), "/d/4.out"),
     Bool.and(String.eq(Sh.bg_slot("/d", "4", ".in"), "/d/4.in"),
       Bool.and(String.eq(Sh.bg_slot("/d", "4", ".pid"), "/d/4.pid"),
         Bool.and(String.eq(Sh.bg_slot("/d", "4", ".rc"), "/d/4.rc"),
           String.eq(Sh.bg_slot("/d", "4", ".cmd"), "/d/4.cmd")))))
    == True{} : Bool}

# LAW: the sync window is a clock, never a count of poll turns: a
# `sleep <after>` timer beside the runner, and the poll hands off when
# the timer is gone. (A count of `sleep 0.1` turns ran slow by the
# exec of each sleep: under load a 6 s window lasted 8 s and a 30 s
# one up to 40 s.) The timer holds no pipe and no fifo, is killed on
# a sync end and by the trap when the wrapper is stopped. The poll
# still probes fast first (5 x 20 ms), then 100 ms steps.
law bg_wrapper_window_is_a_clock:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "r=$!
sleep 5 >/dev/null 2>&1 </dev/null 9<&- & t=$!
trap 'kill $t 2>/dev/null; exit 143' HUP INT TERM
"),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "  if ! kill -0 $t 2>/dev/null; then"),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "done
kill $t 2>/dev/null; wait $t 2>/dev/null
wait $r"),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "  if [ $e -lt 5 ]; then sleep 0.02; else sleep 0.1; fi"),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "if [ $e -ge "))))))
    == True{} : Bool}

# LAW: the headline names the window, the slot id and the dir — the
# agent can ls the dir right away.
law bg_headline_names_window_id_dir:
  {Sh.bg_headline("/tmp/bend-bg-0", "3", 30)
    == "background: still running after 30s - id 3 (dir /tmp/bend-bg-0)"
    : String}

# LAW: each contract line is a COMPLETE command on a CONCRETE path —
# the agent copy-pastes, it never resolves a variable.
law bg_tail_cmd_concrete:
  {Sh.bg_tail_cmd("/tmp/bend-bg-0", "3")
    == "  output: tail -n 50 /tmp/bend-bg-0/3.out" : String}

law bg_stdin_cmd_concrete:
  {Bool.and(
     String.eq(Sh.bg_stdin_cmd("/tmp/bend-bg-0", "3"),
       "  stdin (only if the command reads the fifo in BEND_STDIN): printf '%s\\n' 'text' > /tmp/bend-bg-0/3.in  (otherwise its stdin is /dev/null)"),
     String.contains(Sh.bg_stdin_cmd("/tmp/bend-bg-0", "3"),
       "BEND_STDIN")) == True{} : Bool}

law bg_status_cmd_concrete:
  {Sh.bg_status_cmd("/tmp/bend-bg-0", "3")
    == "  status: kill -0 $(cat /tmp/bend-bg-0/3.pid) && echo RUNNING || cat /tmp/bend-bg-0/3.rc 2>/dev/null"
    : String}

law bg_kill_cmd_concrete:
  {Sh.bg_kill_cmd("/tmp/bend-bg-0", "3")
    == "  kill: kill $(cat /tmp/bend-bg-0/3.pid)" : String}

# ---- wrapper invariants (structure, not bytes) ----

# LAW: the fifo is held RDWR by the wrapper — never EOF for the
# reader, never blocks at open.
# LAW: every wait is total — the wrapper polls its own child, the
# runner ($!), and takes the exit code from `wait` (the runner exits
# with it), never from a file. Once: on a full disk the runner could
# not write the .rc, the wrapper waited for it forever, and a bash call
# hung 19 minutes.
law bg_wrapper_waits_on_runner:
  {Bool.and(
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "fi; exit $R ) >/dev/null 2>&1 &
r=$!"),
       String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "while kill -0 $r 2>/dev/null; do")),
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "wait $r
s=$?"),
       Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "while [ ! -e $BG/$i.rc ]"))))
    == True{} : Bool}

# LAW: a slot file the runner could not write is named to the model —
# the call returns, and says why a file is missing.
law bg_wrapper_reports_unwritable_files:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_write_note("! -f", "/d", ".out", " (disk full?)")),
     Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "could not write /d/$i.rc (disk full?)"),
       String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_cmd_fail("/d"))))
    == True{} : Bool}

law bg_wrapper_fifo_rdrw:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "exec 9<>$BG/$i.in")
    == True{} : Bool}

# LAW (issue #9): the command's stdin is /dev/null, never the fifo: a
# command that reads a non-tty stdin (rg with no path, cat) gets EOF at
# once instead of waiting out the sync window. The fifo is reached by
# name only ($BEND_STDIN), and the command does not inherit fd 9.
law bg_wrapper_stdin_devnull:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "( BEND_STDIN=$BG/$i.in /bin/sh $BG/$i.cmd ) > $BG/$i.out 2>&1 </dev/null 9<&- &"),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "<&9")))
    == True{} : Bool}

# LAW: a detached runner is the command's real parent and records the
# TRUE exit code (a sibling cannot wait — that produced 127). The
# record is ownership-checked: the rc lands only if the runner's pid
# still owns the slot.
law bg_wrapper_runner_waits_and_records:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "C=$!; set +m; echo $C > $BG/$i.pid; "),
   Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), " & fi; wait $C; R=$?"),
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "echo $R > $BG/$i.rc")))
    == True{} : Bool}

# LAW: output goes to a FILE, never a pipe — the orphan-holding-pipe
# bug class (minute-long hangs) is structurally gone. The one pipe of
# the wrapper is the memory watchdog's own ps | awk, read by $( ) inside
# the watchdog: no command output goes through it.
law bg_wrapper_output_is_a_file:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), " /bin/sh $BG/$i.cmd ) > $BG/$i.out 2>&1"),
   Bool.and(String.contains(Sh.bg_mem_watch(), "k=$(ps -axo pgid=,pid=,rss=,ucomm= 2>/dev/null | awk "),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), ".cmd ) |"))))
    == True{} : Bool}

# LAW: the wrapper NEVER kills the command — ending a background
# command is the agent's decision. One exception, for the machine: the
# memory watchdog kills the command's process group (kill -9 -$C) once
# one of its processes is over the limit; no other kill -9 exists.
law bg_wrapper_never_kills:
  {Bool.and(Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "kill $c")),
     Bool.and(Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "kill -9 $")),
       String.contains(Sh.bg_mem_watch(), "$3>l {print")))
    == True{} : Bool}

# LAW: every command runs in its own process group under the memory
# watchdog (the vercel deploys of 2026-10-01: 32 + 17 GB of node left
# running, the Mac out of memory). The group survives an orphaned child,
# so `( cmd & )` and a pipe whose reader quit are watched too; the watch
# lasts while any process of the group lives; the limit has an override.
# (bg_wrapper_script pins both lines in the wrapper, byte for byte)
law bg_wrapper_watches_memory:
  {Bool.and(String.contains(Sh.bg_mem_watch(), "while kill -0 -$C"),
   Bool.and(String.contains(Sh.bg_mem_watch(), "kill -9 -$C"),
     String.contains(Sh.bg_mem_limit(), "L=${BEND_MEM_LIMIT_MB:-}")))
    == True{} : Bool}

# LAW: a command that finishes inside the window leaves no trace —
# every slot file is removed.
# LAW: a background command that ends frees its slot (pid, cmd and
# fifo are removed by the runner; out and rc stay readable) — the bg
# dir cannot grow without bound and slots are recycled.
law bg_wrapper_runner_frees_slot:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
     "rm -f $BG/$i.pid $BG/$i.cmd $BG/$i.in") == True{} : Bool}

law bg_wrapper_cleans_slot:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
     "rm -f $BG/$i.in $BG/$i.out $BG/$i.pid $BG/$i.cmd $BG/$i.rc")
    == True{} : Bool}

# LAW: the handoff prints the marker as the wrapper's FIRST output
# line and exits 0 — the marker (not an exit code) is the signal, so a
# command that really exits 199 still reports 199.
law bg_wrapper_handoff_marker:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       String.append("printf '%s\\n' \"", Sh.HANDOFF_MARKER())),
     Bool.not(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "exit 199")))
    == True{} : Bool}

# LAW: the wrapper carries a vanished-dir guard — an agent that cleans
# its bg dir with rm -rf gets a FAST failure with the dir name, never
# a hung tool.
law bg_rc_guard_names_the_dir:
  {String.contains(Sh.bg_rc_guard("/tmp/bend-bg-0"),
     "if [ ! -d /tmp/bend-bg-0 ]; then echo \"background: the bg dir vanished before the exit code was recorded: /tmp/bend-bg-0\"; rm -f \"$0\"; exit 1; fi")
    == True{} : Bool}

# LAW: the wrapper embeds both guards (the waits are all bounded by
# the dir check).
# LAW: slot allocation is an atomic mkdir CAS — two wrappers starting
# at the same instant can never take the same slot (once: concurrent
# calls collided on id 0 and corrupted each other's out/rc). It is
# bounded: a mkdir failing for another reason than an existing slot
# (full disk, read-only dir) ends the call after 20 tries (once: the
# loop retried forever).
law bg_wrapper_atomic_slot_allocation:
  {Bool.and(
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "|| ! mkdir $BG/$i.slot 2>/dev/null; do
  if [ -e $BG/$i.slot ] || [ -e $BG/$i.out ] || [ -e $BG/$i.rc ]; then i=$((i+1)); else f=$((f+1)); fi"),
     String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_slot_fail("/d")))
    == True{} : Bool}

# LAW: a slot that still holds a finished job's results (.out, .rc) is
# never handed to the next command: recycling it truncated the job's
# output, and a next command that cat'd that .out fed its own output
# back into itself up to the ulimit (a 100 MiB result: "bend: memory
# fault" in three agents).
law bg_wrapper_keeps_finished_results:
  {String.starts_with(Sh.bg_slot_loop(),
     "while [ -e $BG/$i.out ] || [ -e $BG/$i.rc ] || ")
    == True{} : Bool}

# LAW: what the REPL reads back is bounded at 1 MiB, with a line that
# gives the real size (the REPL died on a 100 MiB String).
law bg_wrapper_output_back_bounded:
  {Bool.and(String.eq(Sh.bg_out_max(), "1048576"),
     Bool.and(String.contains(Sh.bg_cat_out("/d"), "head -c 1048576 $BG/$i.out"),
       String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_cat_out("/d"))))
    == True{} : Bool}

# LAW: the command never inherits the REPL's private environment (its
# session file, wire log, context file, port): a REPL started from an
# agent's shell wrote its turns into the live agent's wire log.
law bg_wrapper_unsets_repl_env:
  {Bool.and(String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
       "
unset BEND_SESSION_FILE BEND_CONTEXT_FILE BEND_WIRE_LOG BEND_REPL_PORT "),
     Bool.not(String.contains(Sh.bg_unset_private(), "SB_SOCKET")))
    == True{} : Bool}

# LAW: the runner caps the process's file writes (ulimit -f 102400 =
# 50MiB) — an output-looping background command dies at the cap
# instead of filling the disk (once: a 6.4GB .out).
law bg_wrapper_output_capped:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "( ulimit -f 102400; set -m; ( ")
    == True{} : Bool}

# LAW: the runner checks it still OWNS the slot before writing the rc
# and freeing the files — a recycled slot cannot be corrupted by the
# previous command's dying runner.
law bg_wrapper_runner_ownership_check:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"),
     "if [ \"$(cat $BG/$i.pid 2>/dev/null)\" = \"$C\" ]; then") == True{}
    : Bool}

# LAW: both exit paths delete the wrapper script ($0) — no more
# +1 leaked script per bash call (the byte law pins both occurrences).
law bg_wrapper_cleans_its_script:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), "rm -f \"$0\"") == True{}
    : Bool}

# LAW: multi-line commands stay intact — the heredoc and the runner
# subshell carry them verbatim (flatten once silently joined lines
# with spaces, restructuring scripts).
law bg_wrapper_multiline_command_intact:
  {Bool.and(
     String.contains(Sh.bg_wrap("echo a\necho b", "/d", 5, "/i"), "echo a\necho b"),
     Bool.not(String.contains(Sh.bg_wrap("echo a\necho b", "/d", 5, "/i"),
       "echo a echo b"))) == True{} : Bool}

# LAW: the .cmd heredoc sentinel derives from the command's own hash —
# a command cannot contain its own sentinel without containing its
# own hash.
law bg_sentinel_derives_from_command:
  {Bool.and(
     String.eq(Sh.bg_sentinel("whoami"), Sh.bg_sentinel("whoami")),
     Bool.not(String.eq(Sh.bg_sentinel("whoami"),
       Sh.bg_sentinel("whoamiX")))) == True{} : Bool}

law bg_wrapper_has_the_guard:
  {String.contains(Sh.bg_wrap("x", "/d", 5, "/i"), Sh.bg_rc_guard("/d"))
    == True{} : Bool}

# LAW: the wrapper filename derives from the script's own content:
# re-running the same command rewrites IDENTICAL bytes (an orphaned
# wrapper just continues its own script), and the hash is pinned so
# the naming cannot silently degrade.
law bg_script_hash_deterministic:
  {Sh.bg_script_hash("ab") == 1294271946 : U32}

# LAW: the hash is a U32 - code points past 2^20 included (the former
# Nat sum overflowed the runtime's 2^48-1 and killed the REPL).
law bg_script_hash_high_code_points:
  {Sh.bg_script_hash("é—􏿿") == 1694011805 : U32}

# LAW: different scripts hash apart — two commands never share a
# wrapper file, so an orphan can never re-read another command's
# bytes when it wakes from a poll sleep.
law bg_script_hash_distinguishes:
  {Bool.not(String.eq(
      U32.show(Sh.bg_script_hash("ab")),
      U32.show(Sh.bg_script_hash("cd"))))
    == True{} : Bool}

law bash_script_path_per_command:
  {Bool.and(String.eq(Sh.bash_script_path("/r", "7702", "a b"),
      Sh.bash_script_path("/r", "7702", "a b")),
     Bool.not(String.eq(Sh.bash_script_path("/r", "7702", "a b"),
       Sh.bash_script_path("/r", "7702", "a c")))) == True{} : Bool}

# LAW: the wrapper scripts live in the side dir.
law bash_script_path_in_side_dir:
  {String.starts_with(Sh.bash_script_path("/h/agents/a/run", "7", "ls"),
     "/h/agents/a/run/bend-sh-7-") == True{} : Bool}

# ---- the sandbox (approvals-sandbox, approvals-design.md §6) ----

# LAW: the allow line's flags pick the profile in the agent's run/
# folder: none, the closed one, or the network-open one.
law sandbox_profile_none_without_the_flag:
  {Sh.sandbox_profile("/h/agents/a/run", "") == "" : String}

law sandbox_profile_closed:
  {Sh.sandbox_profile("/h/agents/a/run", "sandbox")
    == "/h/agents/a/run/sandbox.sb" : String}

law sandbox_profile_net:
  {Sh.sandbox_profile("/h/agents/a/run", "sandbox net")
    == "/h/agents/a/run/sandbox-net.sb" : String}

# LAW: with no profile the wrapper runs as before; with one, under
# sandbox-exec (background jobs inherit it: they are the wrapper's
# children).
law sh_argv_plain:
  {Sh.sh_argv("", "/r/bend-sh-7-1.sh") == Con{"/bin/sh", Con{"/r/bend-sh-7-1.sh", Nil{}}}
    : List<&2, String>}

law sh_argv_sandboxed:
  {Sh.sh_argv("/r/sandbox.sb", "/r/bend-sh-7-1.sh")
    == Con{"/usr/bin/sandbox-exec", Con{"-f", Con{"/r/sandbox.sb",
         Con{"/bin/sh", Con{"/r/bend-sh-7-1.sh", Nil{}}}}}} : List<&2, String>}

# LAW: a failed run that shows a refused write or a failed network call
# was stopped by the sandbox; a success never was, nor another failure.
law sandbox_stopped_write:
  {Sh.sandbox_stopped(Xt.Ex{False{},
     "exit 1: /bin/sh: /Users/u/Desktop/x.txt: Operation not permitted"}) == True{} : Bool}

law sandbox_stopped_network:
  {Sh.sandbox_stopped(Xt.Ex{False{},
     "exit 6: curl: (6) Could not resolve host: example.com"}) == True{} : Bool}

law sandbox_stopped_not_on_success:
  {Sh.sandbox_stopped(Xt.Ex{True{}, "Operation not permitted"}) == False{} : Bool}

law sandbox_stopped_not_on_a_test_failure:
  {Sh.sandbox_stopped(Xt.Ex{False{}, "exit 101: test result: FAILED"}) == False{} : Bool}

# ---- exec_of: the tool result the model actually reads ----

# LAW: exit 0 is a success carrying the output.
law exec_of_success_is_ok:
  {Sh.exec_of("0\nhello") == Xt.Ex{True{}, "hello"} : Xt.ExecRes}

# LAW: an ordinary failure reports its code and output.
law exec_of_failure_carries_code:
  {Sh.exec_of("3\nbad") == Xt.Ex{False{}, "exit 3: bad"} : Xt.ExecRes}

# LAW: a garbage status line fails closed as exit 1 (never a success).
law exec_of_garbage_fails_closed:
  {Sh.exec_of("not-a-code\nx") == Xt.Ex{False{}, "exit 1: x"} : Xt.ExecRes}

# LAW: a result with no status line at all fails closed too.
law exec_of_no_newline_fails_closed:
  {Sh.exec_of("plain") == Xt.Ex{False{}, "exit 1: "} : Xt.ExecRes}


# ---- the tool output cap (runtime/tools.bend) ----

# LAW: outputs that fit pass through untouched.
law cap_out_keeps_small:
  {Xt.cap_out("hello") == "hello" : String}

# LAW: the cap limit is 20000.
law cap_out_limit_is_20000:
  {Xt.cap_out_limit() == 20000n : Nat}

# LAW: outputs that fit pass through untouched.
law cap_out_at_keeps_small:
  {Xt.cap_out_at(5n, "12345") == "12345" : String}

# LAW: an oversized output is cut at the limit with the marker.
law cap_out_at_truncates:
  {Xt.cap_out_at(5n, "12345678")
    == "12345  [output truncated at 20000 chars; 3 more chars discarded]"
    : String}

# ---- the provider retry classification (runtime/provider.bend) ----

# LAW: transient statuses retry; client errors and success do not.
law retryable_status_table:
  {Bool.and(
      Bool.and(Pv.retryable_status(500), Pv.retryable_status(503)),
      Bool.and(Pv.retryable_status(429),
        Bool.and(Bool.not(Pv.retryable_status(200)),
          Bool.not(Pv.retryable_status(404)))))
    == True{} : Bool}

# LAW: connection loss, DNS and timeout retry; TLS and URL errors do
# not (retrying a TLS failure or a bad URL only wastes the turn).
law retryable_err_table:
  {Bool.and(
      Bool.and(Pv.retryable_err(Pv.NConnect{1, "refused"}),
        Pv.retryable_err(Pv.NDns{})),
      Bool.and(Pv.retryable_err(Pv.NTimeout{}),
        Bool.and(Bool.not(Pv.retryable_err(Pv.NTls{1, "handshake"})),
          Bool.not(Pv.retryable_err(Pv.NUrl{}))))
    )
    == True{} : Bool}

# LAW: a transient network failure classifies as a retry with the
# failure text carried for the final error if retries run out.
law attempt_timeout_retries:
  {Pv.attempt_err("api.example.com", Pv.NTimeout{})
    == Pv.ARetry{0, "no answer from api.example.com (timeout)"}
    : Pv.Attempt}

# LAW: a TLS failure classifies as final — no retry.
law attempt_tls_fails:
  {Pv.attempt_err("api.example.com", Pv.NTls{1, "handshake"})
    == Pv.AFail{"TLS handshake with api.example.com failed (tls 1 handshake) — check your network or VPN"}
    : Pv.Attempt}

# LAW: the other transient statuses retry too - 529 (Anthropic
# overloaded), 408, 425 - while auth and validation errors do not.
law retryable_status_transient:
  {Bool.and(
      Bool.and(Pv.retryable_status(529), Pv.retryable_status(408)),
      Bool.and(Pv.retryable_status(425),
        Bool.and(Bool.not(Pv.retryable_status(400)),
          Bool.not(Pv.retryable_status(401)))))
    == True{} : Bool}

# LAW: a connection cut mid-response (read, write) and a garbled
# response retry: the request never produced an answer.
law retryable_err_midstream:
  {Bool.and(Pv.retryable_err(Pv.NRead{54, "reset"}),
      Bool.and(Pv.retryable_err(Pv.NWrite{32, "broken pipe"}),
        Pv.retryable_err(Pv.NBad{})))
    == True{} : Bool}

# LAW: ten attempts in total.
law provider_max_attempts:
  {Pv.max_attempts() == 10n : Nat}

# LAW: one read may wait long enough for the longest non-streaming
# reply (32k tokens took 255 s on opus-5.5): 10 minutes.
law provider_step_timeout_covers_long_replies:
  {Pv.step_timeout_ms() == 600000 : U32}

# LAW: the backoff doubles from 1s and caps at 30s; a server hint
# (Retry-After) wins over it.
law provider_backoff_schedule:
  {Bool.and(U32.is_eq(Pv.retry_delay_ms(0, 0n), 1000),
      Bool.and(U32.is_eq(Pv.retry_delay_ms(0, 3n), 8000),
        Bool.and(U32.is_eq(Pv.retry_delay_ms(0, 8n), 30000),
          U32.is_eq(Pv.retry_delay_ms(5000, 2n), 5000))))
    == True{} : Bool}

# LAW: Retry-After is read in seconds, capped at 60s; garbage is no hint.
law provider_retry_after:
  {Bool.and(U32.is_eq(Pv.retry_after_ms(" 12 "), 12000),
      Bool.and(U32.is_eq(Pv.retry_after_ms("600"), 60000),
        U32.is_eq(Pv.retry_after_ms("soon"), 0)))
    == True{} : Bool}

# LAW: a transient status carries the server's Retry-After into the retry.
law provider_529_retries_with_hint:
  {Pv.attempt_status(529, "7", "ERROR provider 529: busy\nEND\n")
    == Pv.ARetry{7000, "provider 529 (transient)"} : Pv.Attempt}

# LAW: a 200 that carries an error body (the proxy's overload answer)
# is a retry, never a final reply.
law provider_200_error_body_retries:
  {Pv.attempt_status(200, "",
      Api.reply_of_for(Api.Anthropic{}, 200,
        "{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}"))
    == Pv.ARetry{0, "provider 200: Overloaded"} : Pv.Attempt}

# LAW: a good response is the reply itself.
law provider_ok_passes_through:
  {Pv.attempt_status(200, "", "OK hi\nEND\n")
    == Pv.AOk{"OK hi\nEND\n"} : Pv.Attempt}

# LAW: a client error is final: the turn fails on the status and the
# body the style mapper kept - retrying it only wastes the user's time.
law provider_client_error_is_final:
  {Pv.attempt_status(400, "", "ERROR provider 400: bad\nEND\n")
    == Pv.AFail{"provider 400: bad"} : Pv.Attempt}

# LAW: an auth error says what to check.
law provider_auth_error_hint:
  {Pv.attempt_status(401, "", "ERROR provider 401: bad key\nEND\n")
    == Pv.AFail{"provider 401: bad key — check the API key"} : Pv.Attempt}

# LAW (BR-007): a refused connection names the host and the fix; it
# retries (a VPN coming back ends the outage).
law attempt_connect_names_host:
  {Pv.attempt_err(Pv.url_host("https://foundry.example.net/anthropic/v1/messages"),
      Pv.NConnect{61, "Connection refused"})
    == Pv.ARetry{0, "cannot reach foundry.example.net (connect 61 Connection refused) — check your network or VPN"}
    : Pv.Attempt}

# LAW (BR-007): the cleared interrupt flag is an EMPTY file - it is not
# an interrupt; neither is a missing file. Only content is.
law interrupt_flag_needs_content:
  {Bool.and(Bool.not(Pv.interrupt_flag(None{})),
      Bool.and(Bool.not(Pv.interrupt_flag(Some{""})),
        Bool.and(Bool.not(Pv.interrupt_flag(Some{"\n"})),
          Pv.interrupt_flag(Some{"1"}))))
    == True{} : Bool}

# LAW (BR-007): an interrupted retry loop keeps the error that made it
# retry - for every error text.
law interrupted_keeps_last_error:
  for +who: String
  for +n: Nat
  for +why: String
  exs head: String
  {Pv.interrupted_text(who, n, why) == String.append(head, why) : String}

# LAW (interrupt-who): an interrupt names who asked for it. The hub writes the asker in
# the flag ("user", "main", a task's name, "bise"); "1" (an older hub),
# a blank flag or no file read as the user. The stopped call says so:
# "interrupted by main", and a gated call's result too.
law interrupt_names_who:
  {(Pv.interrupter("main"), Pv.interrupter("user"), Pv.interrupter("1"),
    Pv.interrupter(" bise\n"), Pv.interrupter_in(None{}), Pv.interrupter_in(Some{"main"}),
    Pv.interrupted_by(Pv.interrupter("main")), Pv.interrupted_by(Pv.interrupter("user")),
    Gt.found(None{}, Some{"main"}, "7", "a"), Gt.denial(Gt.VStop{"main"}),
    Pv.interrupted_text("main", 2n, "x"))
    == ("main", "the user", "the user", "bise", "the user", "main",
        "interrupted by main", "interrupted by the user",
        Some{Gt.VStop{"main"}}, Some{"interrupted by main"},
        "stopped retrying (interrupted by main) after 2 failed attempts; last error: x")
    : String & String & String & String & String & String & String & String
      & Maybe<&2, Gt.Verdict> & Maybe<&2, String> & String}

# LAW: retries exhausted: the final error ends with the last cause.
law exhausted_keeps_last_error:
  for +n: Nat
  for +why: String
  exs head: String
  {Pv.exhausted_text(n, why) == String.append(head, why) : String}

# LAW: a pause is slept in 100 ms slices, the flag read before each one;
# an interrupt empties what is left.
law retry_wait_slices:
  {Bool.and(Nat.is_eq(Pv.wait_slices(8000), 81n),
      Bool.and(U32.is_eq(Pv.slice_ms(False{}, 2500), 100),
        Bool.and(U32.is_eq(Pv.left_after(False{}, 2500), 2400),
          Bool.and(U32.is_eq(Pv.slice_ms(True{}, 2500), 0),
            U32.is_eq(Pv.left_after(True{}, 2500), 0)))))
    == True{} : Bool}

# LAW: every retry shows the user where the call stands.
law provider_retry_line:
  {Pv.retry_line(0n, "provider 529 (transient)", 1000)
    == "  obs: provider_retry: 1/10 · provider 529 (transient) · retry in 1s"
    : String}

# LAW: the final error says how many attempts ran and why the last failed.
law provider_exhausted_text:
  {Pv.exhausted_text(10n, "timeout")
    == "provider failed after 10 attempts: timeout" : String}

# ---- apply_patch: the parser and the strict matcher (core/patch.bend) ----
# Codex V4A. The parser is pinned byte-for-byte on concrete patches
# (the kind classifier makes every branch of the mode loop reachable
# and deterministic); the matcher is pinned on hits, misses, and the
# sequential cursor.

# LAW: an Add File section accumulates '+' lines; each line keeps a
# trailing newline (Codex pushes '\n' per line).
law patch_parse_add:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: a.txt\n+one\n+two\n*** End Patch")
    == Some{Con{Pa.PAdd{"a.txt", "one\ntwo\n"}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: an Add File section with zero lines creates an EMPTY file, not
# a lone newline.
law patch_parse_add_empty:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: e.txt\n*** End Patch")
    == Some{Con{Pa.PAdd{"e.txt", ""}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: a Delete File section is one action.
law patch_parse_delete:
  {Pa.parse_patch("*** Begin Patch\n*** Delete File: x\n*** End Patch")
    == Some{Con{Pa.PDelete{"x"}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: an Update File section carries its chunks with exact
# old/new line lists and lengths; the @@ hint line is dropped.
law patch_parse_update:
  {Pa.parse_patch("*** Begin Patch\n*** Update File: u.txt\n@@ fn foo\n ctx\n-del\n+add\n*** End Patch")
    == Some{Con{Pa.PUpdate{"u.txt",
      Con{Pa.PChunk{Con{"ctx", Con{"del", Nil{}}}, Con{"ctx", Con{"add", Nil{}}}, 2n, 2n},
        Nil{}}}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: content may open a chunk without @@ (Codex creates the chunk
# implicitly); a lone @@ chunk is skipped, never an action.
law patch_parse_implicit_chunk:
  {Pa.parse_patch("*** Begin Patch\n*** Update File: u\n@@\n@@\n a\n-b\n+c\n*** End Patch")
    == Some{Con{Pa.PUpdate{"u",
      Con{Pa.PChunk{Con{"a", Con{"b", Nil{}}}, Con{"a", Con{"c", Nil{}}}, 2n, 2n},
        Nil{}}}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: a multi-section patch parses to its actions in order (Add,
# Update, Delete here - the list carries them reversed by the fold,
# Delete last-added first-shown).
law patch_parse_multi:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: n\n+hi\n*** Update File: u\n@@\n a\n-b\n+B\n*** Delete File: d\n*** End Patch")
    == Some{Con{Pa.PAdd{"n", "hi\n"}, Con{Pa.PUpdate{"u", Con{Pa.PChunk{Con{"a", Con{"b", Nil{}}}, Con{"a", Con{"B", Nil{}}}, 2n, 2n}, Nil{}}}, Con{Pa.PDelete{"d"}, Nil{}}}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

law patch_rejects_missing_end:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: a\n+x") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: the first line must be '*** Begin Patch'.
law patch_rejects_bad_first:
  {Pa.parse_patch("hello\n*** End Patch") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: an Update File section with no chunk is malformed (Codex:
# "Update file hunk for path is empty").
law patch_rejects_empty_update:
  {Pa.parse_patch("*** Begin Patch\n*** Update File: u\n*** End Patch") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: garbage at the section level is rejected loudly.
law patch_rejects_garbage_section:
  {Pa.parse_patch("*** Begin Patch\nwhat is this\n*** End Patch") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: non-empty content after '*** End Patch' is rejected (Codex:
# the last line must be End Patch).
law patch_rejects_after_end:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: n\n+hi\n*** End Patch\njunk") == None{}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: the whole patch is trimmed first - a trailing newline after
# End Patch is fine.
law patch_tolerates_trailing_newline:
  {Pa.parse_patch("*** Begin Patch\n*** Add File: n\n+hi\n*** End Patch\n")
    == Some{Con{Pa.PAdd{"n", "hi\n"}, Nil{}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# LAW: the classifier is the parser's spine - every kind pinned.
law patch_kind_add_line:
  {Pa.p_kind_of("+x") == Pa.KAdd{} : Pa.PKind}

law patch_kind_del_line:
  {Pa.p_kind_of("-x") == Pa.KDel{} : Pa.PKind}

law patch_kind_ctx_line:
  {Pa.p_kind_of(" x") == Pa.KCtx{} : Pa.PKind}

law patch_kind_empty_line:
  {Pa.p_kind_of("") == Pa.KEmpty{} : Pa.PKind}

law patch_kind_begin:
  {Pa.p_kind_of("*** Begin Patch ") == Pa.KBegin{} : Pa.PKind}

law patch_kind_end:
  {Pa.p_kind_of("*** End Patch") == Pa.KEnd{} : Pa.PKind}

law patch_kind_add_header:
  {Pa.p_kind_of("*** Add File: x") == Pa.KAddHdr{} : Pa.PKind}

law patch_kind_delete_header:
  {Pa.p_kind_of("*** Delete File: x") == Pa.KDelHdr{} : Pa.PKind}

law patch_kind_update_header:
  {Pa.p_kind_of("*** Update File: x") == Pa.KUpdHdr{} : Pa.PKind}

law patch_kind_chunk_head:
  {Pa.p_kind_of("@@ fn foo") == Pa.KHead{} : Pa.PKind}

law patch_kind_garbage:
  {Pa.p_kind_of("what") == Pa.KGarbage{} : Pa.PKind}

# LAW: a chunk's lines replace exactly; the file's trailing newline
# survives (the split/join roundtrip is byte-exact).
law patch_apply_hit:
  {Pa.apply_update("a\nb\nc\n", Con{Pa.PChunk{Con{"b", Nil{}}, Con{"B", Nil{}}, 1n, 1n}, Nil{}})
    == Pa.POut{True{}, "a\nB\nc\n", 1n} : Pa.POut}

# LAW: a miss fails loudly with the chunk's index (0-based).
law patch_apply_miss:
  {Pa.apply_update("a\nb\nc\n", Con{Pa.PChunk{Con{"x", Nil{}}, Con{"X", Nil{}}, 1n, 1n}, Nil{}})
    == Pa.POut{False{}, "", 0n} : Pa.POut}

# LAW: chunks apply in order - the second matches AFTER the first's
# splice (the done/todo cursor is strict).
law patch_apply_sequential:
  {Pa.apply_update("f\na\nf\na\n",
    Con{Pa.PChunk{Con{"f", Nil{}}, Con{"F", Nil{}}, 1n, 1n},
     Con{Pa.PChunk{Con{"f", Nil{}}, Con{"G", Nil{}}, 1n, 1n}, Nil{}}})
    == Pa.POut{True{}, "F\na\nG\na\n", 2n} : Pa.POut}

# LAW: the cursor never rewinds - the second chunk may NOT match a
# line the first chunk already consumed.
law patch_apply_sequential_miss:
  {Pa.apply_update("a\nf\n",
    Con{Pa.PChunk{Con{"a", Nil{}}, Con{"A", Nil{}}, 1n, 1n},
     Con{Pa.PChunk{Con{"a", Nil{}}, Con{"B", Nil{}}, 1n, 1n}, Nil{}}})
    == Pa.POut{False{}, "", 1n} : Pa.POut}

# LAW: a chunk with no old lines is a pure insertion at the cursor.
law patch_apply_insert:
  {Pa.apply_update("a\n", Con{Pa.PChunk{Nil{}, Con{"X", Nil{}}, 0n, 1n}, Nil{}})
    == Pa.POut{True{}, "X\na\n", 1n} : Pa.POut}

# LAW: the failed chunk's first line is the error anchor.
law patch_anchor_names_the_chunk:
  {Pa.p_chunk_anchor(
    Con{Pa.PChunk{Con{"one", Nil{}}, Nil{}, 1n, 0n},
     Con{Pa.PChunk{Con{"two", Nil{}}, Nil{}, 1n, 0n}, Nil{}}}, 1n) == "two" : String}

# LAW: the validate loop reads ahead by action path - every action
# shape carries one.
law patch_path_of_actions:
  {Bool.and(String.eq(Pa.p_path_of(Pa.PAdd{"a", "x"}), "a"),
    Bool.and(String.eq(Pa.p_path_of(Pa.PDelete{"d"}), "d"),
     String.eq(Pa.p_path_of(Pa.PUpdate{"u", Nil{}}), "u"))) == True{} : Bool}

# LAW: the Begin marker matches by prefix - the live model appends
# a stray ' ***' to it (observed end-to-end with GLM).
law patch_kind_begin_prefix:
  {Pa.p_kind_of("*** Begin Patch ***") == Pa.KBegin{} : Pa.PKind}

# LAW: section headers parse without the leading '***' too - safe,
# because a content line inside a chunk always carries a ' ', '+' or
# '-' prefix, so a bare 'Add File:' line can only be a header.
law patch_kind_bare_add_header:
  {Pa.p_kind_of("Add File: x") == Pa.KAddHdr{} : Pa.PKind}

law patch_kind_bare_delete_header:
  {Pa.p_kind_of("Delete File: x") == Pa.KDelHdr{} : Pa.PKind}

law patch_kind_bare_update_header:
  {Pa.p_kind_of("Update File: x") == Pa.KUpdHdr{} : Pa.PKind}

# LAW: the exact variant GLM emitted live (trailing ' ***' on Begin,
# bare section headers) parses to the same actions the canonical
# form gives.
law patch_parse_glm_variant:
  {Pa.parse_patch("*** Begin Patch ***\nUpdate File: ap-e2e/a.txt\n@@ \n one\n-two\n+TWO\nAdd File: ap-e2e/sub/new.txt\n+hello\n+patched\nDelete File: ap-e2e/del.txt\n*** End Patch")
    == Some{Con{Pa.PUpdate{"ap-e2e/a.txt",
      Con{Pa.PChunk{Con{"one", Con{"two", Nil{}}}, Con{"one", Con{"TWO", Nil{}}}, 2n, 2n},
        Nil{}}},
      Con{Pa.PAdd{"ap-e2e/sub/new.txt", "hello\npatched\n"},
      Con{Pa.PDelete{"ap-e2e/del.txt"}, Nil{}}}}}
    : Maybe<&2, List<&2, Pa.PAction>>}

# ---- BR-001: an interrupt must not orphan the calls the turn holds ----

# LAW: interrupting a turn that waits on tool calls answers every
# waiting call with a failed "interrupted" result BEFORE the turn
# dies - the answer stays adjacent to its assistant call (the steering
# the turn held is committed after, by finish_turn).
law interrupt_closes_waiting_calls:
  {S.sess_history(S.res_next(S.apply(steer_flow.called(law_cfg()), T.Interrupt{}),
      steer_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("thinking", Con{T.Call{2, "bash", "ls"}, Nil{}}),
      Con{T.tool_res("tool bash failed: interrupted"),
      Con{T.Msg{T.RUser{}, "penguins", Nil{}, True{}}, Nil{}}}}}
    : List<&2, T.Msg>}

# LAW: an agent pending (nothing in history yet) answers nothing on an
# interrupt.
law interrupt_closes_agent_pending:
  {S.interrupt_closes(Some{T.PAgent{9}}) == Nil{}
    : List<&2, T.Msg>}

# ---- run_typescript: one engine, an ordinary tool (BISE-118) ----
# The Core runs no program: a run_typescript call goes out as a tool act
# like bash; the runtime runs it in V8 (bend-jsrt), live and scripted
# alike, and the program's one result comes back as the tool result.
# The program's own tool calls stay inside the runtime (record and
# re-run), the Core never sees them.

def ts_args() -> String:
  "{\"code\": \"return 1\"}"

def ts_flow.called(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.started(cfg),
      T.CompletionOk{0, "", Con{T.Call{1, "run_typescript", ts_args()},
        Nil{}}}),
    steer_flow.started(cfg))

# LAW: the call goes out: one tool act, name and args as the model wrote
# them (the completion renumbers Call{1} to 2, like any tool call).
law run_typescript_goes_out_as_a_tool:
  {drain_flow.acts(S.apply(steer_flow.started(law_cfg()),
      T.CompletionOk{0, "", Con{T.Call{1, "run_typescript", ts_args()},
        Nil{}}}))
    == Con{T.ToolAct{2, T.Call{2, "run_typescript", ts_args()}}, Nil{}}
    : List<&2, T.Act>}

# LAW: a program batches with other calls in one completion (the live
# behavior; the old scripted core refused "run_typescript must be the
# only call").
law run_typescript_batches_with_other_calls:
  {drain_flow.acts(S.apply(steer_flow.started(law_cfg()),
      T.CompletionOk{0, "", Con{T.Call{1, "run_typescript", ts_args()},
        Con{T.Call{2, "bash", "ls"}, Nil{}}}}))
    == Con{T.ToolAct{2, T.Call{2, "run_typescript", ts_args()}},
      Con{T.ToolAct{3, T.Call{3, "bash", "ls"}}, Nil{}}}
    : List<&2, T.Act>}

# LAW: the result comes back: the program's value is the run_typescript
# tool result, right after its call.
law run_typescript_result_comes_back:
  {S.sess_history(S.res_next(S.apply(ts_flow.called(law_cfg()),
      T.ToolOk{2, "1"}), ts_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("", Con{T.Call{2, "run_typescript", ts_args()}, Nil{}}),
      Con{T.tool_res("tool run_typescript ok: 1"), Nil{}}}}
    : List<&2, T.Msg>}

# LAW: a program that fails (a throw, a failed inner call, a bad
# program) comes back as a failed run_typescript result.
law run_typescript_failure_comes_back:
  {S.sess_history(S.res_next(S.apply(ts_flow.called(law_cfg()),
      T.ToolFail{2, "program error"}), ts_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("", Con{T.Call{2, "run_typescript", ts_args()}, Nil{}}),
      Con{T.tool_res("tool run_typescript failed: program error"), Nil{}}}}
    : List<&2, T.Msg>}

# LAW: an interrupt answers the running program like any waiting call.
law run_typescript_interrupted:
  {S.sess_history(S.res_next(S.apply(ts_flow.called(law_cfg()),
      T.Interrupt{}), ts_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("", Con{T.Call{2, "run_typescript", ts_args()}, Nil{}}),
      Con{T.tool_res("tool run_typescript failed: interrupted"), Nil{}}}}
    : List<&2, T.Msg>}

# LAW: the runtime runs run_typescript on the V8 engine (bend-jsrt).
# The dispatch has no mode: live and scripted sessions run the same code.
law run_typescript_runs_on_jsrt:
  {Rt.tool_of("run_typescript") == Rt.TNode{} : Rt.Tool}

# LAW: the old routing name node_program is no tool of its own any more
# (an unknown name goes to the MCP lookup, which fails it by name).
law node_program_is_not_a_tool:
  {Rt.tool_of("node_program") == Rt.TMcp{} : Rt.Tool}

# LAW: a session saved before BISE-118 holds live programs under the old
# routing name node_program: they load as run_typescript.
law checkpoint_loads_node_program_as_run_typescript:
  {K.fcall("  CALL 7 node_program : x")
    == T.Call{7, "run_typescript", "x"} : T.Call}

# LAW: every other call name loads untouched.
law checkpoint_keeps_call_names:
  {K.fcall("  CALL 7 bash : ls") == T.Call{7, "bash", "ls"} : T.Call}

# ---- BR-004: the deferred scan trusts only standalone tool acks ----

# LAW: assistant prose quoting the ack never fires the deferred
# request (the model echoes tool outputs routinely).
# BR-004 helper: an assistant message that quotes the compact ack
def prose_quoting_ack() -> T.Msg:
  T.asst(String.append("the tool said '",
    String.append(Sf.COMPACT_ACK(), "' - filing it")), Nil{})

law deferred_ignores_assistant_prose:
  {Sf.deferred_of(Con{T.user("hi"), Con{prose_quoting_ack(), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: a tool output QUOTING the harness source never fires - the
# quotes and indentation around the constant break the whole-text
# and whole-line shapes.
law deferred_ignores_source_quotes:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("  \"",
          String.append(Sf.COMPACT_ACK(), "\""))), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: a tool result that merely CONTAINS the marker mid-text never
# fires - the ack must end the text or stand on its own line.
law deferred_ignores_midtext_marker:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("prefix ",
          String.append(Sf.COMPACT_ACK(), " suffix"))), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: the real direct shape fires - the self.* tool result prefixes
# the ack with "tool X ok: ", so the ack ends the text.
law deferred_reads_prefixed_ack:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("tool self.compact ok: ",
          Sf.COMPACT_ACK())), Nil{}}})
    == Sf.Def{True{}, False{}} : Sf.Def}

# LAW: the real program shape fires - the acks ride their own lines of
# the escaped tool text (a literal backslash-n line boundary).
law deferred_reads_program_acked_result:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{S.tool_result_msg("run_typescript",
          String.append("prog out\n", Sf.COMPACT_ACK()), True{}),
        Nil{}}})
    == Sf.Def{True{}, False{}} : Sf.Def}

# ---- the provider adapters (core/api.bend + runtime/provider.bend) ----
# The vibe CLI structure: one mapping per api_style, a provider table
# in the runtime. The OpenAI mapping above stays exactly as the GLM
# laws pinned it; the Anthropic mapping below is the Messages shape
# the foundry proxy serves (probed live: thinking blocks are optional,
# tool_use without history thinking is accepted, a thinking block
# without its signature is rejected). A SIGNED history reasoning
# replays its thinking block verbatim with the signature (the
# reasoning continuity across tool calls); an unsigned one stays
# stripped.

# The provider registry (runtime/provider-pure.bend, BISE-141): a model
# id is "provider/model"; the provider gives the family, the base_url
# and the key env; the models file Rust writes (BISE_MODELS_FILE) sits
# over the entries built in; a bare name is an alias, else the legacy
# guess (claude* -> foundry, else mistral). Pv.resolve(BISE_MODEL,
# BEND_MODEL, BISE_ROLE, config.toml, models file, BEND_PROVIDER_URL).

# LAW: no model anywhere (no env var, no config, no models file): no
# call, the error says to pick a provider (BISE-266: no built-in
# default). The opus-5.5 alias still goes to the foundry proxy (Messages
# API, its key), a bare non-claude name to Mistral (Chat, its key).
law provider_no_default_model:
  {Pv.resolve("", "", "", "", "", "", "")
    == Pv.RErr{"", Pv.NO_MODEL()} : Pv.Resolved}

law provider_resolves_alias:
  {Pv.resolve("", "", "", "", "model = \"opus-5.5\"",
      "[providers.foundry]\nbase_url = \"https://foundry.example.net/anthropic/v1\"", "")
    == Pv.ROk{Pv.Prov{"foundry", "claude-opus-5-5", Api.Anthropic{},
      "https://foundry.example.net/anthropic/v1/messages",
      "ANTHROPIC_FOUNDRY_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}} : Pv.Resolved}

law provider_legacy_claude_is_foundry:
  {Pv.resolve("", "claude-opus-5-5", "", "", "",
      "[providers.foundry]\nbase_url = \"https://foundry.example.net/anthropic/v1\"", "")
    == Pv.ROk{Pv.Prov{"foundry", "claude-opus-5-5", Api.Anthropic{},
      "https://foundry.example.net/anthropic/v1/messages",
      "ANTHROPIC_FOUNDRY_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}} : Pv.Resolved}

# LAW: bise ships no URL for the foundry proxy (a private one): without
# a base_url in config.toml (so in the models file), no call, the error
# says where to set it.
law provider_foundry_needs_a_base_url:
  {Pv.resolve("", "", "", "", "model = \"opus-5.5\"", "", "")
    == Pv.RErr{"foundry/claude-opus-5-5", "provider \"foundry\" (model \"foundry/claude-opus-5-5\") has no base_url: add it under [providers.foundry] in ~/.bise/config.toml"} : Pv.Resolved}

# LAW: the models file names the variable that would give the URL
# (base_url_env, rust/catalog: ANTHROPIC_FOUNDRY_BASE_URL): the error
# says to set it.
law provider_foundry_names_its_url_variable:
  {Pv.resolve("", "", "", "", "model = \"opus-5.5\"",
      "[providers.foundry]
base_url_env = \"ANTHROPIC_FOUNDRY_BASE_URL\"", "")
    == Pv.RErr{"foundry/claude-opus-5-5", "provider \"foundry\" (model \"foundry/claude-opus-5-5\") has no base_url: set ANTHROPIC_FOUNDRY_BASE_URL, or add it under [providers.foundry] in ~/.bise/config.toml"} : Pv.Resolved}

# LAW: base_url = "" (an older models file, Ben's first run 2026-10-01)
# is no base_url: the same error, never a call to "/messages".
law provider_empty_base_url_is_none:
  {Pv.resolve("", "", "", "", "model = \"opus-5.5\"",
      "[providers.foundry]
base_url = \"\"", "")
    == Pv.RErr{"foundry/claude-opus-5-5", "provider \"foundry\" (model \"foundry/claude-opus-5-5\") has no base_url: add it under [providers.foundry] in ~/.bise/config.toml"} : Pv.Resolved}

law provider_legacy_glm_is_mistral:
  {Pv.resolve("", "zai-glm-5-3", "", "", "", "", "")
    == Pv.ROk{Pv.Prov{"mistral", "zai-glm-5-3", Api.OpenAI{},
      "https://api.mistral.ai/v1/chat/completions", "MISTRAL_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}} : Pv.Resolved}

# LAW: a model the registry does not list works on its provider's
# defaults.
law provider_unlisted_model_works:
  {Pv.resolve("", "", "", "", "model = \"mistral/brand-new-7b\"", "", "")
    == Pv.ROk{Pv.Prov{"mistral", "brand-new-7b", Api.OpenAI{},
      "https://api.mistral.ai/v1/chat/completions", "MISTRAL_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}} : Pv.Resolved}

# LAW: the choice order (as rust/catalog's Setup): main = BISE_MODEL >
# BEND_MODEL > config model; a sub-agent (BISE_ROLE=agent) =
# BISE_AGENT_MODEL > config agent_model > the main order.
law provider_bise_model_first:
  {Pv.shown_model(Pv.resolve("mistral/a", "mistral/b", "mistral/e", "main",
      "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/a" : String}

law provider_bend_model_over_config:
  {Pv.shown_model(Pv.resolve("", "mistral/b", "mistral/e", "main",
      "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/b" : String}

law provider_agent_env_first:
  {Pv.shown_model(Pv.resolve("mistral/a", "mistral/b", "mistral/e", "agent",
      "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/e" : String}

law provider_agent_model_for_agents:
  {Pv.shown_model(Pv.resolve("mistral/a", "mistral/b", "", "agent",
      "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/d" : String}

law provider_agent_follows_main:
  {Pv.shown_model(Pv.resolve("", "mistral/b", "", "agent", "model = \"mistral/c\"", "", ""))
    == "mistral/b" : String}

law provider_agent_model_not_for_main:
  {Pv.shown_model(Pv.resolve("", "", "mistral/e", "main", "model = \"mistral/c\"\nagent_model = \"mistral/d\"", "", ""))
    == "mistral/c" : String}

# LAW (BISE-298): the model roles table. [roles] main / agents (a line,
# or [roles.<r>] model = ...) come before the old model / agent_model;
# the efforts too ([roles.<r>] effort before (agent_)reasoning_effort).
law provider_roles_main_over_model:
  {Pv.shown_model(Pv.resolve("", "", "", "main",
      "model = \"mistral/c\"
[roles]
main = \"mistral/r\"", "", ""))
    == "mistral/r" : String}

law provider_roles_agents_table:
  {(Pv.shown_model(Pv.resolve("", "", "", "agent",
      "agent_model = \"mistral/d\"
[roles.agents]
model = \"mistral/t\"", "", "")),
    Pv.shown_model(Pv.resolve("", "", "", "main",
      "[roles]
main = \"mistral/r\"
agents = \"mistral/s\"", "", "")),
    Pv.shown_model(Pv.resolve("", "", "", "agent",
      "[roles]
main = \"mistral/r\"", "", "")))
    == ("mistral/t", "mistral/r", "mistral/r") : String & String & String}

law effort_of_the_roles:
  {(Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "agent",
        "model = \"opus-5.5\"
agent_reasoning_effort = \"low\"
[roles.agents]
effort = \"medium\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "main",
        "model = \"opus-5.5\"
reasoning_effort = \"high\"
[roles.main]
effort = \"low\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"", "")))
    == ("medium", "low") : String & String}

# LAW (BISE-150): the compaction threshold defaults to 80 % of the
# model's context window: its `context` in the models file, else its
# provider's; nothing says (an unknown provider, no models file for a
# provider the built-in table lacks): 128000, as rust/catalog's
# DEFAULT_CAPS. BEND_THRESHOLD > config compaction_threshold > that
# default (BISE-300: never above it either).
law threshold_is_80_percent_of_the_window:
  {(Pv.threshold_of(1000000), Pv.threshold_of(200000), Pv.threshold_of(128000),
      Pv.threshold_of(131072), Pv.threshold_of(0))
    == (800000, 160000, 102400, 104857, 0) : U32 & U32 & U32 & U32 & U32}

law window_builtin_setups:
  {(Pv.window("", "", "", "", "model = \"opus-5.5\"", ""),
      Pv.window("", "", "", "", "model = \"zai-glm-5-3\"", ""))
    == (1000000, 128000) : U32 & U32}

law window_model_over_provider:
  {(Pv.window("", "", "", "", "model = \"work/big\"",
        "[providers.work]\nbase_url = \"http://x\"\ncontext = 32768\n\n[models.\"work/big\"]\ncontext = 262144\n"),
      Pv.window("", "", "", "", "model = \"work/small\"",
        "[providers.work]\nbase_url = \"http://x\"\ncontext = 32768\n"))
    == (262144, 32768) : U32 & U32}

law window_of_the_agent_model:
  {Pv.window("", "", "", "agent", "model = \"a/x\"\nagent_model = \"b/y\"",
      "[providers.a]\ncontext = 1000\n\n[providers.b]\ncontext = 2000\n")
    == 2000 : U32}

law window_unknown_model_default:
  {(Pv.window("nowhere/m", "", "", "", "", ""),
      Pv.window("", "", "", "", "model = \"work/m\"",
        "[providers.work]\ncontext = lots\n"))
    == (128000, 128000) : U32 & U32}

law threshold_precedence:
  {(Pv.threshold("5000", "compaction_threshold = 7000", 1000000),
      Pv.threshold("", "compaction_threshold = 7000", 1000000),
      Pv.threshold("", "model = \"x\"", 200000),
      Pv.threshold("oops", "compaction_threshold = 7000", 200000))
    == (5000, 7000, 160000, 160000) : U32 & U32 & U32 & U32}

# LAW: the Core gets the threshold in its own units, three quarters of
# the provider's tokens (Pv.est_units). Main's 450000 becomes 337500:
# its history crossed 450607 real tokens at an estimate of 343760 and
# peaked at 507700 real (estimate 396485) without compacting; both
# estimates are past 337500, so it would have compacted near 450k (an
# estimate just under 337500 still goes out).
law threshold_in_core_units:
  {(Pv.est_units(450000), Pv.est_units(160000), Pv.est_units(7),
      U32.is_lt(343760, Pv.est_units(450000)),
      U32.is_lt(337499, Pv.est_units(450000)))
    == (337500, 120000, 5, False{}, True{})
    : U32 & U32 & U32 & Bool & Bool}

# LAW (BISE-300): config `compaction_threshold` is a number of tokens or
# a share of the model's window ("45%", spaces allowed around it);
# BEND_THRESHOLD reads the same way.
law threshold_tokens_or_percent:
  {(Pv.threshold("", "compaction_threshold = 450000", 1000000),
      Pv.threshold("", "compaction_threshold = \"45%\"", 1000000),
      Pv.threshold("", "compaction_threshold = \"45%\"", 200000),
      Pv.threshold("", "compaction_threshold = \" 45 % \"", 128000),
      Pv.threshold("10%", "compaction_threshold = 7000", 200000))
    == (450000, 450000, 90000, 57600, 20000)
    : U32 & U32 & U32 & U32 & U32}

# LAW (BISE-300): the threshold never goes above 80 % of the model's
# window, whoever sets it: a number written for a 1M model on a 200k
# one, a share above 80 %, the environment.
law threshold_capped_at_80_percent:
  {(Pv.threshold("", "compaction_threshold = 450000", 200000),
      Pv.threshold("", "compaction_threshold = \"95%\"", 1000000),
      Pv.threshold("", "compaction_threshold = \"250%\"", 200000),
      Pv.threshold("900000", "", 200000),
      Pv.threshold("", "compaction_threshold = 160000", 200000))
    == (160000, 800000, 160000, 160000, 160000)
    : U32 & U32 & U32 & U32 & U32}

# LAW (BISE-300): a value that is no threshold (0, 0 %, a word, a
# decimal share, a lone %) leaves the default; the old key `threshold`
# is not read any more.
law threshold_bad_values_default:
  {(Pv.threshold("", "compaction_threshold = 0", 200000),
      Pv.threshold("", "compaction_threshold = \"0%\"", 200000),
      Pv.threshold("", "compaction_threshold = \"lots\"", 200000),
      Pv.threshold("", "compaction_threshold = \"4.5%\"", 200000),
      Pv.threshold("", "compaction_threshold = \"%\"", 200000),
      Pv.threshold("", "threshold = 7000", 200000))
    == (160000, 160000, 160000, 160000, 160000, 160000)
    : U32 & U32 & U32 & U32 & U32 & U32}

# LAW (BISE-150): the config template leaves the threshold to the model
law config_template_threshold_unset:
  {Cf.cfg_get(St.CONFIG_TEMPLATE(), "compaction_threshold") == None{}
    : Maybe<&2, String>}

# LAW: a provider of the models file: its family, base_url (a trailing
# slash dropped) + the family's endpoint, its key env; the model id
# keeps every '/' after the first.
law provider_from_models_file:
  {Pv.resolve("", "", "", "", "model = \"work/qwen/q3-coder\"",
      "[providers.work]\napi = \"openai-chat\"\nbase_url = \"https://llm.corp.example/v1/\"\nkey_env = \"CORP_KEY\"\nneeds = \"\"",
      "")
    == Pv.ROk{Pv.Prov{"work", "qwen/q3-coder", Api.OpenAI{},
      "https://llm.corp.example/v1/chat/completions", "CORP_KEY", Api.MFacts{False{}, 0n, "max_tokens", "", "", "", False{}}}}
    : Pv.Resolved}

# LAW: a model's own key wins over its provider's (a model on another
# family), and key_env = "" (a local server) needs no key.
law provider_model_key_wins:
  {Pv.resolve("", "", "", "", "model = \"local/m\"",
      "[providers.local]\napi = \"openai-chat\"\nbase_url = \"http://localhost:1234/v1\"\nkey_env = \"\"\n[models.\"local/m\"]\napi = \"anthropic\"",
      "")
    == Pv.ROk{Pv.Prov{"local", "m", Api.Anthropic{},
      "http://localhost:1234/v1/messages", "", Api.MFacts{False{}, 0n, "max_tokens", "", "", "", False{}}}} : Pv.Resolved}

# LAW: the models file wins over the built-in entries, key by key; its
# aliases and default_model are used.
law provider_file_over_builtin:
  {Pv.resolve("", "", "", "", "",
      "default_model = \"big\"\n[aliases]\n\"big\" = \"foundry/claude-x\"\n[providers.foundry]\nbase_url = \"https://api.anthropic.com/v1\"",
      "")
    == Pv.ROk{Pv.Prov{"foundry", "claude-x", Api.Anthropic{},
      "https://api.anthropic.com/v1/messages",
      "ANTHROPIC_FOUNDRY_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}} : Pv.Resolved}

# LAW: BEND_PROVIDER_URL (the tests' fake provider) wins over the URL.
law provider_url_env_wins:
  {Pv.resolve("", "mistral-small-latest", "", "", "", "",
      "http://127.0.0.1:9/v1/chat/completions")
    == Pv.ROk{Pv.Prov{"mistral", "mistral-small-latest", Api.OpenAI{},
      "http://127.0.0.1:9/v1/chat/completions", "MISTRAL_API_KEY", Api.MFacts{True{}, 0n, "max_tokens", "", "", "high", False{}}}}
    : Pv.Resolved}

# LAW: a long file is read to its end (a pair line takes 3 steps of the
# walk: the fuel counts them; the models file has ~350 lines).
law config_reads_long_file:
  {Cf.cfg_get("[providers.p0]\nbase_url = \"u0\"\n[providers.p1]\nbase_url = \"u1\"\n[providers.p2]\nbase_url = \"u2\"\n[providers.p3]\nbase_url = \"u3\"\n[providers.p4]\nbase_url = \"u4\"\n[providers.p5]\nbase_url = \"u5\"\n[providers.p6]\nbase_url = \"u6\"\n[providers.p7]\nbase_url = \"u7\"\n[providers.p8]\nbase_url = \"u8\"\n[providers.p9]\nbase_url = \"u9\"\n[providers.p10]\nbase_url = \"u10\"\n[providers.p11]\nbase_url = \"u11\"", "providers.p11.base_url")
    == Some{"u11"} : Maybe<&2, String>}

# LAW: what does not resolve is an error at call time, with its reason.
law provider_unknown_is_error:
  {Pv.resolve("nope/m", "", "", "", "", "", "")
    == Pv.RErr{"nope/m", "unknown provider \"nope\" (model \"nope/m\"): add a [providers.nope] table (base_url, key_env, api) to config.toml"}
    : Pv.Resolved}

law provider_needs_is_error:
  {Pv.resolve("az/m", "", "", "", "",
      "[providers.az]\napi = \"openai-chat\"\nbase_url = \"https://x\"\nneeds = \"BISE-149\"", "")
    == Pv.RErr{"az/m", "model \"az/m\": provider \"az\" is not supported yet (needs BISE-149)"}
    : Pv.Resolved}

law provider_unknown_family_is_error:
  {Pv.resolve("g/m", "", "", "", "",
      "[providers.g]\napi = \"gemini\"\nbase_url = \"https://x\"", "")
    == Pv.RErr{"g/m", "model \"g/m\": api \"gemini\" of provider \"g\" is not supported yet"}
    : Pv.Resolved}

# LAW: a family's api name reads back as that family.
law family_name_roundtrip:
  for st: Api.Style
  {Api.style_of_api(Api.api_name(st)) == Some{st} : Maybe<&2, Api.Style>}

# LAW: the headers of each family, in order (the bytes of today's
# calls); no key, no auth header.
law family_headers_anthropic:
  {Api.headers(Api.Anthropic{}, "", "k")
    == [Api.Hdr{"x-api-key", "k"},
      Api.Hdr{"anthropic-version", "2023-06-01"},
      Api.Hdr{"anthropic-beta",
        "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14,prompt-caching-2024-07-31,context-1m-2025-08-07"},
      Api.Hdr{"Content-Type", "application/json"}] : List<&2, Api.Hdr>}

law family_headers_openai:
  {Api.headers(Api.OpenAI{}, "", "k")
    == [Api.Hdr{"Authorization", "Bearer k"},
      Api.Hdr{"Content-Type", "application/json"}] : List<&2, Api.Hdr>}

law family_headers_no_key:
  {Api.headers(Api.OpenAI{}, "", "")
    == [Api.Hdr{"Content-Type", "application/json"}] : List<&2, Api.Hdr>}

# LAW: every family streams: the Messages API and OpenAI Chat (BISE-144).
law family_anthropic_streams:
  {Api.streams(Api.Anthropic{}) == True{} : Bool}

law family_openai_streams:
  {Api.streams(Api.OpenAI{}) == True{} : Bool}

# LAW: the style dispatch routes the body and the reply to the right
# adapter - the OpenAI side byte-identical to api_body/reply_of.
law style_routes_body:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : hi\nEND\n"))
    == Oai.api_body("m",
      W.parse_wire("MODEL default\nMSG user : hi\nEND\n"))
    : String}

law style_routes_reply:
  {Api.reply_of_for(Api.OpenAI{}, 401, "{\"error\":\"bad key\"}")
    == Oai.reply_of(401, "{\"error\":\"bad key\"}") : String}

# LAW: an Anthropic body maps a simple wire byte-for-byte: the system
# prompt becomes the top-level system field, the user message a text
# block, the thinking config the vibe CLI sends for opus-5.5 (adaptive,
# summarized, effort high, the adaptive max_tokens budget).
law api_body_anth_maps:
  {Api.api_body_for(Api.Anthropic{}, "claude-opus-5-5", Api.facts0(),
      W.parse_wire("MODEL default\nMSG system : You are precise.\nMSG user : bonjour\nEND\n"))
    == "{\"system\":[{\"type\":\"text\",\"text\":\"You are precise.\",\"cache_control\":{\"type\":\"ephemeral\"}}],\"model\":\"claude-opus-5-5\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"bonjour\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: an Anthropic compaction call (no TOOL lines) omits tools and
# tool_choice - the model cannot even offer a tool call.
law api_body_anth_bare_omits_tools:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : salut\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"salut\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: the Anthropic tools carry the {name, description, input_schema}
# shape and tool_choice is the auto object.
law api_body_anth_tools_shape:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nTOOL echo : Returns text.\nMSG user : bonjour\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"bonjour\",\"cache_control\":{\"type\":\"ephemeral\"}}]}],\"tools\":[{\"name\":\"echo\",\"description\":\"Returns text.\",\"input_schema\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]},\"cache_control\":{\"type\":\"ephemeral\"}}],\"tool_choice\":{\"type\":\"auto\"}}"
    : String}

# LAW: an assistant call pairs with its tool result exactly as the
# OpenAI adapter pairs them - the tool_use id and the tool_use_id of
# the result agree, and the result rides a USER message.
law anth_tool_result_pairs:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : thinking\n  CALL 7 echo : hi\nMSG tool : tool echo ok: out\nEND\n"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"thinking\"},{\"type\":\"tool_use\",\"id\":\"call_7\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"},\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"tool echo ok: out\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: consecutive tool results merge into ONE user message - the
# vibe mapper's shape (the ids pair FIFO).
law anth_tool_results_merge:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : thinking\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_8\",\"content\":\"out2\"}")
    == False{} : Bool}

# LAW: parallel calls pair with their results in order - the first
# CALL line takes the first tool result (the calls of one message
# were once reversed, so each result went to the other call).
law oai_parallel_results_pair_in_order:
  {String.contains(Oai.api_body("m",
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "{\"role\":\"tool\",\"content\":\"out1\",\"tool_call_id\":\"call_7\"},{\"role\":\"tool\",\"content\":\"out2\",\"tool_call_id\":\"call_8\"}")
    == True{} : Bool}

# LAW: the same pairing on the Anthropic adapter.
law anth_parallel_results_pair_in_order:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 7 echo : hi\n  CALL 8 bash : ls\nMSG tool : out1\nMSG tool : out2\nEND\n")),
    "{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"out1\"},{\"type\":\"tool_result\",\"tool_use_id\":\"call_8\",\"content\":\"out2\"")
    == True{} : Bool}

# LAW: the marked history reasoning is STRIPPED on the way back - a
# thinking block without its signature is rejected by the Messages
# API (probed), so no \"type\":\"thinking\" block ever rides an
# assistant message.
law anth_unsigned_thinking_stays_stripped:
  {Bool.not(String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
       W.parse_wire("MODEL default
MSG assistant : <think>2+2 = 4</think>\\n4
END
")),
       "type\":\"thinking\""))
    == True{} : Bool}

# LAW: a SIGNED marked reasoning replays its thinking block verbatim
# with the signature at block level - the reasoning continuity the
# Messages API needs across tool calls; the model no longer re-thinks
# after every tool result.
law anth_signed_thinking_replays:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
       W.parse_wire("MODEL default
MSG assistant : <think>2+2 = 4\\nBENDSIG::sig</think>\\n4
END
")),
       "{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"}")
    == True{} : Bool}

# LAW: EVERY signed span of one message replays as its own thinking
# block - a reply with two thinking blocks leaves no marker in the
# text (a span riding as plain text is refused as reasoning
# extraction, probed).
law anth_multi_span_replays:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
       W.parse_wire("MODEL default
MSG assistant : <think>a\\nBENDSIG::s1</think>\\n<think>b\\nBENDSIG::s2</think>\\nok
END
"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"a\",\"signature\":\"s1\"},{\"type\":\"thinking\",\"thinking\":\"b\",\"signature\":\"s2\"},{\"type\":\"text\",\"text\":\"ok\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: a long tool run keeps every message - 20 tool results still
# reach the body (the walk fuel covers the three hops of a tool message).
law anth_long_tool_run_keeps_messages:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nMSG user : go\nMSG assistant : x\n  CALL 1 echo : hi\nMSG tool : out1\nMSG assistant : x\n  CALL 2 echo : hi\nMSG tool : out2\nMSG assistant : x\n  CALL 3 echo : hi\nMSG tool : out3\nMSG assistant : x\n  CALL 4 echo : hi\nMSG tool : out4\nMSG assistant : x\n  CALL 5 echo : hi\nMSG tool : out5\nMSG assistant : x\n  CALL 6 echo : hi\nMSG tool : out6\nMSG assistant : x\n  CALL 7 echo : hi\nMSG tool : out7\nMSG assistant : x\n  CALL 8 echo : hi\nMSG tool : out8\nMSG assistant : x\n  CALL 9 echo : hi\nMSG tool : out9\nMSG assistant : x\n  CALL 10 echo : hi\nMSG tool : out10\nMSG assistant : x\n  CALL 11 echo : hi\nMSG tool : out11\nMSG assistant : x\n  CALL 12 echo : hi\nMSG tool : out12\nMSG assistant : x\n  CALL 13 echo : hi\nMSG tool : out13\nMSG assistant : x\n  CALL 14 echo : hi\nMSG tool : out14\nMSG assistant : x\n  CALL 15 echo : hi\nMSG tool : out15\nMSG assistant : x\n  CALL 16 echo : hi\nMSG tool : out16\nMSG assistant : x\n  CALL 17 echo : hi\nMSG tool : out17\nMSG assistant : x\n  CALL 18 echo : hi\nMSG tool : out18\nMSG assistant : x\n  CALL 19 echo : hi\nMSG tool : out19\nMSG assistant : x\n  CALL 20 echo : hi\nMSG tool : out20\nEND\n")),
    "\"content\":\"out20\"")
    == True{} : Bool}

# LAW: a refusal (empty content, stop_reason "refusal") surfaces as a
# provider error - never an empty reply retried to the null limit.
law anth_refusal_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[],\"stop_reason\":\"refusal\",\"stop_details\":{\"explanation\":\"blocked\"}}")
    == "ERROR provider refusal: blocked\nEND\n" : String}

# comment header preserved below
# LAW: Anthropic text blocks become separate lines (the GLM parity).
law anth_blocks_join_lines:
  {W.unescape_nl(Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"text\",\"text\":\"a\"},{\"type\":\"text\",\"text\":\"b\"}]}"))
    == "OK a\nb\nEND\n" : String}

# LAW: Anthropic thinking blocks surface in the reply wrapped in the
# think markers - the same transport the GLM adapter uses, so the TUI
# and the history are unchanged.
law anth_thinking_surfaces:
  {W.unescape_nl(Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}]}"))
    == "OK <think>2+2 = 4
BENDSIG::sig</think>
4
END
" : String}

# comment header preserved below
# LAW: tool_use blocks map onto the CALL reply lines the Core parses
# (1-based ids, the flat arg rehydrated from the input object).
law anth_tool_calls_map:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"}}]}")
    == "OK \nCALL 1 echo : hi\nEND\n" : String}

# LAW: a structured input rides as its JSON on the CALL line.
law anth_structured_call_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"message\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"run_typescript\",\"input\":{\"code\":\"x()\"}}]}")
    == "OK \nCALL 1 run_typescript : {\"code\":\"x()\"}\nEND\n" : String}

# LAW: a 200 that carries an error body surfaces as an error.
law anth_error_body_maps:
  {Api.reply_of_for(Api.Anthropic{}, 200,
      "{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# LAW: a non-200 still maps to the flattened ERROR line with the status.
law anth_status_error_maps:
  {Api.reply_of_for(Api.Anthropic{}, 401, "{\"error\":\"bad key\"}")
    == "ERROR provider 401: bad key\nEND\n" : String}

# ---- tool names on the wire (BISE-293) ----
# Every family bise speaks (openai-chat: OpenAI, Mistral, OpenRouter...;
# anthropic) takes tool names matching ^[a-zA-Z0-9_-]{1,64}$ (OpenAI
# refused "self.compact" with a 400 on the user's first message). A
# name rides through W.wire_name on the way out (tools[] and the
# history's calls) and comes back through the request's own tools
# (W.name_in); the names bise sends today are already valid, so they
# ride unchanged.

# LAW: the live catalog's names are their own wire names.
law wire_name_keeps_the_catalog:
  {Bool.and(String.eq(W.wire_name("search_tool_functions"), "search_tool_functions"),
    Bool.and(String.eq(W.wire_name("bash"), "bash"),
     Bool.and(String.eq(W.wire_name("run_typescript"), "run_typescript"),
      Bool.and(String.eq(W.wire_name("skill"), "skill"),
       String.eq(W.wire_name("apply_patch"), "apply_patch"))))) == True{} : Bool}

# LAW: any other name is made valid: a dot (an MCP connector's
# "group.fn", the old self.* tools), a slash, a space, a non-ASCII char
# become "_", a long name is cut to 64 chars, an empty one is "_".
law wire_name_sanitizes:
  {Bool.and(String.eq(W.wire_name("self.compact"), "self_compact"),
    Bool.and(String.eq(W.wire_name("github.search_issues"), "github_search_issues"),
     Bool.and(String.eq(W.wire_name("my tool/v2"), "my_tool_v2"),
      Bool.and(String.eq(W.wire_name("café"), "caf_"),
       Bool.and(String.eq(W.wire_name(""), "_"),
        String.eq(W.wire_name("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
          "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")))))) == True{} : Bool}

# LAW: what comes out matches the pattern, and the pattern check
# refuses what the providers refuse.
law wire_name_matches_the_pattern:
  {Bool.and(W.name_ok(W.wire_name("self.reload")),
    Bool.and(W.name_ok(W.wire_name("linear.list-issues")),
     Bool.and(W.name_ok(W.wire_name("")),
      Bool.and(W.name_ok(W.wire_name("mcp.server.tool with spaces and ünicode.and.a.name.longer.than.sixty.four.chars")),
       Bool.and(Bool.not(W.name_ok("self.compact")),
        Bool.and(Bool.not(W.name_ok("")),
         Bool.not(W.name_ok("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"))))))))
    == True{} : Bool}

# LAW: the round trip: a call coming back under its wire name is named
# as the harness knows it; a valid name stays; a name no tool of the
# request has passes unchanged (the Core answers it "unknown tool").
law wire_names_round_trip:
  {W.reply_names_in(
      [W.WTool{"bash", ""}, W.WTool{"github.search_issues", ""}],
      "OK hi\nCALL 1 github_search_issues : {\"q\":\"a b\"}\nCALL 2 bash : ls\nCALL 3 self_compact : \nEND\n")
    == "OK hi\nCALL 1 github.search_issues : {\"q\":\"a b\"}\nCALL 2 bash : ls\nCALL 3 self_compact : \nEND\n"
    : String}

# LAW: both families send the wire names, in tools[] and in the
# history's calls: an old session's top-level self.compact call
# replays as self_compact.
law openai_body_sends_wire_names:
  {Bool.and(String.contains(Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "\"name\":\"github_search_issues\""),
    Bool.and(String.contains(Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "\"name\":\"self_compact\""),
     Bool.not(String.contains(Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "self.compact"))))
    == True{} : Bool}

law anthropic_body_sends_wire_names:
  {Bool.and(String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "\"name\":\"github_search_issues\""),
     String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
        W.parse_wire("MODEL default\nTOOL github.search_issues : Search.\nMSG user : hi\nMSG assistant : x\n  CALL 1 self.compact : \nMSG tool : ok\nEND\n")),
      "\"name\":\"self_compact\""))
    == True{} : Bool}

# ---- the OpenAI Responses family (BISE-147) ----
# OpenAI's gpt-6 models take tools with reasoning on /v1/responses only.
# Stateless like OpenCode and Codex: store false, the encrypted
# reasoning back (include), the whole conversation replayed with this
# family's own reasoning items; reasoning {effort, summary auto}.

law resp_family_name_and_endpoint:
  {Bool.and(String.eq(Api.api_name(Api.OpenAIResp{}), "openai-responses"),
    Bool.and(String.eq(Api.endpoint(Api.OpenAIResp{}), "/responses"),
      Api.streams(Api.OpenAIResp{}))) == True{} : Bool}

law resp_style_of_api:
  {Api.style_of_api("openai-responses") == Some{Api.OpenAIResp{}}
    : Maybe<&2, Api.Style>}

# LAW: the body: stateless, reasoning at the call's effort with its
# summary, the encrypted reasoning asked back; the system prompt is a
# developer message; the history's own reasoning replays (ENC1) as a
# reasoning item before its text and call, the call and its output
# pair on call_1; Anthropic's signed span is dropped (not replayed as
# text either); the tools are functions, strict off.
law resp_body_maps:
  {Api.api_body_for(Api.OpenAIResp{}, "gpt-6.1-sol", Api.MFacts{True{}, 0n, "max_tokens", "", "", "medium", False{}},
      W.parse_wire("MODEL default\nTOOL bash : Run a command.\nMSG system : Be brief.\nMSG user : hi\nMSG assistant : <think>plan\\nBENDSIG::oair:ENC1</think>\\nok\n  CALL 1 bash : ls\nMSG tool : a.txt\nMSG assistant : <think>old\\nBENDSIG::anthsig</think>\\nbye\nMSG user : again\nEND\n"))
    == "{\"model\":\"gpt-6.1-sol\",\"store\":false,\"reasoning\":{\"effort\":\"medium\",\"summary\":\"auto\"},\"include\":[\"reasoning.encrypted_content\"],\"input\":[{\"role\":\"developer\",\"content\":\"Be brief.\"},{\"role\":\"user\",\"content\":\"hi\"},{\"type\":\"reasoning\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"plan\"}],\"encrypted_content\":\"ENC1\"},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"ok\"}]},{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\"},{\"type\":\"function_call_output\",\"call_id\":\"call_1\",\"output\":\"a.txt\"},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"bye\"}]},{\"role\":\"user\",\"content\":\"again\"}],\"tools\":[{\"type\":\"function\",\"name\":\"bash\",\"description\":\"Run a command.\",\"parameters\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"},\"description\":{\"type\":\"string\",\"description\":\"One short line shown to the user while this runs: what you are doing, in the user's language. 8 words at most, starting lowercase, no final period. E.g. 'weighing the hero image', 'je lance les tests du signup'.\"}},\"required\":[\"arg\"]},\"strict\":false}],\"tool_choice\":\"auto\"}"
    : String}

# LAW: a model the catalog does not mark as reasoning sends no
# reasoning, no include; an output cap rides as max_output_tokens.
law resp_body_plain_model:
  {Api.api_body_for(Api.OpenAIResp{}, "m", Api.MFacts{False{}, 64n, "max_tokens", "", "", "", False{}},
      W.parse_wire("MODEL default\nMSG user : hi\nEND\n"))
    == "{\"model\":\"m\",\"store\":false,\"max_output_tokens\":64,\"input\":[{\"role\":\"user\",\"content\":\"hi\"}]}"
    : String}

# LAW: the Anthropic family never replays an OpenAI reasoning span
# (its signature is not Anthropic's).
law anthropic_drops_resp_reasoning:
  {String.contains(Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(),
      W.parse_wire("MODEL default\nTOOL bash : Run a command.\nMSG system : Be brief.\nMSG user : hi\nMSG assistant : <think>plan\\nBENDSIG::oair:ENC1</think>\\nok\n  CALL 1 bash : ls\nMSG tool : a.txt\nMSG assistant : <think>old\\nBENDSIG::anthsig</think>\\nbye\nMSG user : again\nEND\n")), "oair:")
    == False{} : Bool}

# LAW: a whole response: the reasoning item is a think span signed
# "oair:<encrypted_content>", the message text follows, a
# function_call is a CALL line (its args flattened as for Chat).
law resp_reply_maps:
  {Rs.reply_of(200, "{\"id\":\"resp_1\",\"status\":\"completed\",\"error\":null,\"output\":[{\"type\":\"reasoning\",\"id\":\"rs_1\",\"encrypted_content\":\"ENC2\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"think\"}]},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"hello\"}]},{\"type\":\"function_call\",\"call_id\":\"call_x\",\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"}]}")
    == "OK <think>think\\nBENDSIG::oair:ENC2</think>\\nhello\nCALL 1 bash : pwd\nEND\n" : String}

# LAW: a stream is read from its terminal event (the whole response
# rides in response.completed); the deltas before it are not needed.
law resp_stream_reads_completed:
  {Rs.reply_any("event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"hel\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"sequence_number\":9,\"response\":{\"error\":null,\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"hello\"}]}],\"usage\":{\"input_tokens\":12,\"input_tokens_details\":{\"cached_tokens\":8},\"output_tokens\":3}}}\n\n")
    == "OK hello\nEND\n" : String}

# LAW: an error event, or a stream cut before its end, is an error
# reply (the retry policy retries a 200 error).
law resp_stream_error:
  {Bool.and(String.eq(Rs.reply_any("event: error\ndata: {\"type\":\"error\",\"code\":\"server_error\",\"message\":\"boom\"}\n\n"),
      "ERROR provider 200: boom\nEND\n"),
    String.starts_with(Rs.reply_any("event: response.created\ndata: {\"type\":\"response.created\"}\n\n"),
      "ERROR provider 200: the stream ended")) == True{} : Bool}

# LAW: a refused request keeps OpenAI's own words (BISE-293).
law resp_status_error:
  {Rs.reply_of(400, "{\"error\":{\"message\":\"Invalid value.\",\"type\":\"invalid_request_error\"}}")
    == "ERROR provider 400: Invalid value.\nEND\n" : String}

# LAW: the usage: input_tokens counts the cached part already.
law resp_usage:
  {Api.usage_of(Api.OpenAIResp{},
      Rs.usage("data: {\"type\":\"response.completed\",\"response\":{\"output\":[],\"usage\":{\"input_tokens\":12,\"input_tokens_details\":{\"cached_tokens\":8},\"output_tokens\":3}}}\n"))
    == Api.Usage{12n, 3n, 8n, 0n} : Api.Usage}

# ---- the ChatGPT plan route (shape = "chatgpt-plan", docs/subscriptions-design.md) ----
# OpenAI's plan route (developers.openai.com/siwc, preview limitations)
# takes store false and stream true, refuses an output cap and the
# other fields below, a role "system" item, and function tools outside
# a namespace. The fixture: a reasoning model WITH an output cap in the
# catalog (the plan body must drop it), a system prompt, a tool, a call
# and its output in the history.

def plan.req() -> W.WReq:
  W.parse_wire("MODEL default
TOOL bash : Run a command.
MSG system : Be brief.
MSG user : hi
MSG assistant : <think>plan\\nBENDSIG::oair:ENC1</think>\\nok
  CALL 1 bash : ls
MSG tool : a.txt
MSG user : again
END
")

# what goes on the wire: the family's stream body of the plan body
def plan.body() -> String:
  Api.wire_body(Api.OpenAIResp{}, Api.api_body_for(Api.OpenAIResp{}, "gpt-6.1-sol",
    Api.MFacts{True{}, 4096n, "max_tokens", "", "", "medium", True{}}, plan.req()))

# LAW: the plan body, whole: streamed, not stored, no output cap though
# the catalog sets one, the system prompt a developer message, the tool
# inside the "bise" namespace, the replayed call carrying its namespace.
law resp_plan_body_maps:
  {plan.body()
    == "{\"stream\":true,\"model\":\"gpt-6.1-sol\",\"store\":false,\"reasoning\":{\"effort\":\"medium\",\"summary\":\"auto\"},\"include\":[\"reasoning.encrypted_content\"],\"input\":[{\"role\":\"developer\",\"content\":\"Be brief.\"},{\"role\":\"user\",\"content\":\"hi\"},{\"type\":\"reasoning\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"plan\"}],\"encrypted_content\":\"ENC1\"},{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"ok\"}]},{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\":\\\"ls\\\"}\",\"namespace\":\"bise\"},{\"type\":\"function_call_output\",\"call_id\":\"call_1\",\"output\":\"a.txt\"},{\"role\":\"user\",\"content\":\"again\"}],\"tools\":[{\"type\":\"namespace\",\"name\":\"bise\",\"description\":\"bise's tools\",\"tools\":[{\"type\":\"function\",\"name\":\"bash\",\"description\":\"Run a command.\",\"parameters\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"},\"description\":{\"type\":\"string\",\"description\":\"One short line shown to the user while this runs: what you are doing, in the user's language. 8 words at most, starting lowercase, no final period. E.g. 'weighing the hero image', 'je lance les tests du signup'.\"}},\"required\":[\"arg\"]},\"strict\":false}]}],\"tool_choice\":\"auto\"}"
    : String}

# LAW: present: "stream": true first, "store": false.
law resp_plan_streams_unstored:
  {Bool.and(String.starts_with(plan.body(), "{\"stream\":true,"),
    String.contains(plan.body(), "\"store\":false,")) == True{} : Bool}

# LAW: absent: every field the plan route refuses, as a key.
law resp_plan_no_refused_fields:
  {List.any(&2, String, k => String.contains(plan.body(), k),
    ["\"max_output_tokens\":", "\"temperature\":", "\"top_p\":", "\"metadata\":",
      "\"user\":", "\"prompt_cache_retention\":", "\"safety_identifier\":",
      "\"truncation\":", "\"previous_response_id\":", "\"background\":",
      "\"conversation\":", "\"max_tool_calls\":", "\"top_logprobs\":",
      "\"moderation\":", "\"prompt\":"])
    == False{} : Bool}

# LAW: absent: a role "system" item (the system prompt is a developer
# message); present: "tools" opens on the "bise" namespace (the
# function tools sit in it: resp_plan_body_maps pins the whole list).
law resp_plan_no_system_item_no_bare_tool:
  {Bool.or(String.contains(plan.body(), "\"role\":\"system\""),
    Bool.not(String.contains(plan.body(),
      "],\"tools\":[{\"type\":\"namespace\",\"name\":\"bise\",")))
    == False{} : Bool}

# LAW: the namespace round trip: a call the plan route sends back
# inside the "bise" namespace is a CALL line with the tool's own name,
# and that name maps back to the harness tool of the request.
law resp_plan_namespaced_call_back:
  {W.reply_names_in(W.req_tools(plan.req()),
      Rs.reply_any("event: response.completed
data: {\"type\":\"response.completed\",\"response\":{\"error\":null,\"output\":[{\"type\":\"function_call\",\"call_id\":\"call_x\",\"name\":\"bash\",\"namespace\":\"bise\",\"arguments\":\"{\\\"arg\\\":\\\"pwd\\\"}\"}]}}

"))
    == "OK 
CALL 1 bash : pwd
END
" : String}

# LAW: without the shape, the body is today's: no namespace, no
# namespace on a replayed call, the input items and tools as before
# (resp_body_maps and resp_body_plain_model pin today's whole bodies).
law resp_no_shape_items_unchanged:
  for xs: List<&2, J.Json>
  {Rs.items_for(False{}, xs) == xs : List<&2, J.Json>}

law resp_no_shape_tools_unchanged:
  for ts: List<&2, W.WTool>
  {Rs.tools_for(False{}, ts) == Rs.tools_json(ts) : List<&2, J.Json>}

law resp_no_shape_no_namespace:
  {String.contains(Api.api_body_for(Api.OpenAIResp{}, "gpt-6.1-sol",
      Api.MFacts{True{}, 4096n, "max_tokens", "", "", "medium", False{}}, plan.req()),
    "namespace") == False{} : Bool}

# LAW: the shape comes from the models file: providers.<p>.shape =
# "chatgpt-plan" is the plan; absent, or another word, is not.
law plan_shape_from_models_file:
  {Bool.and(Api.facts_plan(Pv.facts(Cf.parse_toml("[providers.chatgpt]
api = \"openai-responses\"
shape = \"chatgpt-plan\"
"), "chatgpt", "chatgpt/gpt-6.1-sol", "")),
    Bool.not(Bool.or(Api.facts_plan(Pv.facts(Cf.parse_toml("[providers.openai]
api = \"openai-responses\"
"), "openai", "openai/gpt-6.1-sol", "")),
      Api.facts_plan(Pv.facts(Cf.parse_toml("[providers.chatgpt]
shape = \"chatgpt\"
"), "chatgpt", "chatgpt/gpt-6.1-sol", "")))))
    == True{} : Bool}

# ---- the plan's errors, one line each (design, final words §5) ----

# LAW: the usage limit mid-stream (response.failed, a 200) ends the
# turn on bise's line, no retry.
law plan_limit_mid_stream:
  {Pv.attempt_plan(True{}, 200,
      Api.error_code(Api.OpenAIResp{}, "event: response.output_text.delta
data: {\"type\":\"response.output_text.delta\",\"delta\":\"hel\"}

event: response.failed
data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"subscription_sharing_usage_limit_exceeded\",\"message\":\"limit\"},\"output\":[]}}

"),
      Pv.ARetry{0, "provider 200: limit"})
    == Pv.AFail{"your ChatGPT plan's limit for bise is reached. your usage is at chatgpt.com/settings/usage, or switch model with /model."}
    : Pv.Attempt}

# LAW: the usage limit before the stream (a 429 with OpenAI's error
# object): the same line, not a retry.
law plan_limit_429:
  {Pv.attempt_plan(True{}, 429,
      Api.error_code(Api.OpenAIResp{}, "{\"error\":{\"code\":\"subscription_sharing_usage_limit_exceeded\",\"message\":\"limit\",\"type\":\"usage_limit\"}}"),
      Pv.attempt_status(429, "", "ERROR provider 429: limit
END
"))
    == Pv.AFail{"your ChatGPT plan's limit for bise is reached. your usage is at chatgpt.com/settings/usage, or switch model with /model."}
    : Pv.Attempt}

# LAW: usage unavailable mid-stream (OpenAI: the usage could not be
# checked, retry later with a bounded backoff): a retry, its cause
# named, not a final line.
law plan_usage_unavailable_mid_stream:
  {Pv.attempt_plan(True{}, 200,
      Api.error_code(Api.OpenAIResp{}, "event: response.failed
data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"subscription_sharing_usage_unavailable\",\"message\":\"off\"},\"output\":[]}}

"), Pv.ARetry{0, "provider 200: off"})
    == Pv.ARetry{0, "ChatGPT could not check your plan's usage (subscription_sharing_usage_unavailable)"}
    : Pv.Attempt}

# LAW: usage unavailable before the stream (a 503): a retry that keeps
# the server's Retry-After.
law plan_usage_unavailable_503:
  {Pv.attempt_plan(True{}, 503,
      Api.error_code(Api.OpenAIResp{}, "{\"error\":{\"code\":\"subscription_sharing_usage_unavailable\",\"message\":\"off\"}}"),
      Pv.attempt_status(503, "5", "ERROR provider 503: off
END
"))
    == Pv.ARetry{5000, "ChatGPT could not check your plan's usage (subscription_sharing_usage_unavailable)"}
    : Pv.Attempt}

# LAW: once the retries are spent on an unchecked usage, the turn ends
# on the line alone (no "provider failed after N attempts:"); any other
# cause keeps today's text.
law plan_usage_unavailable_exhausted:
  {Bool.and(String.eq(Pv.exhausted_for(10n,
        "ChatGPT could not check your plan's usage (subscription_sharing_usage_unavailable)"),
      "ChatGPT couldn't check your plan's usage. try again in a moment, or switch model with /model."),
    String.eq(Pv.exhausted_for(10n, "timeout"), Pv.exhausted_text(10n, "timeout")))
    == True{} : Bool}

# LAW: plan use not allowed for this user or workspace
# (subscription_sharing_user_not_eligible, a 403): final, its line.
law plan_not_eligible:
  {Pv.attempt_plan(True{}, 403,
      Api.error_code(Api.OpenAIResp{}, "{\"error\":{\"code\":\"subscription_sharing_user_not_eligible\",\"message\":\"no\"}}"),
      Pv.AFail{"OpenAI refused the key (403). check it with /setup."})
    == Pv.AFail{"ChatGPT plan use is off for bise. turn it on in your ChatGPT settings, or pick another provider in /provider."}
    : Pv.Attempt}

# LAW: a 401 on a plan provider (OpenAI's {"detail": ...} admission
# body, or any other) is the expired sign-in, never "refused the key".
law plan_401_expired:
  {Pv.attempt_plan(True{}, 401,
      Api.error_code(Api.OpenAIResp{}, "{\"detail\":\"Unauthorized\"}"),
      Pv.AFail{"OpenAI refused the key (401). check it with /setup."})
    == Pv.AFail{"your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt."}
    : Pv.Attempt}

# LAW: the token command failing on a plan provider (signed out, the
# refresh refused) is the expired sign-in too; another provider's
# key_command keeps its line.
law plan_key_command_failed:
  {Bool.and(String.eq(Pv.key_why(True{}, True{}, "chatgpt", ""),
      "your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt."),
    String.eq(Pv.key_why(True{}, False{}, "corp", ""),
      "no corp key: its key_command failed or printed nothing (config.toml)"))
    == True{} : Bool}

# LAW: a plan provider's other attempts are unchanged (a completed
# stream, another error code, another status).
law plan_other_attempts_unchanged:
  for a: Pv.Attempt
  {Pv.attempt_plan(True{}, 400, "invalid_request_error", a) == a : Pv.Attempt}

# LAW: without the shape, no attempt changes, whatever the code.
law plan_no_shape_unchanged:
  for +s: U32
  for +c: String
  for a: Pv.Attempt
  {Pv.attempt_plan(False{}, s, c, a) == a : Pv.Attempt}

# ---- a refused request, in bise's words and the provider's (BISE-293) ----
# The feed showed the provider's raw JSON twice; now one line: who
# refused, the status, what to check, then the provider's own message
# (its error.message), one line, ~200 chars, the key masked.

law refusal_quotes_the_provider:
  {Pv.refusal("OpenAI", 400,
      "{\n  \"error\": {\n    \"message\": \"Invalid 'tools[0].function.name': string does not match pattern.\",\n    \"type\": \"invalid_request_error\"\n  }\n}", "")
    == "OpenAI refused the request (400). OpenAI said: \"Invalid 'tools[0].function.name': string does not match pattern.\""
    : String}

law refusal_of_a_key_masks_it:
  {Pv.refusal("OpenAI", 401,
      "{\"error\":{\"message\":\"Incorrect API key provided: sk-abcdefghijklmnop.\"}}",
      "sk-abcdefghijklmnop")
    == "OpenAI refused the key (401). check it with /setup. OpenAI said: \"Incorrect API key provided: …mnop.\""
    : String}

law refusal_without_a_name_or_json:
  {Pv.refusal("", 404, "no  such\n model", "")
    == "the provider refused the request (404). check the model name and the provider URL. the provider said: \"no such model\""
    : String}

law refusal_without_words:
  {Pv.refusal("Mistral", 400, "", "")
    == "Mistral refused the request (400)." : String}

# LAW (big-request): a refusal for size says so, in the words the Core
# reads to compact: the foundry proxy's 400, a 413; another 400 does not.
law refusal_for_size:
  {Pv.refusal("Anthropic", 400,
      "{\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"Request content length exceeded 32 MB limit.\"}}", "")
    == "Anthropic refused the request (400). the request is too large. Anthropic said: \"Request content length exceeded 32 MB limit.\""
    : String}

law size_refused_cases:
  {Con{W.size_refused(400, "Request content length exceeded 32 MB limit."),
    Con{W.size_refused(413, ""),
      Con{W.size_refused(400, "{\"error\":{\"type\":\"request_too_large\"}}"),
        Con{W.size_refused(400, "invalid base64 data"),
          Con{W.size_refused(500, "request too large"), Nil{}}}}}}
    == Con{True{}, Con{True{}, Con{True{}, Con{False{}, Con{False{}, Nil{}}}}}}
    : List<&2, Bool>}

# LAW: the attempt the Core reads: a refusal in those words, a reply's
# calls named back, a transport failure (status 0) as it was.
law attempt_for_names_the_calls:
  {Pv.attempt_for([W.WTool{"a.b", ""}], "OpenAI", "", 200, "",
      Pv.AOk{"OK \nCALL 1 a_b : x\nEND\n"})
    == Pv.AOk{"OK \nCALL 1 a.b : x\nEND\n"} : Pv.Attempt}

law attempt_for_words_a_refusal:
  {Pv.attempt_for(Nil{}, "OpenAI", "", 400, "{\"error\":{\"message\":\"bad\"}}",
      Pv.AFail{"provider 400: bad"})
    == Pv.AFail{"OpenAI refused the request (400). OpenAI said: \"bad\""}
    : Pv.Attempt}

law attempt_for_keeps_a_transport_failure:
  {Pv.attempt_for(Nil{}, "OpenAI", "", 0, "", Pv.AFail{"no route to api.openai.com"})
    == Pv.AFail{"no route to api.openai.com"} : Pv.Attempt}

# ---- the config file (core/config.bend) ----
# A pragmatic TOML subset: key = value lines, [section] headers, #
# comments. Precedence everywhere: env var > config > default.

# LAW: flat keys parse byte-for-byte - strings quoted, numbers bare.
law config_parse_flat:
  {Cf.parse_toml("model = \"opus-5.5\"\ncompaction_threshold = 800000\n")
    == Con{Cf.KV{"model", "opus-5.5"},
      Con{Cf.KV{"compaction_threshold", "800000"},
      Nil{}}} : List<&2, Cf.KV>}

# LAW: comments and blank lines never reach the pairs.
law config_skips_comments_blanks:
  {Cf.parse_toml("# a comment\n\n  \nmodel = \"m\"\n")
    == Con{Cf.KV{"model", "m"}, Nil{}} : List<&2, Cf.KV>}

# LAW: a key inside a section carries the section prefix.
law config_sections_prefix:
  {Cf.parse_toml("[harness]\nbg_after = 5\n")
    == Con{Cf.KV{"harness.bg_after", "5"}, Nil{}} : List<&2, Cf.KV>}

# LAW: a quoted value may carry '=' itself (the pair splits on the
# FIRST one only).
law config_value_keeps_equals:
  {Cf.parse_toml("cmd = \"a=b\"\n")
    == Con{Cf.KV{"cmd", "a=b"}, Nil{}} : List<&2, Cf.KV>}

# LAW: the lookups answer the exact key - Some when present, the
# default when not.
law config_lookup_found:
  {Cf.cfg_get("model = \"opus-5.5\"\n", "model") == Some{"opus-5.5"}
    : Maybe<&2, String>}

law config_lookup_missing:
  {Cf.cfg_or(Cf.cfg_get("model = \"m\"\n", "compaction_threshold"), "800000")
    == "800000" : String}

# LAW: numbers ride as U32.
law config_u32_parses:
  {Cf.cfg_u32("bg_after = 45\n", "bg_after", 30) == 45 : U32}

law config_u32_defaults:
  {Cf.cfg_u32("model = \"m\"\n", "bg_after", 30) == 30 : U32}

# LAW: the precedence - the environment variable wins, then the
# config file, then the default.
law config_env_wins:
  {Cf.env_cfg_or("zai-glm-5-3",
      Cf.cfg_get("model = \"opus-5.5\"\n", "model"), "claude-opus-5-5")
    == "zai-glm-5-3" : String}

law config_env_empty_falls_to_file:
  {Cf.env_cfg_or("",
      Cf.cfg_get("model = \"opus-5.5\"\n", "model"), "claude-opus-5-5")
    == "opus-5.5" : String}

law config_env_empty_no_file:
  {Cf.env_cfg_or("", Cf.cfg_get("", "model"), "claude-opus-5-5")
    == "claude-opus-5-5" : String}

# LAW: the template the runtime writes parses - its own keys are
# what the lookups find.
law config_template_parses:
  {Cf.cfg_get(St.CONFIG_TEMPLATE(), "model") == None{}
    : Maybe<&2, String>}

law config_template_threshold:
  {Cf.cfg_u32(St.CONFIG_TEMPLATE(), "compaction_threshold",
      800000) == 800000
    : U32}

# ---- the harness-info line: the single source of truth for clients ----
# The TUI (through the bend-harness parent) and bend_client.py read
# this line instead of recomputing the model, the threshold or the
# side-channel paths. Its format is a contract with both parsers.
law info_line_format:
  {Rt.info_line("claude-opus-5-5", "800000", "/tmp/bend-steer-7.txt",
      "/tmp/bend-interrupt-7.txt")
    == "harness-info model=claude-opus-5-5 threshold=800000 steer=/tmp/bend-steer-7.txt interrupt=/tmp/bend-interrupt-7.txt"
    : String}

# LAW: the announced side-channel paths are the ones the runtime
# drains - the same builders, per port.
law info_paths_are_the_drained_ones:
  {Bool.and(String.eq(Rt.steer_path_of("/r", "7"), "/r/bend-steer-7.txt"),
     String.eq(Pv.interrupt_path_of("/r", "7"), "/r/bend-interrupt-7.txt"))
    == True{} : Bool}

# ---- --resume: the restored history replays as live wire lines ----

# LAW: a restored history replays as the lines a live turn prints, each
# prefixed "history ": the compaction summary (its preamble skipped), the
# user message, each tool call with its code and its result (paired in
# call order, no id in the result), the assistant text on ONE wire line
# (a real newline escaped as backslash-n, chars 92 110, as live replies
# arrive); a call left without a result (interrupt, crash) replays as
# failed.
law resume_replays_history:
  {Rt.replay(Con{T.injected_user("The earlier conversation was compacted. A summary replaces it; continue from the preserved user messages."),
      Con{T.injected_user("Summary of the earlier conversation:
<summary>court</summary>"),
      Con{T.user("salut"),
      Con{T.asst("", Con{T.Call{7, "bash", "ls -la"}, Nil{}}),
      Con{T.tool_res("tool bash failed: boom"),
      Con{T.asst("<think>hmm</think>
fini", Nil{}),
      Con{T.asst("", Con{T.Call{9, "bash", "pwd"}, Nil{}}),
      Nil{}}}}}}}})
    == Con{"history   obs: compaction_done: court...",
      Con{"history you : salut",
      Con{"history   obs: tool_started #7",
      Con{"history tool #7 bash : ls -la",
      Con{"history tool_code #7 : ls -la",
      Con{"history tool_result #7 fail : boom",
      Con{"history   obs: tool_finished #7 failed",
      Con{String.append("history   obs: assistant: <think>hmm</think>",
        SCon{Chr{92}, SCon{Chr{110}, "fini"}}),
      Con{"history   obs: tool_started #9",
      Con{"history tool #9 bash : pwd",
      Con{"history tool_code #9 : pwd",
      Con{"history   obs: tool_finished #9 failed",
      Nil{}}}}}}}}}}}}}
    : List<&2, String>}

# ---- the Anthropic stream (core/anth-stream.bend) ----

# LAW: a streamed request is the same body with "stream": true first.
law anth_stream_body:
  {As.stream_body("{\"model\":\"m\"}") == "{\"stream\":true,\"model\":\"m\"}" : String}

# LAW: text and tool input stream in pieces; the reply is rebuilt whole -
# pings, padding and the empty first input piece change nothing.
law anth_sse_tool_call:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}      \n\nevent: ping\ndata: {\"type\": \"ping\"}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"I'll list\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" /tmp.\"}}   \n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"bash\",\"input\":{},\"caller\":{\"type\":\"direct\"}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"arg\\\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\": \\\"ls -la /tmp\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\" | head\\\"}\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":72}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == "OK I'll list /tmp.\nCALL 1 bash : ls -la /tmp | head\nEND\n" : String}

# LAW: a streamed reply maps to exactly what the same message sent whole
# maps to - signed thinking included (the signature streams as its own
# delta and must survive for the replay).
law anth_sse_matches_whole_reply:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"2+2\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\" = 4\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"4\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == Api.reply_of_for(Api.Anthropic{}, 200, "{\"type\":\"message\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"2+2 = 4\",\"signature\":\"sig\"},{\"type\":\"text\",\"text\":\"4\"}],\"stop_reason\":\"end_turn\"}") : String}

# LAW: an error event mid-stream is the reply: the 200 error body the
# retry policy already retries.
law anth_sse_error_event:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: ping\ndata: {\"type\":\"ping\"}\n\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# LAW: a streamed reply prints the usage a whole reply would: the input
# side from message_start, output_tokens from the last message_delta.
law anth_sse_usage_line:
  {Us.usage_line(Api.Anthropic{}, "m", 200, "event: message_start
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"content\":[],\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":2,\"cache_read_input_tokens\":30,\"output_tokens\":1}}}

event: message_delta
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":72}}

")
    == "  obs: usage: model=m in=42 out=72 cache_read=30 cache_write=2" : String}

# LAW: a stream with no message_start (an error event) prints no usage.
law anth_sse_no_usage:
  {Us.usage_line(Api.Anthropic{}, "m", 200, "event: error
data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}

") == "" : String}

# LAW: a streamed refusal surfaces like a whole one.
law anth_sse_refusal:
  {As.reply("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"stop_reason\":null}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":{\"type\":\"refusal\",\"explanation\":\"blocked\"}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n")
    == "ERROR provider refusal: blocked\nEND\n" : String}

# LAW: a 200 that is a whole JSON error body (the proxy's overload
# answer, even to a streamed request) still maps to the provider error
# the retry policy retries.
law anth_stream_json_error_body:
  {As.reply_any("{\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}")
    == "ERROR provider 200: Overloaded\nEND\n" : String}

# ---- the OpenAI Chat stream (core/oai-stream.bend, BISE-144) ----

# LAW: a streamed OpenAI Chat request is the same body with "stream":
# true and the usage asked for (stream_options.include_usage) first.
law oai_stream_body:
  {Os.stream_body("{\"model\":\"m\"}")
    == "{\"stream\":true,\"stream_options\":{\"include_usage\":true},\"model\":\"m\"}" : String}

# LAW: text split over many chunks is rebuilt whole: the streamed reply
# maps to what the same message sent whole maps to (reasoning_content
# rides along as the whole message carries it).
law oai_stream_text_split:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":null,\"reasoning_content\":\"Think\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo wor\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ld\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}

data: [DONE]

")
    == Oai.reply_of(200, "{\"object\":\"chat.completion\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":\"Hello world\",\"reasoning_content\":\"Think\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}") : String}

law oai_stream_text_split_is:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":null,\"reasoning_content\":\"Think\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo wor\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ld\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}

data: [DONE]

")
    == "OK Hello world
END
" : String}

# LAW: tool_calls deltas are gathered by index, even interleaved: the
# first piece of an index has id + name, the next ones argument text.
law oai_stream_interleaved_tools:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"a\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"b\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\"\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"arg\\\": \\\"l\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"function\":{\"arguments\":\": \\\"pwd\\\"}\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"s\\\"}\"}}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}

data: [DONE]

")
    == "OK 
CALL 1 bash : ls
CALL 2 bash : pwd
END
" : String}

# LAW: the usage comes in the last chunk (choices []): the usage line a
# whole reply prints (prompt_tokens counts the cached ones).
law oai_stream_usage_line:
  {Us.usage_line(Api.OpenAI{}, "m", 200, "data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":null,\"reasoning_content\":\"Think\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo wor\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ld\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":2}}}

data: [DONE]

")
    == "  obs: usage: model=m in=10 out=5 cache_read=2 cache_write=0" : String}

# LAW: Mistral sends a whole tool call in one delta, in the chunk that
# also carries finish_reason and the usage.
law oai_stream_mistral_one_delta:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"p\":\"abc\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"wBvFpU1Qf\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\": \\\"echo ok\\\"}\"},\"index\":0}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":115,\"total_tokens\":133,\"completion_tokens\":18,\"prompt_tokens_details\":{\"cached_tokens\":0}}}

data: [DONE]

")
    == "OK 
CALL 1 bash : echo ok
END
" : String}

law oai_stream_mistral_usage:
  {Us.usage_line(Api.OpenAI{}, "m", 200, "data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"p\":\"abc\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"wBvFpU1Qf\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"arg\\\": \\\"echo ok\\\"}\"},\"index\":0}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":115,\"total_tokens\":133,\"completion_tokens\":18,\"prompt_tokens_details\":{\"cached_tokens\":0}}}

data: [DONE]

")
    == "  obs: usage: model=m in=115 out=18 cache_read=0 cache_write=0" : String}

# LAW: Mistral's reasoning models stream content as blocks (thinking,
# text): rebuilt as the whole reply's blocks, pieces joined.
law oai_stream_thinking_blocks:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2\"}]}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\" = 4\"}]}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":[{\"type\":\"text\",\"text\":\"4\"}]},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"},\"finish_reason\":null}]}

data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}

data: [DONE]

")
    == Oai.reply_of(200, "{\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"2+2 = 4\"}]},{\"type\":\"text\",\"text\":\"4!\"}]},\"finish_reason\":\"stop\"}]}") : String}

# LAW: an error chunk mid-stream (OpenRouter) is the reply: a 200 error
# body, which the retry policy retries (its message only, BISE-293).
law oai_stream_error_chunk:
  {Os.reply("data: {\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}],\"usage\":null}

data: {\"object\":\"chat.completion.chunk\",\"error\":{\"code\":\"server_error\",\"message\":\"Internal server error\"},\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\"},\"finish_reason\":\"error\"}]}

")
    == "ERROR provider 200: Internal server error
END
" : String}

# LAW: a 200 that is one JSON body (a server ignoring "stream") maps as
# a whole reply.
law oai_stream_json_body:
  {Api.stream_reply(Api.OpenAI{}, "{\"choices\":[{\"message\":{\"content\":\"hi\"}}]}")
    == Oai.reply_of(200, "{\"choices\":[{\"message\":{\"content\":\"hi\"}}]}") : String}

# LAW: the catalog drives the body (BISE-145): a model that does not
# reason gets no reasoning_effort; its max_output is the cap, named
# max_completion_tokens on OpenAI, max_tokens elsewhere.
law oai_body_no_reasoning_cap:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.MFacts{False{}, 4096n, "max_tokens", "", "", "", False{}},
      W.parse_wire("MODEL default
MSG user : hi
END
"))
    == "{\"model\":\"m\",\"max_tokens\":4096,\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}" : String}

law oai_body_openai_cap:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.MFacts{True{}, 128000n, "max_completion_tokens", "", "", "high", False{}},
      W.parse_wire("MODEL default
MSG user : hi
END
"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"max_completion_tokens\":128000,\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}" : String}

# LAW: the facts come from the models file, a model's key over its
# provider's; the OpenAI provider names its cap max_completion_tokens.
# (a small max_output: a Nat literal past 256n is too deep to check)
law provider_facts_from_file:
  {Pv.resolve("", "", "", "", "model = \"openai/gpt-4.1\"",
      "[providers.openai]
api = \"openai-chat\"
base_url = \"https://api.openai.com/v1\"
key_env = \"OPENAI_API_KEY\"
max_output = 128000
reasoning = true
[models.\"openai/gpt-4.1\"]
max_output = 200
reasoning = false",
      "")
    == Pv.ROk{Pv.Prov{"openai", "gpt-4.1", Api.OpenAI{},
      "https://api.openai.com/v1/chat/completions", "OPENAI_API_KEY",
      Api.MFacts{False{}, 200n, "max_completion_tokens", "", "", "", False{}}}}
    : Pv.Resolved}

# LAW: only a non-empty flag is a Ctrl+C - the runtime clears the flag to
# "" at every turn end, and that must never read as an interrupt.
law interrupt_flag_semantics:
  {Bool.and(Pv.interrupt_requested("1"),
      Bool.and(Bool.not(Pv.interrupt_requested("")),
        Bool.not(Pv.interrupt_requested("\n"))))
    == True{} : Bool}

# LAW: a streamed read waits 90 s at most (events come every few seconds).
law provider_stream_step_timeout:
  {Pv.stream_step_ms() == 90000 : U32}

# LAW: a provider with no idle_timeout_sec (or 0, or not a number) waits
# 90 s, as before the key existed: nothing changes for who sets nothing.
law provider_idle_timeout_default:
  {Bool.and(U32.is_eq(Pv.wait_ms(Pv.stream_wait("", "fake", "fake/m")), 90000),
      Bool.and(U32.is_eq(Pv.wait_ms(Pv.stream_wait(
          "[providers.fake]\nidle_timeout_sec = 0\n", "fake", "fake/m")), 90000),
        U32.is_eq(Pv.wait_ms(Pv.stream_wait(
          "[providers.fake]\nidle_timeout_sec = \"soon\"\n", "fake", "fake/m")), 90000)))
    == True{} : Bool}

# LAW: idle_timeout_sec under [providers.<id>] sets the wait of its
# models, a model's own table wins, another provider keeps 90 s, and a
# huge value is capped at a day.
law provider_idle_timeout_set:
  {Bool.and(U32.is_eq(Pv.wait_ms(Pv.stream_wait(
        "[providers.fake]\nidle_timeout_sec = 600\n", "fake", "fake/m")), 600000),
      Bool.and(U32.is_eq(Pv.wait_ms(Pv.stream_wait(
          "[providers.fake]\nidle_timeout_sec = 600\n[models.\"fake/m\"]\nidle_timeout_sec = 5\n",
          "fake", "fake/m")), 5000),
        Bool.and(U32.is_eq(Pv.wait_ms(Pv.stream_wait(
            "[providers.fake]\nidle_timeout_sec = 600\n", "mistral", "mistral/m")), 90000),
          U32.is_eq(Pv.wait_ms(Pv.stream_wait(
            "[providers.fake]\nidle_timeout_sec = 99999999\n", "fake", "fake/m")), 86400000))))
    == True{} : Bool}

# LAW: a streamed call that times out still retries, and its text names
# the host, the limit and the key to raise, under its provider's table.
law attempt_timeout_names_the_key:
  {Pv.attempt_err_in("127.0.0.1:8080", Pv.Wait{"ollama", 5000}, Pv.NTimeout{})
    == Pv.ARetry{0, "no answer from 127.0.0.1:8080 in 5 s (timeout) — a slow model or gateway? raise idle_timeout_sec under [providers.ollama] in ~/.bise/config.toml"}
    : Pv.Attempt}

# LAW: the other transport failures of a streamed call read as before.
law attempt_err_in_others:
  {Pv.attempt_err_in("api.example.com", Pv.Wait{"x", 5000}, Pv.NDns{})
    == Pv.attempt_err("api.example.com", Pv.NDns{}) : Pv.Attempt}

# LAW: a debug event is one JSON object on one line: ts, src, kind, then
# the fields in order; strings are JSON-escaped (a newline stays \\n).
law debug_event_line:
  {Dg.ev_line(7n, "a\"b", Con{Dg.FStr{"k", "x\ny"}, Con{Dg.FNat{"n", 3n}, Nil{}}})
    == "{\"ts\":7,\"src\":\"repl\",\"kind\":\"a\\\"b\",\"k\":\"x\\ny\",\"n\":3}" : String}

# LAW: only obs and tool annotations become breadcrumbs, one line each.
law debug_crumbs_filter:
  {Dg.crumbs(Con{"  obs: turn_started", Con{"hello", Con{"tool #3 bash : ls", Nil{}}}}, 5n)
    == "{\"ts\":5,\"src\":\"repl\",\"kind\":\"sent\",\"line\":\"obs: turn_started\"}\n{\"ts\":5,\"src\":\"repl\",\"kind\":\"sent\",\"line\":\"tool #3 bash : ls\"}\n" : String}

# LAW: a first attempt that works is not logged; a later one that works
# is a recovery; a failure, a retry or a 200 carrying an ERROR reply is
# an incident.
law debug_plan_ok_first:
  {Dg.plan(Pv.AOk{"OK\nEND\n"}, 0n) == Dg.PSkip{} : Dg.Plan}

law debug_plan_recovered:
  {Dg.plan(Pv.AOk{"OK\nEND\n"}, 2n) == Dg.PRecovered{} : Dg.Plan}

law debug_plan_error_reply:
  {Dg.plan(Pv.AOk{"ERROR provider refusal: x"}, 0n)
    == Dg.PFailed{"error_reply", "ERROR provider refusal: x", 0} : Dg.Plan}

law debug_plan_retry:
  {Dg.plan(Pv.ARetry{5000, "provider 529 (transient)"}, 1n)
    == Dg.PFailed{"retry", "provider 529 (transient)", 5000} : Dg.Plan}

# LAW: an artifact is cut at the limit, and says how long it was.
law debug_clip:
  {Bool.and(String.eq(Dg.clip("abcdef", 3n), "abc\n[... truncated: 6 chars in total]"),
      String.eq(Dg.clip("abc", 3n), "abc"))
    == True{} : Bool}

# LAW: the reply artifact names the attempt 1-based, like the retry notes.
law debug_reply_name:
  {Dg.reply_name(12n, 0n) == "reply-12-1.txt" : String}

# LAW: the clock probe answers "<status>\n<epoch ms>": the offset is
# epoch - mono; an unreadable answer gives no offset (0).
law debug_clock_anchor:
  {Dg.anchor(Con{"0", Con{"1500", Nil{}}}, 500n) == 1000n : Nat}

law debug_clock_anchor_failed:
  {Dg.anchor(Con{"127", Con{"sh: perl: not found", Nil{}}}, 500n) == 0n : Nat}

# LAW: an event is stamped mono + the stored offset, mono without one.
law debug_stamp:
  {Bool.and(Nat.is_eq(Dg.stamp(Some{"1000\n"}, 5n), 1005n),
      Nat.is_eq(Dg.stamp(None{}, 5n), 5n))
    == True{} : Bool}

# ---- switchboard hooks (BEND_WORKDIR, BEND_CONTEXT_FILE, BEND_EXTRA_PROMPT) ----

# LAW: without BEND_WORKDIR the command is unchanged.
law workdir_empty_keeps_command:
  {Sh.with_workdir("", "ls -la") == "ls -la" : String}

# LAW: with BEND_WORKDIR the command first enters the (quoted) directory,
# and fails instead of running elsewhere when it cannot.
law workdir_cds_first:
  {Sh.with_workdir("/tmp/a b", "ls") == "cd '/tmp/a b' || exit 1\nls" : String}

# LAW: an empty context changes nothing.
law context_empty_keeps_request:
  {Rm.with_context("", "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n")
    == "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n" : String}

# LAW: a context becomes the LAST message of an agent request (role
# `context`: a user message on the wire's families, never a cache
# breakpoint, BISE-268), newlines escaped like every wire message.
law context_is_last_user_message:
  {Rm.with_context("board\nline 2", "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nEND\n")
    == "MODEL default\nTOOL bash : run\n\nMSG user : hi\n\nMSG context : board\\nline 2\nEND\n" : String}

# LAW: a compaction request (no tool catalog) never carries the context.
law context_skips_compaction:
  {Rm.with_context("board", "MODEL default\nMSG user : hi\n\nEND\n")
    == "MODEL default\nMSG user : hi\n\nEND\n" : String}

# LAW: nor does the first attempt of a compaction, which carries the
# tools to share the agent calls' prefix (BISE-268): its COMPACT line.
law context_skips_compaction_with_tools:
  {Rm.with_context("board", "MODEL default\nCOMPACT\nTOOL bash : run\n\nMSG user : hi\n\nEND\n")
    == "MODEL default\nCOMPACT\nTOOL bash : run\n\nMSG user : hi\n\nEND\n" : String}

# LAW: the extra prompt ends the system prompt; an empty one adds nothing.
law extra_prompt_appends:
  {Rp.prompt_extra("base", "role") == "base\n\nrole" : String}

law extra_prompt_empty_keeps:
  {Rp.prompt_extra("base", "  ") == "base" : String}

# LAW: a session without a role (solo) gets the tone block; one with a
# role does not (the role carries the same block): never twice.
law solo_tone_without_role:
  {Rp.solo_tone("  ", "tone") == "tone" : String}

law solo_tone_not_with_role:
  {Rp.solo_tone("role", "tone") == "" : String}

# LAW: a drained steering line carries escaped newlines back as real ones.
law steer_line_unescapes:
  {Rt.steer_cmd("a\\nb") == T.UserMsg{T.Steer{}, "a\nb"} : T.Cmd}

# ---- Switchboard hub (hub/*.bend, the sb-core process) ----
# The laws quantify over EVERY hub state, EVERY input (any JSON line the
# daemon may send) and EVERY sequence of inputs: no test could enumerate
# them. HC.step is the decision of one input, HC.commit what sb-core keeps
# of it, HC.run its loop over a list of inputs.

# LAW: after any input, the hub's state is exactly the replay of the
# journal events it emitted over the state before (the journal is a
# faithful log: a restart that replays it rebuilds the same state).
law hub_step_replays:
  for +h: HC.Hub
  for +i: J.Json
  {HX.Core.d(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))
    == HM.apply_all(HX.journal(HX.Core.fx(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))), HC.Hub.d(h)) : HM.St}

# LAW: over any sequence of inputs (git answers still missing included),
# the final state is the replay of the whole journal over the first one.
law hub_run_replays:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  {HC.Hub.d(HC.RunR.hub(HC.run(ins, h))) == HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)) : HM.St}

# LAW: every message the hub ever sends, over any sequence of inputs,
# takes the next free id of the state it is sent in: no id is reused, so
# no message overwrites (loses) another.
law msg_ids_fresh:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Fresh(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h))

# LAW: no message the hub ever sends is both an automatic reply and a
# question (RFC 0003 §7): automatic replies can never start a chain.
law auto_never_asks:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Quiet(HC.RunR.journal(HC.run(ins, h)))

# LAW: after a tick, every wait still pending ends later than now: a wait
# past its deadline always gets its timeout (with the 25 s cap of `sb
# wait`, no set of agents waiting on each other blocks longer).
law tick_clears_late_waiters:
  for -o: HM.St
  for +cx: HX.Cx<o>
  HC.Later(HC.waiters_of_cx(o, HC.tick(o, cx)), HX.R.now(HX.Cx.r(o, cx)))

# LAW: stopping or dropping a task leaves no wait of it behind (each got
# its `stopped` error), whatever the state.
law stop_leaves_no_waiter:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HM.Life
  for +why: String
  HC.NoneOf(HC.waiters_of_cx(o, HC.stop_task(o, cx, n, life, why)), n)

# ---- the timers of `sb every` (hub/timers.bend, core.bend `timer_fire`) ----
# The timers' state is in HM.St and their lines (every_set, every_fired,
# every_run, every_stop) are events like the others: hub_run_replays,
# msg_ids_fresh and the rest cover them as they are. The two laws on the
# wake are LOCAL: they are stated on core.bend's `timer_fire`, the only
# path a timer wake takes (the tick's due wakes and /scheduled's run now),
# not over any sequence of inputs: the global form would need a fourth
# proof field in HX.Core and an obligation at every emit of the hub.

# LAW: a fire counts only when its message was queued (amb-tools m_5822: a
# one-shot timer was spent on a lost wake). After the wake's send `p`: a
# refused send adds only the retry line (a due wake: every_fired
# undelivered, fired unchanged, next in a minute; a run now: nothing), no
# count and no stop; a queued one adds exactly the fire (every_fired with
# fired + 1, or every_run for a run now) and, for its last time, the
# timer's stop.
law fire_is_a_message:
  for -o: HM.St
  for p: HX.Cx<o> & HC.SendR
  for +t: HTM.Timer
  for +due: Bool
  for +next: Nat
  {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.timer_fire.sent(o, p, t, due, next)))) == HI.fire_journal(o, p, t, due, next) : List<&2, HM.Ev>}

# LAW: a timer never stacks wakes: while a message from bise is still
# queued for its agent, a fire (due or run now) changes nothing at all.
law wake_never_stacked:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +t: HTM.Timer
  for +text: String
  for +due: Bool
  for +next: Nat
  for q: {HC.bise_queued(HX.d(o, cx), HTM.Timer.agent(t)) == True{} : Bool}
  {HC.timer_fire(o, cx, t, text, due, next) == cx : HX.Cx<o>}

# LAW (per event): a counted fire leaves a timer below its times, or it was
# the last one (`spent`: fire_is_a_message shows the same step stops it).
# With the guard (a due wake fires only below its times, HTM.may_fire), a
# timer with times = n never fires more than n times.
law times_bound:
  for +t: HTM.Timer
  for +next: Nat
  for +at: Nat
  {HTM.under(HTM.Timer.fire(t, 1n+HTM.Timer.fired(t), next, at, False{})) == Bool.not(HTM.spent(t)) : Bool}

# LAW: dropping (archiving) a task leaves no live timer of it, in the same
# step (beside stop_leaves_no_waiter). A stop (`sb stop`) keeps them: they
# wait, as before.
law stop_leaves_no_timer:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for +why: String
  HI.TimerNoneOf(HTM.Tms.live(HM.St.tm(HX.d(o, HC.stop_task(o, cx, n, HM.LArchived{}, why)))), n)

# LAW (L2, no message lost or doubled): a new hub keeps each message as
# ONE record — its text and its single delivery state; no state exists
# without its message (the state is a field of the record).
law init_unique:
  for +w: String
  HI.Unique(HM.init(w))

# LAW (L2): from a state where each message is one record, any sequence
# of inputs keeps it so: no two records ever share an id.
law msgs_unique:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.Unique(HC.Hub.d(h))
  HI.Unique(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L2): over any sequence of inputs, the messages of the first state
# start, in order, the messages of the last: a message once sent is never
# removed (its record, and so its state, stays).
law msgs_kept:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HI.Kept(HC.Hub.d(h), HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L3, delivered at most once, RFC 0003 §6.1): over any sequence of
# inputs, a message becomes Delivered only while it is Queued, and it goes
# back to Queued only as an explicit steer_leftover (steering the runtime
# never read). So a message is delivered once, and again only after an
# explicit steer_leftover.
law delivered_once:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  HX.Deliv(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h))

# LAW (L12): stopping or dropping a task, in any state where each message
# is one record (every state the hub reaches: msgs_unique), leaves no
# message queued for it (each got recipient_unavailable).
law stop_leaves_no_queue:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HM.Life
  for +why: String
  for u: HI.Unique(HX.d(o, cx))
  {HM.queued_for(HX.d(o, HC.stop_task(o, cx, n, life, why)), n) == Nil{} : List<&2, HM.Msg>}

# ---- queued user input (the window's "send queued", bise desktop) ----
# The user's `input {queued: true}` is a queued-mode message (Msg.queued),
# the path `sb send --mode queued` already takes. An older sb-core ignores
# the field and delivers at once (a safe degrade, no journal change).

# LAW: a queued input never answers a question card, at send: it is a
# message for the next turn. A card still open at the turn's end stays
# open; the input arrives as a message (says.card with no card).
law user_queued_never_answers_a_card:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +to: String
  for +text: String
  {HC.user_says(o, cx, to, text, True{}) == HC.says.card(o, None{}, cx, to, text, True{}) : HX.Cx<o>}

def any_queued(xs: List<&2, HM.Msg>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      HM.Msg.queued(h) || any_queued(t)

# LAW: what a busy agent is handed (steered) holds no queued-mode message:
# a queued input waits for the turn's end. So a plain input sent while
# queued ones wait goes first (steered); the queued ones follow at idle,
# in id order (queued_for), in one say batch (user_queued_said_at_idle),
# each once (delivered_once).
law user_queued_waits_for_turn_end:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +name: String
  for +q: List<&2, HM.Msg>
  for +m: Bool
  {HC.pump.run(o, HM.RBusy{}, cx, name, q, m) == HC.pump.batch(o, HC.not_queued_mode(q), cx, name, True{}, m) : HX.Cx<o>}

law busy_batch_has_no_queued:
  for +xs: List<&2, HM.Msg>
  {any_queued(HC.not_queued_mode(xs)) == False{} : Bool}

# LAW: at idle, everything queued for the agent goes in one say batch, in
# its order (queued and plain alike).
law user_queued_said_at_idle:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +name: String
  for +q: List<&2, HM.Msg>
  for +m: Bool
  {HC.pump.run(o, HM.RIdle{}, cx, name, q, m) == HC.pump.batch(o, q, cx, name, False{}, m) : HX.Cx<o>}

# ---- the composer rule (lead m_8777, architect m_8801) ----
# An approval card takes one of its options, never typed words: the TUI's
# /answer and the window share it. The words leave the card open and wake
# nobody (the window sends them to the agent as a message).

# LAW: typed words on a confirm card (not 1, 2, 3 nor the TUI's explicit
# `deny: <why>`) are refused: no verdict, the card stays open.
law card_confirm_words_refused:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +c: HM.Card
  for +text: String
  for e: {HC.confirm_ok(text) == False{} : Bool}
  {HC.answer.kind(o, HC.KConfirm{}, cx, c, text) == HC.words_refused(o, cx, c, HC.confirm_opts()) : HX.Cx<o>}

# LAW: `no` and `no: <why>` (any case, lead m_10900) are deny aliases: the
# card gets the same verdict path as its option 3, with his words as the
# note (the gate reads them as no, never allow: gate::answer_of).
law card_confirm_no_alias:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +c: HM.Card
  for +text: String
  for e: {HC.is_no(String.to_lower(String.trim(text))) == True{} : Bool}
  {HC.answer.kind(o, HC.KConfirm{}, cx, c, text) == HC.answer.confirm(o, True{}, cx, c, text) : HX.Cx<o>}

# LAW: only `no` itself or its `no:` prefix: words that start like it stay
# words.
law card_confirm_nope_is_words:
  {HC.confirm_ok("nope") == False{} : Bool}

law card_confirm_now_is_words:
  {HC.confirm_ok("now") == False{} : Bool}

law card_confirm_no_thanks_is_words:
  {HC.confirm_ok("No thanks") == False{} : Bool}

# LAW: a hub item (merge, feature_*, update, signin) takes only one of its
# options' numbers: other words are refused, the item stays open, main is
# not woken.
law card_choice_words_refused:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +c: HM.Card
  for +text: String
  for e: {HC.is_option(HM.Card.text(c), text) == False{} : Bool}
  {HC.answer.kind(o, HC.KChoice{}, cx, c, text) == HC.words_refused(o, cx, c, "one of its numbers") : HX.Cx<o>}

# ---- bise's cross-hub messages (desktop S2, hub/xhub.bend) ----
# Local laws on xhub.bend's decisions and xhub-model.bend's state: the
# core's `cross-hub` section only wires them (it decides nothing).

# LAW: only main forwards the user's words to a project.
law xhub_only_main_forwards:
  for +s: HM.St
  for +from: String
  for +id: Nat
  for e: {String.eq(from, HM.MAIN()) == False{} : Bool}
  {HXB.forward(s, from, id) == HXB.FwdErr{"only main forwards to the projects"} : HXB.Fwd}

# LAW: a message of his is forwarded once: with an outbox entry for it,
# main's forward is refused.
law xhub_forward_once:
  for +s: HM.St
  for +id: Nat
  for +ob: HXH.Out
  for e: {HXH.out_by_msg(HXH.Xs.outbox(HM.St.x(s)), id) == Some{ob} : Maybe<&2, HXH.Out>}
  {HXB.forward(s, HM.MAIN(), id) == HXB.FwdErr{String.append(String.append(HXB.mid(id), " was already forwarded to "), HXH.Out.project(ob))} : HXB.Fwd}

# LAW: only his own words go by reference (architect's B): a message not
# from the user is refused, whatever it says (bise's own question goes by
# `sb project ask`, from @bise, with no authority).
law xhub_forward_needs_his_message:
  for +m: HM.Msg
  for +id: Nat
  for e: {String.eq(HM.Msg.from(m), HM.USER()) == False{} : Bool}
  {HXB.his.go(Some{m}, id) == HXB.FwdErr{String.append(HXB.mid(id), " is not a message the user sent you: forward his own words only (your own question: sb project ask)")} : HXB.Fwd}

# LAW: bise's own question needs words: a blank one is refused, any other
# may go (architect m_8864: the core's section validates nothing itself).
law xhub_ask_needs_words:
  for +text: String
  for b: Bool
  for e: {HM.trim_is_empty(text) == b : Bool}
  {HXB.ask_ok(text) == Bool.pick(Maybe<&2, String>, b, Some{"empty question"}, None{}) : Maybe<&2, String>}

# LAW: a send settles once: after its settle, no answer opens it again
# (a second x_reply for it changes nothing: core's xanswer on None).
law xhub_outbox_settles_once:
  for +x: HXH.Xs
  for +xid: Nat
  {HXB.open(HXH.x_op(x, xid, HXH.OpSettled{}), xid) == None{} : Maybe<&2, HXH.Out>}

# LAW: a delivery is said once: once received, the same origin (hub, xid)
# is seen (core's xin acks it and says nothing).
law xhub_xin_delivered_once:
  for +x: HXH.Xs
  for +i: HXH.XIn
  {HXB.seen(HXH.x_in(x, i), HXH.XIn.hub(i), HXH.XIn.xid(i)) == True{} : Bool}

# LAW: an answered record is never due again (core's xreplied journals
# x_replied with each answer it sends back).
law xhub_xin_replied_once:
  for +i: HXH.XIn
  {HXB.reply.of(Some{HXH.XIn.done(i)}) == None{} : Maybe<&2, HXH.XIn>}

# ---- the routing hold (desktop S2 step 2, xhub.bend + core's `routing
# hold` section) ----

# LAW: a route has one outcome: once it ended (sent or cancelled), it is
# never held again, so no fire, correction or cancel acts on it twice.
law route_one_outcome:
  for +r: HXH.Route
  for s: HXH.RSt
  for e: {HXB.is_held(s) == False{} : Bool}
  {HXB.held.of(Some{HXH.route_op(HXH.REnd{s}, r)}) == None{} : Maybe<&2, HXH.Route>}

# LAW: a correction acts only on a held route: one that went or was
# cancelled gets a notice, nothing else.
law route_correct_only_held:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +rid: Nat
  for +to: String
  for +name: String
  {HC.route_correct.go(o, None{}, cx, rid, to, name) == HC.route_gone(o, cx, rid) : HX.Cx<o>}

# LAW: a correction changes the project only: its due time stays (his 2 s
# don't start again).
law route_correct_keeps_due:
  for +r: HXH.Route
  for +to: String
  for +name: String
  {HXH.Route.due(HXH.route_op(HXH.RTo{to, name}, r)) == HXH.Route.due(r) : Nat}

# LAW: a held route's act is due HOLD_MS (2 s) after the hold, so with
# act_due_only it fires at that tick or later, never before.
law route_fires_at_due_only:
  for +x: HXH.Xs
  for +tm: HTM.Tms
  for +text: String
  for +via: String
  for +to: String
  for +name: String
  for +why: String
  for +now: Nat
  for +context: String
  {HTM.Act.due(HXB.route_act(HXB.new_route(x, tm, text, via, to, name, why, now, context))) == Nat.add(now, 2000n) : Nat}

# LAW (architect m_8883): a fire makes no turn: his message is journaled
# (an id for the history and `--input`) by route_said, never by a send
# (which pumps main), then delivered, settled and forwarded; main reads
# it in a note at its next turn.
law route_fire_makes_no_turn:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +r: HXH.Route
  {HC.route_fire(o, Some{r}, cx) == HC.route_fire.sent(o, HC.route_said(o, cx, r), r, HM.St.next_msg(HC.st(o, cx))) : HX.Cx<o>}

# ---- unclear routes (desktop S2 step 5, architect m_9321: his words the
# guess can't place wait PICK_MS for BISE_ROUTE_MODEL's pick in sb-core's
# hold, never in the shell) ----

# LAW: an unclear route (held, no target) whose act fires goes plain to
# bise's main (the cancel path), never to a project's outbox.
law route_unclear_goes_plain:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +r: HXH.Route
  for e: {HXB.is_unclear(r) == True{} : Bool}
  {HC.route_go(o, Some{r}, cx) == HC.route_plain(o, cx, r) : HX.Cx<o>}

# LAW: an unclear route's act is due PICK_MS (1.5 s) after the hold: the
# model's time, after which his words stay with bise.
law route_unclear_waits_pick_ms:
  for +x: HXH.Xs
  for +tm: HTM.Tms
  for +text: String
  for +via: String
  for +now: Nat
  for +context: String
  {HTM.Act.due(HXB.route_act(HXB.new_route_at(x, tm, text, via, "", "", "", Nat.add(now, HXB.PICK_MS()), context))) == Nat.add(now, 1500n) : Nat}

# LAW: a pick acts only on a route still held: one that went (its due
# passed, plain to main) or got a target changes nothing.
law route_pick_only_unclear:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +to: String
  for +name: String
  for +why: String
  {HC.route_pick.go(o, None{}, cx, to, name, why) == cx : HX.Cx<o>}

# LAW: a plain input to bise's main while an unclear route of his waits is
# held behind it (no send now), due at the latest unclear due ...
law route_plain_waits_behind_unclear:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +d: Nat
  for +to: String
  for +text: String
  for +queued: Bool
  {HC.user_input.behind(o, Some{d}, cx, to, text, queued) == HC.route_behind(o, cx, text, d) : HX.Cx<o>}

# LAW: ... which is no earlier than any unclear route's due, so the
# unclear one fires first (acts_due keeps the acts' order) and both reach
# main in order.
law route_behind_is_latest:
  for +r: HXH.Route
  for +t: List<&2, HXH.Route>
  for e: {HXB.is_unclear(r) == True{} : Bool}
  {Nat.is_ge(Maybe.default(&2, Nat, HXB.unclear_due(r <> t), 0n), HXH.Route.due(r)) == True{} : Bool}

# ---- followed jobs (desktop S10, core.bend's `followed jobs`) ----

# LAW: before its end a followed task's progress makes no job_end: a
# progress report changes nothing but the report itself.
law follow_quiet_before_end:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +from: String
  for +summary: String
  {HC.req_report.kind(o, cx, from, "progress", summary) == cx : HX.Cx<o>}

# LAW: its done ends the job in the same step (job_end when followed).
law follow_done_at_its_report:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +from: String
  for +summary: String
  {HC.req_report.kind(o, cx, from, "done", summary) == HC.job_end(o, HC.report_done(o, cx, from), from, "done", summary) : HX.Cx<o>}

# LAW: a failure ends it at once, in the same step as the failure.
law follow_failed_at_once:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +from: String
  for +summary: String
  {HC.req_report.kind(o, cx, from, "failed", summary) == HC.job_end(o, HC.report_failed(o, cx, from, summary), from, "failed", summary) : HX.Cx<o>}

# LAW: an agent not followed has no job_end (so a second end of the same
# job, after the first turned follow off, says nothing).
law follow_done_once:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +name: String
  for +state: String
  for +summary: String
  for +label: String
  for +hub: String
  {HC.job_end.go(o, False{}, cx, name, state, summary, label, hub) == cx : HX.Cx<o>}

# LAW (architect m_9154): one job per follow: job_end turns follow off.
law follow_one_shot:
  for +a: HM.Agent
  {HM.Agent.follow(HM.agent_op(a, HM.OFollow{None{}})) == False{} : Bool}

# LAW (J, architect m_10223): the hub that asked for a follow is kept
# with it (journaled by EFollow), so its end can go back there.
law follow_hub_journaled:
  for +a: HM.Agent
  for +hub: String
  {HM.Agent.follow_hub(HM.agent_op(a, HM.OFollow{Some{hub}})) == hub : String}

# LAW (J): the job's end clears it with the follow: a later end of the
# same agent goes back nowhere.
law follow_hub_cleared:
  for +a: HM.Agent
  {HM.Agent.follow_hub(HM.agent_op(a, HM.OFollow{None{}})) == "" : String}

# LAW (J): an end goes back only to a hub that asked: a follow of this
# hub's own main or window ("") puts nothing in the outbox.
law follow_end_back_only_to_who_asked:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +hub: String
  for +j: J.Json
  {HC.job_end.back(o, True{}, cx, hub, j) == cx : HX.Cx<o>}

# LAW (one clock): an act fires once: after its act_done, it is gone.
law act_fires_once:
  for +tm: HTM.Tms
  for +id: Nat
  {HTM.act_find(HTM.Tms.acts(HTM.act_done(tm, id)), id) == None{} : Maybe<&2, HTM.Act>}

def all_due(xs: List<&2, HTM.Act>, +now: Nat) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Nat.is_le(HTM.Act.due(h), now) && all_due(t, now)

# LAW (one clock): a tick fires only acts due by its now.
law act_due_only:
  for +xs: List<&2, HTM.Act>
  for +now: Nat
  {all_due(HTM.acts_due(xs, now), now) == True{} : Bool}

# LAW (old-core replay, architect m_8524 c.): a journal line of a kind
# this sb-core doesn't know decodes to nothing, so it is skipped, never
# misread; the script test replays a fixture journal with the base's
# sb-core for the other direction.
law unknown_kind_skipped:
  for +j: J.Json
  for +n: String
  {HCo.ev_of.kind(HCo.KUnknown{}, j, n) == None{} : Maybe<&2, HM.Ev>}

# LAW (L12): stopping or dropping a task, in any state where no two open
# cards share an id (every state the hub reaches: cards_unique), leaves no
# open card of it.
law stop_leaves_no_card:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for +life: HM.Life
  for +why: String
  for u: HI.UniqueCards(HX.d(o, cx))
  {HC.card_ids(HM.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), n, Nil{}) == Nil{} : List<&2, Nat>}

# LAW (L12 support): a new hub has no two open cards with one id...
law init_cards_unique:
  for +w: String
  HI.UniqueCards(HM.init(w))

# ...and any sequence of inputs keeps it so (each card takes the next
# free id).
law cards_unique:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.UniqueCards(HC.Hub.d(h))
  HI.UniqueCards(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))

# LAW (L4 support): a new hub keeps the agents' names apart...
law init_names_apart:
  for +w: String
  HI.NamesApart(HM.init(w))

# ...and any sequence of inputs keeps them apart: no name or old name of an
# agent is ever a name or old name of another one (a message sent to an
# agent can only be for it).
law names_apart:
  for +ins: List<&2, J.Json>
  for +h: HC.Hub
  for u: HI.NamesApart(HC.Hub.d(h))
  HI.NamesApart(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))))


# ---- images (docs/images.md, core/image.bend) ----

# LAW: a text splits into its text parts and its image markers, in order.
law image_split_marker:
  {Im.img_split("a <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\"> b")
    == Con{Im.IText{"a "}, Con{Im.IImg{"[Image #1]", "p.png", "image/png", "/s/x.b64"},
      Con{Im.IText{" b"}, Nil{}}}}
    : List<&2, Im.IPart>}

# LAW: a marker that does not name a .b64 file is no image.
law image_split_refuses_bad_marker:
  {Im.img_paths("<image name=\"x\" path=\"p\" mime=\"image/png\" b64=\"/etc/passwd\">")
    == Nil{} : List<&2, String>}

# LAW: OpenAI style - a user message with a marker becomes content
# parts, the Codex frame around a data-url placeholder.
law api_body_image_user_parts:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.facts0(), W.parse_wire("MODEL default
MSG user : see <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\">
END
"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"high\",\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"see \"},{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},\"@@BENDPART:o|image/png|/s/x.b64|[Image #1]@@\",{\"type\":\"text\",\"text\":\"</image>\"}]}]}"
    : String}

# LAW: Anthropic - a tool result with a marker carries image blocks.
law api_body_image_anth_tool_result:
  {Api.api_body_for(Api.Anthropic{}, "m", Api.facts0(), W.parse_wire("MODEL default
MSG user : go
MSG assistant : 
  CALL 1 run_typescript : {}
MSG tool : tool run_typescript ok: <image name=\"[Image #1]\" path=\"p.png\" mime=\"image/png\" b64=\"/s/x.b64\">
END
"))
    == "{\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"run_typescript\",\"input\":{},\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_1\",\"content\":[{\"type\":\"text\",\"text\":\"tool run_typescript ok: \"},{\"type\":\"text\",\"text\":\"<image name=[Image #1] path=\\\"p.png\\\">\"},\"@@BENDPART:a|image/png|/s/x.b64|[Image #1]@@\",{\"type\":\"text\",\"text\":\"</image>\"}],\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: only the last `cap` loaded images are kept.
law image_keep_last:
  {Im.img_keep(Con{True{}, Con{False{}, Con{True{}, Con{True{}, Nil{}}}}}, 2n)
    == Con{False{}, Con{False{}, Con{True{}, Con{True{}, Nil{}}}}}
    : List<&2, Bool>}

# LAW: a marker that is not kept becomes plain text; the others stay.
law image_defang_unkept:
  {Im.img_defang("<image name=\"a\" path=\"p\" mime=\"image/png\" b64=\"/s/1.b64\"> <image name=\"b\" path=\"q\" mime=\"image/png\" b64=\"/s/2.b64\">",
      Con{False{}, Con{True{}, Nil{}}})
    == "<image-unavailable name=\"a\" path=\"p\" mime=\"image/png\" b64=\"/s/1.b64\"> <image name=\"b\" path=\"q\" mime=\"image/png\" b64=\"/s/2.b64\">"
    : String}

# LAW: the splice puts each image's data at its placeholder (an unknown
# one becomes empty).
law image_splice_data:
  {Im.img_splice("{\"d\":\"@@BENDIMG:/s/1.b64@@\",\"e\":\"@@BENDIMG:/s/9.b64@@\"}",
      Con{Im.IL{"/s/1.b64", "QUJD"}, Nil{}})
    == "{\"d\":\"QUJD\",\"e\":\"\"}" : String}

# ---- Anthropic per model: prompt caching, thinking, max_tokens (BISE-146) ----

# LAW: prompt caching - four cache_control breakpoints, the API's
# maximum: the system block, the last tool, and the last block of each
# of the two newest messages; the older messages carry none.
law anth_cache_breakpoints:
  {Anth.api_body_anth_for("m", 0n, "", True{}, "high", W.parse_wire("MODEL default
MSG system : S
TOOL echo : Returns text.
TOOL bash : Runs.
MSG user : go
MSG assistant : x
  CALL 7 echo : hi
MSG tool : out1
END
"))
    == "{\"system\":[{\"type\":\"text\",\"text\":\"S\",\"cache_control\":{\"type\":\"ephemeral\"}}],\"model\":\"m\",\"max_tokens\":32768,\"thinking\":{\"type\":\"adaptive\",\"display\":\"summarized\"},\"output_config\":{\"effort\":\"high\"},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"x\"},{\"type\":\"tool_use\",\"id\":\"call_7\",\"name\":\"echo\",\"input\":{\"arg\":\"hi\"},\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"out1\",\"cache_control\":{\"type\":\"ephemeral\"}}]}],\"tools\":[{\"name\":\"bash\",\"description\":\"Runs.\",\"input_schema\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"},\"description\":{\"type\":\"string\",\"description\":\"One short line shown to the user while this runs: what you are doing, in the user's language. 8 words at most, starting lowercase, no final period. E.g. 'weighing the hero image', 'je lance les tests du signup'.\"}},\"required\":[\"arg\"]}},{\"name\":\"echo\",\"description\":\"Returns text.\",\"input_schema\":{\"type\":\"object\",\"properties\":{\"arg\":{\"type\":\"string\"}},\"required\":[\"arg\"]},\"cache_control\":{\"type\":\"ephemeral\"}}],\"tool_choice\":{\"type\":\"auto\"}}"
    : String}

# LAW: a thinking block never carries a breakpoint (the API refuses
# it): a newest message ending in one gets none. And `thinking = none`
# sends no thinking field, even for a model that reasons; max_tokens is
# the catalog's max output.
law anth_thinking_none_and_no_cache_on_thinking:
  {Anth.api_body_anth_for("m", 64000n, "none", True{}, "high", W.parse_wire("MODEL default
MSG user : a
MSG assistant : <think>t\\nBENDSIG::s1</think>
MSG user : b
END
"))
    == "{\"model\":\"m\",\"max_tokens\":64000,\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"a\"}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"thinking\",\"thinking\":\"t\",\"signature\":\"s1\"}]},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"b\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: `thinking = budget` (Haiku 4.5 refuses adaptive thinking):
# extended thinking with budget_tokens = half the max output, at most
# 16000; no output_config.
law anth_thinking_budget:
  {Anth.api_body_anth_for("claude-haiku-4-5", 64000n, "budget", True{}, "high",
      W.parse_wire("MODEL default
MSG user : salut
END
"))
    == "{\"model\":\"claude-haiku-4-5\",\"max_tokens\":64000,\"thinking\":{\"type\":\"enabled\",\"budget_tokens\":16000},\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"salut\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

# LAW: a budget under the API's 1024 minimum: no thinking at all.
law anth_thinking_budget_too_small:
  {String.contains(Anth.api_body_anth_for("m", 1500n, "budget", True{}, "high",
      W.parse_wire("MODEL default
MSG user : salut
END
")), "thinking")
    == False{} : Bool}

# LAW: no thinking word: a model that does not reason thinks not; one
# that reasons gets adaptive thinking (the foundry body).
law anth_thinking_default_follows_reasoning:
  {Anth.api_body_anth_for("m", 8192n, "", False{}, "", W.parse_wire("MODEL default
MSG user : salut
END
"))
    == "{\"model\":\"m\",\"max_tokens\":8192,\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"salut\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"
    : String}

law anth_thinking_default_reasons_adaptive:
  {Anth.thinking_mode("", True{}) == Anth.TAdaptive{} : Anth.TMode}

law anth_thinking_words:
  {[Anth.thinking_mode("adaptive", False{}), Anth.thinking_mode("budget", False{}),
      Anth.thinking_mode("none", True{}), Anth.thinking_mode("high", False{})]
    == [Anth.TAdaptive{}, Anth.TBudget{}, Anth.TNone{}, Anth.TNone{}]
    : List<&2, Anth.TMode>}

# LAW: a model's catalog `betas` is the anthropic-beta header (the
# direct API: no 1M-context flag); unset, the foundry proxy's list
# (family_headers_anthropic).
law family_headers_anthropic_betas:
  {Api.headers(Api.Anthropic{}, "interleaved-thinking-2025-05-14", "k")
    == [Api.Hdr{"x-api-key", "k"},
      Api.Hdr{"anthropic-version", "2023-06-01"},
      Api.Hdr{"anthropic-beta", "interleaved-thinking-2025-05-14"},
      Api.Hdr{"Content-Type", "application/json"}] : List<&2, Api.Hdr>}

# LAW: the Anthropic family's body follows the model's catalog facts
# (the dispatch passes max output, thinking word and reasoning through).
law api_body_anth_follows_facts:
  {Api.api_body_for(Api.Anthropic{}, "m",
      Api.MFacts{False{}, 200n, "max_tokens", "", "", "", False{}},
      W.parse_wire("MODEL default
MSG user : salut
END
"))
    == Anth.api_body_anth_for("m", 200n, "", False{}, "", W.parse_wire("MODEL default
MSG user : salut
END
")) : String}

# ---- the model and the effort in use, and the session's choice (BISE-135) ----
# /model and /reasoning write the session's choice file
# (BISE_SESSION_CHOICE); Pv.resolve_chosen(choice, BISE_MODEL,
# BEND_MODEL, BISE_AGENT_MODEL, BISE_ROLE, config.toml, models file,
# BEND_PROVIDER_URL). The same order as rust/catalog's Setup::in_use.

# LAW: the session's model comes first, over every env var and config
# key, for main and for a sub-agent; an alias of the choice resolves.
law choice_model_first:
  {(Pv.shown_model(Pv.resolve_chosen("model = \"mistral/s\"", "mistral/a",
        "mistral/b", "mistral/e", "main", "model = \"mistral/c\"", "", "")),
      Pv.shown_model(Pv.resolve_chosen("model = \"mistral/s\"", "mistral/a",
        "", "mistral/e", "agent", "agent_model = \"mistral/d\"", "", "")),
      Pv.shown_model(Pv.resolve_chosen("model = \"opus-5.5\"", "mistral/a",
        "", "", "main", "", "", "")))
    == ("mistral/s", "mistral/s", "foundry/claude-opus-5-5")
    : String & String & String}

# LAW: no choice (or a choice of the effort alone): the role's order,
# as before.
law choice_empty_keeps_the_role_order:
  {(Pv.shown_model(Pv.resolve_chosen("", "", "", "mistral/e", "agent",
        "model = \"mistral/c\"", "", "")),
      Pv.shown_model(Pv.resolve_chosen("reasoning_effort = \"low\"", "", "",
        "", "main", "model = \"mistral/c\"", "", "")))
    == ("mistral/e", "mistral/c") : String & String}

# LAW: the effort order: the session's choice > agent_reasoning_effort
# (sub-agents) > reasoning_effort (config.toml) > the model's default
# (high, today's).
law effort_order:
  {(Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"max\"", "", "",
        "", "agent", "model = \"opus-5.5\"
reasoning_effort = \"low\"
agent_reasoning_effort = \"medium\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "agent",
        "model = \"opus-5.5\"
reasoning_effort = \"low\"
agent_reasoning_effort = \"medium\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "main",
        "model = \"opus-5.5\"
reasoning_effort = \"low\"
agent_reasoning_effort = \"medium\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "main", "model = \"opus-5.5\"", "[providers.foundry]
base_url = \"https://foundry.example.net/anthropic/v1\"", "")))
    == ("max", "medium", "low", "high") : String & String & String & String}

# LAW: an effort the model does not take falls to its default: max is
# an Anthropic word, not a chat one; a model's own list and default
# (efforts, effort keys of the models file) win over its family's.
law effort_outside_the_list_falls_to_the_default:
  {(Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"max\"", "",
        "zai-glm-5-3", "", "", "", "", "")),
      Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"low\"", "",
        "", "", "", "model = \"work/m\"", "[providers.work]
base_url = \"http://x\"
reasoning = true
efforts = \"none,high\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("", "", "", "", "",
        "model = \"work/m\"", "[providers.work]
base_url = \"http://x\"
reasoning = true
efforts = \"minimal,low\"
effort = \"minimal\"", "")))
    == ("high", "high", "minimal") : String & String & String}

# LAW: a model that does not reason (or an Anthropic one whose thinking
# is none) has no effort: none is sent, whatever was asked.
law effort_none_without_reasoning:
  {(Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"high\"", "", "",
        "", "", "model = \"work/m\"", "[providers.work]
base_url = \"http://x\"", "")),
      Pv.shown_effort(Pv.resolve_chosen("reasoning_effort = \"high\"", "", "",
        "", "", "model = \"work/m\"", "[providers.work]
base_url = \"http://x\"
api = \"anthropic\"
reasoning = true
thinking = \"none\"", "")))
    == ("", "") : String & String}

# LAW: the window (so the compaction threshold) follows the session's
# model.
law window_follows_the_choice:
  {Pv.window_chosen("model = \"zai-glm-5-3\"", "", "", "", "", "", "")
    == 128000 : U32}

# LAW: the chat body sends the call's effort word as is.
law oai_body_sends_the_effort:
  {Api.api_body_for(Api.OpenAI{}, "m", Api.MFacts{True{}, 0n, "max_tokens", "", "", "low", False{}},
      W.parse_wire("MODEL default
MSG user : hi
END
"))
    == "{\"model\":\"m\",\"reasoning_effort\":\"low\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}" : String}

# LAW: Anthropic adaptive thinking carries the effort in
# output_config; effort none turns thinking off (no thinking, no
# output_config).
law anth_adaptive_effort:
  {(String.contains(Anth.api_body_anth_for("m", 0n, "adaptive", True{}, "low",
        W.parse_wire("MODEL default
MSG user : salut
END
")), "\"output_config\":{\"effort\":\"low\"}"),
      String.contains(Anth.api_body_anth_for("m", 0n, "adaptive", True{}, "none",
        W.parse_wire("MODEL default
MSG user : salut
END
")), "thinking"))
    == (True{}, False{}) : Bool & Bool}

# LAW: a thinking budget follows the effort: low 4000, medium 8000,
# high 16000, max half the output cap (each at most half of it).
law anth_budget_follows_the_effort:
# (shown: a Nat compared past 256n is too deep to check)
  {[Nat.show(Anth.budget_for("low", 64000n)), Nat.show(Anth.budget_for("medium", 64000n)),
      Nat.show(Anth.budget_for("high", 64000n)), Nat.show(Anth.budget_for("max", 64000n)),
      Nat.show(Anth.budget_for("", 64000n)), Nat.show(Anth.budget_for("low", 6000n))]
    == ["4000", "8000", "16000", "32000", "16000", "3000"] : List<&2, String>}

# ---- calls and results stay paired (BISE-242) ----

# LAW: whatever the history (a call cut by a restart, a lost result, a
# stray result), the guard answers every call before any other message.
law pair_calls_paired:
  for +ms: List<&2, T.Msg>
  {H.paired(H.pair_calls(ms), Nil{}) == True{} : Bool}

# LAW: so every model input the Core sends is paired: the provider never
# sees a tool call without its result right after it.
law model_input_paired:
  for +cfg: T.Cfg
  for +h: List<&2, T.Msg>
  {H.paired(S.model_input(cfg, h), Nil{}) == True{} : Bool}

# LAW: on a history that is already paired, the guard changes nothing.
law pair_calls_keeps_paired:
  for +ms: List<&2, T.Msg>
  for p: {H.paired(ms, Nil{}) == True{} : Bool}
  {H.pair_calls(ms) == ms : List<&2, T.Msg>}

# LAW: the wire guard: a call cut before its result gets a failed one,
# right after its assistant message.
law pair_wire_answers_the_cut:
  {W.pair_wire(W.WReq{Nil{}, [W.WMsg{"user", "q", Nil{}},
      W.WMsg{"assistant", "", [W.WCall{"call_7", "bash", "{}"}]},
      W.WMsg{"user", "next", Nil{}}], None{}})
    == W.WReq{Nil{}, [W.WMsg{"user", "q", Nil{}},
      W.WMsg{"assistant", "", [W.WCall{"call_7", "bash", "{}"}]},
      W.WMsg{"tool", "tool bash failed: no result: bise restarted while this ran", Nil{}},
      W.WMsg{"user", "next", Nil{}}], None{}} : W.WReq}

# ---- the prompt cache (BISE-268): the start of every request stays
# byte-identical from one call of a session to the next ----

# LAW: past the cap, the oldest images go in steps of half the cap: the
# dropped set stays the same while cap/2 new images come (a slide of
# one would change the history at the oldest image on every call).
law image_keep_stepped:
  {Im.img_keep([True{}, True{}, True{}, True{}, True{}], 4n)
    == [False{}, False{}, True{}, True{}, True{}] : List<&2, Bool>}

law image_keep_stepped_same_drop:
  {Im.img_keep([True{}, True{}, True{}, True{}, True{}, True{}], 4n)
    == [False{}, False{}, True{}, True{}, True{}, True{}] : List<&2, Bool>}

# LAW: the next step drops the next half at once (7 images, cap 4: 3 kept).
law image_keep_next_step:
  {Im.img_keep([True{}, True{}, True{}, True{}, True{}, True{}, True{}], 4n)
    == [False{}, False{}, False{}, False{}, True{}, True{}, True{}]
    : List<&2, Bool>}

# three calls of one session, then its compaction: a tool round trip,
# then a new user turn; each agent call ends with its own context
def pc_req1() -> String:
  "MODEL default\nTOOL bash : Runs.\n\nMSG system : S\nMSG user : go\n\nEND\n"

def pc_req2() -> String:
  "MODEL default\nTOOL bash : Runs.\n\nMSG system : S\nMSG user : go\nMSG assistant : x\n  CALL 7 bash : ls\nMSG tool : out1\n\nEND\n"

def pc_req3() -> String:
  "MODEL default\nTOOL bash : Runs.\n\nMSG system : S\nMSG user : go\nMSG assistant : x\n  CALL 7 bash : ls\nMSG tool : out1\nMSG assistant : done\nMSG user : next\n\nEND\n"

def pc_compact() -> String:
  "MODEL default\nCOMPACT\nTOOL bash : Runs.\n\nMSG system : S\nMSG user : go\nMSG assistant : x\n  CALL 7 bash : ls\nMSG tool : out1\nMSG assistant : done\nMSG user : next\nMSG user : Summarize.\n\nEND\n"

# a request's body as the provider gets it, without the moving cache
# marks (Anthropic's breakpoints follow the tail by design: they are not
# part of the cached prefix)
def pc_body(+st: Api.Style, +ctx: String, +req: String) -> String:
  Tx.drop_all(Api.api_body_for(st, "m", Api.facts0(),
      W.pair_wire(W.parse_wire(Rm.with_context(ctx, req)))),
    ",\"cache_control\":{\"type\":\"ephemeral\"}")

# `b1` up to its context message (found by `needle`; not found: False,
# the law is never vacuous) starts `b2`
def pc_stable.at(at: Maybe<&2, Nat>, +b1: String, +b2: String) -> Bool:
  match at:
    case None{}:
      False{}
    case Some{+n}:
      Bool.and(Nat.is_gt(n, 100n), String.starts_with(b2, String.take(b1, n)))

def pc_stable(+b1: String, +b2: String, +needle: String) -> Bool:
  pc_stable.at(Tx.find(b1, needle, 0n), b1, b2)

def pc_session(+st: Api.Style, +needle: String) -> Bool:
  Bool.and(pc_stable(pc_body(st, "board v1", pc_req1()),
      pc_body(st, "board v2", pc_req2()), needle),
    Bool.and(pc_stable(pc_body(st, "board v2", pc_req2()),
        pc_body(st, "board v3", pc_req3()), needle),
      pc_stable(pc_body(st, "board v3", pc_req3()),
        pc_body(st, "board v4", pc_compact()), needle)))

# LAW: prefix stability, OpenAI Chat family: each request of a session
# starts with the whole previous one up to its context message (the
# only thing dropped), across a tool round trip, a new user turn and
# the compaction call.
law prefix_stable_oai:
  {pc_session(Api.OpenAI{}, "{\"role\":\"user\",\"content\":\"board v") == True{} : Bool}

# LAW: the same for the Anthropic family, its cache marks aside.
law prefix_stable_anth:
  {pc_session(Api.Anthropic{}, "{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"board v") == True{} : Bool}

# LAW: the context never carries a cache breakpoint; the two newest
# history messages do (the newest writes, the one before reads).
law anth_context_not_a_breakpoint:
  {String.contains(Anth.api_body_anth_for("m", 0n, "", True{}, "high",
        W.parse_wire(Rm.with_context("board", pc_req2()))),
      "\"input\":{\"arg\":\"ls\"},\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"call_7\",\"content\":\"out1\",\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"board\"}]}],")
    == True{} : Bool}

# LAW: on the OpenAI Chat family the context is a plain user message.
law oai_context_is_user:
  {String.contains(Oai.api_body_with("m", Oai.Opts{False{}, 0n, "max_tokens", ""},
      W.parse_wire(Rm.with_context("board", pc_req1()))),
    "{\"role\":\"user\",\"content\":\"board\"}]") == True{} : Bool}

# LAW: the routing key of a session is the same on every call (its
# first two messages do not change) and says nothing but a hash.
law session_key_stable:
  {Api.session_key(W.parse_wire(pc_req3()))
    == Api.session_key(W.parse_wire(pc_req1())) : String}

law session_key_is_a_hash:
  {Api.session_key(W.parse_wire(pc_req1())) == "bise-2869019270" : String}

# LAW: the key rides as the body field the catalog names, first; no
# field named, the body is unchanged.
law cache_key_field:
  {Api.with_cache_key("prompt_cache_key", "bise-1", "{\"model\":\"m\"}")
    == "{\"prompt_cache_key\":\"bise-1\",\"model\":\"m\"}" : String}

law cache_key_field_none:
  {Api.with_cache_key("", "bise-1", "{\"model\":\"m\"}")
    == "{\"model\":\"m\"}" : String}

# LAW: or as the header the catalog names, after the family's; none
# named, the headers are unchanged.
law cache_key_header:
  {Api.with_cache_header("x-grok-conv-id", "bise-1", [Api.Hdr{"a", "b"}])
    == [Api.Hdr{"a", "b"}, Api.Hdr{"x-grok-conv-id", "bise-1"}]
    : List<&2, Api.Hdr>}

law cache_key_header_none:
  {Api.with_cache_header("", "bise-1", [Api.Hdr{"a", "b"}])
    == [Api.Hdr{"a", "b"}] : List<&2, Api.Hdr>}

# ---- a gateway's extra headers (catalog headers_env) ----
# LAW: one "Name: value" per line (Claude Code's
# ANTHROPIC_CUSTOM_HEADERS), trimmed, after the family's; a value keeps
# its own ':'; a line with no ':' or no name is skipped.
law extra_headers_lines:
  {Api.with_extra_headers("source: bise\norg-id : 2 \nurl: a:b",
      [Api.Hdr{"x-api-key", "k"}])
    == [Api.Hdr{"x-api-key", "k"}, Api.Hdr{"source", "bise"},
      Api.Hdr{"org-id", "2"}, Api.Hdr{"url", "a:b"}]
    : List<&2, Api.Hdr>}

law extra_headers_skipped:
  {Api.extra_headers("no colon\n: no name\n\n") == [] : List<&2, Api.Hdr>}

law extra_headers_none:
  {Api.with_extra_headers("", [Api.Hdr{"a", "b"}])
    == [Api.Hdr{"a", "b"}] : List<&2, Api.Hdr>}

# LAW: a key_command's key also rides as a bearer token, after the
# family's headers (Claude Code's apiKeyHelper); else nothing is added.
law key_command_bearer:
  {Api.with_bearer(True{}, "k", [Api.Hdr{"x-api-key", "k"}])
    == [Api.Hdr{"x-api-key", "k"}, Api.Hdr{"Authorization", "Bearer k"}]
    : List<&2, Api.Hdr>}

law key_command_bearer_off:
  {Api.with_bearer(False{}, "k", [Api.Hdr{"x-api-key", "k"}])
    == [Api.Hdr{"x-api-key", "k"}] : List<&2, Api.Hdr>}

# LAW: a compaction's first attempt shares the prefix (tools included,
# the COMPACT line first); its retry goes bare (no tool to call).
law compact_first_attempt_shares:
  {Rt.compact_first(Some{T.PComp{1, 1n, 1, "", True{}}}) == True{} : Bool}

law compact_retry_is_bare:
  {Rt.compact_first(Some{T.PComp{1, 2n, 2, "", True{}}}) == False{} : Bool}

# LAW (image-tag, 2026-10-02): a marker in a tool's result is an image only
# for run_typescript (its image blocks); bash output, a file read or sb
# inspect printing the user's marker is text.
law image_scope_tool_text:
  {W.img_scope("tool", "tool bash ok: x") == False{} : Bool}

law image_scope_run_typescript:
  {W.img_scope("tool", "tool run_typescript ok: x") == True{} : Bool}

# LAW: a marker value holding a newline is no marker (the wire escapes it,
# so the wire scan and the message parse must agree it is text).
law image_marker_newline_is_text:
  {Im.img_paths(String.append(Im.img_open(),
    "a\" path=\"p\nq\" mime=\"image/png\" b64=\"/s/1.b64\">"))
    == Nil{} : List<&2, String>}

# LAW: an image part the IO step could not fill becomes a text part with
# its name and reason; a filled one gets its data.
law image_part_unavailable_is_text:
  {Im.body_fill(Im.body_segs("[{\"a\":1},\"@@BENDPART:a|image/png|/s/1.b64|[Image #1]@@\"]"),
    Con{Im.IRBad{"file missing"}, Nil{}})
    == "[{\"a\":1},{\"type\":\"text\",\"text\":\"[image unavailable: [Image #1] (file missing)]\"}]"
    : String}

law image_part_filled:
  {Im.body_fill(Im.body_segs("[{\"a\":1},\"@@BENDPART:o|image/png|/s/1.b64|n@@\"]"),
    Con{Im.IROk{"QUJD", 4n}, Nil{}})
    == "[{\"a\":1},{\"type\":\"image_url\",\"image_url\":{\"url\":\"data:image/png;base64,QUJD\"}}]"
    : String}

# LAW: the 20 most recent good images go; an older one becomes text.
law image_part_cap:
  {Im.res_cap(Con{Im.IROk{"a", 1n}, Con{Im.IRBad{"x"}, Con{Im.IROk{"b", 1n}, Nil{}}}}, 1n)
    == Con{Im.IRBad{"older than the 1 most recent images"}, Con{Im.IRBad{"x"},
      Con{Im.IROk{"b", 1n}, Nil{}}}} : List<&2, Im.IRes>}

# LAW (big-request): the images that go fit the request's byte limit.
# Four images of 1 MiB under a 3 MiB limit: the count cap halves (20,
# 10, 5, 2) until the kept ones fit, the oldest become text that says
# why.
law image_part_fit_bytes:
  {Im.res_fit(Con{Im.IROk{"a", 1048576n}, Con{Im.IROk{"b", 1048576n},
      Con{Im.IROk{"c", 1048576n}, Con{Im.IROk{"d", 1048576n}, Nil{}}}}},
      20n, 5n, 3145728n)
    == Con{Im.IRBad{"removed to keep the request under 3 MB"},
      Con{Im.IRBad{"removed to keep the request under 3 MB"},
        Con{Im.IROk{"c", 1048576n}, Con{Im.IROk{"d", 1048576n}, Nil{}}}}}
    : List<&2, Im.IRes>}

# LAW (big-request): the count cap and the byte limit together: the ones
# over the count say so, the next ones dropped for the bytes say that, a
# bad one keeps its own reason, the newest goes.
law image_part_fit_count_then_bytes:
  {Im.res_fit(Con{Im.IROk{"a", 1048576n}, Con{Im.IRBad{"x"},
      Con{Im.IROk{"b", 1048576n}, Con{Im.IROk{"c", 1048576n},
        Con{Im.IROk{"d", 1048576n}, Nil{}}}}}}, 2n, 5n, 1572864n)
    == Con{Im.IRBad{"older than the 2 most recent images"}, Con{Im.IRBad{"x"},
      Con{Im.IRBad{"older than the 2 most recent images"},
        Con{Im.IRBad{"removed to keep the request under 1 MB"},
          Con{Im.IROk{"d", 1048576n}, Nil{}}}}}}
    : List<&2, Im.IRes>}

# LAW (big-request): under the limit nothing changes; a limit no image
# fits sends none.
law image_part_fit_under_limit:
  {Im.res_fit(Con{Im.IROk{"a", 10n}, Con{Im.IROk{"b", 10n}, Nil{}}}, 20n, 5n, 100n)
    == Con{Im.IROk{"a", 10n}, Con{Im.IROk{"b", 10n}, Nil{}}} : List<&2, Im.IRes>}

law image_part_fit_none:
  {Im.res_fit(Con{Im.IROk{"a", 10n}, Nil{}}, 20n, 50n, 40n)
    == Con{Im.IRBad{"removed to keep the request under 0 KB"}, Nil{}}
    : List<&2, Im.IRes>}

# LAW (big-request): a filled body says what it weighs: its text plus the
# images that went.
law image_fill_fit_weighs:
  {Im.fill_fit(Im.body_segs("[{\"a\":1},\"@@BENDPART:a|image/png|/s/1.b64|n@@\"]"),
      Con{Im.IROk{"QUJD", 4n}, Nil{}}, 20n, 1000n)
    == Im.IFill{"[{\"a\":1},{\"type\":\"image\",\"source\":{\"type\":\"base64\",\"media_type\":\"image/png\",\"data\":\"QUJD\"}}]",
      14n, 4n} : Im.IFill}

# LAW: a .b64 file is sent only if non-empty, a multiple of 4 bytes, base64
law image_why_checks:
  {Con{Im.img_why(0, ""), Con{Im.img_why(5, "QUJDR"), Con{Im.img_why(4, "da:a"),
    Con{Im.img_why(4, "QUJD"), Nil{}}}}}
    == Con{"empty file", Con{"invalid base64", Con{"invalid base64", Con{"", Nil{}}}}}
    : List<&2, String>}

# ---- a stopped turn is a typed fact (desktop reset, architect m_9650 D,
# m_9693): sb-core, where the interrupt is decided, writes one 'stopped'
# line in that agent's feed in the same step as fx interrupt ----

# LAW: the window's or the TUI's interrupt of a running turn emits fx
# interrupt and then exactly one 'stopped' line for that agent.
law interrupt_says_stopped_once:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for e: {HC.is_busy(HX.Rt.run(HC.rt(o, cx, n))) == True{} : Bool}
  {HC.interrupt.go(o, Some{n}, cx) == HC.line(o, HC.interrupt.fx(o, cx, n), n, "stopped", "stopped") : HX.Cx<o>}

# LAW: with no turn running, the interrupt is only a notice: no fx
# interrupt, no 'stopped' line.
law interrupt_idle_says_nothing:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +n: String
  for e: {HC.is_busy(HX.Rt.run(HC.rt(o, cx, n))) == False{} : Bool}
  {HC.interrupt.go(o, Some{n}, cx) == HC.notice(o, cx, "no turn in progress to interrupt") : HX.Cx<o>}

# LAW: an agent's `sb interrupt` of a running turn says it the same way
# (one 'stopped' line after fx interrupt), then answers ok.
law req_interrupt_says_stopped_once:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +token: Nat
  for +agent: String
  for +n: String
  for e: {HC.is_busy(HX.Rt.run(HC.rt(o, cx, n))) == True{} : Bool}
  {HC.req_interrupt(o, Some{n}, cx, token, agent) == HC.reply(o, HC.line(o, HC.interrupt.fx(o, cx, n), n, "stopped", "stopped"), token, HC.ok_body(Nil{})) : HX.Cx<o>}

# ---- what a task sent is a fact of its own thread (desktop bar A.3 and
# S16, architect m_10203): sb-core writes one 'sent' line in the sender's
# own feed for a task's message to another agent; main's feed keeps its
# 'msg' lines, a message to the user stays 'msg-you'. A card main opens
# for a task's message shows in that task's feed too. Views only ----

# LAW: a task's message to another agent writes exactly one 'sent' line
# in the task's feed: to, the message id, 1 if it waits for a reply, text.
law task_send_says_sent_once:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +m: HM.Msg
  for e: {HC.is_task_send(o, cx, HM.Msg.from(m), HM.Msg.to(m)) == True{} : Bool}
  {HC.feed_sender(o, cx, m) == HC.line_fields(o, cx, HM.Msg.from(m), "sent", [HM.Msg.to(m), HC.mid(HM.Msg.id(m)), Bool.pick(String, HM.Msg.expect(m), "1", "0"), HM.Msg.text(m)]) : HX.Cx<o>}

# LAW: main's messages, the user's, a message to the user, and a message
# from a name that isn't an agent of this hub write no 'sent' line.
law other_send_says_no_sent:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +m: HM.Msg
  for e: {HC.is_task_send(o, cx, HM.Msg.from(m), HM.Msg.to(m)) == False{} : Bool}
  {HC.feed_sender(o, cx, m) == cx : HX.Cx<o>}

# LAW: a sender this hub has no agent of ('@bise', a cross-hub
# delegation, or any unknown name) is never a task's send.
law unknown_sender_is_no_task_send:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +from: String
  for +to: String
  for e: {HC.has_agent(o, cx, from) == False{} : Bool}
  {HC.is_task_send(o, cx, from, to) == False{} : Bool}

# LAW: a card opened for a task's message also writes its 'card' line in
# that task's feed (main's own cards stay in main's feed only).
law task_card_shows_in_its_feed:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +id: Nat
  for +agent: String
  for +text: String
  for e: {HC.is_task_of(o, cx, agent) == True{} : Bool}
  {HC.card_to_asker(o, cx, id, agent, text) == HC.line(o, cx, agent, "card", HC.cat(["#", HC.ns(id), " question @", agent, " : ", text])) : HX.Cx<o>}

# LAW: a card whose agent isn't an agent of this hub (or is main) writes
# no extra line.
law other_card_no_extra_line:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +id: Nat
  for +agent: String
  for +text: String
  for e: {HC.is_task_of(o, cx, agent) == False{} : Bool}
  {HC.card_to_asker(o, cx, id, agent, text) == cx : HX.Cx<o>}

# LAW: an agent this hub doesn't have is never a task of it.
law unknown_agent_is_no_task:
  for -o: HM.St
  for +cx: HX.Cx<o>
  for +agent: String
  for e: {HC.has_agent(o, cx, agent) == False{} : Bool}
  {HC.is_task_of(o, cx, agent) == False{} : Bool}

# ---- MCP calls that fail (runtime/mcp-pure.bend) ----

# LAW: a call is tried again (fresh initialize) only when it never
# reached the tool and a new try may work: initialize got no answer or a
# 5xx, the call could not connect, or the server lost the session. Once
# tools/call may have run (no answer read, a timeout, a 5xx, an error it
# answered) it is never sent again; an auth refusal is not retried.
law mcp_retry_only_unsent:
  {Bool.and(Mc.retry(Mc.SInit{}, Mc.WNet{Pv.NConnect{61, "refused"}}),
    Bool.and(Mc.retry(Mc.SInit{}, Mc.WStatus{503, ""}),
    Bool.and(Bool.not(Mc.retry(Mc.SInit{}, Mc.WStatus{401, ""})),
    Bool.and(Mc.retry(Mc.SCall{}, Mc.WStatus{404, "unknown session"}),
    Bool.and(Mc.retry(Mc.SCall{}, Mc.WNet{Pv.NConnect{61, "refused"}}),
    Bool.and(Bool.not(Mc.retry(Mc.SCall{}, Mc.WNet{Pv.NRead{54, "reset"}})),
    Bool.and(Bool.not(Mc.retry(Mc.SCall{}, Mc.WNet{Pv.NTimeout{}})),
    Bool.and(Bool.not(Mc.retry(Mc.SCall{}, Mc.WStatus{502, ""})),
      Bool.not(Mc.retry(Mc.SCall{}, Mc.WRpc{"no such tool"})))))))))) == True{} : Bool}

# LAW: the model reads the server, the phase, the cause, what bise
# tried, and whether calling again is safe - never just "initialize
# failed".
law mcp_fail_text_says_why_and_safety:
  {Bool.and(String.eq(Mc.fail_text("web_search.web_search", "api.mistral.ai",
      Mc.SInit{}, Mc.WNet{Pv.NDns{}}, 2n),
      "mcp call failed: web_search.web_search (api.mistral.ai), initialize: cannot resolve api.mistral.ai (dns) — check your network or VPN. bise reconnected and retried 2 times. the call never reached the server: safe to call again."),
    String.eq(Mc.fail_text("x.y", "h", Mc.SCall{}, Mc.WStatus{502, "{\"message\":\"bad gateway\"}"}, 0n),
      "mcp call failed: x.y (h), tools/call: h answered HTTP 502: bad gateway. not retried: the call may have run on the server, check before calling it again (safe if the tool only reads).")) == True{} : Bool}
