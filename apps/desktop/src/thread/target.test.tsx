import type { ReactNode } from 'react';
// @vitest-environment happy-dom
// Whom his words go to, clear at a glance (his ask m_44704, as main's tab had it, designer m_28208):
// a thread of an agent that is not main says 'to perf' first in its composer's strip, in the
// accent; main's own thread 'to bise' text-2. That name is a picker of the project's agents, never
// cut (his m_47126, designer m_47129); no project in the strip, the sidebar's (his m_50754). The
// strip as ThreadScreen draws it (the field mocked, its head the frame's `.head`).
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { ThreadScreen } from './ThreadScreen';
import { toAccent } from './ThreadComposer';

vi.mock('@/composer', () => ({
  Composer: ({ head }: { head?: ReactNode }) => <div data-testid="composer"><div className="head" style={{ padding: 0, columnGap: 0 }}>{head}</div></div>,
  useComposerSources: () => ({ agents: [], skills: [], commands: [], files: () => Promise.resolve([]), saveImage: () => Promise.resolve(null) }),
  useApprovalsFlash: () => false,
  flashWords: () => ({ word: '', rest: '' }),
}));

afterEach(cleanup);

const P = 'acme-1a2b3c4d';
const ag = (name: string, over: Partial<Agent> = {}): Agent => ({ name, dir: name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: '', since_ms: 1, waits: 0, watching: [], ...over });

function strip(agent: string, over: Partial<Agent> = {}, router = createRouter({ screen: 'thread', project: P, agent })) {
  const store = new Store(null);
  act(() => {
    store.apply({ ev: 'projects', projects: [{ project: P, name: 'acme', path: '/x/acme' }] });
    store.apply({ ev: 'agents', project: P, agents: [ag('main', agent === 'main' ? over : {}), ag('native-lead', agent === 'main' ? {} : over), ag('perf', { status: 'working' }), ag('old', { archived: true })] });
  });
  return render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <ThreadScreen project={P} agent={agent} />
      </RouterContext>
    </StoreProvider>,
  );
}

describe("the composer names an agent that is not main, in the accent (his ask m_44704)", () => {
  it("a sub-agent's thread: 'to native-lead' first in the strip, the accent's item", () => {
    const v = strip('native-lead');
    const to = v.getByTestId('composer.ctx.to');
    expect(to.textContent).toBe('to native-lead');
    // the accent is the `.it.to` item's (composer.css): its class, the strip's first item
    expect(to.classList.contains('to')).toBe(true);
    expect(v.getByTestId('composer.ctx').firstElementChild).toBe(to);
  });

  it("an open item his words close ('to perf · the approval waits for 1 or 2'): 'to perf' in the accent too, never main's, never over a question's own pink (designer m_44890)", () => {
    // pinkState gives an item its asker closes the border alone (quiet), main's plain, a question the full pink
    expect(toAccent({ quiet: true }, null)).toBe(true);
    expect(toAccent({ quiet: false }, null)).toBe(false);
    expect(toAccent({ quiet: true }, 'docs')).toBe(false);
  });

  it("main's own thread: 'to bise' first, text-2, never the accent (designer m_47129)", () => {
    const v = strip('main');
    const to = v.getByTestId('composer.ctx.to');
    expect(to.textContent).toBe('to bise');
    expect(to.classList.contains('acc')).toBe(false);
    expect(v.getByTestId('composer.ctx.to').closest('.cmp-ctx')?.firstElementChild).toBe(to);
  });
  it("a sub-agent's name is the accent's (its `acc` class)", () => {
    expect(strip('native-lead').getByTestId('composer.ctx.to').classList.contains('acc')).toBe(true);
  });
});

