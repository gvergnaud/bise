import { describe, expect, it } from 'vitest';
import type { HelperCmd } from '../../proto';
import { Looks, LOOK_TIMEOUT_MS } from './look';

function rig() {
  const sent: HelperCmd[] = [];
  const dropped: string[] = [];
  const timers: { ms: number; f: () => void; live: boolean }[] = [];
  const looks = new Looks({
    helper: (c) => sent.push(c),
    later: (ms, f) => {
      const t = { ms, f, live: true };
      timers.push(t);
      return () => {
        t.live = false;
      };
    },
    drop: (p) => dropped.push(p),
  });
  const fire = () => {
    for (const t of timers) if (t.live) t.f();
  };
  return { looks, sent, dropped, timers, fire };
}

describe("a file's Quick Look thumbnail, asked of the helper", () => {
  it('asks with its path, out and width, and takes the answer of its own id, with its length', async () => {
    const { looks, sent } = rig();
    const a = looks.look('/w/plan.xlsx', '/c/.ql-a.png', 480);
    const b = looks.look('/w/talk.mov', '/c/.ql-b.png', 480);
    expect(sent).toEqual([
      { cmd: 'thumb', id: 't1', path: '/w/plan.xlsx', out: '/c/.ql-a.png', px: 480 },
      { cmd: 'thumb', id: 't2', path: '/w/talk.mov', out: '/c/.ql-b.png', px: 480 },
    ]);
    looks.ev({ ev: 'thumb', id: 't2', ok: true, ms: 61500 });
    looks.ev({ ev: 'thumb', id: 't1' });
    await expect(b).resolves.toEqual({ ok: true, ms: 61500 });
    // no ok: Quick Look can't draw it (the card asks the file's app)
    await expect(a).resolves.toEqual({ ok: false });
  });

  it('gives up at 5 s, and deletes the PNG of an answer that comes after', async () => {
    const { looks, dropped, timers, fire } = rig();
    const a = looks.look('/w/big.key', '/c/.ql-a.png', 480);
    expect(timers.map((t) => t.ms)).toEqual([LOOK_TIMEOUT_MS]);
    fire();
    await expect(a).resolves.toBeNull();
    looks.ev({ ev: 'thumb', id: 't1', ok: true });
    expect(dropped).toEqual(['/c/.ql-a.png']);
    // a stranger's id and other events: nothing
    looks.ev({ ev: 'thumb', id: 't9', ok: true });
    looks.ev({ ev: 'quiet', on: false, why: '' });
    expect(dropped).toEqual(['/c/.ql-a.png']);
  });

  it('an answer in time stops its timer', async () => {
    const { looks, timers } = rig();
    const a = looks.look('/w/a.pdf', '/c/.ql-a.png', 480);
    looks.ev({ ev: 'thumb', id: 't1', ok: true });
    await expect(a).resolves.toEqual({ ok: true });
    expect(timers[0]?.live).toBe(false);
  });
});
