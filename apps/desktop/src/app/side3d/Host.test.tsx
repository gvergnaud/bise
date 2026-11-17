// @vitest-environment happy-dom
// The right side as he sees it on fake rows: the world and the four bubbles with their counts, a
// card per bubble, an ask's option sent through the store's own pick, esc and ⌥⌘1 (the screen
// action), nothing clipped on the way up (noClip), nothing while the panel is open.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { Profiler } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/proto';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { side } from '@/proto/words_side.gen';
import { bims } from '@/proto/words_bims.gen';
import { artifact, artifacts, card, ev, projects, row } from '../../../tests/feature/inputs';
import { bimsLit } from '@/bims/focus';
import { createRouter, RouterContext } from '../router';
import { emitAction } from '../menu';
import { HINT_HOLD_MS } from '../held';
import { CARD_GRACE_MS, CARD_HOLD_MS, CARD_HOVER_MS, CARD_OUT_MS, CARD_WARM_MS } from './Cards';
import { CoinSlot } from './Minis';
import { keyAction } from '../keys';
import { SideHost, bubbleTops } from './Host';
import { CARD_MAX, cardMaxH } from './layout';
import { sideLayoutNow } from './place';
import { beatPlace, hitBox } from './World';
import { outerWorld, TILE_H, TILE_MIN } from './tilestate';
import { welcoming } from '@/welcome/shown';
import { FOLLOW_BEAT_MS } from './OuterWorld';
import { panelState } from '../panel/state';
import { openTab as openPanelTab } from '../panel/tabs';
import '../escape';

// the office's renderer and the 3D pictures need WebGL: a stand-in office, pictures that never draw (the plain tile)
const office = vi.hoisted(() => ({
  cameraFrom: [] as (HTMLElement | null)[],
  follow: [] as (string | null)[],
  refit: [] as number[],
  zoomed: null as ((z: boolean) => void) | null,
}));
vi.mock('@/bims/Office', () => ({
  Office: ({ cameraFrom = null, follow = null, refit = 0, onZoomed }: { cameraFrom?: HTMLElement | null; follow?: string | null; refit?: number; onZoomed?: (z: boolean) => void }) => {
    office.cameraFrom.push(cameraFrom);
    office.follow.push(follow);
    office.refit.push(refit);
    office.zoomed = onZoomed ?? null;
    return <canvas data-testid="office" />;
  },
}));
// the town's renderer needs WebGL too: a stand-in town that lists its offices and gives its two clicks
vi.mock('@/bims/town/Town', () => ({
  Town: ({ projects, maxH, onOpenProject, onOpenAgent }: { projects: readonly { id: string }[]; maxH?: number; onOpenProject: (p: string) => void; onOpenAgent: (p: string, a: string) => void }) => (
    <div data-testid="town" data-max-h={maxH}>
      {projects.map((p) => (
        <button key={p.id} data-testid={`town.${p.id}`} onClick={() => {
          onOpenProject(p.id);
        }} onDoubleClick={() => {
          onOpenAgent(p.id, 'perf');
        }} />
      ))}
    </div>
  ),
}));
vi.mock('./render', () => ({
  outfitSig: () => '',
  objectPng: () => {
    throw new Error('no WebGL');
  },
  headPng: () => {
    throw new Error('no WebGL');
  },
  bubblePng: () => {
    throw new Error('no WebGL');
  },
}));
const panel = vi.hoisted(() => ({ open: false, width: 384, over: false, full: false }));
// the tabs the side opened (the tile's '⤢': the Office tab)
const opened = vi.hoisted(() => [] as unknown[]);
vi.mock('./ports', async (orig) => ({
  ...(await orig<typeof import('./ports')>()),
  openTab: (e: unknown) => {
    opened.push(e);
  },
  usePanelBox: () => ({ open: panel.open, width: panel.open && !panel.over ? panel.width : 0, over: panel.open && panel.over, full: panel.open && panel.full }),
  useFrontArtKind: () => undefined,
}));

afterEach(() => {
  cleanup();
  panel.open = false;
  panel.width = 384;
  panel.over = false;
  panel.full = false;
  opened.length = 0;
  outerWorld.hide(false);
  outerWorld.height(TILE_H, 900);
});

const HOME = 'bise-home-00000000';
const ACME = 'acme-1a2b3c4d';
const agent = (name: string, over: Partial<Agent> = {}): Agent => ({
  name, dir: name, main: false, status: 'idle', archived: false, title: 'speed', purpose: '', since_ms: 0, waits: 0, watching: [], ...over,
});

function side3d(winW = 1100, onCommit?: () => void) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  (window as unknown as { bise: unknown }).bise = { notify: { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined } };
  // 1440 wide: the world small (sideLayout), the rail and the sidebar on the left
  Object.defineProperty(window, 'innerWidth', { value: winW, configurable: true });
  const store = new Store(port);
  store.setCore('up');
  const ask = card({ project: ACME, id: 7, agent: 'perf', kind: 'question', question: 'run rm -rf target?', options: [{ n: 1, label: 'yes' }, { n: 2, label: 'no' }], approval: false });
  store.apply(projects(row({ project: HOME, name: 'bise', home: true }), row({ project: ACME, name: 'acme', cards: [ask], waits: 1 })));
  store.apply(ev('agents', {
    project: ACME,
    agents: [
      agent('main', { main: true }),
      agent('perf', { status: 'working', waits: 1, rank: 1 }),
      agent('docs', { status: 'working', rank: 2, note: 'checking the links' }),
      agent('old', { archived: true }),
    ],
  }));
  store.apply(artifacts(ACME, artifact({ id: 'a1', kind: 'pr', title: 'PR #6', agent: 'perf', new: true }), artifact({ id: 'a2', kind: 'page', title: 'Perf notes', agent: 'perf', new: false })));
  const does = vi.spyOn(store, 'do');
  const router = createRouter({ screen: 'thread', project: ACME, agent: 'perf' });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <Profiler id="side" onRender={() => onCommit?.()}>
          <SideHost />
        </Profiler>
      </RouterContext>
    </StoreProvider>,
  );
  return { v, store, does, router };
}

