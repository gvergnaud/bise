// @vitest-environment happy-dom
// The hover pin never covers an item he clicks (his m_57202, designer m_57214): each kind of agent
// line (PR, PR and age, ψ, ψ and PR, age, nothing, pinned with a PR) and a process's port, hovered,
// then each of its items clicked. happy-dom has no layout nor :hover: side3d.css is read with
// `:hover` as `[data-hover]` and the row hovered by that attribute; the pin covers the end's last
// item unless the row keeps its room, so 'never under the pin' is: shown, and either not last or
// the room kept (36 right). Mutation: side3d.css's hide rule back to
// `:is(.s3-orow-age, .s3-orow-pr):last-child`, or the room rule dropped, or the PR after the age in
// OuterCard.tsx, turns this red.
import { cleanup, fireEvent, render } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { ev, projects, row } from '../../../tests/feature/inputs';
import type { BarRow, ProjAgent } from '../barrows';
import { SideRow } from './Flyout';
import { OuterAgents } from './OuterCard';

// the heads' 3D pictures need WebGL: pictures that never draw
vi.mock('./Bubbles', () => ({ ThingPic: () => <span /> }));

const P = 'acme-1a2b3c4d';
/** The pin's room: its 24 box, 8 from the row's right, 4 apart (side3d.css). */
const ROOM = 36;
/** A computed length in px (happy-dom leaves calc() of px unresolved). Pure. */
const px = (v: string): number => [...v.matchAll(/(-?[\d.]+)px/gu)].reduce((n, m) => n + Number(m[1]), 0);

beforeAll(() => {
  const style = document.createElement('style');
  style.textContent = ['../../ds/tokens.css', 'side3d.css'].map((f) => readFileSync(join(__dirname, f), 'utf8')).join(String.fromCharCode(10)).replaceAll(':hover', '[data-hover]');
  document.head.append(style);
});
afterEach(cleanup);

/** An agent line's row: `time` its age (an agent at work), `pr` its PR. */
const line = (agent: string, o: { time?: string; pr?: number } = {}): BarRow => ({
  key: `${P}/${agent}`, project: P, agent, kind: 'agent', glyph: '', tone: 'ink', name: agent, sub: '', end: '', endTone: 'dim', title: agent, testId: agent, waits: false,
  ...(o.time === undefined ? {} : { time: o.time, state: { tone: 'work', g: '', tip: '' } }),
  ...(o.pr === undefined ? {} : { mark: { pr: o.pr, tip: `#${String(o.pr)}` } }),
}) as unknown as BarRow;
const pa = (name: string, wt = false): ProjAgent => ({ project: P, projectName: 'acme', a: { name, ...(wt ? { mode: 'worktree', worktree: `/h/wt/${name}/acme`, branch: `sb/${name}` } : {}) } }) as unknown as ProjAgent;

function store(): Store {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const s = new Store(port);
  s.setCore('up');
  s.apply(projects(row({ project: P, name: 'acme' })));
  s.apply(ev('prefs', { prefs: { pins: { [P]: ['pinned'] } } }));
  return s;
}

/** The row hovered (the stylesheet's :hover read as [data-hover]). */
function hover(row: HTMLElement): void {
  row.setAttribute('data-hover', '');
  row.querySelector('[data-row]')?.setAttribute('data-hover', '');
}

/** The end item is never under the shown pin: it shows, and it is not the end's last, or the row keeps the pin's room. */
function clear(item: HTMLElement, open: HTMLElement): void {
  expect(getComputedStyle(item).visibility).not.toBe('hidden');
  if (item.nextElementSibling === null) expect(px(getComputedStyle(open).paddingRight)).toBe(ROOM);
}

