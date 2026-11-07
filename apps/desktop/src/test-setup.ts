// Every unit test starts with an empty keep (src/app/keep.ts): the window's keep is one per
// renderer, so without this a panel one test opened (or a field it typed in) would open the next
// test's too. Owner: amb-win.
// And no test keeps the event loop busy more than 20 s without a macrotask (src/test-busy.ts,
// gate-reds): one that does fails by its name instead of a whole run's 'Timeout calling
// onTaskUpdate' on a loaded machine.
// Test isolation (vitest's default, one module graph per file) is load-bearing: with
// --no-isolate, 33 of the 110 files that import thread/Code.tsx went red (module-level stores
// with no reset here) and the run was slower, 78 s against 48 s (c3-quick-90s, 2026-11-07).
// A store that needs a reset gets it here before isolation can go.
import { afterEach, beforeEach } from 'vitest';
import { keep } from '@/app/keep';
import { busyFault, busyWatch } from '@/test-busy';

const busy = busyWatch();

beforeEach(() => {
  busy.reset();
});

afterEach((ctx) => {
  keep.clear();
  const f = busyFault(ctx.task.name, busy.worst());
  if (f !== null) throw new Error(f);
});
