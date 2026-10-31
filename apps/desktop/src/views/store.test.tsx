// @vitest-environment happy-dom
// The window's views against a fake hub and a fake view posting real window messages: a message
// counts only from that view's proxy window at that view's origin; the resource's states; the
// layer's model in the DOM (the frame over its card, hidden when the card unmounts, the same
// frame in the panel tab when fullscreen, past MAX_LIVE the oldest torn down).
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { step, ViewsLayer } from './Layer';
import { ViewCard } from './Card';
import { ViewTab } from './Tab';
import { MAX_LIVE, type Place } from './place';
import { UI, type Json, type JsonObject } from './protocol';
import { viewKey, type ViewAnswer, type ViewCmd, type ViewRef } from './port';
import { resourceOf, ViewsStore, type ViewsEnv } from './store';

const ORIGIN = 'http://v-0123456789abcdef.localhost:47123';

const SESSION = '0123456789abcdef0123456789abcdef';

function resource(origin = ORIGIN): Json {
  return { origin, proxy: `${origin}/mcp-app?s=${SESSION}`, html: '<p>v</p>', csp: {}, permissions: ['midi'], declared: ['midi', 'camera'], prefersBorder: true, toolInput: { a: 1 }, toolResult: { structuredContent: { count: 1 } }, tool: { name: 'show_counter' } };
}

function rig(answer: (c: ViewCmd) => ViewAnswer = () => ({ result: resource() })) {
  const tabs = new Set<string>();
  const timers: (() => void)[] = [];
  const asked: ViewCmd[] = [];
  let now = 1000;
  const env: ViewsEnv = {
    ask: (c) => {
      asked.push(c);
      return Promise.resolve(answer(c));
    },
    base: () => ({ theme: 'dark', variables: {}, locale: 'en', timeZone: 'UTC' }),
    openLink: () => undefined,
    download: () => undefined,
    gesture: () => Promise.resolve(false),
    panel: {
      has: (r) => tabs.has(viewKey(r)),
      open: (r) => tabs.add(viewKey(r)),
      close: (r) => tabs.delete(viewKey(r)),
    },
    watch: () => () => undefined,
    now: () => (now += 1),
    later: (_ms, f) => timers.push(f),
  };
  const store = new ViewsStore(env);
  return { store, tabs, timers, asked };
}

const flush = () => act(() => new Promise<void>((r) => setTimeout(r, 0)));

/** A fake proxy window: what the host posts to it, and its messages to the host as real window events. */
function fakeProxy() {
  const got: JsonObject[] = [];
  const win = {
    postMessage: (m: JsonObject, origin: string) => {
      got.push({ ...m, _to: origin });
    },
  };
  const send = (data: JsonObject, origin = ORIGIN, source: unknown = win) => {
    window.dispatchEvent(new MessageEvent('message', { data, origin, source: source as Window }));
  };
  return { win, got, send };
}

function frameFor(store: ViewsStore, r: ViewRef, win: unknown) {
  store.frame(viewKey(r), { contentWindow: win } as HTMLIFrameElement);
}

const REF: ViewRef = { project: 'p', agent: 'a', call: '1-1' };

afterEach(() => {
  cleanup();
});