describe('an agent line hovered: the pin never covers its PR', () => {
  const kinds = [
    { name: 'pr', o: { pr: 75286 }, wt: false },
    { name: 'prage', o: { pr: 75287, time: '34 d' }, wt: false },
    { name: 'wtpr', o: { pr: 75288 }, wt: true },
    { name: 'wt', o: {}, wt: true },
    { name: 'aged', o: { time: '19 h' }, wt: false },
    { name: 'bare', o: {}, wt: false },
    { name: 'pinned', o: { pr: 75289, time: '2 h' }, wt: false },
  ] as const;

  it('each kind: its PR shows and stays clear of the pin, its age gives way; a click on each item does its own thing', () => {
    const s = store();
    const onOpen = vi.fn();
    const onOpenPr = vi.fn();
    const sends = vi.spyOn(s, 'command');
    const v = render(
      <StoreProvider store={s}>
        <OuterAgents live={kinds.map((k) => line(k.name, k.o))} agents={kinds.map((k) => pa(k.name, k.wt))} cards={[]} at={null}
          here={{ project: null, agent: null }} now={0} onOpen={onOpen} onOpenPr={onOpenPr} bims={null} />
      </StoreProvider>,
    );
    for (const k of kinds) {
      const id = `app.outer.agents.${P}.${k.name}`;
      const rowEl = v.getByTestId(id);
      const open = rowEl.querySelector<HTMLElement>('.s3-orow-open');
      if (open === null) throw new Error(`${k.name}: no line`);
      hover(rowEl);
      expect(getComputedStyle(v.getByTestId(`${id}.pin`).parentElement ?? open).opacity, k.name).toBe('1');
      const pr = v.queryByTestId(`${id}.pr`);
      const age = v.queryByTestId(`${id}.age`);
      expect(pr !== null, k.name).toBe('pr' in k.o);
      expect(age !== null, k.name).toBe('time' in k.o);
      if (pr !== null) clear(pr, open);
      // the age is words: the pin takes its place, its room never kept for it
      if (age !== null) {
        expect(age.nextElementSibling, k.name).toBeNull();
        expect(getComputedStyle(age).visibility, k.name).toBe('hidden');
        expect(px(getComputedStyle(open).paddingRight), k.name).not.toBe(ROOM);
      }
      if (k.wt) clear(v.getByTestId(`${id}.wt`), open);
      // its PR opens the PR, never the thread; the line opens the thread; the pin pins, never opens
      onOpen.mockClear();
      onOpenPr.mockClear();
      if (pr !== null) {
        fireEvent.click(pr);
        expect(onOpenPr.mock.calls.map(([r]) => (r as BarRow).agent), k.name).toEqual([k.name]);
        expect(onOpen, k.name).not.toHaveBeenCalled();
      }
      fireEvent.click(open);
      expect(onOpen.mock.calls.map(([r]) => (r as BarRow).agent), k.name).toEqual([k.name]);
      onOpen.mockClear();
      sends.mockClear();
      fireEvent.click(v.getByTestId(`${id}.pin`));
      expect(onOpen, k.name).not.toHaveBeenCalled();
      expect(sends.mock.calls.map(([c]) => (c as { cmd: string }).cmd), k.name).toEqual(['prefs_set']);
    }
  });
});

describe('a process row hovered: the pin never covers its port', () => {
  const proc = (id: string, runs: boolean) => (
    <SideRow pic={<span />} title="pnpm dev" sub="" label="pnpm dev" tip="pnpm dev" onClick={() => undefined} menu={{ onContextMenu: () => undefined, open: () => undefined }}
      pin={{ kind: 'process', project: P, id, on: false }} testId={id}
      end={
        <>
          <span className="s3-port" role="link" data-testid={`${id}.port`}>:5242</span>
          {runs && <span className="s3-wave" data-testid={`${id}.runs`}>∿</span>}
        </>
      } />
  );

  it('its port shows and stays clear of the pin, with or without the ∿ after it', () => {
    const v = render(<StoreProvider store={store()}>{proc('runs', true)}{proc('done', false)}</StoreProvider>);
    for (const id of ['runs', 'done']) {
      const rowEl = v.getByTestId(id);
      const body = rowEl.querySelector<HTMLElement>('.s3-row-body');
      if (body === null) throw new Error(`${id}: no body`);
      hover(rowEl);
      clear(v.getByTestId(`${id}.port`), body);
      // the ∿ after it is words: the pin takes its place, no room kept
      expect(px(getComputedStyle(body).paddingRight), id).toBe(id === 'runs' ? 12 : ROOM);
    }
    expect(getComputedStyle(v.getByTestId('runs.runs')).visibility).toBe('hidden');
  });
});
