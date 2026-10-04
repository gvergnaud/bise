# PROOF.bend — proofs of the harness laws. Each holds by unfolding the
# definitions involved; the checker verifies the equalities.
import Base

import ./core/types.bend as T

import ./core/history.bend as H

import ./core/compaction.bend as C

import ./core/checkpoint.bend as K

import ./core/session.bend as S

import ./vendor/json.bend as J

import ./hub/model.bend as HM

import ./hub/cx.bend as HX

import ./hub/core.bend as HC

import ./hub/inv.bend as HI

import ./hub/timers.bend as HTM

import ./core/api.bend as Api

import ./LAWS.bend as Laws

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

def Laws.compaction_preview_one_line():
  {==}

def Laws.compaction_preview_unescapes():
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

def Laws.api_wire_parses():
  {==}

def Laws.api_body_maps():
  {==}

def Laws.glm_blocks_join_lines():
  {==}

def Laws.glm_thinking_surfaces():
  {==}

def Laws.api_tool_calls_map():
  {==}

def Laws.api_error_maps():
  {==}

def Laws.unclosed_summary_falls_back():
  {==}

def Laws.escape_keeps_lines_single():
  {==}

def Laws.escape_roundtrip():
  {==}

def Laws.api_bare_body_omits_tools():
  {==}

def Laws.checkpoint_text_roundtrip():
  {==}

def Laws.checkpoint_text_roundtrip_queued():
  {==}

def Laws.renumber_is_sequential():
  {==}

def Laws.renumber_keeps_length(c1, c2, base):
  match c1:
    case T.Call{i1, n1, a1}:
      match c2:
        case T.Call{i2, n2, a2}:
          {==}

def Laws.wire_echo_keeps_names():
  {==}

def Laws.result_text_keeps_names():
  {==}

def Laws.args_json_passes_structured():
  {==}

def Laws.args_json_passes_search():
  {==}

def Laws.args_json_wraps_plain():
  {==}

def Laws.call_code_extracts():
  {==}

def Laws.call_code_passthrough():
  {==}

def Laws.flat_cmd_json():
  {==}

def Laws.flat_cmd_raw():
  {==}

def Laws.call_intent_reads():
  {==}

def Laws.call_intent_none():
  {==}

def Laws.call_intent_one_line():
  {==}

def Laws.arg_of_keeps_bash_intent():
  {==}

def Laws.arg_of_bash_plain():
  {==}

def Laws.arg_of_other_drops_intent():
  {==}

def Laws.args_json_passes_bash_intent():
  {==}

def Laws.input_json_bash_intent():
  {==}

def Laws.ann_bash_intent_shows_cmd():
  {==}

def Laws.intent_anns_bash():
  {==}

def Laws.intent_anns_ts():
  {==}

def Laws.intent_anns_none():
  {==}

def Laws.intent_anns_other_tool():
  {==}

def Laws.best_match_names_only():
  {==}

def Laws.best_match_ranks_words():
  {==}

def Laws.best_match_caps_and_flags():
  {==}

def Laws.details_missing_is_loud():
  {==}

def Laws.details_show_found_and_missing():
  {==}

def Laws.details_alias_resolves():
  {==}

def Laws.flat_details_still_works():
  {==}

def Laws.best_match_connectors_filter():
  {==}

def Laws.tool_ann_format():
  {==}

def Laws.tool_res_ann_ok():
  {==}

def Laws.tool_res_ann_fail():
  {==}

def Laws.subtool_ann_format():
  {==}

def Laws.ann_flat_single_line():
  {==}

def Laws.ann_flat_caps_at_200():
  {==}

def Laws.node_err_extracts_message():
  {==}

def Laws.bg_wrapper_script():
  {==}

def Laws.bg_after_defaults():
  {==}

def Laws.bg_after_clamps_high():
  {==}

def Laws.bg_after_keeps_low():
  {==}

def Laws.exec_of_background_maps_ok():
  {==}

def Laws.cap_out_keeps_small():
  {==}

def Laws.cap_out_limit_is_20000():
  {==}

def Laws.cap_out_at_keeps_small():
  {==}

def Laws.cap_out_at_truncates():
  {==}

def Laws.retryable_status_table():
  {==}

def Laws.retryable_err_table():
  {==}

def Laws.attempt_timeout_retries():
  {==}

def Laws.attempt_tls_fails():
  {==}

def Laws.retryable_status_transient():
  {==}

def Laws.retryable_err_midstream():
  {==}

def Laws.provider_max_attempts():
  {==}

def Laws.provider_step_timeout_covers_long_replies():
  {==}

def Laws.provider_backoff_schedule():
  {==}

def Laws.provider_retry_after():
  {==}

def Laws.provider_529_retries_with_hint():
  {==}

def Laws.provider_200_error_body_retries():
  {==}

def Laws.provider_ok_passes_through():
  {==}

def Laws.provider_client_error_is_final():
  {==}

def Laws.provider_retry_line():
  {==}

def Laws.provider_exhausted_text():
  {==}

def Laws.provider_auth_error_hint():
  {==}

def Laws.attempt_connect_names_host():
  {==}

def Laws.interrupt_flag_needs_content():
  {==}

def Laws.interrupted_keeps_last_error(who, n, why):
  (String.concat(["stopped retrying (interrupted by ", who, ") after ",
    Nat.show(n), " failed attempts; last error: "]), {==})

# every case computes: the flag's text to who, then the texts
def Laws.interrupt_names_who():
  {==}

def Laws.exhausted_keeps_last_error(n, why):
  (String.concat(["provider failed after ", Nat.show(n), " attempts: "]),
    {==})

def Laws.retry_wait_slices():
  {==}

def Laws.exec_of_exit_still_fails():
  {==}

def Laws.slash_reload_maps():
  {==}

def Laws.reload_raw_word():
  {==}

def Laws.reload_allowed_when_fresh(cfg):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      {==}

def Laws.reload_refused_while_turn(cfg, text):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      match text:
        case "":
          {==}
        case _:
          {==}

def Laws.reload_allows_with_queue(cfg, text):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      match text:
        case "":
          {==}
        case _:
          {==}

def Laws.reload_allows_with_notif(cfg, text):
  match cfg:
    case T.Cfg{_, _, _, _, _}:
      match text:
        case "":
          {==}
        case _:
          {==}

def Laws.bg_dir_default_root():
  {==}

def Laws.bg_dir_custom_root():
  {==}

def Laws.side_dir_is_the_agent_run():
  {==}

def Laws.side_dir_falls_back_to_tmpdir():
  {==}

def Laws.side_dir_last_resort_tmp():
  {==}

def Laws.dir_trim_keeps_root():
  {==}

def Laws.bg_dir_is_the_agent_tmp_bg():
  {==}

def Laws.bg_dir_root_then_tmpdir():
  {==}

def Laws.bash_cleanup_cmd_quotes_the_dir():
  {==}

def Laws.bg_slot_paths():
  {==}

def Laws.bg_wrapper_window_is_a_clock():
  {==}

def Laws.bg_headline_names_window_id_dir():
  {==}

def Laws.bg_tail_cmd_concrete():
  {==}

def Laws.bg_stdin_cmd_concrete():
  {==}

def Laws.bg_status_cmd_concrete():
  {==}

def Laws.bg_kill_cmd_concrete():
  {==}

def Laws.bg_wrapper_fifo_rdrw():
  {==}

def Laws.bg_wrapper_stdin_devnull():
  {==}

def Laws.bg_wrapper_runner_waits_and_records():
  {==}

def Laws.bg_wrapper_output_is_a_file():
  {==}

def Laws.bg_wrapper_never_kills():
  {==}

def Laws.bg_wrapper_watches_memory():
  {==}

def Laws.bg_wrapper_cleans_slot():
  {==}

def Laws.bg_wrapper_handoff_marker():
  {==}

def Laws.exec_of_success_is_ok():
  {==}

def Laws.exec_of_failure_carries_code():
  {==}

def Laws.exec_of_garbage_fails_closed():
  {==}

def Laws.exec_of_no_newline_fails_closed():
  {==}

def Laws.bg_rc_guard_names_the_dir():
  {==}

def Laws.bg_wrapper_has_the_guard():
  {==}

def Laws.bg_script_hash_deterministic():
  {==}

def Laws.bg_script_hash_high_code_points():
  {==}

def Laws.bg_script_hash_distinguishes():
  {==}

def Laws.bash_script_path_per_command():
  {==}

def Laws.bash_script_path_in_side_dir():
  {==}

def Laws.sandbox_profile_none_without_the_flag():
  {==}

def Laws.sandbox_profile_closed():
  {==}

def Laws.sandbox_profile_net():
  {==}

def Laws.sh_argv_plain():
  {==}

def Laws.sh_argv_sandboxed():
  {==}

def Laws.sandbox_stopped_write():
  {==}

def Laws.sandbox_stopped_network():
  {==}

def Laws.sandbox_stopped_not_on_success():
  {==}

def Laws.sandbox_stopped_not_on_a_test_failure():
  {==}

def Laws.self_reload_acks():
  {==}

def Laws.self_compact_acks():
  {==}

def Laws.self_tool_routing_table():
  {==}

def Laws.sleep_program_bounds():
  {==}

def Laws.sleep_program_secs():
  {==}

def Laws.self_acks_carry_markers():
  {==}

def Laws.deferred_reads_reload_request():
  {==}

def Laws.deferred_reads_compact_request():
  {==}

def Laws.deferred_reads_both_requests():
  {==}

def Laws.deferred_resets_at_user():
  {==}

def Laws.deferred_survives_injected_message():
  {==}

def Laws.deferred_resets_after_final_reply():
  {==}

def Laws.deferred_survives_tool_calls():
  {==}

def Laws.deferred_empty_history():
  {==}

def Laws.def_acks_suffix_empty_when_none():
  {==}

def Laws.def_acks_suffix_carries_reload():
  {==}

def Laws.def_acks_suffix_carries_both():
  {==}

def Laws.bg_wrapper_runner_frees_slot():
  {==}

def Laws.bg_wrapper_waits_on_runner():
  {==}

def Laws.bg_wrapper_reports_unwritable_files():
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

def Laws.exec_of_real_199_fails():
  {==}

def Laws.bg_wrapper_atomic_slot_allocation():
  {==}

def Laws.bg_wrapper_keeps_finished_results():
  {==}

def Laws.bg_wrapper_output_back_bounded():
  {==}

def Laws.bg_wrapper_unsets_repl_env():
  {==}

def Laws.bg_wrapper_output_capped():
  {==}

def Laws.bg_wrapper_runner_ownership_check():
  {==}

def Laws.bg_wrapper_cleans_its_script():
  {==}

def Laws.bg_wrapper_multiline_command_intact():
  {==}

def Laws.bg_sentinel_derives_from_command():
  {==}

def Laws.mcp_alias_normalizes():
  {==}

def Laws.mcp_alias_dedupes():
  {==}

def Laws.mcp_bootstrap_url_parity():
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

def Laws.skills_rules_contract():
  {==}

def Laws.skills_escape_xml():
  {==}

def Laws.skills_entry_format():
  {==}

def Laws.skills_catalog_dedupes():
  {==}

def Laws.skills_catalog_empty_is_empty():
  {==}

def Laws.skills_malformed_lines_skipped():
  {==}

def Laws.skills_args_name_json():
  {==}

def Laws.skills_args_name_bare():
  {==}

def Laws.skills_args_name_rejects_empty():
  {==}

def Laws.skills_miss_lists_available():
  {==}

def Laws.skills_find_exact():
  {==}

def Laws.skills_prompt_gated():
  {==}

def Laws.skills_strip_frontmatter():
  {==}

def Laws.merged_surface_carries_live_catalog():
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

def Laws.api_body_high_reasoning():
  {==}

def Laws.asst_think_roundtrip():
  {==}

def Laws.asst_think_only_no_text_block():
  {==}

def Laws.asst_plain_content_stays_plain():
  {==}

def Laws.api_body_returns_thinking_block():
  {==}

def Laws.chunk_lines_split():
  {==}

def Laws.chunk_lines_empty():
  {==}

def Laws.interrupt_path_is_per_port():
  {==}

def Laws.say_multiline_unescapes():
  {==}

def Laws.ser_msg_escapes_newlines():
  {==}

def Laws.checkpoint_multiline_roundtrip():
  {==}

def Laws.patch_parse_add():
  {==}

def Laws.patch_parse_add_empty():
  {==}

def Laws.patch_parse_delete():
  {==}

def Laws.patch_parse_update():
  {==}

def Laws.patch_parse_implicit_chunk():
  {==}

def Laws.patch_parse_multi():
  {==}

def Laws.patch_rejects_missing_end():
  {==}

def Laws.patch_rejects_bad_first():
  {==}

def Laws.patch_rejects_empty_update():
  {==}

def Laws.patch_rejects_garbage_section():
  {==}

def Laws.patch_rejects_after_end():
  {==}

def Laws.patch_tolerates_trailing_newline():
  {==}

def Laws.patch_kind_add_line():
  {==}

def Laws.patch_kind_del_line():
  {==}

def Laws.patch_kind_ctx_line():
  {==}

def Laws.patch_kind_empty_line():
  {==}

def Laws.patch_kind_begin():
  {==}

def Laws.patch_kind_end():
  {==}

def Laws.patch_kind_add_header():
  {==}

def Laws.patch_kind_delete_header():
  {==}

def Laws.patch_kind_update_header():
  {==}

def Laws.patch_kind_chunk_head():
  {==}

def Laws.patch_kind_garbage():
  {==}

def Laws.patch_apply_hit():
  {==}

def Laws.patch_apply_miss():
  {==}

def Laws.patch_apply_sequential():
  {==}

def Laws.patch_apply_sequential_miss():
  {==}

def Laws.patch_apply_insert():
  {==}

def Laws.patch_anchor_names_the_chunk():
  {==}

def Laws.patch_path_of_actions():
  {==}

def Laws.patch_kind_begin_prefix():
  {==}

def Laws.patch_kind_bare_add_header():
  {==}

def Laws.patch_kind_bare_delete_header():
  {==}

def Laws.patch_kind_bare_update_header():
  {==}

def Laws.patch_parse_glm_variant():
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

def Laws.deferred_ignores_assistant_prose():
  {==}

def Laws.deferred_ignores_source_quotes():
  {==}

def Laws.deferred_ignores_midtext_marker():
  {==}

def Laws.deferred_reads_prefixed_ack():
  {==}

def Laws.deferred_reads_program_acked_result():
  {==}

def Laws.provider_no_default_model():
  {==}

def Laws.provider_resolves_alias():
  {==}

def Laws.provider_legacy_claude_is_foundry():
  {==}

def Laws.provider_foundry_needs_a_base_url():
  {==}

def Laws.provider_foundry_names_its_url_variable():
  {==}

def Laws.provider_empty_base_url_is_none():
  {==}

def Laws.provider_legacy_glm_is_mistral():
  {==}

def Laws.provider_unlisted_model_works():
  {==}

def Laws.provider_bise_model_first():
  {==}

def Laws.provider_bend_model_over_config():
  {==}

def Laws.provider_agent_env_first():
  {==}

def Laws.provider_agent_model_for_agents():
  {==}

def Laws.provider_agent_follows_main():
  {==}

def Laws.provider_agent_model_not_for_main():
  {==}

def Laws.provider_roles_main_over_model():
  {==}

def Laws.provider_roles_agents_table():
  {==}

def Laws.effort_of_the_roles():
  {==}

def Laws.threshold_is_80_percent_of_the_window():
  {==}

def Laws.window_builtin_setups():
  {==}

def Laws.window_model_over_provider():
  {==}

def Laws.window_of_the_agent_model():
  {==}

def Laws.window_unknown_model_default():
  {==}

def Laws.threshold_precedence():
  {==}

def Laws.threshold_in_core_units():
  {==}

def Laws.threshold_tokens_or_percent():
  {==}

def Laws.threshold_capped_at_80_percent():
  {==}

def Laws.threshold_bad_values_default():
  {==}

def Laws.config_template_threshold_unset():
  {==}

def Laws.provider_from_models_file():
  {==}

def Laws.provider_model_key_wins():
  {==}

def Laws.provider_file_over_builtin():
  {==}

def Laws.provider_url_env_wins():
  {==}

def Laws.config_reads_long_file():
  {==}

def Laws.provider_unknown_is_error():
  {==}

def Laws.provider_needs_is_error():
  {==}

def Laws.provider_unknown_family_is_error():
  {==}

def Laws.family_headers_anthropic():
  {==}

def Laws.family_headers_openai():
  {==}

def Laws.family_headers_no_key():
  {==}

def Laws.family_anthropic_streams():
  {==}

def Laws.family_openai_streams():
  {==}

def Laws.family_name_roundtrip(st):
  match st:
    case Api.OpenAI{}:
      {==}
    case Api.Anthropic{}:
      {==}
    case Api.OpenAIResp{}:
      {==}

def Laws.style_routes_body():
  {==}

def Laws.style_routes_reply():
  {==}

def Laws.api_body_anth_maps():
  {==}

def Laws.api_body_anth_bare_omits_tools():
  {==}

def Laws.api_body_anth_tools_shape():
  {==}

def Laws.anth_tool_result_pairs():
  {==}

def Laws.anth_tool_results_merge():
  {==}

def Laws.oai_parallel_results_pair_in_order():
  {==}

def Laws.anth_parallel_results_pair_in_order():
  {==}

def Laws.anth_unsigned_thinking_stays_stripped():
  {==}

def Laws.anth_signed_thinking_replays():
  {==}

def Laws.anth_multi_span_replays():
  {==}

def Laws.anth_refusal_maps():
  {==}

def Laws.anth_long_tool_run_keeps_messages():
  {==}

def Laws.glm_thinking_signature_surfaces():
  {==}

def Laws.glm_signed_think_signature_replays():
  {==}

def Laws.anth_blocks_join_lines():
  {==}

def Laws.anth_thinking_surfaces():
  {==}

def Laws.anth_tool_calls_map():
  {==}

def Laws.anth_structured_call_maps():
  {==}

def Laws.anth_error_body_maps():
  {==}

def Laws.anth_status_error_maps():
  {==}

def Laws.wire_name_keeps_the_catalog():
  {==}

def Laws.wire_name_sanitizes():
  {==}

def Laws.wire_name_matches_the_pattern():
  {==}

def Laws.wire_names_round_trip():
  {==}

def Laws.openai_body_sends_wire_names():
  {==}

def Laws.anthropic_body_sends_wire_names():
  {==}

def Laws.resp_family_name_and_endpoint():
  {==}

def Laws.resp_style_of_api():
  {==}

def Laws.resp_body_maps():
  {==}

def Laws.resp_body_plain_model():
  {==}

def Laws.anthropic_drops_resp_reasoning():
  {==}

def Laws.resp_reply_maps():
  {==}

def Laws.resp_stream_reads_completed():
  {==}

def Laws.resp_stream_error():
  {==}

def Laws.resp_status_error():
  {==}

def Laws.resp_usage():
  {==}

# the ChatGPT plan route (core/oai-resp.bend, runtime/provider-pure.bend)
def Laws.resp_plan_body_maps():
  {==}

def Laws.resp_plan_streams_unstored():
  {==}

def Laws.resp_plan_no_refused_fields():
  {==}

def Laws.resp_plan_no_system_item_no_bare_tool():
  {==}

def Laws.resp_plan_namespaced_call_back():
  {==}

def Laws.resp_no_shape_items_unchanged(xs):
  {==}

def Laws.resp_no_shape_tools_unchanged(ts):
  {==}

def Laws.resp_no_shape_no_namespace():
  {==}

def Laws.plan_shape_from_models_file():
  {==}

def Laws.plan_limit_mid_stream():
  {==}

def Laws.plan_limit_429():
  {==}

def Laws.plan_usage_unavailable_mid_stream():
  {==}

def Laws.plan_usage_unavailable_503():
  {==}

def Laws.plan_usage_unavailable_exhausted():
  {==}

def Laws.plan_not_eligible():
  {==}

def Laws.plan_401_expired():
  {==}

def Laws.plan_key_command_failed():
  {==}

def Laws.plan_other_attempts_unchanged(a):
  {==}

def Laws.plan_no_shape_unchanged(s, c, a):
  {==}

def Laws.refusal_quotes_the_provider():
  {==}

def Laws.refusal_of_a_key_masks_it():
  {==}

def Laws.refusal_without_a_name_or_json():
  {==}

def Laws.refusal_without_words():
  {==}

def Laws.refusal_for_size():
  {==}

def Laws.size_refused_cases():
  {==}

def Laws.attempt_for_names_the_calls():
  {==}

def Laws.attempt_for_words_a_refusal():
  {==}

def Laws.attempt_for_keeps_a_transport_failure():
  {==}

def Laws.config_parse_flat():
  {==}

def Laws.config_skips_comments_blanks():
  {==}

def Laws.config_sections_prefix():
  {==}

def Laws.config_value_keeps_equals():
  {==}

def Laws.config_lookup_found():
  {==}

def Laws.config_lookup_missing():
  {==}

def Laws.config_u32_parses():
  {==}

def Laws.config_u32_defaults():
  {==}

def Laws.config_env_wins():
  {==}

def Laws.config_env_empty_falls_to_file():
  {==}

def Laws.config_env_empty_no_file():
  {==}

def Laws.config_template_parses():
  {==}

def Laws.config_template_threshold():
  {==}

def Laws.info_line_format():
  {==}

def Laws.info_paths_are_the_drained_ones():
  {==}

def Laws.resume_replays_history():
  {==}

def Laws.anth_stream_body():
  {==}

def Laws.anth_sse_tool_call():
  {==}

def Laws.anth_sse_matches_whole_reply():
  {==}

def Laws.anth_sse_error_event():
  {==}

def Laws.anth_sse_usage_line():
  {==}

def Laws.anth_sse_no_usage():
  {==}

def Laws.anth_sse_refusal():
  {==}

def Laws.anth_stream_json_error_body():
  {==}

def Laws.oai_stream_body():
  {==}

def Laws.oai_stream_text_split():
  {==}

def Laws.oai_stream_text_split_is():
  {==}

def Laws.oai_stream_interleaved_tools():
  {==}

def Laws.oai_stream_usage_line():
  {==}

def Laws.oai_stream_mistral_one_delta():
  {==}

def Laws.oai_stream_mistral_usage():
  {==}

def Laws.oai_stream_thinking_blocks():
  {==}

def Laws.oai_stream_error_chunk():
  {==}

def Laws.oai_stream_json_body():
  {==}

def Laws.oai_body_no_reasoning_cap():
  {==}

def Laws.oai_body_openai_cap():
  {==}

def Laws.provider_facts_from_file():
  {==}

def Laws.interrupt_flag_semantics():
  {==}

def Laws.provider_stream_step_timeout():
  {==}

def Laws.debug_event_line():
  {==}

def Laws.debug_crumbs_filter():
  {==}

def Laws.debug_plan_ok_first():
  {==}

def Laws.debug_plan_recovered():
  {==}

def Laws.debug_plan_error_reply():
  {==}

def Laws.debug_plan_retry():
  {==}

def Laws.debug_clip():
  {==}

def Laws.debug_reply_name():
  {==}

def Laws.debug_clock_anchor():
  {==}

def Laws.debug_clock_anchor_failed():
  {==}

def Laws.debug_stamp():
  {==}

def Laws.workdir_empty_keeps_command():
  {==}

def Laws.workdir_cds_first():
  {==}

def Laws.context_empty_keeps_request():
  {==}

def Laws.context_is_last_user_message():
  {==}

def Laws.context_skips_compaction():
  {==}

def Laws.extra_prompt_appends():
  {==}

def Laws.extra_prompt_empty_keeps():
  {==}

def Laws.solo_tone_without_role():
  {==}

def Laws.solo_tone_not_with_role():
  {==}

def Laws.steer_line_unescapes():
  {==}

# ---- Switchboard hub ----

# a step context's state is the replay of its journal: the proof it carries
def hub_core_ok(-o: HM.St, c: HX.Core<o>) -> {HX.Core.d(o, c) == HM.apply_all(HX.journal(HX.Core.fx(o, c)), o) : HM.St}:
  match c:
    case HX.Core{d, fx, p, f, q, v}:
      p

def Laws.hub_step_replays(h, i):
  hub_core_ok(HC.Hub.d(h), HX.Cx.c(HC.Hub.d(h), HC.step(h, i)))

def hub_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HM.St)
    -> {HM.apply_all(b, HM.apply_all(a, s)) == HM.apply_all(List.append(&2, HM.Ev, a, b), s) : HM.St}:
  match a:
    case Nil{}:
      {==}
    case Con{e, t}:
      hub_append(t, b, HM.apply(s, e))

def hub_commit_go(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> {HC.Hub.d(HC.Out.hub(HC.commit.go(HC.Hub.d(h), need, c, r, h)))
        == HM.apply_all(HX.journal(HC.Out.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)) : HM.St}:
  match need:
    case Some{q}:
      {==}
    case None{}:
      hub_core_ok(HC.Hub.d(h), c)

def hub_commit_cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> {HC.Hub.d(HC.Out.hub(HC.commit.cx(HC.Hub.d(h), cx, h)))
        == HM.apply_all(HX.journal(HC.Out.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)) : HM.St}:
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go(h, HX.R.need(r), c, r)

# what sb-core keeps of an input is the replay of its journal
def hub_commit_ok(+h: HC.Hub, +i: J.Json)
    -> {HC.Hub.d(HC.Out.hub(HC.commit(h, i))) == HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)) : HM.St}:
  hub_commit_cx(h, HC.step(h, i))

def Laws.hub_run_replays(ins, h):
  match ins:
    case Nil{}:
      {==}
    case Con{i, rest}:
      Equal.trans(HM.St,
        HC.Hub.d(HC.RunR.hub(HC.run(rest, HC.Out.hub(HC.commit(h, i))))),
        HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(HC.Out.hub(HC.commit(h, i)))),
        HM.apply_all(List.append(&2, HM.Ev, HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i))))), HC.Hub.d(h)),
        Laws.hub_run_replays(rest, HC.Out.hub(HC.commit(h, i))),
        Equal.trans(HM.St,
          HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(HC.Out.hub(HC.commit(h, i)))),
          HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h))),
          HM.apply_all(List.append(&2, HM.Ev, HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i))))), HC.Hub.d(h)),
          Equal.cong(HM.St, HM.St, x => HM.apply_all(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), x),
            HC.Hub.d(HC.Out.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)),
            hub_commit_ok(h, i)),
          hub_append(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(h))))

def hub_fresh_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HM.St, fa: HX.Fresh(a, s),
    fb: HX.Fresh(b, HM.apply_all(a, s))) -> HX.Fresh(List.append(&2, HM.Ev, a, b), s):
  match a:
    case Nil{}:
      fb
    case Con{e, t}:
      match fa:
        case HX.Both{x, y}:
          HX.Both{x, hub_fresh_append(t, b, HM.apply(s, e), y, fb)}

def hub_quiet_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, qa: HX.Quiet(a), qb: HX.Quiet(b))
    -> HX.Quiet(List.append(&2, HM.Ev, a, b)):
  match a:
    case Nil{}:
      qb
    case Con{e, t}:
      match qa:
        case HX.Both{x, y}:
          HX.Both{x, hub_quiet_append(t, b, y, qb)}

def hub_fresh_tr(-evs: List<&2, HM.Ev>, -x: HM.St, -y: HM.St, e: {x == y : HM.St}, f: HX.Fresh(evs, x)) -> HX.Fresh(evs, y):
  %e : HX.Fresh(evs, _)
  f

def hub_commit_go_f(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> HX.Both<HX.Fresh(HX.journal(HC.Out.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)),
               HX.Quiet(HX.journal(HC.Out.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))))>:
  match need:
    case Some{q}:
      HX.Both{Unit{}, Unit{}}
    case None{}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          HX.Both{f, q}

def hub_commit_f.cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> HX.Both<HX.Fresh(HX.journal(HC.Out.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)),
               HX.Quiet(HX.journal(HC.Out.fx(HC.commit.cx(HC.Hub.d(h), cx, h))))>:
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go_f(h, HX.R.need(r), c, r)

def hub_commit_f(+h: HC.Hub, +i: J.Json)
    -> HX.Both<HX.Fresh(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Out.fx(HC.commit(h, i))))>:
  hub_commit_f.cx(h, HC.step(h, i))

def hub_fst(-A: Data, -B: Data, x: HX.Both<A, B>) -> A:
  match x:
    case HX.Both{a, b}:
      a

def hub_snd(-A: Data, -B: Data, x: HX.Both<A, B>) -> B:
  match x:
    case HX.Both{a, b}:
      b

def Laws.msg_ids_fresh(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      hub_fresh_append(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(h),
        hub_fst(HX.Fresh(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Out.fx(HC.commit(h, i)))), hub_commit_f(h, i)),
        hub_fresh_tr(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))),
          HC.Hub.d(HC.Out.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)),
          hub_commit_ok(h, i), Laws.msg_ids_fresh(rest, HC.Out.hub(HC.commit(h, i)))))

def Laws.auto_never_asks(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      hub_quiet_append(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))),
        hub_snd(HX.Fresh(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)), HX.Quiet(HX.journal(HC.Out.fx(HC.commit(h, i)))), hub_commit_f(h, i)),
        Laws.auto_never_asks(rest, HC.Out.hub(HC.commit(h, i))))

def hub_set_waiters(-o: HM.St, cx: HX.Cx<o>, +ws: List<&2, HX.Waiter>)
    -> {HC.waiters_of_cx(o, HC.set_waiters(o, cx, ws)) == ws : List<&2, HX.Waiter>}:
  match cx:
    case HX.Cx{c, r}:
      match r:
        case HX.R{a, b, st, d, e, f, g, h, i, j, k, l}:
          {==}

def hub_later_tr(-x: List<&2, HX.Waiter>, -y: List<&2, HX.Waiter>, -now: Nat, e: {x == y : List<&2, HX.Waiter>},
    l: HC.Later(y, now)) -> HC.Later(x, now):
  %Equal.sym(List<&2, HX.Waiter>, x, y, e) : HC.Later(_, now)
  l

def hub_keep_later.pick(c: Bool, +w: HX.Waiter, -t: List<&2, HX.Waiter>, -now: Nat,
    +e: {Nat.is_le(HX.Waiter.deadline(w), now) == c : Bool}, ih: HC.Later(HC.waiters_keep(t, now), now))
    -> HC.Later(Bool.pick(List<&2, HX.Waiter>, Nat.is_le(HX.Waiter.deadline(w), now), HC.waiters_keep(t, now), w <> HC.waiters_keep(t, now)), now):
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_le(HX.Waiter.deadline(w), now), True{}, e) : HC.Later(Bool.pick(List<&2, HX.Waiter>, _, HC.waiters_keep(t, now), w <> HC.waiters_keep(t, now)), now)
      ih
    case False{}:
      %Equal.sym(Bool, Nat.is_le(HX.Waiter.deadline(w), now), False{}, e) : HC.Later(Bool.pick(List<&2, HX.Waiter>, _, HC.waiters_keep(t, now), w <> HC.waiters_keep(t, now)), now)
      HX.Both{e, ih}

def hub_keep_later(+ws: List<&2, HX.Waiter>, +now: Nat) -> HC.Later(HC.waiters_keep(ws, now), now):
  match ws:
    case Nil{}:
      Unit{}
    case Con{w, t}:
      hub_keep_later.pick(Nat.is_le(HX.Waiter.deadline(w), now), w, t, now, {==}, hub_keep_later(t, now))

def Laws.tick_clears_late_waiters(o, cx):
  hub_later_tr(HC.waiters_of_cx(o, HC.tick(o, cx)), HC.tick.keep(o, cx), HX.R.now(HX.Cx.r(o, cx)),
    hub_set_waiters(o, HC.tick.go(o, cx), HC.tick.keep(o, cx)),
    hub_keep_later(HX.R.waiters(HX.Cx.r(o, cx)), HX.R.now(HX.Cx.r(o, cx))))

def hub_none_tr(-x: List<&2, HX.Waiter>, -y: List<&2, HX.Waiter>, -n: String, e: {x == y : List<&2, HX.Waiter>},
    l: HC.NoneOf(y, n)) -> HC.NoneOf(x, n):
  %Equal.sym(List<&2, HX.Waiter>, x, y, e) : HC.NoneOf(_, n)
  l

def hub_not_none.pick(c: Bool, +w: HX.Waiter, -t: List<&2, HX.Waiter>, -n: String,
    +e: {String.eq(HX.Waiter.agent(w), n) == c : Bool}, ih: HC.NoneOf(HC.waiters_not(t, n), n))
    -> HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, String.eq(HX.Waiter.agent(w), n), HC.waiters_not(t, n), w <> HC.waiters_not(t, n)), n):
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HX.Waiter.agent(w), n), True{}, e) : HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, _, HC.waiters_not(t, n), w <> HC.waiters_not(t, n)), n)
      ih
    case False{}:
      %Equal.sym(Bool, String.eq(HX.Waiter.agent(w), n), False{}, e) : HC.NoneOf(Bool.pick(List<&2, HX.Waiter>, _, HC.waiters_not(t, n), w <> HC.waiters_not(t, n)), n)
      HX.Both{e, ih}