// his m_47126 (4): the name he talks to is a picker of the project's agents. Mutation: AgentItem
// drawn as a plain label again (no onPick) turns it red.
describe("the strip's name is a picker of the project's agents", () => {
  it("opens bise first, then the live agents with their signs, the current one checked; a pick opens that agent's thread", () => {
    const router = createRouter({ screen: 'thread', project: P, agent: 'native-lead' });
    const v = strip('native-lead', {}, router);
    fireEvent.click(v.getByTestId('composer.ctx.to'));
    const menu = v.getByTestId('composer.ctx.to.menu');
    const rows = Array.from(menu.querySelectorAll<HTMLElement>('[data-testid^="composer.ctx.to.pick."]')).filter((e) => !e.dataset['testid']?.endsWith('.sign') && !e.dataset['testid']?.includes('.sign.'));
    expect(rows.map((r) => r.dataset['testid'])).toEqual(['composer.ctx.to.pick.main', 'composer.ctx.to.pick.native-lead', 'composer.ctx.to.pick.perf']);
    expect(rows[0]?.textContent).toContain('bise');
    expect(v.getByTestId('composer.ctx.to.pick.perf.sign').dataset['sign']).toBe('∿');
    expect(rows[1]?.getAttribute('aria-checked') ?? rows[1]?.getAttribute('aria-selected') ?? rows[1]?.className).toMatch(/true|current|on/u);
    fireEvent.click(v.getByTestId('composer.ctx.to.pick.perf'));
    expect(router.get()).toMatchObject({ screen: 'thread', project: P, agent: 'perf' });
    expect(v.queryByTestId('composer.ctx.to.menu')).toBeNull();
  });
});

// his m_50754: the strip names no project and picks none, the sidebar does both; on a project's
// thread and on bise's own screen alike. Mutation: the project item drawn back turns it red.
describe('the strip says no project', () => {
  it("a sub-agent's thread and main's: no project item, no project menu", () => {
    for (const agent of ['native-lead', 'main']) {
      const v = strip(agent);
      expect(v.getByTestId('composer.ctx').textContent).not.toContain('acme');
      expect(v.queryByTestId('composer.ctx.project')).toBeNull();
      cleanup();
    }
  });
});

// ui-review m_47192: at 680 with a panel the branch kept 's…' in 40 px, under BRANCH_MIN: the fit
// read its width as its css had cut it. Mutation: the branch read by its box alone again turns it red.
describe("the strip's branch at the narrowest", () => {
  it('goes whole when its whole words would not fit BRANCH_MIN, read uncut', () => {
    const sw = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollWidth');
    const cw = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth');
    // the branch cut by its css to 40 (its words show 20 of their 70), the strip 60 after 'to native-lead'
    Object.defineProperty(HTMLElement.prototype, 'scrollWidth', { configurable: true, get(this: HTMLElement) {
      const id = this.dataset['testid'] ?? '';
      if (id === 'composer.ctx.to') return 110;
      if (id === 'composer.ctx.branch') return 40;
      if (this.classList.contains('nm') && this.closest('[data-item="branch"]') !== null) return 70;
      return 0;
    } });
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get(this: HTMLElement) {
      if (this.classList.contains('head')) return 165;
      if (this.classList.contains('nm') && this.closest('[data-item="branch"]') !== null) return 20;
      return 0;
    } });
    try {
      const v = strip('native-lead', { branch: 'sb/native-lead' });
      expect(v.getByTestId('composer.ctx.branch').classList.contains('gone')).toBe(true);
    } finally {
      if (sw !== undefined) Object.defineProperty(HTMLElement.prototype, 'scrollWidth', sw);
      if (cw !== undefined) Object.defineProperty(HTMLElement.prototype, 'clientWidth', cw);
    }
  });
});

describe("the strip says the addressee's state where 'local' was (his m_44719)", () => {
  it("running: main's breath (WorkingMark) and 'working · 3 min'", () => {
    const v = strip('native-lead', { status: 'working', turn_ms: 180_000 });
    const st = v.getByTestId('composer.ctx.state');
    expect(st.dataset['state']).toBe('working');
    expect(v.getByTestId('composer.ctx.state.mark').dataset['mark']).toBe('working');
    expect(st.querySelectorAll('.wk-sign').length).toBe(1);
    expect(st.textContent).toContain('working · 3 min');
  });

  it("something waits on him: the ask mark and 'waits for you', no breath", () => {
    const v = strip('main', { waits: 1, status: 'working', turn_ms: 1_000 });
    const st = v.getByTestId('composer.ctx.state');
    expect(st.dataset['state']).toBe('waits');
    expect(st.textContent).toContain('waits for you');
    expect(st.querySelector('.wk-sign')).toBeNull();
  });

  it("idle: nothing at all; and no 'local' place control anywhere in the composer", () => {
    for (const agent of ['main', 'native-lead']) {
      const v = strip(agent);
      expect(v.queryByTestId('composer.ctx.state')).toBeNull();
      expect(v.queryByTestId('composer.ctx.place')).toBeNull();
      expect(v.getByTestId('composer').textContent).not.toMatch(/\blocal\b/u);
      cleanup();
    }
  });
});
