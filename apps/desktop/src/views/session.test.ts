// One view's session against a fake view and a fake hub: the proxy's handshake, nothing before
// `initialized`, the tool's input then its result, its requests through the hub, its size, the
// teardown (docs/mcp-apps.md C7).
import { describe, expect, it } from 'vitest';
import { UI, type JsonObject } from './protocol';
import type { ViewAnswer, ViewCmd } from './port';
import { ViewSession, type Note, type Phase, type ViewResource } from './session';

const RES: ViewResource = {
  origin: 'http://v-0123456789abcdef.localhost:47123',
  proxy: 'http://v-0123456789abcdef.localhost:47123/mcp-app?s=0123456789abcdef0123456789abcdef',
  html: '<p>counter</p>',
  csp: { connectDomains: [] },
  permissions: ['microphone'],
  prefersBorder: true,
  toolInput: { start: 3 },
  toolResult: { content: [{ type: 'text', text: 'count 3' }], structuredContent: { count: 3 } },
  tool: { name: 'show_counter' },
};

const REF = { project: 'p', agent: 'a', call: '1730000000000-4' };

function rig(res: ViewResource = RES, answer: (c: ViewCmd) => ViewAnswer = () => ({ result: {} })) {
  const posted: JsonObject[] = [];
  const asked: ViewCmd[] = [];
  const phases: Phase[] = [];
  const notes: Note[] = [];
  const sizes: [number, number][] = [];
  const links: string[] = [];
  const saved: string[] = [];
  const modes: string[] = [];
  let messages = 0;
  // his fresh pointer-down in the view, as the host tells it (main's, once each)
  let gestures = 0;
  const s = new ViewSession(REF, res, {
    post: (m) => posted.push(m),
    ask: (c) => {
      asked.push(c);
      return Promise.resolve(answer(c));
    },
    context: () => ({ theme: 'dark', variables: {}, mode: 'inline', width: 600, height: 800, locale: 'en', timeZone: 'UTC', tool: res.tool }),
    openLink: (u) => links.push(u),
    download: (fs) => saved.push(...fs.map((f) => f.name)),
    gesture: () => {
      const had = gestures > 0;
      if (had) gestures -= 1;
      return Promise.resolve(had);
    },
    requestMode: (m) => {
      modes.push(m);
      return m;
    },
    onSize: (w, h) => sizes.push([w, h]),
    onPhase: (p) => phases.push(p),
    onNote: (n) => notes.push(n),
    onMessage: () => {
      messages += 1;
    },
  });
  const methods = () => posted.map((m) => m['method'] ?? `#${JSON.stringify(m['id'])}`);
  const req = (id: number, method: string, params: JsonObject = {}) => {
    s.receive({ jsonrpc: '2.0', id, method, params });
  };
  const note = (method: string, params: JsonObject = {}) => {
    s.receive({ jsonrpc: '2.0', method, params });
  };
  const flush = () => new Promise((r) => setTimeout(r, 0));
  const answerOf = (id: number) => posted.find((m) => m['id'] === id && m['method'] === undefined);
  const click = (n = 1) => {
    gestures += n;
  };
  return { s, posted, asked, phases, notes, sizes, links, saved, modes, methods, req, note, flush, answerOf, click, messages: () => messages };
}

/** the handshake up to `initialized` */
async function live(r: ReturnType<typeof rig>) {
  r.note(UI.proxyReady);
  r.req(1, UI.initialize, { protocolVersion: '2026-01-26', appInfo: { name: 'x', version: '1' }, appCapabilities: {} });
  await r.flush();
  r.note(UI.initialized);
}

