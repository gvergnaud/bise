// Main's views requests: only a view command with its ids goes; view_message's gesture_ms is
// main's stamp, never the window's; the core's answer goes back to that call only.
import { describe, expect, it, vi } from 'vitest';

vi.mock('electron', () => ({ ipcMain: {} }));

const { isViewCmd, outgoing, setGestureStamp, viewAnswered, viewRefused } = await import('./views');

const ids = { project: 'p', agent: 'a', call: '1-1' };

describe('outgoing', () => {
  it('a view command with its ids, else null', () => {
    expect(outgoing({ cmd: 'view_resource', ...ids }, 1)).toEqual({ cmd: 'view_resource', ...ids });
    expect(outgoing({ cmd: 'view_resource', project: 'p', agent: 'a' }, 1)).toBeNull();
    expect(outgoing({ cmd: 'send', ...ids }, 1)).toBeNull();
    expect(outgoing(null, 1)).toBeNull();
    expect(isViewCmd({ cmd: 'view_message' })).toBe(true);
    expect(isViewCmd({ cmd: 'send' })).toBe(false);
  });

  it("the window's gesture_ms never goes; view_message's is the stamp's, by its window", () => {
    expect(outgoing({ cmd: 'view_call', ...ids, method: 'tools/call', params: {}, gesture_ms: 5 }, 1)).toEqual({ cmd: 'view_call', ...ids, method: 'tools/call', params: {} });
    expect(outgoing({ cmd: 'view_message', ...ids, content: [], gesture_ms: 5 }, 1)).toEqual({ cmd: 'view_message', ...ids, content: [] });
    setGestureStamp((cmd, win) => ({ ...cmd, gesture_ms: win * 1000 }));
    expect(outgoing({ cmd: 'view_message', ...ids, content: [], gesture_ms: 5 }, 7)).toEqual({ cmd: 'view_message', ...ids, content: [], gesture_ms: 7000 });
  });
});

describe('answers', () => {
  it('a command main did not send for a view is not taken', () => {
    expect(viewAnswered({ cmd: 'view_call' }, {})).toBe(false);
    expect(viewRefused({ cmd: 'view_call' }, { code: -1, message: 'x' })).toBe(false);
  });
});
