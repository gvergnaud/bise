# bend/laws/session.bend: the session: history, wire, steering, queue, compaction, commands.
# Open claims, each one closed by bend/proofs/session.bend (moved from
# LAWS.bend). The gate checks every bend/proofs/*.bend: a law here
# with no proof there fails it.
import Base
import ../core/types.bend as T
import ../core/history.bend as H
import ../core/compaction.bend as C
import ../core/checkpoint.bend as K
import ../core/session.bend as S
import ../core/commands.bend as Cm
import ../core/wire.bend as W
import ../core/oai-chat.bend as Oai
import ../core/api.bend as Api
import ../core/obs.bend as O
import ../runtime/remote.bend as Rm
import ../runtime/main-pure.bend as Rt
import ../runtime/bash-pure.bend as Sh
import ../runtime/provider-pure.bend as Pv
import ../runtime/repl-core-pure.bend as Rp
import ../core/utf8.bend as U8
import ../vendor/json.bend as J
import ../runtime/race-pure.bend as Rc

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

# ---- a line longer than one recv (BISE-240, big-msg-perf) ----

# the socket reader between two recvs: the commands so far, the carry
# (Rp.feed) and the bytes of the char the last recv cut (core/utf8.bend)
type Rd is Data:
  Rd{lines: List<&2, String>, carry: List<&2, String>, pend: List<&2, U32>}

def rd_fed(f: List<&2, String> & List<&2, String>, +lines: List<&2, String>,
    +pend: List<&2, U32>) -> Rd:
  match f:
    case (+got, +carry):
      Rd{List.append(&2, String, lines, got), carry, pend}

def rd_dec(r: String & List<&2, U32>, +lines: List<&2, String>,
    +carry: List<&2, String>) -> Rd:
  match r:
    case (+text, +pend):
      rd_fed(Rp.feed(carry, text), lines, pend)

# one recv's bytes, decoded after the cut char's bytes, then fed after
# the unfinished line, as the REPL reads its socket
def rd_bytes(st: Rd, +bs: List<&2, U32>) -> Rd:
  match st:
    case Rd{+lines, +carry, +pend}:
      rd_dec(U8.decode(pend, bs), lines, carry)

# one chunk of text, fed after the unfinished line
def rd_text(st: Rd, +c: String) -> Rd:
  match st:
    case Rd{+lines, +carry, +pend}:
      rd_fed(Rp.feed(carry, c), lines, pend)

def rd_lines(st: Rd) -> List<&2, String>:
  match st:
    case Rd{+lines, _, _}:
      lines

def recv_go(+chunks: List<&2, List<&2, U32>>, st: Rd) -> Rd:
  match chunks:
    case Nil{}:
      st
    case Con{+bs, +rest}:
      recv_go(rest, rd_bytes(st, bs))

def text_go(+chunks: List<&2, String>, st: Rd) -> Rd:
  match chunks:
    case Nil{}:
      st
    case Con{+c, +rest}:
      text_go(rest, rd_text(st, c))

# the commands the recvs `chunks` (their bytes) carry
def recv_lines(+chunks: List<&2, List<&2, U32>>) -> List<&2, String>:
  rd_lines(recv_go(chunks, Rd{Nil{}, Nil{}, Nil{}}))

# the commands of text chunks fed one after the other
def text_lines(+chunks: List<&2, String>) -> List<&2, String>:
  rd_lines(text_go(chunks, Rd{Nil{}, Nil{}, Nil{}}))

# LAW: a line cut over three chunks is one command, run when its
# newline comes.
law long_line_is_one_command:
  {text_lines(["say ab", "cd", "ef\n"]) == ["say abcdef"]
    : List<&2, String>}

# LAW: several lines in one chunk are separate commands, blank ones
# dropped; the line the chunk leaves unfinished waits for its newline.
law chunk_lines_and_a_tail:
  {text_lines(["say a\n\nsay b\nsay c", " d\n"])
    == ["say a", "say b", "say c d"] : List<&2, String>}