describe('the order', () => {
  it('the proxy gets the page; nothing else before the view is initialized; then input, then result', async () => {
    const r = rig();
    r.note(UI.proxyReady);
    expect(r.posted[0]).toEqual({ jsonrpc: '2.0', method: UI.resourceReady, params: { html: '<p>counter</p>', csp: { connectDomains: [] }, permissions: { microphone: {} } } });
    r.req(1, UI.initialize, {});
    await r.flush();
    const init = r.answerOf(1)?.['result'] as JsonObject;
    expect(init['protocolVersion']).toBe('2026-01-26');
    expect((init['hostContext'] as JsonObject)['platform']).toBe('desktop');
    // initialized: the tool's input, then its result, nothing between
    expect(r.methods()).toEqual([UI.resourceReady, '#1']);
    r.note(UI.initialized);
    expect(r.methods()).toEqual([UI.resourceReady, '#1', UI.toolInput, UI.toolResult]);
    expect(r.posted[2]?.['params']).toEqual({ arguments: { start: 3 } });
    expect(r.posted[3]?.['params']).toEqual(RES.toolResult);
    expect(r.phases).toEqual(['init', 'ready', 'live']);
    // a second initialized sends nothing again
    r.note(UI.initialized);
    expect(r.posted).toHaveLength(4);
  });

  it('an initialized before initialize gives nothing', () => {
    const r = rig();
    r.note(UI.proxyReady);
    r.note(UI.initialized);
    expect(r.methods()).toEqual([UI.resourceReady]);
  });

  it("a view's request before ui/initialize is refused (ping answers)", async () => {
    const r = rig();
    r.note(UI.proxyReady);
    r.req(1, UI.toolsCall, { name: 'bump' });
    r.req(2, UI.ping);
    await r.flush();
    expect(r.asked).toEqual([]);
    expect(r.answerOf(1)?.['error']).toMatchObject({ code: -32602 });
    expect(r.answerOf(2)?.['result']).toEqual({});
  });

  it('no stored result: tool-cancelled; too large: cancelled and the card says so', async () => {
    const none = rig({ ...RES, toolResult: null });
    await live(none);
    expect(none.methods().slice(-2)).toEqual([UI.toolInput, UI.toolCancelled]);
    const big = rig({ ...RES, toolResult: { truncated: true } });
    await live(big);
    expect(big.methods().slice(-2)).toEqual([UI.toolInput, UI.toolCancelled]);
    expect(big.notes).toEqual(['too-large']);
  });
});

describe("the view's requests", () => {
  it('its tools and resources go to the hub as view_call, the answer back as is', async () => {
    const r = rig(RES, (c) => (c.cmd === 'view_call' && c.params['name'] === 'whisper' ? { error: { code: -32031, message: 'not visible', data: { reason: 'not-visible' } } } : { result: { structuredContent: { count: 4 } } }));
    await live(r);
    r.req(5, UI.toolsCall, { name: 'bump', arguments: { by: 1 } });
    r.req(6, UI.toolsCall, { name: 'whisper' });
    r.req(7, UI.resourcesRead, { uri: 'ui://fake-app/data' });
    await r.flush();
    expect(r.asked).toEqual([
      { cmd: 'view_call', ...REF, method: 'tools/call', params: { name: 'bump', arguments: { by: 1 } } },
      { cmd: 'view_call', ...REF, method: 'tools/call', params: { name: 'whisper' } },
      { cmd: 'view_call', ...REF, method: 'resources/read', params: { uri: 'ui://fake-app/data' } },
    ]);
    expect(r.answerOf(5)?.['result']).toEqual({ structuredContent: { count: 4 } });
    expect(r.answerOf(6)?.['error']).toEqual({ code: -32031, message: 'not visible', data: { reason: 'not-visible' } });
  });

  it("a tool call's view has no document: resources/url is not offered, never asked", async () => {
    const r = rig();
    await live(r);
    r.req(5, 'resources/url', { uri: 'ui://fake-app/data' });
    await r.flush();
    expect(r.answerOf(5)?.['error']).toMatchObject({ code: -32601 });
    expect(r.asked.some((c) => c.cmd === 'view_call')).toBe(false);
  });

  it('plugin off: the error to the view, and the card says its server is gone', async () => {
    const r = rig(RES, () => ({ error: { code: -32032, message: 'plugin off' } }));
    await live(r);
    r.req(5, UI.toolsCall, { name: 'bump' });
    await r.flush();
    expect(r.notes).toEqual(['gone']);
  });

  it('a message goes as view_message with its text blocks only, never a gesture of its own', async () => {
    const r = rig();
    await live(r);
    r.req(8, UI.message, { role: 'user', content: [{ type: 'text', text: 'make it major' }], gesture_ms: 1 });
    r.req(9, UI.message, { role: 'user', content: [{ type: 'image', data: 'x' }] });
    r.req(10, UI.updateContext, { content: [{ type: 'text', text: 'counter=4' }] });
    await r.flush();
    expect(r.asked).toEqual([
      { cmd: 'view_message', ...REF, content: [{ type: 'text', text: 'make it major' }] },
      { cmd: 'view_context', ...REF, content: [{ type: 'text', text: 'counter=4' }] },
    ]);
    expect(r.answerOf(9)?.['error']).toMatchObject({ code: -32602 });
    expect(r.messages()).toBe(1);
  });

  it('links (http only), display modes, sizes; an unknown method is not found', async () => {
    const r = rig();
    await live(r);
    r.click();
    r.req(11, UI.openLink, { url: 'https://example.com' });
    r.req(12, UI.openLink, { url: 'javascript:void(0)' });
    r.req(13, UI.displayMode, { mode: 'fullscreen' });
    r.req(14, 'sampling/createMessage', {});
    r.note(UI.sizeChanged, { width: 400, height: 233 });
    r.note(UI.sizeChanged, { width: 'x', height: 1 });
    await r.flush();
    expect(r.links).toEqual(['https://example.com']);
    expect(r.answerOf(12)?.['error']).toMatchObject({ code: -32602 });
    expect(r.answerOf(13)?.['result']).toEqual({ mode: 'fullscreen' });
    expect(r.modes).toEqual(['fullscreen']);
    expect(r.answerOf(14)?.['error']).toMatchObject({ code: -32601 });
    expect(r.sizes).toEqual([[400, 233]]);
  });

  it('a link or a download needs his fresh click in the view, one each: never in a loop (-32037 no-gesture)', async () => {
    const r = rig();
    await live(r);
    const file = { contents: [{ type: 'resource', resource: { uri: 'file:///a.txt', mimeType: 'text/plain', text: 'a' } }] };
    r.req(21, UI.openLink, { url: 'https://example.com/1' });
    r.req(22, UI.download, file);
    await r.flush();
    expect(r.links, 'no click: no link').toEqual([]);
    expect(r.saved, 'no click: no save').toEqual([]);
    expect(r.answerOf(21)?.['error']).toMatchObject({ code: -32037, data: { reason: 'no-gesture' } });
    expect(r.answerOf(22)?.['error']).toMatchObject({ code: -32037, data: { reason: 'no-gesture' } });
    r.click();
    for (let i = 0; i < 5; i += 1) r.req(30 + i, UI.openLink, { url: `https://example.com/${String(i)}` });
    await r.flush();
    expect(r.links, 'one click: one link').toEqual(['https://example.com/0']);
    r.click();
    r.req(40, UI.download, file);
    await r.flush();
    expect(r.saved).toEqual(['a.txt']);
    expect(r.answerOf(40)?.['result']).toEqual({});
  });
});

