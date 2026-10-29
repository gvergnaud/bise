# bend/proofs/runtime.bend: the proofs of bend/laws/runtime.bend (moved from
# PROOF.bend). Each holds by unfolding the definitions involved; the
# checker verifies the equalities.
import Base
import ../laws/runtime.bend as Laws

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

def Laws.exec_of_exit_still_fails():
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

def Laws.bg_wrapper_runner_frees_slot():
  {==}

def Laws.bg_wrapper_waits_on_runner():
  {==}

def Laws.bg_wrapper_reports_unwritable_files():
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

def Laws.bg_wrapper_caps_only_its_output():
  {==}

def Laws.bg_wrapper_runner_ownership_check():
  {==}

def Laws.bg_wrapper_cleans_its_script():
  {==}

def Laws.bg_wrapper_multiline_command_intact():
  {==}

def Laws.bg_sentinel_derives_from_command():
  {==}


def Laws.ui_field_splits_off():
  {==}

def Laws.ui_field_absent():
  {==}

def Laws.ui_field_is_the_last():
  {==}

def Laws.ui_field_has_no_space():
  {==}

def Laws.index_tool_hides_the_ui_field():
  {==}

def Laws.asks_view_only_local_with_ui():
  {==}

def Laws.view_entry_line():
  {==}

def Laws.view_lines_each_under_its_id():
  {==}

def Laws.views_files_in_side_dir():
  {==}