# LAW: a chunk without a newline gives no command and goes on the carry
# as it is: the line's earlier pieces are not copied or split again
# (appending each recv to the line and splitting it whole cost ~318e9
# instructions for a 2 MB `say`).
law chunk_without_newline_is_a_piece:
  {Rp.feed(["cd", "say ab"], "ef") == (Nil{}, ["ef", "cd", "say ab"])
    : List<&2, String> & List<&2, String>}

# LAW: a multibyte char (é: 0xC3 0xA9) cut by the recv boundary inside
# a two-chunk line reaches the command whole.
law cut_char_in_a_long_line:
  {recv_lines([[115, 97, 121, 32, 120, 195], [169, 121, 10]])
    == [String.append("say x", SCon{Chr{233}, "y"})] : List<&2, String>}

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

# ---- a program's inner call, recorded for the re-run (main-pure) ----

# LAW: a success is recorded as its text, stop or not (a result that is
# in wins, as race_result_wins).
law prog_success_is_recorded:
  for +out: String
  for stop: Bool
  {Rt.prog_after(True{}, out, stop) == Rt.PRecord{J.JStr{out}} : Rt.ProgStep}

# LAW: a failure (a tool error, a gate denial with its note) is recorded
# whole as {error: text}: the program goes on, that call's promise
# rejects with the text (rust/jsrt's ToolError).
law prog_failure_rejects_whole:
  for +out: String
  {Rt.prog_after(False{}, out, False{})
    == Rt.PRecord{J.JObj{[Tuple{"error", J.JStr{out}}]}} : Rt.ProgStep}

law prog_denial_rejects_with_its_note:
  {Rt.prog_after(False{}, "denied by the user: not on main", False{})
    == Rt.PRecord{J.JObj{[Tuple{"error", J.JStr{"denied by the user: not on main"}}]}} : Rt.ProgStep}

# LAW: a failure with a stop in ends the program with its text.
law prog_stop_ends_the_program:
  for +out: String
  {Rt.prog_after(False{}, out, True{})
    == Rt.PEnd{String.append("program call failed: ", out)} : Rt.ProgStep}

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
  {S.apply(T.new_session(cfg), T.ToolOk{0, "out", False{}})
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

# LAW: an observation renders as exactly one record line (issue 38).
law obs_wire_format:
  {O.show_obs(T.Assistant{"bonjour"}) == "obs assistant :: {\"text\":\"bonjour\"}"
    : String}

# LAW: a turn's end is its record: how it ended and why.
law obs_turn_done_record:
  {O.show_obs(T.TurnDone{"failed", "provider 500"})
    == "obs turn_done :: {\"end\":\"failed\",\"why\":\"provider 500\"}" : String}

# LAW: the session log's turn_ended keeps the words a turn's end had
# before issue 38 (bise_session reads them on resume).
law turn_words_keep_the_old_outcomes:
  {Con{T.turn_words("completed", ""), Con{T.turn_words("interrupted", ""),
      Con{T.turn_words("interrupted", "main"), Con{T.turn_words("failed", "provider 500"), Nil{}}}}}
    == Con{"completed", Con{"interrupted", Con{"failed: interrupted by main",
      Con{"failed: provider 500", Nil{}}}}} : List<&2, String>}

# LAW: a failed call's turn: an interrupt's error ends it as interrupted
# by that one, any other as failed with its error.
law turn_failed_reads_an_interrupt:
  {Con{T.turn_failed("interrupted by main"), Con{T.turn_failed("boom"), Nil{}}}
    == Con{T.TurnDone{"interrupted", "main"}, Con{T.TurnDone{"failed", "boom"}, Nil{}}}
    : List<&2, T.Obs>}

# LAW: compaction_done carries the whole summary the model gets after
# the compaction, never cut, on one wire line: the JSON encoder escapes
# its newline, tab and quotes (rust/proto obs/tests.rs decodes this very
# line back to the same summary).
law compaction_done_carries_whole_summary:
  {O.show_obs(T.CompactionDone{"## Le user
- veut	\"ça\" en entier, sans coupe après quarante caractères"})
    == "obs compaction_done :: {\"summary\":\"## Le user\\n- veut\\t\\\"ça\\\" en entier, sans coupe après quarante caractères\"}" : String}

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
  S.res_next(S.apply(steer_flow.called(cfg), T.ToolOk{2, "out", False{}}),
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
        case T.TurnDone{_, _}:
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
  S.res_next(S.apply(steer_order.called(cfg), T.ToolOk{2, "out", False{}}),
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
    == Con{"obs steering_received :: {\"text\":\"penguins\"}", Nil{}} : List<&2, String>}

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
      T.ToolOk{2, "1", False{}}), ts_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("", Con{T.Call{2, "run_typescript", ts_args()}, Nil{}}),
      Con{T.tool_res("tool run_typescript ok: 1"), Nil{}}}}
    : List<&2, T.Msg>}

