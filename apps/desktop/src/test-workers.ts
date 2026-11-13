// How many vitest workers a run takes (vitest.config.ts's maxWorkers). Under heavy-run, the job's
// share of the cpus, which heavy-run gives it at its start (HEAVY_CPUS: the cpus over the heavy
// jobs running, itself included), at least 2, at most 4. On 11-11 several gates at once ran 4
// workers each at load 45, and the workers' RPC to the runner missed its deadline ('Timeout
// calling onTaskUpdate': 3 reds of fast-loop's with every test green, and kb-desktop-ui's and
// apps-hub-files' the same day; architect m_60240). Outside heavy-run (pnpm test by hand, no
// HEAVY_CPUS): half the cpus, at most 4, as before.

const MOST = 4;
const LEAST = 2;

// heavyCpus: HEAVY_CPUS as heavy-run set it, or undefined outside heavy-run
export function vitestWorkers(cpus: number, heavyCpus: string | undefined): number {
  const share = heavyCpus !== undefined && /^\d+$/.test(heavyCpus) ? Number(heavyCpus) : null;
  if (share === null) return Math.max(1, Math.min(MOST, Math.floor(cpus / 2)));
  return Math.max(LEAST, Math.min(MOST, share));
}
