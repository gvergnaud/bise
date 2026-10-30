// @vitest-environment happy-dom
// Pin from the tab (tab-pin, his m_49983: « pin un artifact depuis la tab dans laquelle il est
// ouvert, et pas seulement depuis la sidebar de droite »; his m_50763: in the tab, left of its ×,
// on hover; no 'open in a window'): a tab's pin writes the side's own prefs, so a pin made in the tab shows on the side's row at once and the other
// way round; the tab's menu has Pin/Unpin too. A fake core: its prefs_set echoes as the hub's prefs.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { CoreCmd, CorePort } from '@/shared/api';
import type { JsonValue, Proc } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { menus } from '@/proto/words_menus.gen';
import { artifact, artifacts, ev, projects, row } from '../../../tests/feature/inputs';
import { createRouter, RouterContext } from '@/app/router';
import { PinButton } from '@/app/pinbutton';
import { pinnedIn, pinRefOf, procPinId, usePins, type PinKind } from '@/app/pins';
import { actsOf, menuOf } from '@/app/menus/registry';
import { runAct, type MenuFx } from '@/app/menus/run';
import { factsOf } from '@/app/menus/use';
import { resetHolds } from '@/app/pinhold';
import { PanelHost } from './Host';
import { panelState } from './state';
import { openTab } from './tabs';
import { tabPinOf } from './TabPin';

const ACME = 'acme-1a2b3c4d';

beforeEach(() => {
  panelState.reset();
  Object.defineProperty(window, 'innerWidth', { value: 1920, configurable: true });
});
afterEach(() => {
  cleanup();
  resetHolds();
});

const proc = (id: string, agent: string): Proc =>
  ({ id, agent, kind: 'bg', cmd: `run ${agent}`, pid: 100, started_ms: 1, state: { is: 'running' }, cpu: 0, rss_mb: 1, procs: 1, out: 'none' }) as unknown as Proc;

/** A fake core whose prefs_set comes back as the hub's prefs (as the real one echoes it). */
function world() {
  const cmds: CoreCmd[] = [];
  let prefs: Record<string, Record<string, JsonValue>> = {};
  const port: CorePort = {
    send: (c) => {
      cmds.push(c);
      if (c.cmd === 'prefs_set') {
        const [root, project] = c.key.split('.') as [string, string];
        prefs = { ...prefs, [root]: { ...prefs[root], [project]: c.value } };
        queueMicrotask(() => {
          store.apply({ ev: 'prefs', prefs });
        });
      }
    },
    onEvent: () => () => undefined,
    onState: () => () => undefined,
    state: () => Promise.resolve('up'),
  };
  const store = new Store(port);
  (window as unknown as { bise: unknown }).bise = { notify: { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined } };
  store.setCore('up');
  store.apply(projects(row({ project: 'bise-home-00000000', name: 'bise', home: true }), row({ project: ACME, name: 'acme' })));
  store.apply(ev('agents', { project: ACME }));
  store.apply(artifacts(ACME, artifact({ id: 'changelog', kind: 'page', title: 'changelog' }), artifact({ id: 'bench', kind: 'file', title: 'bench' })));
  store.apply({ ev: 'processes', project: ACME, items: [proc('bg1', 'perf')] });
  store.apply({ ev: 'prefs', prefs: {} });
  const router = createRouter({ screen: 'thread', project: ACME, agent: 'perf' });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <PanelHost />
        <SideRow kind="artifact" id="changelog" />
      </RouterContext>
    </StoreProvider>,
  );
  return { v, store, cmds };
}

/** the right side's row of a thing: its hover pin, its state from the prefs */
function SideRow({ kind, id }: { kind: PinKind; id: string }) {
  const on = pinnedIn(usePins(kind), ACME, id);
  return (
    <div className="s3-card">
      <div className="s3-row" data-on={on || undefined} data-testid="side.row">
        <PinButton pin={{ kind, project: ACME, id, on }} testId="side.pin" />
      </div>
    </div>
  );
}

const icon = (el: HTMLElement) => el.querySelector('svg')?.getAttribute('data-icon');
/** the tab in front's pin, inside its tab (null: none) */
const frontPin = (): HTMLElement | null => document.querySelector<HTMLElement>('.pn-tabs [role="tab"][aria-selected="true"] [data-testid$=".pin"]');
const pinOf = (): HTMLElement => {
  const el = frontPin();
  if (el === null) throw new Error('no pin in the tab in front');
  return el;
};
const flush = () => act(() => Promise.resolve());