def hub_not_none(+ws: List<&2, HX.Waiter>, +n: String) -> HC.NoneOf(HC.waiters_not(ws, n), n):
  match ws:
    case Nil{}:
      Unit{}
    case Con{w, t}:
      hub_not_none.pick(String.eq(HX.Waiter.agent(w), n), w, t, n, {==}, hub_not_none(t, n))

def Laws.stop_leaves_no_waiter(o, cx, n, life, why):
  hub_none_tr(HC.waiters_of_cx(o, HC.stop_task(o, cx, n, life, why)), HC.waiters_not(HX.R.waiters(HX.Cx.r(o, cx)), n), n,
    hub_set_waiters(o, HC.timers_gone(o, life, HC.stop_task.go(o, cx, n, life, why), n), HC.waiters_not(HX.R.waiters(HX.Cx.r(o, cx)), n)),
    hub_not_none(HX.R.waiters(HX.Cx.r(o, cx)), n))

# ---- L2: each message is one record (msgs_unique) ----

def hub_bdisc(b: Bool) -> Type:
  match b:
    case True{}:
      Unit
    case False{}:
      Empty

def hub_ft_absurd(e: {False{} == True{} : Bool}) -> Empty:
  %Equal.sym(Bool, False{}, True{}, e) : hub_bdisc(_)
  Unit{}

def hub_absurd_eq(x: Empty, -A: Type, -a: A, -b: A) -> {a == b : A}:
  match x:

# y < x: x is not y
def hub_lt_neq(+y: Nat, +x: Nat, h: {Nat.is_lt(y, x) == True{} : Bool}) -> {Nat.is_eq(x, y) == False{} : Bool}:
  match y x:
    case 0n 0n:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_eq(x, y), False{})
    case 0n 1n+xp:
      {==}
    case 1n+yp 0n:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_eq(x, y), False{})
    case 1n+yp 1n+xp:
      hub_lt_neq(yp, xp, h)

# x < a and a <= b: x < b
def hub_lt_ge(+x: Nat, +a: Nat, +b: Nat, h: {Nat.is_lt(x, a) == True{} : Bool}, g: {Nat.is_ge(b, a) == True{} : Bool})
    -> {Nat.is_lt(x, b) == True{} : Bool}:
  match x a b:
    case _ 0n _:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Nat.is_lt(x, b), True{})
    case 0n 1n+ap 0n:
      hub_absurd_eq(hub_ft_absurd(g), Bool, Nat.is_lt(x, b), True{})
    case 0n 1n+ap 1n+bp:
      {==}
    case 1n+xp 1n+ap 0n:
      hub_absurd_eq(hub_ft_absurd(g), Bool, Nat.is_lt(x, b), True{})
    case 1n+xp 1n+ap 1n+bp:
      hub_lt_ge(xp, ap, bp, h, g)

def hub_lt_succ(+n: Nat) -> {Nat.is_lt(n, 1n+n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      hub_lt_succ(p)

def hub_rec_op_id(+r: HM.MRec, op: HM.MsgOp, -rest: List<&2, Nat>)
    -> {HM.Msg.id(HM.MRec.msg(HM.rec_op(r, op))) <> rest == HM.Msg.id(HM.MRec.msg(r)) <> rest : List<&2, Nat>}:
  match r:
    case HM.MRec{m, st, se}:
      match op:
        case HM.OSt{x}:
          {==}
        case HM.OSettle{}:
          {==}

def hub_ids_map.pick(c: Bool, +h: HM.MRec, -t: List<&2, HM.MRec>, -id: Nat, +op: HM.MsgOp,
    +e: {Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id) == c : Bool}, ih: {HI.ids(HM.map_msg(t, id, op)) == HI.ids(t) : List<&2, Nat>})
    -> {HI.ids(Bool.pick(List<&2, HM.MRec>, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id), HM.rec_op(h, op) <> t, h <> HM.map_msg(t, id, op)))
        == HI.ids(h <> t) : List<&2, Nat>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id), True{}, e) : {HI.ids(Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(h, op) <> t, h <> HM.map_msg(t, id, op))) == HI.ids(h <> t) : List<&2, Nat>}
      hub_rec_op_id(h, op, HI.ids(t))
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id), False{}, e) : {HI.ids(Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(h, op) <> t, h <> HM.map_msg(t, id, op))) == HI.ids(h <> t) : List<&2, Nat>}
      Equal.cong(List<&2, Nat>, List<&2, Nat>, xs => HM.Msg.id(HM.MRec.msg(h)) <> xs, HI.ids(HM.map_msg(t, id, op)), HI.ids(t), ih)

# updating one record keeps every id in place
def hub_ids_map(+xs: List<&2, HM.MRec>, +id: Nat, +op: HM.MsgOp) -> {HI.ids(HM.map_msg(xs, id, op)) == HI.ids(xs) : List<&2, Nat>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_ids_map.pick(Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), id), h, t, id, op, {==}, hub_ids_map(t, id, op))

# settling a question keeps every id in place
def hub_settle_if_ids(ok: Bool, +xs: List<&2, HM.MRec>, +id: Nat) -> {HI.ids(HM.apply.settle.if(ok, xs, id)) == HI.ids(xs) : List<&2, Nat>}:
  match ok:
    case True{}:
      hub_ids_map(xs, id, HM.OSettle{})
    case False{}:
      {==}

def hub_settle_go_ids(o: Maybe<&2, HM.MRec>, +xs: List<&2, HM.MRec>, +id: Nat, +from: String)
    -> {HI.ids(HM.apply.settle.go(o, xs, id, from)) == HI.ids(xs) : List<&2, Nat>}:
  match o:
    case None{}:
      {==}
    case Some{+r}:
      hub_settle_if_ids(String.eq(HM.Msg.to(HM.MRec.msg(r)), from) || String.eq(from, HM.USER()), xs, id)

def hub_settle_ids(r: Maybe<&2, Nat>, +xs: List<&2, HM.MRec>, +from: String)
    -> {HI.ids(HM.apply.settle(r, xs, from)) == HI.ids(xs) : List<&2, Nat>}:
  match r:
    case None{}:
      {==}
    case Some{+id}:
      hub_settle_go_ids(HM.rec_of(xs, id), xs, id, from)

def hub_ids_snoc(+xs: List<&2, HM.MRec>, +r: HM.MRec)
    -> {HI.ids(HM.snoc(HM.MRec, xs, r)) == HM.snoc(Nat, HI.ids(xs), HM.Msg.id(HM.MRec.msg(r))) : List<&2, Nat>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, Nat>, List<&2, Nat>, ys => HM.Msg.id(HM.MRec.msg(h)) <> ys,
        HI.ids(HM.snoc(HM.MRec, t, r)), HM.snoc(Nat, HI.ids(t), HM.Msg.id(HM.MRec.msg(r))), hub_ids_snoc(t, r))

def hub_tf_absurd(e: {True{} == False{} : Bool}) -> Empty:
  %e : hub_bdisc(_)
  Unit{}

def hub_or_false_l(a: Bool, -b: Bool, e: {Bool.or(a, b) == False{} : Bool}) -> {a == False{} : Bool}:
  match a:
    case False{}:
      {==}
    case True{}:
      hub_absurd_eq(hub_tf_absurd(e), Bool, True{}, False{})

def hub_or_false_r(a: Bool, -b: Bool, e: {Bool.or(a, b) == False{} : Bool}) -> {b == False{} : Bool}:
  match a:
    case False{}:
      e
    case True{}:
      hub_absurd_eq(hub_tf_absurd(e), Bool, b, False{})

def hub_or_false(a: Bool, -b: Bool, ea: {a == False{} : Bool}, eb: {b == False{} : Bool}) -> {Bool.or(a, b) == False{} : Bool}:
  %Equal.sym(Bool, a, False{}, ea) : {Bool.or(_, b) == False{} : Bool}
  eb

# y is neither in t nor x: it is not in t ++ [x]
def hub_nat_in_snoc(+t: List<&2, Nat>, +x: Nat, +y: Nat, +a: {HM.nat_in(t, y) == False{} : Bool}, b: {Nat.is_eq(x, y) == False{} : Bool})
    -> {HM.nat_in(HM.snoc(Nat, t, x), y) == False{} : Bool}:
  match t:
    case Nil{}:
      hub_or_false(Nat.is_eq(x, y), False{}, b, {==})
    case Con{h, tt}:
      hub_or_false(Nat.is_eq(h, y), HM.nat_in(HM.snoc(Nat, tt, x), y),
        hub_or_false_l(Nat.is_eq(h, y), HM.nat_in(tt, y), a),
        hub_nat_in_snoc(tt, x, y, hub_or_false_r(Nat.is_eq(h, y), HM.nat_in(tt, y), a), b))

def hub_distinct_snoc(+ns: List<&2, Nat>, +x: Nat, d: HI.Distinct(ns), bl: HI.Below(ns, x)) -> HI.Distinct(HM.snoc(Nat, ns, x)):
  match ns:
    case Nil{}:
      HX.Both{{==}, Unit{}}
    case Con{y, t}:
      match d bl:
        case HX.Both{dy, dt} HX.Both{ly, bt}:
          HX.Both{hub_nat_in_snoc(t, x, y, dy, hub_lt_neq(y, x, ly)), hub_distinct_snoc(t, x, dt, bt)}

def hub_below_mono(+ns: List<&2, Nat>, +a: Nat, +b: Nat, +g: {Nat.is_ge(b, a) == True{} : Bool}, bl: HI.Below(ns, a)) -> HI.Below(ns, b):
  match ns:
    case Nil{}:
      Unit{}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          HX.Both{hub_lt_ge(y, a, b, ly, g), hub_below_mono(t, a, b, g, bt)}

def hub_below_snoc(+ns: List<&2, Nat>, +x: Nat, +n: Nat, bl: HI.Below(ns, n), lx: {Nat.is_lt(x, n) == True{} : Bool})
    -> HI.Below(HM.snoc(Nat, ns, x), n):
  match ns:
    case Nil{}:
      HX.Both{lx, Unit{}}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          HX.Both{ly, hub_below_snoc(t, x, n, bt, lx)}

def hub_distinct_tr(-x: List<&2, Nat>, -y: List<&2, Nat>, e: {x == y : List<&2, Nat>}, d: HI.Distinct(y)) -> HI.Distinct(x):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.Distinct(_)
  d

def hub_below_tr(-x: List<&2, Nat>, -y: List<&2, Nat>, -n: Nat, e: {x == y : List<&2, Nat>}, b: HI.Below(y, n)) -> HI.Below(x, n):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.Below(_, n)
  b

def hub_below_n(-ns: List<&2, Nat>, -a: Nat, -b: Nat, e: {a == b : Nat}, bl: HI.Below(ns, b)) -> HI.Below(ns, a):
  %Equal.sym(Nat, a, b, e) : HI.Below(ns, _)
  bl

# an update of one record keeps the state unique
def hub_unique_map(+ms: List<&2, HM.MRec>, +id: Nat, +op: HM.MsgOp, -nm: Nat,
    u: HX.Both<HI.Distinct(HI.ids(ms)), HI.Below(HI.ids(ms), nm)>)
    -> HX.Both<HI.Distinct(HI.ids(HM.map_msg(ms, id, op))), HI.Below(HI.ids(HM.map_msg(ms, id, op)), nm)>:
  match u:
    case HX.Both{d, b}:
      HX.Both{hub_distinct_tr(HI.ids(HM.map_msg(ms, id, op)), HI.ids(ms), hub_ids_map(ms, id, op), d),
        hub_below_tr(HI.ids(HM.map_msg(ms, id, op)), HI.ids(ms), nm, hub_ids_map(ms, id, op), b)}

# a new message with the next free id keeps the state unique
def hub_unique_sent(+ms: List<&2, HM.MRec>, +m: HM.Msg, +nm: Nat, k: {HM.Msg.id(m) == nm : Nat},
    u: HX.Both<HI.Distinct(HI.ids(ms)), HI.Below(HI.ids(ms), nm)>)
    -> HX.Both<HI.Distinct(HI.ids(HM.apply.settle(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m)))),
               HI.Below(HI.ids(HM.apply.settle(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m))),
                 Nat.max(Nat.max(nm, 1n+HM.Msg.id(m)), 1n+HM.Msg.thread(m)))>:
  match u:
    case HX.Both{+d, +b}:
      +xs = HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}})
      +a = Nat.max(nm, 1n+HM.Msg.id(m))
      +top = Nat.max(a, 1n+HM.Msg.thread(m))
      +e = Equal.trans(List<&2, Nat>, HI.ids(HM.apply.settle(HM.Msg.reply_to(m), xs, HM.Msg.from(m))), HI.ids(xs),
        HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)),
        hub_settle_ids(HM.Msg.reply_to(m), xs, HM.Msg.from(m)), hub_ids_snoc(ms, HM.MRec{m, HM.Queued{"new"}, False{}}))
      +lt = hub_lt_ge(HM.Msg.id(m), a, top,
        hub_lt_ge(HM.Msg.id(m), 1n+HM.Msg.id(m), a, hub_lt_succ(HM.Msg.id(m)), Nat.max_ge_r(nm, 1n+HM.Msg.id(m))),
        Nat.max_ge_l(a, 1n+HM.Msg.thread(m)))
      HX.Both{hub_distinct_tr(HI.ids(HM.apply.settle(HM.Msg.reply_to(m), xs, HM.Msg.from(m))), HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)), e,
          hub_distinct_snoc(HI.ids(ms), HM.Msg.id(m), d, hub_below_n(HI.ids(ms), HM.Msg.id(m), nm, k, b))),
        hub_below_tr(HI.ids(HM.apply.settle(HM.Msg.reply_to(m), xs, HM.Msg.from(m))), HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)), top, e,
          hub_below_snoc(HI.ids(ms), HM.Msg.id(m), top,
            hub_below_mono(HI.ids(ms), a, top, Nat.max_ge_l(a, 1n+HM.Msg.thread(m)),
              hub_below_mono(HI.ids(ms), nm, a, Nat.max_ge_l(nm, 1n+HM.Msg.id(m)), b)), lt))}

# every event with its journal invariant (a fresh id for a new message)
# keeps the state unique
def hub_apply_unique(s: HM.St, e: HM.Ev, u: HI.Unique(s), k: HX.FreshEv(s, e)) -> HI.Unique(HM.apply(s, e)):
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      match e:
        case HM.ETaskCreated{a, b, c, d, f}:
          u
        case HM.ELifecycle{a, b, c}:
          u
        case HM.EWorkspace{a, b}:
          u
        case HM.ESnapshot{a, b}:
          u
        case HM.EDeclared{a, b, c}:
          u
        case HM.EReported{a, b}:
          u
        case HM.EFileTouched{a, b}:
          u
        case HM.ERenamed{a, b}:
          u
        case HM.EMsgSent{m}:
          hub_unique_sent(ms, m, nm, k, u)
        case HM.EMsgState{id, x}:
          hub_unique_map(ms, id, HM.OSt{x}, nm, u)
        case HM.ESettled{id}:
          hub_unique_map(ms, id, HM.OSettle{}, nm, u)
        case HM.ECardOpened{c}:
          u
        case HM.ECardClosed{a, b}:
          u
        case HM.EMainNote{a, b}:
          u
        case HM.ENotesFlushed{}:
          u
        case HM.ETimerSet{a, b}:
          u
        case HM.ETimerFired{a, b, c, d, f}:
          u
        case HM.ETimerRun{a, b}:
          u
        case HM.ETimerStop{a, b, c}:
          u

def hub_apply_all_unique(+evs: List<&2, HM.Ev>, +s: HM.St, u: HI.Unique(s), f: HX.Fresh(evs, s)) -> HI.Unique(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      u
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          hub_apply_all_unique(t, HM.apply(s, e), hub_apply_unique(s, e, u, k), ft)

def Laws.init_unique(w):
  HX.Both{Unit{}, Unit{}}

def hub_unique_tr(-x: HM.St, -y: HM.St, e: {x == y : HM.St}, u: HI.Unique(y)) -> HI.Unique(x):
  %Equal.sym(HM.St, x, y, e) : HI.Unique(_)
  u

def Laws.msgs_unique(ins, h, u):
  hub_unique_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h),
    hub_apply_all_unique(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))


# ---- L2: a message once sent is never removed (msgs_kept) ----

def hub_prefix_refl(+a: List<&2, Nat>) -> HI.IsPrefix(a, a):
  match a:
    case Nil{}:
      Unit{}
    case Con{x, t}:
      HX.Both{{==}, hub_prefix_refl(t)}

def hub_prefix_snoc(+a: List<&2, Nat>, +b: List<&2, Nat>, -x: Nat, p: HI.IsPrefix(a, b)) -> HI.IsPrefix(a, HM.snoc(Nat, b, x)):
  match a b:
    case Nil{} _:
      Unit{}
    case Con{y, t} Nil{}:
      match p:
    case Con{y, t} Con{z, u}:
      match p:
        case HX.Both{e, pt}:
          HX.Both{e, hub_prefix_snoc(t, u, x, pt)}

def hub_prefix_trans(+a: List<&2, Nat>, +b: List<&2, Nat>, +c: List<&2, Nat>, p: HI.IsPrefix(a, b), q: HI.IsPrefix(b, c)) -> HI.IsPrefix(a, c):
  match a b c:
    case Nil{} _ _:
      Unit{}
    case Con{x, t} Nil{} _:
      match p:
    case Con{x, t} Con{y, u} Nil{}:
      match q:
    case Con{x, t} Con{y, u} Con{z, v}:
      match p q:
        case HX.Both{e1, pt} HX.Both{e2, qt}:
          HX.Both{Equal.trans(Nat, x, y, z, e1, e2), hub_prefix_trans(t, u, v, pt, qt)}

def hub_prefix_tr(-a: List<&2, Nat>, -x: List<&2, Nat>, -y: List<&2, Nat>, e: {x == y : List<&2, Nat>}, p: HI.IsPrefix(a, y)) -> HI.IsPrefix(a, x):
  %Equal.sym(List<&2, Nat>, x, y, e) : HI.IsPrefix(a, _)
  p

# every event keeps the messages, in order (a new one comes last)
def hub_apply_kept(s: HM.St, e: HM.Ev) -> HI.Kept(s, HM.apply(s, e)):
  match s:
    case HM.St{ag, od, +ms, ca, no, nm, nc, tm}:
      match e:
        case HM.EMsgSent{+m}:
          hub_prefix_tr(HI.ids(ms),
            HI.ids(HM.apply.settle(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m))),
            HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)),
            Equal.trans(List<&2, Nat>,
              HI.ids(HM.apply.settle(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m))),
              HI.ids(HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}})), HM.snoc(Nat, HI.ids(ms), HM.Msg.id(m)),
              hub_settle_ids(HM.Msg.reply_to(m), HM.snoc(HM.MRec, ms, HM.MRec{m, HM.Queued{"new"}, False{}}), HM.Msg.from(m)),
              hub_ids_snoc(ms, HM.MRec{m, HM.Queued{"new"}, False{}})),
            hub_prefix_snoc(HI.ids(ms), HI.ids(ms), HM.Msg.id(m), hub_prefix_refl(HI.ids(ms))))
        case HM.EMsgState{+id, +x}:
          hub_prefix_tr(HI.ids(ms), HI.ids(HM.map_msg(ms, id, HM.OSt{x})), HI.ids(ms), hub_ids_map(ms, id, HM.OSt{x}), hub_prefix_refl(HI.ids(ms)))
        case HM.ESettled{+id}:
          hub_prefix_tr(HI.ids(ms), HI.ids(HM.map_msg(ms, id, HM.OSettle{})), HI.ids(ms), hub_ids_map(ms, id, HM.OSettle{}), hub_prefix_refl(HI.ids(ms)))
        case HM.ETaskCreated{a, b, c, d, f}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ELifecycle{a, b, c}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EWorkspace{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ESnapshot{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EDeclared{a, b, c}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EReported{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EFileTouched{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ERenamed{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ECardOpened{c}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ECardClosed{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.EMainNote{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ENotesFlushed{}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ETimerSet{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ETimerFired{a, b, c, d, f}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ETimerRun{a, b}:
          hub_prefix_refl(HI.ids(ms))
        case HM.ETimerStop{a, b, c}:
          hub_prefix_refl(HI.ids(ms))

def hub_apply_all_kept(+evs: List<&2, HM.Ev>, +s: HM.St) -> HI.Kept(s, HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      hub_prefix_refl(HI.ids(HM.St.msgs(s)))
    case Con{e, t}:
      hub_prefix_trans(HI.ids(HM.St.msgs(s)), HI.ids(HM.St.msgs(HM.apply(s, e))), HI.ids(HM.St.msgs(HM.apply_all(t, HM.apply(s, e)))),
        hub_apply_kept(s, e), hub_apply_all_kept(t, HM.apply(s, e)))

def hub_kept_tr(-a: HM.St, -x: HM.St, -y: HM.St, e: {x == y : HM.St}, k: HI.Kept(a, y)) -> HI.Kept(a, x):
  %Equal.sym(HM.St, x, y, e) : HI.Kept(a, _)
  k

def Laws.msgs_kept(ins, h):
  hub_kept_tr(HC.Hub.d(h), HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h), hub_apply_all_kept(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)))

# ---- L3: delivered only from the queue (delivered_once) ----

def hub_deliv_append(+a: List<&2, HM.Ev>, -b: List<&2, HM.Ev>, -s: HM.St, fa: HX.Deliv(a, s),
    fb: HX.Deliv(b, HM.apply_all(a, s))) -> HX.Deliv(List.append(&2, HM.Ev, a, b), s):
  match a:
    case Nil{}:
      fb
    case Con{e, t}:
      match fa:
        case HX.Both{x, y}:
          HX.Both{x, hub_deliv_append(t, b, HM.apply(s, e), y, fb)}

def hub_deliv_tr(-evs: List<&2, HM.Ev>, -x: HM.St, -y: HM.St, e: {x == y : HM.St}, f: HX.Deliv(evs, x)) -> HX.Deliv(evs, y):
  %e : HX.Deliv(evs, _)
  f

def hub_commit_go_v(+h: HC.Hub, need: Maybe<&2, J.Json>, c: HX.Core<HC.Hub.d(h)>, +r: HX.R)
    -> HX.Deliv(HX.journal(HC.Out.fx(HC.commit.go(HC.Hub.d(h), need, c, r, h))), HC.Hub.d(h)):
  match need:
    case Some{q}:
      Unit{}
    case None{}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          v

def hub_commit_v.cx(+h: HC.Hub, cx: HX.Cx<HC.Hub.d(h)>)
    -> HX.Deliv(HX.journal(HC.Out.fx(HC.commit.cx(HC.Hub.d(h), cx, h))), HC.Hub.d(h)):
  match cx:
    case HX.Cx{c, +r}:
      hub_commit_go_v(h, HX.R.need(r), c, r)

def Laws.delivered_once(ins, h):
  match ins:
    case Nil{}:
      Unit{}
    case Con{i, rest}:
      hub_deliv_append(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))), HC.Hub.d(h),
        hub_commit_v.cx(h, HC.step(h, i)),
        hub_deliv_tr(HC.RunR.journal(HC.run(rest, HC.Out.hub(HC.commit(h, i)))),
          HC.Hub.d(HC.Out.hub(HC.commit(h, i))), HM.apply_all(HX.journal(HC.Out.fx(HC.commit(h, i))), HC.Hub.d(h)),
          hub_commit_ok(h, i), Laws.delivered_once(rest, HC.Out.hub(HC.commit(h, i)))))

# ---- L12: a stop leaves no queued message and no card of the task ----

def hub_d_out(-o: HM.St, cx: HX.Cx<o>, -x: J.Json) -> {HX.d(o, HX.out(o, cx, x)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_with_r(-o: HM.St, cx: HX.Cx<o>, -r: HX.R) -> {HX.d(o, HX.with_r(o, cx, r)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r0}:
      {==}

def hub_d_emit(-o: HM.St, cx: HX.Cx<o>, -e: HM.Ev, -k: HX.FreshEv(HX.d(o, cx), e), -kq: HX.QuietEv(e), -kv: HX.DelivEv(HX.d(o, cx), e))
    -> {HX.d(o, HX.emit(o, cx, e, k, kq, kv)) == HM.apply(HX.d(o, cx), e) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_set_rt(-o: HM.St, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.set_rt(o, cx, x)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_if_agent(-o: HM.St, known: Bool, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.if_agent(o, known, cx, x)) == HX.d(o, cx) : HM.St}:
  match known:
    case True{}:
      hub_d_set_rt(o, cx, x)
    case False{}:
      {==}

def hub_d_upd_rt(-o: HM.St, +cx: HX.Cx<o>, +x: HX.Rt) -> {HX.d(o, HC.upd_rt(o, cx, x)) == HX.d(o, cx) : HM.St}:
  hub_d_if_agent(o, HC.has_agent(o, cx, HX.Rt.name(x)), cx, x)

def hub_d_reply_err(-o: HM.St, +cx: HX.Cx<o>, +t: Nat, +why: String) -> {HX.d(o, HC.reply_err(o, cx, t, why)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_reply_all(-o: HM.St, +ws: List<&2, HX.Waiter>, +cx: HX.Cx<o>, +why: String) -> {HX.d(o, HC.reply_all(o, ws, cx, why)) == HX.d(o, cx) : HM.St}:
  match ws:
    case Nil{}:
      {==}
    case Con{w, rest}:
      Equal.trans(HM.St, HX.d(o, HC.reply_all(o, rest, HC.reply_err(o, cx, HX.Waiter.token(w), why), why)),
        HX.d(o, HC.reply_err(o, cx, HX.Waiter.token(w), why)), HX.d(o, cx),
        hub_d_reply_all(o, rest, HC.reply_err(o, cx, HX.Waiter.token(w), why), why),
        hub_d_reply_err(o, cx, HX.Waiter.token(w), why))

def hub_d_fail_waiters(-o: HM.St, +cx: HX.Cx<o>, +a: String, +why: String) -> {HX.d(o, HC.fail_waiters(o, cx, a, why)) == HX.d(o, cx) : HM.St}:
  +ws = HX.R.waiters(HC.rr(o, cx))
  +cx2 = HC.set_r(o, cx, HX.R.with_waiters(HC.rr(o, cx), HC.waiters_not(ws, a)))
  +cx3 = HC.reply_all(o, HC.waiters_of(ws, a), cx2, why)
  Equal.trans(HM.St, HX.d(o, HC.upd_rt(o, cx3, HX.Rt.with_waiting(HC.rt(o, cx3, a), False{}))), HX.d(o, cx3), HX.d(o, cx),
    hub_d_upd_rt(o, cx3, HX.Rt.with_waiting(HC.rt(o, cx3, a), False{})),
    Equal.trans(HM.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx),
      hub_d_reply_all(o, HC.waiters_of(ws, a), cx2, why),
      hub_d_with_r(o, cx, HX.R.with_waiters(HC.rr(o, cx), HC.waiters_not(ws, a)))))

def hub_eq_refl(+n: Nat) -> {Nat.is_eq(n, n) == True{} : Bool}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      hub_eq_refl(p)

def hub_eq_sym(+a: Nat, +b: Nat) -> {Nat.is_eq(a, b) == Nat.is_eq(b, a) : Bool}:
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      {==}
    case 1n+ap 1n+bp:
      hub_eq_sym(ap, bp)

# the records after the rejections of stop_task
def hub_rej(ids: List<&2, Nat>, +xs: List<&2, HM.MRec>) -> List<&2, HM.MRec>:
  match ids:
    case Nil{}:
      xs
    case Con{+id, +t}:
      hub_rej(t, HM.map_msg(xs, id, HM.OSt{HM.Rejected{"recipient_unavailable"}}))

def hub_msgs_state(s: HM.St, +id: Nat, +x: HM.MSt) -> {HM.St.msgs(HM.apply(s, HM.EMsgState{id, x})) == HM.map_msg(HM.St.msgs(s), id, HM.OSt{x}) : List<&2, HM.MRec>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_msgs_reject(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {HM.St.msgs(HX.d(o, HC.reject_ids(o, ids, cx))) == hub_rej(ids, HM.St.msgs(HX.d(o, cx))) : List<&2, HM.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +cx2 = HC.emit(o, cx, HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}}, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, HC.reject_ids(o, t, cx2))), hub_rej(t, HM.St.msgs(HX.d(o, cx2))),
        hub_rej(id <> t, HM.St.msgs(HX.d(o, cx))),
        hub_msgs_reject(o, t, cx2),
        Equal.cong(List<&2, HM.MRec>, List<&2, HM.MRec>, xs => hub_rej(t, xs), HM.St.msgs(HX.d(o, cx2)),
          HM.map_msg(HM.St.msgs(HX.d(o, cx)), id, HM.OSt{HM.Rejected{"recipient_unavailable"}}),
          Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, cx2)), HM.St.msgs(HM.apply(HX.d(o, cx), HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}})),
            HM.map_msg(HM.St.msgs(HX.d(o, cx)), id, HM.OSt{HM.Rejected{"recipient_unavailable"}}),
            Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}}),
              hub_d_emit(o, cx, HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}}, Unit{}, Unit{}, Unit{})),
            hub_msgs_state(HX.d(o, cx), id, HM.Rejected{"recipient_unavailable"}))))

def hub_rej_skip.go(+i: Nat, -ns: List<&2, Nat>, +h: HM.MRec, +t: List<&2, HM.MRec>,
    ef: {Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), i) == False{} : Bool},
    ih: {hub_rej(ns, h <> HM.map_msg(t, i, HM.OSt{HM.Rejected{"recipient_unavailable"}})) == h <> hub_rej(ns, HM.map_msg(t, i, HM.OSt{HM.Rejected{"recipient_unavailable"}})) : List<&2, HM.MRec>})
    -> {hub_rej(i <> ns, h <> t) == h <> hub_rej(i <> ns, t) : List<&2, HM.MRec>}:
  %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), i), False{}, ef) : {hub_rej(ns, Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(h, HM.OSt{HM.Rejected{"recipient_unavailable"}}) <> t, h <> HM.map_msg(t, i, HM.OSt{HM.Rejected{"recipient_unavailable"}}))) == h <> hub_rej(i <> ns, t) : List<&2, HM.MRec>}
  ih

# a record whose id is not rejected stays in place
def hub_rej_skip(+ids: List<&2, Nat>, +h: HM.MRec, +t: List<&2, HM.MRec>, +nf: {HM.nat_in(ids, HM.Msg.id(HM.MRec.msg(h))) == False{} : Bool})
    -> {hub_rej(ids, h <> t) == h <> hub_rej(ids, t) : List<&2, HM.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{i, ns}:
      hub_rej_skip.go(i, ns, h, t,
        Equal.trans(Bool, Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), i), Nat.is_eq(i, HM.Msg.id(HM.MRec.msg(h))), False{},
          hub_eq_sym(HM.Msg.id(HM.MRec.msg(h)), i),
          hub_or_false_l(Nat.is_eq(i, HM.Msg.id(HM.MRec.msg(h))), HM.nat_in(ns, HM.Msg.id(HM.MRec.msg(h))), nf)),
        hub_rej_skip(ns, h, HM.map_msg(t, i, HM.OSt{HM.Rejected{"recipient_unavailable"}}),
          hub_or_false_r(Nat.is_eq(i, HM.Msg.id(HM.MRec.msg(h))), HM.nat_in(ns, HM.Msg.id(HM.MRec.msg(h))), nf)))

def hub_q_sub.pick(c: Bool, +h: HM.MRec, -tt: List<&2, HM.MRec>, -n: List<&2, String>, +x: Nat,
    +e: {Bool.and(HM.str_in(n, HM.Msg.to(HM.MRec.msg(h))), HM.is_queued(HM.MRec.st(h))) == c : Bool},
    e1: {Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), x) == False{} : Bool},
    ih: {HM.nat_in(HC.ids_of(HM.queued_in(tt, n)), x) == False{} : Bool})
    -> {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HM.Msg>, Bool.and(HM.str_in(n, HM.Msg.to(HM.MRec.msg(h))), HM.is_queued(HM.MRec.st(h))),
          HM.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(HM.str_in(n, HM.Msg.to(HM.MRec.msg(h))), HM.is_queued(HM.MRec.st(h))), True{}, e) : {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HM.Msg>, _, HM.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}
      hub_or_false(Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), x), HM.nat_in(HC.ids_of(HM.queued_in(tt, n)), x), e1, ih)
    case False{}:
      %Equal.sym(Bool, Bool.and(HM.str_in(n, HM.Msg.to(HM.MRec.msg(h))), HM.is_queued(HM.MRec.st(h))), False{}, e) : {HM.nat_in(HC.ids_of(Bool.pick(List<&2, HM.Msg>, _, HM.MRec.msg(h) <> HM.queued_in(tt, n), HM.queued_in(tt, n))), x) == False{} : Bool}
      ih

