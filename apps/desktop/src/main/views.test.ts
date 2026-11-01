// Main's views requests: only a view command with its ids goes; view_message's gesture_ms is
// main's stamp, never the window's; the core's answer goes back to that call only.
import { describe, expect, it, vi } from 'vitest';

vi.mock('electron', () => ({ ipcMain: {} }));

const views = await import('./views');
const { isViewCmd, viewAnswered, viewRefused } = views;

const ids = { project: 'p', agent: 'a', call: '1-1' };
// main's stamp (viewframes.stampViewGesture), here by its window id
const stamp = (cmd: Record<string, unknown>, win: number): Record<string, unknown> => ({ ...cmd, gesture_ms: win * 1000 });
const outgoing = (raw: unknown, win: number): Record<string, unknown> | null => views.outgoing(raw, win, stamp);

describe('outgoing', () => {
  it('a view command with its ids, else null', () => {
    expect(outgoing({ cmd: 'view_resource', ...ids }, 1)).toEqual({ cmd: 'view_resource', ...ids });
    expect(outgoing({ cmd: 'view_resource', project: 'p', agent: 'a' }, 1)).toBeNull();
    expect(outgoing({ cmd: 'send', ...ids }, 1)).toBeNull();
    expect(outgoing(null, 1)).toBeNull();
    expect(outgoing({ cmd: 'plugin_views', project: 'p', plugin: 'piano' }, 1)).toEqual({ cmd: 'plugin_views', project: 'p', plugin: 'piano' });
    expect(outgoing({ cmd: 'plugin_views', project: 'p' }, 1)).toBeNull();
    expect(isViewCmd({ cmd: 'view_message' })).toBe(true);
    expect(isViewCmd({ cmd: 'send' })).toBe(false);
  });

  it("the window's gesture_ms never goes; view_message's is the stamp's, by its window", () => {
    expect(outgoing({ cmd: 'view_call', ...ids, method: 'tools/call', params: {}, gesture_ms: 5 }, 1)).toEqual({ cmd: 'view_call', ...ids, method: 'tools/call', params: {} });
    expect(outgoing({ cmd: 'view_message', ...ids, content: [], gesture_ms: 5 }, 7)).toEqual({ cmd: 'view_message', ...ids, content: [], gesture_ms: 7000 });
    // the stamp sees the command without the window's gesture_ms: a stamp with no pointer-down adds none
    expect(views.outgoing({ cmd: 'view_message', ...ids, content: [], gesture_ms: 5 }, 7, (c) => c)).toEqual({ cmd: 'view_message', ...ids, content: [] });
  });

  it('the stamp is an argument, never a module global set by whoever installs first (review: install order lost every gesture)', () => {
    expect(Object.keys(views)).not.toContain('setGestureStamp');
    expect(views.installViewCalls.length, 'installViewCalls(send, stamp)').toBe(2);
  });
});

describe('answers', () => {
  it('a command main did not send for a view is not taken', () => {
    expect(viewAnswered({ cmd: 'view_call' }, {})).toBe(false);
    expect(viewRefused({ cmd: 'view_call' }, { code: -1, message: 'x' })).toBe(false);
  });
});

describe('the bound of a held call (review d3)', () => {
  it('a call the core never answers gets -32603 at VIEW_CALL_MAX_MS, once; an answer before clears it', async () => {
    const { VIEW_CALL_MAX_MS } = await import('../proto/view_rules.gen');
    vi.useFakeTimers();
    try {
      const got: unknown[] = [];
      const late = { cmd: 'view_call' };
      views.hold(late, 1, (a) => got.push(a));
      vi.advanceTimersByTime(VIEW_CALL_MAX_MS - 1);
      expect(got).toEqual([]);
      vi.advanceTimersByTime(1);
      expect(got).toEqual([views.TOO_LATE]);
      expect(viewAnswered(late, {}), 'answered late: nobody holds it any more').toBe(false);
      const fast = { cmd: 'view_call' };
      views.hold(fast, 1, (a) => got.push(a));
      expect(viewAnswered(fast, { ok: 1 })).toBe(true);
      vi.advanceTimersByTime(VIEW_CALL_MAX_MS);
      expect(got).toEqual([views.TOO_LATE, { result: { ok: 1 } }]);
    } finally {
      vi.useRealTimers();
    }
  });

  it("a window gone drops its calls only (nobody to answer), their timers with them", () => {
    vi.useFakeTimers();
    try {
      const got: string[] = [];
      const [a, b] = [{ cmd: 'view_call' }, { cmd: 'view_call' }];
      views.hold(a, 1, () => got.push('a'));
      views.hold(b, 2, () => got.push('b'));
      views.viewsWindowGone(1);
      expect(viewAnswered(a, {})).toBe(false);
      expect(viewAnswered(b, {})).toBe(true);
      vi.runAllTimers();
      expect(got).toEqual(['b']);
    } finally {
      vi.useRealTimers();
    }
  });
});
