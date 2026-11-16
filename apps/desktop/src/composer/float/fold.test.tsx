// @vitest-environment happy-dom
// chat-collapse (his m_65744, designer m_65748): over an artifact shown full width, the float's
// inbox starts folded to one line, a notification that arrives too; the chevron at its top right
// shows the inbox and folds it back; ⌃n, ⌘I and a notification click open the item shown; an item
// carried over from 'open beside' stays folded, his words still answer it and no key picks an
// option he cannot see. Nothing waiting, the float is as everywhere else.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Agent, Artifact, Card } from '@/proto';
import type { CoreCmd, CorePort } from '@/shared/api';
import { core_ev } from '@/proto/fixtures.gen';
import { composer } from '@/proto/words.gen';
import { replay, Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { FOLDED } from '@/app/panel/model';
import { EMPTY_REC, putView, switchTo } from '@/app/panel/scope';
import { panelState } from '@/app/panel/state';
import { openInbox } from '@/inbox';
import { FloatHost } from './Host';

/** what the field was given last: its placeholder, its pink and its send */
let field: { placeholder?: string; answering?: boolean; onSend?: (out: { text: string }) => void } = {};

vi.mock('@/composer', () => ({
  // the field is the composer's own (its tests); here a stand-in that keeps its props
  Composer: (p: { placeholder?: string; answering?: boolean; onSend?: (out: { text: string }) => void; testId?: string }) => {
    Object.assign(field, { placeholder: p.placeholder, answering: p.answering, onSend: p.onSend });
    return <div data-testid={p.testId} />;
  },
  useComposerSources: () => ({ agents: [], skills: [], commands: [], files: () => Promise.resolve([]), saveImage: () => Promise.resolve(null) }),
  useApprovalsFlash: () => false,
  flashWords: () => ({ word: '', rest: '' }),
}));

const P = 'acme-1a2b3c4d';
const HOME = 'bise-home-00000000';
const PAGE: Artifact = { id: 'rates', kind: 'page', title: 'taux et prêts', agent: 'taux-immo', version: 5, at_ms: 1791100000000, url: 'http://127.0.0.1:7841/p/rates', new: false };
const agent = (name: string): Agent => ({ name, dir: name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: '', since_ms: 1, waits: 0, watching: [] });

beforeEach(() => {
  // wide enough for a full-width panel (under 1100 it is a sheet, never full)
  Object.defineProperty(window, 'innerWidth', { value: 1440, configurable: true });
  localStorage.clear();
  const tab = { id: `artifact:${P}:${PAGE.id}`, what: { kind: 'thing' as const, e: { kind: 'artifact' as const, project: P, id: PAGE.id } } };
  panelState.reset(putView(switchTo(EMPTY_REC, HOME), { ...FOLDED, open: true, full: true, tabs: { list: [tab], active: tab.id } }));
  field = {};
});
afterEach(cleanup);

/** core_ev with each project's cards through `f` (none: no item waits anywhere) */
const cardsBy = (f: (cs: readonly Card[]) => Card[]): typeof core_ev => core_ev.map((e) => (e.ev !== 'projects' ? e : { ...e, projects: e.projects.map((r) => ({ ...r, cards: f(r.cards ?? []) })) }));

function mount(opts: { events?: typeof core_ev; keptOpen?: { project: string; card: number } } = {}) {
  const sent: CoreCmd[] = [];
  const port: CorePort = { send: (c) => sent.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  replay(store, opts.events ?? core_ev);
  store.apply({ ev: 'artifacts', project: P, items: [PAGE] });
  store.apply({ ev: 'agents', project: P, agents: [agent('main'), agent('taux-immo')] });
  // the item he opened beside its page, carried to the block the artifact view mounts
  if (opts.keptOpen !== undefined) store.keepOpen(opts.keptOpen);
  render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'bise' })}>
        <FloatHost />
      </RouterContext>
    </StoreProvider>,
  );
  // what goes to the hub about a card (the card in view is the window's own fact)
  const hub = () => sent.filter((c) => c.cmd !== 'card_in_view' && 'card' in c);
  return { store, hub };
}

