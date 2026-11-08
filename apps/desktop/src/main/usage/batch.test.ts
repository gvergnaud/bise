import { describe, expect, it } from 'vitest';
import { app_cmd } from '../../proto/fixtures.gen';
import { GESTURES_MAX, isGesture, UsageBatch } from './batch';

describe('the usage batch', () => {
  it('sends what waits and keeps it until the core took it', () => {
    const b = new UsageBatch();
    expect(b.cmd()).toBeNull();
    b.add({ open_s: 60, focused_s: 42, engaged_s: 30 });
    b.gesture('inbox_opened');
    b.gesture('artifact_doc');
    b.gesture('crash_core');
    // the shape of rust/proto/fixtures/app_cmd.jsonl's usage_time line
    expect(b.cmd()).toEqual(app_cmd.find((c) => c.cmd === 'usage_time'));
    // not taken (the core was down): it waits, with what came since
    b.add({ open_s: 60, focused_s: 0, engaged_s: 0 });
    expect(b.cmd()?.open_s).toBe(120);
    b.sent();
    expect(b.cmd()).toBeNull();
  });

  it("drops a window's gesture that isn't one, and past the cap", () => {
    const b = new UsageBatch();
    for (const g of ['/Users/camille/notes.md', 'unknown', 42, null, { g: 'inbox_opened' }]) b.gesture(g);
    expect(b.cmd()).toBeNull();
    expect(isGesture('toString')).toBe(false);
    for (let i = 0; i < GESTURES_MAX + 10; i += 1) b.gesture('artifact_page');
    expect(b.cmd()?.gestures).toHaveLength(GESTURES_MAX);
  });
});
