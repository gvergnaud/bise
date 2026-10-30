# bend/proofs/session.bend: the proofs of bend/laws/session.bend (moved from
# PROOF.bend). Each holds by unfolding the definitions involved; the
# checker verifies the equalities.
import Base
import ../core/types.bend as T
import ../laws/session.bend as Laws

def Laws.checkpoint_roundtrip(s):
  {==}

def Laws.summary_rejects_tool_calls(t, c):
  {==}

def Laws.empty_block_malformed():
  {==}

def Laws.fallback_accepts_plain_text():
  {==}

def Laws.drop_one_empty():
  {==}

def Laws.manual_instructions_tagged():
  {==}

def Laws.model_input_leads_with_system(cfg, h):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      {==}

def Laws.compaction_request_ends_with_prompt():
  {==}

def Laws.commits_append_at_tail():
  {==}

def Laws.wire_call_roundtrip():
  {==}

def Laws.wire_reply_roundtrip():
  {==}

def Laws.wire_args_reversible():
  {==}

def Laws.wire_args_keep_backslash_n():
  {==}

def Laws.wire_args_keep_sentinel():
  {==}

def Laws.reply_encodes_multiline_args():
  {==}

def Laws.parse_reply_decodes_multiline_args():
  {==}

def Laws.wire_echo_encodes_multiline_args():
  {==}

def Laws.api_wire_decodes_args():
  {==}

def Laws.orphan_tool_result_rejected(cfg):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      {==}

def Laws.idle_interrupt_noop(cfg):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      {==}

def Laws.truncate_marks_the_cut():
  {==}

def Laws.truncate_keeps_small_messages():
  {==}

def Laws.preserve_recent_keeps_users():
  {==}

def Laws.compaction_replacement_opens_with_summary():
  {==}

def Laws.compaction_keeps_newest_within_budget():
  {==}

def Laws.compaction_stops_at_first_misfit():
  {==}

def Laws.compaction_keeps_latest_over_budget():
  {==}

def Laws.compaction_second_drops_old_kept():
  {==}

def Laws.compaction_third_drops_old_kept():
  {==}

def Laws.compaction_idle_recompaction_stable():
  {==}

def Laws.compaction_kept_within_budget():
  {==}

def Laws.compaction_prompt_carries_requests():
  {==}

def Laws.summary_message_parses_back():
  {==}

def Laws.summary_message_summary_is_the_block():
  {==}

def Laws.obs_wire_format():
  {==}

def Laws.obs_turn_done_record():
  {==}

def Laws.turn_words_keep_the_old_outcomes():
  {==}

def Laws.turn_failed_reads_an_interrupt():
  {==}

def Laws.compaction_done_carries_whole_summary():
  {==}

def Laws.implicit_say():
  {==}

def Laws.raw_words_pass_through():
  {==}

def Laws.slash_compact_maps():
  {==}

def Laws.slash_steer_is_not_a_command():
  {==}

def Laws.slash_notify_is_not_a_command():
  {==}

def Laws.slash_quit_maps():
  {==}

def Laws.unknown_slash_refused():
  {==}

def Laws.steering_lands_in_history():
  {==}

def Laws.steering_survives_interrupt():
  {==}

def Laws.steering_survives_fail():
  {==}

def Laws.size_refusal_compacts():
  {==}

def Laws.steering_arrival_order():
  {==}

def Laws.steer_path_is_per_port():
  {==}

def Laws.steer_channel_skips_blank_lines():
  {==}

def Laws.drain_emits_steering_received():
  {==}

def Laws.drain_steering_is_act_free():
  {==}

def Laws.drain_next_of_rejected_keeps_session(s):
  {==}

def Laws.queue_hold_is_act_free():
  {==}

def Laws.queue_holds_mid_turn():
  {==}

def Laws.queue_survives_turn_end():
  {==}

def Laws.queue_pops_at_idle():
  {==}

def Laws.queue_pop_removes_head():
  {==}

def Laws.queue_delivered_in_history():
  {==}

def Laws.chunk_lines_split():
  {==}

def Laws.chunk_lines_empty():
  {==}

def Laws.long_line_is_one_command():
  {==}

def Laws.chunk_lines_and_a_tail():
  {==}

def Laws.chunk_without_newline_is_a_piece():
  {==}

def Laws.cut_char_in_a_long_line():
  {==}

def Laws.interrupt_path_is_per_port():
  {==}

def Laws.race_result_wins(f):
  {==}

def Laws.race_no_flag_waits():
  {==}

def Laws.race_flag_stops_naming_who():
  {==}

def Laws.race_ticker_ends_on_close():
  {==}

def Laws.race_cancel_is_its_own():
  {==}

def Laws.prog_success_is_recorded(out, stop):
  {==}

def Laws.prog_failure_rejects_whole(out):
  {==}

def Laws.prog_denial_rejects_with_its_note():
  {==}

def Laws.prog_stop_ends_the_program(out):
  {==}

def Laws.race_cancel_path_is_per_port():
  {==}

def Laws.bash_stop_cuts_its_own_group():
  {==}

def Laws.say_multiline_unescapes():
  {==}

def Laws.ser_msg_escapes_newlines():
  {==}

def Laws.checkpoint_multiline_roundtrip():
  {==}

def Laws.interrupt_closes_waiting_calls():
  {==}

def Laws.interrupt_closes_agent_pending():
  {==}

def Laws.run_typescript_goes_out_as_a_tool():
  {==}

def Laws.run_typescript_batches_with_other_calls():
  {==}

def Laws.run_typescript_result_comes_back():
  {==}

def Laws.run_typescript_failure_comes_back():
  {==}

def Laws.run_typescript_interrupted():
  {==}

def Laws.run_typescript_runs_on_jsrt():
  {==}

def Laws.node_program_is_not_a_tool():
  {==}

def Laws.checkpoint_loads_node_program_as_run_typescript():
  {==}

def Laws.checkpoint_keeps_call_names():
  {==}

def Laws.asked_compaction_starts_in_turn():
  {==}

def Laws.asked_compaction_goes_on():
  {==}

def Laws.unasked_result_does_not_compact():
  {==}

def Laws.asked_compaction_waits_for_the_round():
  {==}

def Laws.no_compaction_after_compaction():
  {==}

def Laws.fit_keeps_the_summary():
  {==}

def Laws.fit_never_drops_the_summary():
  {==}

def Laws.only_the_compaction_writes_a_summary():
  {==}

def Laws.chained_compaction_sees_the_summary():
  {==}

def Laws.model_input_leaves_out_hollow_messages():
  {==}

def Laws.compaction_input_leaves_out_hollow_messages():
  {==}

def Laws.hollow_reply_is_a_null_iteration():
  {==}

def Laws.signed_think_reply_is_committed():
  {==}

def Laws.hollow_session_loads_and_sends():
  {==}
