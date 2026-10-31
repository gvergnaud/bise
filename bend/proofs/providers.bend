# bend/proofs/providers.bend: the proofs of bend/laws/providers.bend (moved from
# PROOF.bend). Each holds by unfolding the definitions involved; the
# checker verifies the equalities.
import Base
import ../core/api.bend as Api
import ../laws/providers.bend as Laws

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

def Laws.threshold_cap_is_85_percent_of_the_window():
  {==}

def Laws.threshold_rows():
  {==}

def Laws.default_caps_at_450k_from_1m():
  {==}

def Laws.written_value_not_capped_by_450k():
  {==}

def Laws.default_never_past_85():
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

def Laws.threshold_capped_at_85_percent():
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
    case Api.DeepSeek{}:
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

def Laws.api_body_anth_xhigh():
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

def Laws.resp_stream_items_when_completed_empty():
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

def Laws.refusal_names_the_host_of_a_brought_url():
  {==}