const block = () => screen.queryByTestId('inbox.block');
const grip = () => screen.getByTestId('composer.float.grip');
const head = () => screen.queryByTestId('composer.float.head');
const anyOpen = () => document.querySelector('[data-testid^="inbox.block.open."]');
const press = () => {
  act(() => {
    fireEvent.click(grip());
  });
};
const key = (init: KeyboardEventInit) => {
  act(() => {
    fireEvent.keyDown(document.body, init);
  });
};

describe('the inbox on the float over a full-width artifact', () => {
  it('folded by default: the inbox line between ─ and ^, no head, no list, no option; the line updates when an item arrives', () => {
    const v = mount();
    expect(block()?.dataset['rest']).toBe('line');
    expect(head()).toBeNull();
    expect(anyOpen()).toBeNull();
    // the line holds the float's two ends
    expect(block()?.contains(screen.getByTestId('composer.float.fold'))).toBe(true);
    expect(block()?.contains(grip())).toBe(true);
    expect([grip().getAttribute('aria-expanded'), grip().getAttribute('aria-label')]).toEqual(['false', composer.float_inbox_open]);
    const n = Number(block()?.dataset['count']);
    // a notification arrives: still folded, its line counts it
    act(() => {
      replay(v.store, cardsBy((cs) => cs.flatMap((c) => (c.id === 9 ? [c, { ...c, id: 77, question: 'which port?' }] : [c]))).filter((e) => e.ev === 'projects'));
    });
    expect(block()?.dataset['rest']).toBe('line');
    expect(Number(block()?.dataset['count'])).toBe(n + 1);
    expect(anyOpen()).toBeNull();
  });

  it('^ shows the inbox (its rows, the head over them), v folds it back to the line', () => {
    mount();
    press();
    expect(block()?.dataset['rest']).toBeUndefined();
    expect(screen.getByTestId('inbox.block.row.1')).toBeTruthy();
    expect(head()).not.toBeNull();
    expect([grip().getAttribute('aria-expanded'), grip().getAttribute('aria-label')]).toEqual(['true', composer.float_inbox_close]);
    press();
    expect(block()?.dataset['rest']).toBe('line');
    expect(head()).toBeNull();
  });

  it('⌃1 opens the item shown, its options in view; esc lets it go, back to the line', () => {
    mount();
    key({ key: '1', code: 'Digit1', ctrlKey: true });
    expect(anyOpen()).not.toBeNull();
    expect(grip().getAttribute('aria-expanded')).toBe('true');
    key({ key: 'Escape' });
    expect(anyOpen()).toBeNull();
    expect(block()?.dataset['rest']).toBe('line');
  });

  it('a notification he clicks (and ⌘I) opens its item shown', () => {
    mount();
    act(() => {
      openInbox({ project: P, card: 9 });
    });
    expect(screen.getByTestId(`inbox.block.card.${P}-9`)).toBeTruthy();
    expect(grip().getAttribute('aria-expanded')).toBe('true');
  });

  it("an item carried over from 'open beside' starts folded: the head says it, his words answer it, a digit picks nothing", () => {
    const v = mount({ keptOpen: { project: P, card: 9 } });
    expect(block()).toBeNull();
    expect(head()?.textContent).toContain('perf');
    expect(grip().getAttribute('aria-expanded')).toBe('false');
    expect(field.answering).toBe(true);
    // the page has the focus: '1' is no option he can see
    key({ key: '1', code: 'Digit1' });
    key({ key: 'Backspace', metaKey: true });
    expect(v.hub()).toEqual([]);
    expect(head()).not.toBeNull();
    // ^ shows it whole, still open
    press();
    expect(screen.getByTestId(`inbox.block.card.${P}-9`)).toBeTruthy();
    press();
    act(() => {
      field.onSend?.({ text: 'warm' });
    });
    expect(v.hub()).toContainEqual(expect.objectContaining({ project: P, card: 9 }));
  });

  it("nothing waits: as everywhere else, the bare pill at rest; focused, ^ opens the mini thread (designer m_65750)", () => {
    mount({ events: cardsBy(() => []) });
    expect(block()).toBeNull();
    expect(head()).toBeNull();
    act(() => {
      fireEvent.focus(screen.getByTestId('composer'));
    });
    expect(grip().dataset['opens']).toBe('thread');
    press();
    expect(screen.getByTestId('composer.float').dataset['state']).toBe('open');
  });
});
