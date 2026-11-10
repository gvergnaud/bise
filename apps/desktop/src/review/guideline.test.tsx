// @vitest-environment happy-dom
// The thread's 'guided review ready' line (page pr-guided V2, architect m_58377): from the hub's
// guide answer that names its target (the fixture line with `target`), never from a log entry;
// asked once per PR head with run false; gone when the head moves until the new guide is ready; a
// click opens the PR's panel. Fake events only. Mutation: the store not keeping guide-for-target, or
// the line not reading it, turns these red.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { CoreCmd, CorePort } from '@/shared/api';
import type { HubEv, Pr } from '@/proto';
import { hub_ev } from '@/proto/fixtures.gen';
import { pages } from '@/proto/words_pages.gen';
import { Store, StoreProvider } from '@/store';
import { guideTargetKey } from '@/store/types';
import { createRouter, RouterContext } from '@/app/router';
import { panelState } from '@/app/panel/state';
import { ev, projects, row } from '../../tests/feature/inputs';
import { GuideLine } from './GuideLine';

const P = 'acme-1a2b3c4d';
// the fixture's answer that names its target: a PR's guide, ready
const answer = hub_ev.find((e): e is Extract<HubEv, { ev: 'guide' }> => e.ev === 'guide' && e.target != null);
if (answer?.target?.kind !== 'pr' || answer.guide == null) throw new Error('fixture: a ready guide answer with a PR target');
const URL = answer.target.url;
// the hub's PR row (places, pr_link form) is what the thread asks with: it must key as the answer
const row412 = hub_ev.flatMap((e) => (e.ev === 'agents' ? (e.places ?? []) : [])).find((p) => p.pr?.number === 412)?.pr;
const pr = (head: string): Pr => ({ number: 412, url: URL, branch: 'sb/perf', agents: ['perf'], state: 'open', checks: 'pass', review: 'none', words: '', text: '', head_oid: head });
const agent = (name: string) => ({ name, dir: name, main: name === 'main', status: 'idle' as const, archived: false, title: '', purpose: '', since_ms: 0, waits: 0, watching: [] });

function shell(head = 'abc') {
  const sent: CoreCmd[] = [];
  const port: CorePort = { send: (c) => sent.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: P, name: 'acme' })));
  const places = (h: string) => ev('agents', { project: P, agents: [agent('main'), agent('perf')], places: [{ id: 'wt:perf', branch: 'sb/perf', agents: ['perf'], pr: pr(h) }] });
  store.apply(places(head));
  const router = createRouter({ screen: 'thread', project: P, agent: 'perf' });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}><GuideLine project={P} agent="perf" /></RouterContext>
    </StoreProvider>,
  );
  const send = (e: Parameters<Store['apply']>[0]) => {
    act(() => {
      store.apply(e);
    });
  };
  const asks = () => sent.filter((c) => c.cmd === 'guide');
  return { v, send, asks, places };
}

beforeEach(() => {
  panelState.reset();
});
afterEach(cleanup);

describe("the thread's guided review line", () => {
  it("the fixture's PR row and its guide answer key the same change (the hub's one link form)", () => {
    if (row412 == null || answer.target == null) throw new Error('fixture: PR #412 row and a targeted answer');
    expect(guideTargetKey({ kind: 'pr', url: row412.url })).toBe(guideTargetKey(answer.target));
  });

  it('asks its PR’s guide once, never to run; shows once the hub names that PR’s guide ready; a click opens the PR', () => {
    const { v, send, asks } = shell();
    expect(asks()).toEqual([{ cmd: 'guide', project: P, target: { kind: 'pr', url: URL } }]);
    expect(v.queryByTestId('thread.guide')).toBeNull();
    // a guide for another diff (no target, or another change's) says nothing here
    send({ ...answer, target: { kind: 'agent', name: 'perf' }, hash: 'other' });
    expect(v.queryByTestId('thread.guide')).toBeNull();
    send(answer);
    expect(v.getByTestId('thread.guide').textContent).toBe(`${pages.review_guide_line({ tag: '#412', n: answer.guide?.parts.length ?? 0 })} ›`);
    fireEvent.click(v.getByTestId('thread.guide.open'));
    const front = panelState.get().tabs.list.find((t) => t.id === panelState.get().tabs.active)?.what;
    expect(front).toEqual({ kind: 'thing', e: { kind: 'pr', project: P, url: URL } });
  });

  it('a push: asked again; the new head’s answer replaces the old one, so the line goes until it is ready', () => {
    const { v, send, asks, places } = shell();
    send(answer);
    expect(v.getByTestId('thread.guide')).toBeTruthy();
    send(places('def'));
    expect(asks()).toHaveLength(2);
    send({ ev: 'guide', project: P, hash: 'newhead', target: { kind: 'pr', url: URL }, state: 'writing' });
    expect(v.queryByTestId('thread.guide')).toBeNull();
    send({ ...answer, hash: 'newhead' });
    expect(v.getByTestId('thread.guide')).toBeTruthy();
  });
});
