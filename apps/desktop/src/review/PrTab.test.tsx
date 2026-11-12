// @vitest-environment happy-dom
// A PR's tab on a fake core (pr-integration A rules 7 to 12, page pr-guided V2): its one state line
// from the hub's row only, Open on GitHub always, Merge only when mergeable and at the head he sees,
// its title's ▾ with Close behind a confirm, the description folded to its first line over the
// guide, a failing check's piece that opens the checks, the Summary's waits and misses in words,
// Ask <agent> to fix that fills the composer and never sends. Owner: pr-artifacts.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CoreCmd } from '@/shared/api';
import type { DiffFile, Pr } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { panel } from '@/proto/words_panel.gen';
import { artifact, artifacts, projects, row } from '../../tests/feature/inputs';
import { PrTab } from './PrTab';

const P = 'acme-1a2b3c4d';
const URL = 'https://github.com/acme/web/pull/412';

const status = (over: Partial<Pr> = {}): Pr => ({
  number: 412, url: URL, branch: 'sb/perf', base: 'main', agents: ['perf'], state: 'open', checks: 'pass', review: 'approved',
  words: 'approved · checks ✓ 12/12 · your merge', text: '', yours: 'merge', mergeable: true, head_oid: 'abc123',
  pieces: [{ text: 'your merge', tone: 'yours' }, { text: 'approved', tone: 'plain' }, { text: 'checks ✓ 12/12', tone: 'plain' }], ...over,
});

function shell(s: Pr | null, bise: Record<string, unknown> = {}) {
  const sent: CoreCmd[] = [];
  (window as unknown as { bise: unknown }).bise = { openExternal: () => undefined, copy: () => undefined, ...bise };
  const store = new Store({ send: (c) => sent.push(c) });
  store.setCore('up');
  store.apply(projects(row({ project: P, name: 'acme' })));
  if (s !== null) store.apply(artifacts(P, artifact({ id: 'pr-412', kind: 'pr', title: 'Faster signup', url: URL, pr: { repo: 'acme/web', number: 412, tag: '#412', status: s } })));
  const router = createRouter({ screen: 'thread', project: P, agent: 'main' });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <PrTab project={P} url={URL} />
      </RouterContext>
    </StoreProvider>,
  );
  const last = <K extends CoreCmd['cmd']>(cmd: K) => sent.filter((c): c is Extract<CoreCmd, { cmd: K }> => c.cmd === cmd).at(-1);
  return { v, store, sent, last, router };
}

afterEach(cleanup);

