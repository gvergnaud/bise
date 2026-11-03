// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { FRAME_KINDS } from '../shared/frame';
import { FIND_AGENT } from './findagent';

type Listener = (e: { data: unknown; source: unknown }) => unknown;

/** The injected script, run as a frame runs it on this document: its parent and its listeners given. */
// eslint-disable-next-line @typescript-eslint/no-implied-eval -- the script guard.ts injects, run as the frame would
const run = new Function('window', 'parent', 'addEventListener', 'removeEventListener', FIND_AGENT) as (...globals: unknown[]) => void;

function frame(html: string) {
  document.body.innerHTML = html;
  const on: Listener[] = [];
  const keys: ((e: Record<string, unknown>) => void)[] = [];
  const posted: unknown[] = [];
  const parent = { postMessage: (m: unknown) => posted.push(m) };
  run({}, parent, (t: string, f: Listener) => {
    if (t === 'message') on.push(f);
    if (t === 'keydown') keys.push(f as never);
  }, () => undefined);
  const find = async (q: string, at = -1, seq = 1, open = true) => {
    await Promise.all(on.map((f) => f({ data: { bise: FRAME_KINDS.find, q, at, seq, open }, source: parent })));
    return posted.at(-1);
  };
  /** an esc in the page: whether the agent took it (ESC_FORWARD then never sends it to close the artifact) */
  const esc = (): boolean => {
    let taken = false;
    const e = { key: 'Escape', repeat: false, isComposing: false, defaultPrevented: false, preventDefault: () => { taken = true; } };
    for (const f of keys) f(e);
    return taken;
  };
  return { find, esc, posted, on, parent };
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('the find agent in an artifact frame (FIND_AGENT)', () => {
  it('counts the needle case-insensitive, across text nodes, never in a script or a style', async () => {
    const f = frame('<p>Kiwi and <b>ki</b>wi</p><pre>a kiwi\nKIWI</pre><script>var kiwi</script><style>.kiwi{}</style>');
    expect(await f.find('kiwi', 0, 7)).toEqual({ bise: FRAME_KINDS.found, seq: 7, n: 4, head: 0 });
    expect(await f.find('nope', -1, 8)).toEqual({ bise: FRAME_KINDS.found, seq: 8, n: 0, head: 0 });
    expect(await f.find('', -1, 9)).toEqual({ bise: FRAME_KINDS.found, seq: 9, n: 0, head: 0 });
  });

  it("while the window's bar shows, an esc in the page is the bar's: taken, said up; closed, the page keeps it (ui-review m_52737)", async () => {
    const f = frame('<p>kiwi</p>');
    expect(f.esc()).toBe(false);
    await f.find('kiwi', 0, 1, true);
    expect(f.esc()).toBe(true);
    expect(f.posted.at(-1)).toEqual({ bise: FRAME_KINDS.found, esc: true });
    // once said, the next esc is the page's again (the bar is closing)
    expect(f.esc()).toBe(false);
    await f.find('', -1, 2, false);
    expect(f.esc()).toBe(false);
  });

  it('hears its parent only', async () => {
    const f = frame('<p>kiwi</p>');
    await Promise.all(f.on.map((l) => l({ data: { bise: FRAME_KINDS.find, q: 'kiwi', at: 0, seq: 1 }, source: {} })));
    expect(f.posted).toEqual([]);
  });
});
