#!/usr/bin/env bash
# Every switchboard check, from the repo root:
#   tests/run_all.sh            # deterministic (no model)
#   tests/run_all.sh --serial   # e2e + tmux tests one by one
#   tests/run_all.sh --live     # + a real-model smoke test
# e2e.py and the tmux tests run in parallel (SB_TEST_JOBS, default 4: each
# has its own tmux session and throwaway hub; more jobs overload the machine
# and the timing-sensitive tests flake). FUZZ_RUNS=2000 for the long fuzz run.
# cargo test and clippy run next to them (the test binaries built first); a
# red test, cargo's or the pool's, is rerun once alone before it counts
# (FLAKY lines, flaky.tsv next to timings.tsv).
set -euo pipefail
cd "$(dirname "$0")/.."
# A test's git never reaches this repo (keep-current m_34361): `git bisect
# run` exports GIT_DIR, and e2e.py's `git init && git config user.email t@t`
# in its throwaway ws wrote core.bare=true and user t into the real
# .git/config, so every sb land said 'not in a repo'. The repo variables go
# (e2e.py drops them too, for a test run alone), and a run that changes this
# repo's core.bare, core.worktree or user.* fails, the diff shown: those keys
# only, as an agent's own git (a branch's upstream, a remote) may write the
# rest of that file during a run.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_NAMESPACE GIT_PREFIX
repo_config="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)/config"
repo_keys() { git config --file "$repo_config" --get-regexp '^(core\.(bare|worktree)|user\.)' 2>/dev/null || :; }
repo_keys_before="$(repo_keys)"
repo_config_guard() {
  local rc=$? now
  now="$(repo_keys)"
  if [ "$now" != "$repo_keys_before" ]; then
    echo "FAILED: this run changed $repo_config (a test's git left its throwaway repo):"
    diff <(echo "$repo_keys_before") <(echo "$now") || :
    exit 1
  fi
  exit $rc
}
trap repo_config_guard EXIT
# bend: the one this tree pins (bend/VERSION, scripts/bend-pin.sh); the
# mcp_* probes compile with the bend on PATH
. scripts/bend-pin.sh
run_all_bend_dir="$(bend_bin_dir "$PWD" "$(bend_tools_home)")" || exit 1
export PATH="$run_all_bend_dir:$HOME/.cargo/bin:$PATH"
live=0 jobs="${SB_TEST_JOBS:-4}" only="" picked=0 pick=""
while [ $# -gt 0 ]; do
  case "$1" in
    --live) live=1 ;;
    --serial) jobs=1 ;;
    --only) only="${2:?--only <tests>}"; shift ;;
    --tests) picked=1 pick="${2-}"; shift ;;
    *) echo "usage: run_all.sh [--serial] [--live] [--only '<e2e/tmux tests>'] [--tests '<e2e/tmux tests>']" >&2; exit 2 ;;
  esac
  shift
