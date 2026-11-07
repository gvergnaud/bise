# bend/proofs/streams.bend: the proofs of bend/laws/streams.bend (moved from
# PROOF.bend). Each holds by unfolding the definitions involved; the
# checker verifies the equalities.
import Base
import ../laws/streams.bend as Laws

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

def Laws.debug_crumbs_record():
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

def Laws.context_skips_compaction_with_tools():
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

def Laws.usage_carries_timing():
  {==}

def Laws.timing_without_head():
  {==}
