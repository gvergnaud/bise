#!/usr/bin/env bash
# Every switchboard check, from the repo root:
#   tests/run_all.sh            # deterministic (no model)
#   tests/run_all.sh --serial   # e2e + tmux tests one by one
#   tests/run_all.sh --live     # + a real-model smoke test
# e2e.py and the tmux tests run in parallel (SB_TEST_JOBS, default 4: each
# has its own tmux session and throwaway hub; more jobs overload the machine
# and the timing-sensitive tests flake). FUZZ_RUNS=2000 for the long fuzz run.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.bend/bin:$HOME/.cargo/bin:$PATH"
live=0 jobs="${SB_TEST_JOBS:-4}"
for a in "$@"; do
  case "$a" in
    --live) live=1 ;;
    --serial) jobs=1 ;;
    *) echo "usage: run_all.sh [--serial] [--live]" >&2; exit 2 ;;
  esac
done
echo "== Rust: build, unit and scenario tests, clippy"
# an agent's shell points SB_CORE_BIN at its hub's (older) sb-core: the
# core tests must spawn this tree's
unset SB_CORE_BIN
# the Bend binaries the e2e/tmux tests run (./repl-live, ./repl-scripted,
# ./sb-core; not in git): from bins.sh's cache, compiled on a miss
scripts/bins.sh repl-live repl-scripted sb-core || exit 1
log="$(mktemp -t sb-run-all)"
(cd rust && cargo build --offline -q) || exit 1
# every test binary's summary; a failure stops here with cargo's report
if ! (cd rust && cargo test --offline -q --workspace) >"$log" 2>&1; then
  grep -E "^test .* FAILED|^failures:|panicked|test result" "$log" | head -40
  echo "FAILED: cargo test (full log: $log)"
  exit 1
fi
grep "test result" "$log"
rm -f "$log"
# in its own target dir, like the quick gate: gate.sh new's seed has it warm
# (in the build's target dir clippy re-checked ~90 deps, BISE-244)
clippy_dir="${CARGO_TARGET_DIR:-$PWD/rust/target}/clippy"
(cd rust && cargo clippy --offline -q --workspace --all-targets --target-dir "$clippy_dir" -- -D warnings)
TESTS="e2e agents_md_e2e images_mem_e2e agent_tmp_e2e foundry_url_e2e approvals_e2e approvals_restart_e2e approvals_sandbox_e2e one_key_auto_e2e edit_tools_e2e bise_demo_e2e PROOF worktree_home feature_e2e proc_cleanup core_restart scripted_ts session_ev compaction_e2e repl_bash_env mcp_bootstrap mcp_utf8 plugins_reload_e2e skills_reload_e2e skills_scan provider_families home_migrate versions_prune bins_path repo_paths tui_tmux tui_help_tmux tui_ctrl_hints_tmux tui_composer_tmux tui_version_tmux tui_at_files_tmux tui_skills_reload_tmux tui_checker_tmux
tui_images_tmux tui_paste_tmux tui_clear_tmux tui_archived_tmux tui_waits_tmux tui_undelivered_tmux tui_queue_tmux
tui_onboarding_tmux tui_onboarding_links_tmux tui_stuck_start_tmux tui_keys_tmux tui_provider_tmux tui_checker_tmux tui_panel_click_tmux tui_drafts_tmux tui_reload_tmux tui_links_tmux tui_tool_rows_tmux tui_tabs_tmux tui_select_popup_tmux tui_release_tmux tui_cards_tmux tui_approvals_tmux tui_inbox_split_tmux tui_find_tmux tui_term_select_tmux tui_palette_tmux tui_file_links_tmux tui_cmd_a_tmux tui_cmd_arrows_tmux tui_composer_scroll_tmux tui_turn_time_tmux tui_markdown_tmux tui_code_blocks_tmux tui_demo_tips_tmux tui_text_layer_tmux tui_tool_names_tmux tui_openai_responses_tmux tui_voice_tmux tui_voice_keys_tmux tui_voice_mute_tmux tui_you_fold_click_tmux tui_mcp_login_tmux"
# alone, after the others: under parallel load its Ctrl+U sometimes leaves
# the composer text (failed 2 runs out of 5 in parallel, 0 alone)
ALONE="tui_term_tmux"
echo "== E2E (real hub, REPLs, git; scripted provider), Bend laws (PROOF.bend), the TUI under tmux ($jobs jobs)"
out="$(mktemp -d -t sb-run-all)"
one() {  # <test>: its last line; its whole output kept on failure
  local s=$SECONDS rc=0
  if [ "$1" = PROOF ]; then
    bend bend/PROOF.bend >"$out/$1.log" 2>&1 && grep -q "ALL PROOFS CHECK" "$out/$1.log" || rc=1
  else
    python3 -u "tests/$1.py" >"$out/$1.log" 2>&1 || rc=$?
  fi
  if [ $rc = 0 ]; then echo "ok   $1 ($((SECONDS - s))s): $(tail -1 "$out/$1.log")"
  else echo "FAIL $1 ($((SECONDS - s))s): $out/$1.log"; fi
  return $rc
}
export -f one; export out
rc=0
# e2e and PROOF first: the longest
printf '%s\n' $TESTS | xargs -P "$jobs" -I{} bash -c 'one {}' || rc=$?
for t in $ALONE; do one "$t" || rc=1; done
if [ $rc != 0 ]; then
  for f in "$out"/*.log; do grep -q -E "Error|FAIL|Traceback|FALSE|fail" "$f" && { echo "---- $f"; tail -15 "$f"; }; done
  echo "FAILED: e2e/tmux (logs: $out)"
  exit 1
fi
rm -rf "$out"
if [ $live = 1 ]; then
  echo "== live model"
  python3 -u tests/live_smoke.py
fi
