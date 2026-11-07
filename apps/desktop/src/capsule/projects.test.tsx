// @vitest-environment happy-dom
// The tray menu's projects block (designer m_27872): which lines show, their tips, the built menu.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import type { SpotData, SpotProject } from '@/spotlight';
import { common } from '@/proto/words.gen';
import { panel } from '@/proto/words_panel.gen';
import { Capsule } from './Capsule';
import { MENU_PROJECTS, menuCounts, projectLines } from './projects';

afterEach(cleanup);

const P = (id: string, waits = 0, working = 0, more: Partial<SpotProject> = {}): SpotProject => ({ id, name: id, path: `/p/${id}`, branch: 'main', agents: 0, waits, working, ...more });

/** a normal day: bise + 4 projects */
const day: SpotData = {
  agents: [], cards: [],
  home: { waits: 1, working: 2 },
  projects: [P('harness', 1, 3), P('site', 0, 1), P('dotfiles'), P('notes', 0, 0, { missing: true, working: 2 })],
};

/** 9 projects, 2 live past the 6th */
const many: SpotData = {
  agents: [], cards: [], home: { waits: 0, working: 0 },
  projects: [P('a'), P('b', 0, 1), P('c'), P('d'), P('e'), P('f'), P('g', 2), P('h'), P('i', 0, 4)],
};

const menu = (spot: SpotData, onOpenWindow = vi.fn()) =>
  render(
    <Capsule
      view={{ kind: 'menu' }} spot={spot} answer={null} heard="" level={0}
      card={() => null} route={null} onRouteKeep={vi.fn()} onRouteTo={vi.fn()} onAsk={vi.fn()} onOpen={vi.fn()}
      onOpenWindow={onOpenWindow} onClose={vi.fn()} onQuit={null} onSize={vi.fn()}
    />,
  );

describe("the tray menu's projects (designer m_27872)", () => {
  it("bise's line first, then his projects in his order; the hub's counts; a gone folder is quiet", () => {
    const { lines, more } = projectLines(day);
    expect(lines.map((l) => [l.name, l.waits, l.working])).toEqual([['bise', 1, 2], ['harness', 1, 3], ['site', 0, 1], ['dotfiles', 0, 0], ['notes', 0, 2]]);
    expect(more).toBe(0);
    expect(lines.map((l) => l.route)).toEqual(['bise', 'project/harness', 'project/site', 'project/dotfiles', 'project/notes']);
    expect(lines[1]?.tip).toBe(['harness', common.waiting_for_you({ n: 1 }), common.agents_at_work({ n: 3 }), panel.tray_on_branch({ branch: 'main' })].join(' · '));
    expect(lines[3]?.tip).toBe(['dotfiles', common.all_quiet, panel.tray_on_branch({ branch: 'main' })].join(' · '));
    expect(lines[4]?.tip).toBe(['notes', common.all_quiet, panel.tray_on_branch({ branch: 'main' })].join(' · '));
    expect(lines[0]?.tip).toBe(['bise', common.waiting_for_you({ n: 1 }), common.agents_at_work({ n: 2 })].join(' · '));
  });

  it(`at most ${String(MENU_PROJECTS)} projects, never a live one hidden: quiet ones fill the rest, in his order`, () => {
    const { lines, more } = projectLines(many);
    expect(lines.map((l) => l.name)).toEqual(['bise', 'a', 'b', 'c', 'd', 'g', 'i']);
    expect(more).toBe(3);
    // more live than the cap: every live one, no quiet one
    const busy: SpotData = { agents: [], cards: [], projects: Array.from({ length: 8 }, (_, i) => P(`p${String(i)}`, 0, 1)) };
    expect(projectLines(busy)).toMatchObject({ more: 0 });
    expect(projectLines(busy).lines).toHaveLength(8);
    // no home known: no bise line
    expect(projectLines(busy).lines[0]?.name).toBe('p0');
  });

  it("the built menu: one row per line, the mark, the end '? n' then '∿ n' (none when quiet), the tip, a click opens it", () => {
    const onOpenWindow = vi.fn();
    const ui = menu(day, onOpenWindow);
    const block = ui.getByTestId('capsule.menu.projects');
    expect(block.textContent.startsWith(panel.projects)).toBe(true);
    // a live line ends with '›', its submenu (designer m_29577)
    expect(ui.getByTestId('capsule.menu.project.harness').textContent).toBe('?harness? 1∿ 3›');
    expect(ui.getByTestId('capsule.menu.project.site').querySelector('.b-cap-pj-end')?.textContent).toBe('∿ 1›');
    expect(ui.getByTestId('capsule.menu.project.dotfiles').querySelector('.b-cap-pj-end')).toBeNull();
    expect(ui.getByTestId('capsule.menu.project.notes').querySelector('.b-cap-pj-end')).toBeNull();
    expect(ui.getByTestId('capsule.menu.project.dotfiles.line').className).toBe('quiet');
    expect(ui.getByTestId('capsule.menu.project.harness.line').getAttribute('title')).toBe(projectLines(day).lines[1]?.tip);
    expect(ui.getByTestId('capsule.menu.project.bise').querySelector('.b-cap-pj-end')?.textContent).toBe('? 1∿ 2');
    expect(ui.queryByTestId('capsule.menu.projects.more')).toBeNull();
    // the head reads the same counts as the lines (designer m_27930): a gone folder's out
    expect(ui.getByTestId('capsule.menu.head').textContent).toBe('6 at work · 2 waiting for you');
    expect(menuCounts(day)).toEqual({ work: 6, waits: 2 });
    expect(menuCounts({ agents: [{ name: 'a', project: 'x', status: 'working', title: '' }], projects: [], cards: [] })).toEqual({ work: 1, waits: 0 });
    ui.getByTestId('capsule.menu.project.harness').click();
    ui.getByTestId('capsule.menu.project.bise').click();
    expect(onOpenWindow.mock.calls).toEqual([['project/harness'], ['bise']]);
  });

  it("the last row counts what's left out and opens bise; no project known, no block", () => {
    const onOpenWindow = vi.fn();
    const ui = menu(many, onOpenWindow);
    expect(ui.getByTestId('capsule.menu.projects.more').textContent).toBe(panel.tray_more({ n: 3 }));
    ui.getByTestId('capsule.menu.projects.more').click();
    expect(onOpenWindow).toHaveBeenCalledWith();
    ui.unmount();
    expect(menu({ agents: [], projects: [], cards: [] }).queryByTestId('capsule.menu.projects')).toBeNull();
  });
});

