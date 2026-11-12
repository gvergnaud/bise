// @vitest-environment happy-dom
// The gallery opens on For you (page artifacts-signal v3, designer m_60689): Kept then Open work
// under their heads, Done and Between agents one click away and a history step, ← For you back;
// ◆ on a kept card; the card menu's Keep sends the hub its command.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import type { Artifact } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { RouterContext, createRouter } from '@/app/router';
import { library } from '@/proto/words_library.gen';
import { ArtifactsScreen } from './index';

afterEach(cleanup);

const art = (id: string, extra: Partial<Artifact>): Artifact => ({ id, kind: 'doc', title: id, agent: 'perf', version: 1, at_ms: 1_000, url: '', new: false, ...extra });

function mount() {
  const store = new Store(null);
  store.apply({ ev: 'projects', projects: [{ project: 'shop-1', name: 'storefront', path: '/x/shop' }] });
  store.apply({
    ev: 'artifacts',
    project: 'shop-1',
    items: [
      art('plan', { section: 'open', touched_ms: 5_000 }),
      art('notebook', { section: 'kept', kept: { by: 'learnings', at_ms: 1 }, touched_ms: 2_000 }),
      art('old-plan', { section: 'done', done: true, touched_ms: 3_000 }),
      art('shots', { section: 'agents', audience: 'agents', touched_ms: 9_000 }),
    ],
  });
  const command = vi.spyOn(store, 'command');
  const router = createRouter({ screen: 'artifacts', project: '' });
  render(<StoreProvider store={store}><RouterContext.Provider value={router}>{<ArtifactsScreen />}</RouterContext.Provider></StoreProvider>);
  return { command, router };
}

describe('the sections', () => {
  it('For you: Kept then Open work under their heads; Done and Between agents one click away, and back', () => {
    const { router } = mount();
    const grid = screen.getByTestId('artifacts.grid');
    expect(grid.textContent).toContain(`${library.sect_kept} · 1`);
    expect(grid.textContent).toContain(`${library.sect_open} · 1`);
    expect(grid.textContent).not.toContain('old-plan');
    // no Recent strip over For you: its sections are newest first already (designer m_61101)
    expect(screen.queryByTestId('artifacts.recent')).toBeNull();
    expect(grid.textContent).not.toContain('shots');
    const quiet = screen.getByTestId('artifacts.quiet');
    expect(within(quiet).getByTestId('artifacts.quiet.done').textContent).toBe(`${library.sect_done} · 1 ▸`);
    expect(within(quiet).getByTestId('artifacts.quiet.agents').textContent).toBe(`${library.sect_agents} · 1 ▸`);
    // a click opens Between agents: a route step, the back row over it, no quiet rows
    fireEvent.click(within(quiet).getByTestId('artifacts.quiet.agents'));
    expect(router.get()).toEqual({ screen: 'artifacts', project: '', at: 'agents' });
  });

  it('Between agents is one flat list: its back row, no Recent strip over it, no empty Pull requests chip', () => {
    const store = new Store(null);
    store.apply({ ev: 'projects', projects: [{ project: 'shop-1', name: 'storefront', path: '/x/shop' }] });
    store.apply({ ev: 'artifacts', project: 'shop-1', items: [art('shots', { section: 'agents', audience: 'agents' }), art('pr-1', { kind: 'pr', section: 'open' })] });
    render(<StoreProvider store={store}><RouterContext.Provider value={createRouter({ screen: 'artifacts', project: '', at: 'agents' })}>{<ArtifactsScreen at="agents" />}</RouterContext.Provider></StoreProvider>);
    expect(screen.getByTestId('artifacts.back.go').textContent).toBe(library.back_for_you);
    expect(screen.queryByTestId('artifacts.recent')).toBeNull();
    expect(screen.queryByTestId('artifacts.quiet')).toBeNull();
    expect(screen.getByTestId('artifacts.grid').textContent).toContain('shots');
    expect(screen.getByTestId('artifacts.grid').textContent).not.toContain('pr-1');
    expect(screen.getByTestId('artifacts.toolbar').textContent).not.toContain(library.kind_prs);
  });

  it("◆ on a kept card, its agent on the line; the menu's Keep sends artifacts_keep", () => {
    const { command } = mount();
    // the group head is 'artifacts.grid.day.kept'; the mark is the card's own
    const marks = screen.getByTestId('artifacts.grid').querySelectorAll('.art-kept');
    expect([...marks].map((m) => [m.textContent, m.getAttribute('data-by')])).toEqual([['◆', 'learnings']]);
    expect(screen.getByTestId('artifacts.grid').textContent).toContain(library.kept_by({ agent: 'learnings' }));
    const card = within(screen.getByTestId('artifacts.grid')).getAllByRole('option').find((el) => el.getAttribute('data-row') === 'shop-1/plan');
    expect(card).toBeDefined();
    fireEvent.contextMenu(card as HTMLElement, { clientX: 10, clientY: 10 });
    fireEvent.click(screen.getByTestId('artifacts.menu.keep'));
    expect(command).toHaveBeenCalledWith({ cmd: 'artifacts_keep', project: 'shop-1', ids: ['plan'], keep: true });
  });
});