done
# --only: those e2e/tmux tests alone, nothing else (gate.sh rerun: the
# tests a red full gate failed, on the same tree, its Rust part green).
# --tests: the Rust part, then only those e2e/tmux tests (gate.sh full's
# selection, tests/test_map.py; "" = none of them)
# every test's wall and CPU time (user + sys of it and what it waited for)
# go to a history shared by every worktree (loop-speed.md §10): the slow
# tests are found from it, and the next run starts the longest ones first.
# Its 8th field is who ran it: gate-<mode> from gate.sh, else adhoc (an
# agent's own loop; loop_audit.py leaves those out of the failing tests)
timings="${SB_TIMINGS:-${BISE_HOME:-$HOME/.bise}/gate/timings.tsv}"
mkdir -p "$(dirname "$timings")" 2>/dev/null || timings=/dev/null
rev="$(git rev-parse --short HEAD 2>/dev/null || echo '?')"
# A red test gets one rerun before it counts (fast-release, gate-lane): a
# cargo test that failed, by its name; an e2e/tmux test that failed in the
# pool, alone. Green the second time = flaky: a "FLAKY <kind> <test>" line
# and a line in flaky.tsv next to timings.tsv (this file owns it:
# <epoch> <rev> cargo|e2e <test>), the flakes' to-do list. More than
# SB_RERUN_MAX (5) reds is a broken tree, not flakes: no rerun.
flaky="${SB_FLAKY:-$(dirname "$timings")/flaky.tsv}"; [ "$timings" = /dev/null ] && flaky=/dev/null
rerun_max="${SB_RERUN_MAX:-5}"
flaky_note() {  # <kind> <test>...
  local k=$1 t; shift
  for t in "$@"; do echo "FLAKY $k $t (red, then green alone)"; printf '%s\t%s\t%s\t%s\n' "$(date +%s)" "$rev" "$k" "$t" >>"$flaky"; done
}
rust_tests() {  # <dir>: cargo test of the workspace, each red test rerun once by name
  local d=$1 names
  (cd rust && cargo test --offline -q --workspace --no-fail-fast) >"$d/test.log" 2>&1 && { grep "test result" "$d/test.log"; return 0; }
  # the failed tests' names (cargo's "---- <name> stdout ----"); a doc test
  # ("src/x.rs - f (line 3)"), a crash or a build error has none: no rerun
  names="$(sed -n 's/^---- \(.*\) stdout ----$/\1/p' "$d/test.log" | sort -u)"
  if [ -z "$names" ] || grep -q ' - ' <<<"$names" || [ "$(wc -l <<<"$names")" -gt "$rerun_max" ]; then
    grep -E "^test .* FAILED|^failures:|panicked|test result" "$d/test.log" | head -40; return 1
  fi
  echo "cargo test: $(echo $names) failed: once more, alone"
  # shellcheck disable=SC2086
  if (cd rust && cargo test --offline -q --workspace --no-fail-fast --tests -- --exact $names) >"$d/rerun.log" 2>&1; then
    grep "test result" "$d/test.log" | grep -v FAILED; flaky_note cargo $names; return 0
  fi
  grep -E "^test .* FAILED|^failures:|panicked|test result: FAILED" "$d/rerun.log" | head -40; return 1
}
rust_pid=""
rust_wait() {  # the result of the cargo tests and clippy started next to the e2e/tmux tests
  [ -n "$rust_pid" ] || return 0
  wait "$rust_pid"; rust_pid=""
  cat "$rust_out/test.out" 2>/dev/null
  if [ "$(cat "$rust_out/test.rc" 2>/dev/null)" != 0 ]; then
    echo "FAILED: cargo test (full log: $rust_out/test.log)"; return 1
  fi
  if [ "$(cat "$rust_out/clippy.rc" 2>/dev/null)" != 0 ]; then
    head -60 "$rust_out/clippy.log"; echo "FAILED: clippy (full log: $rust_out/clippy.log)"; return 1
  fi
  rm -rf "$rust_out"
}
if [ -z "$only" ]; then
echo "== No blind sleep in tests/ (tests/sleep_check.py: wait for a state with tests/wait.py)"
python3 tests/sleep_check.py --self-test && python3 tests/sleep_check.py || exit 1
echo "== Waits count awake time, not the wall clock (tests/wait_test.py)"
python3 -B tests/wait_test.py || exit 1
echo "== The terminal and the desktop app release apart (tests/release_split_test.py)"
python3 tests/release_split_test.py || exit 1
echo "== One macOS signing core, the terminal release signed (tests/mac_sign_test.py)"
python3 -B tests/mac_sign_test.py || exit 1
if [ -f tests/cargo_mtimes_test.py ]; then
echo "== Fresh checkouts keep their unchanged crates (tests/cargo_mtimes_test.py)"
python3 -B tests/cargo_mtimes_test.py || exit 1
fi
echo "== The desktop release: signing order, feed, channels (tests/desktop_release_test.py)"
python3 -B tests/desktop_release_test.py || exit 1
echo "== publish-release.sh tags unsigned and says each step (tests/publish_release_test.py)"
python3 tests/publish_release_test.py || exit 1
echo "== The release's built-in plugins are exactly the shipped list (tests/release_plugins_test.py)"
python3 tests/release_plugins_test.py || exit 1
echo "== The nightly gate makes its tree in the heavy slot (tests/nightly_gate_test.py)"
python3 -B tests/nightly_gate_test.py || exit 1
echo "== The loop audit reads every gate log and timings.tsv (tests/loop_audit_test.py)"
python3 -B tests/loop_audit_test.py || exit 1
echo "== Rust: build, unit and scenario tests, clippy"
# the Bend binaries the e2e/tmux tests run (./repl-live, ./repl-scripted,
# ./sb-core; not in git): from bins.sh's cache, compiled on a miss
scripts/bins.sh repl-live repl-scripted sb-core || exit 1
# and the V8 engine of these sources in the cache, once, before the tests
# that run it ask for it in parallel (scripted_ts.jsrt_env: a fresh
# worktree has no rust/jsrt build; on a miss each would compile its own)
scripts/bins.sh path bend-jsrt >/dev/null || exit 1
(cd rust && cargo build --offline -q) || exit 1
# the test binaries compiled now, before the e2e/tmux tests start: cargo
# never relinks the bise they run while they run
(cd rust && cargo test --offline -q --workspace --no-run) || { echo "FAILED: cargo test (build)"; exit 1; }
# then the tests themselves and clippy run next to the e2e/tmux phase
# (they read nothing it writes; ~2-4 min of a mostly waiting phase), their
# result read at its end (rust_wait)
rust_out="$(mktemp -d -t sb-run-all-rust)"
( set +e
  rust_tests "$rust_out" >"$rust_out/test.out" 2>&1; echo $? >"$rust_out/test.rc"
  # in its own target dir, like the quick gate: gate.sh new's seed has it warm
  # (in the build's target dir clippy re-checked ~90 deps, BISE-244)
  clippy_dir="${CARGO_TARGET_DIR:-$PWD/rust/target}/clippy"
  (cd rust && cargo clippy --offline -q --workspace --all-targets --target-dir "$clippy_dir" -- -D warnings) >"$rust_out/clippy.log" 2>&1
  echo $? >"$rust_out/clippy.rc" ) &