describe('context and teardown', () => {
  it('a context change reaches a live view only, with what changed', async () => {
    let theme: 'dark' | 'light' = 'dark';
    const r = rig();
    // the same rig, with a theme that moves
    const s = new ViewSession(REF, RES, {
      post: (m) => r.posted.push(m),
      ask: () => Promise.resolve({ result: {} }),
      context: () => ({ theme, variables: {}, mode: 'inline', width: 600, height: 800, locale: 'en', timeZone: 'UTC', tool: null }),
      openLink: () => undefined,
      download: () => undefined,
      gesture: () => Promise.resolve(false),
      requestMode: (m) => m,
      onSize: () => undefined,
      onPhase: () => undefined,
      onNote: () => undefined,
      onMessage: () => undefined,
    });
    s.contextChanged();
    expect(r.posted).toEqual([]);
    s.receive({ jsonrpc: '2.0', method: UI.proxyReady });
    s.receive({ jsonrpc: '2.0', id: 1, method: UI.initialize, params: {} });
    await r.flush();
    s.receive({ jsonrpc: '2.0', method: UI.initialized });
    theme = 'light';
    s.contextChanged();
    expect(r.posted.at(-1)).toEqual({ jsonrpc: '2.0', method: UI.contextChanged, params: { theme: 'light' } });
  });

  it('teardown asks the view, waits for its answer, then posts nothing more', async () => {
    const r = rig();
    await live(r);
    let done = false;
    void r.s.teardown().then(() => {
      done = true;
    });
    const t = r.posted.at(-1);
    expect(t?.['method']).toBe(UI.teardown);
    await r.flush();
    expect(done).toBe(false);
    r.s.receive({ jsonrpc: '2.0', id: t?.['id'] as string, result: {} });
    await r.flush();
    expect(done).toBe(true);
    expect(r.s.phase).toBe('torn');
    const n = r.posted.length;
    r.req(20, UI.ping);
    await r.flush();
    expect(r.posted).toHaveLength(n);
  });

  it('a view that never got its page is torn at once', async () => {
    const r = rig();
    await r.s.teardown();
    expect(r.posted).toEqual([]);
    expect(r.s.phase).toBe('torn');
  });
});
