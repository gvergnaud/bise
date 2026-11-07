// @vitest-environment happy-dom
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Pr } from '@/proto';
import { hub_ev } from '@/proto/fixtures.gen';
import { pages } from '@/proto/words_pages.gen';
import type { CoreCmd, CorePort } from '@/shared/api';
import { Store, StoreProvider, replay } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { formatRoute, parseRoute } from '@/app/route';
import { runWindowCommand } from '@/thread/commands';
import { agentsOf, toneOf, wordsOf } from './prs';
import { PrsScreen } from './PrsScreen';
import { panelState } from '@/app/panel/state';
import { ChangesScreen } from './ChangesScreen';

const P = 'acme-1a2b3c4d';
const pr = (over: Partial<Pr> = {}): Pr => ({ number: 1, url: 'u', branch: 'sb/x', agents: [], state: 'open', checks: 'none', review: 'none', words: '', text: '', ...over });

const fixtures = hub_ev.flatMap((e) => (e.ev === 'prs' ? [e] : []));

describe('PRs, pure (fixtures)', () => {
  it('the fixtures have a list and an empty one with the hub words', () => {
    expect(fixtures.length).toBeGreaterThanOrEqual(2);
    expect(fixtures.some((e) => e.items.length === 0 && (e.none ?? '') !== '')).toBe(true);
  });
  it("tone from meaning: failing checks, changes asked, running, ready, resting; the words say it too", () => {
    const list = fixtures.find((e) => e.items.length > 0)?.items ?? [];
    const dark = list.find((p) => p.number === 412);
    const perf = list.find((p) => p.number === 415);
    if (!dark || !perf) throw new Error('fixture PRs 412 and 415');
    expect(toneOf(dark)).toBe('err');
    expect(wordsOf(dark)).toBe('changes asked · checks fail: e2e');
    expect(agentsOf(dark)).toBe('dark-mode');
    expect(toneOf(perf)).toBe('wk');
    expect(wordsOf(perf)).toBe('draft · checks running');
    expect(agentsOf(perf)).toBe('no agent');
    expect(toneOf(pr({ review: 'changes', checks: 'pass' }))).toBe('nd');
    expect(toneOf(pr({ review: 'approved', checks: 'pass' }))).toBe('ok');
    expect(toneOf(pr({ state: 'draft', review: 'approved', checks: 'pass' }))).toBe('rest');
    expect(toneOf(pr())).toBe('rest');
  });
  it('words from the fields when the hub sent none', () => {
    expect(wordsOf(pr({ state: 'draft', checks: 'fail', failing: ['lint', 'e2e'] }))).toBe('draft · checks fail: lint, e2e');
    expect(wordsOf(pr({ review: 'approved', checks: 'pass' }))).toBe('approved · checks pass');
  });
  it('/prs and its route', () => {
    expect(runWindowCommand('/prs', '', { project: P, agent: 'main' })).toEqual({ screen: 'prs', project: P });
    expect(parseRoute(`prs/${P}`)).toEqual({ screen: 'prs', project: P });
    expect(formatRoute({ screen: 'prs', project: P })).toBe(`prs/${P}`);
  });
});

function setup() {
  const cmds: CoreCmd[] = [];
  const port: CorePort = { send: (c) => cmds.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  const router = createRouter({ screen: 'bise' });
  const wrap = (ui: React.ReactNode) => (
    <StoreProvider store={store}>
      <RouterContext value={router}>{ui}</RouterContext>
    </StoreProvider>
  );
  return { store, cmds, router, wrap };
}

afterEach(cleanup);

describe('the PR list', () => {
  it('asks prs on open; rows say number, branch, agents, state in words; a click opens its tab, ⌘-click the forge', () => {
    const { store, cmds, wrap } = setup();
    const list = fixtures.find((e) => e.items.length > 0);
    if (!list) throw new Error('fixture');
    store.apply(list);
    const openExternal = vi.fn(() => Promise.resolve());
    (window as unknown as { bise: unknown }).bise = { openExternal };
    const v = render(wrap(<PrsScreen project={P} />));
    expect(cmds).toContainEqual({ cmd: 'prs', project: P });
    expect(v.getByTestId('prs.screen').textContent).toContain('2 PRs open');
    const row = v.getByTestId('prs.row.412');
    expect(row.textContent).toContain('#412');
    expect(row.textContent).toContain('sb/dark-mode');
    expect(row.textContent).toContain('dark-mode');
    expect(v.getByTestId('prs.words.412').textContent).toBe('changes asked · checks fail: e2e');
    expect(row.closest('[data-tone]')?.getAttribute('data-tone')).toBe('err');
    expect(v.getByTestId('prs.row.415').textContent).toContain('draft');
    fireEvent.click(v.getByTestId('prs.row.412.open'));
    expect(openExternal, 'a plain click stays in the app').not.toHaveBeenCalled();
    expect(panelState.get().tabs.list.map((t) => t.id)).toContain(`pr:${P}/https://github.com/acme/acme/pull/412`);
    fireEvent.click(v.getByTestId('prs.row.412.open'), { metaKey: true });
    expect(openExternal).toHaveBeenCalledWith('https://github.com/acme/acme/pull/412');
    delete (window as unknown as { bise?: unknown }).bise;
  });

  it("empty: one line, never the banner's sentence again (designer m_43933 #7)", () => {
    const { store, wrap } = setup();
    const none = fixtures.find((e) => e.items.length === 0);
    if (!none) throw new Error('fixture');
    store.apply(none);
    const v = render(wrap(<PrsScreen project={P} />));
    expect(v.getByTestId('prs.none').textContent).toBe(pages.no_pr);
  });

  it('the changes screen asks prs, says how many, and opens the list', () => {
    const { store, cmds, router, wrap } = setup();
    replay(store);
    const v = render(wrap(<ChangesScreen project={P} />));
    expect(cmds).toContainEqual({ cmd: 'prs', project: P });
    fireEvent.click(v.getByTestId('changes.prs.open'));
    expect(router.get()).toEqual({ screen: 'prs', project: P });
  });
});
