// @vitest-environment happy-dom
// The tabbed panel drawn (native-mac-qa §7, §12's testids): folded by default, ▯ unfolds it on its
// tabs (none: a New tab; the Overview left it for the outer card, page outer-side v2), '+' opens New tab whose rows act, × closes a tab, the last one folds the panel, a tab
// in front always shows past six.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { useLayoutEffect } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { artifact, artifacts, ev, projects, row } from '../../../tests/feature/inputs';
import { createRouter, RouterContext } from '@/app/router';
import { BarTabs } from './BarTabs';
import { PanelHost } from './Host';
import { panelState, panelToggle, reopenClosed, setFull, usePanelScope, usePanelState } from './state';
import { PANEL_FULL_KEY } from './keys';
import { artFit, shareOf } from './model';
import { openTab } from './tabs';
import { PANEL_KEYS } from './keys';
import { panel } from '@/proto/words_panel.gen';

afterEach(cleanup);
beforeEach(() => {
  panelState.reset();
  // a wide window: the panel's width is what its tab asks (fitW keeps the thread 560 beside the left)
  Object.defineProperty(window, 'innerWidth', { value: 1920, configurable: true });
});
const ACME = 'acme-1a2b3c4d';
/** a PR's link, its tab's key (forge::pr_link's one form) */
const PR = (n: number) => `https://github.com/acme/web/pull/${String(n)}`;

function shell(more: { events?: readonly unknown[]; bise?: Record<string, unknown> } = {}) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const notify = { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined };
  (window as unknown as { bise: unknown }).bise = { notify, ...more.bise };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: 'bise-home-00000000', name: 'bise', home: true }), row({ project: ACME, name: 'acme' })));
  // the fixtures' agents: perf has a diff (3 files), so New tab shows Changes
  store.apply(ev('agents', { project: ACME }));
  for (const e of more.events ?? []) store.apply(e);
  const router = createRouter({ screen: 'thread', project: ACME, agent: 'perf' });
  return render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <BarTabs />
        <PanelHost />
      </RouterContext>
    </StoreProvider>,
  );
}

