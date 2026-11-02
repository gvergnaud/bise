// @vitest-environment happy-dom
// Office.tsx on fake parts and a fake renderer: the toggle, the loop's start and stop, the window
// away, a lost WebGL context, a click on a bim. No three, no GPU.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/proto';
import type { Activity, Label, Motion, Office as OfficeMap, OfficeRenderer, Pose, Timeline } from './model';
import { BimsSwitch, labelsSig, Office, tagOf, type Parts } from './Office';
import { bimsLit, FOLLOW_ZOOM } from './focus';
import { bimsView } from './view';
import { readyMaps } from './maps';
import { bims as bimsWords } from '@/proto/words_bims.gen';

// ---- a hand-driven rAF: a test runs the frames itself
let frames: (() => void)[] = [];
const flush = async (n = 1) => {
  for (let i = 0; i < n; i++) {
    const now = frames;
    frames = [];
    await act(async () => {
      for (const f of now) f();
      await Promise.resolve();
    });
  }
};

const agent = (name: string, main = false): Agent => ({ name, main, status: 'working', archived: false }) as unknown as Agent;
const AGENTS = [agent('main', true), agent('perf')];

/** A renderer that says what the test wants (`motion`) and counts its frames. */
function fakeRenderer() {
  let lostCb = (): void => undefined;
  const r = {
    motion: 'still' as Motion,
    frames: 0,
    setOffice: vi.fn(),
    setObjects: vi.fn(() => Promise.resolve(false)),
    setSize: vi.fn(() => 120),
    setCam: vi.fn(),
    frame: vi.fn((): Motion => {
      r.frames++;
      return r.motion;
    }),
    /** s to the next idle move, as the renderer says it after a frame (render/idle.ts) */
    wake: null as number | null,
    idleWake: () => r.wake,
    headOf: vi.fn(() => ({ x: 10, y: 20, feet: 40 })),
    pick: vi.fn((x: number): string | null => (x < 50 ? 'perf' : null)),
    /** the bims' desks on the canvas (an object's tip keeps off them) */
    deskSpots: () => [],
    copyTo: vi.fn(),
    falling: () => null,
    onLost: (cb: () => void) => {
      lostCb = cb;
    },
    lose: () => {
      lostCb();
    },
    dispose: vi.fn(),
  };
  return r;
}

function fakeParts(r: ReturnType<typeof fakeRenderer>, o: { moving?: () => boolean; labels?: () => ReadonlyMap<string, Label>; fail?: boolean } = {}) {
  const pose = (name: string): Pose => ({ x: 0, z: 0, dir: 0, moving: false, since: 0, seg: { t: 0, act: { k: 'work', at: 'desk' }, sig: name, spot: { x: 0, z: 0, wz: 0, ax: 0, face: 0 }, path: { pts: [], seg: [], len: 0 }, speed: 2.4, from: null } });
  const timeline: Timeline & { updates: number } = {
    updates: 0,
    update() {
      timeline.updates++;
    },
    poseAt: (name) => pose(name),
    names: () => ['main', 'perf'],
    moving: () => o.moving?.() ?? false,
  };
  const parts: Parts = {
    activities: (snap) => new Map<string, Activity>(snap.agents.map((a) => [a.name, { k: 'work', at: 'desk' }])),
    layoutOf: () => ({}) as OfficeMap,
    newTimeline: () => timeline,
    labelsOf: () => o.labels?.() ?? new Map(),
    load: () => (o.fail === true ? Promise.reject(new Error('no chunk')) : Promise.resolve(() => r as unknown as OfficeRenderer)),
  };
  return { parts, timeline };
}

const mount = (parts: Parts, onOpen = vi.fn()) =>
  render(<Office project="p1" agents={AGENTS} cards={[]} places={[]} parts={parts} onOpen={onOpen} />);

beforeEach(() => {
  frames = [];
  vi.stubGlobal('requestAnimationFrame', (f: () => void) => {
    frames.push(f);
    return frames.length;
  });
  vi.stubGlobal('cancelAnimationFrame', () => {
    frames = [];
  });
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  );
  (window as { bise?: { test?: boolean } }).bise = { test: true };
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => false });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  bimsView.reset();
});

