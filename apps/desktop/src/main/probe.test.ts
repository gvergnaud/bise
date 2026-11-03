import { createServer, type Server } from 'node:http';
import { afterEach, describe, expect, it } from 'vitest';
import { httpProbe, probePage, probePort } from './probe';

let open: Server | null = null;
afterEach(() => {
  open?.close();
  open = null;
});

/** A pages server on a free port (it answers 404 under /gone/, else 200), and that port. */
function listen(): Promise<number> {
  return new Promise((resolve) => {
    const s = createServer((q, r) => {
      r.statusCode = q.url?.startsWith('/gone/') === true ? 404 : 200;
      r.end(r.statusCode === 404 ? 'no such file: a/x/1/x.md' : 'ok');
    });
    open = s;
    s.listen(0, '127.0.0.1', () => {
      const a = s.address();
      resolve(typeof a === 'object' && a !== null ? a.port : 0);
    });
  });
}

/** A port nothing listens on: one that was free a moment ago. */
async function deadPort(): Promise<number> {
  const p = await listen();
  await new Promise<void>((r) => open?.close(() => { r(); }));
  open = null;
  return p;
}

describe('probePort (only the local pages server is asked)', () => {
  it("a pages-server url's port", () => {
    expect(probePort('http://127.0.0.1:47415/p/blokhus-own-deployment')).toBe(47415);
    expect(probePort('http://127.0.0.1:47415/a/x/view?theme=dark#top')).toBe(47415);
  });
  it('anything else: none', () => {
    expect(probePort('https://127.0.0.1:47415/p/x')).toBeNull();
    expect(probePort('http://localhost:47415/p/x')).toBeNull();
    expect(probePort('https://github.com/o/r/pull/1')).toBeNull();
    expect(probePort(42)).toBeNull();
  });
});

describe('probePage (page-unreach: Benjamin\'s refused page)', () => {
  it('a server that answers: up', async () => {
    const port = await listen();
    expect(await probePage(`http://127.0.0.1:${String(port)}/p/x`)).toBe('up');
  });
  it("its file gone (the server's 404): gone, never the raw line in the frame (designer m_52746)", async () => {
    const port = await listen();
    expect(await probePage(`http://127.0.0.1:${String(port)}/gone/view`)).toBe('gone');
  });
  it('nothing listening (its hub stopped): down, at once', async () => {
    const port = await deadPort();
    const t0 = Date.now();
    expect(await httpProbe(`http://127.0.0.1:${String(port)}/p/x`)).toBe('down');
    expect(Date.now() - t0).toBeLessThan(700);
  });
  it('not a pages url: never probed, up', async () => {
    let asked = false;
    const r = await probePage('https://example.com', () => {
      asked = true;
      return Promise.resolve('down');
    });
    expect([r, asked]).toEqual(['up', false]);
  });
});