# the queued ids of a list are among its ids
def hub_q_sub(+t: List<&2, HM.MRec>, +x: Nat, +n: List<&2, String>, +nf: {HM.nat_in(HI.ids(t), x) == False{} : Bool})
    -> {HM.nat_in(HC.ids_of(HM.queued_in(t, n)), x) == False{} : Bool}:
  match t:
    case Nil{}:
      {==}
    case Con{h, tt}:
      hub_q_sub.pick(Bool.and(HM.str_in(n, HM.Msg.to(HM.MRec.msg(h))), HM.is_queued(HM.MRec.st(h))), h, tt, n, x, {==},
        hub_or_false_l(Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), x), HM.nat_in(HI.ids(tt), x), nf),
        hub_q_sub(tt, x, n, hub_or_false_r(Nat.is_eq(HM.Msg.id(HM.MRec.msg(h)), x), HM.nat_in(HI.ids(tt), x), nf)))

def hub_and_left(a: Bool, -b: Bool, e: {Bool.and(a, b) == True{} : Bool}) -> {a == True{} : Bool}:
  match a:
    case True{}:
      {==}
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, False{}, True{})

def hub_K.pick(c: Bool, +m: HM.Msg, +st: HM.MSt, +se: Bool, +t: List<&2, HM.MRec>, +n: List<&2, String>,
    +e: {Bool.and(HM.str_in(n, HM.Msg.to(m)), HM.is_queued(st)) == c : Bool}, +dh: {HM.nat_in(HI.ids(t), HM.Msg.id(m)) == False{} : Bool},
    ih: {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n) == Nil{} : List<&2, HM.Msg>})
    -> {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HM.Msg>, Bool.and(HM.str_in(n, HM.Msg.to(m)), HM.is_queued(st)), m <> HM.queued_in(t, n), HM.queued_in(t, n))), HM.MRec{m, st, se} <> t), n) == Nil{} : List<&2, HM.Msg>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(HM.str_in(n, HM.Msg.to(m)), HM.is_queued(st)), True{}, e) : {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HM.Msg>, _, m <> HM.queued_in(t, n), HM.queued_in(t, n))), HM.MRec{m, st, se} <> t), n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(m), HM.Msg.id(m)), True{}, hub_eq_refl(HM.Msg.id(m))) : {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(HM.MRec{m, st, se}, HM.OSt{HM.Rejected{"recipient_unavailable"}}) <> t, HM.MRec{m, st, se} <> HM.map_msg(t, HM.Msg.id(m), HM.OSt{HM.Rejected{"recipient_unavailable"}}))), n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(List<&2, HM.MRec>, hub_rej(HC.ids_of(HM.queued_in(t, n)), HM.MRec{m, HM.Rejected{"recipient_unavailable"}, se} <> t), HM.MRec{m, HM.Rejected{"recipient_unavailable"}, se} <> hub_rej(HC.ids_of(HM.queued_in(t, n)), t), hub_rej_skip(HC.ids_of(HM.queued_in(t, n)), HM.MRec{m, HM.Rejected{"recipient_unavailable"}, se}, t, hub_q_sub(t, HM.Msg.id(m), n, dh))) : {HM.queued_in(_, n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(Bool, HM.str_in(n, HM.Msg.to(m)), True{}, hub_and_left(HM.str_in(n, HM.Msg.to(m)), HM.is_queued(st), e)) : {Bool.pick(List<&2, HM.Msg>, Bool.and(_, False{}), m <> HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n), HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n)) == Nil{} : List<&2, HM.Msg>}
      ih
    case False{}:
      %Equal.sym(Bool, Bool.and(HM.str_in(n, HM.Msg.to(m)), HM.is_queued(st)), False{}, e) : {HM.queued_in(hub_rej(HC.ids_of(Bool.pick(List<&2, HM.Msg>, _, m <> HM.queued_in(t, n), HM.queued_in(t, n))), HM.MRec{m, st, se} <> t), n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(List<&2, HM.MRec>, hub_rej(HC.ids_of(HM.queued_in(t, n)), HM.MRec{m, st, se} <> t), HM.MRec{m, st, se} <> hub_rej(HC.ids_of(HM.queued_in(t, n)), t), hub_rej_skip(HC.ids_of(HM.queued_in(t, n)), HM.MRec{m, st, se}, t, hub_q_sub(t, HM.Msg.id(m), n, dh))) : {HM.queued_in(_, n) == Nil{} : List<&2, HM.Msg>}
      %Equal.sym(Bool, Bool.and(HM.str_in(n, HM.Msg.to(m)), HM.is_queued(st)), False{}, e) : {Bool.pick(List<&2, HM.Msg>, _, m <> HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n), HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(t, n)), t), n)) == Nil{} : List<&2, HM.Msg>}
      ih

# rejecting every message queued for `n` leaves none queued for it
def hub_K(+xs: List<&2, HM.MRec>, +n: List<&2, String>, d: HI.Distinct(HI.ids(xs))) -> {HM.queued_in(hub_rej(HC.ids_of(HM.queued_in(xs, n)), xs), n) == Nil{} : List<&2, HM.Msg>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match h:
        case HM.MRec{m, st, se}:
          match d:
            case HX.Both{dh, dt}:
              hub_K.pick(Bool.and(HM.str_in(n, HM.Msg.to(m)), HM.is_queued(st)), m, st, se, t, n, {==}, dh, hub_K(t, n, dt))

def hub_d_hout(-o: HM.St, +cx: HX.Cx<o>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> {HX.d(o, HC.out(o, cx, kvs)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_msgs_closed(s: HM.St, +id: Nat, +res: String) -> {HM.St.msgs(HM.apply(s, HM.ECardClosed{id, res})) == HM.St.msgs(s) : List<&2, HM.MRec>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_msgs_life(s: HM.St, +n: String, +l: HM.Life, +r: Maybe<&2, String>) -> {HM.St.msgs(HM.apply(s, HM.ELifecycle{n, l, r})) == HM.St.msgs(s) : List<&2, HM.MRec>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_msgs_eq(-o: HM.St, -a: HX.Cx<o>, -b: HX.Cx<o>, e: {HX.d(o, a) == HX.d(o, b) : HM.St}) -> {HM.St.msgs(HX.d(o, a)) == HM.St.msgs(HX.d(o, b)) : List<&2, HM.MRec>}:
  Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, a), HX.d(o, b), e)

# ---- a drop's timer stops (HC.timers_gone) touch no message, agent or card ----

def hub_msgs_tstop(s: HM.St, +id: Nat, +why: String, +at: Nat) -> {HM.St.msgs(HM.apply(s, HM.ETimerStop{id, why, at})) == HM.St.msgs(s) : List<&2, HM.MRec>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_agents_tstop(s: HM.St, +id: Nat, +why: String, +at: Nat) -> {HM.St.agents(HM.apply(s, HM.ETimerStop{id, why, at})) == HM.St.agents(s) : List<&2, HM.Agent>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_cards_tstop(s: HM.St, +id: Nat, +why: String, +at: Nat) -> {HM.St.cards(HM.apply(s, HM.ETimerStop{id, why, at})) == HM.St.cards(s) : List<&2, HM.Card>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_msgs_gone_each(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +why: String)
    -> {HM.St.msgs(HX.d(o, HC.timers_gone.each(o, ids, cx, why))) == HM.St.msgs(HX.d(o, cx)) : List<&2, HM.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.ETimerStop{id, why, HC.now(o, cx)} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, HC.timers_gone.each(o, t, cx2, why))), HM.St.msgs(HX.d(o, cx2)), HM.St.msgs(HX.d(o, cx)),
        hub_msgs_gone_each(o, t, cx2, why),
        Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, cx2)), HM.St.msgs(HM.apply(HX.d(o, cx), ev)), HM.St.msgs(HX.d(o, cx)),
          Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_msgs_tstop(HX.d(o, cx), id, why, HC.now(o, cx))))

def hub_agents_gone_each(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +why: String)
    -> {HM.St.agents(HX.d(o, HC.timers_gone.each(o, ids, cx, why))) == HM.St.agents(HX.d(o, cx)) : List<&2, HM.Agent>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.ETimerStop{id, why, HC.now(o, cx)} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, HC.timers_gone.each(o, t, cx2, why))), HM.St.agents(HX.d(o, cx2)), HM.St.agents(HX.d(o, cx)),
        hub_agents_gone_each(o, t, cx2, why),
        Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, cx2)), HM.St.agents(HM.apply(HX.d(o, cx), ev)), HM.St.agents(HX.d(o, cx)),
          Equal.cong(HM.St, List<&2, HM.Agent>, s => HM.St.agents(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_agents_tstop(HX.d(o, cx), id, why, HC.now(o, cx))))

def hub_cards_gone_each(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +why: String)
    -> {HM.St.cards(HX.d(o, HC.timers_gone.each(o, ids, cx, why))) == HM.St.cards(HX.d(o, cx)) : List<&2, HM.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.ETimerStop{id, why, HC.now(o, cx)} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, HC.timers_gone.each(o, t, cx2, why))), HM.St.cards(HX.d(o, cx2)), HM.St.cards(HX.d(o, cx)),
        hub_cards_gone_each(o, t, cx2, why),
        Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx2)), HM.St.cards(HM.apply(HX.d(o, cx), ev)), HM.St.cards(HX.d(o, cx)),
          Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_cards_tstop(HX.d(o, cx), id, why, HC.now(o, cx))))

def hub_msgs_gone(-o: HM.St, +g: Bool, +cx: HX.Cx<o>, +n: String, +ns: List<&2, String>)
    -> {HM.St.msgs(HX.d(o, HC.timers_gone.if(o, g, cx, n, ns))) == HM.St.msgs(HX.d(o, cx)) : List<&2, HM.MRec>}:
  match g:
    case False{}:
      {==}
    case True{}:
      hub_msgs_gone_each(o, HTM.ids_in(HC.live(o, cx), ns), cx, HC.cat(["@", n, " is gone"]))

def hub_agents_gone(-o: HM.St, +g: Bool, +cx: HX.Cx<o>, +n: String, +ns: List<&2, String>)
    -> {HM.St.agents(HX.d(o, HC.timers_gone.if(o, g, cx, n, ns))) == HM.St.agents(HX.d(o, cx)) : List<&2, HM.Agent>}:
  match g:
    case False{}:
      {==}
    case True{}:
      hub_agents_gone_each(o, HTM.ids_in(HC.live(o, cx), ns), cx, HC.cat(["@", n, " is gone"]))

def hub_cards_gone(-o: HM.St, +g: Bool, +cx: HX.Cx<o>, +n: String, +ns: List<&2, String>)
    -> {HM.St.cards(HX.d(o, HC.timers_gone.if(o, g, cx, n, ns))) == HM.St.cards(HX.d(o, cx)) : List<&2, HM.Card>}:
  match g:
    case False{}:
      {==}
    case True{}:
      hub_cards_gone_each(o, HTM.ids_in(HC.live(o, cx), ns), cx, HC.cat(["@", n, " is gone"]))

def hub_d_line(-o: HM.St, +cx: HX.Cx<o>, +a: String, +k: String, +t: String) -> {HX.d(o, HC.line(o, cx, a, k, t)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

# a delivered message's feed line (or none, for a steer_leftover) keeps the state
def hub_d_dline(-o: HM.St, again: Bool, you: Bool, +cx: HX.Cx<o>, +name: String, +m: HM.Msg)
    -> {HX.d(o, HC.deliver.line(o, again, you, cx, name, m)) == HX.d(o, cx) : HM.St}:
  match again you:
    case True{} _:
      {==}
    case False{} True{}:
      hub_d_line(o, cx, name, "you", HM.Msg.text(m))
    case False{} False{}:
      hub_d_line(o, cx, name, "msg-in", HC.cat([HM.Msg.from(m), " ", HC.mid(HM.Msg.id(m)), " : ", HM.Msg.text(m)]))

# closing cards keeps the messages
def hub_msgs_close_ids(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +res: String)
    -> {HM.St.msgs(HX.d(o, HC.close_ids(o, ids, cx, res))) == HM.St.msgs(HX.d(o, cx)) : List<&2, HM.MRec>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, rest}:
      +cx2 = HC.emit(o, cx, HM.ECardClosed{id, res}, Unit{}, Unit{}, Unit{})
      +cx3 = HC.line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))
      Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, HC.close_ids(o, rest, cx3, res))), HM.St.msgs(HX.d(o, cx3)), HM.St.msgs(HX.d(o, cx)),
        hub_msgs_close_ids(o, rest, cx3, res),
        Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, cx3)), HM.St.msgs(HX.d(o, cx2)), HM.St.msgs(HX.d(o, cx)),
          hub_msgs_eq(o, cx3, cx2, hub_d_line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))),
          Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, cx2)), HM.St.msgs(HM.apply(HX.d(o, cx), HM.ECardClosed{id, res})), HM.St.msgs(HX.d(o, cx)),
            Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), HM.ECardClosed{id, res}),
              hub_d_emit(o, cx, HM.ECardClosed{id, res}, Unit{}, Unit{}, Unit{})),
            hub_msgs_closed(HX.d(o, cx), id, res))))

def hub_msgs_tr(-a: List<&2, HM.MRec>, -b: List<&2, HM.MRec>, -n: List<&2, String>, e: {a == b : List<&2, HM.MRec>},
    k: {HM.queued_in(b, n) == Nil{} : List<&2, HM.Msg>}) -> {HM.queued_in(a, n) == Nil{} : List<&2, HM.Msg>}:
  %Equal.sym(List<&2, HM.MRec>, a, b, e) : {HM.queued_in(_, n) == Nil{} : List<&2, HM.Msg>}
  k

def hub_agents_eq(-o: HM.St, -a: HX.Cx<o>, -b: HX.Cx<o>, e: {HX.d(o, a) == HX.d(o, b) : HM.St}) -> {HM.St.agents(HX.d(o, a)) == HM.St.agents(HX.d(o, b)) : List<&2, HM.Agent>}:
  Equal.cong(HM.St, List<&2, HM.Agent>, s => HM.St.agents(s), HX.d(o, a), HX.d(o, b), e)

def hub_agents_closed(s: HM.St, +id: Nat, +res: String) -> {HM.St.agents(HM.apply(s, HM.ECardClosed{id, res})) == HM.St.agents(s) : List<&2, HM.Agent>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_agents_close_ids(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +res: String)
    -> {HM.St.agents(HX.d(o, HC.close_ids(o, ids, cx, res))) == HM.St.agents(HX.d(o, cx)) : List<&2, HM.Agent>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, rest}:
      +ev = {HM.ECardClosed{id, res} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      +cx3 = HC.line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))
      Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, HC.close_ids(o, rest, cx3, res))), HM.St.agents(HX.d(o, cx3)), HM.St.agents(HX.d(o, cx)),
        hub_agents_close_ids(o, rest, cx3, res),
        Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, cx3)), HM.St.agents(HX.d(o, cx2)), HM.St.agents(HX.d(o, cx)),
          hub_agents_eq(o, cx3, cx2, hub_d_line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))),
          Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, cx2)), HM.St.agents(HM.apply(HX.d(o, cx), ev)), HM.St.agents(HX.d(o, cx)),
            Equal.cong(HM.St, List<&2, HM.Agent>, s => HM.St.agents(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
            hub_agents_closed(HX.d(o, cx), id, res))))

def hub_agents_state(s: HM.St, +id: Nat, +x: HM.MSt) -> {HM.St.agents(HM.apply(s, HM.EMsgState{id, x})) == HM.St.agents(s) : List<&2, HM.Agent>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_agents_reject(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {HM.St.agents(HX.d(o, HC.reject_ids(o, ids, cx))) == HM.St.agents(HX.d(o, cx)) : List<&2, HM.Agent>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, HC.reject_ids(o, t, cx2))), HM.St.agents(HX.d(o, cx2)), HM.St.agents(HX.d(o, cx)),
        hub_agents_reject(o, t, cx2),
        Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, cx2)), HM.St.agents(HM.apply(HX.d(o, cx), ev)), HM.St.agents(HX.d(o, cx)),
          Equal.cong(HM.St, List<&2, HM.Agent>, s => HM.St.agents(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_agents_state(HX.d(o, cx), id, HM.Rejected{"recipient_unavailable"})))

def hub_names_tr(-a: List<&2, HM.MRec>, -b: List<&2, HM.MRec>, -na: List<&2, String>, -nb: List<&2, String>,
    e: {a == b : List<&2, HM.MRec>}, en: {na == nb : List<&2, String>},
    k: {HM.queued_in(b, nb) == Nil{} : List<&2, HM.Msg>}) -> {HM.queued_in(a, na) == Nil{} : List<&2, HM.Msg>}:
  %Equal.sym(List<&2, HM.MRec>, a, b, e) : {HM.queued_in(_, na) == Nil{} : List<&2, HM.Msg>}
  %Equal.sym(List<&2, String>, na, nb, en) : {HM.queued_in(b, _) == Nil{} : List<&2, HM.Msg>}
  k

def Laws.stop_leaves_no_queue(o, cx, n, life, why, u):
  +cx1 = HC.out(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
  +cx2 = HC.fail_waiters(o, cx1, n, "stopped")
  +r = HC.rr(o, cx2)
  +cx3 = HC.set_r(o, cx2, HX.R.with_steered(r, HC.steer_del(HX.R.steered(r), n)))
  +x4 = HX.Rt.with_turn(HX.Rt.with_run(HC.rt(o, cx3, n), HM.RDown{}), None{})
  +cx4 = HC.upd_rt(o, cx3, x4)
  +ev = {HM.ELifecycle{n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})} : HM.Ev}
  +cx5 = HC.emit(o, cx4, ev, Unit{}, Unit{}, Unit{})
  +m5 = HM.St.msgs(HX.d(o, cx5))
  +n5 = HM.names_of(HX.d(o, cx5), n)
  +ids = HC.ids_of(HM.queued_in(m5, n5))
  +cx6 = HC.reject_ids(o, ids, cx5)
  +g = HC.stop_task.go(o, cx, n, life, why)
  +tg = HC.timers_gone(o, life, g, n)
  # d is unchanged up to cx4
  +e4 = Equal.trans(HM.St, HX.d(o, cx4), HX.d(o, cx3), HX.d(o, cx), hub_d_upd_rt(o, cx3, x4),
    Equal.trans(HM.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx), hub_d_with_r(o, cx2, HX.R.with_steered(r, HC.steer_del(HX.R.steered(r), n))),
      Equal.trans(HM.St, HX.d(o, cx2), HX.d(o, cx1), HX.d(o, cx), hub_d_fail_waiters(o, cx1, n, "stopped"),
        hub_d_hout(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})]))))
  # the messages of cx5 are those of cx
  +e5 = Equal.trans(List<&2, HM.MRec>, m5, HM.St.msgs(HM.apply(HX.d(o, cx4), ev)), HM.St.msgs(HX.d(o, cx)),
    Equal.cong(HM.St, List<&2, HM.MRec>, s => HM.St.msgs(s), HX.d(o, cx5), HM.apply(HX.d(o, cx4), ev), hub_d_emit(o, cx4, ev, Unit{}, Unit{}, Unit{})),
    Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HM.apply(HX.d(o, cx4), ev)), HM.St.msgs(HX.d(o, cx4)), HM.St.msgs(HX.d(o, cx)),
      hub_msgs_life(HX.d(o, cx4), n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})),
      hub_msgs_eq(o, cx4, cx, e4)))
  # the messages at the end: those of cx5 with the queued ones rejected
  +ef = Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.msgs(HX.d(o, g)), hub_rej(ids, m5),
    Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.msgs(HX.d(o, tg)), HM.St.msgs(HX.d(o, g)),
      hub_msgs_eq(o, HC.stop_task(o, cx, n, life, why), tg, hub_d_with_r(o, tg, HX.R.with_waiters(HC.rr(o, tg), HC.waiters_not(HX.R.waiters(HC.rr(o, cx)), n)))),
      hub_msgs_gone(o, HC.life_gone(life), g, n, HM.names_of(HX.d(o, g), n))),
    Equal.trans(List<&2, HM.MRec>, HM.St.msgs(HX.d(o, g)), HM.St.msgs(HX.d(o, cx6)), hub_rej(ids, m5),
      hub_msgs_close_ids(o, HC.card_ids(HM.St.cards(HX.d(o, cx6)), n, Nil{}), cx6, "agent stopped"),
      hub_msgs_reject(o, ids, cx5)))
  # the names at the end: those of cx5 (no agent changes after it)
  +en = Equal.cong(List<&2, HM.Agent>, List<&2, String>, xs => n <> HM.names_of.go(HM.agent_named(xs, n)),
    HM.St.agents(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.agents(HX.d(o, cx5)),
    Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.agents(HX.d(o, g)), HM.St.agents(HX.d(o, cx5)),
      Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.agents(HX.d(o, tg)), HM.St.agents(HX.d(o, g)),
        hub_agents_eq(o, HC.stop_task(o, cx, n, life, why), tg, hub_d_with_r(o, tg, HX.R.with_waiters(HC.rr(o, tg), HC.waiters_not(HX.R.waiters(HC.rr(o, cx)), n)))),
        hub_agents_gone(o, HC.life_gone(life), g, n, HM.names_of(HX.d(o, g), n))),
      Equal.trans(List<&2, HM.Agent>, HM.St.agents(HX.d(o, g)), HM.St.agents(HX.d(o, cx6)), HM.St.agents(HX.d(o, cx5)),
        hub_agents_close_ids(o, HC.card_ids(HM.St.cards(HX.d(o, cx6)), n, Nil{}), cx6, "agent stopped"),
        hub_agents_reject(o, ids, cx5))))
  hub_names_tr(HM.St.msgs(HX.d(o, HC.stop_task(o, cx, n, life, why))), hub_rej(ids, m5), HM.names_of(HX.d(o, HC.stop_task(o, cx, n, life, why)), n), n5, ef, en,
    hub_K(m5, n5, hub_distinct_tr(HI.ids(m5), HI.ids(HM.St.msgs(HX.d(o, cx))),
      Equal.cong(List<&2, HM.MRec>, List<&2, Nat>, xs => HI.ids(xs), m5, HM.St.msgs(HX.d(o, cx)), e5),
      hub_fst(HI.Distinct(HI.ids(HM.St.msgs(HX.d(o, cx)))), HI.Below(HI.ids(HM.St.msgs(HX.d(o, cx))), HM.St.next_msg(HX.d(o, cx))), u))))

# the cards after the closes of stop_task
def hub_dropc(ids: List<&2, Nat>, +cs: List<&2, HM.Card>) -> List<&2, HM.Card>:
  match ids:
    case Nil{}:
      cs
    case Con{+id, +t}:
      hub_dropc(t, HM.drop_card(cs, id))

def hub_dropc_skip.go(+i: Nat, -ns: List<&2, Nat>, +h: HM.Card, +t: List<&2, HM.Card>,
    ef: {Nat.is_eq(HM.Card.id(h), i) == False{} : Bool},
    ih: {hub_dropc(ns, h <> HM.drop_card(t, i)) == h <> hub_dropc(ns, HM.drop_card(t, i)) : List<&2, HM.Card>})
    -> {hub_dropc(i <> ns, h <> t) == h <> hub_dropc(i <> ns, t) : List<&2, HM.Card>}:
  %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), i), False{}, ef) : {hub_dropc(ns, Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, i))) == h <> hub_dropc(i <> ns, t) : List<&2, HM.Card>}
  ih

def hub_dropc_skip(+ids: List<&2, Nat>, +h: HM.Card, +t: List<&2, HM.Card>, +nf: {HM.nat_in(ids, HM.Card.id(h)) == False{} : Bool})
    -> {hub_dropc(ids, h <> t) == h <> hub_dropc(ids, t) : List<&2, HM.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{i, ns}:
      hub_dropc_skip.go(i, ns, h, t,
        Equal.trans(Bool, Nat.is_eq(HM.Card.id(h), i), Nat.is_eq(i, HM.Card.id(h)), False{},
          hub_eq_sym(HM.Card.id(h), i),
          hub_or_false_l(Nat.is_eq(i, HM.Card.id(h)), HM.nat_in(ns, HM.Card.id(h)), nf)),
        hub_dropc_skip(ns, h, HM.drop_card(t, i), hub_or_false_r(Nat.is_eq(i, HM.Card.id(h)), HM.nat_in(ns, HM.Card.id(h)), nf)))

def hub_c_sub.pick(c: Bool, +h: HM.Card, -t: List<&2, HM.Card>, -n: String, +x: Nat,
    +e: {Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))) == c : Bool}, e1: {Nat.is_eq(HM.Card.id(h), x) == False{} : Bool},
    ih: {HM.nat_in(HC.card_ids(t, n, Nil{}), x) == False{} : Bool})
    -> {HM.nat_in(Bool.pick(List<&2, Nat>, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), True{}, e) : {HM.nat_in(Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}
      hub_or_false(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HC.card_ids(t, n, Nil{}), x), e1, ih)
    case False{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), False{}, e) : {HM.nat_in(Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), x) == False{} : Bool}
      ih

def hub_c_sub(+t: List<&2, HM.Card>, +x: Nat, +n: String, +nf: {HM.nat_in(HI.card_ids(t), x) == False{} : Bool})
    -> {HM.nat_in(HC.card_ids(t, n, Nil{}), x) == False{} : Bool}:
  match t:
    case Nil{}:
      {==}
    case Con{h, tt}:
      hub_c_sub.pick(Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), h, tt, n, x, {==},
        hub_or_false_l(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(tt), x), nf),
        hub_c_sub(tt, x, n, hub_or_false_r(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(tt), x), nf)))

def hub_KC.pick(c: Bool, +h: HM.Card, +t: List<&2, HM.Card>, +n: String,
    +e: {Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))) == c : Bool}, +dh: {HM.nat_in(HI.card_ids(t), HM.Card.id(h)) == False{} : Bool},
    ih: {HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{}) == Nil{} : List<&2, Nat>})
    -> {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), True{}, e) : {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), HM.Card.id(h)), True{}, hub_eq_refl(HM.Card.id(h))) : {HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, HM.Card.id(h)))), n, Nil{}) == Nil{} : List<&2, Nat>}
      ih
    case False{}:
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), False{}, e) : {HC.card_ids(hub_dropc(Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(t, n, Nil{}), HC.card_ids(t, n, Nil{})), h <> t), n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(List<&2, HM.Card>, hub_dropc(HC.card_ids(t, n, Nil{}), h <> t), h <> hub_dropc(HC.card_ids(t, n, Nil{}), t), hub_dropc_skip(HC.card_ids(t, n, Nil{}), h, t, hub_c_sub(t, HM.Card.id(h), n, dh))) : {HC.card_ids(_, n, Nil{}) == Nil{} : List<&2, Nat>}
      %Equal.sym(Bool, Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), False{}, e) : {Bool.pick(List<&2, Nat>, _, HM.Card.id(h) <> HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{}), HC.card_ids(hub_dropc(HC.card_ids(t, n, Nil{}), t), n, Nil{})) == Nil{} : List<&2, Nat>}
      ih

# closing every card of `n` leaves none of its cards
def hub_KC(+cs: List<&2, HM.Card>, +n: String, d: HI.Distinct(HI.card_ids(cs))) -> {HC.card_ids(hub_dropc(HC.card_ids(cs, n, Nil{}), cs), n, Nil{}) == Nil{} : List<&2, Nat>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match d:
        case HX.Both{dh, dt}:
          hub_KC.pick(Bool.and(String.eq(HM.Card.agent(h), n), Bool.or(List.is_empty(&2, String, Nil{}), HM.str_in(Nil{}, HM.Card.kind(h)))), h, t, n, {==}, dh, hub_KC(t, n, dt))

def hub_cards_eq(-o: HM.St, -a: HX.Cx<o>, -b: HX.Cx<o>, e: {HX.d(o, a) == HX.d(o, b) : HM.St}) -> {HM.St.cards(HX.d(o, a)) == HM.St.cards(HX.d(o, b)) : List<&2, HM.Card>}:
  Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, a), HX.d(o, b), e)

def hub_cards_closed(s: HM.St, +id: Nat, +res: String) -> {HM.St.cards(HM.apply(s, HM.ECardClosed{id, res})) == HM.drop_card(HM.St.cards(s), id) : List<&2, HM.Card>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_cards_life(s: HM.St, +n: String, +l: HM.Life, +r: Maybe<&2, String>) -> {HM.St.cards(HM.apply(s, HM.ELifecycle{n, l, r})) == HM.St.cards(s) : List<&2, HM.Card>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_cards_state(s: HM.St, +id: Nat, +x: HM.MSt) -> {HM.St.cards(HM.apply(s, HM.EMsgState{id, x})) == HM.St.cards(s) : List<&2, HM.Card>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

# rejecting messages keeps the cards
def hub_cards_reject(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>)
    -> {HM.St.cards(HX.d(o, HC.reject_ids(o, ids, cx))) == HM.St.cards(HX.d(o, cx)) : List<&2, HM.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, t}:
      +ev = {HM.EMsgState{id, HM.Rejected{"recipient_unavailable"}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, HC.reject_ids(o, t, cx2))), HM.St.cards(HX.d(o, cx2)), HM.St.cards(HX.d(o, cx)),
        hub_cards_reject(o, t, cx2),
        Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx2)), HM.St.cards(HM.apply(HX.d(o, cx), ev)), HM.St.cards(HX.d(o, cx)),
          Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
          hub_cards_state(HX.d(o, cx), id, HM.Rejected{"recipient_unavailable"})))

# closing cards drops them, one by one
def hub_cards_close_ids(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +res: String)
    -> {HM.St.cards(HX.d(o, HC.close_ids(o, ids, cx, res))) == hub_dropc(ids, HM.St.cards(HX.d(o, cx))) : List<&2, HM.Card>}:
  match ids:
    case Nil{}:
      {==}
    case Con{id, rest}:
      +ev = {HM.ECardClosed{id, res} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      +cx3 = HC.line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))
      Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, HC.close_ids(o, rest, cx3, res))), hub_dropc(rest, HM.St.cards(HX.d(o, cx3))),
        hub_dropc(id <> rest, HM.St.cards(HX.d(o, cx))),
        hub_cards_close_ids(o, rest, cx3, res),
        Equal.cong(List<&2, HM.Card>, List<&2, HM.Card>, cs => hub_dropc(rest, cs), HM.St.cards(HX.d(o, cx3)), HM.drop_card(HM.St.cards(HX.d(o, cx)), id),
          Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx3)), HM.St.cards(HX.d(o, cx2)), HM.drop_card(HM.St.cards(HX.d(o, cx)), id),
            hub_cards_eq(o, cx3, cx2, hub_d_line(o, cx2, HM.MAIN(), "card-closed", HC.cat(["#", HC.ns(id), " ", res]))),
            Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx2)), HM.St.cards(HM.apply(HX.d(o, cx), ev)), HM.drop_card(HM.St.cards(HX.d(o, cx)), id),
              Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
              hub_cards_closed(HX.d(o, cx), id, res)))))

def hub_cids_tr(-a: List<&2, HM.Card>, -b: List<&2, HM.Card>, -n: String, e: {a == b : List<&2, HM.Card>},
    k: {HC.card_ids(b, n, Nil{}) == Nil{} : List<&2, Nat>}) -> {HC.card_ids(a, n, Nil{}) == Nil{} : List<&2, Nat>}:
  %Equal.sym(List<&2, HM.Card>, a, b, e) : {HC.card_ids(_, n, Nil{}) == Nil{} : List<&2, Nat>}
  k

