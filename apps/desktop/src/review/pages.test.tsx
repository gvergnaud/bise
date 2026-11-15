// @vitest-environment happy-dom
// nm-pages-b's pages (bise fait mac, page native-mac §9), light and dark: each one's banner (ds
// PageBanner with its signed vignette, never the old PageHero), its rows as ds ListRows with their
// '…' (MoreButton), sentence-case group labels, one ds Empty when there is nothing, and the
// floating composer's room at the scroller's foot (the class that carries --float-room).
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { ReactElement } from 'react';
import type { Agent } from '@/proto';
import { hub_ev } from '@/proto/fixtures.gen';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { hero } from '@/proto/words_hero.gen';
import { pages } from '@/proto/words_pages.gen';
import { AgentsScreen } from '@/agents/AgentsScreen';
import { ReportScreen } from '@/app/ReportScreen';
import { ChangesScreen } from './ChangesScreen';
import { PrsScreen } from './PrsScreen';
import { EnvironmentsScreen } from './EnvironmentsScreen';

const P = 'acme-1a2b3c4d';
const ag = (name: string, over: Partial<Agent> = {}): Agent => ({ name, dir: name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: '', since_ms: 1, waits: 0, watching: [], ...over });

function world() {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  // the hub's PR rows on their places (the PRs screen reads them there)
  const prs = hub_ev.flatMap((e) => (e.ev === 'prs' ? [e.items] : [])).find((items) => items.length > 0) ?? [];
  const places = prs.map((pr) => ({ id: `wt:${pr.branch}`, branch: pr.branch, agents: pr.agents, pr }));
  store.apply({ ev: 'agents', project: P, places, agents: [ag('main'), ag('perf', { status: 'working', worktree: '/w/perf', branch: 'sb/perf' }), ag('alerts', { waits: 1 }), ag('old', { archived: true, status: 'done' })] });
  store.apply({ ev: 'worktrees', project: P, items: [{ path: '/w/perf', branch: 'sb/perf', agent: 'perf', ahead: 2, behind: 0, dirty: true }] });
  const wrap = (ui: ReactElement) => (
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'agents', project: P })}>{ui}</RouterContext>
    </StoreProvider>
  );
  return { store, wrap };
}

afterEach(() => {
  cleanup();
  document.documentElement.removeAttribute('data-theme');
});

const PAGES = [
  { name: 'agents', ui: <AgentsScreen project={P} />, lead: 'agents.lead', title: hero.agents_title, vignette: 'agents', scroll: 'ag-scroll' },
  { name: 'archived', ui: <AgentsScreen project={P} archived />, lead: 'agents.lead', title: hero.archived_title, vignette: 'archived', scroll: 'ag-scroll' },
  { name: 'changes', ui: <ChangesScreen project={P} />, lead: 'changes.lead', title: hero.changes_title, vignette: 'changes', scroll: 'rv-scroll' },
  { name: 'prs', ui: <PrsScreen project={P} />, lead: 'prs.lead', title: hero.prs_title, vignette: 'prs', scroll: 'rv-scroll' },
  { name: 'environments', ui: <EnvironmentsScreen project={P} />, lead: 'environments.lead', title: hero.environments_title, vignette: 'environments', scroll: 'rv-scroll' },
  { name: 'crash report', ui: <ReportScreen />, lead: 'app.report.lead', title: hero.crash_title, vignette: 'crash', scroll: 'app-report-scroll' },
] as const;

describe.each(['light', 'dark'] as const)('the pages in %s', (theme) => {
  it.each(PAGES)('$name: its banner with its vignette, no hero card, the float room at its foot', ({ ui, lead, title, vignette, scroll }) => {
    document.documentElement.setAttribute('data-theme', theme);
    const { wrap } = world();
    const v = render(wrap(ui));
    const banner = v.getByTestId(lead);
    expect(banner.classList.contains('b-banner')).toBe(true);
    expect(banner.querySelector('.title')?.textContent).toBe(title);
    // both stills are there; the theme's CSS shows one (ds banner.css)
    expect([...banner.querySelectorAll('img')].map((i) => [i.className, i.getAttribute('data-vignette')])).toEqual([['light', vignette], ['dark', vignette]]);
    expect(v.container.querySelector('.b-hero')).toBeNull();
    // the banner first in the page's one scroller
    expect(v.container.querySelector(`.${scroll}`)?.firstElementChild).toBe(banner);
  });
});

describe("the pages' rows and labels", () => {
  it('agents: ListRows with their menu, the accent only on the one that asks him, the filter shows both lists', () => {
    const { wrap } = world();
    const v = render(wrap(<AgentsScreen project={P} />));
    // the ask line icon in the accent, never a '?' character (ui-review m_44027)
    expect(v.getByTestId('agents.row.alerts').querySelector('.b-lrow .b-rowmark[data-icon="ask"]')?.getAttribute('data-tone')).toBe('accent');
    expect(v.getByTestId('agents.row.perf').querySelector('.b-rowmark[data-icon="ask"]')).toBeNull();
    for (const a of ['main', 'perf', 'alerts']) expect(v.getByTestId(`agents.more.${a}`).classList.contains('b-more')).toBe(true);
    expect(v.queryByTestId('agents.row.old')).toBeNull();
    expect(v.getByTestId('agents.filter.live').textContent).toBe(`${pages.agents_live}3`);
    expect(v.getByTestId('agents.filter.archived').textContent).toBe(`${pages.agents_archived}1`);
    expect(v.getByTestId('agents.lead').querySelector('.wait')?.textContent).toBe(hero.wait_for_you({ n: 1 }));
  });

  it('changes: sentence-case group labels, a branch row with its menu, one Empty when nothing merged', () => {
    const { wrap } = world();
    const v = render(wrap(<ChangesScreen project={P} />));
    const labels = [...v.container.querySelectorAll('.b-glabel')].map((g) => g.firstChild?.textContent);
    expect(labels).toEqual([pages.group_branches, pages.group_merged]);
    for (const l of labels) expect(l?.charAt(0)).toBe(l?.charAt(0).toUpperCase());
    expect(v.getByTestId('changes.branch.perf.more')).toBeTruthy();
    expect(v.getByTestId('changes.merged.none').classList.contains('b-empty')).toBe(true);
  });

  it('environments: a group per place, its agents and servers as ListRows', () => {
    const { wrap, store } = world();
    act(() => {
      store.apply({ ev: 'processes', project: P, items: [] });
    });
    const v = render(wrap(<EnvironmentsScreen project={P} />));
    expect([...v.container.querySelectorAll('.b-glabel')].map((g) => g.textContent)).toEqual(['sb/perf · 2 ahead · changed', pages.shared_folder]);
    expect(v.getByTestId('environments.agent.perf').classList.contains('b-lrow')).toBe(true);
  });

  it('the crash report: none is one Empty, no actions', () => {
    const v = render(<ReportScreen />);
    expect(v.getByTestId('app.report.none').textContent).toContain(pages.no_crash);
    expect(v.queryByTestId('app.report.copy')).toBeNull();
  });
});