describe('messages', () => {
  it('a view is answered only from its proxy window at its origin', async () => {
    const { store } = rig();
    const stop = store.listen(window);
    store.ensure(REF);
    await flush();
    const proxy = fakeProxy();
    frameFor(store, REF, proxy.win);
    // another window, the right origin: dropped
    proxy.send({ jsonrpc: '2.0', method: UI.proxyReady }, ORIGIN, {});
    // the right window, a wrong origin: dropped
    proxy.send({ jsonrpc: '2.0', method: UI.proxyReady }, 'http://v-ffffffffffffffff.localhost:47123');
    proxy.send({ jsonrpc: '2.0', method: UI.proxyReady }, 'null');
    expect(proxy.got).toEqual([]);
    // its proxy, its origin: the page goes to that origin only
    proxy.send({ jsonrpc: '2.0', method: UI.proxyReady });
    expect(proxy.got).toHaveLength(1);
    expect(proxy.got[0]?.['method']).toBe(UI.resourceReady);
    expect(proxy.got[0]?.['_to']).toBe(ORIGIN);
    // the handshake, then input and result in that order
    proxy.send({ jsonrpc: '2.0', id: 1, method: UI.initialize, params: {} });
    await flush();
    proxy.send({ jsonrpc: '2.0', method: UI.initialized });
    expect(proxy.got.map((m) => m['method'] ?? 'answer')).toEqual([UI.resourceReady, 'answer', UI.toolInput, UI.toolResult]);
    stop();
  });

  it("a size the view says is its card's height, within bounds", async () => {
    const { store } = rig();
    const stop = store.listen(window);
    store.ensure(REF);
    await flush();
    const proxy = fakeProxy();
    frameFor(store, REF, proxy.win);
    proxy.send({ jsonrpc: '2.0', method: UI.sizeChanged, params: { width: 300, height: 233.2 } });
    expect(store.state(REF)).toMatchObject({ kind: 'live', height: 234 });
    proxy.send({ jsonrpc: '2.0', method: UI.sizeChanged, params: { width: 300, height: 99999 } });
    expect(store.state(REF)).toMatchObject({ height: 800 });
    stop();
  });
});

describe('a view\'s states', () => {
  it('asked of the hub once; loading, then live with what it was not granted', async () => {
    const { store, asked } = rig();
    store.ensure(REF);
    store.ensure(REF);
    expect(store.state(REF)).toEqual({ kind: 'loading', height: 96 });
    await flush();
    expect(asked).toEqual([{ cmd: 'view_resource', ...REF }]);
    expect(store.state(REF)).toMatchObject({ kind: 'live', phase: 'proxy', mode: 'inline', ungranted: ['camera'], tool: 'show_counter' });
    expect(store.live()).toEqual([viewKey(REF)]);
  });

  it('expired (-32030) and failed; failed tries again on his click', async () => {
    let n = 0;
    const { store } = rig(() => (n++ === 0 ? { error: { code: -32030, message: 'expired' } } : { error: { code: -32032, message: 'plugin off' } }));
    store.ensure(REF);
    await flush();
    expect(store.state(REF)).toEqual({ kind: 'expired' });
    const other = { ...REF, call: '1-2' };
    store.ensure(other);
    await flush();
    expect(store.state(other)).toEqual({ kind: 'failed', message: 'plugin off' });
    store.retry(other);
    expect(store.state(other)).toEqual({ kind: 'loading', height: 96 });
  });

  it("a teardown's end never wipes the new session its card started meanwhile (else 'loading' for good)", async () => {
    const { store, timers, asked } = rig();
    const stop = store.listen(window);
    store.ensure(REF);
    await flush();
    const proxy = fakeProxy();
    frameFor(store, REF, proxy.win);
    // the page went to the proxy: its teardown waits for the view's answer, bounded by TEARDOWN_MS
    proxy.send({ jsonrpc: '2.0', method: UI.proxyReady });
    void store.teardown(viewKey(REF));
    // the card shows again before the bound: a new session from the hub
    store.ensure(REF);
    await flush();
    expect(asked.filter((c) => c.cmd === 'view_resource')).toHaveLength(2);
    expect(store.state(REF).kind).toBe('live');
    // the old teardown's bound runs now
    for (const t of timers) t();
    expect(store.state(REF).kind, 'the new session stays live').toBe('live');
    expect(store.live()).toEqual([viewKey(REF)]);
    expect(store.liveOf(viewKey(REF))).toBeDefined();
    stop();
  });

  it("the frame loads the hub's proxy URL (its session): on the view's origin only, else not a view", () => {
    const r = resource() as JsonObject;
    expect(resourceOf(r)?.proxy).toBe(`${ORIGIN}/mcp-app?s=${SESSION}`);
    expect(resourceOf({ ...r, proxy: undefined } as unknown as Json)?.proxy, 'an older hub: its plain proxy').toBe(`${ORIGIN}/mcp-app`);
    for (const bad of ['http://v-ffffffffffffffff.localhost:47123/mcp-app?s=1', `${ORIGIN}/other?s=1`, `${ORIGIN}.evil.com/mcp-app`, 'javascript:alert(1)', 7]) {
      expect(resourceOf({ ...r, proxy: bad }), String(bad)).toBe(null);
    }
  });

  it('a result that is not a view is a failure', async () => {
    const { store } = rig(() => ({ result: { html: 1 } }));
    store.ensure(REF);
    await flush();
    expect(store.state(REF).kind).toBe('failed');
  });
});