def Laws.stop_leaves_no_card(o, cx, n, life, why, u):
  +cx1 = HC.out(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})])
  +cx2 = HC.fail_waiters(o, cx1, n, "stopped")
  +r = HC.rr(o, cx2)
  +cx3 = HC.set_r(o, cx2, HX.R.with_steered(r, HC.steer_del(HX.R.steered(r), n)))
  +x4 = HX.Rt.with_turn(HX.Rt.with_run(HC.rt(o, cx3, n), HM.RDown{}), None{})
  +cx4 = HC.upd_rt(o, cx3, x4)
  +ev = {HM.ELifecycle{n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})} : HM.Ev}
  +cx5 = HC.emit(o, cx4, ev, Unit{}, Unit{}, Unit{})
  +ids = HC.ids_of(HM.queued_in(HM.St.msgs(HX.d(o, cx5)), HM.names_of(HX.d(o, cx5), n)))
  +cx6 = HC.reject_ids(o, ids, cx5)
  +c6 = HM.St.cards(HX.d(o, cx6))
  +g = HC.stop_task.go(o, cx, n, life, why)
  +tg = HC.timers_gone(o, life, g, n)
  +e4 = Equal.trans(HM.St, HX.d(o, cx4), HX.d(o, cx3), HX.d(o, cx), hub_d_upd_rt(o, cx3, x4),
    Equal.trans(HM.St, HX.d(o, cx3), HX.d(o, cx2), HX.d(o, cx), hub_d_with_r(o, cx2, HX.R.with_steered(r, HC.steer_del(HX.R.steered(r), n))),
      Equal.trans(HM.St, HX.d(o, cx2), HX.d(o, cx1), HX.d(o, cx), hub_d_fail_waiters(o, cx1, n, "stopped"),
        hub_d_hout(o, cx, [J.kv("fx", J.JStr{"kill"}), J.kv("agent", J.JStr{n})]))))
  # the cards of cx6 are those of cx
  +e6 = Equal.trans(List<&2, HM.Card>, c6, HM.St.cards(HX.d(o, cx5)), HM.St.cards(HX.d(o, cx)),
    hub_cards_reject(o, ids, cx5),
    Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, cx5)), HM.St.cards(HM.apply(HX.d(o, cx4), ev)), HM.St.cards(HX.d(o, cx)),
      Equal.cong(HM.St, List<&2, HM.Card>, s => HM.St.cards(s), HX.d(o, cx5), HM.apply(HX.d(o, cx4), ev), hub_d_emit(o, cx4, ev, Unit{}, Unit{}, Unit{})),
      Equal.trans(List<&2, HM.Card>, HM.St.cards(HM.apply(HX.d(o, cx4), ev)), HM.St.cards(HX.d(o, cx4)), HM.St.cards(HX.d(o, cx)),
        hub_cards_life(HX.d(o, cx4), n, life, Bool.pick(Maybe<&2, String>, String.is_empty(why), None{}, Some{why})),
        hub_cards_eq(o, cx4, cx, e4))))
  # the cards at the end: those of cx6 without the task's
  +ef = Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.cards(HX.d(o, g)),
    hub_dropc(HC.card_ids(c6, n, Nil{}), c6),
    Equal.trans(List<&2, HM.Card>, HM.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), HM.St.cards(HX.d(o, tg)), HM.St.cards(HX.d(o, g)),
      hub_cards_eq(o, HC.stop_task(o, cx, n, life, why), tg, hub_d_with_r(o, tg, HX.R.with_waiters(HC.rr(o, tg), HC.waiters_not(HX.R.waiters(HC.rr(o, cx)), n)))),
      hub_cards_gone(o, HC.life_gone(life), g, n, HM.names_of(HX.d(o, g), n))),
    hub_cards_close_ids(o, HC.card_ids(c6, n, Nil{}), cx6, "agent stopped"))
  hub_cids_tr(HM.St.cards(HX.d(o, HC.stop_task(o, cx, n, life, why))), hub_dropc(HC.card_ids(c6, n, Nil{}), c6), n, ef,
    hub_KC(c6, n, hub_distinct_tr(HI.card_ids(c6), HI.card_ids(HM.St.cards(HX.d(o, cx))),
      Equal.cong(List<&2, HM.Card>, List<&2, Nat>, cs => HI.card_ids(cs), c6, HM.St.cards(HX.d(o, cx)), e6),
      hub_fst(HI.Distinct(HI.card_ids(HM.St.cards(HX.d(o, cx)))), HI.Below(HI.card_ids(HM.St.cards(HX.d(o, cx))), HM.St.next_card(HX.d(o, cx))), u))))


# ---- L12 support: no two open cards share an id (cards_unique) ----

def hub_below_notin(+ns: List<&2, Nat>, +x: Nat, bl: HI.Below(ns, x)) -> {HM.nat_in(ns, x) == False{} : Bool}:
  match ns:
    case Nil{}:
      {==}
    case Con{y, t}:
      match bl:
        case HX.Both{ly, bt}:
          hub_or_false(Nat.is_eq(y, x), HM.nat_in(t, x),
            Equal.trans(Bool, Nat.is_eq(y, x), Nat.is_eq(x, y), False{}, hub_eq_sym(y, x), hub_lt_neq(y, x, ly)),
            hub_below_notin(t, x, bt))

def hub_drop_absent.go(c: Bool, +h: HM.Card, +t: List<&2, HM.Card>, -id: Nat, +e: {Nat.is_eq(HM.Card.id(h), id) == c : Bool},
    ec: {c == False{} : Bool}, ih: {HM.drop_card(t, id) == t : List<&2, HM.Card>}) -> {HM.drop_card(h <> t, id) == h <> t : List<&2, HM.Card>}:
  match c:
    case True{}:
      hub_absurd_eq(hub_tf_absurd(ec), List<&2, HM.Card>, HM.drop_card(h <> t, id), h <> t)
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), False{}, e) : {Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id)) == h <> t : List<&2, HM.Card>}
      Equal.cong(List<&2, HM.Card>, List<&2, HM.Card>, xs => h <> xs, HM.drop_card(t, id), t, ih)

# dropping an id no card has changes nothing
def hub_drop_absent(+cs: List<&2, HM.Card>, +id: Nat, +nf: {HM.nat_in(HI.card_ids(cs), id) == False{} : Bool}) -> {HM.drop_card(cs, id) == cs : List<&2, HM.Card>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_drop_absent.go(Nat.is_eq(HM.Card.id(h), id), h, t, id, {==},
        hub_or_false_l(Nat.is_eq(HM.Card.id(h), id), HM.nat_in(HI.card_ids(t), id), nf),
        hub_drop_absent(t, id, hub_or_false_r(Nat.is_eq(HM.Card.id(h), id), HM.nat_in(HI.card_ids(t), id), nf)))

def hub_cids_snoc(+cs: List<&2, HM.Card>, +c: HM.Card) -> {HI.card_ids(HM.snoc(HM.Card, cs, c)) == HM.snoc(Nat, HI.card_ids(cs), HM.Card.id(c)) : List<&2, Nat>}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, Nat>, List<&2, Nat>, ys => HM.Card.id(h) <> ys, HI.card_ids(HM.snoc(HM.Card, t, c)), HM.snoc(Nat, HI.card_ids(t), HM.Card.id(c)), hub_cids_snoc(t, c))

def hub_drop_notin.go(c: Bool, +h: HM.Card, +t: List<&2, HM.Card>, -id: Nat, +x: Nat, +e: {Nat.is_eq(HM.Card.id(h), id) == c : Bool},
    +nf: {HM.nat_in(HI.card_ids(h <> t), x) == False{} : Bool}, ih: {HM.nat_in(HI.card_ids(HM.drop_card(t, id)), x) == False{} : Bool})
    -> {HM.nat_in(HI.card_ids(HM.drop_card(h <> t, id)), x) == False{} : Bool}:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), True{}, e) : {HM.nat_in(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id))), x) == False{} : Bool}
      hub_or_false_r(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf)
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), False{}, e) : {HM.nat_in(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id))), x) == False{} : Bool}
      hub_or_false(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(HM.drop_card(t, id)), x),
        hub_or_false_l(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf), ih)

# dropping a card never brings back an id
def hub_drop_notin(+cs: List<&2, HM.Card>, +id: Nat, +x: Nat, +nf: {HM.nat_in(HI.card_ids(cs), x) == False{} : Bool})
    -> {HM.nat_in(HI.card_ids(HM.drop_card(cs, id)), x) == False{} : Bool}:
  match cs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_drop_notin.go(Nat.is_eq(HM.Card.id(h), id), h, t, id, x, {==}, nf,
        hub_drop_notin(t, id, x, hub_or_false_r(Nat.is_eq(HM.Card.id(h), x), HM.nat_in(HI.card_ids(t), x), nf)))

def hub_ucards_cons(+h: HM.Card, -ys: List<&2, HM.Card>, -n: Nat, nf: {HM.nat_in(HI.card_ids(ys), HM.Card.id(h)) == False{} : Bool},
    lh: {Nat.is_lt(HM.Card.id(h), n) == True{} : Bool}, u: HX.Both<HI.Distinct(HI.card_ids(ys)), HI.Below(HI.card_ids(ys), n)>)
    -> HX.Both<HI.Distinct(HI.card_ids(h <> ys)), HI.Below(HI.card_ids(h <> ys), n)>:
  match u:
    case HX.Both{d2, b2}:
      HX.Both{HX.Both{nf, d2}, HX.Both{lh, b2}}

def hub_ucards_drop.go(c: Bool, +h: HM.Card, +t: List<&2, HM.Card>, +id: Nat, -n: Nat, +e: {Nat.is_eq(HM.Card.id(h), id) == c : Bool},
    +dh: {HM.nat_in(HI.card_ids(t), HM.Card.id(h)) == False{} : Bool}, +lh: {Nat.is_lt(HM.Card.id(h), n) == True{} : Bool},
    +ut: HX.Both<HI.Distinct(HI.card_ids(t)), HI.Below(HI.card_ids(t), n)>, ih: HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(t, id))), HI.Below(HI.card_ids(HM.drop_card(t, id)), n)>) -> HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(h <> t, id))), HI.Below(HI.card_ids(HM.drop_card(h <> t, id)), n)>:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), True{}, e) : HX.Both<HI.Distinct(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id)))), HI.Below(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id))), n)>
      ut
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Card.id(h), id), False{}, e) : HX.Both<HI.Distinct(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id)))), HI.Below(HI.card_ids(Bool.pick(List<&2, HM.Card>, _, t, h <> HM.drop_card(t, id))), n)>
      hub_ucards_cons(h, HM.drop_card(t, id), n, hub_drop_notin(t, id, HM.Card.id(h), dh), lh, ih)

# closing a card keeps the open cards unique
def hub_ucards_drop(+cs: List<&2, HM.Card>, +id: Nat, +n: Nat, u: HX.Both<HI.Distinct(HI.card_ids(cs)), HI.Below(HI.card_ids(cs), n)>) -> HX.Both<HI.Distinct(HI.card_ids(HM.drop_card(cs, id))), HI.Below(HI.card_ids(HM.drop_card(cs, id)), n)>:
  match cs:
    case Nil{}:
      u
    case Con{h, t}:
      match u:
        case HX.Both{d, b}:
          match d b:
            case HX.Both{+dh, +dt} HX.Both{+lh, +bt}:
              hub_ucards_drop.go(Nat.is_eq(HM.Card.id(h), id), h, t, id, n, {==}, dh, lh, HX.Both{dt, bt}, hub_ucards_drop(t, id, n, HX.Both{dt, bt}))

# opening a card with the next free id keeps the open cards unique
def hub_ucards_open(+cs: List<&2, HM.Card>, +c: HM.Card, +nc: Nat, +k: {HM.Card.id(c) == nc : Nat}, u: HX.Both<HI.Distinct(HI.card_ids(cs)), HI.Below(HI.card_ids(cs), nc)>)
    -> HX.Both<HI.Distinct(HI.card_ids(HM.put_card(cs, c))), HI.Below(HI.card_ids(HM.put_card(cs, c)), Nat.max(nc, 1n+HM.Card.id(c)))>:
  match u:
    case HX.Both{+d, +b}:
      +top = Nat.max(nc, 1n+HM.Card.id(c))
      +absent = hub_drop_absent(cs, HM.Card.id(c), hub_below_notin(HI.card_ids(cs), HM.Card.id(c), hub_below_n(HI.card_ids(cs), HM.Card.id(c), nc, k, b)))
      +e = Equal.trans(List<&2, Nat>, HI.card_ids(HM.put_card(cs, c)), HI.card_ids(HM.snoc(HM.Card, cs, c)), HM.snoc(Nat, HI.card_ids(cs), HM.Card.id(c)),
        Equal.cong(List<&2, HM.Card>, List<&2, Nat>, xs => HI.card_ids(HM.snoc(HM.Card, xs, c)), HM.drop_card(cs, HM.Card.id(c)), cs, absent),
        hub_cids_snoc(cs, c))
      HX.Both{hub_distinct_tr(HI.card_ids(HM.put_card(cs, c)), HM.snoc(Nat, HI.card_ids(cs), HM.Card.id(c)), e,
          hub_distinct_snoc(HI.card_ids(cs), HM.Card.id(c), d, hub_below_n(HI.card_ids(cs), HM.Card.id(c), nc, k, b))),
        hub_below_tr(HI.card_ids(HM.put_card(cs, c)), HM.snoc(Nat, HI.card_ids(cs), HM.Card.id(c)), top, e,
          hub_below_snoc(HI.card_ids(cs), HM.Card.id(c), top, hub_below_mono(HI.card_ids(cs), nc, top, Nat.max_ge_l(nc, 1n+HM.Card.id(c)), b),
            hub_lt_ge(HM.Card.id(c), 1n+HM.Card.id(c), top, hub_lt_succ(HM.Card.id(c)), Nat.max_ge_r(nc, 1n+HM.Card.id(c)))))}

def hub_apply_ucards(s: HM.St, e: HM.Ev, u: HI.UniqueCards(s), k: HX.FreshEv(s, e)) -> HI.UniqueCards(HM.apply(s, e)):
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      match e:
        case HM.ECardOpened{c}:
          hub_ucards_open(ca, c, nc, k, u)
        case HM.ECardClosed{id, res}:
          hub_ucards_drop(ca, id, nc, u)
        case HM.ETaskCreated{a, b, c, d, f}:
          u
        case HM.ELifecycle{a, b, c}:
          u
        case HM.EWorkspace{a, b}:
          u
        case HM.ESnapshot{a, b}:
          u
        case HM.EDeclared{a, b, c}:
          u
        case HM.EReported{a, b}:
          u
        case HM.EFileTouched{a, b}:
          u
        case HM.ERenamed{a, b}:
          u
        case HM.EMsgSent{m}:
          u
        case HM.EMsgState{a, b}:
          u
        case HM.ESettled{a}:
          u
        case HM.EMainNote{a, b}:
          u
        case HM.ENotesFlushed{}:
          u
        case HM.ETimerSet{a, b}:
          u
        case HM.ETimerFired{a, b, c, d, f}:
          u
        case HM.ETimerRun{a, b}:
          u
        case HM.ETimerStop{a, b, c}:
          u

def hub_apply_all_ucards(+evs: List<&2, HM.Ev>, +s: HM.St, u: HI.UniqueCards(s), f: HX.Fresh(evs, s)) -> HI.UniqueCards(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      u
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          hub_apply_all_ucards(t, HM.apply(s, e), hub_apply_ucards(s, e, u, k), ft)

def hub_ucards_tr(-x: HM.St, -y: HM.St, e: {x == y : HM.St}, u: HI.UniqueCards(y)) -> HI.UniqueCards(x):
  %Equal.sym(HM.St, x, y, e) : HI.UniqueCards(_)
  u

def Laws.init_cards_unique(w):
  HX.Both{Unit{}, Unit{}}

def Laws.cards_unique(ins, h, u):
  hub_ucards_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h),
    hub_apply_all_ucards(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))

# ---- L4 support: string equality is reflexive ----

def hub_bool_cmp_refl(b: Bool) -> {Bool.cmp(b, b) == EQ{} : Cmp}:
  match b:
    case True{}:
      {==}
    case False{}:
      {==}

def hub_word_cmp_refl(+n: Nat, +w: Word(n)) -> {Word.cmp(n, w, w) == EQ{} : Cmp}:
  match n:
    case 0n:
      {==}
    case 1n+p:
      match w:
        case WCon{b, t}:
          %Equal.sym(Cmp, Word.cmp(p, t, t), EQ{}, hub_word_cmp_refl(p, t)) : {Word.cmp.fin(b, b, _) == EQ{} : Cmp}
          hub_bool_cmp_refl(b)

def hub_char_cmp_refl(+c: Char) -> {Char.cmp(c, c) == ((c, c), EQ{}) : (Char & Char) & Cmp}:
  match c:
    case Chr{x}:
      match x:
        case U32{w}:
          %Equal.sym(Cmp, Word.cmp(32n, w, w), EQ{}, hub_word_cmp_refl(32n, w)) : {((Chr{U32{w}}, Chr{U32{w}}), _) == ((Chr{U32{w}}, Chr{U32{w}}), EQ{}) : (Char & Char) & Cmp}
          {==}

def hub_str_cmp_refl(+s: String) -> {String.cmp(s, s) == ((s, s), EQ{}) : (String & String) & Cmp}:
  match s:
    case SNil{}:
      {==}
    case SCon{h, t}:
      %Equal.sym((Char & Char) & Cmp, Char.cmp(h, h), ((h, h), EQ{}), hub_char_cmp_refl(h)) : {String.cmp.fin(t, t, _) == ((SCon{h, t}, SCon{h, t}), EQ{}) : (String & String) & Cmp}
      %Equal.sym((String & String) & Cmp, String.cmp(t, t), ((t, t), EQ{}), hub_str_cmp_refl(t)) : {String.cmp.rec(h, h, _) == ((SCon{h, t}, SCon{h, t}), EQ{}) : (String & String) & Cmp}
      {==}

# every string equals itself
def hub_str_eq_refl(+s: String) -> {String.eq(s, s) == True{} : Bool}:
  %Equal.sym((String & String) & Cmp, String.cmp(s, s), ((s, s), EQ{}), hub_str_cmp_refl(s)) : {Cmp.is_eq(Pair.snd(String & String, Cmp, _)) == True{} : Bool}
  {==}

# ---- L4 support: a delivery attempt never leaves its agent stuck ----

def hub_rt_put_find.pick(c: Bool, +h: HX.Rt, +t: List<&2, HX.Rt>, +n: String, +run: HM.Run, +w: Bool, +tm: Maybe<&2, Nat>,
    +e: {String.eq(HX.Rt.name(h), n) == c : Bool}, ih: {HC.rt_find(HC.rt_put(t, HX.Rt{n, run, w, tm}), n) == HX.Rt{n, run, w, tm} : HX.Rt})
    -> {HC.rt_find(Bool.pick(List<&2, HX.Rt>, String.eq(HX.Rt.name(h), n), HX.Rt{n, run, w, tm} <> t, h <> HC.rt_put(t, HX.Rt{n, run, w, tm})), n) == HX.Rt{n, run, w, tm} : HX.Rt}:
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HX.Rt.name(h), n), True{}, e) : {HC.rt_find(Bool.pick(List<&2, HX.Rt>, _, HX.Rt{n, run, w, tm} <> t, h <> HC.rt_put(t, HX.Rt{n, run, w, tm})), n) == HX.Rt{n, run, w, tm} : HX.Rt}
      %Equal.sym(Bool, String.eq(n, n), True{}, hub_str_eq_refl(n)) : {Bool.pick(HX.Rt, _, HX.Rt{n, run, w, tm}, HC.rt_find(t, n)) == HX.Rt{n, run, w, tm} : HX.Rt}
      {==}
    case False{}:
      %Equal.sym(Bool, String.eq(HX.Rt.name(h), n), False{}, e) : {HC.rt_find(Bool.pick(List<&2, HX.Rt>, _, HX.Rt{n, run, w, tm} <> t, h <> HC.rt_put(t, HX.Rt{n, run, w, tm})), n) == HX.Rt{n, run, w, tm} : HX.Rt}
      %Equal.sym(Bool, String.eq(HX.Rt.name(h), n), False{}, e) : {Bool.pick(HX.Rt, _, h, HC.rt_find(HC.rt_put(t, HX.Rt{n, run, w, tm}), n)) == HX.Rt{n, run, w, tm} : HX.Rt}
      ih

# putting an agent's runtime state and reading it back gives it
def hub_rt_put_find(+xs: List<&2, HX.Rt>, +n: String, +run: HM.Run, +w: Bool, +tm: Maybe<&2, Nat>)
    -> {HC.rt_find(HC.rt_put(xs, HX.Rt{n, run, w, tm}), n) == HX.Rt{n, run, w, tm} : HX.Rt}:
  match xs:
    case Nil{}:
      %Equal.sym(Bool, String.eq(n, n), True{}, hub_str_eq_refl(n)) : {Bool.pick(HX.Rt, _, HX.Rt{n, run, w, tm}, HC.rt_find(Nil{}, n)) == HX.Rt{n, run, w, tm} : HX.Rt}
      {==}
    case Con{h, t}:
      hub_rt_put_find.pick(String.eq(HX.Rt.name(h), n), h, t, n, run, w, tm, {==}, hub_rt_put_find(t, n, run, w, tm))

def hub_p1(-o: HM.St, p: HX.Cx<o> & List<&2, J.Json>) -> HX.Cx<o>:
  match p:
    case (cx, a):
      cx

def hub_ag(-o: HM.St, +cx: HX.Cx<o>) -> List<&2, HM.Agent>:
  HM.St.agents(HX.d(o, cx))

def hub_ag_notes(s: HM.St) -> {HM.St.agents(HM.apply(s, HM.ENotesFlushed{})) == HM.St.agents(s) : List<&2, HM.Agent>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      {==}

def hub_ag_flush(-o: HM.St, f: Bool, +cx: HX.Cx<o>) -> {hub_ag(o, HC.pump.flush(o, f, cx)) == hub_ag(o, cx) : List<&2, HM.Agent>}:
  match f:
    case True{}:
      Equal.trans(List<&2, HM.Agent>, hub_ag(o, HC.pump.flush(o, True{}, cx)), HM.St.agents(HM.apply(HX.d(o, cx), HM.ENotesFlushed{})), hub_ag(o, cx),
        Equal.cong(HM.St, List<&2, HM.Agent>, s => HM.St.agents(s), HX.d(o, HC.pump.flush(o, True{}, cx)), HM.apply(HX.d(o, cx), HM.ENotesFlushed{}),
          hub_d_emit(o, cx, HM.ENotesFlushed{}, Unit{}, Unit{}, Unit{})),
        hub_ag_notes(HX.d(o, cx)))
    case False{}:
      {==}

def hub_ag_mark.go(-o: HM.St, +cx: HX.Cx<o>, +id: Nat, c: Bool, +e: {HX.queued_at(HX.d(o, cx), id) == c : Bool})
    -> {hub_ag(o, HC.mark_delivered.go(o, cx, id, c, e)) == hub_ag(o, cx) : List<&2, HM.Agent>}:
  match c:
    case True{}:
      Equal.trans(List<&2, HM.Agent>, hub_ag(o, HC.mark_delivered.go(o, cx, id, True{}, e)), HM.St.agents(HM.apply(HX.d(o, cx), HM.EMsgState{id, HM.Delivered{}})), hub_ag(o, cx),
        Equal.cong(HM.St, List<&2, HM.Agent>, s => HM.St.agents(s), HX.d(o, HC.mark_delivered.go(o, cx, id, True{}, e)), HM.apply(HX.d(o, cx), HM.EMsgState{id, HM.Delivered{}}),
          hub_d_emit(o, cx, HM.EMsgState{id, HM.Delivered{}}, Unit{}, Unit{}, e)),
        hub_agents_state(HX.d(o, cx), id, HM.Delivered{}))
    case False{}:
      {==}

def hub_ag_mark(-o: HM.St, +cx: HX.Cx<o>, +id: Nat) -> {hub_ag(o, HC.mark_delivered(o, cx, id)) == hub_ag(o, cx) : List<&2, HM.Agent>}:
  hub_ag_mark.go(o, cx, id, HX.queued_at(HX.d(o, cx), id), {==})

def hub_ag_line(-o: HM.St, +cx: HX.Cx<o>, +a: String, +k: String, +t: String) -> {hub_ag(o, HC.line(o, cx, a, k, t)) == hub_ag(o, cx) : List<&2, HM.Agent>}:
  Equal.cong(HM.St, List<&2, HM.Agent>, s => HM.St.agents(s), HX.d(o, HC.line(o, cx, a, k, t)), HX.d(o, cx), hub_d_line(o, cx, a, k, t))

def hub_ag_pick(-o: HM.St, c: Bool, +x: HX.Cx<o>, +y: HX.Cx<o>, -z: HX.Cx<o>, ex: {hub_ag(o, x) == hub_ag(o, z) : List<&2, HM.Agent>},
    ey: {hub_ag(o, y) == hub_ag(o, z) : List<&2, HM.Agent>}) -> {hub_ag(o, Bool.pick(HX.Cx<o>, c, x, y)) == hub_ag(o, z) : List<&2, HM.Agent>}:
  match c:
    case True{}:
      ex
    case False{}:
      ey

# delivering a batch keeps the agents
def hub_ag_deliver(-o: HM.St, +b: List<&2, HM.Msg>, +cx: HX.Cx<o>, +name: String, +acc: List<&2, J.Json>)
    -> {hub_ag(o, hub_p1(o, HC.deliver_each(o, b, cx, name, acc))) == hub_ag(o, cx) : List<&2, HM.Agent>}:
  match b:
    case Nil{}:
      {==}
    case Con{m, rest}:
      +cx2 = HC.mark_delivered(o, cx, HM.Msg.id(m))
      +s = HC.st(o, cx)
      +again = HC.leftover_at(s, HM.Msg.id(m))
      +you = HM.Msg.plain(m) || Maybe.is_some(&2, String, HM.Msg.via(m))
      +cx3 = HC.deliver.line(o, again, you, cx2, name, m)
      +rel = HC.relation(HM.Msg.from(m), HM.Msg.to(m), HC.parent_of(HM.get_agent(s, HM.Msg.from(m))), HC.parent_of(HM.get_agent(s, HM.Msg.to(m))))
      Equal.trans(List<&2, HM.Agent>, hub_ag(o, hub_p1(o, HC.deliver_each(o, rest, cx3, name, HM.snoc(J.Json, acc, J.obj([J.kv("id", J.JNum{HM.Msg.id(m)}), J.kv("rel", J.JStr{rel})]))))),
        hub_ag(o, cx3), hub_ag(o, cx),
        hub_ag_deliver(o, rest, cx3, name, HM.snoc(J.Json, acc, J.obj([J.kv("id", J.JNum{HM.Msg.id(m)}), J.kv("rel", J.JStr{rel})]))),
        Equal.trans(List<&2, HM.Agent>, hub_ag(o, cx3), hub_ag(o, cx2), hub_ag(o, cx),
          Equal.cong(HM.St, List<&2, HM.Agent>, s2 => HM.St.agents(s2), HX.d(o, cx3), HX.d(o, cx2), hub_d_dline(o, again, you, cx2, name, m)),
          hub_ag_mark(o, cx, HM.Msg.id(m))))

def hub_rts(-o: HM.St, +cx: HX.Cx<o>) -> List<&2, HX.Rt>:
  HX.R.rts(HX.Cx.r(o, cx))

def hub_rts_dirty_out(-o: HM.St, cx: HX.Cx<o>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> {hub_rts(o, HC.dirty(o, HC.out(o, cx, kvs))) == hub_rts(o, cx) : List<&2, HX.Rt>}:
  match cx:
    case HX.Cx{c, r}:
      match r:
        case HX.R{a, b, c2, d, e, f, g, h, i, j, k, l}:
          {==}

def hub_rts_set_rt(-o: HM.St, cx: HX.Cx<o>, +x: HX.Rt) -> {hub_rts(o, HC.set_rt(o, cx, x)) == HC.rt_put(hub_rts(o, cx), x) : List<&2, HX.Rt>}:
  match cx:
    case HX.Cx{c, r}:
      match r:
        case HX.R{a, b, c2, d, e, f, g, h, i, j, k, l}:
          {==}

def hub_run_if_agent(-o: HM.St, known: Bool, +cx: HX.Cx<o>, +n: String, +w: Bool, +tm: Maybe<&2, Nat>, +e: {known == True{} : Bool})
    -> {HX.Rt.run(HC.rt_find(hub_rts(o, HC.if_agent(o, known, cx, HX.Rt{n, HM.RBusy{}, w, tm})), n)) == HM.RBusy{} : HM.Run}:
  match known:
    case True{}:
      %Equal.sym(List<&2, HX.Rt>, hub_rts(o, HC.set_rt(o, cx, HX.Rt{n, HM.RBusy{}, w, tm})), HC.rt_put(hub_rts(o, cx), HX.Rt{n, HM.RBusy{}, w, tm}),
          hub_rts_set_rt(o, cx, HX.Rt{n, HM.RBusy{}, w, tm})) : {HX.Rt.run(HC.rt_find(_, n)) == HM.RBusy{} : HM.Run}
      %Equal.sym(HX.Rt, HC.rt_find(HC.rt_put(hub_rts(o, cx), HX.Rt{n, HM.RBusy{}, w, tm}), n), HX.Rt{n, HM.RBusy{}, w, tm},
          hub_rt_put_find(hub_rts(o, cx), n, HM.RBusy{}, w, tm)) : {HX.Rt.run(_) == HM.RBusy{} : HM.Run}
      {==}
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), HM.Run, HX.Rt.run(HC.rt_find(hub_rts(o, HC.if_agent(o, False{}, cx, HX.Rt{n, HM.RBusy{}, w, tm})), n)), HM.RBusy{})

# a new turn marks its agent busy
def hub_send_busy(-o: HM.St, p: HX.Cx<o> & List<&2, J.Json>, +n: String, +ids: List<&2, Nat>, +kvs0: List<&2, Sigma<&2, &2, String, _ => J.Json>>,
    h: {HC.has_agent(o, hub_p1(o, p), n) == True{} : Bool})
    -> {HX.Rt.run(HC.rt_find(hub_rts(o, HC.pump.send(o, False{}, p, n, ids, kvs0)), n)) == HM.RBusy{} : HM.Run}:
  match p:
    case (+cx, +msgs):
      +x = {HX.Rt{n, HM.RBusy{}, HX.Rt.waiting(HC.rt(o, cx, n)), Some{HC.now(o, cx)}} : HX.Rt}
      +cx2 = HC.upd_rt(o, cx, x)
      +kvs = HM.snoc(Sigma<&2, &2, String, _ => J.Json>, kvs0, J.kv("msgs", J.JArr{msgs}))
      %Equal.sym(List<&2, HX.Rt>, hub_rts(o, HC.dirty(o, HC.out(o, cx2, J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"say"}) <> kvs))), hub_rts(o, cx2),
          hub_rts_dirty_out(o, cx2, J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"say"}) <> kvs)) : {HX.Rt.run(HC.rt_find(_, n)) == HM.RBusy{} : HM.Run}
      hub_run_if_agent(o, HC.has_agent(o, cx, n), cx, n, HX.Rt.waiting(HC.rt(o, cx, n)), Some{HC.now(o, cx)}, h)

def hub_has_tr(-o: HM.St, -a: HX.Cx<o>, -b: HX.Cx<o>, -n: String, e: {hub_ag(o, a) == hub_ag(o, b) : List<&2, HM.Agent>},
    h: {HC.has_agent(o, b, n) == True{} : Bool}) -> {HC.has_agent(o, a, n) == True{} : Bool}:
  %Equal.sym(List<&2, HM.Agent>, hub_ag(o, a), hub_ag(o, b), e) : {Maybe.is_some(&2, HM.Agent, HM.agent_named(_, n)) == True{} : Bool}
  h

# a batch to an idle agent starts its turn: the agent ends busy
def hub_batch_idle(-o: HM.St, +m: HM.Msg, +t: List<&2, HM.Msg>, +cx: HX.Cx<o>, +n: String, +im: Bool,
    h: {HC.has_agent(o, cx, n) == True{} : Bool})
    -> {HX.Rt.run(HC.rt_find(hub_rts(o, HC.pump.batch(o, m <> t, cx, n, False{}, im)), n)) == HM.RBusy{} : HM.Run}:
  +b = {m <> t : List<&2, HM.Msg>}
  +notes = Bool.pick(List<&2, String>, im, HM.St.notes(HC.st(o, cx)), Nil{})
  +cx2 = HC.pump.flush(o, Bool.not(List.is_empty(&2, String, notes)), cx)
  hub_send_busy(o, HC.deliver_each(o, b, cx2, n, Nil{}), n, HC.ids_of(b), [J.kv("agent", J.JStr{n}), J.kv("notes", J.JArr{HC.jstrs(notes)}), J.kv("status", J.JBool{im && HC.any_from_user(b)})],
    hub_has_tr(o, hub_p1(o, HC.deliver_each(o, b, cx2, n, Nil{})), cx, n,
      Equal.trans(List<&2, HM.Agent>, hub_ag(o, hub_p1(o, HC.deliver_each(o, b, cx2, n, Nil{}))), hub_ag(o, cx2), hub_ag(o, cx),
        hub_ag_deliver(o, b, cx2, n, Nil{}), hub_ag_flush(o, Bool.not(List.is_empty(&2, String, notes)), cx)),
      h))

def hub_rts_emit(-o: HM.St, cx: HX.Cx<o>, +e: HM.Ev, +k: HX.FreshEv(HX.d(o, cx), e), +kq: HX.QuietEv(e), +kv: HX.DelivEv(HX.d(o, cx), e))
    -> {hub_rts(o, HC.emit(o, cx, e, k, kq, kv)) == hub_rts(o, cx) : List<&2, HX.Rt>}:
  match cx:
    case HX.Cx{c, r}:
      match r:
        case HX.R{a, b, c2, d, e2, f, g, h, i, j, k2, l}:
          {==}

def hub_rts_line(-o: HM.St, cx: HX.Cx<o>, +a: String, +k: String, +t: String) -> {hub_rts(o, HC.line(o, cx, a, k, t)) == hub_rts(o, cx) : List<&2, HX.Rt>}:
  match cx:
    case HX.Cx{c, r}:
      {==}

