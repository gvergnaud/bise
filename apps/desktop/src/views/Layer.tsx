// src/views · the views layer (docs/mcp-apps.md risk 2, native-lead m_50044): mounted once in the
// window next to the Frame's columns, it holds every live view's frame and places each over its
// anchor every animation frame (an anchor moves with its thread's scroll, a row above that grows,
// the panel's width): its box, clip-path to its column, z 2 in the thread and 4 in a tab. Its
// container is absolute with no z-index (no stacking context), so the window's floats, scrims and
// menus stay above. A frame is never moved in the DOM; an anchor gone hides it (visibility). It
// tells Electron main the frames that show (window.bise.views.frames: permissions and gestures
// are main's). Owner: mcp-apps-host.
import { memo, useEffect, useRef } from 'react';
import { allowOf } from '@/shared/viewperms';
import { pick, samePlace, type Anchors, type Place, type Rect } from './place';
import type { Anchor, ViewsStore } from './store';
import { useViewOpen, useViewsVersion } from './hooks';
import { viewsStore } from './window';
import type { ViewFrame } from '@/shared/api';
import './views.css';

function rectOf(el: Element, off: { x: number; y: number }): Rect {
  const r = el.getBoundingClientRect();
  return { x: r.left - off.x, y: r.top - off.y, w: r.width, h: r.height };
}

function boxOf(a: Anchor | undefined, off: { x: number; y: number }): Anchors['thread'] {
  if (a === undefined || !a.el.isConnected) return undefined;
  return { box: rectOf(a.el, off), clips: [...a.clips.filter((c) => c.isConnected).map((c) => rectOf(c, off)), { x: -off.x, y: -off.y, w: window.innerWidth, h: window.innerHeight }] };
}

/** Writes a place on a frame: its box, clip and z, or hidden; `round`: its corners at r-2 (a view that prefers a border, designer m_50319). */
function put(el: HTMLIFrameElement, p: Place | null, round: boolean): void {
  if (p === null) {
    el.style.visibility = 'hidden';
    return;
  }
  const s = el.style;
  s.visibility = 'visible';
  s.left = `${String(p.box.x)}px`;
  s.top = `${String(p.box.y)}px`;
  s.width = `${String(p.box.w)}px`;
  s.height = `${String(p.box.h)}px`;
  s.zIndex = String(p.z);
  const [t, r, b, l] = p.inset;
  const corner = round ? ' round var(--r-2)' : '';
  s.clipPath = t === 0 && r === 0 && b === 0 && l === 0 && !round ? '' : `inset(${String(t)}px ${String(r)}px ${String(b)}px ${String(l)}px${corner})`;
}

/** One place per live view, read from its anchors now (the layer's frame step). */
export function step(store: ViewsStore, root: HTMLElement, last: Map<string, Place | null>): ViewFrame[] {
  const o = root.getBoundingClientRect();
  const off = { x: o.left, y: o.top };
  const frames: ViewFrame[] = [];
  const showing = new Set<string>();
  for (const key of store.live()) {
    const live = store.liveOf(key);
    const el = root.querySelector<HTMLIFrameElement>(`iframe[data-view="${CSS.escape(key)}"]`);
    if (live === undefined || el === null) continue;
    const a = store.anchorsOf(key);
    const anchors: Anchors = { ...(boxOf(a.thread, off) === undefined ? {} : { thread: boxOf(a.thread, off) }), ...(boxOf(a.tab, off) === undefined ? {} : { tab: boxOf(a.tab, off) }) };
    if (anchors.thread !== undefined) store.size(key, 'thread', anchors.thread.box.w, anchors.thread.box.h);
    if (anchors.tab !== undefined) store.size(key, 'tab', anchors.tab.box.w, anchors.tab.box.h);
    const p = pick(store.modeOfKey(key), anchors);
    if (!samePlace(last.get(key) ?? null, p)) put(el, p, live.res.prefersBorder);
    last.set(key, p);
    if (p === null) continue;
    showing.add(key);
    store.seen(key);
    frames.push({ project: live.ref.project, agent: live.ref.agent, call: live.ref.call, origin: live.res.origin, rect: { x: p.seen.x + off.x, y: p.seen.y + off.y, w: p.seen.w, h: p.seen.h }, perms: [...live.res.permissions] });
  }
  for (const k of [...last.keys()]) if (!store.live().includes(k)) last.delete(k);
  return frames;
}

const ViewFrameEl = memo(function ViewFrameEl({ store, k }: { store: ViewsStore; k: string }) {
  const live = store.liveOf(k);
  if (live === undefined) return null;
  return (
    <iframe
      ref={(el) => {
        store.frame(k, el);
      }}
      className="vw-frame"
      data-view={k}
      data-testid={`views.frame.${k}`}
      src={`${live.res.origin}/mcp-app`}
      sandbox="allow-scripts allow-same-origin allow-forms"
      allow={allowOf(live.res.permissions)}
      title={k}
      style={{ visibility: 'hidden' }}
    />
  );
});

/** The window's views layer: one per window, in app/ beside the Frame's columns. */
export function ViewsLayer({ store = viewsStore }: { store?: ViewsStore }) {
  useViewsVersion(store);
  useViewOpen(store);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => store.listen(window), [store]);
  // a tab opened or closed is a mode changed; a theme changed is a context changed
  useEffect(() => store.watch(() => {
    store.changed();
  }), [store]);
  useEffect(() => {
    const mo = new MutationObserver(() => {
      store.changed();
    });
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    const mq = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: light)') : null;
    const on = () => {
      store.changed();
    };
    mq?.addEventListener('change', on);
    return () => {
      mo.disconnect();
      mq?.removeEventListener('change', on);
    };
  }, [store]);
  const live = store.live();
  const any = live.length > 0;
  // every animation frame while a view lives: its anchor's box can move without an event
  useEffect(() => {
    const el = root.current;
    if (!any || el === null) return;
    const last = new Map<string, Place | null>();
    let told = '';
    let raf = 0;
    const tick = () => {
      const frames = step(store, el, last);
      const said = JSON.stringify(frames);
      if (said !== told) {
        told = said;
        (window as Partial<Window>).bise?.views?.frames(frames);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      (window as Partial<Window>).bise?.views?.frames([]);
    };
  }, [store, any]);
  return (
    <div ref={root} className="vw-layer" data-testid="views.layer">
      {live.map((k) => (
        <ViewFrameEl key={k} store={store} k={k} />
      ))}
    </div>
  );
}