describe('a tab that fills its pane (fill)', () => {
  it("the canvas is the holder's whole box, the office centred in it; off, its natural height for the width", async () => {
    const w = vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(624);
    const h = vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(800);
    try {
      const r = fakeRenderer();
      const { parts } = fakeParts(r);
      render(<Office project="p1" agents={AGENTS} cards={[]} places={[]} parts={parts} onOpen={vi.fn()} fill />);
      await flush();
      expect(screen.getByTestId('app.right.bims').dataset.fill).toBe('true');
      expect(r.setSize).toHaveBeenCalledWith(624, 800, true);
      expect(screen.getByTestId('app.right.bims').style.height).toBe('');
      cleanup();
      const r2 = fakeRenderer();
      mount(fakeParts(r2).parts);
      await flush();
      expect(r2.setSize).toHaveBeenCalledWith(624, 4000);
      expect(screen.getByTestId('app.right.bims').style.height).toBe('120px');
    } finally {
      w.mockRestore();
      h.mockRestore();
    }
  });
});

describe('the toggle', () => {
  it("no 'bims' pill in the head (the title bar's icon); 320 | wide only with the office on (the default, r2-launcher #9); kept", () => {
    bimsView.setView('list');
    const { rerender } = render(<BimsSwitch view={bimsView.get()} />);
    expect(screen.queryByTestId('app.right.agents.view.bims')).toBeNull();
    expect(screen.queryByTestId('app.right.agents.width.wide')).toBeNull();
    bimsView.setView('bims');
    rerender(<BimsSwitch view={bimsView.get()} />);
    fireEvent.click(screen.getByTestId('app.right.agents.width.wide'));
    expect(bimsView.get()).toEqual({ view: 'bims', wide: true });
    bimsView.reset();
    expect(bimsView.get()).toEqual({ view: 'bims', wide: true });
  });
  it('each choice says itself on hover (taste 15)', () => {
    render(<BimsSwitch view={{ view: 'bims', wide: false }} />);
    for (const id of ['width.narrow', 'width.wide']) expect(screen.getByTestId(`app.right.agents.${id}`).dataset.tip).not.toBe('');
  });
});

describe('the loop', () => {
  it('nothing while the renderer loads (the rows show under it), then the office; one frame and it stops when all is still', async () => {
    const r = fakeRenderer();
    const { parts } = fakeParts(r);
    mount(parts);
    expect(screen.getByTestId('app.right.bims').dataset.state).toBe('loading');
    await flush();
    expect(screen.getByTestId('app.right.bims').dataset.state).toBe('on');
    await flush(3);
    expect(r.frames).toBe(1);
    expect(frames).toHaveLength(0);
    expect(r.setOffice).toHaveBeenCalledTimes(1);
  });
  it('runs while something walks, stops after', async () => {
    const r = fakeRenderer();
    let walking = true;
    const { parts } = fakeParts(r, { moving: () => walking });
    mount(parts);
    await flush(5);
    expect(r.frames).toBe(4);
    walking = false;
    await flush(5);
    expect(r.frames).toBe(5);
    expect(frames).toHaveLength(0);
  });
  it("the renderer's walk keeps it at the display rate", async () => {
    const r = fakeRenderer();
    r.motion = 'walk';
    mount(fakeParts(r).parts);
    await flush(4);
    expect(frames).toHaveLength(1);
    r.motion = 'still';
    await flush(3);
    expect(frames).toHaveLength(0);
  });
  it('the window hidden: no frame at all; back: one frame', async () => {
    const r = fakeRenderer();
    r.motion = 'walk';
    let hidden = false;
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden });
    mount(fakeParts(r).parts);
    await flush(2);
    const n = r.frames;
    hidden = true;
    act(() => {
      document.dispatchEvent(new Event('visibilitychange'));
    });
    await flush(5);
    expect(r.frames).toBe(n);
    expect(frames).toHaveLength(0);
    hidden = false;
    r.motion = 'still';
    act(() => {
      document.dispatchEvent(new Event('visibilitychange'));
    });
    await flush(3);
    expect(r.frames).toBe(n + 1);
  });
  it("a still office with resting bims sleeps until the next idle move (render/idle.ts), then draws the burst at 30 fps", async () => {
    const waits: number[] = [];
    const set = globalThis.setTimeout;
    vi.stubGlobal('setTimeout', (f: () => void, ms?: number) => {
      waits.push(ms ?? 0);
      return set(f, 1e9);
    });
    const r = fakeRenderer();
    r.wake = 7.25;
    mount(fakeParts(r).parts);
    await flush(3);
    // no frame between moves: one timer, at the move's start
    expect(frames).toHaveLength(0);
    expect(waits).toContain(7250);
    expect(waits).not.toContain(33);
    cleanup();
    // a move running: the renderer says 'idle', a frame every 33 ms
    waits.length = 0;
    const s = fakeRenderer();
    s.motion = 'idle';
    s.wake = 20;
    mount(fakeParts(s).parts);
    await flush(3);
    expect(waits).toContain(33);
    expect(waits).not.toContain(20000);
  });
  it('a new snapshot kicks one frame and the timeline hears of it', async () => {
    const r = fakeRenderer();
    const { parts, timeline } = fakeParts(r);
    const { rerender } = mount(parts);
    await flush(2);
    expect(timeline.updates).toBe(1);
    rerender(<Office project="p1" agents={[...AGENTS, agent('ui')]} cards={[]} places={[]} parts={parts} onOpen={vi.fn()} />);
    await flush(2);
    expect(r.frames).toBe(2);
    expect(timeline.updates).toBe(2);
    // a new agent: a new office
    expect(r.setOffice).toHaveBeenCalledTimes(2);
  });
});