def hub_rts_dline(-o: HM.St, again: Bool, you: Bool, +cx: HX.Cx<o>, +name: String, +m: HM.Msg)
    -> {hub_rts(o, HC.deliver.line(o, again, you, cx, name, m)) == hub_rts(o, cx) : List<&2, HX.Rt>}:
  match again you:
    case True{} _:
      {==}
    case False{} True{}:
      hub_rts_line(o, cx, name, "you", HM.Msg.text(m))
    case False{} False{}:
      hub_rts_line(o, cx, name, "msg-in", HC.cat([HM.Msg.from(m), " ", HC.mid(HM.Msg.id(m)), " : ", HM.Msg.text(m)]))

def hub_rts_mark.go(-o: HM.St, +cx: HX.Cx<o>, +id: Nat, c: Bool, +e: {HX.queued_at(HX.d(o, cx), id) == c : Bool})
    -> {hub_rts(o, HC.mark_delivered.go(o, cx, id, c, e)) == hub_rts(o, cx) : List<&2, HX.Rt>}:
  match c:
    case True{}:
      hub_rts_emit(o, cx, HM.EMsgState{id, HM.Delivered{}}, Unit{}, Unit{}, e)
    case False{}:
      {==}

def hub_rts_flush(-o: HM.St, f: Bool, +cx: HX.Cx<o>) -> {hub_rts(o, HC.pump.flush(o, f, cx)) == hub_rts(o, cx) : List<&2, HX.Rt>}:
  match f:
    case True{}:
      hub_rts_emit(o, cx, HM.ENotesFlushed{}, Unit{}, Unit{}, Unit{})
    case False{}:
      {==}

def hub_rts_pick(-o: HM.St, c: Bool, +x: HX.Cx<o>, +y: HX.Cx<o>, -z: HX.Cx<o>, ex: {hub_rts(o, x) == hub_rts(o, z) : List<&2, HX.Rt>},
    ey: {hub_rts(o, y) == hub_rts(o, z) : List<&2, HX.Rt>}) -> {hub_rts(o, Bool.pick(HX.Cx<o>, c, x, y)) == hub_rts(o, z) : List<&2, HX.Rt>}:
  match c:
    case True{}:
      ex
    case False{}:
      ey

def hub_rts_deliver(-o: HM.St, +b: List<&2, HM.Msg>, +cx: HX.Cx<o>, +name: String, +acc: List<&2, J.Json>)
    -> {hub_rts(o, hub_p1(o, HC.deliver_each(o, b, cx, name, acc))) == hub_rts(o, cx) : List<&2, HX.Rt>}:
  match b:
    case Nil{}:
      {==}
    case Con{m, rest}:
      +cx2 = HC.mark_delivered(o, cx, HM.Msg.id(m))
      +s = HC.st(o, cx)
      +again = HC.leftover_at(s, HM.Msg.id(m))
      +you = HM.Msg.plain(m) || Maybe.is_some(&2, String, HM.Msg.via(m))
      +cx3 = HC.deliver.line(o, again, you, cx2, name, m)
      +rel = HC.relation(HM.Msg.from(m), HM.Msg.to(m), HC.parent_of(HM.get_agent(s, HM.Msg.from(m))), HC.parent_of(HM.get_agent(s, HM.Msg.to(m))))
      Equal.trans(List<&2, HX.Rt>, hub_rts(o, hub_p1(o, HC.deliver_each(o, rest, cx3, name, HM.snoc(J.Json, acc, J.obj([J.kv("id", J.JNum{HM.Msg.id(m)}), J.kv("rel", J.JStr{rel})]))))),
        hub_rts(o, cx3), hub_rts(o, cx),
        hub_rts_deliver(o, rest, cx3, name, HM.snoc(J.Json, acc, J.obj([J.kv("id", J.JNum{HM.Msg.id(m)}), J.kv("rel", J.JStr{rel})]))),
        Equal.trans(List<&2, HX.Rt>, hub_rts(o, cx3), hub_rts(o, cx2), hub_rts(o, cx),
          hub_rts_dline(o, again, you, cx2, name, m),
          hub_rts_mark.go(o, cx, HM.Msg.id(m), HX.queued_at(HX.d(o, cx), HM.Msg.id(m)), {==})))

def hub_rts_steer(-o: HM.St, p: HX.Cx<o> & List<&2, J.Json>, +n: String, +ids: List<&2, Nat>, +kvs0: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> {hub_rts(o, HC.pump.send(o, True{}, p, n, ids, kvs0)) == hub_rts(o, hub_p1(o, p)) : List<&2, HX.Rt>}:
  match p:
    case (cx, msgs):
      match cx:
        case HX.Cx{c, r}:
          match r:
            case HX.R{a, b, c2, d, e, f, g, h, i, j, k, l}:
              {==}

# a batch to a busy agent (steering) keeps every runtime state
def hub_batch_busy(-o: HM.St, +b: List<&2, HM.Msg>, +cx: HX.Cx<o>, +n: String, +im: Bool)
    -> {hub_rts(o, HC.pump.batch(o, b, cx, n, True{}, im)) == hub_rts(o, cx) : List<&2, HX.Rt>}:
  match b:
    case Nil{}:
      {==}
    case Con{m, t}:
      +bb = {m <> t : List<&2, HM.Msg>}
      +notes = Bool.pick(List<&2, String>, im, HM.St.notes(HC.st(o, cx)), Nil{})
      +cx2 = HC.pump.flush(o, Bool.not(List.is_empty(&2, String, notes)), cx)
      Equal.trans(List<&2, HX.Rt>, hub_rts(o, HC.pump.send(o, True{}, HC.deliver_each(o, bb, cx2, n, Nil{}), n, HC.ids_of(bb),
          [J.kv("agent", J.JStr{n}), J.kv("notes", J.JArr{HC.jstrs(notes)}), J.kv("status", J.JBool{im && HC.any_from_user(bb)})])),
        hub_rts(o, hub_p1(o, HC.deliver_each(o, bb, cx2, n, Nil{}))), hub_rts(o, cx),
        hub_rts_steer(o, HC.deliver_each(o, bb, cx2, n, Nil{}), n, HC.ids_of(bb),
          [J.kv("agent", J.JStr{n}), J.kv("notes", J.JArr{HC.jstrs(notes)}), J.kv("status", J.JBool{im && HC.any_from_user(bb)})]),
        Equal.trans(List<&2, HX.Rt>, hub_rts(o, hub_p1(o, HC.deliver_each(o, bb, cx2, n, Nil{}))), hub_rts(o, cx2), hub_rts(o, cx),
          hub_rts_deliver(o, bb, cx2, n, Nil{}), hub_rts_flush(o, Bool.not(List.is_empty(&2, String, notes)), cx)))

def hub_and_false_r(a: Bool) -> {Bool.and(a, False{}) == False{} : Bool}:
  match a:
    case True{}:
      {==}
    case False{}:
      {==}

# an agent that is not idle is not stuck
def hub_stuck_run(+s: HM.St, +rts: List<&2, HX.Rt>, +n: String, -r: HM.Run, e: {HX.Rt.run(HC.rt_find(rts, n)) == r : HM.Run},
    ni: {HC.is_idle(r) == False{} : Bool}) -> {HI.stuck(s, rts, n) == False{} : Bool}:
  %Equal.sym(HM.Run, HX.Rt.run(HC.rt_find(rts, n)), r, e) : {Bool.and(HC.is_idle(_), HI.stuck.agent(HM.get_agent(s, n), s, n)) == False{} : Bool}
  %Equal.sym(Bool, HC.is_idle(r), False{}, ni) : {Bool.and(_, HI.stuck.agent(HM.get_agent(s, n), s, n)) == False{} : Bool}
  {==}

# an agent whose own record says it is not stuck is not stuck
def hub_stuck_agent(+s: HM.St, +rts: List<&2, HX.Rt>, +n: String, e: {HI.stuck.agent(HM.get_agent(s, n), s, n) == False{} : Bool})
    -> {HI.stuck(s, rts, n) == False{} : Bool}:
  %Equal.sym(Bool, HI.stuck.agent(HM.get_agent(s, n), s, n), False{}, e) : {Bool.and(HC.is_idle(HX.Rt.run(HC.rt_find(rts, n))), _) == False{} : Bool}
  hub_and_false_r(HC.is_idle(HX.Rt.run(HC.rt_find(rts, n))))

def hub_pw.idle.nil(-o: HM.St, +a: HM.Agent, +cx: HX.Cx<o>, +n: String,
    e: {HM.get_agent(HX.d(o, cx), n) == Some{a} : Maybe<&2, HM.Agent>}, ec: {HC.is_active(HM.Agent.life(a)) == True{} : Bool},
    eq: {HM.queued_for(HX.d(o, cx), n) == Nil{} : List<&2, HM.Msg>}) -> {HI.stuck.agent(HM.get_agent(HX.d(o, cx), n), HX.d(o, cx), n) == False{} : Bool}:
  %Equal.sym(Maybe<&2, HM.Agent>, HM.get_agent(HX.d(o, cx), n), Some{a}, e) : {HI.stuck.agent(_, HX.d(o, cx), n) == False{} : Bool}
  %Equal.sym(Bool, HC.is_active(HM.Agent.life(a)), True{}, ec) : {Bool.and(_, Bool.not(List.is_empty(&2, HM.Msg, HM.queued_for(HX.d(o, cx), n)))) == False{} : Bool}
  %Equal.sym(List<&2, HM.Msg>, HM.queued_for(HX.d(o, cx), n), Nil{}, eq) : {Bool.and(True{}, Bool.not(List.is_empty(&2, HM.Msg, _))) == False{} : Bool}
  {==}

def hub_has_some(-o: HM.St, -a: HM.Agent, +cx: HX.Cx<o>, +n: String, e: {HM.get_agent(HX.d(o, cx), n) == Some{a} : Maybe<&2, HM.Agent>})
    -> {HC.has_agent(o, cx, n) == True{} : Bool}:
  %Equal.sym(Maybe<&2, HM.Agent>, HM.get_agent(HX.d(o, cx), n), Some{a}, e) : {Maybe.is_some(&2, HM.Agent, _) == True{} : Bool}
  {==}

def hub_pw.idle(-o: HM.St, q: List<&2, HM.Msg>, +a: HM.Agent, +cx: HX.Cx<o>, +n: String,
    +e: {HM.get_agent(HX.d(o, cx), n) == Some{a} : Maybe<&2, HM.Agent>}, +ec: {HC.is_active(HM.Agent.life(a)) == True{} : Bool},
    +er: {HX.Rt.run(HC.rt_find(hub_rts(o, cx), n)) == HM.RIdle{} : HM.Run}, +eq: {HM.queued_for(HX.d(o, cx), n) == q : List<&2, HM.Msg>})
    -> {HI.stuck(HX.d(o, HC.pump.batch(o, q, cx, n, False{}, HM.Agent.is_main(a))), hub_rts(o, HC.pump.batch(o, q, cx, n, False{}, HM.Agent.is_main(a))), n) == False{} : Bool}:
  match q:
    case Nil{}:
      hub_stuck_agent(HX.d(o, cx), hub_rts(o, cx), n, hub_pw.idle.nil(o, a, cx, n, e, ec, eq))
    case Con{+m, +t}:
      hub_stuck_run(HX.d(o, HC.pump.batch(o, m <> t, cx, n, False{}, HM.Agent.is_main(a))), hub_rts(o, HC.pump.batch(o, m <> t, cx, n, False{}, HM.Agent.is_main(a))), n, HM.RBusy{},
        hub_batch_idle(o, m, t, cx, n, HM.Agent.is_main(a), hub_has_some(o, a, cx, n, e)), {==})

def hub_pw.run(-o: HM.St, r: HM.Run, +a: HM.Agent, +cx: HX.Cx<o>, +n: String,
    +e: {HM.get_agent(HX.d(o, cx), n) == Some{a} : Maybe<&2, HM.Agent>}, +ec: {HC.is_active(HM.Agent.life(a)) == True{} : Bool},
    +er: {HX.Rt.run(HC.rt_find(hub_rts(o, cx), n)) == r : HM.Run})
    -> {HI.stuck(HX.d(o, HC.pump.run(o, r, cx, n, HM.queued_for(HX.d(o, cx), n), HM.Agent.is_main(a))), hub_rts(o, HC.pump.run(o, r, cx, n, HM.queued_for(HX.d(o, cx), n), HM.Agent.is_main(a))), n) == False{} : Bool}:
  match r:
    case HM.RDown{}:
      hub_stuck_run(HX.d(o, cx), hub_rts(o, cx), n, HM.RDown{}, er, {==})
    case HM.RStarting{}:
      hub_stuck_run(HX.d(o, cx), hub_rts(o, cx), n, HM.RStarting{}, er, {==})
    case HM.RBusy{}:
      +fin = HC.pump.batch(o, HC.not_queued_mode(HM.queued_for(HX.d(o, cx), n)), cx, n, True{}, HM.Agent.is_main(a))
      hub_stuck_run(HX.d(o, fin), hub_rts(o, fin), n, HM.RBusy{},
        Equal.trans(HM.Run, HX.Rt.run(HC.rt_find(hub_rts(o, fin), n)), HX.Rt.run(HC.rt_find(hub_rts(o, cx), n)), HM.RBusy{},
          Equal.cong(List<&2, HX.Rt>, HM.Run, xs => HX.Rt.run(HC.rt_find(xs, n)), hub_rts(o, fin), hub_rts(o, cx),
            hub_batch_busy(o, HC.not_queued_mode(HM.queued_for(HX.d(o, cx), n)), cx, n, HM.Agent.is_main(a))), er), {==})
    case HM.RIdle{}:
      hub_pw.idle(o, HM.queued_for(HX.d(o, cx), n), a, cx, n, e, ec, er, {==})

def hub_pw.inactive(-o: HM.St, +a: HM.Agent, +cx: HX.Cx<o>, +n: String,
    e: {HM.get_agent(HX.d(o, cx), n) == Some{a} : Maybe<&2, HM.Agent>}, ec: {HC.is_active(HM.Agent.life(a)) == False{} : Bool}) -> {HI.stuck.agent(HM.get_agent(HX.d(o, cx), n), HX.d(o, cx), n) == False{} : Bool}:
  %Equal.sym(Maybe<&2, HM.Agent>, HM.get_agent(HX.d(o, cx), n), Some{a}, e) : {HI.stuck.agent(_, HX.d(o, cx), n) == False{} : Bool}
  %Equal.sym(Bool, HC.is_active(HM.Agent.life(a)), False{}, ec) : {Bool.and(_, Bool.not(List.is_empty(&2, HM.Msg, HM.queued_for(HX.d(o, cx), n)))) == False{} : Bool}
  {==}

def hub_pw.active(-o: HM.St, c: Bool, +a: HM.Agent, +cx: HX.Cx<o>, +n: String,
    +e: {HM.get_agent(HX.d(o, cx), n) == Some{a} : Maybe<&2, HM.Agent>}, +ec: {HC.is_active(HM.Agent.life(a)) == c : Bool})
    -> {HI.stuck(HX.d(o, HC.pump.active(o, c, cx, n, HM.Agent.is_main(a))), hub_rts(o, HC.pump.active(o, c, cx, n, HM.Agent.is_main(a))), n) == False{} : Bool}:
  match c:
    case True{}:
      hub_pw.run(o, HX.Rt.run(HC.rt_find(hub_rts(o, cx), n)), a, cx, n, e, ec, {==})
    case False{}:
      hub_stuck_agent(HX.d(o, cx), hub_rts(o, cx), n, hub_pw.inactive(o, a, cx, n, e, ec))

def hub_pw.none(-o: HM.St, +cx: HX.Cx<o>, +n: String, e: {HM.get_agent(HX.d(o, cx), n) == None{} : Maybe<&2, HM.Agent>}) -> {HI.stuck.agent(HM.get_agent(HX.d(o, cx), n), HX.d(o, cx), n) == False{} : Bool}:
  %Equal.sym(Maybe<&2, HM.Agent>, HM.get_agent(HX.d(o, cx), n), None{}, e) : {HI.stuck.agent(_, HX.d(o, cx), n) == False{} : Bool}
  {==}

def hub_pw.agent(-o: HM.St, m: Maybe<&2, HM.Agent>, +cx: HX.Cx<o>, +n: String, +e: {HM.get_agent(HX.d(o, cx), n) == m : Maybe<&2, HM.Agent>})
    -> {HI.stuck(HX.d(o, HC.pump.agent(o, m, cx, n)), hub_rts(o, HC.pump.agent(o, m, cx, n)), n) == False{} : Bool}:
  match m:
    case None{}:
      hub_stuck_agent(HX.d(o, cx), hub_rts(o, cx), n, hub_pw.none(o, cx, n, e))
    case Some{+a}:
      hub_pw.active(o, HC.is_active(HM.Agent.life(a)), a, cx, n, e, {==})

# a delivery attempt never leaves its agent idle with queued messages
def hub_pump_wakes(-o: HM.St, +cx: HX.Cx<o>, +n: String) -> {HI.stuck(HX.d(o, HC.pump(o, cx, n)), hub_rts(o, HC.pump(o, cx, n)), n) == False{} : Bool}:
  hub_pw.agent(o, HM.get_agent(HX.d(o, cx), n), cx, n, {==})


# ---- L4 support: string equality is sound (equal strings are the same) ----

def hub_cdisc(c: Cmp) -> Type:
  match c:
    case EQ{}:
      Unit
    case _:
      Empty

def hub_lt_eq(e: {LT{} == EQ{} : Cmp}) -> Empty:
  %Equal.sym(Cmp, LT{}, EQ{}, e) : hub_cdisc(_)
  Unit{}

def hub_gt_eq(e: {GT{} == EQ{} : Cmp}) -> Empty:
  %Equal.sym(Cmp, GT{}, EQ{}, e) : hub_cdisc(_)
  Unit{}

def hub_bool_sound(a: Bool, b: Bool, e: {Bool.cmp(a, b) == EQ{} : Cmp}) -> {a == b : Bool}:
  match a b:
    case False{} False{}:
      {==}
    case True{} True{}:
      {==}
    case False{} True{}:
      hub_absurd_eq(hub_lt_eq(e), Bool, False{}, True{})
    case True{} False{}:
      hub_absurd_eq(hub_gt_eq(e), Bool, True{}, False{})

def hub_word_sound.fin(a: Bool, b: Bool, t: Cmp, +et: {t == EQ{} : Cmp}, e: {Word.cmp.fin(a, b, t) == EQ{} : Cmp}) -> {a == b : Bool}:
  match t:
    case LT{}:
      hub_absurd_eq(hub_lt_eq(et), Bool, a, b)
    case GT{}:
      hub_absurd_eq(hub_gt_eq(et), Bool, a, b)
    case EQ{}:
      hub_bool_sound(a, b, e)

def hub_word_tail_eq(a: Bool, b: Bool, t: Cmp, e: {Word.cmp.fin(a, b, t) == EQ{} : Cmp}) -> {t == EQ{} : Cmp}:
  match t:
    case LT{}:
      e
    case GT{}:
      e
    case EQ{}:
      {==}

# equal words are the same word
def hub_word_sound(+n: Nat, w1: Word(n), w2: Word(n), +e: {Word.cmp(n, w1, w2) == EQ{} : Cmp}) -> {w1 == w2 : Word(n)}:
  match n:
    case 0n:
      match w1 w2:
        case WNil{} WNil{}:
          {==}
    case 1n+p:
      match w1 w2:
        case WCon{+a, +at} WCon{+b, +bt}:
          +et = hub_word_tail_eq(a, b, Word.cmp(p, at, bt), e)
          +ht = hub_word_sound(p, at, bt, et)
          +hb = hub_word_sound.fin(a, b, Word.cmp(p, at, bt), et, e)
          %hb : {WCon{a, at} == WCon{_, bt} : Word(1n+p)}
          %ht : {WCon{a, at} == WCon{a, _} : Word(1n+p)}
          {==}

def hub_ccomb(c: Cmp, r: Cmp) -> Cmp:
  match c:
    case EQ{}:
      r
    case LT{}:
      LT{}
    case GT{}:
      GT{}

def hub_ccmp(a: Char, b: Char) -> Cmp:
  match a b:
    case Chr{x} Chr{y}:
      U32.cmp(x, y)

# the comparison of String.cmp, on its own
def hub_cmpc(a: String, b: String) -> Cmp:
  match a b:
    case SNil{} SNil{}:
      EQ{}
    case SNil{} SCon{h, t}:
      LT{}
    case SCon{h, t} SNil{}:
      GT{}
    case SCon{h1, t1} SCon{h2, t2}:
      hub_ccomb(hub_ccmp(h1, h2), hub_cmpc(t1, t2))

def hub_scmp_shape.fin(+x: U32, +y: U32, +t1: String, +t2: String, c: Cmp, +ec: {U32.cmp(x, y) == c : Cmp},
    ih: {String.cmp(t1, t2) == ((t1, t2), hub_cmpc(t1, t2)) : (String & String) & Cmp})
    -> {String.cmp.fin(t1, t2, ((Chr{x}, Chr{y}), U32.cmp(x, y))) == ((SCon{Chr{x}, t1}, SCon{Chr{y}, t2}), hub_ccomb(U32.cmp(x, y), hub_cmpc(t1, t2))) : (String & String) & Cmp}:
  match c:
    case LT{}:
      %Equal.sym(Cmp, U32.cmp(x, y), LT{}, ec) : {String.cmp.fin(t1, t2, ((Chr{x}, Chr{y}), _)) == ((SCon{Chr{x}, t1}, SCon{Chr{y}, t2}), hub_ccomb(_, hub_cmpc(t1, t2))) : (String & String) & Cmp}
      {==}
    case GT{}:
      %Equal.sym(Cmp, U32.cmp(x, y), GT{}, ec) : {String.cmp.fin(t1, t2, ((Chr{x}, Chr{y}), _)) == ((SCon{Chr{x}, t1}, SCon{Chr{y}, t2}), hub_ccomb(_, hub_cmpc(t1, t2))) : (String & String) & Cmp}
      {==}
    case EQ{}:
      %Equal.sym(Cmp, U32.cmp(x, y), EQ{}, ec) : {String.cmp.fin(t1, t2, ((Chr{x}, Chr{y}), _)) == ((SCon{Chr{x}, t1}, SCon{Chr{y}, t2}), hub_ccomb(_, hub_cmpc(t1, t2))) : (String & String) & Cmp}
      %Equal.sym((String & String) & Cmp, String.cmp(t1, t2), ((t1, t2), hub_cmpc(t1, t2)), ih) : {String.cmp.rec(Chr{x}, Chr{y}, _) == ((SCon{Chr{x}, t1}, SCon{Chr{y}, t2}), hub_cmpc(t1, t2)) : (String & String) & Cmp}
      {==}

# String.cmp hands its strings back with hub_cmpc
def hub_scmp_shape(+a: String, +b: String) -> {String.cmp(a, b) == ((a, b), hub_cmpc(a, b)) : (String & String) & Cmp}:
  match a b:
    case SNil{} SNil{}:
      {==}
    case SNil{} SCon{h, t}:
      {==}
    case SCon{h, t} SNil{}:
      {==}
    case SCon{Chr{x}, t1} SCon{Chr{y}, t2}:
      hub_scmp_shape.fin(x, y, t1, t2, U32.cmp(x, y), {==}, hub_scmp_shape(t1, t2))

def hub_cmpc_tail(c: Cmp, -r: Cmp, e: {Cmp.is_eq(hub_ccomb(c, r)) == True{} : Bool}) -> {Cmp.is_eq(r) == True{} : Bool}:
  match c:
    case EQ{}:
      e
    case LT{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, Cmp.is_eq(r), True{})
    case GT{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, Cmp.is_eq(r), True{})

def hub_cmpc_head(c: Cmp, -r: Cmp, e: {Cmp.is_eq(hub_ccomb(c, r)) == True{} : Bool}) -> {c == EQ{} : Cmp}:
  match c:
    case EQ{}:
      {==}
    case LT{}:
      hub_absurd_eq(hub_ft_absurd(e), Cmp, LT{}, EQ{})
    case GT{}:
      hub_absurd_eq(hub_ft_absurd(e), Cmp, GT{}, EQ{})

def hub_u32_sound(x: U32, y: U32, e: {U32.cmp(x, y) == EQ{} : Cmp}) -> {Chr{x} == Chr{y} : Char}:
  match x y:
    case U32{w1} U32{w2}:
      %hub_word_sound(32n, w1, w2, e) : {Chr{U32{w1}} == Chr{U32{_}} : Char}
      {==}

# a comparison that finds the strings equal means they are the same
def hub_cmpc_sound(+a: String, +b: String, +e: {Cmp.is_eq(hub_cmpc(a, b)) == True{} : Bool}) -> {a == b : String}:
  match a b:
    case SNil{} SNil{}:
      {==}
    case SNil{} SCon{h, t}:
      hub_absurd_eq(hub_ft_absurd(e), String, SNil{}, SCon{h, t})
    case SCon{h, t} SNil{}:
      hub_absurd_eq(hub_ft_absurd(e), String, SCon{h, t}, SNil{})
    case SCon{Chr{x}, t1} SCon{Chr{y}, t2}:
      +ht = hub_cmpc_sound(t1, t2, hub_cmpc_tail(U32.cmp(x, y), hub_cmpc(t1, t2), e))
      +hh = hub_u32_sound(x, y, hub_cmpc_head(U32.cmp(x, y), hub_cmpc(t1, t2), e))
      %hh : {SCon{Chr{x}, t1} == SCon{_, t2} : String}
      %ht : {SCon{Chr{x}, t1} == SCon{Chr{x}, _} : String}
      {==}

def hub_str_eq_sound.k(+a: String, +b: String, e: {String.eq(a, b) == True{} : Bool}) -> {Cmp.is_eq(hub_cmpc(a, b)) == True{} : Bool}:
  %hub_scmp_shape(a, b) : {Cmp.is_eq(Pair.snd(String & String, Cmp, _)) == True{} : Bool}
  e

# String.eq is sound: strings it finds equal are the same string
def hub_str_eq_sound(+a: String, +b: String, e: {String.eq(a, b) == True{} : Bool}) -> {a == b : String}:
  hub_cmpc_sound(a, b, hub_str_eq_sound.k(a, b, e))

# ---- L4 support: a delivery only shrinks queues ----

def hub_is_q_deliv(st: HM.MSt) -> {HM.is_queued(HM.MRec.st(HM.rec_op(HM.MRec{HM.Msg{0n, 0n, "", "", None{}, False{}, False{}, "", 0n, False{}, False{}, None{}}, st, False{}}, HM.OSt{HM.Delivered{}}))) == False{} : Bool}:
  {==}

def hub_rt_put_other.pick(c: Bool, +h: HX.Rt, +t: List<&2, HX.Rt>, +a: String, +n: String, +run: HM.Run, +w: Bool, +tm: Maybe<&2, Nat>,
    +e: {String.eq(HX.Rt.name(h), a) == c : Bool}, +ne: {String.eq(a, n) == False{} : Bool},
    ih: {HC.rt_find(HC.rt_put(t, HX.Rt{a, run, w, tm}), n) == HC.rt_find(t, n) : HX.Rt})
    -> {HC.rt_find(Bool.pick(List<&2, HX.Rt>, String.eq(HX.Rt.name(h), a), HX.Rt{a, run, w, tm} <> t, h <> HC.rt_put(t, HX.Rt{a, run, w, tm})), n) == HC.rt_find(h <> t, n) : HX.Rt}:
  match c:
    case True{}:
      +hn = hub_str_eq_sound(HX.Rt.name(h), a, e)
      %Equal.sym(Bool, String.eq(HX.Rt.name(h), a), True{}, e) : {HC.rt_find(Bool.pick(List<&2, HX.Rt>, _, HX.Rt{a, run, w, tm} <> t, h <> HC.rt_put(t, HX.Rt{a, run, w, tm})), n) == HC.rt_find(h <> t, n) : HX.Rt}
      %Equal.sym(Bool, String.eq(a, n), False{}, ne) : {Bool.pick(HX.Rt, _, HX.Rt{a, run, w, tm}, HC.rt_find(t, n)) == HC.rt_find(h <> t, n) : HX.Rt}
      %Equal.sym(String, HX.Rt.name(h), a, hn) : {HC.rt_find(t, n) == Bool.pick(HX.Rt, String.eq(_, n), h, HC.rt_find(t, n)) : HX.Rt}
      %Equal.sym(Bool, String.eq(a, n), False{}, ne) : {HC.rt_find(t, n) == Bool.pick(HX.Rt, _, h, HC.rt_find(t, n)) : HX.Rt}
      {==}
    case False{}:
      %Equal.sym(Bool, String.eq(HX.Rt.name(h), a), False{}, e) : {HC.rt_find(Bool.pick(List<&2, HX.Rt>, _, HX.Rt{a, run, w, tm} <> t, h <> HC.rt_put(t, HX.Rt{a, run, w, tm})), n) == HC.rt_find(h <> t, n) : HX.Rt}
      Equal.cong(HX.Rt, HX.Rt, y => Bool.pick(HX.Rt, String.eq(HX.Rt.name(h), n), h, y), HC.rt_find(HC.rt_put(t, HX.Rt{a, run, w, tm}), n), HC.rt_find(t, n), ih)

# writing A's runtime state leaves every other agent's as it was
def hub_rt_put_other(+xs: List<&2, HX.Rt>, +a: String, +n: String, +run: HM.Run, +w: Bool, +tm: Maybe<&2, Nat>, +ne: {String.eq(a, n) == False{} : Bool})
    -> {HC.rt_find(HC.rt_put(xs, HX.Rt{a, run, w, tm}), n) == HC.rt_find(xs, n) : HX.Rt}:
  match xs:
    case Nil{}:
      %Equal.sym(Bool, String.eq(a, n), False{}, ne) : {Bool.pick(HX.Rt, _, HX.Rt{a, run, w, tm}, HC.rt_find(Nil{}, n)) == HC.rt_find(Nil{}, n) : HX.Rt}
      {==}
    case Con{h, t}:
      hub_rt_put_other.pick(String.eq(HX.Rt.name(h), a), h, t, a, n, run, w, tm, {==}, ne, hub_rt_put_other(t, a, n, run, w, tm, ne))


# ---- L4: the frame of an agent (what makes it stuck) ----

def hub_q(+s: HM.St, +n: String) -> Bool:
  List.is_empty(&2, HM.Msg, HM.queued_for(s, n))

# the queue of n in s2 is empty whenever it is in s1
def hub_qimp(+s1: HM.St, +s2: HM.St, +n: String) -> Bool:
  Bool.or(Bool.not(hub_q(s1, n)), hub_q(s2, n))

def hub_not_true(q: Bool, e: {q == True{} : Bool}) -> {Bool.not(q) == False{} : Bool}:
  match q:
    case True{}:
      {==}
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, True{}, False{})

# not(q1) is false and q1 => q2: not(q2) is false
def hub_sa.q(q1: Bool, +q2: Bool, +e: {q1 == q1 : Bool}, h: {Bool.not(q1) == False{} : Bool}, qi: {Bool.or(Bool.not(q1), q2) == True{} : Bool})
    -> {Bool.not(q2) == False{} : Bool}:
  match q1:
    case True{}:
      hub_not_true(q2, qi)
    case False{}:
      hub_absurd_eq(hub_tf_absurd(h), Bool, Bool.not(q2), False{})

def hub_sa.e1(x: Bool, -y: Bool, e: {x == True{} : Bool}, h: {Bool.and(x, y) == False{} : Bool}) -> {y == False{} : Bool}:
  match x:
    case True{}:
      h
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, y, False{})

def hub_sa.act(b: Bool, +a: HM.Agent, +s1: HM.St, +s2: HM.St, +n: String, +eb: {HC.is_active(HM.Agent.life(a)) == b : Bool},
    e1: {Bool.and(HC.is_active(HM.Agent.life(a)), Bool.not(hub_q(s1, n))) == False{} : Bool}, qi: {hub_qimp(s1, s2, n) == True{} : Bool})
    -> {Bool.and(HC.is_active(HM.Agent.life(a)), Bool.not(hub_q(s2, n))) == False{} : Bool}:
  match b:
    case False{}:
      %Equal.sym(Bool, HC.is_active(HM.Agent.life(a)), False{}, eb) : {Bool.and(_, Bool.not(hub_q(s2, n))) == False{} : Bool}
      {==}
    case True{}:
      %Equal.sym(Bool, HC.is_active(HM.Agent.life(a)), True{}, eb) : {Bool.and(_, Bool.not(hub_q(s2, n))) == False{} : Bool}
      hub_sa.q(hub_q(s1, n), hub_q(s2, n), {==}, hub_sa.e1(HC.is_active(HM.Agent.life(a)), Bool.not(hub_q(s1, n)), eb, e1), qi)

# an agent not stuck by its record in s1 is not in s2, when its queue can
# only have emptied
def hub_sa(m: Maybe<&2, HM.Agent>, +s1: HM.St, +s2: HM.St, +n: String,
    e1: {HI.stuck.agent(m, s1, n) == False{} : Bool}, qi: {hub_qimp(s1, s2, n) == True{} : Bool})
    -> {HI.stuck.agent(m, s2, n) == False{} : Bool}:
  match m:
    case None{}:
      {==}
    case Some{+a}:
      hub_sa.act(HC.is_active(HM.Agent.life(a)), a, s1, s2, n, {==}, e1, qi)

