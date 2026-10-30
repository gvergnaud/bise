# bend/proofs/models.bend: the proofs of bend/laws/models.bend (moved from
# PROOF.bend). Each holds by unfolding the definitions involved; the
# checker verifies the equalities.
import Base
import ../core/types.bend as T
import ../core/history.bend as H
import ./lib/hub-1.bend as PL1
import ../laws/models.bend as Laws

def Laws.image_split_marker():
  {==}

def Laws.image_split_caption():
  {==}

def Laws.image_frame_caption():
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

def Laws.anth_no_empty_text_block():
  {==}

def Laws.oai_no_empty_content():
  {==}

def Laws.resp_no_empty_content():
  {==}

def Laws.deepseek_no_empty_content():
  {==}

def Laws.anth_image_only_user_has_no_empty_text():
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

def Laws.effort_without_models_file_is_none():
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
          PL1.hub_absurd_eq(PL1.hub_ft_absurd(p), List<&2, T.Msg>, H.pair_calls.go(Nil{}, Con{c, ow}), Nil{})
    case Con{m, rest}:
      match m:
        case T.Msg{role, text, calls, inj}:
          match role:
            case T.RTool{}:
              match owed:
                case Nil{}:
                  PL1.hub_absurd_eq(PL1.hub_ft_absurd(p), List<&2, T.Msg>,
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
                  PL1.hub_absurd_eq(PL1.hub_ft_absurd(p), List<&2, T.Msg>,
                    H.pair_calls.go(Con{T.Msg{T.RAsst{}, text, calls, inj}, rest}, Con{c, ow}),
                    Con{T.Msg{T.RAsst{}, text, calls, inj}, rest})
            case T.RUser{}:
              match owed:
                case Nil{}:
                  Equal.cong(List<&2, T.Msg>, List<&2, T.Msg>,
                    x => Con{T.Msg{T.RUser{}, text, calls, inj}, x},
                    H.pair_calls.go(rest, Nil{}), rest, pair_keep(rest, Nil{}, p))
                case Con{c, ow}:
                  PL1.hub_absurd_eq(PL1.hub_ft_absurd(p), List<&2, T.Msg>,
                    H.pair_calls.go(Con{T.Msg{T.RUser{}, text, calls, inj}, rest}, Con{c, ow}),
                    Con{T.Msg{T.RUser{}, text, calls, inj}, rest})
            case T.RSys{}:
              match owed:
                case Nil{}:
                  Equal.cong(List<&2, T.Msg>, List<&2, T.Msg>,
                    x => Con{T.Msg{T.RSys{}, text, calls, inj}, x},
                    H.pair_calls.go(rest, Nil{}), rest, pair_keep(rest, Nil{}, p))
                case Con{c, ow}:
                  PL1.hub_absurd_eq(PL1.hub_ft_absurd(p), List<&2, T.Msg>,
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
