// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Agent, Card, Feature } from '@/proto';
import type { CoreCmd, CorePort } from '@/shared/api';
import { Store, StoreProvider, replay } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { parseRoute, formatRoute } from '@/app/route';
import { choicesOf, distance, dropAsk, groupsOf, stateWords } from './feature';
import { FeatureScreen } from './FeatureScreen';
import { ChangesScreen } from './ChangesScreen';

const P = 'acme-1a2b3c4d';
const T = 1_791_100_000_000;
const ag = (name: string, over: Partial<Agent> = {}): Agent => ({ name, dir: name, main: false, status: 'idle', archived: false, title: '', purpose: '', since_ms: T, waits: 0, watching: [], ...over });
const feat = (over: Partial<Feature> = {}): Feature => ({
  name: 'gift-cards',
  branch: 'gift-cards',
  base: 'main',
  agents: ['gift-api', 'gift-ui'],
  ahead: 2,
  behind: 0,
  adds: 42,
  dels: 3,
  tip: 'abc',
  checked: false,
  trial: false,
  building: false,
  created_ms: T,
  ...over,
});
const mergeCard: Card = {
  id: 7,
  project: P,
  kind: 'feature_merge',
  agent: 'main',
  question: 'gift-cards is on trial: merge it?',
  options: [
    { n: 1, label: 'merge' },
    { n: 2, label: 'keep working' },
    { n: 3, label: 'drop the branch' },
  ],
  urgent: false,
  since_ms: T,
  approval: true,
};

describe('features, pure', () => {
  it("groups the panel as the TUI: agents in no feature first, then each feature's head with its agents", () => {
    const live = [ag('p99'), ag('gift-api'), ag('alerts'), ag('gift-ui')];
    const { solo, groups } = groupsOf(live, [feat()]);
    expect(solo.map((a) => a.name)).toEqual(['p99', 'alerts']);
    expect(groups.map((g) => [g.feature.name, g.agents.map((a) => a.name)])).toEqual([['gift-cards', ['gift-api', 'gift-ui']]]);
  });
  it('says its state and how far it is', () => {
    expect(stateWords(feat({ building: true, trial: true }))).toBe('building a try');
    expect(stateWords(feat({ trial: true, tried: { sha: '4b825dc', run: 'x', at_ms: T } }))).toBe('on trial · 4b825dc');
    expect(stateWords(feat({ checked: true }))).toBe('checked · ready to try');
    expect(stateWords(feat({ ahead: 0 }))).toBe('nothing yet');
    expect(distance(feat({ behind: 1 }))).toBe('2 ahead · 1 behind · +42 −3');
    expect(distance(feat({ adds: 0, dels: 0 }))).toBe('2 ahead');
  });
  it("the merge card's drop asks once more (the TUI's drop_ask_shape); its yes sends the drop's digit", () => {
    expect(choicesOf(mergeCard, false)).toEqual([
      { reply: '1', label: 'merge' },
      { reply: '2', label: 'keep working' },
      { label: 'drop the branch', confirm: true },
    ]);
    expect(choicesOf(mergeCard, true)).toEqual([{ reply: '3', label: 'drop it' }, { label: 'keep it', back: true }]);
    expect(dropAsk(feat())).toBe('drop gift-cards? 2 commits go.');
    const tryCard: Card = { ...mergeCard, kind: 'feature_try', options: [{ n: 1, label: 'try it' }, { n: 2, label: 'show the diff' }, { n: 3, label: 'not yet' }] };
    expect(choicesOf(tryCard, false).every((c) => c.reply !== undefined)).toBe(true);
    // try-diff-open: an option the hub's row gives an act (its '2 show the diff' opens the
    // feature's diff artifact) opens it, never sent as an answer
    const open = { act: 'open', artifact: 'cu-diff' } as const;
    const withDiff: Card = { ...tryCard, options: tryCard.options.map((o) => (o.n === 2 ? { ...o, act: open } : o)) };
    expect(choicesOf(withDiff, false)).toEqual([{ reply: '1', label: 'try it' }, { act: open, label: 'show the diff', n: 2 }, { reply: '3', label: 'not yet' }]);
  });
  it('has its route', () => {
    expect(parseRoute('feature/shop/gift-cards')).toEqual({ screen: 'feature', project: 'shop', name: 'gift-cards' });
    expect(formatRoute({ screen: 'feature', project: 'shop', name: 'gift-cards' })).toBe('feature/shop/gift-cards');
    expect(parseRoute('feature/shop').screen).toBe('notfound');
  });
});

function setup() {
  const cmds: CoreCmd[] = [];
  const port: CorePort = { send: (c) => cmds.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  replay(store);
  const router = createRouter({ screen: 'bise' });
  const wrap = (ui: React.ReactNode) => (
    <StoreProvider store={store}>
      <RouterContext value={router}>{ui}</RouterContext>
    </StoreProvider>
  );
  return { store, cmds, router, wrap };
}

afterEach(cleanup);

describe('the feature screen', () => {
  it("draws the fixture's gift-cards: branch, base, distance, state, the try it ran; asks features on open", () => {
    const { cmds, wrap } = setup();
    const v = render(wrap(<FeatureScreen project={P} name="gift-cards" />));
    expect(cmds).toContainEqual({ cmd: 'features', project: P });
    const text = v.getByTestId('feature.screen').textContent;
    expect(v.getByTestId('feature.screen').querySelector('.b-head')?.textContent).toContain('gift-cards');
    expect(text).toContain('gift-cards → main · 2 ahead · 1 behind · +42 −3');
    expect(v.getByTestId('feature.state').textContent).toContain('on trial · 4b825dc');
    expect(v.getByTestId('feature.tried').textContent).toContain('~/.bise/dev/versions/4b825dc/bise');
  });

  it('merge answers its card; drop asks once more, keep it goes back, drop it answers 3', () => {
    const { store, cmds, wrap } = setup();
    act(() => {
      // a project's cards are its row's (the one copy, store reads.ts cards)
      store.apply({ ev: 'projects', projects: store.projects().map((r) => (r.project === P ? { ...r, cards: [mergeCard] } : r)) });
    });
    const v = render(wrap(<FeatureScreen project={P} name="gift-cards" />));
    fireEvent.click(v.getByTestId('feature.drop'));
    expect(v.getByTestId('feature.card.7').textContent).toContain('drop gift-cards? 2 commits go.');
    expect(cmds.some((c) => c.cmd === 'answer')).toBe(false);
    fireEvent.click(v.getByTestId('feature.keep'));
    expect(v.getByTestId('feature.card.7').textContent).toContain('merge it?');
    fireEvent.click(v.getByTestId('feature.drop'));
    fireEvent.click(v.getByTestId('feature.drop.yes'));
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'answer', project: P, card: 7, reply: '3' }));
    fireEvent.click(v.getByTestId('feature.opt.1'));
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'answer', project: P, card: 7, reply: '1' }));
  });

  it('an unknown feature says so', () => {
    const { wrap } = setup();
    const v = render(wrap(<FeatureScreen project={P} name="nope" />));
    expect(v.getByTestId('feature.screen').textContent).toContain('no such feature');
  });

  it('the changes screen lists the features, a row opens one', () => {
    const { cmds, router, wrap } = setup();
    const v = render(wrap(<ChangesScreen project={P} />));
    expect(cmds).toContainEqual({ cmd: 'features', project: P });
    fireEvent.click(v.getByTestId('changes.feature.gift-cards.open'));
    expect(router.get()).toEqual({ screen: 'feature', project: P, name: 'gift-cards' });
  });
});