# LAW: a program that fails (a throw, a failed inner call, a bad
# program) comes back as a failed run_typescript result.
law run_typescript_failure_comes_back:
  {S.sess_history(S.res_next(S.apply(ts_flow.called(law_cfg()),
      T.ToolFail{2, "program error", False{}}), ts_flow.called(law_cfg())))
    == Con{T.user("hi"),
      Con{T.asst("", Con{T.Call{2, "run_typescript", ts_args()}, Nil{}}),
      Con{T.tool_res("tool run_typescript failed: program error"), Nil{}}}}
    : List<&2, T.Msg>}

# ---- the agent's own compaction (self.compact) goes on in the turn ----
# A program that filed self.compact comes back with ToolOk's compact
# flag. The turn compacts before its next model call and goes on from
# the summary: no turn_done, no message from anyone needed. It used to
# compact after turn_done, and the agent sat idle until someone said
# 'continue'.

def ask_flow.asked(+cfg: T.Cfg) -> T.Res:
  S.apply(ts_flow.called(cfg), T.ToolOk{2, "1", True{}})

def ask_flow.compacting(+cfg: T.Cfg) -> T.Session:
  S.res_next(ask_flow.asked(cfg), ts_flow.called(cfg))

# the compaction call is action 3 (0: the first call, 1-2: the tool round)
def ask_flow.summary() -> T.Cmd:
  T.CompletionOk{3, "<summary>the work so far; next: finish it</summary>", Nil{}}

# what an apply's actions are, in short
def act_kinds(acts: List<&2, T.Act>) -> List<&2, String>:
  match acts:
    case Nil{}:
      Nil{}
    case Con{+a, rest}:
      match a:
        case T.AgentCall{_, _}:
          Con{"agent", act_kinds(rest)}
        case T.ToolAct{_, _}:
          Con{"tool", act_kinds(rest)}
        case T.CompactionCall{_, _}:
          Con{"compact", act_kinds(rest)}

def res_kinds(r: T.Res) -> List<&2, String>:
  match r:
    case T.Accepted{_, acts, _}:
      act_kinds(acts)
    case T.Rejected{_}:
      Nil{}

# how an apply's compaction was started: "auto", "asked" or "none"
def comp_label(obs: List<&2, T.Obs>) -> String:
  match obs:
    case Nil{}:
      "none"
    case Con{+o, rest}:
      match o:
        case T.CompactionStarted{_, +auto}:
          Bool.pick(String, auto, "auto", "asked")
        case _:
          comp_label(rest)

def res_label(r: T.Res) -> String:
  match r:
    case T.Accepted{obs, _, _}:
      comp_label(obs)
    case T.Rejected{_}:
      "none"

# LAW: a program's compaction request starts a compaction right after
# its result, inside the turn (no turn_done), labelled as asked (not
# automatic).
law asked_compaction_starts_in_turn:
  {Con{big_flow.of(ask_flow.asked(law_cfg())),
    Con{res_label(ask_flow.asked(law_cfg())), res_kinds(ask_flow.asked(law_cfg()))}}
    == Con{"compact", Con{"asked", Con{"compact", Nil{}}}} : List<&2, String>}

