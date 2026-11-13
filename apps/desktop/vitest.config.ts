// Unit and component tests: `pnpm test` runs every src/**/*.test.ts(x) next
// to its code. Node by default; a component test opts into the DOM with a
// first-line `// @vitest-environment happy-dom`. End-to-end checks are
// Playwright's (tests/, `pnpm test:e2e`), never here. The feature tests
// (tests/feature/*.feature.test.ts, fast-try p5) run here too: typed inputs
// in, a screen's view and the sent commands out, in milliseconds.
import { availableParallelism } from 'node:os';
import { resolve } from 'node:path';
import { configDefaults, defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { vitestWorkers } from './src/test-workers';

// every test: src/**/*.test.ts(x) next to its code, the feature tests, the checks' pure helpers
// (tests/checks/**/*.test.ts: composer/still.ts; the checks themselves are *.spec.ts)
const TESTS = ['src/**/*.test.ts', 'src/**/*.test.tsx', 'tests/feature/**/*.test.ts', 'tests/checks/**/*.test.ts'];
// the bims' world simulations: the slowest files of the suite (measured 2026-11-07, 4 workers at
// load 22: nap 209 s, clip 206 s, cozy 154 s, collide 145 s, shade 64 s, walklaw 46 s,
// pathlaw 44 s, workshop-view 27 s, standing 24 s, parkpath 22 s)
const BIMS_LAWS = [
  'src/bims/render/nap.test.ts',
  'src/bims/render/clip.test.ts',
  'src/bims/render/cozy.test.ts',
  'src/bims/render/collide.test.ts',
  'src/bims/render/shade.test.ts',
  'src/bims/walklaw.test.ts',
  'src/bims/render/pathlaw.test.ts',
  'src/bims/render/maps/workshop-view.test.ts',
  'src/bims/render/standing.test.ts',
  'src/bims/parkpath.test.ts',
];

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { '@': resolve(__dirname, 'src') } },
  test: {
    // tests/checks/**/*.test.ts: a check's pure helpers (composer/still.ts); the checks are *.spec.ts
    // (the projects' include, below: a root include would merge into each project's)
    environment: 'node',
    // an iframe's src is never fetched: happy-dom loaded every <iframe src="http://127.0.0.1:47123/p/…">
    // a component test renders (artifacts.test.tsx: 7 fetches a run, to nothing), and under load
    // the test ended first: the teardown's 'AsyncTaskManager has been destroyed' failed the run
    // with every test green (14 of 50 desktop_check reds in a day, the loop audit of 10-28)
    environmentOptions: { happyDOM: { settings: { navigation: { disableChildFrameNavigation: true } } } },
    passWithNoTests: true,
    // the window's keep starts empty for every test (src/test-setup.ts)
    setupFiles: ['src/test-setup.ts'],
    // at most 4 workers: 11 workers at load 25 starved each other's RPC to the runner ('Timeout
    // calling onTaskUpdate', -43's gate), and 4 runs as fast: the long bims files set the wall
    // time. Under heavy-run, the job's share it gives (HEAVY_CPUS), at least 2: 4 each at load 45
    // timed out again on 11-11 (src/test-workers.ts, architect m_60240, m_60250).
    maxWorkers: vitestWorkers(availableParallelism(), process.env.HEAVY_CPUS),
    // two projects (c3-quick-90s): bims-laws, the bims' world simulations (each file 20-210 s
    // at load 22, ~1000 s of the bims' 1090), and unit, the rest. `pnpm test` runs both (the
    // full gate, the tip, a release); the quick gate runs unit, and bims-laws only when a
    // src/bims file changed (tests/gate_plan.py's dbims). A new slow bims law goes in BIMS_LAWS.
    projects: [
      { extends: true, test: { name: 'unit', include: TESTS, exclude: [...configDefaults.exclude, ...BIMS_LAWS] } },
      { extends: true, test: { name: 'bims-laws', include: BIMS_LAWS } },
    ],
  },
});