describe('the right side, folded', () => {
  it('the world and the four bubbles, each with its tip; the counts, none at 0; the agents\' two pills: active at the bottom, what waits for him on top in the accent (designer m_44986)', () => {
    const { v } = side3d();
    // the world a 5th coin while the card does not show (designer m_46218: never floating any more)
    expect(v.getByTestId('app.bubble.world')).toBeTruthy();
    expect(v.queryByTestId('app.world')).toBeNull();
    // each says what it counts to VoiceOver; its card is its tip (folded-side v9)
    for (const b of ['agents', 'artifacts', 'scheduled', 'processes']) expect(v.getByTestId(`app.bubble.${b}`).getAttribute('aria-label')).toBeTruthy();
    // 2 working (1 of them asks): 2 active, plain; the project's inbox (1) on top, the accent
    expect(v.getByTestId('app.bubble.agents.count').textContent).toBe('2');
    expect(v.getByTestId('app.bubble.agents.count').hasAttribute('data-accent')).toBe(false);
    expect(v.getByTestId('app.bubble.agents.count').getAttribute('data-tip')).toBe(side.active({ n: 2 }));
    expect(v.getByTestId('app.bubble.agents.badge').textContent).toBe('1');
    expect(v.getByTestId('app.bubble.agents.badge').hasAttribute('data-accent')).toBe(true);
    expect(v.getByTestId('app.bubble.agents.badge').getAttribute('data-tip')).toBe(side.waiting_you({ n: 1 }));
    expect(v.getByTestId('app.bubble.artifacts.count').textContent).toBe('1');
    expect(v.getByTestId('app.bubble.artifacts.count').hasAttribute('data-accent')).toBe(false);
    expect(v.queryByTestId('app.bubble.processes.count')).toBeNull();
    expect(v.getByTestId('app.bubble.agents').getAttribute('aria-label')).toBe(
      side.bubble_tip({ name: side.agents, says: `${side.active({ n: 2 })} · ${side.waiting_you({ n: 1 })}` }),
    );
  });

  it('the agents card: in the hub\'s rank, the asking one with its options; an option sends the inbox\'s pick from the thread in view', () => {
    const { v, does } = side3d();
    fireEvent.click(v.getByTestId('app.bubble.agents'));
    // Overview's rows (folded-side v9, A's floats): main first as bise, then by rank
    const lines = [...v.getByTestId('app.flyout.agents').querySelectorAll('.s3-arow')].map((e) => e.getAttribute('data-testid'));
    expect(lines).toEqual([`app.flyout.agents.${ACME}.main`, `app.flyout.agents.${ACME}.perf`, `app.flyout.agents.${ACME}.docs`]);
    // perf asks him: its row on the accent, its card's question as its line, its options in one click
    expect(v.getByTestId(`app.flyout.agents.${ACME}.perf`).hasAttribute('data-asks')).toBe(true);
    expect(v.getByTestId(`app.flyout.agents.${ACME}.perf.role`).textContent).toBe('run rm -rf target?');
    expect(v.getByTestId(`app.flyout.agents.${ACME}.docs`).hasAttribute('data-asks')).toBe(false);
    fireEvent.click(v.getByTestId(`app.flyout.agents.${ACME}.perf.opt.1`));
    expect(does).toHaveBeenCalledWith({ i: 'pick', project: ACME, card: 7, n: 1, from: { at: { project: ACME, agent: 'perf' } } });
    // a question (not an approval): 'answer…' (designer m_45824: an approval says 'write to perf…')
    expect(v.getByTestId(`app.flyout.agents.${ACME}.perf.answer`).textContent).toBe(side.answer);
    // ARCHIVED folded in its foot, open on a click
    expect(v.getByTestId('app.flyout.agents.archived').textContent).toContain(side.archived({ n: 1 }));
    expect(v.queryByTestId('app.flyout.agents.archived.rows')).toBeNull();
    fireEvent.click(v.getByTestId('app.flyout.agents.archived'));
    expect(v.getByTestId('app.flyout.agents.archived.rows').textContent).toContain('old');
  });

  it('the artifacts card: the new one with its dot; one card at a time; esc closes it', () => {
    const { v } = side3d();
    fireEvent.click(v.getByTestId('app.bubble.artifacts'));
    expect(v.getByTestId(`app.flyout.artifacts.${ACME}.a1`).querySelector('.s3-dot[data-tone="acc"]')).not.toBeNull();
    expect(v.getByTestId(`app.flyout.artifacts.${ACME}.a2`).querySelector('.s3-dot')).toBeNull();
    fireEvent.click(v.getByTestId('app.bubble.agents'));
    expect(v.queryByTestId('app.flyout.artifacts')).toBeNull();
    expect(v.getByTestId('app.bubble.agents').getAttribute('aria-expanded')).toBe('true');
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(v.queryByTestId('app.flyout.agents')).toBeNull();
  });

  it('a click in a float that navigates closes it, so one esc with nothing in focus closes the artifact in front (native-lead m_48053); one that acts in place keeps it', () => {
    panelState.reset();
    const { v, router } = side3d();
    act(() => {
      openPanelTab({ kind: 'artifact', project: ACME, id: 'a2' });
    });
    fireEvent.click(v.getByTestId('app.bubble.agents'));
    // the archived fold acts in place: the float stays
    fireEvent.click(v.getByTestId('app.flyout.agents.archived'));
    const old = v.getByTestId('app.flyout.agents.archived.rows').querySelector<HTMLElement>('[data-row]');
    expect(old).not.toBeNull();
    fireEvent.click(old as HTMLElement);
    expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'old' });
    expect(v.queryByTestId('app.flyout.agents')).toBeNull();
    // the window's one esc, the focus on nothing: the artifact's tab closes (escape.ts 7), the route stays
    (document.activeElement as HTMLElement | null)?.blur();
    act(() => {
      document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    });
    expect(panelState.get().tabs.list.some((t) => t.id === `artifact:${ACME}/a2`)).toBe(false);
    expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'old' });
    panelState.reset();
  });

  it('⌥⌘1-⌥⌘4 are the bubbles: the key gives the screen action, the side opens its card', () => {
    const k = (code: string) => keyAction({ key: '', code, metaKey: true, altKey: true, ctrlKey: false, shiftKey: false }, { route: { screen: 'bise' }, projects: [], agents: () => [], typing: false });
    expect(['Digit1', 'Digit2', 'Digit3', 'Digit4', 'Digit5'].map(k)).toEqual([{ screen: 'side-1' }, { screen: 'side-2' }, { screen: 'side-3' }, { screen: 'side-4' }, undefined]);
    const { v } = side3d();
    act(() => {
      emitAction('side-4');
    });
    expect(v.getByTestId('app.flyout.processes').textContent).toContain(side.no_processes);
    // the scheduled bubble: its own float (designer m_44975), its head, its one foot with its own link
    act(() => {
      emitAction('side-3');
    });
    const card = v.getByTestId('app.flyout.scheduled');
    expect(card.querySelector('.s3-card-layer[data-now] .s3-card-head b')?.textContent).toBe(side.scheduled);
    expect(card.querySelector('.s3-card-layer[data-now]')?.textContent).toContain(side.no_scheduled);
    expect(card.querySelector('.s3-card-layer[data-now]')?.textContent).not.toContain(side.processes);
    // its 'all … ›' on the float's one foot, never on a head (designer m_44411)
    expect(card.querySelector('.s3-card-head .s3-link')).toBeNull();
    expect(card.querySelectorAll('.s3-card-layer[data-now] footer')).toHaveLength(1);
    expect([...card.querySelectorAll('.s3-card-layer[data-now] footer .s3-link')].map((e) => e.textContent)).toEqual([side.all_scheduled]);
  });

  it('from one bubble to another, the one float moves: the same frame, its top and height animated 240 ms, the old content fading out inert over the new; reduced motion: a fade, no move (designer m_44990)', () => {
    const calls: Keyframe[][] = [];
    const proto = HTMLElement.prototype as unknown as { animate?: unknown };
    const had = proto.animate;
    proto.animate = function (frames: Keyframe[]) {
      calls.push(frames);
      return { cancel: () => undefined } as unknown as Animation;
    };
    try {
      const { v } = side3d();
      fireEvent.click(v.getByTestId('app.bubble.agents'));
      const frame = v.getByTestId('app.flyout.agents');
      expect(calls).toHaveLength(0);
      fireEvent.click(v.getByTestId('app.bubble.scheduled'));
      // one frame, its anchor and content changed, never a second mount
      expect(v.getByTestId('app.flyout.scheduled')).toBe(frame);
      expect(v.getByTestId('app.side').querySelectorAll('.s3-card')).toHaveLength(1);
      expect(frame.hasAttribute('data-moving')).toBe(true);
      expect(frame.style.top).toBe(v.getByTestId('app.bubble.scheduled').style.top);
      expect(calls).toHaveLength(1);
      expect(calls[0]?.[1]).toMatchObject({ top: `${String(bubbleTops('bubble').bubbles.scheduled)}px` });
      expect(calls[0]?.[0]).toHaveProperty('top');
      const old = v.getByTestId('app.flyout.agents.leaving');
      expect(old.hasAttribute('inert')).toBe(true);
      expect(old.getAttribute('aria-hidden')).toBe('true');
      // reduced motion: the frame takes its new place at once, the whole float fades in, no leaving layer
      document.documentElement.setAttribute('data-reduce-motion', '');
      fireEvent.click(v.getByTestId('app.bubble.processes'));
      expect(v.getByTestId('app.flyout.processes')).toBe(frame);
      expect(calls[1]).toEqual([{ opacity: 0 }, { opacity: 1 }]);
      expect(v.queryByTestId('app.flyout.scheduled.leaving')).toBeNull();
    } finally {
      document.documentElement.removeAttribute('data-reduce-motion');
      proto.animate = had;
    }
  });

  it('⌥ held alone 300 ms: each bubble shows its key in its count\'s place and its card, all four; gone at release (designer m_44292, m_44373; folded-side v9)', () => {
    vi.useFakeTimers();
    try {
      const { v } = side3d();
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Alt', altKey: true }));
        vi.advanceTimersByTime(HINT_HOLD_MS);
      });
      act(() => {
        vi.advanceTimersByTime(CARD_HOLD_MS - HINT_HOLD_MS - 1);
      });
      // not yet: the hints' 150 ms is not the cards' 300
      expect(v.queryByTestId('app.bubble.agents.key')).toBeNull();
      expect(v.queryByTestId('app.bubble.agents.card')).toBeNull();
      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(['agents', 'artifacts', 'scheduled', 'processes'].map((b) => v.getByTestId(`app.bubble.${b}.key`).textContent)).toEqual(['⌥⌘1', '⌥⌘2', '⌥⌘3', '⌥⌘4']);
      for (const b of ['agents', 'artifacts', 'scheduled', 'processes']) expect(v.getByTestId(`app.bubble.${b}.card`)).toBeTruthy();
      // the agents card: perf asks him, in the accent; 2 working under it
      const agents = v.getByTestId('app.bubble.agents.card');
      expect(agents.querySelector('[data-tone="acc"]')?.textContent).toBe(side.card_asks_one({ agent: 'perf' }));
      expect(agents.textContent).toContain(side.working({ n: 2 }));
      expect(v.queryByTestId('app.bubble.agents.count')).toBeNull();
      // the top accent pill stays while ⌥ is held
      expect(v.getByTestId('app.bubble.agents.badge').textContent).toBe('1');
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Alt' }));
      });
      expect(v.queryByTestId('app.bubble.agents.key')).toBeNull();
      expect(v.queryByTestId('app.bubble.agents.card')).toBeNull();
      expect(v.getByTestId('app.bubble.agents.count').textContent).toBe('2');
    } finally {
      vi.useRealTimers();
    }
  });

  // the law of the ⌥-alone hold (folded-side v9): ⌥ is a typing key ({ } [ ] | on a French
  // keyboard), so ⌥ with any other key never shows a card, however long it is held after
  it('⌥ with another key never shows the cards: ⌥[ , ⌥⇧, ⌥⌘, a key typed mid-hold', () => {
    vi.useFakeTimers();
    try {
      const { v } = side3d();
      // no card ever; ⌥⌘ held alone shows the coins' caps (their true key, designer m_45787), never a card
      const none = () => {
        expect(v.container.querySelector('.s3-tcard')).toBeNull();
      };
      const press = (...keys: KeyboardEventInit[]) => {
        act(() => {
          for (const k of keys) window.dispatchEvent(new KeyboardEvent('keydown', k));
          vi.advanceTimersByTime(HINT_HOLD_MS);
        });
        act(() => {
          vi.advanceTimersByTime(CARD_HOLD_MS * 4);
        });
        none();
        act(() => {
          window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Alt' }));
          window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Shift' }));
          window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Meta' }));
        });
      };
      // ⌥ then a character: { on a French keyboard (⌥( )
      press({ key: 'Alt', altKey: true }, { key: '{', code: 'Digit5', altKey: true });
      // ⌥ down while another modifier is held, or one joins it
      press({ key: 'Shift', shiftKey: true }, { key: 'Alt', altKey: true, shiftKey: true });
      press({ key: 'Alt', altKey: true }, { key: 'Shift', altKey: true, shiftKey: true });
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Alt', altKey: true }));
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Meta', altKey: true, metaKey: true }));
        vi.advanceTimersByTime(HINT_HOLD_MS);
      });
      act(() => {
        vi.advanceTimersByTime(CARD_HOLD_MS * 4);
      });
      none();
      expect(v.getByTestId('app.bubble.agents.key').textContent).toBe('⌥⌘1');
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Meta' }));
        window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Alt' }));
      });
      // a key typed mid-hold, after 200 ms: the hold ends, nothing shows after
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Alt', altKey: true }));
        vi.advanceTimersByTime(200);
        window.dispatchEvent(new KeyboardEvent('keydown', { key: '[', altKey: true }));
      });
      act(() => {
        vi.advanceTimersByTime(CARD_HOLD_MS * 4);
      });
      none();
      // shown, then a key: gone at once
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Alt' }));
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Alt', altKey: true }));
        vi.advanceTimersByTime(HINT_HOLD_MS);
      });
      act(() => {
        vi.advanceTimersByTime(CARD_HOLD_MS - HINT_HOLD_MS);
      });
      expect(v.getByTestId('app.bubble.agents.card')).toBeTruthy();
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: '|', altKey: true }));
      });
      none();
    } finally {
      vi.useRealTimers();
    }
  });

  it('a hover 150 ms shows that coin\'s card alone, information only; gone on leave; nothing at rest; a click opens the float, no card over it', () => {
    vi.useFakeTimers();
    try {
      const { v } = side3d();
      expect(v.container.querySelector('.s3-tcard')).toBeNull();
      // the card is the coin's tip: no ds tip on the coin
      expect(v.getByTestId('app.bubble.scheduled').hasAttribute('data-tip')).toBe(false);
      fireEvent.pointerEnter(v.getByTestId('app.bubble.scheduled'));
      act(() => {
        vi.advanceTimersByTime(CARD_HOVER_MS - 1);
      });
      expect(v.queryByTestId('app.bubble.scheduled.card')).toBeNull();
      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(v.getByTestId('app.bubble.scheduled.card').textContent).toContain(side.card_no_scheduled);
      expect(v.container.querySelectorAll('.s3-tcard')).toHaveLength(1);
      // no key on a hover: they are ⌥'s
      expect(v.queryByTestId('app.bubble.scheduled.key')).toBeNull();
      // gone on leave after a 100 ms grace (the gap between coins doesn't blink), then an 80 ms fade
      fireEvent.pointerLeave(v.getByTestId('app.bubble.scheduled'));
      act(() => {
        vi.advanceTimersByTime(CARD_GRACE_MS - 1);
      });
      expect(v.getByTestId('app.bubble.scheduled.card')).toBeTruthy();
      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(v.queryByTestId('app.bubble.scheduled.card')).toBeNull();
      expect(v.getByTestId('app.bubble.scheduled.card.out')).toBeTruthy();
      act(() => {
        vi.advanceTimersByTime(CARD_OUT_MS);
      });
      expect(v.queryByTestId('app.bubble.scheduled.card.out')).toBeNull();
      // warm: the next coin's card at once, no delay (designer m_46248)
      fireEvent.pointerEnter(v.getByTestId('app.bubble.artifacts'));
      expect(v.getByTestId('app.bubble.artifacts.card').querySelector('.b-artchip')).toBeTruthy();
      // cold again after 1 s
      fireEvent.pointerLeave(v.getByTestId('app.bubble.artifacts'));
      act(() => {
        vi.advanceTimersByTime(CARD_GRACE_MS + CARD_WARM_MS + CARD_OUT_MS);
      });
      fireEvent.pointerEnter(v.getByTestId('app.bubble.artifacts'));
      expect(v.queryByTestId('app.bubble.artifacts.card')).toBeNull();
      act(() => {
        vi.advanceTimersByTime(CARD_HOVER_MS);
      });
      expect(v.getByTestId('app.bubble.artifacts.card')).toBeTruthy();
      fireEvent.click(v.getByTestId('app.bubble.artifacts'));
      expect(v.getByTestId('app.flyout.artifacts')).toBeTruthy();
      expect(v.container.querySelector('.s3-tcard')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('a card opened by its key: its first row focused, ↓ ↑ walk the rows (kept in the card), ⏎ is the row\'s own, esc closes and gives the focus back to its bubble', () => {
    const { v } = side3d();
    act(() => {
      emitAction('side-1');
    });
    const rows = [...v.getByTestId('app.flyout.agents').querySelectorAll<HTMLElement>('.s3-card-body [data-row]')];
    // Overview's rows: main, then the worktree perf and docs share (its ψ head), then theirs
    expect(rows.map((r) => r.getAttribute('data-testid') ?? r.parentElement?.getAttribute('data-testid'))).toEqual([`app.flyout.agents.${ACME}.main`, 'app.flyout.agents.group.sb/dark', `app.flyout.agents.${ACME}.perf`, `app.flyout.agents.${ACME}.docs`]);
    expect(document.activeElement).toBe(rows[0]);
    // its ring at once, not only after the first ↓ (ui-review m_44435)
    expect(v.getByTestId('app.flyout.agents').hasAttribute('data-kb')).toBe(true);
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(rows[1]);
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    // kept in the card: the last row stays
    expect(document.activeElement).toBe(rows[3]);
    fireEvent.keyDown(window, { key: 'ArrowUp' });
    expect(document.activeElement).toBe(rows[2]);
    expect(rows[2]?.tagName).toBe('BUTTON');
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(v.queryByTestId('app.flyout.agents')).toBeNull();
    expect(document.activeElement).toBe(v.getByTestId('app.bubble.agents'));
  });

  it('a running process says \'∿\' in text-3, as the processes page, never a green dot (designer m_44499)', () => {
    const { v, store } = side3d();
    act(() => {
      store.apply(ev('processes', { project: ACME }));
    });
    fireEvent.click(v.getByTestId('app.bubble.processes'));
    const card = v.getByTestId('app.flyout.processes');
    expect([...card.querySelectorAll('.s3-wave')].map((e) => e.textContent)).toContain('∿');
    expect(card.querySelector('.s3-dot')).toBeNull();
  });

  it('1100 wide: the world folds into a 5th bubble on top of the stack', () => {
    const { v } = side3d(1100);
    expect(v.queryByTestId('app.world')).toBeNull();
    expect(v.getByTestId('app.bubble.world').style.top).toBe('40px');
    expect(v.getByTestId('app.bubble.agents').style.top).toBe('120px');
  });

  it('an app page (the inbox): no world, no bubbles, no band kept for them', () => {
    const { v, router } = side3d();
    act(() => {
      router.go({ screen: 'inbox' });
    });
    expect(v.queryByTestId('app.side')).toBeNull();
    expect(sideLayoutNow()).toMatchObject({ side: false, gutter: 0 });
    act(() => {
      router.go({ screen: 'thread', project: ACME, agent: 'perf' });
    });
    expect(v.getByTestId('app.bubble.world')).toBeTruthy();
  });

  it('the panel open beside the thread with room: the coins between, the panel\'s left edge their right, the world a 5th coin (folded-side v9)', () => {
    panel.open = true;
    const { v } = side3d(1440);
    const s = v.getByTestId('app.side');
    expect(s.getAttribute('data-at')).toBe('between');
    expect(s.style.right).toBe('384px');
    expect(v.getByTestId('app.bubble.world')).toBeTruthy();
    expect(v.queryByTestId('app.world')).toBeNull();
    expect(v.getByTestId('app.bubble.agents')).toBeTruthy();
  });

  it('no room beside the thread: the coins are minis in the header\'s slot, the bubbles gone; a click on one opens its float under the header, a second closes it; ⌥ shows their keys, never the cards', () => {
    vi.useFakeTimers();
    try {
      panel.open = true;
      panel.width = 900;
      const { v } = side3d(1440);
      const slot = render(<CoinSlot />);
      expect(v.getByTestId('app.side').getAttribute('data-at')).toBe('header');
      expect(v.queryByTestId('app.bubble.agents')).toBeNull();
      const minis = slot.getByTestId('app.coins.header');
      expect([...minis.querySelectorAll('[data-testid^="app.mini."][aria-haspopup]')].map((e) => e.getAttribute('data-testid'))).toEqual(['app.mini.agents', 'app.mini.artifacts', 'app.mini.scheduled', 'app.mini.processes']);
      expect(slot.getByTestId('app.mini.agents.n').textContent).toBe('2');
      expect(slot.getByTestId('app.mini.agents.badge').textContent).toBe('1');
      fireEvent.click(slot.getByTestId('app.mini.agents'));
      expect(v.getByTestId('app.flyout.agents')).toBeTruthy();
      expect(slot.getByTestId('app.mini.agents').hasAttribute('data-open')).toBe(true);
      fireEvent.pointerDown(slot.getByTestId('app.mini.agents'));
      fireEvent.click(slot.getByTestId('app.mini.agents'));
      expect(v.queryByTestId('app.flyout.agents')).toBeNull();
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Alt', altKey: true }));
        vi.advanceTimersByTime(HINT_HOLD_MS);
      });
      act(() => {
        vi.advanceTimersByTime(CARD_HOLD_MS);
      });
      expect(slot.getByTestId('app.mini.scheduled.n').textContent).toBe('⌥⌘3');
      expect(document.querySelector('.s3-tcard')).toBeNull();
      act(() => {
        window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Alt' }));
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it('a sheet over the thread: the minis stay in the title bar after the crumb, never in its tab row (his m_45864)', () => {
    panel.open = true;
    panel.over = true;
    const { v } = side3d(1000);
    const slot = render(<CoinSlot />);
    expect(v.getByTestId('app.side').getAttribute('data-at')).toBe('tabs');
    expect(slot.getByTestId('app.coins.header').querySelector('[data-testid="app.mini.agents"]')).toBeTruthy();
  });

  it('the world takes the pointer on its island only: grown, never on the bubbles\' column', () => {
    expect(hitBox(280, false)).toEqual({ width: 148, height: 108 });
    const big = hitBox(280, true);
    expect(big).toMatchObject({ width: 384, height: 280 });
    expect(big.clipPath).toContain('calc(100% - 68px) 112px');
  });

  it('a drop of files on the artifacts coin adds them to the project, as on the list (folded-side v9)', () => {
    const { v, store } = side3d();
    const sent = vi.spyOn(store, 'command');
    const bise = (window as unknown as { bise: Record<string, unknown> }).bise;
    bise['filePath'] = (f: File) => `/tmp/${f.name}`;
    try {
      const file = new File(['# notes'], 'notes.md', { type: 'text/markdown' });
      const dataTransfer = { types: ['Files'], files: [file], dropEffect: 'none' };
      fireEvent.dragOver(v.getByTestId('app.bubble.artifacts'), { dataTransfer });
      fireEvent.drop(v.getByTestId('app.bubble.artifacts'), { dataTransfer });
      expect(sent).toHaveBeenCalledWith(expect.objectContaining({ cmd: 'artifacts_add', project: ACME, target: '/tmp/notes.md' }));
      // the other coins take no drop
      sent.mockClear();
      fireEvent.drop(v.getByTestId('app.bubble.processes'), { dataTransfer });
      expect(sent).not.toHaveBeenCalledWith(expect.objectContaining({ cmd: 'artifacts_add' }));
    } finally {
      delete bise['filePath'];
    }
  });

  it('the agents card: one foot, the card\'s end (the archived fold · all agents)', () => {
    const { v } = side3d();
    fireEvent.click(v.getByTestId('app.bubble.agents'));
    const card = v.getByTestId('app.flyout.agents');
    expect(card.querySelectorAll('footer')).toHaveLength(1);
    expect(card.querySelector('.s3-card-layer[data-now]')?.lastElementChild?.tagName).toBe('FOOTER');
    expect(card.hasAttribute('data-foot')).toBe(true);
    expect(card.querySelector('footer')?.textContent).toBe(`▸${side.archived({ n: 1 })}${side.all_agents}`);
  });

  it('a card\'s top is always its bubble\'s; its height at most the room under it less 16 (designer m_44499)', () => {
    Object.defineProperty(window, 'innerHeight', { value: 900, configurable: true });
    const { v } = side3d();
    fireEvent.click(v.getByTestId('app.bubble.processes'));
    const card = v.getByTestId('app.flyout.processes');
    expect(card.style.top).toBe(v.getByTestId('app.bubble.processes').style.top);
    // happy-dom puts the side at 0: 900 - 16 - the processes coin's top (the world a 5th coin)
    expect(card.style.maxHeight).toBe(`${String(cardMaxH(0, bubbleTops('bubble').bubbles.processes, 900))}px`);
    fireEvent.click(v.getByTestId('app.bubble.agents'));
    expect(v.getByTestId('app.flyout.agents').style.maxHeight).toBe(`${String(CARD_MAX)}px`);
  });

  it('a card open stays put: its own resize with nothing changed makes no new render (the jitter, Gabriel m_44190)', () => {
    Object.defineProperty(window, 'innerHeight', { value: 900, configurable: true });
    const seen: ResizeObserverCallback[] = [];
    vi.stubGlobal('ResizeObserver', class {
      constructor(cb: ResizeObserverCallback) {
        seen.push(cb);
      }
      observe() {}
      disconnect() {}
      unobserve() {}
    });
    try {
      let commits = 0;
      const { v } = side3d(1100, () => {
        commits++;
      });
      fireEvent.click(v.getByTestId('app.bubble.processes'));
      const card = v.getByTestId('app.flyout.processes');
      const box = [card.style.top, card.style.maxHeight];
      const after = commits;
      act(() => {
        for (let i = 0; i < 5; i++) for (const cb of seen) cb([], {} as ResizeObserver);
      });
      expect(commits).toBe(after);
      expect([card.style.top, card.style.maxHeight]).toEqual(box);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('a beat\'s label sits over its bim\'s head on the whole folded map, scaled 148 / 384; none found, over the island\'s middle', () => {
    expect(beatPlace({ x: 192, y: 96 }, 300)).toEqual({ x: 74, y: 37 });
    expect(beatPlace(null, 384)).toEqual({ x: 74, y: 74 });
  });

  it('the bubbles under the world at 152, 80 apart, put whatever the world does; as a 5th bubble the world tops the stack', () => {
    expect(bubbleTops('small').bubbles).toEqual({ agents: 152, artifacts: 232, scheduled: 312, processes: 392 });
    expect(bubbleTops('bubble')).toEqual({ world: 40, bubbles: { agents: 120, artifacts: 200, scheduled: 280, processes: 360 } });
  });
});

describe('the outer card (page outer-side v2, his pick)', () => {
  it('the panel folded at 1440: the card, no coins; agents in the hub\'s rank, one line each; the asking one with its age in the accent', () => {
    const { v } = side3d(1440);
    expect(v.getByTestId('app.outer')).toBeTruthy();
    expect(v.queryByTestId('app.bubble.agents')).toBeNull();
    const rows = [...v.getByTestId('app.outer.agents.rows').querySelectorAll('.s3-orow')].map((e) => e.getAttribute('data-testid'));
    expect(rows).toEqual([`app.outer.agents.${ACME}.main`, `app.outer.agents.${ACME}.perf`, `app.outer.agents.${ACME}.docs`]);
    expect(v.getByTestId(`app.outer.agents.${ACME}.perf`).hasAttribute('data-asks')).toBe(true);
    expect(v.getByTestId(`app.outer.agents.${ACME}.perf.age`)).toBeTruthy();
    // at rest the line never grows: no question under it until a click
    expect(v.queryByTestId(`app.outer.agents.${ACME}.perf.ask`)).toBeNull();
    expect(v.getByTestId('app.outer.agents.loud').textContent).toContain(side.head_waits({ n: 1 }));
    expect(v.getByTestId('app.outer.artifacts')).toBeTruthy();
    // nothing scheduled, no process: no section (outer-empty.test.tsx)
    for (const s of ['scheduled', 'processes']) expect(v.queryByTestId(`app.outer.${s}`)).toBeNull();
  });

  it('a click on an asking line opens its ask in place, its options one click; on another line it opens that thread', () => {
    const { v, does, router } = side3d(1440);
    fireEvent.click(v.getByTestId(`app.outer.agents.${ACME}.perf`).querySelector('[data-row]') as HTMLElement);
    expect(v.getByTestId(`app.outer.agents.${ACME}.perf.ask`).textContent).toContain('run rm -rf target?');
    fireEvent.click(v.getByTestId(`app.outer.agents.${ACME}.perf.opt.1`));
    expect(does).toHaveBeenCalledWith(expect.objectContaining({ i: 'pick', project: ACME, card: 7, n: 1 }));
    fireEvent.click(v.getByTestId(`app.outer.agents.${ACME}.docs`).querySelector('[data-row]') as HTMLElement);
    expect(router.get()).toMatchObject({ screen: 'thread', project: ACME, agent: 'docs' });
  });

  it('a panel open gives way: the card goes, the coins come back between the thread and the panel', () => {
    panel.open = true;
    const { v } = side3d(1440);
    expect(v.queryByTestId('app.outer')).toBeNull();
    expect(v.getByTestId('app.side').getAttribute('data-at')).toBe('between');
  });
});

describe('the bims\' tile over the outer card (page outer-world v2, his pick #799)', () => {
  const tall = (h: number) => {
    Object.defineProperty(window, 'innerHeight', { value: h, configurable: true });
  };

  it('at 1440 the tile shows over the card at 140, the project\'s office in it; the card starts under it and its 8 handle', () => {
    tall(900);
    const { v } = side3d(1440);
    const tile = v.getByTestId('app.outer.world');
    expect(tile.style.height).toBe('140px');
    expect(tile.querySelector('[data-testid="office"]')).toBeTruthy();
    // 12 under the bar's 52, as at the window's right and bottom (his m_46677: it sat on the bar)
    expect(tile.style.top).toBe('64px');
    // 292 wide, the card and the handle too (his m_47018, designer m_47025)
    expect(tile.style.width).toBe('292px');
    expect(v.getByTestId('app.outer').style.width).toBe('292px');
    expect(v.getByTestId('app.outer.world.handle').style.width).toBe('292px');
    // the card under the tile and its 8 handle
    expect(v.getByTestId('app.outer').style.top).toBe('212px');
    expect(v.getByTestId('app.outer.world.handle').getAttribute('aria-valuenow')).toBe('140');
    // the handle sits in the 8 gap under the tile (ui-review m_46538: it sat over the tile's top)
    expect(v.getByTestId('app.outer.world.handle').style.top).toBe('204px');
    expect(v.getByTestId('app.outer.world.handle').style.height).toBe('8px');
    // shown: no cube in the Agents head
    expect(v.queryByTestId('app.outer.agents.cube')).toBeNull();
  });

  it('on bise\'s own the tile holds the town, every project\'s office side by side; a project\'s thread its office again (his m_47415: lost in 6d9d07180)', () => {
    tall(900);
    const { v, router } = side3d(1440);
    // acme's thread: acme's office, no town
    expect(v.queryByTestId('town')).toBeNull();
    act(() => {
      router.go({ screen: 'thread', project: HOME, agent: 'main' });
    });
    const tile = v.getByTestId('app.outer.world');
    expect(tile.hasAttribute('data-town')).toBe(true);
    expect(tile.getAttribute('aria-label')).toBe(bims.town_label);
    expect(tile.querySelector('[data-testid="office"]')).toBeNull();
    // both projects in it, fitted to the tile's height
    expect(v.getByTestId(`town.${HOME}`)).toBeTruthy();
    expect(v.getByTestId(`town.${ACME}`)).toBeTruthy();
    expect(v.getByTestId('town').getAttribute('data-max-h')).toBe('140');
    // no '⤢': the panel has no town tab
    expect(v.queryByTestId('app.outer.world.panel')).toBeNull();
    // the dived office's 'open': that project's main thread; a bim in it: its agent's
    fireEvent.click(v.getByTestId(`town.${ACME}`));
    expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'main' });
    expect(v.queryByTestId('town')).toBeNull();
    act(() => {
      router.go({ screen: 'bise' });
    });
    fireEvent.doubleClick(v.getByTestId(`town.${ACME}`));
    expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'perf' });
  });

  it('× hides it: the card takes the full height, a cube at the right of the Agents head brings it back; both kept', () => {
    tall(900);
    const { v } = side3d(1440);
    fireEvent.click(v.getByTestId('app.outer.world.hide'));
    expect(v.queryByTestId('app.outer.world')).toBeNull();
    expect(v.queryByTestId('app.outer.world.handle')).toBeNull();
    // the card alone keeps the same 12 under the bar
    expect(v.getByTestId('app.outer').style.top).toBe('64px');
    const cube = v.getByTestId('app.outer.agents.cube');
    expect(cube.getAttribute('aria-label')).toBe(side.tile_show);
    expect(outerWorld.get().hidden).toBe(true);
    fireEvent.click(cube);
    expect(v.getByTestId('app.outer.world')).toBeTruthy();
    expect(v.queryByTestId('app.outer.agents.cube')).toBeNull();
    expect(outerWorld.get().hidden).toBe(false);
  });

  it('View › Show the Bims brings a hidden tile back, and Hide hides it', () => {
    tall(900);
    outerWorld.hide(true);
    const { v } = side3d(1440);
    expect(v.queryByTestId('app.outer.world')).toBeNull();
    act(() => {
      outerWorld.hide(!outerWorld.get().hidden);
    });
    expect(v.getByTestId('app.outer.world')).toBeTruthy();
  });

  it('the handle: a drag sets the height between 96 and half the window, a double-click puts 140 back, the height is kept', () => {
    tall(900);
    const { v } = side3d(1440);
    const handle = v.getByTestId('app.outer.world.handle');
    fireEvent.pointerDown(handle, { clientY: 200, button: 0, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientY: 296, pointerId: 1 });
    expect(v.getByTestId('app.outer.world').style.height).toBe('236px');
    expect(v.getByTestId('app.outer.world').style.top).toBe('64px');
    expect(v.getByTestId('app.outer').style.top).toBe('308px');
    expect(handle.style.top).toBe('300px');
    // past half the window: 450
    fireEvent.pointerMove(handle, { clientY: 900, pointerId: 1 });
    expect(v.getByTestId('app.outer.world').style.height).toBe('450px');
    // under 96: 96
    fireEvent.pointerMove(handle, { clientY: -400, pointerId: 1 });
    expect(v.getByTestId('app.outer.world').style.height).toBe(`${String(TILE_MIN)}px`);
    fireEvent.pointerUp(handle, { pointerId: 1 });
    // a move after the release does nothing
    fireEvent.pointerMove(handle, { clientY: 600, pointerId: 1 });
    expect(outerWorld.get().h).toBe(TILE_MIN);
    fireEvent.doubleClick(handle);
    expect(v.getByTestId('app.outer.world').style.height).toBe('140px');
  });

  it('a new project\'s welcome in main\'s thread: the tile keeps its frame and place, empty (one office on screen, native-lead m_47921; designer m_47929); the welcome gone, its office mounts there', () => {
    tall(900);
    const gone = welcoming.add(ACME);
    try {
      const { v } = side3d(1440);
      const tile = v.getByTestId('app.outer.world');
      expect(tile.querySelector('[data-testid="office"]')).toBeNull();
      expect(tile.hasAttribute('data-waiting')).toBe(true);
      // no words in the empty frame: no ⤢ nor ×
      expect(v.queryByTestId('app.outer.world.panel')).toBeNull();
      expect(v.queryByTestId('app.outer.world.hide')).toBeNull();
      // the frame where the office will be: no jump of the card
      expect(tile.style.height).toBe('140px');
      expect(v.getByTestId('app.outer').style.top).toBe('212px');
      act(() => {
        gone();
      });
      expect(tile.querySelector('[data-testid="office"]')).toBeTruthy();
      expect(tile.hasAttribute('data-waiting')).toBe(false);
    } finally {
      if (welcoming.has(ACME)) gone();
    }
  });

  it('the welcome docking: the tile\'s office mounts at its fit under the flying copy, its chrome held; landed, the camera eases to the open thread\'s bim and the chrome comes (designer m_47950)', () => {
    tall(900);
    const gone = welcoming.add(ACME);
    const landed = welcoming.dock(ACME);
    try {
      const { v } = side3d(1440);
      const tile = v.getByTestId('app.outer.world');
      expect(tile.querySelector('[data-testid="office"]')).toBeNull();
      act(() => {
        gone();
      });
      // in flight: the office drawn whole, no follow, no data-landed yet
      expect(tile.querySelector('[data-testid="office"]')).toBeTruthy();
      expect(office.follow.at(-1)).toBeNull();
      expect(tile.hasAttribute('data-landing')).toBe(true);
      expect(tile.hasAttribute('data-landed')).toBe(false);
      vi.useFakeTimers();
      act(() => {
        landed();
      });
      expect(tile.hasAttribute('data-landing')).toBe(false);
      expect(tile.hasAttribute('data-landed')).toBe(true);
      expect(v.getByTestId('app.outer.world.hide')).toBeTruthy();
      // one fixed beat after the draw, then the camera finds the bim (designer m_47972)
      act(() => {
        vi.advanceTimersByTime(FOLLOW_BEAT_MS - 1);
      });
      expect(office.follow.at(-1)).toBeNull();
      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(office.follow.at(-1)).toBe('perf');
    } finally {
      vi.useRealTimers();
      gone();
      landed();
    }
  });

  it('a welcome in another project leaves this project\'s tile live', () => {
    tall(900);
    const gone = welcoming.add(HOME);
    try {
      const { v } = side3d(1440);
      expect(v.getByTestId('app.outer.world').querySelector('[data-testid="office"]')).toBeTruthy();
    } finally {
      gone();
    }
  });

  it('a kept height taller than half a smaller window shows at half of it', () => {
    tall(900);
    outerWorld.height(400, 900);
    tall(600);
    const { v } = side3d(1440);
    expect(v.getByTestId('app.outer.world').style.height).toBe('300px');
    expect(outerWorld.get().h).toBe(400);
  });

  it('⤢ opens the world in the panel\'s Office tab; a click on a bim opens its thread', () => {
    tall(900);
    const { v } = side3d(1440);
    expect(v.getByTestId('app.outer.world.panel').getAttribute('aria-label')).toBe(side.tile_panel);
    fireEvent.click(v.getByTestId('app.outer.world.panel'));
    expect(opened).toEqual([{ kind: 'office', project: ACME }]);
  });

  it('the open agent\'s bim is followed (his m_47261): another agent picked, the tile goes to it; main\'s thread, main\'s bim (his m_47407)', () => {
    tall(900);
    const { router } = side3d(1440);
    expect(office.follow.at(-1)).toBe('perf');
    act(() => {
      router.go({ screen: 'thread', project: ACME, agent: 'docs' });
    });
    expect(office.follow.at(-1)).toBe('docs');
    act(() => {
      router.go({ screen: 'thread', project: ACME, agent: 'main' });
    });
    expect(office.follow.at(-1)).toBe('main');
  });

  it('an ask opened in place in the card: the tile looks at its bim while it is open, then back to the thread\'s (designer m_47300)', () => {
    tall(900);
    const { v, router } = side3d(1440);
    act(() => {
      router.go({ screen: 'thread', project: ACME, agent: 'docs' });
    });
    const perf = v.getByTestId(`app.outer.agents.${ACME}.perf`).querySelector<HTMLElement>('[data-row]');
    if (perf === null) throw new Error('no perf row');
    fireEvent.click(perf);
    expect(v.getByTestId(`app.outer.agents.${ACME}.perf.ask`)).toBeTruthy();
    expect(office.follow.at(-1)).toBe('perf');
    expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'docs' });
    fireEvent.click(perf);
    expect(office.follow.at(-1)).toBe('docs');
    // esc folds it too (app/escape.ts row 4, designer m_47321): the focus back on its row, the route stays
    fireEvent.click(perf);
    expect(office.follow.at(-1)).toBe('perf');
    fireEvent.keyDown(window, { key: 'Escape', code: 'Escape' });
    expect(v.queryByTestId(`app.outer.agents.${ACME}.perf.ask`)).toBeNull();
    expect(document.activeElement).toBe(perf);
    expect(office.follow.at(-1)).toBe('docs');
    expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'docs' });
  });

  it('a bim hovered tints its row in the card, a row hovered lights its bim (bims/focus, his m_47261)', () => {
    tall(900);
    const { v } = side3d(1440);
    const row = v.getByTestId(`app.outer.agents.${ACME}.docs`);
    expect(row.hasAttribute('data-lit')).toBe(false);
    act(() => {
      bimsLit.set({ name: 'docs', from: 'bim' });
    });
    expect(row.hasAttribute('data-lit')).toBe(true);
    act(() => {
      bimsLit.set(null, 'bim');
    });
    expect(row.hasAttribute('data-lit')).toBe(false);
    fireEvent.pointerEnter(row);
    expect(bimsLit.get()).toEqual({ name: 'docs', from: 'row' });
    // the row lit from itself takes no tint of its own: its hover already shows
    expect(row.hasAttribute('data-lit')).toBe(false);
    fireEvent.pointerLeave(row);
    expect(bimsLit.get()).toBeNull();
  });

  it('a tall tile folds nothing: each section keeps its rows, the card scrolls as a whole (designer m_46693)', () => {
    tall(800);
    outerWorld.height(400, 800);
    const { v } = side3d(1440);
    expect(v.getByTestId('app.outer.artifacts').querySelectorAll('.s3-row').length).toBe(2);
    expect(v.queryByTestId('app.outer.processes')).toBeNull();
    expect(v.queryByTestId('app.outer.artifacts.more')).toBeNull();
  });
});