describe("a live line's summary and its submenu (designer m_29577)", () => {
  const live: SpotData = {
    home: { id: 'home', waits: 0, working: 1 },
    projects: [P('shop', 1, 2), P('docs')],
    agents: [
      { name: 'main', project: 'home', status: 'working', title: '', main: true, note: 'watching the launch' },
      { name: 'perf', project: 'shop', status: 'working', title: 'cache', note: 'merging the cache fix', since: 1000, sign: { g: '∿', tone: 'ink' } },
      { name: 'web', project: 'shop', status: 'working', title: 'the web build', since: 500, sign: { g: '∿', tone: 'ink' } },
      { name: 'search', project: 'shop', status: 'idle', title: 'search', since: 10, sign: { g: '○', tone: 'dim' } },
    ],
    cards: [{ id: 'shop.4', card: 4, agent: 'search', project: 'shop', text: 'which size system should the filter use?', kind: 'question' }],
  };
  const key = (k: string) => {
    fireEvent.keyDown(window, { key: k });
  };

  it("the second line: what waits first, bise's note; a quiet line has none", () => {
    const ui = menu(live);
    // its whole words: the cut (whole words, measured) has no room here (happy-dom lays out nothing), the tip keeps them
    const words = (id: string): string | null | undefined => ui.getByTestId(`capsule.menu.project.${id}.sum`).querySelector('.t')?.getAttribute('title');
    expect(ui.getByTestId('capsule.menu.project.shop.sum').querySelector('.w')?.textContent).toBe('?');
    expect(words('shop')).toBe('search · which size system should the filter use?');
    expect(words('bise')).toBe('watching the launch');
    expect(ui.queryByTestId('capsule.menu.project.docs.sum')).toBeNull();
    expect(ui.getByTestId('capsule.menu.project.docs.line').className).toBe('quiet');
  });

  it('keys: ↓ selects, → opens the submenu on its first row, ↓ moves in it, ⏎ opens that thread, ← and esc close it', () => {
    const onOpenWindow = vi.fn();
    const onClose = vi.fn();
    const ui = render(
      <Capsule
        view={{ kind: 'menu' }} spot={live} answer={null} heard="" level={0}
        card={() => null} route={null} onRouteKeep={vi.fn()} onRouteTo={vi.fn()} onAsk={vi.fn()} onOpen={vi.fn()}
        onOpenWindow={onOpenWindow} onClose={onClose} onQuit={null} onSize={vi.fn()}
      />,
    );
    // the inbox row, then bise, then shop
    for (let i = 0; i < 3; i++) act(() => { key('ArrowDown'); });
    expect(ui.getByTestId('capsule.menu.project.shop').className).toContain('on');
    act(() => { key('ArrowRight'); });
    expect(ui.getByTestId('capsule.menu.sub.head').textContent).toBe('shop2 working · 1 waiting for you');
    // waiting on him first, then at work newest first; never the idle one twice
    expect(ui.getAllByTestId(/^capsule\.menu\.sub\.agent\./u).map((e) => e.getAttribute('data-testid'))).toEqual(['search', 'perf', 'web'].map((n) => `capsule.menu.sub.agent.${n}`));
    expect(ui.getByTestId('capsule.menu.sub.agent.search').className).toContain('on');
    act(() => { key('ArrowDown'); });
    act(() => { key('Enter'); });
    expect(onOpenWindow).toHaveBeenLastCalledWith('agent/shop/perf');
    act(() => { key('ArrowLeft'); });
    expect(ui.queryByTestId('capsule.menu.sub')).toBeNull();
    act(() => { key('ArrowRight'); });
    expect(ui.getByTestId('capsule.menu.sub')).toBeTruthy();
    // esc: the submenu first, then the menu
    act(() => { key('Escape'); });
    expect(ui.queryByTestId('capsule.menu.sub')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    vi.useFakeTimers({ now: Date.now() + 1000 });
    act(() => { key('Escape'); });
    vi.useRealTimers();
    expect(onClose).toHaveBeenCalledTimes(1);
    // a quiet line has no submenu
    act(() => { key('ArrowDown'); });
    act(() => { key('ArrowRight'); });
    expect(ui.getByTestId('capsule.menu.project.docs').className).toContain('on');
    expect(ui.queryByTestId('capsule.menu.sub')).toBeNull();
  });

  it('pointer: resting on a live line opens its submenu, its last row opens the project; a quiet line closes it', () => {
    vi.useFakeTimers();
    try {
      const onOpenWindow = vi.fn();
      const ui = menu(live, onOpenWindow);
      fireEvent.mouseMove(ui.getByTestId('capsule.menu.project.shop'));
      expect(ui.queryByTestId('capsule.menu.sub')).toBeNull();
      act(() => { vi.advanceTimersByTime(160); });
      expect(ui.getByTestId('capsule.menu.sub')).toBeTruthy();
      expect(ui.getByTestId('capsule.menu.sub.open').textContent).toBe('open shop');
      fireEvent.mouseMove(ui.getByTestId('capsule.menu.sub.open'));
      expect(ui.getByTestId('capsule.menu.sub.open').className).toContain('on');
      ui.getByTestId('capsule.menu.sub.open').click();
      expect(onOpenWindow).toHaveBeenLastCalledWith('agent/shop/main');
      fireEvent.mouseMove(ui.getByTestId('capsule.menu.project.docs'));
      act(() => { vi.advanceTimersByTime(400); });
      expect(ui.queryByTestId('capsule.menu.sub')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("the tray menu's waiting items (spotlight/menu, one-inbox 3, designer m_19631)", () => {
  // 7 cards in the block's order (data.ts: the hub's rank, then the oldest)
  const cards: SpotData['cards'] = Array.from({ length: 7 }, (_, i) => ({
    id: `shop.${String(i + 1)}`,
    card: i + 1,
    agent: i === 0 ? 'main' : `a${String(i + 1)}`,
    project: 'shop',
    text: `question ${String(i + 1)}?\nits second line`,
    kind: i === 1 ? ('approval' as const) : ('question' as const),
    pname: 'shop',
  }));
  const spot: SpotData = { agents: [], projects: [], cards };

  it("shows 5 rows in the block's order, '<n> <sign> <agent> <first line> <project>', then '+ 2 more in the inbox'", () => {
    const onOpenWindow = vi.fn();
    const onOpenCard = vi.fn();
    const ui = render(
      <Capsule
        view={{ kind: 'menu' }} spot={spot} answer={null} heard="" level={0}
        card={() => null} route={null} onRouteKeep={vi.fn()} onRouteTo={vi.fn()} onAsk={vi.fn()} onOpen={vi.fn()}
        onOpenWindow={onOpenWindow} onOpenCard={onOpenCard} onClose={vi.fn()} onQuit={null} onSize={vi.fn()}
      />,
    );
    const rows = [...ui.getByTestId('capsule.menu.items').querySelectorAll('[data-testid^="capsule.menu.item."]')];
    expect(rows.map((r) => r.getAttribute('data-testid'))).toEqual([1, 2, 3, 4, 5].map((n) => `capsule.menu.item.${String(n)}`));
    // main reads 'bise'; an approval its '!'; the first line only, no age
    expect(rows[0]?.textContent).toBe('1?bisequestion 1?shop');
    expect(rows[1]?.textContent).toBe('2!a2question 2?shop');
    expect(rows[4]?.textContent).toBe('5?a5question 5?shop');
    expect(ui.getByTestId('capsule.menu.more').textContent).toBe('+ 2 more in the inbox');
    fireEvent.click(rows[2] as Element);
    expect(onOpenCard).toHaveBeenCalledWith('shop.3');
    fireEvent.click(ui.getByTestId('capsule.menu.more'));
    expect(onOpenWindow).toHaveBeenCalledWith('inbox');
  });
});
