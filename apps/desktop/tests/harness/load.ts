// How much slower than an idle Mac this one is now: the 1-minute load over the cpus, from 1 to 4,
// read when asked. tests/wait.py's load_factor, the same rule, so a Python test and a desktop check
// stretch alike. A check's waits, its test timeout and its timing bars written for an idle Mac take
// it on a busy one (a release pass, an agent's build, him at work: 11.11's checks on a1c7dd6cd at
// load ~15, main m_64400): on an idle Mac the bar is exactly the one written, never looser.
import { cpus, loadavg } from 'node:os';

export function loadFactor(): number {
  return Math.max(1, Math.min(4, (loadavg()[0] ?? 0) / Math.max(1, cpus().length)));
}

/** ms written for an idle Mac, on this one now */
export function underLoad(ms: number): number {
  return Math.round(ms * loadFactor());
}
