// gonelog.ts (pr-crash): a renderer or child process that died or hung is one app.log line; a
// clean exit is none; a page's query (a hub token) never goes in it.
import { describe, expect, it, vi } from 'vitest';

vi.mock('electron', () => ({ app: {}, crashReporter: {} }));
const { goneLine } = await import('./gonelog');

describe('a process gone or hung, in app.log', () => {
  it('a renderer gone says its reason, exit code and page, without its query', () => {
    expect(goneLine({ kind: 'renderer', url: 'file:///app/index.html?token=secret#/thread', details: { reason: 'oom', exitCode: 5 } }))
      .toBe('renderer gone: oom (exit 5) at file:///app/index.html');
  });
  it('a clean exit says nothing', () => {
    expect(goneLine({ kind: 'renderer', url: 'file:///a', details: { reason: 'clean-exit', exitCode: 0 } })).toBeNull();
    expect(goneLine({ kind: 'child', details: { type: 'GPU', reason: 'clean-exit', exitCode: 0 } })).toBeNull();
  });
  it('a child gone names it, a hang and its end are lines too', () => {
    expect(goneLine({ kind: 'child', details: { type: 'GPU', reason: 'crashed', exitCode: 11 } })).toBe('child gone: GPU crashed (exit 11)');
    expect(goneLine({ kind: 'unresponsive', url: 'file:///a?x=1' })).toBe('renderer unresponsive at file:///a');
    expect(goneLine({ kind: 'responsive', url: 'file:///a' })).toBe('renderer responsive again at file:///a');
  });
});