describe('a PR tab', () => {
  it('one state line from the hub row, pink when it waits on him, Open on GitHub and Merge on it; the guide right under', () => {
    const { v } = shell(status());
    expect(v.getByTestId('prtab.title').textContent).toBe('Faster signup');
    // only what waits on him in the accent, first; the rest dim (designer m_54748)
    const said = v.getByTestId('prtab.status');
    expect(said.textContent).toBe('#412 · your merge · approved · checks ✓ 12/12');
    expect([...said.querySelectorAll('.prw-acc')].map((e) => e.textContent)).toEqual([' · your merge']);
    const line = v.getByTestId('prtab.line');
    expect(line.contains(v.getByTestId('prtab.open'))).toBe(true);
    expect(line.contains(v.getByTestId('prtab.merge'))).toBe(true);
    expect(v.getByTestId('prtab.open').textContent).toBe(panel.prtab_open_on({ forge: 'GitHub' }));
    // V2: no Summary | Changes tabs, the state and the description scroll away over the guide
    expect(v.queryByTestId('prtab.tabs')).toBeNull();
    expect(v.getByTestId('prtab.changes.files').contains(v.getByTestId('prtab.lead'))).toBe(true);
  });

  it('the description folded to its first line; open, the Summary under it (its branch first); shut again', () => {
    const { v, store, last } = shell(status({ base: 'main' }));
    act(() => {
      store.apply({ ev: 'pr_detail', project: P, req: last('pr_detail')?.req ?? 0, url: URL, detail: { body: '## Cut the **growth**\n\nmore', checks: [], notes: [] } });
    });
    const fold = v.getByTestId('prtab.desc.fold');
    expect(fold.textContent).toBe(`▸ ${panel.prtab_desc_fold}Cut the growth`);
    expect(v.queryByTestId('prtab.summary')).toBeNull();
    fireEvent.click(fold);
    expect(v.getByTestId('prtab.facts').textContent).toBe('sb/perf → main · perf');
    expect(v.getByTestId('prtab.body').textContent).toContain('more');
    fireEvent.click(fold);
    expect(v.queryByTestId('prtab.summary')).toBeNull();
  });

  it('a failing check in the line opens the description on its checks', () => {
    const { v, store, last } = shell(status({ checks: 'fail', pieces: [{ text: 'checks ✗ 1', tone: 'fail' }] }));
    act(() => {
      store.apply({ ev: 'pr_detail', project: P, req: last('pr_detail')?.req ?? 0, url: URL, detail: { body: 'x', checks: [{ name: 'e2e', state: 'fail' }], notes: [] } });
    });
    expect(v.queryByTestId('prtab.checks')).toBeNull();
    fireEvent.click(v.getByTestId('prtab.status.fail'));
    expect(v.getByTestId('prtab.check.e2e').getAttribute('data-state')).toBe('fail');
  });

  it('its title is its menu: on GitHub, copy link and branch, Close; a right click opens the same', async () => {
    const shown: string[][] = [];
    const opened: string[] = [];
    const { v } = shell(status(), {
      popupMenu: (items: { id?: string }[]) => { shown.push(items.flatMap((i) => (i.id === undefined ? [] : [i.id]))); return Promise.resolve('open'); },
      openExternal: (u: string) => { opened.push(u); },
    });
    await act(async () => {
      fireEvent.click(v.getByTestId('prtab.menu'));
      await Promise.resolve();
    });
    await act(async () => {
      fireEvent.contextMenu(v.getByTestId('prtab.menu'));
      await Promise.resolve();
    });
    expect(shown).toEqual([['open', 'copy-link', 'copy-branch', 'close'], ['open', 'copy-link', 'copy-branch', 'close']]);
    expect(opened).toEqual([URL, URL]);
  });

  it('before its row comes: the plain wait, Open on GitHub beside it, no Merge', () => {
    const { v } = shell(null);
    expect(v.getByTestId('prtab.waiting').textContent).toContain(panel.prtab_waiting_row);
    expect(v.getAllByTestId('prtab.open').length).toBeGreaterThan(0);
    expect(v.queryByTestId('prtab.merge')).toBeNull();
  });

  it('Merge only when mergeable; his click sends the head he sees, then says why it was refused', () => {
    expect(shell(status({ mergeable: false })).v.queryByTestId('prtab.merge')).toBeNull();
    cleanup();
    const { v, store, last } = shell(status());
    fireEvent.click(v.getByTestId('prtab.merge'));
    const cmd = last('pr_act');
    expect(cmd).toMatchObject({ project: P, url: URL, head: 'abc123', act: 'merge' });
    expect(v.getByTestId('prtab.acted').textContent).toBe(panel.prtab_merging);
    expect(v.queryByTestId('prtab.merge'), 'one at a time').toBeNull();
    act(() => {
      store.apply({ ev: 'pr_acted', project: P, req: cmd?.req ?? 0, url: URL, act: 'merge', err: 'moved' });
    });
    expect(v.getByTestId('prtab.acted').textContent).toBe(panel.prtab_err_moved);
  });

  it('the PR’s size at the line’s end, GitHub’s: the + and − over every file and five squares, once its diff comes', () => {
    const { v, store, last } = shell(status());
    expect(v.queryByTestId('prtab.stat')).toBeNull();
    const f = (path: string, add: number, del: number): DiffFile => ({ path, status: 'modified', add, del, hunks: [] });
    act(() => {
      store.apply({ ev: 'diff', project: P, req: last('diff')?.req ?? 0, agent: 'perf', base: 'main',
        files: [f('src/a.ts', 400, 100), { ...f('src/big.ts', 38, 51), truncated: true }] });
    });
    const stat = v.getByTestId('prtab.stat');
    expect(v.getByTestId('prtab.line').contains(stat)).toBe(true);
    expect([stat.querySelector('.prt-add')?.textContent, stat.querySelector('.prt-del')?.textContent]).toEqual(['+438', '−151']);
    const squares = [...v.getByTestId('prtab.stat.squares').querySelectorAll('rect')].map((r) => r.getAttribute('class'));
    expect(squares).toEqual(['prt-sq-add', 'prt-sq-add', 'prt-sq-add', 'prt-sq-del', 'prt-sq-none']);
  });

  it('Close is in its title’s ▾ only, and asks first', async () => {
    const { v, last } = shell(status(), { popupMenu: () => Promise.resolve('close') });
    expect(v.queryByText(panel.prtab_close)).toBeNull();
    await act(async () => {
      fireEvent.click(v.getByTestId('prtab.menu'));
      await Promise.resolve();
    });
    expect(v.getByTestId('prtab.confirm').textContent).toContain(panel.prtab_close_confirm({ tag: '#412', forge: 'GitHub' }));
    expect(last('pr_act'), 'nothing before his yes').toBeUndefined();
    fireEvent.click(v.getByTestId('prtab.confirm.yes'));
    expect(last('pr_act')).toMatchObject({ act: 'close', url: URL });
  });

  it('the Summary: what it waits for, then the detail; a late answer in one line with a retry', () => {
    const { v, store, last } = shell(status());
    fireEvent.click(v.getByTestId('prtab.desc.fold'));
    expect(v.getByTestId('prtab.loading').textContent).toContain(panel.prtab_loading_detail({ forge: 'GitHub' }));
    const first = last('pr_detail')?.req ?? 0;
    act(() => {
      store.apply({ ev: 'pr_detail', project: P, req: first, url: URL, err: 'late' });
    });
    expect(v.getByTestId('prtab.miss').textContent).toContain(panel.prtab_late({ forge: 'GitHub' }));
    fireEvent.click(v.getByTestId('prtab.retry'));
    const second = last('pr_detail')?.req ?? 0;
    expect(second).not.toBe(first);
    act(() => {
      store.apply({
        ev: 'pr_detail', project: P, req: second, url: URL,
        detail: { body: 'Signup was **slow**. <img src=x onerror=alert(1)>', checks: [{ name: 'ci / test', state: 'fail' }, { name: 'lint', state: 'pass' }], notes: [{ author: 'alice', body: 'one nit', at_ms: 1, verdict: 'changes' }], more_notes: 3 },
      });
    });
    expect(v.getByTestId('prtab.body').innerHTML).not.toContain('<img');
    expect(v.getByTestId('prtab.check.ci / test').getAttribute('data-state')).toBe('fail');
    expect(v.getByTestId('prtab.notes').textContent).toContain('alice · asked for changes');
    expect(v.getByTestId('prtab.more_notes').textContent).toBe(panel.prtab_more_notes({ n: 3, forge: 'GitHub' }));
  });

  it('Ask perf to fix: reads the log, fills perf’s composer, goes to its thread, never sends', () => {
    const { v, store, last, sent, router } = shell(status());
    const req = last('pr_detail')?.req ?? 0;
    act(() => {
      store.apply({ ev: 'pr_detail', project: P, req, url: URL, detail: { body: '', checks: [{ name: 'e2e', state: 'fail' }], notes: [] } });
    });
    fireEvent.click(v.getByTestId('prtab.desc.fold'));
    fireEvent.click(v.getByTestId('prtab.fix.e2e'));
    const ask = last('pr_check_log');
    expect(ask).toMatchObject({ url: URL, check: 'e2e' });
    expect(v.getByText(panel.prtab_ask_fix_reading)).toBeTruthy();
    act(() => {
      store.apply({ ev: 'pr_check_log', project: P, req: ask?.req ?? 0, url: URL, check: 'e2e', lines: ['FAIL cold start'] });
    });
    expect(router.get()).toMatchObject({ screen: 'thread', project: P, agent: 'perf' });
    expect(sent.some((c) => c.cmd === 'send'), 'never sent').toBe(false);
  });
});