describe('WebGL lost', () => {
  it('one line, never a blank section (the rows are the bar\'s, under it); the loop stops', async () => {
    const r = fakeRenderer();
    r.motion = 'walk';
    mount(fakeParts(r).parts);
    await flush(2);
    act(() => {
      r.lose();
    });
    expect(screen.getByTestId('app.right.bims.lost').textContent).not.toBe('');
    expect(screen.queryByTestId('app.right.bims.canvas')).toBeNull();
    const n = r.frames;
    await flush(3);
    expect(r.frames).toBe(n);
  });
  it('a renderer that fails to load: the same', async () => {
    mount(fakeParts(fakeRenderer(), { fail: true }).parts);
    await flush();
    expect(screen.getByTestId('app.right.bims.lost')).toBeTruthy();
  });
});

describe('the overlay', () => {
  it('a pill alone has no name; hover, ctrl or an event give the line', () => {
    expect(tagOf({ name: 'perf', mode: 'pill', words: '', title: '', pill: 'asks' })).toEqual({ text: null, pill: 'asks' });
    expect(tagOf({ name: 'perf', mode: 'name', words: '', title: '', pill: 'asks' })).toEqual({ text: 'perf', pill: 'asks' });
    expect(tagOf({ name: 'perf', mode: 'event', words: 'done', title: '', pill: null })).toEqual({ text: 'perf · done', pill: null });
    expect(tagOf({ name: 'perf', mode: 'name', words: '', title: '', pill: null })).toEqual({ text: 'perf', pill: null });
    expect(tagOf({ name: 'perf', mode: 'hover', words: 'working', title: 'fix', pill: 'failed' })).toEqual({ text: 'perf', pill: 'failed' });
    // main is 'bise', as its AGENTS row (designer m_28987)
    expect(tagOf({ name: 'main', mode: 'hover', words: 'working', title: '', pill: null })).toEqual({ text: 'bise', pill: null });
    expect(tagOf({ name: 'main', mode: 'event', words: 'done', title: '', pill: null })).toEqual({ text: 'bise · done', pill: null });
  });
  it('labels re-render only when their content changes', () => {
    const a = new Map<string, Label>([['perf', { name: 'perf', mode: 'event', words: 'done', title: '', pill: null }]]);
    expect(labelsSig(a)).toBe(labelsSig(new Map(a)));
    expect(labelsSig(a)).not.toBe(labelsSig(new Map<string, Label>([['perf', { name: 'perf', mode: 'name', words: '', title: '', pill: null }]])));
  });
  it('shows the pills with their tip; a click on a pill or a bim opens it', async () => {
    const r = fakeRenderer();
    const onOpen = vi.fn();
    mount(fakeParts(r, { labels: () => new Map<string, Label>([['perf', { name: 'perf', mode: 'pill', words: '', title: '', pill: 'asks' }]]) }).parts, onOpen);
    await flush(2);
    const pill = screen.getByTestId('app.right.bims.pill.perf');
    expect(pill.dataset.pill).toBe('asks');
    expect(pill.dataset.tip).toContain('perf');
    expect(screen.queryByTestId('app.right.bims.label.perf')).toBeNull();
    fireEvent.click(pill);
    expect(onOpen).toHaveBeenCalledWith('perf');
    fireEvent.click(screen.getByTestId('app.right.bims'), { clientX: 10, clientY: 10 });
    expect(onOpen).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByTestId('app.right.bims'), { clientX: 200, clientY: 10 });
    expect(onOpen).toHaveBeenCalledTimes(2);
  });
});