describe('the pin in the tab', () => {
  it('an artifact in front: its pin in its tab, just left of its ×, the outline and Pin; a click pins it, and the side’s row shows it', async () => {
    const { v, cmds } = world();
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: 'changelog' });
    });
    const pin = pinOf();
    // his m_50763: in the tab, the × right after it, and no 'open in a window' at the row's end
    const close = pin.closest('[role="tab"]')?.querySelector('[data-testid$=".close"]');
    expect(pin.closest('.pn-tab-pin')?.nextElementSibling).toBe(close);
    expect(v.getByTestId('app.panel.top').querySelectorAll('.pn-top > button, .pn-top > span')).toHaveLength(0);
    expect(icon(pin)).toBe('pin');
    expect(pin.getAttribute('aria-label')).toBe(menus.pin);
    expect(v.getByTestId('side.row').hasAttribute('data-on')).toBe(false);
    fireEvent.click(pin);
    // at once, before the echo
    expect(icon(pinOf())).toBe('pin-on');
    expect(cmds.at(-1)).toEqual({ cmd: 'prefs_set', key: `pins_art.${ACME}`, value: ['changelog'] });
    await flush();
    expect(v.getByTestId('side.row').hasAttribute('data-on')).toBe(true);
    expect(icon(v.getByTestId('side.pin'))).toBe('pin-on');
    expect(pinOf().getAttribute('aria-label')).toBe(menus.unpin);
    // again: unpinned, on both
    fireEvent.click(pinOf());
    await flush();
    expect(cmds.at(-1)).toEqual({ cmd: 'prefs_set', key: `pins_art.${ACME}`, value: [] });
    expect(v.getByTestId('side.row').hasAttribute('data-on')).toBe(false);
    expect(icon(pinOf())).toBe('pin');
  });

  it('a pin made on the side shows in the tab at once, and its unpin too', async () => {
    const { v } = world();
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: 'changelog' });
    });
    fireEvent.click(v.getByTestId('side.pin'));
    await flush();
    expect(icon(pinOf())).toBe('pin-on');
    fireEvent.click(v.getByTestId('side.pin'));
    await flush();
    expect(icon(pinOf())).toBe('pin');
  });

  it('an agent’s thread and a process in front: their own pins (pins, pins_proc)', async () => {
    const { cmds } = world();
    act(() => {
      openTab({ kind: 'agent', project: ACME, agent: 'perf' });
    });
    fireEvent.click(pinOf());
    expect(cmds.at(-1)).toEqual({ cmd: 'prefs_set', key: `pins.${ACME}`, value: ['perf'] });
    await flush();
    act(() => {
      openTab({ kind: 'process', project: ACME, agent: 'perf', id: 'bg1' });
    });
    expect(icon(pinOf())).toBe('pin');
    fireEvent.click(pinOf());
    expect(cmds.at(-1)).toEqual({ cmd: 'prefs_set', key: `pins_proc.${ACME}`, value: [procPinId('perf', 'bg1')] });
  });

  it('no pin on what has none, on Scheduled (the whole list), nor on a thing no longer in its list', () => {
    world();
    act(() => {
      openTab({ kind: 'pr', project: ACME, number: 12 });
    });
    expect(frontPin()).toBeNull();
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: 'gone' });
    });
    expect(frontPin()).toBeNull();
    expect(tabPinOf({ kind: 'scheduled', project: ACME, id: 7 })).toBeUndefined();
    expect(tabPinOf({ kind: 'commit', project: ACME, sha: 'abc' })).toBeUndefined();
  });
});

describe('the tab’s menu', () => {
  const fx = (pins: unknown[][]): MenuFx =>
    new Proxy({} as MenuFx, { get: (_, k) => (k === 'pin' ? (...a: unknown[]) => pins.push(a) : () => undefined) });

  it('lists Pin or Unpin for its thing, and its pick pins that thing', () => {
    const inner = (pinned: boolean) => factsOf({ kind: 'artifact', project: ACME, id: 'changelog' }, {
      projects: () => [],
      agents: () => [],
      artifacts: () => [artifact({ id: 'changelog' })],
      processes: () => [],
      scheduled: () => [],
      pinned: () => false,
      pinnedThing: () => pinned,
      reply: () => undefined,
      tab: () => ({ inner: undefined, moveable: false, closable: true, others: false }),
    });
    const tab = (pinned: boolean) => ({ e: { kind: 'tab', id: 'artifact:acme/changelog' } as const, inner: inner(pinned), moveable: false, closable: true, others: false });
    expect(actsOf(menuOf(tab(false)))).toContain('pin');
    expect(actsOf(menuOf(tab(true)))).toContain('unpin');
    const pins: unknown[][] = [];
    runAct('pin', tab(false), fx(pins));
    runAct('unpin', tab(true), fx(pins));
    expect(pins).toEqual([
      ['artifact', ACME, 'changelog', true],
      ['artifact', ACME, 'changelog', false],
    ]);
  });

  it('pinRefOf: each kind’s pin id, as the side’s rows key it', () => {
    expect(pinRefOf({ kind: 'agent', project: ACME, agent: 'perf' })).toEqual({ kind: 'agent', project: ACME, id: 'perf' });
    expect(pinRefOf({ kind: 'process', project: ACME, agent: 'perf', id: 'bg1' })).toEqual({ kind: 'process', project: ACME, id: 'perf/bg1' });
    expect(pinRefOf({ kind: 'scheduled', project: ACME, id: 7 })).toEqual({ kind: 'schedule', project: ACME, id: '7' });
    expect(pinRefOf({ kind: 'pr', project: ACME, number: 1 })).toBeUndefined();
  });
});
