// The environment of every process main starts that runs bise: the core
// (`bise ambient-core`, core/process.ts) and the terminal panel's shells
// (`bise pty`, term/host.ts). The rule is bise_home::env::env_for's, from
// its generated table (env_vars.gen.ts, pinned by a Rust test against
// env::VARS): an internal name (SB_SOCKET, BISE_OWNERS, BEND_WIRE_LOG...)
// never passes, a test name passes only in test mode (the harness configures
// its core and shells through them), every other name passes. An app
// started from an agent's shell (dev, desktop_try) so hands none of that
// agent's plumbing to its children, and `sb` typed in his terminal never acts
// as that agent (architect's merge review B2). One carve-out, from the same
// table (Var::offscreen_core): an off-screen app's core also gets the voice
// fake's test knobs, so a packed app checked off-screen can dictate with the
// fake voice; the core's own rule (--offscreen + an isolated home) stays the
// guard, and the terminal shells never get them (architect m_22561).
//
// Pure: main passes process.env, its test mode, and whether this child is an
// off-screen core.

import { INTERNAL_VARS, OFFSCREEN_CORE_VARS, TEST_VARS } from './env_vars.gen';

const INTERNAL: ReadonlySet<string> = new Set(INTERNAL_VARS);
const TEST: ReadonlySet<string> = new Set(TEST_VARS);
const OFFSCREEN_CORE: ReadonlySet<string> = new Set(OFFSCREEN_CORE_VARS);

/**
 * Names that never go to a child, in test mode too: the launching
 * terminal's, the Bend runtime's per-command stdin fifo, Electron's (none of
 * them bise's, so not in env::VARS) and the app's own knobs (BISE_DESKTOP_*,
 * read by main only; in env::VARS as test names, so a launch of his app
 * scrubs them).
 */
function launchers(k: string): boolean {
  return k === 'TERM' || k === 'TERM_PROGRAM' || k === 'TERM_PROGRAM_VERSION' || k === 'BEND_STDIN' || k.startsWith('GHOSTTY_') || k.startsWith('ELECTRON_') || k.startsWith('BISE_DESKTOP_');
}

/**
 * Whether a child started in this mode inherits `k`. `offscreenCore`: the
 * child is the core of an off-screen run (core/process.ts), never a shell.
 */
export function inherits(k: string, testMode: boolean, offscreenCore = false): boolean {
  if (INTERNAL.has(k) || launchers(k)) return false;
  return testMode || !TEST.has(k) || (offscreenCore && OFFSCREEN_CORE.has(k));
}

/** A child's whole environment from main's (`spawn(.., { env })`). */
export function childEnv(base: Readonly<Record<string, string | undefined>>, testMode: boolean, offscreenCore = false): Record<string, string> {
  const env: Record<string, string> = {};
  for (const [k, v] of Object.entries(base)) {
    if (v !== undefined && inherits(k, testMode, offscreenCore)) env[k] = v;
  }
  return env;
}