describe('the hover card (Gabriel: multi-line, easy to read)', () => {
  it('its name with its pill, its state, its title on its own lines', async () => {
    const r = fakeRenderer();
    const card: Label = { name: 'perf', mode: 'hover', words: 'working', title: 'signup 4.1 s to 0.9 s', pill: 'asks' };
    mount(fakeParts(r, { labels: () => new Map([['perf', card]]) }).parts);
    await flush(2);
    const el = screen.getByTestId('app.right.bims.label.perf');
    expect(el.dataset.mode).toBe('hover');
    expect(el.querySelector('.bims-card-name')?.textContent).toBe('perf');
    expect(screen.getByTestId('app.right.bims.pill.perf').dataset.pill).toBe('asks');
    expect(screen.getByTestId('app.right.bims.card.state.perf').textContent).toBe('working');
    expect(screen.getByTestId('app.right.bims.card.title.perf').textContent).toBe('signup 4.1 s to 0.9 s');
  });
  // Gabriel m_48440, designer m_48444: the card over the tile hid the very bim the row points at
  it('a row hovered lights its bim and never opens its card; a hover on the bim itself does, after the delay', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    try {
      const r = fakeRenderer();
      const { parts } = fakeParts(r);
      const labelsOf = vi.spyOn(parts, 'labelsOf');
      mount(parts);
      await flush(2);
      const last = () => labelsOf.mock.calls.at(-1) as unknown as [readonly { name: string; lit?: string }[], { hover: string | null }];
      act(() => {
        bimsLit.set({ name: 'perf', from: 'row' });
      });
      act(() => {
        vi.advanceTimersByTime(2000);
      });
      await flush(2);
      expect(last()[0].find((f) => f.name === 'perf')?.lit).toBe('row');
      expect(last()[1].hover).toBeNull();
      act(() => {
        bimsLit.set(null, 'row');
      });
      // on the bim itself (x < 50 picks perf): its card after the delay
      const office = screen.getByTestId('app.right.bims');
      office.getBoundingClientRect = () => ({ left: 0, top: 0, right: 320, bottom: 120, width: 320, height: 120, x: 0, y: 0, toJSON: () => ({}) });
      fireEvent.pointerMove(office, { clientX: 10, clientY: 10 });
      act(() => {
        vi.advanceTimersByTime(2000);
      });
      await flush(2);
      expect(last()[1].hover).toBe('perf');
    } finally {
      bimsLit.set(null);
      vi.useRealTimers();
    }
  });
});

/** a wheel event at (100, 50); a pinch is one with ctrlKey (happy-dom's WheelEvent drops ctrlKey and the point from its init) */
function wheel(o: { deltaX?: number; deltaY?: number; ctrlKey?: boolean }): WheelEvent {
  const e = new WheelEvent('wheel', { deltaX: o.deltaX ?? 0, deltaY: o.deltaY ?? 0, cancelable: true, bubbles: true });
  Object.defineProperty(e, 'ctrlKey', { value: o.ctrlKey === true });
  Object.defineProperty(e, 'clientX', { value: 100 });
  Object.defineProperty(e, 'clientY', { value: 50 });
  return e;
}