def hub_frame.idle(b: Bool, +r: HM.Run, m: Maybe<&2, HM.Agent>, +s1: HM.St, +s2: HM.St, +n: String,
    +eb: {HC.is_idle(r) == b : Bool}, h: {Bool.and(HC.is_idle(r), HI.stuck.agent(m, s1, n)) == False{} : Bool},
    qi: {hub_qimp(s1, s2, n) == True{} : Bool}) -> {Bool.and(HC.is_idle(r), HI.stuck.agent(m, s2, n)) == False{} : Bool}:
  match b:
    case False{}:
      %Equal.sym(Bool, HC.is_idle(r), False{}, eb) : {Bool.and(_, HI.stuck.agent(m, s2, n)) == False{} : Bool}
      {==}
    case True{}:
      %Equal.sym(Bool, HC.is_idle(r), True{}, eb) : {Bool.and(_, HI.stuck.agent(m, s2, n)) == False{} : Bool}
      hub_sa(m, s1, s2, n, hub_sa.e1(HC.is_idle(r), HI.stuck.agent(m, s1, n), eb, h), qi)

# an agent not stuck stays so when its run and record are unchanged and
# its queue can only have emptied
def hub_frame(+s1: HM.St, +rts1: List<&2, HX.Rt>, +s2: HM.St, +rts2: List<&2, HX.Rt>, +n: String,
    er: {HX.Rt.run(HC.rt_find(rts2, n)) == HX.Rt.run(HC.rt_find(rts1, n)) : HM.Run},
    ea: {HM.get_agent(s2, n) == HM.get_agent(s1, n) : Maybe<&2, HM.Agent>},
    qi: {hub_qimp(s1, s2, n) == True{} : Bool}, h: {HI.stuck(s1, rts1, n) == False{} : Bool})
    -> {HI.stuck(s2, rts2, n) == False{} : Bool}:
  %Equal.sym(HM.Run, HX.Rt.run(HC.rt_find(rts2, n)), HX.Rt.run(HC.rt_find(rts1, n)), er) : {Bool.and(HC.is_idle(_), HI.stuck.agent(HM.get_agent(s2, n), s2, n)) == False{} : Bool}
  %Equal.sym(Maybe<&2, HM.Agent>, HM.get_agent(s2, n), HM.get_agent(s1, n), ea) : {Bool.and(HC.is_idle(HX.Rt.run(HC.rt_find(rts1, n))), HI.stuck.agent(_, s2, n)) == False{} : Bool}
  hub_frame.idle(HC.is_idle(HX.Rt.run(HC.rt_find(rts1, n))), HX.Rt.run(HC.rt_find(rts1, n)), HM.get_agent(s1, n), s1, s2, n, {==}, h, qi)


# ---- L4 support: a delivery only empties queues ----

def hub_qd.tail(c: Bool, +m: HM.Msg, -st: HM.MSt, -t: List<&2, HM.MRec>, -ns: List<&2, String>, +e: {Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)) == c : Bool},
    h: {List.is_empty(&2, HM.Msg, Bool.pick(List<&2, HM.Msg>, c, m <> HM.queued_in(t, ns), HM.queued_in(t, ns))) == True{} : Bool}) -> {List.is_empty(&2, HM.Msg, HM.queued_in(t, ns)) == True{} : Bool}:
  match c:
    case False{}:
      h
    case True{}:
      hub_absurd_eq(hub_ft_absurd(h), Bool, List.is_empty(&2, HM.Msg, HM.queued_in(t, ns)), True{})

def hub_qd.ch(c: Bool, +m: HM.Msg, -st: HM.MSt, -t: List<&2, HM.MRec>, -ns: List<&2, String>, +e: {Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)) == c : Bool},
    h: {List.is_empty(&2, HM.Msg, Bool.pick(List<&2, HM.Msg>, c, m <> HM.queued_in(t, ns), HM.queued_in(t, ns))) == True{} : Bool}) -> {Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)) == False{} : Bool}:
  match c:
    case False{}:
      e
    case True{}:
      hub_absurd_eq(hub_ft_absurd(h), Bool, Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)), False{})

def hub_qd.pick(ce: Bool, +m: HM.Msg, +st: HM.MSt, +se: Bool, +t: List<&2, HM.MRec>, +id: Nat, +ns: List<&2, String>,
    +eid: {Nat.is_eq(HM.Msg.id(m), id) == ce : Bool}, +ech: {Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)) == False{} : Bool},
    +et: {List.is_empty(&2, HM.Msg, HM.queued_in(t, ns)) == True{} : Bool}, ih: {List.is_empty(&2, HM.Msg, HM.queued_in(HM.map_msg(t, id, HM.OSt{HM.Delivered{}}), ns)) == True{} : Bool})
    -> {List.is_empty(&2, HM.Msg, HM.queued_in(Bool.pick(List<&2, HM.MRec>, Nat.is_eq(HM.Msg.id(m), id), HM.rec_op(HM.MRec{m, st, se}, HM.OSt{HM.Delivered{}}) <> t, HM.MRec{m, st, se} <> HM.map_msg(t, id, HM.OSt{HM.Delivered{}})), ns)) == True{} : Bool}:
  match ce:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(m), id), True{}, eid) : {List.is_empty(&2, HM.Msg, HM.queued_in(Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(HM.MRec{m, st, se}, HM.OSt{HM.Delivered{}}) <> t, HM.MRec{m, st, se} <> HM.map_msg(t, id, HM.OSt{HM.Delivered{}})), ns)) == True{} : Bool}
      %Equal.sym(Bool, Bool.and(HM.str_in(ns, HM.Msg.to(m)), False{}), False{}, hub_and_false_r(HM.str_in(ns, HM.Msg.to(m)))) : {List.is_empty(&2, HM.Msg, Bool.pick(List<&2, HM.Msg>, _, m <> HM.queued_in(t, ns), HM.queued_in(t, ns))) == True{} : Bool}
      et
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HM.Msg.id(m), id), False{}, eid) : {List.is_empty(&2, HM.Msg, HM.queued_in(Bool.pick(List<&2, HM.MRec>, _, HM.rec_op(HM.MRec{m, st, se}, HM.OSt{HM.Delivered{}}) <> t, HM.MRec{m, st, se} <> HM.map_msg(t, id, HM.OSt{HM.Delivered{}})), ns)) == True{} : Bool}
      %Equal.sym(Bool, Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)), False{}, ech) : {List.is_empty(&2, HM.Msg, Bool.pick(List<&2, HM.Msg>, _, m <> HM.queued_in(HM.map_msg(t, id, HM.OSt{HM.Delivered{}}), ns), HM.queued_in(HM.map_msg(t, id, HM.OSt{HM.Delivered{}}), ns))) == True{} : Bool}
      ih

# marking a message delivered never fills a queue
def hub_qdeliv(+xs: List<&2, HM.MRec>, +id: Nat, +ns: List<&2, String>, +e: {List.is_empty(&2, HM.Msg, HM.queued_in(xs, ns)) == True{} : Bool}) -> {List.is_empty(&2, HM.Msg, HM.queued_in(HM.map_msg(xs, id, HM.OSt{HM.Delivered{}}), ns)) == True{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{HM.MRec{m, st, se}, t}:
      hub_qd.pick(Nat.is_eq(HM.Msg.id(m), id), m, st, se, t, id, ns, {==},
        hub_qd.ch(Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)), m, st, t, ns, {==}, e),
        hub_qd.tail(Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)), m, st, t, ns, {==}, e),
        hub_qdeliv(t, id, ns, hub_qd.tail(Bool.and(HM.str_in(ns, HM.Msg.to(m)), HM.is_queued(st)), m, st, t, ns, {==}, e)))


# ---- L4 support: queue implications ----

def hub_qimp_mk(q1: Bool, +q2: Bool, k: {q1 == True{} : Bool} -> {q2 == True{} : Bool}) -> {Bool.or(Bool.not(q1), q2) == True{} : Bool}:
  match q1:
    case True{}:
      k({==})
    case False{}:
      {==}

def hub_qimp_use(q1: Bool, -q2: Bool, h: {Bool.or(Bool.not(q1), q2) == True{} : Bool}, e: {q1 == True{} : Bool}) -> {q2 == True{} : Bool}:
  match q1:
    case True{}:
      h
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, q2, True{})

def hub_qimp_refl(+s: HM.St, +n: String) -> {hub_qimp(s, s, n) == True{} : Bool}:
  hub_qimp_mk(hub_q(s, n), hub_q(s, n), e => e)

def hub_qimp_trans(+s1: HM.St, +s2: HM.St, +s3: HM.St, +n: String, +h12: {hub_qimp(s1, s2, n) == True{} : Bool},
    +h23: {hub_qimp(s2, s3, n) == True{} : Bool}) -> {hub_qimp(s1, s3, n) == True{} : Bool}:
  hub_qimp_mk(hub_q(s1, n), hub_q(s3, n), e => hub_qimp_use(hub_q(s2, n), hub_q(s3, n), h23, hub_qimp_use(hub_q(s1, n), hub_q(s2, n), h12, e)))

def hub_qimp_eq(+s1: HM.St, -s2: HM.St, +n: String, e: {s2 == s1 : HM.St}) -> {hub_qimp(s1, s2, n) == True{} : Bool}:
  %Equal.sym(HM.St, s2, s1, e) : {hub_qimp(s1, _, n) == True{} : Bool}
  hub_qimp_refl(s1, n)

# marking a message delivered never fills a queue (on the whole state)
def hub_qimp_deliv(s: HM.St, +id: Nat, +n: String) -> {hub_qimp(s, HM.apply(s, HM.EMsgState{id, HM.Delivered{}}), n) == True{} : Bool}:
  match s:
    case HM.St{+ag, +od, +ms, +ca, +no, +nm, +nc, +tm}:
      +ns = HM.names_of(HM.St{ag, od, ms, ca, no, nm, nc, tm}, n)
      hub_qimp_mk(List.is_empty(&2, HM.Msg, HM.queued_in(ms, ns)), List.is_empty(&2, HM.Msg, HM.queued_in(HM.map_msg(ms, id, HM.OSt{HM.Delivered{}}), ns)),
        e => hub_qdeliv(ms, id, ns, e))

def hub_qimp_flush(s: HM.St, +n: String) -> {hub_qimp(s, HM.apply(s, HM.ENotesFlushed{}), n) == True{} : Bool}:
  match s:
    case HM.St{+ag, +od, +ms, +ca, +no, +nm, +nc, +tm}:
      hub_qimp_refl(HM.St{ag, od, ms, ca, no, nm, nc, tm}, n)


# ---- L4 support: the frame of a delivery to another agent ----

def hub_d_steer(-o: HM.St, cx: HX.Cx<o>, +a: String, +ids: List<&2, Nat>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> {HX.d(o, HC.pump.send.go(o, True{}, cx, a, ids, kvs)) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_dirty_out(-o: HM.St, cx: HX.Cx<o>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>) -> {HX.d(o, HC.dirty(o, HC.out(o, cx, kvs))) == HX.d(o, cx) : HM.St}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          {==}

def hub_d_send.go(-o: HM.St, b: Bool, +cx: HX.Cx<o>, +a: String, +ids: List<&2, Nat>, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> {HX.d(o, HC.pump.send.go(o, b, cx, a, ids, kvs)) == HX.d(o, cx) : HM.St}:
  match b:
    case True{}:
      hub_d_steer(o, cx, a, ids, kvs)
    case False{}:
      +x = {HX.Rt{a, HM.RBusy{}, HX.Rt.waiting(HC.rt(o, cx, a)), Some{HC.now(o, cx)}} : HX.Rt}
      Equal.trans(HM.St, HX.d(o, HC.pump.send.go(o, False{}, cx, a, ids, kvs)), HX.d(o, HC.upd_rt(o, cx, x)), HX.d(o, cx),
        hub_d_dirty_out(o, HC.upd_rt(o, cx, x), J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"say"}) <> kvs),
        hub_d_upd_rt(o, cx, x))

def hub_d_send(-o: HM.St, b: Bool, p: HX.Cx<o> & List<&2, J.Json>, +a: String, +ids: List<&2, Nat>, +kvs0: List<&2, Sigma<&2, &2, String, _ => J.Json>>)
    -> {HX.d(o, HC.pump.send(o, b, p, a, ids, kvs0)) == HX.d(o, hub_p1(o, p)) : HM.St}:
  match p:
    case (+cx, +msgs):
      hub_d_send.go(o, b, cx, a, ids, HM.snoc(Sigma<&2, &2, String, _ => J.Json>, kvs0, J.kv("msgs", J.JArr{msgs})))

def hub_d_eq_qimp(-o: HM.St, +a: HX.Cx<o>, +b: HX.Cx<o>, +n: String, e: {HX.d(o, b) == HX.d(o, a) : HM.St}) -> {hub_qimp(HX.d(o, a), HX.d(o, b), n) == True{} : Bool}:
  hub_qimp_eq(HX.d(o, a), HX.d(o, b), n, e)

def hub_qimp_mark.go(-o: HM.St, +cx: HX.Cx<o>, +id: Nat, +n: String, c: Bool, +e: {HX.queued_at(HX.d(o, cx), id) == c : Bool})
    -> {hub_qimp(HX.d(o, cx), HX.d(o, HC.mark_delivered.go(o, cx, id, c, e)), n) == True{} : Bool}:
  match c:
    case True{}:
      hub_qimp_trans(HX.d(o, cx), HM.apply(HX.d(o, cx), HM.EMsgState{id, HM.Delivered{}}), HX.d(o, HC.mark_delivered.go(o, cx, id, True{}, e)), n,
        hub_qimp_deliv(HX.d(o, cx), id, n),
        hub_qimp_eq(HM.apply(HX.d(o, cx), HM.EMsgState{id, HM.Delivered{}}), HX.d(o, HC.mark_delivered.go(o, cx, id, True{}, e)), n,
          hub_d_emit(o, cx, HM.EMsgState{id, HM.Delivered{}}, Unit{}, Unit{}, e)))
    case False{}:
      hub_qimp_refl(HX.d(o, cx), n)

def hub_qimp_pick(-o: HM.St, c: Bool, +x: HX.Cx<o>, +y: HX.Cx<o>, +z: HX.Cx<o>, +n: String,
    ex: {HX.d(o, x) == HX.d(o, z) : HM.St}, ey: {HX.d(o, y) == HX.d(o, z) : HM.St})
    -> {hub_qimp(HX.d(o, z), HX.d(o, Bool.pick(HX.Cx<o>, c, x, y)), n) == True{} : Bool}:
  match c:
    case True{}:
      hub_qimp_eq(HX.d(o, z), HX.d(o, x), n, ex)
    case False{}:
      hub_qimp_eq(HX.d(o, z), HX.d(o, y), n, ey)

# delivering a batch never fills a queue
def hub_qimp_deliver(-o: HM.St, +b: List<&2, HM.Msg>, +cx: HX.Cx<o>, +name: String, +acc: List<&2, J.Json>, +n: String)
    -> {hub_qimp(HX.d(o, cx), HX.d(o, hub_p1(o, HC.deliver_each(o, b, cx, name, acc))), n) == True{} : Bool}:
  match b:
    case Nil{}:
      hub_qimp_refl(HX.d(o, cx), n)
    case Con{m, rest}:
      +cx2 = HC.mark_delivered(o, cx, HM.Msg.id(m))
      +s = HC.st(o, cx)
      +again = HC.leftover_at(s, HM.Msg.id(m))
      +you = HM.Msg.plain(m) || Maybe.is_some(&2, String, HM.Msg.via(m))
      +cx3 = HC.deliver.line(o, again, you, cx2, name, m)
      +rel = HC.relation(HM.Msg.from(m), HM.Msg.to(m), HC.parent_of(HM.get_agent(s, HM.Msg.from(m))), HC.parent_of(HM.get_agent(s, HM.Msg.to(m))))
      +acc2 = HM.snoc(J.Json, acc, J.obj([J.kv("id", J.JNum{HM.Msg.id(m)}), J.kv("rel", J.JStr{rel})]))
      hub_qimp_trans(HX.d(o, cx), HX.d(o, cx3), HX.d(o, hub_p1(o, HC.deliver_each(o, rest, cx3, name, acc2))), n,
        hub_qimp_trans(HX.d(o, cx), HX.d(o, cx2), HX.d(o, cx3), n,
          hub_qimp_mark.go(o, cx, HM.Msg.id(m), n, HX.queued_at(HX.d(o, cx), HM.Msg.id(m)), {==}),
          hub_qimp_eq(HX.d(o, cx2), HX.d(o, cx3), n, hub_d_dline(o, again, you, cx2, name, m))),
        hub_qimp_deliver(o, rest, cx3, name, acc2, n))

def hub_qimp_flush_cx(-o: HM.St, f: Bool, +cx: HX.Cx<o>, +n: String) -> {hub_qimp(HX.d(o, cx), HX.d(o, HC.pump.flush(o, f, cx)), n) == True{} : Bool}:
  match f:
    case True{}:
      hub_qimp_trans(HX.d(o, cx), HM.apply(HX.d(o, cx), HM.ENotesFlushed{}), HX.d(o, HC.pump.flush(o, True{}, cx)), n,
        hub_qimp_flush(HX.d(o, cx), n),
        hub_qimp_eq(HM.apply(HX.d(o, cx), HM.ENotesFlushed{}), HX.d(o, HC.pump.flush(o, True{}, cx)), n,
          hub_d_emit(o, cx, HM.ENotesFlushed{}, Unit{}, Unit{}, Unit{})))
    case False{}:
      hub_qimp_refl(HX.d(o, cx), n)

def hub_rts_if_other(-o: HM.St, known: Bool, +cx: HX.Cx<o>, +a: String, +w: Bool, +tm: Maybe<&2, Nat>, +n: String, +ne: {String.eq(a, n) == False{} : Bool})
    -> {HC.rt_find(hub_rts(o, HC.if_agent(o, known, cx, HX.Rt{a, HM.RBusy{}, w, tm})), n) == HC.rt_find(hub_rts(o, cx), n) : HX.Rt}:
  match known:
    case True{}:
      %Equal.sym(List<&2, HX.Rt>, hub_rts(o, HC.set_rt(o, cx, HX.Rt{a, HM.RBusy{}, w, tm})), HC.rt_put(hub_rts(o, cx), HX.Rt{a, HM.RBusy{}, w, tm}), hub_rts_set_rt(o, cx, HX.Rt{a, HM.RBusy{}, w, tm})) : {HC.rt_find(_, n) == HC.rt_find(hub_rts(o, cx), n) : HX.Rt}
      hub_rt_put_other(hub_rts(o, cx), a, n, HM.RBusy{}, w, tm, ne)
    case False{}:
      {==}

# a new turn of A leaves the runtime state of any other agent as it was
def hub_rts_send_other(-o: HM.St, p: HX.Cx<o> & List<&2, J.Json>, +a: String, +ids: List<&2, Nat>, +kvs0: List<&2, Sigma<&2, &2, String, _ => J.Json>>, +n: String, +ne: {String.eq(a, n) == False{} : Bool})
    -> {HC.rt_find(hub_rts(o, HC.pump.send(o, False{}, p, a, ids, kvs0)), n) == HC.rt_find(hub_rts(o, hub_p1(o, p)), n) : HX.Rt}:
  match p:
    case (+cx, +msgs):
      +x = {HX.Rt{a, HM.RBusy{}, HX.Rt.waiting(HC.rt(o, cx, a)), Some{HC.now(o, cx)}} : HX.Rt}
      +cx2 = HC.upd_rt(o, cx, x)
      +kvs = HM.snoc(Sigma<&2, &2, String, _ => J.Json>, kvs0, J.kv("msgs", J.JArr{msgs}))
      %Equal.sym(List<&2, HX.Rt>, hub_rts(o, HC.dirty(o, HC.out(o, cx2, J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"say"}) <> kvs))), hub_rts(o, cx2),
          hub_rts_dirty_out(o, cx2, J.kv("fx", J.JStr{"deliver"}) <> J.kv("mode", J.JStr{"say"}) <> kvs)) : {HC.rt_find(_, n) == HC.rt_find(hub_rts(o, cx), n) : HX.Rt}
      hub_rts_if_other(o, HC.has_agent(o, cx, a), cx, a, HX.Rt.waiting(HC.rt(o, cx, a)), Some{HC.now(o, cx)}, n, ne)

def hub_rt_run_eq(+x: List<&2, HX.Rt>, +y: List<&2, HX.Rt>, +n: String, e: {HC.rt_find(x, n) == HC.rt_find(y, n) : HX.Rt})
    -> {HX.Rt.run(HC.rt_find(x, n)) == HX.Rt.run(HC.rt_find(y, n)) : HM.Run}:
  Equal.cong(HX.Rt, HM.Run, r => HX.Rt.run(r), HC.rt_find(x, n), HC.rt_find(y, n), e)

def hub_ga_eq(-o: HM.St, +a: HX.Cx<o>, +b: HX.Cx<o>, +n: String, e: {hub_ag(o, a) == hub_ag(o, b) : List<&2, HM.Agent>})
    -> {HM.get_agent(HX.d(o, a), n) == HM.get_agent(HX.d(o, b), n) : Maybe<&2, HM.Agent>}:
  Equal.cong(List<&2, HM.Agent>, Maybe<&2, HM.Agent>, xs => HM.agent_named(xs, n), hub_ag(o, a), hub_ag(o, b), e)

def hub_pb_rt.go(-o: HM.St, busy: Bool, +bb: List<&2, HM.Msg>, +cx2: HX.Cx<o>, +cx: HX.Cx<o>, +a: String, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>, +n: String,
    +ne: {String.eq(a, n) == False{} : Bool}, +ep: {hub_rts(o, hub_p1(o, HC.deliver_each(o, bb, cx2, a, Nil{}))) == hub_rts(o, cx) : List<&2, HX.Rt>})
    -> {HX.Rt.run(HC.rt_find(hub_rts(o, HC.pump.send(o, busy, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)), n))
        == HX.Rt.run(HC.rt_find(hub_rts(o, cx), n)) : HM.Run}:
  match busy:
    case True{}:
      hub_rt_run_eq(hub_rts(o, HC.pump.send(o, True{}, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)), hub_rts(o, cx), n,
        Equal.cong(List<&2, HX.Rt>, HX.Rt, xs => HC.rt_find(xs, n), hub_rts(o, HC.pump.send(o, True{}, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)), hub_rts(o, cx),
          Equal.trans(List<&2, HX.Rt>, hub_rts(o, HC.pump.send(o, True{}, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)),
            hub_rts(o, hub_p1(o, HC.deliver_each(o, bb, cx2, a, Nil{}))), hub_rts(o, cx),
            hub_rts_steer(o, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs), ep)))
    case False{}:
      hub_rt_run_eq(hub_rts(o, HC.pump.send(o, False{}, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)), hub_rts(o, cx), n,
        Equal.trans(HX.Rt, HC.rt_find(hub_rts(o, HC.pump.send(o, False{}, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)), n),
          HC.rt_find(hub_rts(o, hub_p1(o, HC.deliver_each(o, bb, cx2, a, Nil{}))), n), HC.rt_find(hub_rts(o, cx), n),
          hub_rts_send_other(o, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs, n, ne),
          Equal.cong(List<&2, HX.Rt>, HX.Rt, xs => HC.rt_find(xs, n), hub_rts(o, hub_p1(o, HC.deliver_each(o, bb, cx2, a, Nil{}))), hub_rts(o, cx), ep)))

# the runtime state of another agent after a batch to A
def hub_pb_rt(-o: HM.St, busy: Bool, +bb: List<&2, HM.Msg>, +cx2: HX.Cx<o>, +cx: HX.Cx<o>, +a: String, +kvs: List<&2, Sigma<&2, &2, String, _ => J.Json>>, +n: String,
    +ne: {String.eq(a, n) == False{} : Bool}, +e2: {hub_rts(o, cx2) == hub_rts(o, cx) : List<&2, HX.Rt>})
    -> {HX.Rt.run(HC.rt_find(hub_rts(o, HC.pump.send(o, busy, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)), n))
        == HX.Rt.run(HC.rt_find(hub_rts(o, cx), n)) : HM.Run}:
  +p1 = hub_p1(o, HC.deliver_each(o, bb, cx2, a, Nil{}))
  +ep = Equal.trans(List<&2, HX.Rt>, hub_rts(o, p1), hub_rts(o, cx2), hub_rts(o, cx), hub_rts_deliver(o, bb, cx2, a, Nil{}), e2)
  hub_pb_rt.go(o, busy, bb, cx2, cx, a, kvs, n, ne, ep)

# a batch to A never makes another agent stuck
def hub_pb_frame(-o: HM.St, +busy: Bool, b: List<&2, HM.Msg>, +cx: HX.Cx<o>, +a: String, +im: Bool, +n: String,
    +ne: {String.eq(a, n) == False{} : Bool}, h: {HI.stuck(HX.d(o, cx), hub_rts(o, cx), n) == False{} : Bool})
    -> {HI.stuck(HX.d(o, HC.pump.batch(o, b, cx, a, busy, im)), hub_rts(o, HC.pump.batch(o, b, cx, a, busy, im)), n) == False{} : Bool}:
  match b:
    case Nil{}:
      h
    case Con{m, t}:
      +bb = {m <> t : List<&2, HM.Msg>}
      +notes = Bool.pick(List<&2, String>, im, HM.St.notes(HC.st(o, cx)), Nil{})
      +f = Bool.not(List.is_empty(&2, String, notes))
      +cx2 = HC.pump.flush(o, f, cx)
      +kvs = {[J.kv("agent", J.JStr{a}), J.kv("notes", J.JArr{HC.jstrs(notes)}), J.kv("status", J.JBool{im && HC.any_from_user(bb)})] : List<&2, Sigma<&2, &2, String, _ => J.Json>>}
      +p1 = hub_p1(o, HC.deliver_each(o, bb, cx2, a, Nil{}))
      +fin = HC.pump.send(o, busy, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)
      +edf = hub_d_send(o, busy, HC.deliver_each(o, bb, cx2, a, Nil{}), a, HC.ids_of(bb), kvs)
      hub_frame(HX.d(o, cx), hub_rts(o, cx), HX.d(o, fin), hub_rts(o, fin), n,
        hub_pb_rt(o, busy, bb, cx2, cx, a, kvs, n, ne, hub_rts_flush(o, f, cx)),
        hub_ga_eq(o, fin, cx, n,
          Equal.trans(List<&2, HM.Agent>, hub_ag(o, fin), hub_ag(o, p1), hub_ag(o, cx),
            Equal.cong(HM.St, List<&2, HM.Agent>, s => HM.St.agents(s), HX.d(o, fin), HX.d(o, p1), edf),
            Equal.trans(List<&2, HM.Agent>, hub_ag(o, p1), hub_ag(o, cx2), hub_ag(o, cx),
              hub_ag_deliver(o, bb, cx2, a, Nil{}), hub_ag_flush(o, f, cx)))),
        hub_qimp_trans(HX.d(o, cx), HX.d(o, cx2), HX.d(o, fin), n,
          hub_qimp_flush_cx(o, f, cx, n),
          hub_qimp_trans(HX.d(o, cx2), HX.d(o, p1), HX.d(o, fin), n,
            hub_qimp_deliver(o, bb, cx2, a, Nil{}, n),
            hub_qimp_eq(HX.d(o, p1), HX.d(o, fin), n, edf))),
        h)

def hub_pf.run(-o: HM.St, r: HM.Run, +cx: HX.Cx<o>, +a: String, +q: List<&2, HM.Msg>, +im: Bool, +n: String,
    +ne: {String.eq(a, n) == False{} : Bool}, +h: {HI.stuck(HX.d(o, cx), hub_rts(o, cx), n) == False{} : Bool})
    -> {HI.stuck(HX.d(o, HC.pump.run(o, r, cx, a, q, im)), hub_rts(o, HC.pump.run(o, r, cx, a, q, im)), n) == False{} : Bool}:
  match r:
    case HM.RDown{}:
      h
    case HM.RStarting{}:
      h
    case HM.RBusy{}:
      hub_pb_frame(o, True{}, HC.not_queued_mode(q), cx, a, im, n, ne, h)
    case HM.RIdle{}:
      hub_pb_frame(o, False{}, q, cx, a, im, n, ne, h)

def hub_pf.active(-o: HM.St, c: Bool, +cx: HX.Cx<o>, +a: String, +im: Bool, +n: String,
    +ne: {String.eq(a, n) == False{} : Bool}, +h: {HI.stuck(HX.d(o, cx), hub_rts(o, cx), n) == False{} : Bool})
    -> {HI.stuck(HX.d(o, HC.pump.active(o, c, cx, a, im)), hub_rts(o, HC.pump.active(o, c, cx, a, im)), n) == False{} : Bool}:
  match c:
    case True{}:
      hub_pf.run(o, HX.Rt.run(HC.rt(o, cx, a)), cx, a, HM.queued_for(HC.st(o, cx), a), im, n, ne, h)
    case False{}:
      h

def hub_pf.agent(-o: HM.St, m: Maybe<&2, HM.Agent>, +cx: HX.Cx<o>, +a: String, +n: String,
    +ne: {String.eq(a, n) == False{} : Bool}, +h: {HI.stuck(HX.d(o, cx), hub_rts(o, cx), n) == False{} : Bool})
    -> {HI.stuck(HX.d(o, HC.pump.agent(o, m, cx, a)), hub_rts(o, HC.pump.agent(o, m, cx, a)), n) == False{} : Bool}:
  match m:
    case None{}:
      h
    case Some{+ag}:
      hub_pf.active(o, HC.is_active(HM.Agent.life(ag)), cx, a, HM.Agent.is_main(ag), n, ne, h)

# a delivery attempt for A never makes another agent stuck
def hub_pump_frame(-o: HM.St, +cx: HX.Cx<o>, +a: String, +n: String,
    +ne: {String.eq(a, n) == False{} : Bool}, +h: {HI.stuck(HX.d(o, cx), hub_rts(o, cx), n) == False{} : Bool})
    -> {HI.stuck(HX.d(o, HC.pump(o, cx, a)), hub_rts(o, HC.pump(o, cx, a)), n) == False{} : Bool}:
  hub_pf.agent(o, HM.get_agent(HC.st(o, cx), a), cx, a, n, ne, h)


# ---- L4: no agent stuck, except the listed ones ----

def hub_nsx(-o: HM.St, +cx: HX.Cx<o>, +xs: List<&2, String>) -> Type:
  @n: String -> @e: {HM.str_in(xs, n) == False{} : Bool} -> {HI.stuck(HX.d(o, cx), hub_rts(o, cx), n) == False{} : Bool}

def hub_pump_nsx.pick(-o: HM.St, c: Bool, +cx: HX.Cx<o>, +a: String, +xs: List<&2, String>, +n: String, +e: {String.eq(a, n) == c : Bool},
    h: hub_nsx(o, cx, a <> xs), +nx: {HM.str_in(xs, n) == False{} : Bool})
    -> {HI.stuck(HX.d(o, HC.pump(o, cx, a)), hub_rts(o, HC.pump(o, cx, a)), n) == False{} : Bool}:
  match c:
    case True{}:
      %hub_str_eq_sound(a, n, e) : {HI.stuck(HX.d(o, HC.pump(o, cx, a)), hub_rts(o, HC.pump(o, cx, a)), _) == False{} : Bool}
      hub_pump_wakes(o, cx, a)
    case False{}:
      hub_pump_frame(o, cx, a, n, e, h(n, hub_or_false(String.eq(a, n), HM.str_in(xs, n), e, nx)))

# delivering to A clears A from the exceptions
def hub_pump_nsx(-o: HM.St, +cx: HX.Cx<o>, +a: String, +xs: List<&2, String>, h: hub_nsx(o, cx, a <> xs)) -> hub_nsx(o, HC.pump(o, cx, a), xs):
  +n => nx => hub_pump_nsx.pick(o, String.eq(a, n), cx, a, xs, n, {==}, h, nx)


# ---- L4 support: names apart — the operations that keep names ----

def hub_is_rename(op: HM.AgentOp) -> Bool:
  match op:
    case HM.ORename{x}:
      True{}
    case _:
      False{}

def hub_names_op(h: HM.Agent, op: HM.AgentOp, nr: {hub_is_rename(op) == False{} : Bool}) -> {HI.names(HM.agent_op(h, op)) == HI.names(h) : List<&2, String>}:
  match h:
    case HM.Agent{name, im, pa, life, de, al, fi, ws, di, br, cr, fa, no, rp, sn}:
      match op:
        case HM.OLife{l, why}:
          {==}
        case HM.OWs{w}:
          {==}
        case HM.ODeclared{d, n}:
          {==}
        case HM.OFile{p}:
          {==}
        case HM.ORename{nn}:
          hub_absurd_eq(hub_tf_absurd(nr), List<&2, String>, HI.names(HM.agent_op(HM.Agent{name, im, pa, life, de, al, fi, ws, di, br, cr, fa, no, rp, sn}, HM.ORename{nn})), HI.names(HM.Agent{name, im, pa, life, de, al, fi, ws, di, br, cr, fa, no, rp, sn}))
        case HM.OSnap{x}:
          {==}
        case HM.OReport{x}:
          {==}