# LAW: a compaction never ends a turn that had work left: when the
# summary comes back, the same turn makes its next model call. And one
# request is one compaction: that call is the agent's, not another
# compaction.
law asked_compaction_goes_on:
  {Con{big_flow.of(S.apply(ask_flow.compacting(law_cfg()), ask_flow.summary())),
    res_kinds(S.apply(ask_flow.compacting(law_cfg()), ask_flow.summary()))}
    == Con{"other", Con{"agent", Nil{}}} : List<&2, String>}

# LAW: without the request, the same result goes straight to the next
# model call: only the flag compacts.
law unasked_result_does_not_compact:
  {res_kinds(S.apply(ts_flow.called(law_cfg()), T.ToolOk{2, "1", False{}}))
    == Con{"agent", Nil{}} : List<&2, String>}

# a two-call round: the program (call 2) asks, bash (call 3) answers last
def ask_flow.two(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(steer_flow.started(cfg),
      T.CompletionOk{0, "", Con{T.Call{1, "run_typescript", ts_args()},
        Con{T.Call{2, "bash", "ls"}, Nil{}}}}),
    steer_flow.started(cfg))

def ask_flow.two.asked(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(ask_flow.two(cfg), T.ToolOk{2, "1", True{}}),
    ask_flow.two(cfg))

# LAW: a request filed by any call of a round holds until the round's
# last result, then compacts.
law asked_compaction_waits_for_the_round:
  {Con{big_flow.of(S.apply(ask_flow.two(law_cfg()), T.ToolOk{2, "1", True{}})),
    Con{big_flow.of(S.apply(ask_flow.two.asked(law_cfg()), T.ToolOk{3, "ok", False{}})),
      Nil{}}}
    == Con{"other", Con{"compact", Nil{}}} : List<&2, String>}

# a steering too big for the threshold even after a compaction
def big_text(+n: Nat) -> String:
  match n:
    case 0n:
      "xxxxxxxxxxxxxxxx"
    case 1n+p:
      +half = big_text(p)
      String.append(half, half)

def ask_flow.flooded(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(ask_flow.compacting(cfg), T.UserMsg{T.Steer{}, big_text(8n)}),
    ask_flow.compacting(cfg))

# LAW: never two compactions in a row. The dispatch right after a
# compaction never starts another one: an input still over the threshold
# (here a steering bigger than it) fails the turn instead of compacting
# forever.
law no_compaction_after_compaction:
  {Con{big_flow.of(S.apply(ask_flow.flooded(law_cfg()), ask_flow.summary())),
    res_kinds(S.apply(ask_flow.flooded(law_cfg()), ask_flow.summary()))}
    == Con{"fail", Nil{}} : List<&2, String>}

# ---- chained compactions keep what was done ----
# An automatic compaction's request always drops its oldest messages
# (it starts when the input passes the threshold, its budget is the
# threshold less the prompt). The summary of the last compaction is
# protected like the latest user message: the next summary is written
# with it, so a long task never loses what it did.

def fit_case.summary() -> T.Msg:
  C.summary_message("SUMMARY-ONE")

def fit_case.round() -> List<&2, T.Msg>:
  Con{T.asst("", Con{T.Call{1, "bash", "ls"}, Nil{}}),
    Con{T.tool_res(big_text(7n)), Nil{}}}

# LAW: a fit that must drop keeps the summary (and the latest user
# message) and drops the tool rounds.
law fit_keeps_the_summary:
  {C.fit_list(300n, Con{fit_case.summary(), Con{T.user("task"), fit_case.round()}})
    == C.FOk{Con{fit_case.summary(), Con{T.user("task"), Nil{}}}} : C.FitR}

