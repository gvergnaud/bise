# The DeepSeek adapter's offline regression proofs.
import Base
import ../laws/deepseek.bend as Laws

def Laws.deepseek_provider_resolves_its_dialect():
  {==}

def Laws.deepseek_dialect_is_provider_and_family_specific():
  {==}

def Laws.deepseek_native_whitespace_is_verbatim():
  {==}

def Laws.deepseek_no_marker_keeps_visible_whitespace():
  {==}

def Laws.deepseek_foreign_signatures_stay_out():
  {==}

def Laws.deepseek_all_spans_are_separated_from_visible_text():
  {==}

def Laws.deepseek_normal_history_high():
  {==}

def Laws.deepseek_normal_history_none():
  {==}

def Laws.deepseek_tools_keep_all_turns_reasoning_and_ids():
  {==}

def Laws.deepseek_tools_none_keeps_history():
  {==}

def Laws.deepseek_tools_none_disables_thinking():
  {==}

def Laws.deepseek_foreign_signed_tool_history_is_sanitized():
  {==}

def Laws.deepseek_foreign_signed_tool_history_none_is_sanitized():
  {==}

def Laws.mistral_history_is_unchanged():
  {==}

def Laws.mistral_signed_history_is_unchanged():
  {==}

def Laws.deepseek_whole_reply_keeps_reasoning():
  {==}

def Laws.deepseek_stream_reply_keeps_reasoning_and_calls():
  {==}

def Laws.deepseek_stream_errors_stay_errors():
  {==}

def Laws.deepseek_http_errors_stay_errors():
  {==}
