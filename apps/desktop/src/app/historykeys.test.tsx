// @vitest-environment happy-dom
// The window's history as a browser's (history-keys, his m_58539 and m_58541; designer m_58552,
// architect m_58553): ⌘← back and ⌘→ forward from the page, never in a field (the caret's move to
// the line's start or end stays his), ⌘[ ⌘] still; and an artifact opened or closed in the panel
// is a step: back closes what opened and brings back what closed, forward redoes both; a tab
// brought to the front, or the tab an artifact's route opens, is no step.
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRouter, type Router } from './router';
import type { Route } from './route';
import { historyKeyOf, useHistoryWays } from './esc-up';
import { activateTab, bindPanelHistory, closeActive, closeTabId, panelState } from './panel/state';
import { openTab } from './panel/tabs';
import { FOLDED, tabOf, type PanelKept } from './panel/model';
import { parseSnap, parseStep, restoreKept, snapOf } from './panel/steps';
import type { Entity } from '@/shared/entity';

const P = 'acme-1a2b3c4d';
const A: Route = { screen: 'thread', project: P, agent: 'main' };
const B: Route = { screen: 'thread', project: P, agent: 'perf' };
const art = (id: string): Entity => ({ kind: 'artifact', project: P, id });

function Ways({ router }: { router: Router }) {
  useHistoryWays(router);
  return null;
}

afterEach(cleanup);
beforeEach(() => {
  panelState.reset();
});

/** A window at B with A behind it, its fields: a text input, the composer (an editable), a textarea. */
function page() {
  const router = createRouter(A);
  router.go(B);
  const v = render(
    <>
      <Ways router={router} />
      <input data-testid="search" defaultValue="perf notes" />
      <div data-testid="composer" contentEditable suppressContentEditableWarning>
        hello
      </div>
      <textarea data-testid="area" defaultValue="two words" />
      <button data-testid="row">row</button>
    </>,
  );
  return { router, v };
}

const press = (el: Element, key: string) => fireEvent.keyDown(el, { key, metaKey: true });

describe('⌘← and ⌘→: back and forward, as a browser', () => {
  it('from the page: ⌘← goes back, ⌘→ forward', () => {
    const { router, v } = page();
    const row = v.getByTestId('row');
    row.focus();
    expect(press(row, 'ArrowLeft'), 'taken: no scroll under it').toBe(false);
    expect(router.get()).toEqual(A);
    press(document.body, 'ArrowRight');
    expect(router.get()).toEqual(B);
  });

  it('in a field, the composer or a textarea: nothing to history, the caret move stays native', () => {
    const { router, v } = page();
    for (const id of ['search', 'composer', 'area']) {
      const el = v.getByTestId(id);
      el.focus();
      expect(document.activeElement, id).toBe(el);
      expect(press(el, 'ArrowLeft'), `${id}: ⌘← not prevented`).toBe(true);
      expect(router.get(), id).toEqual(B);
    }
    router.back();
    for (const id of ['search', 'composer', 'area']) {
      const el = v.getByTestId(id);
      el.focus();
      expect(press(el, 'ArrowRight'), `${id}: ⌘→ not prevented`).toBe(true);
      expect(router.get(), id).toEqual(A);
    }
  });

  it('⌘[ and ⌘] still go, a field focused or not', () => {
    const { router, v } = page();
    const field = v.getByTestId('search');
    field.focus();
    press(field, '[');
    expect(router.get()).toEqual(A);
    press(field, ']');
    expect(router.get()).toEqual(B);
  });

  it('historyKeyOf: other modifiers, a key taken below, a splitter: none', () => {
    const k = (key: string, o: Partial<{ altKey: boolean; shiftKey: boolean; ctrlKey: boolean; defaultPrevented: boolean; metaKey: boolean }> = {}) => ({ key, metaKey: true, altKey: false, ctrlKey: false, shiftKey: false, defaultPrevented: false, ...o });
    expect(historyKeyOf(k('ArrowLeft'), null)).toBe('back');
    expect(historyKeyOf(k('ArrowRight'), document.body)).toBe('forward');
    expect(historyKeyOf(k('ArrowLeft', { shiftKey: true }), null), '⇧⌘← selects').toBeUndefined();
    expect(historyKeyOf(k('ArrowLeft', { altKey: true }), null)).toBeUndefined();
    expect(historyKeyOf(k('ArrowLeft', { metaKey: false }), null)).toBeUndefined();
    expect(historyKeyOf(k('ArrowLeft', { defaultPrevented: true }), null)).toBeUndefined();
    const sep = document.createElement('div');
    sep.setAttribute('role', 'separator');
    expect(historyKeyOf(k('ArrowLeft'), sep)).toBeUndefined();
    expect(historyKeyOf(k('[', { defaultPrevented: true }), sep), 'the old key acts anywhere').toBe('back');
  });
});

