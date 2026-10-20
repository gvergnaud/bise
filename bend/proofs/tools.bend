# bend/proofs/tools.bend: the proofs of bend/laws/tools.bend (moved from
# PROOF.bend). Each holds by unfolding the definitions involved; the
# checker verifies the equalities.
import Base
import ../core/types.bend as T
import ../laws/tools.bend as Laws

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

def Laws.api_tool_call_renamed_arg():
  {==}

def Laws.flat_in_renamed_arg_with_description():
  {==}

def Laws.flat_in_two_fields_no_guess():
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

def Laws.skills_scan_note_quiet():
  {==}

def Laws.skills_scan_note_names_the_skipped():
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

def Laws.glm_thinking_signature_surfaces():
  {==}

def Laws.glm_signed_think_signature_replays():
  {==}

def Laws.live_catalog_edit_toolset_has_seven_tools():
  {==}

def Laws.live_catalog_patch_toolset_has_six_tools():
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

def Laws.edit_apply_lines():
  {==}

def Laws.text_lines_counts():
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

def Laws.read_schema_is_vibes():
  {==}

def Laws.read_desc_is_vibes():
  {==}

def Laws.read_errors():
  {==}

def Laws.read_warnings():
  {==}

def Laws.read_numbers_like_vibe():
  {==}

def Laws.read_result_windows():
  {==}

def Laws.read_pages():
  {==}

def Laws.read_cap():
  {==}

def Laws.read_args_parse():
  {==}

def Laws.read_args_offset_zero_is_none():
  {==}

def Laws.read_empty_path_refused():
  {==}

def Laws.read_ann_names_the_window():
  {==}
