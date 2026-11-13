// childenv.ts: the core and `bise pty` get main's env minus bise's internals
// (always) and its test settings (unless test mode). The idea of the Rust
// env_inherit_e2e on the desktop's side: an app started from an agent's shell
// hands its children nothing of that agent.
import { describe, expect, it } from 'vitest';
import { childEnv, inherits } from './childenv';
import { INTERNAL_VARS, OFFSCREEN_CORE_VARS, TEST_VARS } from './env_vars.gen';

/** An agent's shell: every internal and test name set, plus his own settings. */
function agentShell(): Record<string, string> {
  const env: Record<string, string> = { HOME: '/h', PATH: '/usr/bin:/bin', LANG: 'fr_FR.UTF-8', MISTRAL_API_KEY: 'k', BISE_HOME: '/h/.bise', BISE_AGENT_MODEL: 'm', BEND_MODEL: 'small', BEND_RUNTIME_KNOB: '1' };
  for (const k of [...INTERNAL_VARS, ...TEST_VARS]) env[k] = 'junk';
  return { ...env, TERM: 'xterm-ghostty', TERM_PROGRAM: 'ghostty', GHOSTTY_RESOURCES_DIR: '/g', BEND_STDIN: '/bg/3.in', ELECTRON_RUN_AS_NODE: '1', BISE_DESKTOP_BISE: '/b' };
}

const HIS = { HOME: '/h', PATH: '/usr/bin:/bin', LANG: 'fr_FR.UTF-8', MISTRAL_API_KEY: 'k', BISE_HOME: '/h/.bise', BISE_AGENT_MODEL: 'm', BEND_MODEL: 'small', BEND_RUNTIME_KNOB: '1' };

describe('childEnv', () => {
  it('the table is the one bise has (generated from env::VARS)', () => {
    for (const k of ['SB_SOCKET', 'SB_AGENT', 'BISE_OWNERS', 'BEND_WIRE_LOG', 'BEND_REPL_PORT', 'BISE_APP_ROOT', 'BISE_ONESHOT', 'BISE_EXPORTS_FOR', 'BISE_MODELS_FILE', 'BISE_SKILL_ROOTS', 'BISE_ROLE']) {
      expect(INTERNAL_VARS as readonly string[]).toContain(k);
    }
    expect(TEST_VARS as readonly string[]).toContain('BISE_TEST_HOME');
  });

  it("outside test mode: his settings only, none of an agent's or a test run's", () => {
    expect(childEnv(agentShell(), false)).toEqual(HIS);
  });

  it('the off-screen knob (BISE_DESKTOP_OFFSCREEN, main only: offscreen.ts) never reaches the core, in either mode', () => {
    for (const test of [false, true]) {
      expect(childEnv({ ...agentShell(), BISE_DESKTOP_OFFSCREEN: '1' }, test)).not.toHaveProperty('BISE_DESKTOP_OFFSCREEN');
    }
  });

  it('in test mode: the test settings pass too (the harness configures its core and shells by env), never an internal one', () => {
    const env = childEnv(agentShell(), true);
    for (const k of INTERNAL_VARS) expect(env[k], k).toBeUndefined();
    // the app's own knobs (BISE_DESKTOP_*) are test names too (5083da9db), but main reads them and no child gets them
    for (const k of TEST_VARS) expect(env[k], k).toBe(k.startsWith('BISE_DESKTOP_') ? undefined : 'junk');
    expect(env).toMatchObject(HIS);
  });

  it('every internal name is dropped in both modes; a user setting under an old dropped prefix passes', () => {
    for (const k of INTERNAL_VARS) {
      expect(inherits(k, false), k).toBe(false);
      expect(inherits(k, true), k).toBe(false);
    }
    // the old prefix list dropped BISE_AGENT* and SB_*: his agents' model and his build dir are his
    expect(inherits('BISE_AGENT_MODEL', false)).toBe(true);
    expect(inherits('SB_BUILD_DIR', false)).toBe(true);
  });

  it("an off-screen core outside test mode gets the voice fake's knobs, and only those; a shell never does (architect m_22561)", () => {
    expect([...OFFSCREEN_CORE_VARS]).toEqual(['BISE_AMBIENT_FAKE_VOICE', 'BISE_VOICE_FAKE_HEARD', 'BISE_VOICE_FAKE_HEARD_MS']);
    for (const k of OFFSCREEN_CORE_VARS) expect(TEST_VARS as readonly string[], k).toContain(k);
    // the off-screen core (core/process.ts): his settings plus the three knobs, no other test or internal name
    const core = childEnv(agentShell(), false, true);
    expect(core).toEqual({ ...HIS, BISE_AMBIENT_FAKE_VOICE: 'junk', BISE_VOICE_FAKE_HEARD: 'junk', BISE_VOICE_FAKE_HEARD_MS: 'junk' });
    // the terminal shells (term/host.ts) never pass the flag: on- or off-screen, none of them
    for (const k of OFFSCREEN_CORE_VARS) expect(childEnv(agentShell(), false), k).not.toHaveProperty(k);
    // on-screen outside test mode: neither child gets them
    for (const k of OFFSCREEN_CORE_VARS) expect(inherits(k, false, false), k).toBe(false);
  });

  it('the terminal shells are started without the off-screen core flag (only core/process.ts sets it)', async () => {
    const { readFileSync } = await import('node:fs');
    const host = readFileSync(new URL('./term/host.ts', import.meta.url), 'utf8');
    expect(host).toMatch(/childEnv\(process\.env, this\.testMode\)/);
    const proc = readFileSync(new URL('./core/process.ts', import.meta.url), 'utf8');
    expect(proc).toMatch(/childEnv\([^)]*, this\.opts\.offscreen === true\)/);
  });

  it('an unset value is not written', () => {
    expect(childEnv({ HOME: '/h', GONE: undefined }, false)).toEqual({ HOME: '/h' });
  });
});