describe('his camera (zoom.ts)', () => {
  // happy-dom lays nothing out: the office says it is 320 × 120
  const office = () => {
    const el = screen.getByTestId('app.right.bims');
    el.getBoundingClientRect = () => ({ left: 0, top: 0, right: 320, bottom: 120, width: 320, height: 120, x: 0, y: 0, toJSON: () => ({}) });
    return el;
  };
  const lastCam = (r: ReturnType<typeof fakeRenderer>) => r.setCam.mock.calls.at(-1)?.[0] as { zoom: number; u: number; v: number };
  it('a pinch zooms, a draw follows; at the fit the wheel scrolls the bar', async () => {
    const r = fakeRenderer();
    mount(fakeParts(r).parts);
    await flush(2);
    const plain = wheel({ deltaY: 40 });
    office().dispatchEvent(plain);
    expect(plain.defaultPrevented).toBe(false);
    expect(screen.queryByTestId('app.right.bims.fit')).toBeNull();
    const n = r.frames;
    const pinch = wheel({ deltaY: -50, ctrlKey: true });
    act(() => {
      office().dispatchEvent(pinch);
    });
    expect(pinch.defaultPrevented).toBe(true);
    expect(lastCam(r).zoom).toBeGreaterThan(1);
    await flush(1);
    expect(r.frames).toBe(n + 1);
    // zoomed: two fingers move the view, the bar does not scroll
    const pan = wheel({ deltaX: 20 });
    act(() => {
      office().dispatchEvent(pan);
    });
    expect(pan.defaultPrevented).toBe(true);
    expect(screen.getByTestId('app.right.bims.fit').dataset.tip).not.toBe('');
  });
  it("'⤢ fit', esc and a double-click on the floor go back to the whole office", async () => {
    const r = fakeRenderer();
    const onOpen = vi.fn();
    mount(fakeParts(r).parts, onOpen);
    await flush(2);
    const zoomIn = () => {
      act(() => {
        office().dispatchEvent(wheel({ deltaY: -80, ctrlKey: true }));
      });
    };
    zoomIn();
    fireEvent.click(screen.getByTestId('app.right.bims.fit'));
    expect(lastCam(r)).toEqual({ zoom: 1, u: 0.5, v: 0.5 });
    expect(screen.queryByTestId('app.right.bims.fit')).toBeNull();
    expect(onOpen).not.toHaveBeenCalled();
    zoomIn();
    fireEvent.keyDown(office(), { key: 'Escape' });
    expect(lastCam(r).zoom).toBe(1);
    zoomIn();
    // on a bim (x < 50 picks perf): no fit
    fireEvent.doubleClick(office(), { clientX: 10, clientY: 10 });
    expect(lastCam(r).zoom).toBeGreaterThan(1);
    fireEvent.doubleClick(office(), { clientX: 200, clientY: 10 });
    expect(lastCam(r).zoom).toBe(1);
  });
  it('a drag moves the view while zoomed, and is never a click on a bim', async () => {
    const r = fakeRenderer();
    const onOpen = vi.fn();
    mount(fakeParts(r).parts, onOpen);
    await flush(2);
    act(() => {
      office().dispatchEvent(wheel({ deltaY: -80, ctrlKey: true }));
    });
    const before = lastCam(r);
    office().setPointerCapture = vi.fn();
    fireEvent.pointerDown(office(), { button: 0, pointerId: 1, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(office(), { pointerId: 1, clientX: 12, clientY: 10 });
    expect(lastCam(r)).toEqual(before);
    fireEvent.pointerMove(office(), { pointerId: 1, clientX: 40, clientY: 10 });
    expect(lastCam(r).u).toBeLessThan(before.u);
    fireEvent.pointerUp(office(), { pointerId: 1 });
    fireEvent.click(office(), { clientX: 10, clientY: 10 });
    expect(onOpen).not.toHaveBeenCalled();
    fireEvent.click(office(), { clientX: 10, clientY: 10 });
    expect(onOpen).toHaveBeenCalledWith('perf');
  });
});

describe('the follow camera (focus.ts)', () => {
  it('follows its bim at followZoom (FOLLOW_ZOOM by default); a new zoom eases there', async () => {
    let now = 1_000_000;
    const clock = vi.spyOn(Date, 'now').mockImplementation(() => now);
    try {
      const r = Object.assign(fakeRenderer(), { spotOf: vi.fn(() => ({ x: 0, y: 0, u: 0.5, v: 0.5 })) });
      const { parts } = fakeParts(r);
      const at = (zoom?: number) => <Office project="p1" agents={AGENTS} cards={[]} places={[]} parts={parts} onOpen={vi.fn()} follow="perf" followZoom={zoom} />;
      const run = async () => {
        for (let i = 0; i < 40; i++) {
          now += 100;
          await flush(1);
        }
      };
      const zoom = () => (r.setCam.mock.calls.at(-1)?.[0] as { zoom: number }).zoom;
      const { rerender } = render(at(2.2));
      await run();
      expect(zoom()).toBeCloseTo(2.2, 2);
      rerender(at(undefined));
      await run();
      expect(zoom()).toBeCloseTo(FOLLOW_ZOOM, 2);
      rerender(at(2.2));
      now += 100;
      await flush(1);
      // one step on: between the two, on its way back
      expect(zoom()).toBeLessThan(FOLLOW_ZOOM);
      expect(zoom()).toBeGreaterThan(2.2);
    } finally {
      clock.mockRestore();
    }
  });
});

describe('the map (/bims)', () => {
  /** parts whose office is the map asked for, so a new pick is another office */
  const mapParts = (r: ReturnType<typeof fakeRenderer>) => {
    const { parts } = fakeParts(r);
    return { ...parts, layoutOf: (_agents: unknown, map?: string) => ({ map: map ?? 'loft' }) as OfficeMap };
  };
  const mountMap = (parts: Parts, map: 'loft' | 'kyoto') =>
    render(<Office project="p1" agents={AGENTS} cards={[]} places={[]} map={map} parts={parts} onOpen={vi.fn()} />);

  it('a new map crossfades: the old office copied over the new one, then let go', async () => {
    const r = fakeRenderer();
    const parts = mapParts(r);
    const { rerender } = mountMap(parts, 'loft');
    await flush(3);
    expect(screen.getByTestId('app.right.bims').dataset.map).toBe('loft');
    expect(r.copyTo).not.toHaveBeenCalled();
    rerender(<Office project="p1" agents={AGENTS} cards={[]} places={[]} map="kyoto" parts={parts} onOpen={vi.fn()} />);
    await flush();
    expect(r.copyTo).toHaveBeenCalledTimes(1);
    expect(r.setOffice).toHaveBeenCalledTimes(2);
    const box = screen.getByTestId('app.right.bims');
    expect([box.dataset.want, box.dataset.map]).toEqual(['kyoto', 'kyoto']);
    expect(screen.getByTestId('app.right.bims.xfade').dataset.on).toBe('');
    await flush();
    expect(screen.getByTestId('app.right.bims.xfade').dataset.on).toBeUndefined();
  });

  it('a cut under reduced motion', async () => {
    vi.stubGlobal('matchMedia', (q: string) => ({ matches: q.includes('reduce'), addEventListener() {}, removeEventListener() {} }));
    const r = fakeRenderer();
    const parts = mapParts(r);
    const { rerender } = mountMap(parts, 'loft');
    await flush(3);
    rerender(<Office project="p1" agents={AGENTS} cards={[]} places={[]} map="kyoto" parts={parts} onOpen={vi.fn()} />);
    await flush();
    expect(r.setOffice).toHaveBeenCalledTimes(2);
    expect(r.copyTo).not.toHaveBeenCalled();
  });
});

describe('the map chip (brief bims-mapsel)', () => {
  const mountChip = (parts: Parts, o: { map?: 'loft' | 'kyoto' | 'workshop'; onPickMap?: (m: string) => void } = {}) =>
    render(<Office project="p1" agents={AGENTS} cards={[]} places={[]} {...(o.map === undefined ? {} : { map: o.map })} parts={parts} onOpen={vi.fn()} onPickMap={o.onPickMap ?? vi.fn()} />);
  const chip = () => screen.getByTestId('app.right.bims.map');

  it("shows the map's name, the loft when he never picked one; its name and tip say what it does", async () => {
    const { rerender } = mountChip(fakeParts(fakeRenderer()).parts);
    await flush(2);
    expect(chip().textContent).toBe(bimsWords.map_loft);
    expect(chip().getAttribute('aria-label')).toBe(bimsWords.map_chip_label);
    expect(chip().getAttribute('aria-haspopup')).toBe('menu');
    rerender(<Office project="p1" agents={AGENTS} cards={[]} places={[]} map="workshop" parts={fakeParts(fakeRenderer()).parts} onOpen={vi.fn()} onPickMap={vi.fn()} />);
    expect(chip().textContent).toBe(bimsWords.map_workshop);
  });

  it('a click opens the menu: the ready maps in order, two-line, the current one checked', async () => {
    mountChip(fakeParts(fakeRenderer()).parts, { map: 'kyoto' });
    await flush(2);
    expect(screen.queryByTestId('app.right.bims.map.menu')).toBeNull();
    fireEvent.click(chip());
    expect(screen.getByTestId('app.right.bims.map.menu')).toBeTruthy();
    expect(chip().getAttribute('aria-expanded')).toBe('true');
    const rows = readyMaps().map((id) => screen.getByTestId(`app.right.bims.map.${id}`));
    expect(rows.map((r) => r.getAttribute('data-row'))).toEqual(readyMaps());
    expect(screen.getByTestId('app.right.bims.map.kyoto').getAttribute('aria-checked')).toBe('true');
    expect(screen.getByTestId('app.right.bims.map.loft').getAttribute('aria-checked')).toBe('false');
    expect(screen.getByTestId('app.right.bims.map.loft').textContent).toContain(bimsWords.map_loft_mood);
  });

  it("a pick sends the map (the caller writes /bims's prefs) and closes; no bim opens under it", async () => {
    const onPickMap = vi.fn();
    const onOpen = vi.fn();
    render(<Office project="p1" agents={AGENTS} cards={[]} places={[]} parts={fakeParts(fakeRenderer()).parts} onOpen={onOpen} onPickMap={onPickMap} />);
    await flush(2);
    fireEvent.click(chip(), { clientX: 10, clientY: 10 });
    fireEvent.click(screen.getByTestId('app.right.bims.map.kyoto'), { clientX: 10, clientY: 10 });
    expect(onPickMap).toHaveBeenCalledWith('kyoto');
    expect(onOpen).not.toHaveBeenCalled();
    expect(screen.queryByTestId('app.right.bims.map.menu')).toBeNull();
  });

  it('esc closes it with no change', async () => {
    const onPickMap = vi.fn();
    mountChip(fakeParts(fakeRenderer()).parts, { onPickMap });
    await flush(2);
    fireEvent.click(chip());
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByTestId('app.right.bims.map.menu')).toBeNull();
    expect(onPickMap).not.toHaveBeenCalled();
  });

  it('no chip without onPickMap (the town, the film)', async () => {
    mount(fakeParts(fakeRenderer()).parts);
    await flush(2);
    expect(screen.queryByTestId('app.right.bims.map')).toBeNull();
  });
});

describe('his camera from another element (cameraFrom: the right side\'s grown world)', () => {
  it('its pinch zooms and its drag moves the office\'s view as on the office; the click that ends a drag goes no further', async () => {
    const r = fakeRenderer();
    const from = document.createElement('span');
    const clicked = vi.fn();
    const outer = document.createElement('div');
    outer.addEventListener('click', clicked);
    outer.append(from);
    document.body.append(outer);
    render(<Office project="p1" agents={AGENTS} cards={[]} places={[]} parts={fakeParts(r).parts} onOpen={vi.fn()} cameraFrom={from} />);
    await flush(2);
    screen.getByTestId('app.right.bims').getBoundingClientRect = () => ({ left: 0, top: 0, right: 320, bottom: 120, width: 320, height: 120, x: 0, y: 0, toJSON: () => ({}) });
    const cam = () => r.setCam.mock.calls.at(-1)?.[0] as { zoom: number; u: number; v: number } | undefined;
    const pinch = wheel({ deltaY: -80, ctrlKey: true });
    act(() => {
      from.dispatchEvent(pinch);
    });
    expect(pinch.defaultPrevented).toBe(true);
    expect(cam()?.zoom).toBeGreaterThan(1);
    const before = cam() ?? { zoom: 0, u: 0, v: 0 };
    from.setPointerCapture = vi.fn();
    act(() => {
      from.dispatchEvent(new PointerEvent('pointerdown', { button: 0, pointerId: 1, clientX: 10, clientY: 10, bubbles: true }));
      from.dispatchEvent(new PointerEvent('pointermove', { pointerId: 1, clientX: 40, clientY: 10, bubbles: true }));
      from.dispatchEvent(new PointerEvent('pointerup', { pointerId: 1, bubbles: true }));
    });
    expect(cam()?.u).toBeLessThan(before.u);
    from.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    expect(clicked).not.toHaveBeenCalled();
    from.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    expect(clicked).toHaveBeenCalledTimes(1);
    outer.remove();
  });
});