describe('the tabbed panel', () => {
  it('is folded by default: ▯ in the bar, the panel 0 wide and inert (no Tab, no VoiceOver)', () => {
    const v = shell();
    expect(v.getByTestId('app.panel.toggle').getAttribute('aria-pressed')).toBe('false');
    expect(v.getByTestId('app.panel').style.width).toBe('0px');
    expect(v.getByTestId('app.panel').hasAttribute('inert')).toBe(true);
    expect(v.queryByTestId('app.right')).toBeNull();
  });
  it('▯ with no tab unfolds it on a New tab at 384 (designer m_46177), its tabs in its own top row; again folds it', () => {
    const v = shell();
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    expect(v.getByTestId('app.panel.tab.new:1').getAttribute('aria-selected')).toBe('true');
    expect(v.getByTestId('app.panel.top').contains(v.getByTestId('app.panel.tabs'))).toBe(true);
    expect(v.getByTestId('app.panel').contains(v.getByTestId('app.panel.top'))).toBe(true);
    expect(v.getByTestId('app.panel').style.width).toBe('384px');
    expect(v.getByTestId('app.panel').hasAttribute('inert')).toBe(false);
    expect(v.queryByTestId('app.right')).toBeNull();
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    expect(v.getByTestId('app.panel').style.width).toBe('0px');
  });
  it("'+' opens New tab: search, Open's rows with their keys, Recent", () => {
    const v = shell();
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    fireEvent.click(v.getByTestId('app.panel.add'));
    // the New tab the unfold opened is the one '+' brings back (his m_46959)
    expect(v.getByTestId('app.panel.tab.new:1').getAttribute('aria-selected')).toBe('true');
    expect(v.queryByTestId('app.panel.tab.new:2')).toBeNull();
    expect(v.getByTestId('app.panel.launcher.search')).toBeTruthy();
    for (const k of ['artifacts', 'terminal', 'browser', 'office']) expect(v.getByTestId(`app.panel.launcher.${k}`)).toBeTruthy();
    expect(v.getByTestId('app.panel.launcher.terminal').textContent).toContain('⌃`');
    expect(v.getByTestId('app.panel.launcher.none')).toBeTruthy();
  });
  it("New tab's Changes opens the agent in view's changes in its place", () => {
    const v = shell();
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    fireEvent.click(v.getByTestId('app.panel.launcher.changes'));
    expect(v.queryByTestId('app.panel.tab.new:1')).toBeNull();
    expect(v.getByTestId(`app.panel.tab.diff:${ACME}/perf`).getAttribute('aria-selected')).toBe('true');
  });
  it('a PR’s tab says its row’s tag, the plain word before its row comes, never a number read from the link', () => {
    const v = shell();
    act(() => {
      openTab({ kind: 'pr', project: ACME, url: PR(7) });
    });
    expect(v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(7)}`).textContent).toBe(panel.tab_pr_plain);
  });
  it('openTab unfolds it on the thing; × closes it; the last one folds the panel', () => {
    const six = artifact({ id: 'pr-6', kind: 'pr', url: PR(6), pr: { repo: 'acme/web', number: 6, tag: '#6' } });
    const v = shell({ events: [artifacts(ACME, six)] });
    act(() => {
      openTab({ kind: 'pr', project: ACME, url: PR(6) });
    });
    const tab = v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(6)}`);
    expect(tab.getAttribute('aria-selected')).toBe('true');
    expect(tab.textContent).toContain('PR #6');
    // a PR's tab is its guided review (page pr-guided): it takes a PR artifact's share, never the rows' one width
    expect(v.getByTestId('app.panel').style.width).toBe(`${String(artFit(shareOf('pr', {}), window.innerWidth, 0).width)}px`);
    fireEvent.click(v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(6)}.close`));
    expect(panelState.get().tabs.list).toEqual([]);
    expect(panelState.get().open).toBe(false);
  });
  it('⇧⌘T (his m_46187, as Chrome): close two tabs, reopen twice, both come back in their place, the last closed first', () => {
    const v = shell();
    act(() => {
      openTab({ kind: 'pr', project: ACME, url: PR(6) });
      openTab({ kind: 'pr', project: ACME, url: PR(7) });
      openTab({ kind: 'pr', project: ACME, url: PR(8) });
    });
    const ids = () => panelState.get().tabs.list.map((t) => t.id);
    const all = [`pr:${ACME}/${PR(6)}`, `pr:${ACME}/${PR(7)}`, `pr:${ACME}/${PR(8)}`];
    expect(ids()).toEqual(all);
    fireEvent.click(v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(7)}.close`));
    fireEvent.click(v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(6)}.close`));
    act(() => {
      panelToggle();
    });
    expect(ids()).toEqual([`pr:${ACME}/${PR(8)}`]);
    act(() => {
      reopenClosed();
    });
    expect(panelState.get().open, 'a folded panel unfolds on it').toBe(true);
    expect(ids()).toEqual([`pr:${ACME}/${PR(6)}`, `pr:${ACME}/${PR(8)}`]);
    expect(v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(6)}`).getAttribute('aria-selected')).toBe('true');
    act(() => {
      reopenClosed();
    });
    expect(ids()).toEqual(all);
    expect(v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(7)}`).getAttribute('aria-selected')).toBe('true');
    act(() => {
      reopenClosed();
    });
    expect(ids(), 'nothing left to reopen').toEqual(all);
  });
  it('past six tabs: an overflow button, the tab in front still shows', () => {
    const v = shell();
    act(() => {
      for (let n = 1; n <= 7; n++) openTab({ kind: 'commit', project: ACME, sha: `abcdef${String(n)}` });
    });
    expect(v.getAllByRole('tab')).toHaveLength(6);
    expect(v.getByTestId('app.panel.over').textContent).toContain('1');
    expect(v.getByTestId(`app.panel.tab.commit:${ACME}/abcdef7`)).toBeTruthy();
    expect(v.getByTestId(`app.panel.tab.commit:${ACME}/abcdef7`).getAttribute('aria-selected')).toBe('true');
  });
  it('every tab has a VoiceOver name, a tip only when its title is cut; the strip has its name', () => {
    const v = shell();
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    expect(v.getByRole('tablist').getAttribute('aria-label')).toBe('Panel tabs');
    const t = v.getByTestId('app.panel.tab.new:1');
    expect(t.getAttribute('aria-label')).toBe(panel.tab_new);
    // its whole title shows: no tip repeating it (ui-review m_42619 #5)
    fireEvent.pointerEnter(t);
    expect(t.getAttribute('data-tip')).toBeNull();
    expect(v.getByTestId('app.panel.toggle').getAttribute('data-tip')).toContain('⌥⌘B');
  });
  it("a narrow window: a sheet over the thread, the bar's ▯ its only one (designer m_44391); esc or a click outside folds it (ui-review m_42642)", () => {
    Object.defineProperty(window, 'innerWidth', { value: 1100, configurable: true });
    const v = shell();
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    expect(v.getByTestId('app.panel').hasAttribute('data-over')).toBe(true);
    expect(v.getByTestId('app.panel').style.width).toBe('0px');
    // the sheet's row starts with its tabs: one 'Hide panel' ▯ in the window, the bar's (pressed)
    expect(v.getAllByLabelText(panel.panel_hide({ key: PANEL_KEYS.toggle }))).toEqual([v.getByTestId('app.panel.toggle')]);
    // (after the unseen measure row; the row of a lead, none here, and the tabs)
    expect(v.getByTestId('app.panel.top').firstElementChild?.nextElementSibling?.firstElementChild?.getAttribute('data-testid')).toBe('app.panel.tabs');
    expect(v.getByTestId('app.panel.toggle').getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    expect(v.getByTestId('app.panel').hasAttribute('data-open')).toBe(false);
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(v.getByTestId('app.panel').hasAttribute('data-open')).toBe(false);
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    fireEvent.pointerDown(document.body);
    expect(v.getByTestId('app.panel').hasAttribute('data-open')).toBe(false);
  });
  it("each project its own panel (his m_45500): a switch shows that project's tabs, never a frame of the other's", async () => {
    const BETA = 'beta-5e6f7a8b';
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    (window as unknown as { bise: unknown }).bise = { notify: { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined } };
    const store = new Store(port);
    store.setCore('up');
    store.apply(projects(row({ project: 'bise-home-00000000', name: 'bise', home: true }), row({ project: ACME, name: 'acme' }), row({ project: BETA, name: 'beta' })));
    const router = createRouter({ screen: 'thread', project: ACME, agent: 'main' });
    // what the panel drew at each commit: a frame of the old project's tabs would show here
    const drawn: string[] = [];
    function Scope() {
      usePanelScope();
      return null;
    }
    function Probe() {
      const s = usePanelState();
      useLayoutEffect(() => {
        drawn.push(s.tabs.list.map((t) => t.id).join(','));
      });
      return null;
    }
    const v = render(
      <StoreProvider store={store}>
        <RouterContext value={router}>
          <Scope />
          <BarTabs />
          <PanelHost />
        </RouterContext>
      </StoreProvider>,
    );
    act(() => {
      openTab({ kind: 'pr', project: ACME, url: PR(6) });
    });
    expect(v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(6)}`)).toBeTruthy();
    act(() => {
      router.go({ screen: 'thread', project: BETA, agent: 'main' });
    });
    // beta never opened a panel: none
    expect(v.getByTestId('app.panel').hasAttribute('data-open')).toBe(false);
    expect(v.queryByTestId(`app.panel.tab.pr:${ACME}/${PR(6)}`)).toBeNull();
    act(() => {
      openTab({ kind: 'pr', project: BETA, url: PR(9) });
    });
    // a screen of no project keeps beta's
    act(() => {
      router.go({ screen: 'settings' });
    });
    expect(v.getByTestId(`app.panel.tab.pr:${BETA}/${PR(9)}`)).toBeTruthy();
    const r = render(<Probe />);
    drawn.length = 0;
    act(() => {
      router.go({ screen: 'thread', project: ACME, agent: 'main' });
    });
    expect(v.getByTestId(`app.panel.tab.pr:${ACME}/${PR(6)}`).getAttribute('aria-selected')).toBe('true');
    expect(v.queryByTestId(`app.panel.tab.pr:${BETA}/${PR(9)}`)).toBeNull();
    expect(drawn.every((d) => !d.includes(BETA))).toBe(true);
    // its width snaps for the frame of the switch (ui-review m_45602), the ease back after it
    expect(v.getByTestId('app.panel').hasAttribute('data-snap')).toBe(true);
    await act(async () => {
      await new Promise((ok) => requestAnimationFrame(ok));
    });
    expect(v.getByTestId('app.panel').hasAttribute('data-snap')).toBe(false);
    r.unmount();
  });
  it('a wide window: no sheet, no ▯ in the panel', () => {
    const v = shell();
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    expect(v.getByTestId('app.panel').hasAttribute('data-over')).toBe(false);
    expect(v.getAllByLabelText(panel.panel_hide({ key: PANEL_KEYS.toggle }))).toEqual([v.getByTestId('app.panel.toggle')]);
  });
  it("full width: the tab row leads with the hidden thread's name, a click on it brings the thread back (designer m_45641); the button at its end too", () => {
    const v = shell();
    fireEvent.click(v.getByTestId('app.panel.toggle'));
    expect(v.queryByTestId('app.panel.lead'), 'beside the thread: no lead').toBeNull();
    act(() => {
      setFull(true);
    });
    expect(v.getByTestId('app.panel').hasAttribute('data-full')).toBe(true);
    const lead = v.getByTestId('app.panel.lead');
    expect(lead.textContent).toBe('perf');
    expect(lead.getAttribute('data-tip')).toBe(panel.panel_full_lead({ agent: 'perf', key: PANEL_FULL_KEY }));
    fireEvent.click(lead);
    expect(v.getByTestId('app.panel').hasAttribute('data-full')).toBe(false);
    expect(v.queryByTestId('app.panel.lead')).toBeNull();
    act(() => {
      setFull(true);
    });
    // its glyph is the thread's own bubble, never the sidebar's (the panel mirrored drew it; his m_47445, designer m_47468)
    const back = v.getByTestId('app.panel.thread');
    expect(Array.from(back.querySelectorAll('svg')).map((s) => s.getAttribute('data-icon'))).toEqual(['thread']);
    expect(back.querySelector('.pn-mirror')).toBeNull();
    // the lead's action, the lead's tip: the thread's owner by name (designer m_47517)
    expect(back.getAttribute('data-tip')).toBe(panel.panel_full_lead({ agent: 'perf', key: PANEL_FULL_KEY }));
    fireEvent.click(back);
    expect(v.getByTestId('app.panel').hasAttribute('data-full')).toBe(false);
  });
  it("an html file's tab: ↗ opens the hub's served page his browser takes, never the bare path (his m_45830)", () => {
    const opened: string[] = [];
    const openExternal = (u: string) => {
      opened.push(u);
      return Promise.resolve(true);
    };
    // a file artifact as the hub sends it: url the copied file's path, frame_url its page on the pages server
    const mocks = artifact({
      id: 'mocks',
      kind: 'page',
      title: 'folded side · high-fi mocks',
      version: 3,
      url: '/Users/me/.bise/hubs/h/artifacts/mocks/v3/hifi-index.html',
      frame_url: 'http://127.0.0.1:47123/a/mocks/view',
    });
    const bare = artifact({ id: 'bare', kind: 'page', url: '/Users/me/w/bare.html', frame_url: null });
    const w = shell({ events: [artifacts(ACME, mocks, bare)], bise: { openExternal } });
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: 'mocks' });
    });
    fireEvent.click(w.getByTestId('app.panel.addr.out'));
    expect(opened).toEqual(['http://127.0.0.1:47123/a/mocks/view']);
    // a bare path from an older hub, no served page: ↗ is off (openExternal refuses a path)
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: 'bare' });
    });
    expect(w.getByTestId('app.panel.addr.out').hasAttribute('disabled')).toBe(true);
  });
});