describe('the outer card\'s rows: Agents 10, the others 5, then \'+ N more\' adds 50 (his m_46685, m_46687)', () => {
  const tall = (h: number) => {
    Object.defineProperty(window, 'innerHeight', { value: h, configurable: true });
  };
  const crowd = (store: Store, agents: number, arts: number) => {
    act(() => {
      // the thread in view is perf's: the card's artifacts are its own
      store.apply(ev('agents', { project: ACME, agents: [agent('main', { main: true }), agent('perf', { status: 'working', rank: 1 }), ...Array.from({ length: agents - 2 }, (_, i) => agent(`a${String(i)}`, { status: 'working', rank: i + 2 }))] }));
      store.apply(artifacts(ACME, ...Array.from({ length: arts }, (_, i) => artifact({ id: `x${String(i)}`, kind: 'page', title: `page ${String(i)}`, agent: 'perf', new: false }))));
    });
  };
  const lines = (v: ReturnType<typeof render>, id: string) => v.getByTestId(`app.outer.${id}`).querySelectorAll(id === 'agents' ? '.s3-orow' : '.s3-row').length;

  it('Agents shows its first 10 then \'+ 3 more\'; a click shows all 13 and the row goes', () => {
    tall(900);
    const { v, store } = side3d(1440);
    crowd(store, 13, 0);
    expect(lines(v, 'agents')).toBe(10);
    expect(v.getByTestId('app.outer.agents.more').textContent).toBe(side.more_rows({ n: 3 }));
    expect(v.queryByTestId('app.outer.agents.less')).toBeNull();
    fireEvent.click(v.getByTestId('app.outer.agents.more'));
    expect(lines(v, 'agents')).toBe(13);
    expect(v.queryByTestId('app.outer.agents.more')).toBeNull();
    // all shown: 'show less' alone, back to the first 10
    expect(v.getByTestId('app.outer.agents.less').textContent).toBe(side.show_less);
    fireEvent.click(v.getByTestId('app.outer.agents.less'));
    expect(lines(v, 'agents')).toBe(10);
    expect(v.queryByTestId('app.outer.agents.less')).toBeNull();
  });

  it('\'show less\' folds 50 back step by step to the first 5 (his m_47005)', () => {
    tall(900);
    const { v, store } = side3d(1440);
    crowd(store, 3, 120);
    fireEvent.click(v.getByTestId('app.outer.artifacts.more'));
    fireEvent.click(v.getByTestId('app.outer.artifacts.more'));
    expect(lines(v, 'artifacts')).toBe(105);
    fireEvent.click(v.getByTestId('app.outer.artifacts.less'));
    expect(lines(v, 'artifacts')).toBe(55);
    expect(v.getByTestId('app.outer.artifacts.more').textContent).toBe(side.more_rows({ n: 50 }));
    fireEvent.click(v.getByTestId('app.outer.artifacts.less'));
    expect(lines(v, 'artifacts')).toBe(5);
    expect(v.queryByTestId('app.outer.artifacts.less')).toBeNull();
  });

  it('Artifacts with 62: its first 5, \'+ 50 more\', then 55 and \'+ 7 more\', then all 62', () => {
    tall(900);
    const { v, store } = side3d(1440);
    crowd(store, 3, 62);
    expect(lines(v, 'artifacts')).toBe(5);
    expect(v.getByTestId('app.outer.artifacts.more').textContent).toBe(side.more_rows({ n: 50 }));
    fireEvent.click(v.getByTestId('app.outer.artifacts.more'));
    expect(lines(v, 'artifacts')).toBe(55);
    expect(v.getByTestId('app.outer.artifacts.more').textContent).toBe(side.more_rows({ n: 7 }));
    fireEvent.click(v.getByTestId('app.outer.artifacts.more'));
    expect(lines(v, 'artifacts')).toBe(62);
    expect(v.queryByTestId('app.outer.artifacts.more')).toBeNull();
    // 'all ›' still opens the full screen
    expect(v.getByTestId('app.outer.artifacts.all')).toBeTruthy();
  });

  it('5 or fewer: no \'more\' row; none: no section', () => {
    tall(900);
    const { v, store } = side3d(1440);
    crowd(store, 3, 5);
    expect(lines(v, 'artifacts')).toBe(5);
    expect(v.queryByTestId('app.outer.artifacts.more')).toBeNull();
    expect(v.queryByTestId('app.outer.processes.more')).toBeNull();
    expect(v.queryByTestId('app.outer.processes')).toBeNull();
  });

  it('back to the first rows when the card remounts (a panel opened and folded again)', () => {
    tall(900);
    const { v, store } = side3d(1440);
    crowd(store, 3, 62);
    fireEvent.click(v.getByTestId('app.outer.artifacts.more'));
    expect(lines(v, 'artifacts')).toBe(55);
    cleanup();
    const again = side3d(1440);
    crowd(again.store, 3, 62);
    expect(lines(again.v, 'artifacts')).toBe(5);
  });

  it('a panel open: the tile and the card give way to the coins; folded again, both come back at the tile\'s height', () => {
    tall(900);
    outerWorld.height(200, 900);
    panel.open = true;
    const { v } = side3d(1440);
    expect(v.queryByTestId('app.outer.world')).toBeNull();
    cleanup();
    panel.open = false;
    const again = side3d(1440);
    expect(again.v.getByTestId('app.outer.world').style.height).toBe('200px');
  });
});