describe('an artifact opened or closed in the panel is a step of the history', () => {
  /** The window's router at A, the panel bound to it. */
  function bound(exists: (e: Entity) => boolean = () => true) {
    const router = createRouter(A);
    const off = bindPanelHistory(router, exists);
    return { router, off };
  }
  const front = () => {
    const s = panelState.get();
    return s.open ? s.tabs.active : 'folded';
  };
  const ids = () => panelState.get().tabs.list.map((t) => t.id);

  it('back closes what opened (folded again), forward opens it again', () => {
    const { router, off } = bound();
    openTab(art('notes'));
    expect(front()).toBe('artifact:acme-1a2b3c4d/notes');
    expect(router.canBack()).toBe(true);
    router.back();
    expect(front()).toBe('folded');
    expect(ids()).toEqual([]);
    router.forward();
    expect(front()).toBe('artifact:acme-1a2b3c4d/notes');
    off();
  });

  it('back over a close brings it back in front, forward closes it again; the tab before comes to the front', () => {
    const { router, off } = bound();
    openTab(art('notes'));
    openTab(art('mocks'));
    closeActive();
    expect(front()).toBe('artifact:acme-1a2b3c4d/notes');
    router.back();
    expect(front()).toBe('artifact:acme-1a2b3c4d/mocks');
    expect(ids()).toEqual(['artifact:acme-1a2b3c4d/notes', 'artifact:acme-1a2b3c4d/mocks']);
    router.forward();
    expect(ids()).toEqual(['artifact:acme-1a2b3c4d/notes']);
    router.back();
    router.back();
    expect(front(), 'back over the second open: the first in front').toBe('artifact:acme-1a2b3c4d/notes');
    off();
  });

  it('a tab brought to the front is no step, the step remembers it', () => {
    const { router, off } = bound();
    openTab(art('notes'));
    openTab(art('mocks'));
    activateTab('artifact:acme-1a2b3c4d/notes');
    openTab(art('mocks'));
    expect(router.places('back', 10)).toHaveLength(2);
    router.back();
    router.forward();
    expect(front(), 'the current step kept the front he chose').toBe('artifact:acme-1a2b3c4d/mocks');
    off();
  });

  it("an artifact's route opens its tab quietly: the route is the step; a launcher or a closed launcher is none", () => {
    const { router, off } = bound();
    openTab(art('notes'), { quiet: true });
    expect(router.canBack()).toBe(false);
    off();
  });

  it('a route step between keeps the panel; back over it restores nothing, and steps a page apart come back', () => {
    const { router, off } = bound();
    openTab(art('notes'));
    router.go(B);
    closeTabId('artifact:acme-1a2b3c4d/notes');
    router.back();
    expect(front()).toBe('artifact:acme-1a2b3c4d/notes');
    router.back();
    expect(router.get()).toEqual(A);
    expect(front()).toBe('artifact:acme-1a2b3c4d/notes');
    router.back();
    expect(front()).toBe('folded');
    off();
  });

  it('a step whose artifact is gone since never brings its tab back', () => {
    const { router, off } = bound((e) => !(e.kind === 'artifact' && e.id === 'notes'));
    openTab(art('notes'));
    closeActive();
    router.back();
    expect(ids()).toEqual([]);
    off();
  });
});

describe('the router: a marked step', () => {
  it('back over it calls onRestore once with the step before, never marks; the session keeps both', () => {
    const r = createRouter(A);
    const restore = vi.fn();
    r.onRestore(restore);
    r.mark('before', 'after', { did: 'open' });
    expect(r.get()).toEqual(A);
    expect(r.steps('back', 10)).toEqual([{ route: A }]);
    r.back();
    expect(restore.mock.calls).toEqual([['before']]);
    expect(r.canBack()).toBe(false);
    r.forward();
    expect(restore.mock.calls).toEqual([['before'], ['after']]);
    const again = createRouter(A, JSON.parse(JSON.stringify(r.saved())) as ReturnType<Router['saved']>);
    expect(again.steps('back', 10)).toEqual([{ route: A }]);
    again.onRestore(restore);
    again.back();
    expect(restore).toHaveBeenLastCalledWith('before');
  });

  it('a route step holds no panel: going to it calls nothing', () => {
    const r = createRouter(A);
    const restore = vi.fn();
    r.onRestore(restore);
    r.go(B);
    r.back();
    expect(restore).not.toHaveBeenCalled();
  });
});

describe('steps.ts: a step read back and the panel it gives', () => {
  const withTabs = (ids: readonly string[], active: string, open = true): PanelKept => ({ ...FOLDED, open, tabs: { list: ids.map((id) => tabOf({ kind: 'thing', e: art(id) })), active: active === '' ? '' : `artifact:${P}/${active}` } });

  it('idempotent; an X closed by hand in between comes back, a shell stays', () => {
    const was = withTabs(['a', 'b'], 'b');
    const snap = snapOf(P, was);
    const now0: PanelKept = { ...withTabs(['a'], 'a'), tabs: { list: [...withTabs(['a'], 'a').tabs.list, tabOf({ kind: 'terminal', project: P, n: 0 })], active: `term:${P}#0` } };
    const once = restoreKept(now0, snap);
    expect(once.tabs.list.map((t) => t.id)).toEqual([`artifact:${P}/a`, `artifact:${P}/b`, `term:${P}#0`]);
    expect(once.tabs.active).toBe(`artifact:${P}/b`);
    expect(restoreKept(once, snap)).toEqual(once);
  });

  it('a bad session restores nothing', () => {
    expect(parseSnap({ project: P, open: true, things: [{ e: { kind: 'tab', id: 'x' }, at: 0 }], active: '' })).toBeUndefined();
    expect(parseSnap('nope')).toBeUndefined();
    expect(parseStep({ did: 'open', e: art('a') })).toEqual({ did: 'open', e: art('a') });
    expect(parseStep({ did: 'moved', e: art('a') })).toBeUndefined();
    const snap = snapOf(P, withTabs(['a'], 'a'));
    expect(parseSnap(JSON.parse(JSON.stringify(snap)))).toEqual(snap);
  });
});