# LAW: when only the summary and the latest user message are left and
# they still do not fit, the fit says too large: never a loop, never a
# silently dropped summary.
law fit_never_drops_the_summary:
  {C.fit_list(10n, Con{fit_case.summary(), Con{T.user("task"), Nil{}}})
    == C.FBig{} : C.FitR}

# LAW: the user's own message is never taken for a summary, even one
# that opens like it; the compaction's summary message is.
law only_the_compaction_writes_a_summary:
  {Bool.and(Bool.not(H.is_summary(T.user(String.append(H.SUMMARY_PREFIX(), "mine</summary>")))),
     H.is_summary(C.summary_message("theirs")))
    == True{} : Bool}

# two chained compactions in one turn: the asked one, a tool round with
# a big output, then the automatic one (action 7)
def chain.resumed(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(ask_flow.compacting(cfg), ask_flow.summary()),
    ask_flow.compacting(cfg))

def chain.called(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(chain.resumed(cfg),
      T.CompletionOk{4, "", Con{T.Call{1, "bash", "ls"}, Nil{}}}),
    chain.resumed(cfg))

def chain.second(+cfg: T.Cfg) -> T.Res:
  S.apply(chain.called(cfg), T.ToolOk{6, big_text(7n), False{}})

def msgs_have(ms: List<&2, T.Msg>, +needle: String) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case Con{+m, rest}:
      match m:
        case T.Msg{_, +text, _, _}:
          Bool.or(String.contains(text, needle), msgs_have(rest, needle))

def comp_input_has(acts: List<&2, T.Act>, +needle: String) -> Bool:
  match acts:
    case Nil{}:
      False{}
    case Con{+a, rest}:
      match a:
        case T.CompactionCall{_, +input}:
          msgs_have(input, needle)
        case _:
          comp_input_has(rest, needle)

def res_comp_has(r: T.Res, +needle: String) -> Bool:
  match r:
    case T.Accepted{_, acts, _}:
      comp_input_has(acts, needle)
    case T.Rejected{_}:
      False{}

# LAW: the second compaction of a turn is written WITH the first one's
# summary in its input.
law chained_compaction_sees_the_summary:
  {Con{res_label(chain.second(law_cfg())),
    Con{Bool.pick(String, res_comp_has(chain.second(law_cfg()), "the work so far"),
      "sees it", "lost it"), Nil{}}}
    == Con{"auto", Con{"sees it", Nil{}}} : List<&2, String>}

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

# ---- nothing hollow reaches a request (main's 400 of 2026-10-30) ----

