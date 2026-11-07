// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { CoreCmd } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { HubNoteRow, isFlowNote } from './confirm';

afterEach(cleanup);

const P = 'acme-1a2b3c4d';

function setup() {
  const sent: CoreCmd[] = [];
  const store = new Store({ send: (c) => sent.push(c) }, { now: () => 1000 });
  store.setCore('up');
  render(
    <StoreProvider store={store}>
      <HubNoteRow project={P} />
    </StoreProvider>,
  );
  return { sent, store };
}

// /archive's yes/no strip went (drop-card, architect m_37339): his /archive asks with the inbox's
// 1/2 card; the hub's plain lines stay the thread's quiet row
describe('HubNoteRow', () => {
  it("the hub's plain line is the thread's quiet row, not a box under the composer", () => {
    const { store } = setup();
    expect(screen.queryByTestId('confirm.notice')).toBeNull();
    act(() => {
      store.apply({ ev: 'notice', project: P, cmd: 'slash', text: '@x stays: nothing archived' });
    });
    expect(screen.getByTestId('confirm.notice').textContent).toBe('@x stays: nothing archived');
  });

  it("no mark and no empty mark column: its words start on the column's left edge (designer m_55413)", () => {
    const { store } = setup();
    act(() => {
      store.apply({ ev: 'notice', project: P, cmd: 'answer', text: 'sent to disk-audit: "on peut essayer". the drafts still wait for you.' });
    });
    const row = screen.getByTestId('confirm.notice');
    expect(row.querySelector('.b-rowmark')).toBeNull();
    expect(row.firstElementChild?.textContent).toBe('sent to disk-audit: "on peut essayer". the drafts still wait for you.');
  });

  it("/flow's answer shows until the flow is known, then the composer's strip says it (designer m_43996)", () => {
    const { store } = setup();
    act(() => {
      store.apply({ ev: 'notice', project: P, cmd: 'slash', text: 'flow: lands on main · straight to main' });
    });
    expect(screen.getByTestId('confirm.notice').textContent).toBe('flow: lands on main · straight to main');
    act(() => {
      store.apply({ ev: 'flow', project: P, flow: 'trunk' });
    });
    expect(screen.queryByTestId('confirm.notice')).toBeNull();
    expect(isFlowNote('flow: lands via PRs · a PR per task')).toBe(true);
    expect(isFlowNote('@x stays: nothing archived')).toBe(false);
  });

  describe('his /restart row (ambient-lead m_22833, his m_22835)', () => {
    const HUB = 'restarting the hub on 041168b2 · the agents keep running';
    const withApp = (state: 'current' | 'staged' | 'none', live: string | null) => {
      const swap = vi.fn((): Promise<{ ok: true } | { ok: false; why: string }> => Promise.resolve(state === 'staged' ? { ok: true } : { ok: false, why: 'nothing is staged' }));
      const update = () => Promise.resolve({ state, live, staged: state === 'staged' ? '5a5a5a5a' : null, why: null });
      (window as unknown as { bise?: unknown }).bise = { app: { update, swap } };
      return swap;
    };
    afterEach(() => {
      delete (window as unknown as { bise?: unknown }).bise;
    });
    const restart = async (store: Store, sent: CoreCmd[], hub = '041168b2') => {
      store.command({ cmd: 'slash', project: P, agent: 'main', line: '/restart current' });
      const cmd = sent.at(-1);
      const cid = cmd?.cmd === 'slash' ? cmd.cid : undefined;
      expect(cid, 'his /restart goes with a cid').toBeTypeOf('number');
      await act(async () => {
        store.apply({ ev: 'versions', project: P, current: hub, items: [] });
        store.apply({ ev: 'notice', project: P, cmd: 'slash', text: HUB, ...(cid === undefined ? {} : { cid }) });
        await Promise.resolve();
      });
    };
    const hello = (store: Store) => {
      act(() => {
        store.apply({ ev: 'welcome', project: P, proto: 1, workspace: '/w', name: 'acme' });
      });
    };

    it('staged: the newer app is ready, ⏎ restart to update swaps; it stays once the hub is back', async () => {
      const swap = withApp('staged', 'e91f98d7836a1b2c');
      const { sent, store } = setup();
      await restart(store, sent);
      const row = await screen.findByTestId('confirm.notice.app');
      expect(screen.getByTestId('confirm.notice').textContent).toBe(`${HUB} · this app is e91f98d7, a newer one is ready ⏎ restart to update`);
      expect(row.textContent).toBe(' · this app is e91f98d7, a newer one is ready');
      await act(async () => {
        fireEvent.click(screen.getByTestId('confirm.notice.swap'));
        await Promise.resolve();
      });
      expect(swap).toHaveBeenCalledTimes(1);
      hello(store);
      expect(screen.getByTestId('confirm.notice').textContent).toBe('this app is e91f98d7, a newer one is ready ⏎ restart to update');
    });

    it('nothing staged, another version: it updates at the next try; the row goes once the hub is back', async () => {
      withApp('current', 'e91f98d7836a1b2c');
      const { sent, store } = setup();
      await restart(store, sent);
      await screen.findByTestId('confirm.notice.app');
      expect(screen.getByTestId('confirm.notice').textContent).toBe(`${HUB} · this app is e91f98d7 · it updates at the next try`);
      expect(screen.queryByTestId('confirm.notice.swap')).toBeNull();
      hello(store);
      expect(screen.queryByTestId('confirm.notice')).toBeNull();
    });

    it("the same version: the hub's line alone, gone once the hub is back", async () => {
      withApp('current', '041168b2aaaa');
      const { sent, store } = setup();
      await restart(store, sent);
      await act(async () => {
        await Promise.resolve();
      });
      expect(screen.getByTestId('confirm.notice').textContent).toBe(HUB);
      hello(store);
      expect(screen.queryByTestId('confirm.notice')).toBeNull();
    });
  });
});
