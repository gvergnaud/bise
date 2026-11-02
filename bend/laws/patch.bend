# bend/laws/patch.bend: the sandbox, exec_of, the output cap, provider retries, apply_patch, BR-004.
# Open claims, each one closed by bend/proofs/patch.bend (moved from
# LAWS.bend). The gate checks every bend/proofs/*.bend: a law here
# with no proof there fails it.
import Base
import ../core/types.bend as T
import ../core/session.bend as S
import ../core/patch.bend as Pa
import ../core/api.bend as Api
import ../runtime/tools-pure.bend as Xt
import ../runtime/bash-pure.bend as Sh
import ../runtime/provider-pure.bend as Pv
import ../runtime/selftools-pure.bend as Sf
import ../runtime/gate-pure.bend as Gt

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
        Pv.retryable_err(Pv.NDns{"no address", "your network or VPN"})),
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

# LAW: no TLS library (wire.c's code 2) is not the network's fault: its
# words say what to do, with no "check your network or VPN" after them.
law attempt_tls_no_library_no_network:
  {Pv.attempt_err("api.example.com", Pv.NTls{2, "TLS needs OpenSSL 3 (libssl.so.3)"})
    == Pv.AFail{"TLS handshake with api.example.com failed (tls 2 TLS needs OpenSSL 3 (libssl.so.3))"}
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
    == "obs provider_retry :: {\"delay_ms\":1000,\"n\":1,\"of\":10,\"why\":\"provider 529 (transient)\"}"
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

# ---- BR-004: the deferred scan trusts only standalone tool acks ----
# (it reads reload only: a compaction runs inside its turn, laws/session)

# LAW: assistant prose quoting the ack never fires the deferred
# request (the model echoes tool outputs routinely).
# BR-004 helper: an assistant message that quotes the reload ack
def prose_quoting_ack() -> T.Msg:
  T.asst(String.append("the tool said '",
    String.append(Sf.RELOAD_ACK(), "' - filing it")), Nil{})

law deferred_ignores_assistant_prose:
  {Sf.deferred_of(Con{T.user("hi"), Con{prose_quoting_ack(), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: a tool output QUOTING the harness source never fires - the
# quotes and indentation around the constant break the whole-text
# and whole-line shapes.
law deferred_ignores_source_quotes:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("  \"",
          String.append(Sf.RELOAD_ACK(), "\""))), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: a tool result that merely CONTAINS the marker mid-text never
# fires - the ack must end the text or stand on its own line.
law deferred_ignores_midtext_marker:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("prefix ",
          String.append(Sf.RELOAD_ACK(), " suffix"))), Nil{}}})
    == Sf.Def{False{}, False{}} : Sf.Def}

# LAW: the real direct shape fires - the self.* tool result prefixes
# the ack with "tool X ok: ", so the ack ends the text.
law deferred_reads_prefixed_ack:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{T.tool_res(String.append("tool self.reload ok: ",
          Sf.RELOAD_ACK())), Nil{}}})
    == Sf.Def{False{}, True{}} : Sf.Def}

# LAW: the real program shape fires - the acks ride their own lines of
# the escaped tool text (a literal backslash-n line boundary).
law deferred_reads_program_acked_result:
  {Sf.deferred_of(Con{T.user("hi"),
      Con{S.tool_result_msg("run_typescript",
          String.append("prog out\n", Sf.RELOAD_ACK()), True{}),
        Nil{}}})
    == Sf.Def{False{}, True{}} : Sf.Def}