def hub_prof_map.pick(c: Bool, +h: HM.Agent, +t: List<&2, HM.Agent>, +n: String, +op: HM.AgentOp, +e: {String.eq(HM.Agent.name(h), n) == c : Bool},
    +nr: {hub_is_rename(op) == False{} : Bool}, ih: {HI.profile(HM.map_agent(t, n, op)) == HI.profile(t) : List<&2, List<&2, String>>})
    -> {HI.profile(Bool.pick(List<&2, HM.Agent>, String.eq(HM.Agent.name(h), n), HM.agent_op(h, op) <> t, h <> HM.map_agent(t, n, op))) == HI.profile(h <> t) : List<&2, List<&2, String>>}:
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HM.Agent.name(h), n), True{}, e) : {HI.profile(Bool.pick(List<&2, HM.Agent>, _, HM.agent_op(h, op) <> t, h <> HM.map_agent(t, n, op))) == HI.profile(h <> t) : List<&2, List<&2, String>>}
      Equal.cong(List<&2, String>, List<&2, List<&2, String>>, x => x <> HI.profile(t), HI.names(HM.agent_op(h, op)), HI.names(h), hub_names_op(h, op, nr))
    case False{}:
      %Equal.sym(Bool, String.eq(HM.Agent.name(h), n), False{}, e) : {HI.profile(Bool.pick(List<&2, HM.Agent>, _, HM.agent_op(h, op) <> t, h <> HM.map_agent(t, n, op))) == HI.profile(h <> t) : List<&2, List<&2, String>>}
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, x => HI.names(h) <> x, HI.profile(HM.map_agent(t, n, op)), HI.profile(t), ih)

# an operation that is not a rename keeps every agent's names
def hub_prof_map(+xs: List<&2, HM.Agent>, +n: String, +op: HM.AgentOp, +nr: {hub_is_rename(op) == False{} : Bool})
    -> {HI.profile(HM.map_agent(xs, n, op)) == HI.profile(xs) : List<&2, List<&2, String>>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_prof_map.pick(String.eq(HM.Agent.name(h), n), h, t, n, op, {==}, nr, hub_prof_map(t, n, op, nr))


# ---- L4 support: names apart — a free name ----

def hub_str_eq_sym_false.aa(+a: String, -b: String, s: {b == a : String}, e: {String.eq(a, b) == False{} : Bool}) -> {String.eq(a, a) == False{} : Bool}:
  %s : {String.eq(a, _) == False{} : Bool}
  e

def hub_str_eq_sym_false.bad(+a: String, -b: String, s: {b == a : String}, e: {String.eq(a, b) == False{} : Bool}) -> {True{} == False{} : Bool}:
  Equal.trans(Bool, True{}, String.eq(a, a), False{}, Equal.sym(Bool, String.eq(a, a), True{}, hub_str_eq_refl(a)), hub_str_eq_sym_false.aa(a, b, s, e))

def hub_str_eq_sym_false.pick(c: Bool, +a: String, +b: String, +ec: {String.eq(b, a) == c : Bool}, e: {String.eq(a, b) == False{} : Bool})
    -> {String.eq(b, a) == False{} : Bool}:
  match c:
    case False{}:
      ec
    case True{}:
      hub_absurd_eq(hub_tf_absurd(hub_str_eq_sym_false.bad(a, b, hub_str_eq_sound(b, a, ec), e)), Bool, String.eq(b, a), False{})

def hub_str_eq_sym_false(+a: String, +b: String, e: {String.eq(a, b) == False{} : Bool}) -> {String.eq(b, a) == False{} : Bool}:
  hub_str_eq_sym_false.pick(String.eq(b, a), a, b, {==}, e)

# n is none of the names of the listed agents
def hub_free(ps: List<&2, List<&2, String>>, +n: String) -> Data:
  match ps:
    case Nil{}:
      Unit
    case Con{+p, +r}:
      HX.Both<{HM.str_in(p, n) == False{} : Bool}, hub_free(r, n)>

def hub_mdisc(m: Maybe<&2, HM.Agent>) -> Type:
  match m:
    case None{}:
      Unit
    case Some{x}:
      Empty

def hub_some_none(-a: HM.Agent, e: {Some{a} == None{} : Maybe<&2, HM.Agent>}) -> Empty:
  %Equal.sym(Maybe<&2, HM.Agent>, Some{a}, None{}, e) : hub_mdisc(_)
  Unit{}

def hub_sdisc(m: Maybe<&2, String>) -> Type:
  match m:
    case None{}:
      Unit
    case Some{x}:
      Empty

def hub_some_none_s(-a: String, e: {Some{a} == None{} : Maybe<&2, String>}) -> Empty:
  %Equal.sym(Maybe<&2, String>, Some{a}, None{}, e) : hub_sdisc(_)
  Unit{}

def hub_free_of.absurd1(-a: HM.Agent, -t: List<&2, HM.Agent>, -n: String, x: Empty) -> hub_free(HI.profile(a <> t), n):
  match x:

def hub_free_of.pick(c1: Bool, c2: Bool, +a: HM.Agent, +t: List<&2, HM.Agent>, +n: String,
    +e1: {String.eq(HM.Agent.name(a), n) == c1 : Bool}, +e2: {HM.str_in(HM.Agent.aliases(a), n) == c2 : Bool},
    hn: {Bool.pick(Maybe<&2, HM.Agent>, c1, Some{a}, HM.agent_named(t, n)) == None{} : Maybe<&2, HM.Agent>},
    ha: {Bool.pick(Maybe<&2, String>, c2, Some{HM.Agent.name(a)}, HM.agent_alias(t, n)) == None{} : Maybe<&2, String>},
    k: @p: {HM.agent_named(t, n) == None{} : Maybe<&2, HM.Agent>} -> @q: {HM.agent_alias(t, n) == None{} : Maybe<&2, String>} -> hub_free(HI.profile(t), n))
    -> hub_free(HI.profile(a <> t), n):
  match c1 c2:
    case True{} _:
      hub_free_of.absurd1(a, t, n, hub_some_none(a, hn))
    case False{} True{}:
      hub_free_of.absurd1(a, t, n, hub_some_none_s(HM.Agent.name(a), ha))
    case False{} False{}:
      HX.Both{hub_or_false(String.eq(HM.Agent.name(a), n), HM.str_in(HM.Agent.aliases(a), n), e1, e2), k(hn, ha)}

# a name no agent has as a name or an old name is free in the profile
def hub_free_of(+xs: List<&2, HM.Agent>, +n: String, hn: {HM.agent_named(xs, n) == None{} : Maybe<&2, HM.Agent>}, ha: {HM.agent_alias(xs, n) == None{} : Maybe<&2, String>})
    -> hub_free(HI.profile(xs), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{a, t}:
      hub_free_of.pick(String.eq(HM.Agent.name(a), n), HM.str_in(HM.Agent.aliases(a), n), a, t, n, {==}, {==}, hn, ha,
        p => q => hub_free_of(t, n, p, q))

def hub_nt.absurd(-s: HM.St, -n: String, x: Empty) -> hub_free(HI.profile(HM.St.agents(s)), n):
  match x:

def hub_nt.alias(r: Maybe<&2, String>, +s: HM.St, +n: String, +er: {HM.agent_alias(HM.St.agents(s), n) == r : Maybe<&2, String>},
    +en: {HM.agent_named(HM.St.agents(s), n) == None{} : Maybe<&2, HM.Agent>}, hs: {Maybe.is_some(&2, String, r) == False{} : Bool})
    -> hub_free(HI.profile(HM.St.agents(s)), n):
  match r:
    case None{}:
      hub_free_of(HM.St.agents(s), n, en, er)
    case Some{x}:
      hub_nt.absurd(s, n, hub_ft_absurd(Equal.sym(Bool, True{}, False{}, hs)))

def hub_nt.named(m: Maybe<&2, HM.Agent>, +s: HM.St, +n: String, +em: {HM.agent_named(HM.St.agents(s), n) == m : Maybe<&2, HM.Agent>},
    hs: {Maybe.is_some(&2, String, HM.resolve.pick(m, s, n)) == False{} : Bool})
    -> hub_free(HI.profile(HM.St.agents(s)), n):
  match m:
    case Some{a}:
      hub_nt.absurd(s, n, hub_ft_absurd(Equal.sym(Bool, True{}, False{}, hs)))
    case None{}:
      hub_nt.alias(HM.agent_alias(HM.St.agents(s), n), s, n, {==}, em, hs)

# a name that is not taken is free among the agents' names
def hub_nt(+s: HM.St, +n: String, e: {HM.name_taken(s, n) == False{} : Bool}) -> hub_free(HI.profile(HM.St.agents(s)), n):
  hub_nt.named(HM.agent_named(HM.St.agents(s), n), s, n, {==},
    hub_or_false_r(String.eq(n, HM.MAIN()) || String.eq(n, HM.USER()) || String.eq(n, HM.HUB()) || String.eq(n, HM.GITHUB()), Maybe.is_some(&2, String, HM.resolve(s, n)), e))


# ---- L4 support: names apart — a new agent ----

def hub_allapart(ps: List<&2, List<&2, String>>, +q: List<&2, String>) -> Data:
  match ps:
    case Nil{}:
      Unit
    case Con{+p, +r}:
      HX.Both<HI.Apart(p, q), hub_allapart(r, q)>

def hub_aa_snoc(+p: List<&2, String>, +r: List<&2, List<&2, String>>, +q: List<&2, String>, a: HI.ApartAll(p, r), b: HI.Apart(p, q)) -> HI.ApartAll(p, HM.snoc(List<&2, String>, r, q)):
  match r:
    case Nil{}:
      HX.Both{b, Unit{}}
    case Con{x, t}:
      match a:
        case HX.Both{a1, a2}:
          HX.Both{a1, hub_aa_snoc(p, t, q, a2, b)}

def hub_nok_snoc(+ps: List<&2, List<&2, String>>, +q: List<&2, String>, nok: HI.NamesOK(ps), aa: hub_allapart(ps, q)) -> HI.NamesOK(HM.snoc(List<&2, String>, ps, q)):
  match ps:
    case Nil{}:
      HX.Both{Unit{}, Unit{}}
    case Con{p, r}:
      match nok aa:
        case HX.Both{n1, n2} HX.Both{a1, a2}:
          HX.Both{hub_aa_snoc(p, r, q, n1, a1), hub_nok_snoc(r, q, n2, a2)}

def hub_apart_new(+p: List<&2, String>, +n: String, +f: {HM.str_in(p, n) == False{} : Bool}) -> HI.Apart(p, [n]):
  match p:
    case Nil{}:
      Unit{}
    case Con{m, al}:
      HX.Both{hub_or_false(String.eq(n, m), False{}, hub_str_eq_sym_false(m, n, hub_or_false_l(String.eq(m, n), HM.str_in(al, n), f)), {==}),
        hub_or_false_r(String.eq(m, n), HM.str_in(al, n), f)}

def hub_allapart_new(+ps: List<&2, List<&2, String>>, +n: String, f: hub_free(ps, n)) -> hub_allapart(ps, [n]):
  match ps:
    case Nil{}:
      Unit{}
    case Con{p, r}:
      match f:
        case HX.Both{f1, f2}:
          HX.Both{hub_apart_new(p, n, f1), hub_allapart_new(r, n, f2)}

def hub_put_absent.pick(c: Bool, +h: HM.Agent, +t: List<&2, HM.Agent>, +n: String, +a: HM.Agent, +e: {String.eq(HM.Agent.name(h), n) == c : Bool},
    ec: {c == False{} : Bool}, ih: {HI.profile(HM.put_agent(t, n, a)) == HM.snoc(List<&2, String>, HI.profile(t), HI.names(a)) : List<&2, List<&2, String>>})
    -> {HI.profile(Bool.pick(List<&2, HM.Agent>, String.eq(HM.Agent.name(h), n), a <> t, h <> HM.put_agent(t, n, a))) == HM.snoc(List<&2, String>, HI.profile(h <> t), HI.names(a)) : List<&2, List<&2, String>>}:
  match c:
    case True{}:
      hub_absurd_eq(hub_tf_absurd(ec), List<&2, List<&2, String>>, HI.profile(Bool.pick(List<&2, HM.Agent>, String.eq(HM.Agent.name(h), n), a <> t, h <> HM.put_agent(t, n, a))), HM.snoc(List<&2, String>, HI.profile(h <> t), HI.names(a)))
    case False{}:
      %Equal.sym(Bool, String.eq(HM.Agent.name(h), n), False{}, e) : {HI.profile(Bool.pick(List<&2, HM.Agent>, _, a <> t, h <> HM.put_agent(t, n, a))) == HM.snoc(List<&2, String>, HI.profile(h <> t), HI.names(a)) : List<&2, List<&2, String>>}
      Equal.cong(List<&2, List<&2, String>>, List<&2, List<&2, String>>, x => HI.names(h) <> x, HI.profile(HM.put_agent(t, n, a)), HM.snoc(List<&2, String>, HI.profile(t), HI.names(a)), ih)

# a new agent under a free name goes last
def hub_put_absent(+xs: List<&2, HM.Agent>, +n: String, +a: HM.Agent, f: hub_free(HI.profile(xs), n))
    -> {HI.profile(HM.put_agent(xs, n, a)) == HM.snoc(List<&2, String>, HI.profile(xs), HI.names(a)) : List<&2, List<&2, String>>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      match f:
        case HX.Both{f1, f2}:
          hub_put_absent.pick(String.eq(HM.Agent.name(h), n), h, t, n, a, {==},
            hub_or_false_l(String.eq(HM.Agent.name(h), n), HM.str_in(HM.Agent.aliases(h), n), f1), hub_put_absent(t, n, a, f2))


# ---- L4 support: names apart — a rename ----

def hub_str_in_snoc_false(+l: List<&2, String>, +y: String, +z: String, +a: {HM.str_in(l, z) == False{} : Bool}, b: {String.eq(y, z) == False{} : Bool})
    -> {HM.str_in(HM.snoc(String, l, y), z) == False{} : Bool}:
  match l:
    case Nil{}:
      hub_or_false(String.eq(y, z), False{}, b, {==})
    case Con{h, t}:
      hub_or_false(String.eq(h, z), HM.str_in(HM.snoc(String, t, y), z), hub_or_false_l(String.eq(h, z), HM.str_in(t, z), a),
        hub_str_in_snoc_false(t, y, z, hub_or_false_r(String.eq(h, z), HM.str_in(t, z), a), b))

# the renamed entry against one listed after it
def hub_apart_ren_after(+m: String, +al: List<&2, String>, +new: String, +y: List<&2, String>, ay: HI.Apart(m <> al, y), fy: {HM.str_in(y, new) == False{} : Bool})
    -> HI.Apart(new <> HM.snoc(String, al, m), y):
  match y:
    case Nil{}:
      Unit{}
    case Con{yn, ya}:
      match ay:
        case HX.Both{+a1, +a2}:
          HX.Both{fy, hub_str_in_snoc_false(al, m, yn, a2, hub_str_eq_sym_false(yn, m, hub_or_false_l(String.eq(yn, m), HM.str_in(ya, m), a1)))}

def hub_aa_ren(+m: String, +al: List<&2, String>, +new: String, +ps: List<&2, List<&2, String>>, a: HI.ApartAll(m <> al, ps), f: hub_free(ps, new))
    -> HI.ApartAll(new <> HM.snoc(String, al, m), ps):
  match ps:
    case Nil{}:
      Unit{}
    case Con{y, r}:
      match a f:
        case HX.Both{a1, a2} HX.Both{f1, f2}:
          HX.Both{hub_apart_ren_after(m, al, new, y, a1, f1), hub_aa_ren(m, al, new, r, a2, f2)}

# an entry listed before the renamed one
def hub_apart_ren_before(+x: List<&2, String>, +m: String, +al: List<&2, String>, +new: String, ax: HI.Apart(x, m <> al), +fx: {HM.str_in(x, new) == False{} : Bool})
    -> HI.Apart(x, new <> HM.snoc(String, al, m)):
  match x:
    case Nil{}:
      Unit{}
    case Con{xn, xa}:
      match ax:
        case HX.Both{+b1, +b2}:
          HX.Both{hub_or_false(String.eq(new, xn), HM.str_in(HM.snoc(String, al, m), xn),
              hub_str_eq_sym_false(xn, new, hub_or_false_l(String.eq(xn, new), HM.str_in(xa, new), fx)),
              hub_str_in_snoc_false(al, m, xn, hub_or_false_r(String.eq(m, xn), HM.str_in(al, xn), b1), hub_or_false_l(String.eq(m, xn), HM.str_in(al, xn), b1))),
            hub_or_false_r(String.eq(xn, new), HM.str_in(xa, new), fx)}

def hub_aa_map_ren.top(+x: List<&2, String>, h: HM.Agent, +t: List<&2, HM.Agent>, +new: String, ah: HI.Apart(x, HI.names(h)), +fx: {HM.str_in(x, new) == False{} : Bool},
    at: HI.ApartAll(x, HI.profile(t))) -> HI.ApartAll(x, HI.profile(HM.agent_op(h, HM.ORename{new}) <> t)):
  match h:
    case HM.Agent{name, im, pa, life, de, al, fi, ws, di, br, cr, fa, no, rp, sn}:
      HX.Both{hub_apart_ren_before(x, name, al, new, ah, fx), at}

def hub_aa_map_ren.pick(c: Bool, +x: List<&2, String>, +h: HM.Agent, +t: List<&2, HM.Agent>, +old: String, +new: String, +e: {String.eq(HM.Agent.name(h), old) == c : Bool},
    ah: HI.Apart(x, HI.names(h)), +fx: {HM.str_in(x, new) == False{} : Bool},
    at: HI.ApartAll(x, HI.profile(t)), ih: HI.ApartAll(x, HI.profile(HM.map_agent(t, old, HM.ORename{new}))))
    -> HI.ApartAll(x, HI.profile(Bool.pick(List<&2, HM.Agent>, String.eq(HM.Agent.name(h), old), HM.agent_op(h, HM.ORename{new}) <> t, h <> HM.map_agent(t, old, HM.ORename{new})))):
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HM.Agent.name(h), old), True{}, e) : HI.ApartAll(x, HI.profile(Bool.pick(List<&2, HM.Agent>, _, HM.agent_op(h, HM.ORename{new}) <> t, h <> HM.map_agent(t, old, HM.ORename{new}))))
      hub_aa_map_ren.top(x, h, t, new, ah, fx, at)
    case False{}:
      %Equal.sym(Bool, String.eq(HM.Agent.name(h), old), False{}, e) : HI.ApartAll(x, HI.profile(Bool.pick(List<&2, HM.Agent>, _, HM.agent_op(h, HM.ORename{new}) <> t, h <> HM.map_agent(t, old, HM.ORename{new}))))
      HX.Both{ah, ih}

# entries listed before a renamed one stay apart from the renamed list
def hub_aa_map_ren(+x: List<&2, String>, +t: List<&2, HM.Agent>, +old: String, +new: String, a: HI.ApartAll(x, HI.profile(t)), +fx: {HM.str_in(x, new) == False{} : Bool})
    -> HI.ApartAll(x, HI.profile(HM.map_agent(t, old, HM.ORename{new}))):
  match t:
    case Nil{}:
      Unit{}
    case Con{h, r}:
      match a:
        case HX.Both{+a1, +a2}:
          hub_aa_map_ren.pick(String.eq(HM.Agent.name(h), old), x, h, r, old, new, {==}, a1, fx, a2, hub_aa_map_ren(x, r, old, new, a2, fx))

def hub_nok_ren.top(h: HM.Agent, +t: List<&2, HM.Agent>, +new: String, a1: HI.ApartAll(HI.names(h), HI.profile(t)), ft: hub_free(HI.profile(t), new),
    nt: HI.NamesOK(HI.profile(t))) -> HI.NamesOK(HI.profile(HM.agent_op(h, HM.ORename{new}) <> t)):
  match h:
    case HM.Agent{name, im, pa, life, de, al, fi, ws, di, br, cr, fa, no, rp, sn}:
      HX.Both{hub_aa_ren(name, al, new, HI.profile(t), a1, ft), nt}

def hub_nok_ren.pick(c: Bool, +h: HM.Agent, +t: List<&2, HM.Agent>, +old: String, +new: String, +e: {String.eq(HM.Agent.name(h), old) == c : Bool},
    a1: HI.ApartAll(HI.names(h), HI.profile(t)), nt: HI.NamesOK(HI.profile(t)), +fh: {HM.str_in(HI.names(h), new) == False{} : Bool},
    ft: hub_free(HI.profile(t), new), ih: HI.NamesOK(HI.profile(HM.map_agent(t, old, HM.ORename{new}))))
    -> HI.NamesOK(HI.profile(Bool.pick(List<&2, HM.Agent>, String.eq(HM.Agent.name(h), old), HM.agent_op(h, HM.ORename{new}) <> t, h <> HM.map_agent(t, old, HM.ORename{new})))):
  match c:
    case True{}:
      %Equal.sym(Bool, String.eq(HM.Agent.name(h), old), True{}, e) : HI.NamesOK(HI.profile(Bool.pick(List<&2, HM.Agent>, _, HM.agent_op(h, HM.ORename{new}) <> t, h <> HM.map_agent(t, old, HM.ORename{new}))))
      hub_nok_ren.top(h, t, new, a1, ft, nt)
    case False{}:
      %Equal.sym(Bool, String.eq(HM.Agent.name(h), old), False{}, e) : HI.NamesOK(HI.profile(Bool.pick(List<&2, HM.Agent>, _, HM.agent_op(h, HM.ORename{new}) <> t, h <> HM.map_agent(t, old, HM.ORename{new}))))
      HX.Both{hub_aa_map_ren(HI.names(h), t, old, new, a1, fh), ih}

# a rename to a free name keeps every agent's names apart
def hub_nok_ren(+xs: List<&2, HM.Agent>, +old: String, +new: String, nok: HI.NamesOK(HI.profile(xs)), f: hub_free(HI.profile(xs), new))
    -> HI.NamesOK(HI.profile(HM.map_agent(xs, old, HM.ORename{new}))):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      match nok f:
        case HX.Both{+a1, +nt} HX.Both{+fh, +ft}:
          hub_nok_ren.pick(String.eq(HM.Agent.name(h), old), h, t, old, new, {==}, a1, nt, fh, ft, hub_nok_ren(t, old, new, nt, ft))

def hub_nok_tr(-x: List<&2, List<&2, String>>, -y: List<&2, List<&2, String>>, e: {x == y : List<&2, List<&2, String>>}, d: HI.NamesOK(y)) -> HI.NamesOK(x):
  %Equal.sym(List<&2, List<&2, String>>, x, y, e) : HI.NamesOK(_)
  d

# every event with its journal invariant (a free name for a new task or a
# new name) keeps the names of the agents apart
def hub_apply_names(s: HM.St, e: HM.Ev, u: HI.NamesApart(s), +k: HX.FreshEv(s, e)) -> HI.NamesApart(HM.apply(s, e)):
  match s:
    case HM.St{+ag, +od, +ms, +ca, +no, +nm, +nc, +tm}:
      match e:
        case HM.ETaskCreated{+n, pa, br, ws, at}:
          hub_nok_tr(HI.profile(HM.put_agent(ag, n, HM.new_agent(n, False{}, Some{pa}, ws, br, at))), HM.snoc(List<&2, String>, HI.profile(ag), [n]),
            hub_put_absent(ag, n, HM.new_agent(n, False{}, Some{pa}, ws, br, at), hub_nt(HM.St{ag, od, ms, ca, no, nm, nc, tm}, n, k)),
            hub_nok_snoc(HI.profile(ag), [n], u, hub_allapart_new(HI.profile(ag), n, hub_nt(HM.St{ag, od, ms, ca, no, nm, nc, tm}, n, k))))
        case HM.ERenamed{old, +new}:
          hub_nok_ren(ag, old, new, u, hub_nt(HM.St{ag, od, ms, ca, no, nm, nc, tm}, new, k))
        case HM.ELifecycle{+x, +l, +r}:
          hub_nok_tr(HI.profile(HM.map_agent(ag, x, HM.OLife{l, r})), HI.profile(ag), hub_prof_map(ag, x, HM.OLife{l, r}, {==}), u)
        case HM.EWorkspace{+x, +w}:
          hub_nok_tr(HI.profile(HM.map_agent(ag, x, HM.OWs{w})), HI.profile(ag), hub_prof_map(ag, x, HM.OWs{w}, {==}), u)
        case HM.EDeclared{+x, +d, +nt}:
          hub_nok_tr(HI.profile(HM.map_agent(ag, x, HM.ODeclared{d, nt})), HI.profile(ag), hub_prof_map(ag, x, HM.ODeclared{d, nt}, {==}), u)
        case HM.EFileTouched{+x, +p}:
          hub_nok_tr(HI.profile(HM.map_agent(ag, x, HM.OFile{p})), HI.profile(ag), hub_prof_map(ag, x, HM.OFile{p}, {==}), u)
        case HM.ESnapshot{+x, +sn}:
          hub_nok_tr(HI.profile(HM.map_agent(ag, x, HM.OSnap{sn})), HI.profile(ag), hub_prof_map(ag, x, HM.OSnap{sn}, {==}), u)
        case HM.EReported{+x, +rp}:
          hub_nok_tr(HI.profile(HM.map_agent(ag, x, HM.OReport{rp})), HI.profile(ag), hub_prof_map(ag, x, HM.OReport{rp}, {==}), u)
        case HM.EMsgSent{m}:
          u
        case HM.EMsgState{a, b}:
          u
        case HM.ESettled{a}:
          u
        case HM.ECardOpened{c}:
          u
        case HM.ECardClosed{a, b}:
          u
        case HM.EMainNote{a, b}:
          u
        case HM.ENotesFlushed{}:
          u
        case HM.ETimerSet{a, b}:
          u
        case HM.ETimerFired{a, b, c, d, f}:
          u
        case HM.ETimerRun{a, b}:
          u
        case HM.ETimerStop{a, b, c}:
          u

def hub_apply_all_names(+evs: List<&2, HM.Ev>, +s: HM.St, u: HI.NamesApart(s), f: HX.Fresh(evs, s)) -> HI.NamesApart(HM.apply_all(evs, s)):
  match evs:
    case Nil{}:
      u
    case Con{e, t}:
      match f:
        case HX.Both{k, ft}:
          hub_apply_all_names(t, HM.apply(s, e), hub_apply_names(s, e, u, k), ft)

def hub_napart_tr(-x: HM.St, -y: HM.St, e: {x == y : HM.St}, u: HI.NamesApart(y)) -> HI.NamesApart(x):
  %Equal.sym(HM.St, x, y, e) : HI.NamesApart(_)
  u

def Laws.init_names_apart(w):
  HX.Both{Unit{}, Unit{}}

def Laws.names_apart(ins, h, u):
  hub_napart_tr(HC.Hub.d(HC.RunR.hub(HC.run(ins, h))), HM.apply_all(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h)),
    Laws.hub_run_replays(ins, h),
    hub_apply_all_names(HC.RunR.journal(HC.run(ins, h)), HC.Hub.d(h), u, Laws.msg_ids_fresh(ins, h)))


# ---- L4: the frame of an agent, by its activity and its queue ----

def hub_act(m: Maybe<&2, HM.Agent>) -> Bool:
  match m:
    case None{}:
      False{}
    case Some{a}:
      HC.is_active(HM.Agent.life(a))

def hub_sa_act(m: Maybe<&2, HM.Agent>, +s: HM.St, +n: String) -> {HI.stuck.agent(m, s, n) == Bool.and(hub_act(m), Bool.not(hub_q(s, n))) : Bool}:
  match m:
    case None{}:
      {==}
    case Some{a}:
      {==}

# the state part of an agent's frame: same activity, a queue that can only empty
def hub_sf(+s1: HM.St, +s2: HM.St, +n: String) -> Data:
  HX.Both<{hub_act(HM.get_agent(s2, n)) == hub_act(HM.get_agent(s1, n)) : Bool}, {hub_qimp(s1, s2, n) == True{} : Bool}>

def hub_frame2.q(a: Bool, +q1: Bool, +q2: Bool, h: {Bool.and(a, Bool.not(q1)) == False{} : Bool}, qi: {Bool.or(Bool.not(q1), q2) == True{} : Bool})
    -> {Bool.and(a, Bool.not(q2)) == False{} : Bool}:
  match a:
    case False{}:
      {==}
    case True{}:
      hub_sa.q(q1, q2, {==}, h, qi)

def hub_frame2.h(m: Maybe<&2, HM.Agent>, +s: HM.St, +n: String, h: {HI.stuck.agent(m, s, n) == False{} : Bool}) -> {Bool.and(hub_act(m), Bool.not(hub_q(s, n))) == False{} : Bool}:
  match m:
    case None{}:
      {==}
    case Some{a}:
      h

def hub_frame2.sa(+s1: HM.St, +s2: HM.St, +n: String, sf: hub_sf(s1, s2, n), h: {HI.stuck.agent(HM.get_agent(s1, n), s1, n) == False{} : Bool})
    -> {HI.stuck.agent(HM.get_agent(s2, n), s2, n) == False{} : Bool}:
  match sf:
    case HX.Both{ea, qi}:
      %Equal.sym(Bool, HI.stuck.agent(HM.get_agent(s2, n), s2, n), Bool.and(hub_act(HM.get_agent(s2, n)), Bool.not(hub_q(s2, n))), hub_sa_act(HM.get_agent(s2, n), s2, n)) : {_ == False{} : Bool}
      %Equal.sym(Bool, hub_act(HM.get_agent(s2, n)), hub_act(HM.get_agent(s1, n)), ea) : {Bool.and(_, Bool.not(hub_q(s2, n))) == False{} : Bool}
      hub_frame2.q(hub_act(HM.get_agent(s1, n)), hub_q(s1, n), hub_q(s2, n),
        hub_frame2.h(HM.get_agent(s1, n), s1, n, h), qi)

def Laws.image_split_marker():
  {==}

def Laws.image_split_refuses_bad_marker():
  {==}

def Laws.api_body_image_user_parts():
  {==}

def Laws.api_body_image_anth_tool_result():
  {==}

def Laws.image_keep_last():
  {==}

def Laws.image_defang_unkept():
  {==}

def Laws.image_splice_data():
  {==}

def Laws.anth_cache_breakpoints():
  {==}

def Laws.anth_thinking_none_and_no_cache_on_thinking():
  {==}

def Laws.anth_thinking_budget():
  {==}

def Laws.anth_thinking_budget_too_small():
  {==}

def Laws.anth_thinking_default_follows_reasoning():
  {==}

def Laws.anth_thinking_default_reasons_adaptive():
  {==}

def Laws.anth_thinking_words():
  {==}

def Laws.family_headers_anthropic_betas():
  {==}

def Laws.api_body_anth_follows_facts():
  {==}

def Laws.choice_model_first():
  {==}

def Laws.choice_empty_keeps_the_role_order():
  {==}

def Laws.effort_order():
  {==}

def Laws.effort_outside_the_list_falls_to_the_default():
  {==}

def Laws.effort_none_without_reasoning():
  {==}

def Laws.window_follows_the_choice():
  {==}

def Laws.oai_body_sends_the_effort():
  {==}

def Laws.anth_adaptive_effort():
  {==}

def Laws.anth_budget_follows_the_effort():
  {==}

# ---- calls and results stay paired (BISE-242) ----

def pair_closes(+owed: List<&2, T.Call>) -> {H.paired(H.closes(owed), owed) == True{} : Bool}:
  match owed:
    case Nil{}:
      {==}
    case Con{c, rest}:
      match c:
        case T.Call{i, n, a}:
          pair_closes(rest)

def pair_closes_then(+owed: List<&2, T.Call>, +x: List<&2, T.Msg>)
    -> {H.paired(List.append(&2, T.Msg, H.closes(owed), x), owed) == H.paired(x, Nil{}) : Bool}:
  match owed:
    case Nil{}:
      {==}
    case Con{c, rest}:
      match c:
        case T.Call{i, n, a}:
          pair_closes_then(rest, x)

