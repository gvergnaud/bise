# bend/proofs/patch.bend: the proofs of bend/laws/patch.bend (moved from
# PROOF.bend). Each holds by unfolding the definitions involved; the
# checker verifies the equalities.
import Base
import ../laws/patch.bend as Laws

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

def Laws.attempt_tls_no_library_no_network():
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

def Laws.exec_of_success_is_ok():
  {==}

def Laws.exec_of_failure_carries_code():
  {==}

def Laws.exec_of_garbage_fails_closed():
  {==}

def Laws.exec_of_no_newline_fails_closed():
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