rust_pid=$!
fi
TESTS="e2e subscriptions_e2e agents_md_e2e images_mem_e2e images_prune_e2e agent_tmp_e2e foundry_url_e2e approvals_e2e approvals_restart_e2e approvals_sandbox_e2e connector_confirm_e2e socket_auth_e2e agent_from_e2e one_key_auto_e2e edit_tools_e2e unknown_tool_e2e bise_demo_e2e PROOF worktree_home feature_e2e feature_retire_e2e land_every_e2e land_wait_e2e worktree_base_e2e ambient_core_e2e hub_outside_app_e2e empty_project_e2e version_skew_e2e rpc_e2e proto_e2e_agents proto_e2e_parts proto_e2e_threads bims_facts_e2e older_door_e2e older_tui_bundle_hub_tmux line_mode_e2e ambient_projects_e2e ambient_pages_e2e home_workspace_e2e projects_e2e xhub_e2e route_e2e file_chips_e2e ambient_setup_e2e every_e2e wake_deliver_e2e draft_guard_e2e proc_cleanup env_inherit_e2e idle_exit_e2e repl_recycle_e2e repl_start_e2e artifacts_e2e office_run_e2e office_artifacts_e2e long_socket_e2e utf8_say_e2e core_restart scripted_ts session_ev compaction_e2e repl_bash_env mcp_bootstrap prompt_size_e2e chatgpt_cache_key_e2e mcp_utf8 mcp_retry mcp_errors plugins_reload_e2e plugins_ts_e2e skills_reload_e2e skills_scan skills_hang_e2e provider_families deepseek_history home_migrate versions_prune bins_path repo_paths tui_tmux tui_help_tmux tui_ctrl_hints_tmux tui_composer_tmux tui_version_tmux tui_at_files_tmux tui_skills_reload_tmux tui_checker_tmux
tui_images_tmux tui_card_image_tmux tui_paste_tmux tui_clear_tmux tui_archived_tmux tui_waits_tmux tui_undelivered_tmux tui_queue_tmux tui_computer_use_tmux
tui_onboarding_tmux tui_onboarding_links_tmux subscriptions_tui_tmux tui_stuck_start_tmux tui_keys_tmux tui_subscriptions_tmux tui_provider_tmux tui_panel_click_tmux tui_drafts_tmux tui_reload_tmux tui_keep_state_tmux tui_links_tmux tui_tool_rows_tmux tui_tabs_tmux tui_select_popup_tmux tui_quote_keys_tmux tui_release_tmux tui_update_tmux tui_cards_tmux tui_answer_target_tmux tui_close_number_tmux tui_approvals_tmux tui_inbox_split_tmux tui_find_tmux tui_term_select_tmux tui_palette_tmux tui_file_links_tmux tui_file_chips_tmux tui_cmd_a_tmux tui_cmd_arrows_tmux tui_composer_scroll_tmux tui_turn_time_tmux tui_markdown_tmux tui_code_blocks_tmux tui_demo_tips_tmux tui_text_layer_tmux tui_tool_names_tmux tui_openai_responses_tmux tui_voice_tmux tui_voice_keys_tmux tui_voice_mute_tmux tui_you_fold_click_tmux tui_psi_hover_tmux tui_mcp_login_tmux tui_log_tmux interrupt_e2e partial_e2e tui_live_text_tmux
auth_events_e2e bims_growth_e2e hub_stall_e2e image_tag_e2e name_free_e2e notif_artifact_e2e pause_e2e plan_card_e2e request_size_e2e send_parts_e2e spawn_model_e2e wake_e2e desktop_app_test ambient_qa_world_test
tui_artifacts_tmux tui_artifacts_new_tmux tui_card_draft_keep_tmux tui_diff_quote_tmux tui_find_bar_tmux tui_inbox_tmux tui_iterm_tmux tui_msg_parts_tmux tui_notif_open_tmux tui_page_answer_tmux tui_page_link_tmux tui_panel_more_tmux tui_pause_tmux tui_plan_tmux tui_roles_tmux tui_scheduled_tmux tui_term_title_tmux tui_term_tmux tui_wake_tmux"
[ -n "$only" ] && TESTS="$only"
[ "$picked" = 1 ] && TESTS="$pick"
if [ -z "$(echo $TESTS)" ]; then echo "== no e2e/tmux test picked for this change"; rust_wait || exit 1; exit 0; fi
# PROOF: every bend/proofs/*.bend (tests/proof_run.py), cached by the content of the .bend files it reads
# and the bend version, for every worktree (a rerun of a red full gate, or
# another task on the same hub/ core/: ~2-3 min of one core, skipped)
proof_cache="${BISE_HOME:-$HOME/.bise}/gate/proof-cache"
proof_key="whole-$( { find bend/core bend/hub bend/vendor bend/runtime bend/laws bend/proofs -name '*.bend' -type f 2>/dev/null | sort | while read -r f; do echo "$f"; cat "$f"; done; } | shasum | cut -c1-16)-$(BEND_NO_TELEMETRY=1 bend version 2>/dev/null | tr -dc '0-9.')"
mkdir -p "$proof_cache" 2>/dev/null
echo "== E2E (real hub, REPLs, git; scripted provider), Bend laws (bend/proofs), the TUI under tmux ($jobs jobs)"
out="$(mktemp -d -t sb-run-all)"
one() {  # <test>: its last line; its whole output kept on failure
  local s=$SECONDS rc=0 cpu
  if [ "$1" = PROOF ] && [ -f "$proof_cache/$proof_key" ]; then
    echo "ok   PROOF (cached: these .bend files checked whole before)"; return 0
  elif [ "$1" = PROOF ]; then
    # 2 bend at once: PROOF is one job of the pool
    /usr/bin/time -p -o "$out/$1.time" python3 tests/proof_run.py . 2 "$out/proof" >"$out/$1.log" 2>&1 || rc=1
  else
    /usr/bin/time -p -o "$out/$1.time" python3 -u "tests/$1.py" >"$out/$1.log" 2>&1 || rc=$?
  fi
  cpu="$(awk '$1 == "user" || $1 == "sys" { c += $2 } END { printf "%d", c }' "$out/$1.time" 2>/dev/null)"
  local load; load="$(sysctl -n vm.loadavg 2>/dev/null | awk '{ print $2 }')"
  # one line, one write: the parallel jobs' appends never mix
  echo "$(date +%s)	$rev	$1	$rc	$((SECONDS - s))	${cpu:-0}	${load:-?}	${SB_RUN_KIND:-adhoc}" >>"$timings"
  [ $rc = 0 ] && [ "$1" = PROOF ] && touch "$proof_cache/$proof_key"
  [ $rc = 0 ] || echo "$1" >>"$out/failed"
  if [ $rc = 0 ]; then echo "ok   $1 ($((SECONDS - s))s, cpu ${cpu:-?}s): $(tail -1 "$out/$1.log")"
  else echo "FAIL $1 ($((SECONDS - s))s, cpu ${cpu:-?}s): $out/$1.log"; fi
  return $rc
}
export -f one; export out timings rev proof_cache proof_key
rc=0
# the longest first (median wall of their last 10 runs in $timings; a test
# with no history counts as long), so the jobs end together instead of a
# 4-min test starting last
TESTS="$(python3 tests/slow_first.py "$timings" $TESTS)"
printf '%s\n' $TESTS | xargs -P "$jobs" -I{} bash -c 'one {}' || rc=$?
# the pool's reds once more, alone, one by one (not with --serial: they ran alone already)
if [ $rc != 0 ] && [ "$jobs" -gt 1 ] && [ -s "$out/failed" ] && [ "$(wc -l <"$out/failed")" -le "$rerun_max" ]; then
  first="$(sort -u "$out/failed")"; mv "$out/failed" "$out/failed.pool"
  echo "== failed in the pool: $(echo $first), once more alone"
  for t in $first; do one "$t" || true; done
  if [ ! -s "$out/failed" ]; then rc=0; flaky_note e2e $first; fi
fi
if [ $rc != 0 ]; then
  for f in "$out"/*.log; do grep -q -E "Error|FAIL|Traceback|FALSE|fail" "$f" && { echo "---- $f"; tail -15 "$f"; }; done
  # gate.sh rerun runs these alone on the same tree (its Rust part green only)
  if rust_wait; then [ -n "${SB_RED_FILE:-}" ] && cp "$out/failed" "$SB_RED_FILE" 2>/dev/null; fi
  echo "FAILED: e2e/tmux (logs: $out)"
  exit 1
fi
rm -rf "$out"
rust_wait || exit 1
if [ $live = 1 ]; then
  echo "== live model"
  # his real keys and accounts, for this run he asked for (bise_env.clean_env drops them otherwise)
  BISE_TEST_REAL_ACCOUNTS=1 python3 -u tests/live_smoke.py
fi
