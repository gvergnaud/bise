// The harness's one entry for checks: `import { test, expect, ... } from '../../harness'`.
// test.extend gives each test fixtures that launch out of his sight with the proof step first and
// prove again at the end (a rule broken during the test fails it):
//   desktop     the app (apps/desktop/out) in its test mode
// Each is a Session (launch.ts): { app, page, cdp, proof, watch }. Recorder (frames.ts) records
// what renders; typing.ts, caret.ts, text.ts, perf.ts, inputs.ts measure and act.
import { test as base } from '@playwright/test';
import { launchDesktop, type Session } from './launch';

export { expect } from '@playwright/test';
export * from './launch';
export * from './frames';
export * from './typing';
export * from './caret';
export * from './text';
export * from './perf';
export * from './inputs';
export * from './load';
export { FakeCore, FakeHelper } from './fakes/helper';
export { Vm, freeBytes, FLOOR_BYTES, type VmOptions, type ExecResult } from './vm/vm';
export { Rfb, parseCombo } from './vm/rfb';
export { prove, violationsIn, type Proof, type Sample } from './proof';

const session = (open: () => Promise<Session>) =>
  // eslint-disable-next-line no-empty-pattern -- Playwright fixtures take their deps as a destructured first argument
  async ({}, use: (s: Session) => Promise<void>): Promise<void> => {
    const s = await open();
    try {
      await use(s);
    } finally {
      await s.close();
    }
  };

export const test = base.extend<{ desktop: Session }>({
  desktop: session(() => launchDesktop()),
});