describe('the layer', () => {
  function box(el: Element, r: { x: number; y: number; w: number; h: number }) {
    (el as HTMLElement).getBoundingClientRect = () => ({ left: r.x, top: r.y, width: r.w, height: r.h, x: r.x, y: r.y, right: r.x + r.w, bottom: r.y + r.h, toJSON: () => ({}) });
  }

  it('the frame sits over its card; the card unmounted hides it, never removes it; fullscreen moves the same frame to the tab', async () => {
    const { store, tabs } = rig();
    const layer = render(<ViewsLayer store={store} />);
    const root = layer.getByTestId('views.layer');
    box(root, { x: 0, y: 0, w: 1440, h: 900 });
    const view = { plugin: 'fake-app', server: 'fake-app', uri: 'ui://fake-app/counter', call: REF.call, state: 'live' as const };
    const card = render(<ViewCard project="p" agent="a" view={view} store={store} />);
    await flush();
    const frame = layer.getByTestId(`views.frame.${viewKey(REF)}`) as HTMLIFrameElement;
    expect(frame.getAttribute('src'), "the hub's proxy URL for this load, its session with it").toBe(`${ORIGIN}/mcp-app?s=${SESSION}`);
    expect(frame.getAttribute('sandbox')).toBe('allow-scripts allow-same-origin allow-forms');
    expect(frame.getAttribute('allow')).toBe('midi');
    // the card's anchor, in the window (happy-dom's is 1024 × 768): its frame clipped at the window's bottom
    const anchor = card.getByTestId('views.card.anchor');
    box(anchor, { x: 300, y: 700, w: 560, h: 160 });
    store.anchor(REF, 'thread', anchor);
    const last = new Map<string, Place | null>();
    const frames = step(store, root, last);
    expect(frame.style.visibility).toBe('visible');
    expect([frame.style.left, frame.style.top, frame.style.width, frame.style.height, frame.style.zIndex]).toEqual(['300px', '700px', '560px', '160px', '2']);
    expect(frame.style.clipPath).toBe('inset(0px 0px 92px 0px round var(--r-2))');
    expect(frames).toEqual([{ project: 'p', agent: 'a', call: REF.call, origin: ORIGIN, rect: { x: 300, y: 700, w: 560, h: 68 }, perms: ['midi'], covers: [] }]);
    // fullscreen: its tab opens, the card says so, the same frame goes over the tab at z 4
    act(() => {
      store.fullscreen(REF);
    });
    expect(tabs.has(viewKey(REF))).toBe(true);
    expect(card.getByTestId(`views.card.${REF.call}`).dataset['state']).toBe('panel');
    const tab = render(<ViewTab {...REF} store={store} />);
    const tanchor = tab.getByTestId('views.tab.anchor');
    box(tanchor, { x: 800, y: 52, w: 640, h: 848 });
    store.anchor(REF, 'tab', tanchor);
    step(store, root, last);
    expect(layer.getByTestId(`views.frame.${viewKey(REF)}`)).toBe(frame);
    expect([frame.style.left, frame.style.width, frame.style.zIndex]).toEqual(['800px', '640px', '4']);
    // the tab unmounted (another tab in front): hidden, still there
    tab.unmount();
    step(store, root, last);
    expect(frame.style.visibility).toBe('hidden');
    expect(frame.isConnected).toBe(true);
    // back inline, then the card unmounted (scrolled out of the virtual list): hidden, kept
    act(() => {
      store.inline(REF);
    });
    card.unmount();
    expect(step(store, root, last)).toEqual([]);
    expect(frame.style.visibility).toBe('hidden');
    expect(layer.getByTestId(`views.frame.${viewKey(REF)}`)).toBe(frame);
  });

  it("a menu, a float or a scrim drawn over a frame is told to main as that frame's covers", async () => {
    const { store } = rig();
    const root = document.createElement('div');
    document.body.append(root);
    box(root, { x: 0, y: 0, w: 1440, h: 900 });
    store.ensure(REF);
    await flush();
    const frame = document.createElement('iframe');
    frame.dataset['view'] = viewKey(REF);
    root.append(frame);
    const anchor = document.createElement('div');
    document.body.append(anchor);
    box(anchor, { x: 300, y: 100, w: 560, h: 160 });
    store.anchor(REF, 'thread', anchor);
    expect(step(store, root, new Map())[0]?.covers, 'nothing open').toEqual([]);
    const menu = document.createElement('div');
    menu.setAttribute('role', 'menu');
    box(menu, { x: 800, y: 150, w: 200, h: 300 });
    const tip = document.createElement('div');
    tip.className = 'b-tip';
    box(tip, { x: 300, y: 100, w: 50, h: 20 });
    document.body.append(menu, tip);
    expect(step(store, root, new Map())[0]?.covers, "the menu's part over the frame; a tip takes no press").toEqual([{ x: 800, y: 150, w: 60, h: 110 }]);
    for (const el of [menu, tip, anchor, root]) el.remove();
  });

    it(`past ${String(MAX_LIVE)} live views the one shown longest ago is torn down, then comes back from the hub`, async () => {
    const { store, timers, asked } = rig();
    const refs = Array.from({ length: MAX_LIVE + 1 }, (_, i) => ({ ...REF, call: `1-${String(i)}` }));
    for (const r of refs) {
      store.ensure(r);
      await flush();
    }
    // the oldest went (its teardown bound ran: it had no page yet, so at once)
    for (const t of timers) t();
    expect(store.live()).toHaveLength(MAX_LIVE);
    expect(store.live()).not.toContain(viewKey(refs[0] as ViewRef));
    expect(store.state(refs[0] as ViewRef)).toEqual({ kind: 'none' });
    store.ensure(refs[0] as ViewRef);
    await flush();
    expect(asked.filter((c) => c.call === '1-0')).toHaveLength(2);
    expect(store.state(refs[0] as ViewRef).kind).toBe('live');
  });

  it('a view the layer shows is never torn down, even the one shown longest ago (step tells the store what shows)', async () => {
    const { store, timers } = rig();
    // the layer's DOM without its animation-frame loop (it would mark REF seen at every flush)
    const root = document.createElement('div');
    document.body.append(root);
    box(root, { x: 0, y: 0, w: 1440, h: 900 });
    store.ensure(REF);
    await flush();
    const frame = document.createElement('iframe');
    frame.dataset['view'] = viewKey(REF);
    root.append(frame);
    const anchor = document.createElement('div');
    document.body.append(anchor);
    box(anchor, { x: 300, y: 100, w: 560, h: 160 });
    store.anchor(REF, 'thread', anchor);
    expect(step(store, root, new Map())).toHaveLength(1);
    // MAX_LIVE more cards mount before the next frame: REF, placed now, is the one seen longest ago
    for (let i = 0; i < MAX_LIVE; i += 1) {
      store.ensure({ ...REF, call: `2-${String(i)}` });
      await flush();
    }
    for (const t of timers) t();
    expect(store.live()).toHaveLength(MAX_LIVE);
    expect(store.live(), 'the shown one stays').toContain(viewKey(REF));
    expect(store.live(), 'the oldest hidden one went').not.toContain(viewKey({ ...REF, call: '2-0' }));
    anchor.remove();
    root.remove();
  });
});
