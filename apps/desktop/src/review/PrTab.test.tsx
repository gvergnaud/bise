// @vitest-environment happy-dom
// A PR's tab on a fake core (pr-integration A rules 7 to 12): its head from the hub's row only,
// Open on GitHub always, Merge only when mergeable and at the head he sees, Close behind a confirm,
// the Summary's waits and misses in words, Ask <agent> to fix that fills the composer and never
// sends, and a late answer dropped. Owner: pr-artifacts.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CoreCmd } from '@/shared/api';
import type { Pr } from '@/proto';
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
  it('says its head from the hub row, the status line pink when it waits on him, Open on GitHub always', () => {
    const { v } = shell(status());
    expect(v.getByTestId('prtab.title').textContent).toBe('Faster signup');
    expect(v.getByTestId('prtab.facts').textContent).toBe('#412 · open · sb/perf → main · perf');
    // only what waits on him in the accent, first; the rest dim (designer m_54748)
    const said = v.getByTestId('prtab.status');
    expect(said.textContent).toBe('your merge · approved · checks ✓ 12/12');
    expect([...said.querySelectorAll('.prw-acc')].map((e) => e.textContent)).toEqual(['your merge']);
    expect(v.getByTestId('prtab.open').textContent).toBe(panel.prtab_open_on({ forge: 'GitHub' }));
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

  it('Close is in its ⋯ only, and asks first', async () => {
    const { v, last } = shell(status(), { popupMenu: () => Promise.resolve('close') });
    expect(v.queryByText(panel.prtab_close)).toBeNull();
    await act(async () => {
      fireEvent.click(v.getByTestId('prtab.more'));
      await Promise.resolve();
    });
    expect(v.getByTestId('prtab.confirm').textContent).toContain(panel.prtab_close_confirm({ tag: '#412', forge: 'GitHub' }));
    expect(last('pr_act'), 'nothing before his yes').toBeUndefined();
    fireEvent.click(v.getByTestId('prtab.confirm.yes'));
    expect(last('pr_act')).toMatchObject({ act: 'close', url: URL });
  });

  it('the Summary: what it waits for, then the detail; a late answer in one line with a retry', () => {
    const { v, store, last } = shell(status());
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