# a history saved before the fix (or loaded from a session log): an
# assistant message of only an unsigned think span, a blank user
# message, a blank assistant message, among messages that must stay
def hollow_history() -> List<&2, T.Msg>:
  [T.user("a"),
    T.asst("<think>t
</think>", Nil{}),
    T.user("  "),
    T.asst("<think>t
BENDSIG::s1</think>", Nil{}),
    T.asst("", [T.Call{1, "bash", "ls"}]),
    T.tool_res(""),
    T.asst(" 
 ", Nil{}),
    T.user("b")]

# LAW: the request leaves out an assistant message with no call and a
# hollow text (only unsigned thinking, or blank) and a blank user
# message; a signed think span, a message with calls and its result stay.
law model_input_leaves_out_hollow_messages:
  {S.model_input(law_cfg(), hollow_history())
    == [T.sys("system prompt"), T.user("a"),
      T.asst("<think>t
BENDSIG::s1</think>", Nil{}),
      T.asst("", [T.Call{1, "bash", "ls"}]),
      T.tool_res(""),
      T.user("b")]
    : List<&2, T.Msg>}

# LAW: the compaction request leaves them out too.
law compaction_input_leaves_out_hollow_messages:
  {C.project(law_cfg(), S.sendable(hollow_history()), "")
    == C.POk{[T.sys("system prompt"), T.user("a"),
      T.asst("<think>t
BENDSIG::s1</think>", Nil{}),
      T.asst("", [T.Call{1, "bash", "ls"}]),
      T.tool_res(""),
      T.user("b"),
      T.injected_user(C.compaction_prompt(""))]}
    : C.PRes}

def hollow_flow.started(+cfg: T.Cfg) -> T.Session:
  S.res_next(S.apply(T.new_session(cfg), T.UserMsg{T.Queue{}, "go"}),
    T.new_session(cfg))

def res_obs(r: T.Res) -> List<&2, T.Obs>:
  match r:
    case T.Accepted{+obs, _, _}:
      obs
    case T.Rejected{_}:
      Nil{}

def hollow_flow.res(+cfg: T.Cfg) -> T.Res:
  S.apply(hollow_flow.started(cfg), T.CompletionOk{0, "<think>t
</think>", Nil{}})

# LAW: a reply of only an unsigned think span (no text, no call: main's
# reply req 38870 through a gateway) is no answer: a null iteration, the
# turn asks again, and the history keeps nothing of it.
law hollow_reply_is_a_null_iteration:
  {(List.head(&2, T.Obs, res_obs(hollow_flow.res(law_cfg()))),
      S.sess_history(S.res_next(hollow_flow.res(law_cfg()),
        hollow_flow.started(law_cfg()))))
    == (Some{T.NullIteration{}}, [T.user("go")])
    : Maybe<&2, T.Obs> & List<&2, T.Msg>}

# LAW: a reply of only a SIGNED think span has something to replay: it
# is committed as before.
law signed_think_reply_is_committed:
  {S.sess_history(S.res_next(S.apply(hollow_flow.started(law_cfg()),
      T.CompletionOk{0, "<think>t
BENDSIG::s1</think>", Nil{}}), hollow_flow.started(law_cfg())))
    == [T.user("go"), T.asst("<think>t
BENDSIG::s1</think>", Nil{})]
    : List<&2, T.Msg>}

# the session main saved on 2026-10-30, as the resume projects it:
# tests/fixtures/session/16-hollow-assistant/session.txt, byte for byte
# (its events.jsonl holds the reply of one thinking part, no signature)
def hollow_session_txt() -> String:
  "BEND-SESSION 2
TOOL bash : Run a shell command.
CFG 800000 20000 3 You are bise.
COUNT 2 2
MSG False user : Where were we?
MSG False assistant : <think>Checking where the schema lives.\\n\\n</think>
MSG False user : Are you there?
MSG False assistant : Yes: the schema is in tools.ts.
"

def hollow_loaded(m: Maybe<&2, K.Checkpoint>) -> T.Session:
  match m:
    case Some{+c}:
      K.restore(c)
    case None{}:
      T.new_session(law_cfg())

# the messages of the body of the loaded session's next request
def hollow_msgs(+st: Api.Style, +s: T.Session) -> List<&2, J.Json>:
  match s:
    case T.Sess{+cfg, +history, _, _, _, _, _, _}:
      W.arr_field_or(W.parse_json(Api.api_body_for(st, "m", Api.facts0(),
        W.parse_wire(Rm.build_request(cfg, S.model_input(cfg, history))))),
        "messages")

# LAW: that session loads, and its next request leaves the hollow
# assistant out: no empty content, the two user messages side by side
# (Anthropic and OpenAI Chat take them).
law hollow_session_loads_and_sends:
  {(hollow_msgs(Api.Anthropic{}, hollow_loaded(K.from_text(hollow_session_txt()))),
      hollow_msgs(Api.OpenAI{}, hollow_loaded(K.from_text(hollow_session_txt()))))
    == (W.arr_field_or(W.parse_json("{\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"Where were we?\"}]},{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"Are you there?\",\"cache_control\":{\"type\":\"ephemeral\"}}]},{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"Yes: the schema is in tools.ts.\",\"cache_control\":{\"type\":\"ephemeral\"}}]}]}"), "messages"),
      W.arr_field_or(W.parse_json("{\"messages\":[{\"role\":\"system\",\"content\":\"You are bise.\"},{\"role\":\"user\",\"content\":\"Where were we?\"},{\"role\":\"user\",\"content\":\"Are you there?\"},{\"role\":\"assistant\",\"content\":\"Yes: the schema is in tools.ts.\"}]}"), "messages"))
    : List<&2, J.Json> & List<&2, J.Json>}

# ---- a refused history never wedges the session (main's 400 of 2026-10-30) ----

def gw_said() -> String:
  "{\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"litellm.BadRequestError: messages: text content blocks must be non-empty\"}}"

# LAW: a 400 about the messages or a thinking signature is a refusal of
# the history; one for size, about the tools, or another status is not.
law history_refusal_reads_the_messages:
  {[W.history_refused(400, gw_said()),
      W.history_refused(400, "{\"error\":{\"message\":\"messages.3.content.0: Invalid signature in thinking block\"}}"),
      W.history_refused(400, "{\"error\":{\"message\":\"Request content length exceeded 32 MB limit for messages\"}}"),
      W.history_refused(400, "{\"error\":{\"message\":\"Invalid 'tools[0].function.name'\"}}"),
      W.history_refused(500, gw_said())]
    == [True{}, True{}, False{}, False{}, False{}] : List<&2, Bool>}

def gw_err() -> String:
  Pv.refusal("Gateway", 400, gw_said(), "")

# LAW: bise's line of it says so and carries the mark the Core reads.
law history_refusal_line:
  {gw_err()
    == "Gateway refused the conversation's history (400). Gateway said: \"litellm.BadRequestError: messages: text content blocks must be non-empty\""
    : String}

def wedge_step(+s: T.Session, +c: T.Cmd) -> T.Session:
  S.res_next(S.apply(s, c), s)

# a session whose history replays a signed think span, then a new turn
# whose call (action 1) the gateway refuses
def wedge_flow() -> T.Session:
  wedge_step(wedge_step(wedge_step(T.new_session(law_cfg()),
      T.UserMsg{T.Queue{}, "go"}),
    T.CompletionOk{0, "<think>t
BENDSIG::s1</think>
hi", Nil{}}),
    T.UserMsg{T.Queue{}, "again"})

def res_acts(r: T.Res) -> List<&2, T.Act>:
  match r:
    case T.Accepted{_, +acts, _}:
      acts
    case T.Rejected{_}:
      Nil{}

# LAW: the turn does not fail on a refused history: the call goes again
# at once, its history without the reasoning, and the user reads the
# refusal and what was dropped.
law refused_history_goes_again_without_reasoning:
  {(res_obs(S.apply(wedge_flow(), T.CompletionFail{1, gw_err()})),
      res_acts(S.apply(wedge_flow(), T.CompletionFail{1, gw_err()})))
    == ([T.CandidateDiscarded{S.heal_notice(gw_err(), 1n)}],
      [T.AgentCall{2, [T.sys("system prompt"), T.user("go"),
        T.asst("hi", Nil{}), T.user("again")]}])
    : List<&2, T.Obs> & List<&2, T.Act>}

# LAW: once only: refused again, with nothing left to drop, the turn
# fails as any refusal does (the next turn sends the healed history).
law refused_history_twice_fails_the_turn:
  {res_obs(S.apply(wedge_step(wedge_flow(), T.CompletionFail{1, gw_err()}),
      T.CompletionFail{2, gw_err()}))
    == [T.CandidateDiscarded{gw_err()}, T.TurnDone{"failed", gw_err()}]
    : List<&2, T.Obs>}

# LAW: any other refusal fails the turn at once, history untouched.
law other_refusal_fails_the_turn:
  {res_obs(S.apply(wedge_flow(), T.CompletionFail{1,
      "Gateway refused the request (400)."}))
    == [T.CandidateDiscarded{"Gateway refused the request (400)."},
      T.TurnDone{"failed", "Gateway refused the request (400)."}]
    : List<&2, T.Obs>}