def pair_go(+ms: List<&2, T.Msg>, +owed: List<&2, T.Call>)
    -> {H.paired(H.pair_calls.go(ms, owed), owed) == True{} : Bool}:
  match ms:
    case Nil{}:
      pair_closes(owed)
    case Con{m, rest}:
      match m:
        case T.Msg{role, text, calls, inj}:
          match role:
            case T.RTool{}:
              match owed:
                case Nil{}:
                  pair_go(rest, Nil{})
                case Con{c, ow}:
                  pair_go(rest, ow)
            case T.RAsst{}:
              Equal.trans(Bool,
                H.paired(List.append(&2, T.Msg, H.closes(owed),
                  Con{T.Msg{T.RAsst{}, text, calls, inj}, H.pair_calls.go(rest, calls)}), owed),
                H.paired(Con{T.Msg{T.RAsst{}, text, calls, inj}, H.pair_calls.go(rest, calls)}, Nil{}),
                True{},
                pair_closes_then(owed, Con{T.Msg{T.RAsst{}, text, calls, inj}, H.pair_calls.go(rest, calls)}),
                pair_go(rest, calls))
            case T.RUser{}:
              Equal.trans(Bool,
                H.paired(List.append(&2, T.Msg, H.closes(owed),
                  Con{T.Msg{T.RUser{}, text, calls, inj}, H.pair_calls.go(rest, Nil{})}), owed),
                H.paired(Con{T.Msg{T.RUser{}, text, calls, inj}, H.pair_calls.go(rest, Nil{})}, Nil{}),
                True{},
                pair_closes_then(owed, Con{T.Msg{T.RUser{}, text, calls, inj}, H.pair_calls.go(rest, Nil{})}),
                pair_go(rest, Nil{}))
            case T.RSys{}:
              Equal.trans(Bool,
                H.paired(List.append(&2, T.Msg, H.closes(owed),
                  Con{T.Msg{T.RSys{}, text, calls, inj}, H.pair_calls.go(rest, Nil{})}), owed),
                H.paired(Con{T.Msg{T.RSys{}, text, calls, inj}, H.pair_calls.go(rest, Nil{})}, Nil{}),
                True{},
                pair_closes_then(owed, Con{T.Msg{T.RSys{}, text, calls, inj}, H.pair_calls.go(rest, Nil{})}),
                pair_go(rest, Nil{}))

def Laws.pair_calls_paired(ms):
  pair_go(ms, Nil{})

def Laws.model_input_paired(cfg, h):
  match cfg:
    case T.Cfg{a, b, c, d, e}:
      pair_go(h, Nil{})

def pair_keep(+ms: List<&2, T.Msg>, +owed: List<&2, T.Call>,
    p: {H.paired(ms, owed) == True{} : Bool})
    -> {H.pair_calls.go(ms, owed) == ms : List<&2, T.Msg>}:
  match ms:
    case Nil{}:
      match owed:
        case Nil{}:
          {==}
        case Con{c, ow}:
          hub_absurd_eq(hub_ft_absurd(p), List<&2, T.Msg>, H.pair_calls.go(Nil{}, Con{c, ow}), Nil{})
    case Con{m, rest}:
      match m:
        case T.Msg{role, text, calls, inj}:
          match role:
            case T.RTool{}:
              match owed:
                case Nil{}:
                  hub_absurd_eq(hub_ft_absurd(p), List<&2, T.Msg>,
                    H.pair_calls.go(Con{T.Msg{T.RTool{}, text, calls, inj}, rest}, Nil{}),
                    Con{T.Msg{T.RTool{}, text, calls, inj}, rest})
                case Con{c, ow}:
                  Equal.cong(List<&2, T.Msg>, List<&2, T.Msg>,
                    x => Con{T.Msg{T.RTool{}, text, calls, inj}, x},
                    H.pair_calls.go(rest, ow), rest, pair_keep(rest, ow, p))
            case T.RAsst{}:
              match owed:
                case Nil{}:
                  Equal.cong(List<&2, T.Msg>, List<&2, T.Msg>,
                    x => Con{T.Msg{T.RAsst{}, text, calls, inj}, x},
                    H.pair_calls.go(rest, calls), rest, pair_keep(rest, calls, p))
                case Con{c, ow}:
                  hub_absurd_eq(hub_ft_absurd(p), List<&2, T.Msg>,
                    H.pair_calls.go(Con{T.Msg{T.RAsst{}, text, calls, inj}, rest}, Con{c, ow}),
                    Con{T.Msg{T.RAsst{}, text, calls, inj}, rest})
            case T.RUser{}:
              match owed:
                case Nil{}:
                  Equal.cong(List<&2, T.Msg>, List<&2, T.Msg>,
                    x => Con{T.Msg{T.RUser{}, text, calls, inj}, x},
                    H.pair_calls.go(rest, Nil{}), rest, pair_keep(rest, Nil{}, p))
                case Con{c, ow}:
                  hub_absurd_eq(hub_ft_absurd(p), List<&2, T.Msg>,
                    H.pair_calls.go(Con{T.Msg{T.RUser{}, text, calls, inj}, rest}, Con{c, ow}),
                    Con{T.Msg{T.RUser{}, text, calls, inj}, rest})
            case T.RSys{}:
              match owed:
                case Nil{}:
                  Equal.cong(List<&2, T.Msg>, List<&2, T.Msg>,
                    x => Con{T.Msg{T.RSys{}, text, calls, inj}, x},
                    H.pair_calls.go(rest, Nil{}), rest, pair_keep(rest, Nil{}, p))
                case Con{c, ow}:
                  hub_absurd_eq(hub_ft_absurd(p), List<&2, T.Msg>,
                    H.pair_calls.go(Con{T.Msg{T.RSys{}, text, calls, inj}, rest}, Con{c, ow}),
                    Con{T.Msg{T.RSys{}, text, calls, inj}, rest})

def Laws.pair_calls_keeps_paired(ms, p):
  pair_keep(ms, Nil{}, p)

def Laws.pair_wire_answers_the_cut():
  {==}

# ---- the prompt cache (BISE-268): by computation ----

def Laws.image_keep_stepped():
  {==}

def Laws.image_keep_stepped_same_drop():
  {==}

def Laws.image_keep_next_step():
  {==}

def Laws.prefix_stable_oai():
  {==}

def Laws.prefix_stable_anth():
  {==}

def Laws.anth_context_not_a_breakpoint():
  {==}

def Laws.oai_context_is_user():
  {==}

def Laws.session_key_stable():
  {==}

def Laws.session_key_is_a_hash():
  {==}

def Laws.cache_key_field():
  {==}

def Laws.cache_key_field_none():
  {==}

def Laws.cache_key_header():
  {==}

def Laws.cache_key_header_none():
  {==}

def Laws.extra_headers_lines():
  {==}

def Laws.extra_headers_skipped():
  {==}

def Laws.extra_headers_none():
  {==}

def Laws.key_command_bearer():
  {==}

def Laws.key_command_bearer_off():
  {==}

def Laws.compact_first_attempt_shares():
  {==}

def Laws.compact_retry_is_bare():
  {==}

def Laws.context_skips_compaction_with_tools():
  {==}

def Laws.live_catalog_edit_toolset_has_six_tools():
  {==}

def Laws.live_catalog_patch_toolset_has_five_tools():
  {==}

def Laws.parity_catalog_keeps_three():
  {==}

def Laws.live_catalog_edit_names():
  {==}

def Laws.live_catalog_patch_names():
  {==}

def Laws.apply_patch_desc_names_the_format():
  {==}

def Laws.edit_tools_carry_vibe_descriptions():
  {==}

def Laws.bash_desc_names_the_edit_tool():
  {==}

def Laws.catalog_toolset_read_back():
  {==}

def Laws.patch_provider_is_openai_only():
  {==}

def Laws.edit_steering_lines():
  {==}

def Laws.retool_swaps_tools_and_line():
  {==}

def Laws.edit_err_empty_path():
  {==}

def Laws.edit_err_empty_old():
  {==}

def Laws.edit_err_same():
  {==}

def Laws.edit_err_missing():
  {==}

def Laws.edit_err_not_file():
  {==}

def Laws.edit_err_not_found():
  {==}

def Laws.edit_err_many():
  {==}

def Laws.edit_err_not_text():
  {==}

def Laws.edit_err_permission():
  {==}

def Laws.edit_ok_messages():
  {==}

def Laws.edit_result_text():
  {==}

def Laws.write_err_empty_path():
  {==}

def Laws.write_err_too_big():
  {==}

def Laws.write_err_exists():
  {==}

def Laws.write_result_text():
  {==}

def Laws.edit_check_args_order():
  {==}

def Laws.write_check_order():
  {==}

def Laws.edit_apply_cases():
  {==}

def Laws.edit_keeps_crlf():
  {==}

def Laws.utf8_bad_offsets():
  {==}

def Laws.edit_path_resolves():
  {==}

def Laws.gate_what_is_gated():
  {==}

def Laws.gate_only_auto_asks():
  {==}

def Laws.gate_reads_its_own_verdict():
  {==}

def Laws.gate_poll_order():
  {==}

def Laws.gate_results_and_lines():
  {==}

def Laws.gate_sandbox_rerun_line():
  {==}

def Laws.gate_sandbox_denied_tail():
  {==}

def Laws.edit_gate_args():
  {==}

def Laws.edit_views_are_patches():
  {==}

def Laws.edit_schemas():
  {==}

def Laws.edit_calls_ride_structured():
  {==}

def Laws.image_scope_tool_text():
  {==}

def Laws.image_scope_run_typescript():
  {==}

def Laws.image_marker_newline_is_text():
  {==}

def Laws.image_part_unavailable_is_text():
  {==}

def Laws.image_part_filled():
  {==}

def Laws.image_part_cap():
  {==}

def Laws.image_part_fit_bytes():
  {==}

def Laws.image_part_fit_count_then_bytes():
  {==}

def Laws.image_part_fit_under_limit():
  {==}

def Laws.image_part_fit_none():
  {==}

def Laws.image_fill_fit_weighs():
  {==}

def Laws.image_why_checks():
  {==}

# provider-timeout: idle_timeout_sec (runtime/provider-pure.bend)
def Laws.provider_idle_timeout_default():
  {==}

def Laws.provider_idle_timeout_set():
  {==}

def Laws.attempt_timeout_names_the_key():
  {==}

def Laws.attempt_err_in_others():
  {==}

# ---- the timers of `sb every` (hub/timers.bend, core.bend `timer_fire`) ----

# an emit adds its event at the end of the step's journal
def hub_fx_emit(-o: HM.St, cx: HX.Cx<o>, -e: HM.Ev, -k: HX.FreshEv(HX.d(o, cx), e), -kq: HX.QuietEv(e), -kv: HX.DelivEv(HX.d(o, cx), e))
    -> {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HX.emit(o, cx, e, k, kq, kv)))) == HM.snoc(HM.Ev, HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))), e) : List<&2, HM.Ev>}:
  match cx:
    case HX.Cx{c, r}:
      match c:
        case HX.Core{d, fx, p, f, q, v}:
          Equal.sym(List<&2, HM.Ev>, HM.snoc(HM.Ev, HX.journal(fx), e), HX.journal(HM.snoc(HX.Fx, fx, HX.FJ{e})), HX.journal_snoc_ev(fx, e))

def hub_spent_journal(-o: HM.St, +sp: Bool, +cx: HX.Cx<o>, +id: Nat, +at: Nat)
    -> {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.timer_spent(o, sp, cx, id, at)))) == HI.fire_spent(sp, HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))), id, at) : List<&2, HM.Ev>}:
  match sp:
    case True{}:
      hub_fx_emit(o, cx, HM.ETimerStop{id, "it ran its times", at}, Unit{}, Unit{}, Unit{})
    case False{}:
      {==}

def hub_count_journal(-o: HM.St, +due: Bool, +cx: HX.Cx<o>, +t: HTM.Timer, +next: Nat)
    -> {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.timer_count(o, due, cx, t, next)))) == HI.fire_count(due, HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))), t, next, HC.now(o, cx)) : List<&2, HM.Ev>}:
  match due:
    case False{}:
      hub_fx_emit(o, cx, HM.ETimerRun{HTM.Timer.id(t), HC.now(o, cx)}, Unit{}, Unit{}, Unit{})
    case True{}:
      +ev = {HM.ETimerFired{HTM.Timer.id(t), 1n+HTM.Timer.fired(t), next, HC.now(o, cx), False{}} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      Equal.trans(List<&2, HM.Ev>,
        HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.timer_spent(o, HTM.spent(t), cx2, HTM.Timer.id(t), HC.now(o, cx))))),
        HI.fire_spent(HTM.spent(t), HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx2))), HTM.Timer.id(t), HC.now(o, cx)),
        HI.fire_spent(HTM.spent(t), HM.snoc(HM.Ev, HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))), ev), HTM.Timer.id(t), HC.now(o, cx)),
        hub_spent_journal(o, HTM.spent(t), cx2, HTM.Timer.id(t), HC.now(o, cx)),
        Equal.cong(List<&2, HM.Ev>, List<&2, HM.Ev>, xs => HI.fire_spent(HTM.spent(t), xs, HTM.Timer.id(t), HC.now(o, cx)),
          HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx2))), HM.snoc(HM.Ev, HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))), ev),
          hub_fx_emit(o, cx, ev, Unit{}, Unit{}, Unit{})))

def hub_retry_journal(-o: HM.St, +due: Bool, +cx: HX.Cx<o>, +t: HTM.Timer)
    -> {HX.journal(HX.Core.fx(o, HX.Cx.c(o, HC.timer_retry(o, due, cx, t)))) == HI.fire_retry(due, HX.journal(HX.Core.fx(o, HX.Cx.c(o, cx))), t, HC.now(o, cx)) : List<&2, HM.Ev>}:
  match due:
    case False{}:
      {==}
    case True{}:
      hub_fx_emit(o, cx, HM.ETimerFired{HTM.Timer.id(t), HTM.Timer.fired(t), Nat.add(HC.now(o, cx), HTM.MIN_MS()), HTM.Timer.last(t), True{}},
        Unit{}, Unit{}, Unit{})

def Laws.fire_is_a_message(o, p, t, due, next):
  match p:
    case (cx, HC.SErr{e}):
      hub_retry_journal(o, due, cx, t)
    case (cx, HC.SOk{m}):
      hub_count_journal(o, due, cx, t, next)

def Laws.wake_never_stacked(o, cx, t, text, due, next, q):
  %Equal.sym(Bool, HC.bise_queued(HX.d(o, cx), HTM.Timer.agent(t)), True{}, q) : {HC.timer_fire.go(o, HTM.may_fire(_, due, t, HC.timer_ready(o, cx, HTM.Timer.agent(t)), HC.now(o, cx)), cx, t, text, due, next) == cx : HX.Cx<o>}
  {==}

# a < b is not b <= a
def hub_lt_not_le(+a: Nat, +b: Nat) -> {Nat.is_lt(a, b) == Bool.not(Nat.is_le(b, a)) : Bool}:
  match a b:
    case 0n 0n:
      {==}
    case 0n 1n+bp:
      {==}
    case 1n+ap 0n:
      {==}
    case 1n+ap 1n+bp:
      hub_lt_not_le(ap, bp)

def Laws.times_bound(t, next, at):
  match t:
    case HTM.Timer{id, ag, by, tx, sc, nx, un, tms, fd, pg, ls, rs}:
      match tms:
        case None{}:
          {==}
        case Some{n}:
          hub_lt_not_le(1n+fd, n)

# ---- stop_leaves_no_timer: a drop's stops leave no timer of the agent ----

def hub_keep_nil(+xs: List<&2, HTM.Timer>) -> {HTM.keep_out(xs, Nil{}) == xs : List<&2, HTM.Timer>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      Equal.cong(List<&2, HTM.Timer>, List<&2, HTM.Timer>, ys => h <> ys, HTM.keep_out(t, Nil{}), t, hub_keep_nil(t))

def hub_keep_drop.pick(c: Bool, +h: HTM.Timer, +t: List<&2, HTM.Timer>, +id: Nat, +rest: List<&2, Nat>,
    +e: {Nat.is_eq(HTM.Timer.id(h), id) == c : Bool},
    ih: {HTM.keep_out(HTM.drop(t, id), rest) == HTM.keep_out(t, id <> rest) : List<&2, HTM.Timer>})
    -> {HTM.keep_out(Bool.pick(List<&2, HTM.Timer>, Nat.is_eq(HTM.Timer.id(h), id), HTM.drop(t, id), h <> HTM.drop(t, id)), rest)
        == Bool.pick(List<&2, HTM.Timer>, Nat.is_eq(HTM.Timer.id(h), id) || HTM.nat_in(rest, HTM.Timer.id(h)), HTM.keep_out(t, id <> rest), h <> HTM.keep_out(t, id <> rest)) : List<&2, HTM.Timer>}:
  match c:
    case True{}:
      %Equal.sym(Bool, Nat.is_eq(HTM.Timer.id(h), id), True{}, e) : {HTM.keep_out(Bool.pick(List<&2, HTM.Timer>, _, HTM.drop(t, id), h <> HTM.drop(t, id)), rest)
        == Bool.pick(List<&2, HTM.Timer>, _ || HTM.nat_in(rest, HTM.Timer.id(h)), HTM.keep_out(t, id <> rest), h <> HTM.keep_out(t, id <> rest)) : List<&2, HTM.Timer>}
      ih
    case False{}:
      %Equal.sym(Bool, Nat.is_eq(HTM.Timer.id(h), id), False{}, e) : {HTM.keep_out(Bool.pick(List<&2, HTM.Timer>, _, HTM.drop(t, id), h <> HTM.drop(t, id)), rest)
        == Bool.pick(List<&2, HTM.Timer>, _ || HTM.nat_in(rest, HTM.Timer.id(h)), HTM.keep_out(t, id <> rest), h <> HTM.keep_out(t, id <> rest)) : List<&2, HTM.Timer>}
      Equal.cong(List<&2, HTM.Timer>, List<&2, HTM.Timer>, ys => Bool.pick(List<&2, HTM.Timer>, HTM.nat_in(rest, HTM.Timer.id(h)), ys, h <> ys),
        HTM.keep_out(HTM.drop(t, id), rest), HTM.keep_out(t, id <> rest), ih)

# dropping one id, then the others: the timers of none of them
def hub_keep_drop(+xs: List<&2, HTM.Timer>, +id: Nat, +rest: List<&2, Nat>)
    -> {HTM.keep_out(HTM.drop(xs, id), rest) == HTM.keep_out(xs, id <> rest) : List<&2, HTM.Timer>}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      hub_keep_drop.pick(Nat.is_eq(HTM.Timer.id(h), id), h, t, id, rest, {==}, hub_keep_drop(t, id, rest))

def hub_live_tstop(s: HM.St, +id: Nat, +why: String, +at: Nat)
    -> {HTM.Tms.live(HM.St.tm(HM.apply(s, HM.ETimerStop{id, why, at}))) == HTM.drop(HTM.Tms.live(HM.St.tm(s)), id) : List<&2, HTM.Timer>}:
  match s:
    case HM.St{ag, od, ms, ca, no, nm, nc, tm}:
      match tm:
        case HTM.Tms{l, e, li}:
          {==}

# the live timers after a drop's stops: those of no stopped id
def hub_live_gone_each(-o: HM.St, +ids: List<&2, Nat>, +cx: HX.Cx<o>, +why: String)
    -> {HTM.Tms.live(HM.St.tm(HX.d(o, HC.timers_gone.each(o, ids, cx, why)))) == HTM.keep_out(HTM.Tms.live(HM.St.tm(HX.d(o, cx))), ids) : List<&2, HTM.Timer>}:
  match ids:
    case Nil{}:
      Equal.sym(List<&2, HTM.Timer>, HTM.keep_out(HTM.Tms.live(HM.St.tm(HX.d(o, cx))), Nil{}), HTM.Tms.live(HM.St.tm(HX.d(o, cx))),
        hub_keep_nil(HTM.Tms.live(HM.St.tm(HX.d(o, cx)))))
    case Con{id, rest}:
      +ev = {HM.ETimerStop{id, why, HC.now(o, cx)} : HM.Ev}
      +cx2 = HC.emit(o, cx, ev, Unit{}, Unit{}, Unit{})
      +l = HTM.Tms.live(HM.St.tm(HX.d(o, cx)))
      +l2 = HTM.Tms.live(HM.St.tm(HX.d(o, cx2)))
      +e2 = Equal.trans(List<&2, HTM.Timer>, l2, HTM.Tms.live(HM.St.tm(HM.apply(HX.d(o, cx), ev))), HTM.drop(l, id),
        Equal.cong(HM.St, List<&2, HTM.Timer>, s => HTM.Tms.live(HM.St.tm(s)), HX.d(o, cx2), HM.apply(HX.d(o, cx), ev), hub_d_emit(o, cx, ev, Unit{}, Unit{}, Unit{})),
        hub_live_tstop(HX.d(o, cx), id, why, HC.now(o, cx)))
      Equal.trans(List<&2, HTM.Timer>, HTM.Tms.live(HM.St.tm(HX.d(o, HC.timers_gone.each(o, rest, cx2, why)))), HTM.keep_out(l2, rest), HTM.keep_out(l, id <> rest),
        hub_live_gone_each(o, rest, cx2, why),
        Equal.trans(List<&2, HTM.Timer>, HTM.keep_out(l2, rest), HTM.keep_out(HTM.drop(l, id), rest), HTM.keep_out(l, id <> rest),
          Equal.cong(List<&2, HTM.Timer>, List<&2, HTM.Timer>, ys => HTM.keep_out(ys, rest), l2, HTM.drop(l, id), e2),
          hub_keep_drop(l, id, rest)))

def hub_and_true_l(+x: Bool, -y: Bool, e: {Bool.and(x, y) == True{} : Bool}) -> {x == True{} : Bool}:
  match x:
    case True{}:
      {==}
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, False{}, True{})

def hub_and_true_r(+x: Bool, -y: Bool, e: {Bool.and(x, y) == True{} : Bool}) -> {y == True{} : Bool}:
  match x:
    case True{}:
      e
    case False{}:
      hub_absurd_eq(hub_ft_absurd(e), Bool, y, True{})

def hub_or_true_and(+c: Bool) -> {Bool.and(Bool.or(c, True{}), True{}) == True{} : Bool}:
  match c:
    case True{}:
      {==}
    case False{}:
      {==}

# an id list stays inside a longer one
def hub_all_weak(+xs: List<&2, Nat>, +h: Nat, +l: List<&2, Nat>, +e: {HTM.all_in(xs, l) == True{} : Bool}) -> {HTM.all_in(xs, h <> l) == True{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{x, r}:
      +e1 = hub_and_true_l(HTM.nat_in(l, x), HTM.all_in(r, l), e)
      +ih = hub_all_weak(r, h, l, hub_and_true_r(HTM.nat_in(l, x), HTM.all_in(r, l), e))
      %Equal.sym(Bool, HTM.nat_in(l, x), True{}, e1) : {Bool.and(Bool.or(Nat.is_eq(x, h), _), HTM.all_in(r, h <> l)) == True{} : Bool}
      %Equal.sym(Bool, HTM.all_in(r, h <> l), True{}, ih) : {Bool.and(Bool.or(Nat.is_eq(x, h), True{}), _) == True{} : Bool}
      hub_or_true_and(Nat.is_eq(x, h))

def hub_all_self(+xs: List<&2, Nat>) -> {HTM.all_in(xs, xs) == True{} : Bool}:
  match xs:
    case Nil{}:
      {==}
    case Con{h, t}:
      %Equal.sym(Bool, Nat.is_eq(h, h), True{}, hub_eq_refl(h)) : {Bool.and(Bool.or(_, HTM.nat_in(t, h)), HTM.all_in(t, h <> t)) == True{} : Bool}
      hub_all_weak(t, h, t, hub_all_self(t))

# the ids of the agent's timers in the list's tail are in `l` too
def hub_all_tail(a: Bool, +h: HTM.Timer, +t: List<&2, HTM.Timer>, +ns: List<&2, String>, +l: List<&2, Nat>,
    +ea: {HTM.str_in(ns, HTM.Timer.agent(h)) == a : Bool}, +hall: {HTM.all_in(HTM.ids_in(h <> t, ns), l) == True{} : Bool})
    -> {HTM.all_in(HTM.ids_in(t, ns), l) == True{} : Bool}:
  match a:
    case True{}:
      +e2 = Equal.cong(Bool, Bool, c => HTM.all_in(Bool.pick(List<&2, Nat>, c, HTM.Timer.id(h) <> HTM.ids_in(t, ns), HTM.ids_in(t, ns)), l),
        HTM.str_in(ns, HTM.Timer.agent(h)), True{}, ea)
      hub_and_true_r(HTM.nat_in(l, HTM.Timer.id(h)), HTM.all_in(HTM.ids_in(t, ns), l),
        Equal.trans(Bool, HTM.all_in(HTM.Timer.id(h) <> HTM.ids_in(t, ns), l), HTM.all_in(HTM.ids_in(h <> t, ns), l), True{},
          Equal.sym(Bool, HTM.all_in(HTM.ids_in(h <> t, ns), l), HTM.all_in(HTM.Timer.id(h) <> HTM.ids_in(t, ns), l), e2), hall))
    case False{}:
      +e2 = Equal.cong(Bool, Bool, c => HTM.all_in(Bool.pick(List<&2, Nat>, c, HTM.Timer.id(h) <> HTM.ids_in(t, ns), HTM.ids_in(t, ns)), l),
        HTM.str_in(ns, HTM.Timer.agent(h)), False{}, ea)
      Equal.trans(Bool, HTM.all_in(HTM.ids_in(t, ns), l), HTM.all_in(HTM.ids_in(h <> t, ns), l), True{},
        Equal.sym(Bool, HTM.all_in(HTM.ids_in(h <> t, ns), l), HTM.all_in(HTM.ids_in(t, ns), l), e2), hall)

# a timer kept out of nothing (its id is in no stop) is not the agent's
def hub_str_out(a: Bool, +h: HTM.Timer, +t: List<&2, HTM.Timer>, +ns: List<&2, String>, +l: List<&2, Nat>,
    +ea: {HTM.str_in(ns, HTM.Timer.agent(h)) == a : Bool}, +eb: {HTM.nat_in(l, HTM.Timer.id(h)) == False{} : Bool},
    +hall: {HTM.all_in(HTM.ids_in(h <> t, ns), l) == True{} : Bool})
    -> {HTM.str_in(ns, HTM.Timer.agent(h)) == False{} : Bool}:
  match a:
    case False{}:
      ea
    case True{}:
      +e2 = Equal.cong(Bool, Bool, c => HTM.all_in(Bool.pick(List<&2, Nat>, c, HTM.Timer.id(h) <> HTM.ids_in(t, ns), HTM.ids_in(t, ns)), l),
        HTM.str_in(ns, HTM.Timer.agent(h)), True{}, ea)
      +hin = hub_and_true_l(HTM.nat_in(l, HTM.Timer.id(h)), HTM.all_in(HTM.ids_in(t, ns), l),
        Equal.trans(Bool, HTM.all_in(HTM.Timer.id(h) <> HTM.ids_in(t, ns), l), HTM.all_in(HTM.ids_in(h <> t, ns), l), True{},
          Equal.sym(Bool, HTM.all_in(HTM.ids_in(h <> t, ns), l), HTM.all_in(HTM.Timer.id(h) <> HTM.ids_in(t, ns), l), e2), hall))
      hub_absurd_eq(hub_ft_absurd(Equal.trans(Bool, False{}, HTM.nat_in(l, HTM.Timer.id(h)), True{},
        Equal.sym(Bool, HTM.nat_in(l, HTM.Timer.id(h)), False{}, eb), hin)), Bool, HTM.str_in(ns, HTM.Timer.agent(h)), False{})

def hub_neq_of(c: Bool, +a: String, +n: String, +ns: List<&2, String>, +ec: {String.eq(a, n) == c : Bool},
    ha: {HTM.str_in(ns, a) == False{} : Bool}, hn: {HTM.str_in(ns, n) == True{} : Bool}) -> {String.eq(a, n) == False{} : Bool}:
  match c:
    case False{}:
      ec
    case True{}:
      +s = hub_str_eq_sound(a, n, ec)
      +e3 = Equal.cong(String, Bool, x => HTM.str_in(ns, x), a, n, s)
      hub_absurd_eq(hub_ft_absurd(Equal.trans(Bool, False{}, HTM.str_in(ns, a), True{},
        Equal.sym(Bool, HTM.str_in(ns, a), False{}, ha), Equal.trans(Bool, HTM.str_in(ns, a), HTM.str_in(ns, n), True{}, e3, hn))), Bool, String.eq(a, n), False{})

def hub_none_keep.pick(b: Bool, +h: HTM.Timer, +t: List<&2, HTM.Timer>, +l: List<&2, Nat>, +ns: List<&2, String>, +n: String,
    +eb: {HTM.nat_in(l, HTM.Timer.id(h)) == b : Bool}, +hall: {HTM.all_in(HTM.ids_in(h <> t, ns), l) == True{} : Bool},
    +hn: {HTM.str_in(ns, n) == True{} : Bool}, ih: HI.TimerNoneOf(HTM.keep_out(t, l), n))
    -> HI.TimerNoneOf(Bool.pick(List<&2, HTM.Timer>, HTM.nat_in(l, HTM.Timer.id(h)), HTM.keep_out(t, l), h <> HTM.keep_out(t, l)), n):
  match b:
    case True{}:
      %Equal.sym(Bool, HTM.nat_in(l, HTM.Timer.id(h)), True{}, eb) : HI.TimerNoneOf(Bool.pick(List<&2, HTM.Timer>, _, HTM.keep_out(t, l), h <> HTM.keep_out(t, l)), n)
      ih
    case False{}:
      +ha = hub_str_out(HTM.str_in(ns, HTM.Timer.agent(h)), h, t, ns, l, {==}, eb, hall)
      %Equal.sym(Bool, HTM.nat_in(l, HTM.Timer.id(h)), False{}, eb) : HI.TimerNoneOf(Bool.pick(List<&2, HTM.Timer>, _, HTM.keep_out(t, l), h <> HTM.keep_out(t, l)), n)
      HX.Both{hub_neq_of(String.eq(HTM.Timer.agent(h), n), HTM.Timer.agent(h), n, ns, {==}, ha, hn), ih}

# keeping out every id of the agent's timers (and maybe more) leaves none
def hub_none_keep(+xs: List<&2, HTM.Timer>, +l: List<&2, Nat>, +ns: List<&2, String>, +n: String,
    +hall: {HTM.all_in(HTM.ids_in(xs, ns), l) == True{} : Bool}, +hn: {HTM.str_in(ns, n) == True{} : Bool})
    -> HI.TimerNoneOf(HTM.keep_out(xs, l), n):
  match xs:
    case Nil{}:
      Unit{}
    case Con{h, t}:
      +ht = hub_all_tail(HTM.str_in(ns, HTM.Timer.agent(h)), h, t, ns, l, {==}, hall)
      hub_none_keep.pick(HTM.nat_in(l, HTM.Timer.id(h)), h, t, l, ns, n, {==}, hall, hn, hub_none_keep(t, l, ns, n, ht, hn))

def hub_str_in_head(+n: String, +r: List<&2, String>) -> {HTM.str_in(n <> r, n) == True{} : Bool}:
  %Equal.sym(Bool, String.eq(n, n), True{}, hub_str_eq_refl(n)) : {Bool.or(_, HTM.str_in(r, n)) == True{} : Bool}
  {==}

def hub_tnone_tr(-x: List<&2, HTM.Timer>, -y: List<&2, HTM.Timer>, -n: String, e: {x == y : List<&2, HTM.Timer>},
    l: HI.TimerNoneOf(y, n)) -> HI.TimerNoneOf(x, n):
  %Equal.sym(List<&2, HTM.Timer>, x, y, e) : HI.TimerNoneOf(_, n)
  l

def Laws.stop_leaves_no_timer(o, cx, n, why):
  +g = HC.stop_task.go(o, cx, n, HM.LArchived{}, why)
  +tg = HC.timers_gone(o, HM.LArchived{}, g, n)
  +st = HC.stop_task(o, cx, n, HM.LArchived{}, why)
  +ns = HM.names_of(HX.d(o, g), n)
  +xs = HTM.Tms.live(HM.St.tm(HX.d(o, g)))
  +ids = HTM.ids_in(xs, ns)
  +e1 = Equal.cong(HM.St, List<&2, HTM.Timer>, s => HTM.Tms.live(HM.St.tm(s)), HX.d(o, st), HX.d(o, tg),
    hub_d_with_r(o, tg, HX.R.with_waiters(HC.rr(o, tg), HC.waiters_not(HX.R.waiters(HC.rr(o, cx)), n))))
  +e2 = hub_live_gone_each(o, ids, g, HC.cat(["@", n, " is gone"]))
  hub_tnone_tr(HTM.Tms.live(HM.St.tm(HX.d(o, st))), HTM.keep_out(xs, ids), n,
    Equal.trans(List<&2, HTM.Timer>, HTM.Tms.live(HM.St.tm(HX.d(o, st))), HTM.Tms.live(HM.St.tm(HX.d(o, tg))), HTM.keep_out(xs, ids), e1, e2),
    hub_none_keep(xs, ids, ns, n, hub_all_self(ids), hub_str_in_head(n, HM.names_of.go(HM.get_agent(HX.d(o, g), n)))))
