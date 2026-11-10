import { describe, expect, it } from 'vitest';
import type { Agent, ProjectRow } from '@/proto';
import { keyAction, type KeyCtx, type KeyLike } from './keys';

const row = (project: string, name: string, order: number, home = false): ProjectRow => ({ project, name, path: `/${name}`, git: !home, home, order, agents: 1, working: 0, waits: 0, running: true });
const ag = (name: string, over: Partial<Agent> = {}): Agent => ({ name, dir: name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: '', since_ms: 1, waits: 0, watching: [], ...over });
const projects = [row('h', 'bise', 0, true), row('s', 'shop', 2), row('a', 'acme', 1)];
const agents: Record<string, Agent[]> = {
  h: [ag('main')],
  a: [ag('main'), ag('perf', { status: 'working', since_ms: 5 }), ag('docs', { status: 'working', since_ms: 3 }), ag('old', { archived: true })],
  s: [ag('main')],
};
const ctx = (over: Partial<KeyCtx> = {}): KeyCtx => ({ route: { screen: 'thread', project: 'a', agent: 'main' }, projects, agents: (p) => agents[p] ?? [], typing: false, ...over });
const k = (code: string, mods: Partial<KeyLike> = {}): KeyLike => ({ key: '', code, metaKey: false, altKey: false, ctrlKey: false, shiftKey: false, ...mods });

describe("the window's keys", () => {
  it('⌘0 bise, ⌘1-9 his projects in his order, nothing past the last', () => {
    expect(keyAction(k('Digit0', { metaKey: true }), ctx())).toEqual({ go: { screen: 'bise' } });
    expect(keyAction(k('Digit1', { metaKey: true }), ctx())).toEqual({ go: { screen: 'thread', project: 'a', agent: 'main' } });
    expect(keyAction(k('Digit2', { metaKey: true }), ctx())).toEqual({ go: { screen: 'thread', project: 's', agent: 'main' } });
    expect(keyAction(k('Digit3', { metaKey: true }), ctx())).toBeUndefined();
  });

  it("the panel open: ⌥0 the project's main, ⌥1-9 its live agents in the panel's order (by code: ⌥1 types '¡')", () => {
    const open = (over: Partial<KeyCtx> = {}) => ctx({ panelOpen: true, ...over });
    expect(keyAction(k('Digit0', { altKey: true, key: 'º' }), open())).toEqual({ go: { screen: 'thread', project: 'a', agent: 'main' } });
    expect(keyAction(k('Digit1', { altKey: true, key: '¡' }), open())).toEqual({ go: { screen: 'thread', project: 'a', agent: 'perf' } });
    expect(keyAction(k('Digit2', { altKey: true }), open())).toEqual({ go: { screen: 'thread', project: 'a', agent: 'docs' } });
    expect(keyAction(k('Digit3', { altKey: true }), open())).toBeUndefined();
    // on bise's own screen, bise's agents
    expect(keyAction(k('Digit0', { altKey: true }), open({ route: { screen: 'bise' } }))).toEqual({ go: { screen: 'thread', project: 'h', agent: 'main' } });
  });

  it('⌥0-9 are main and the agents, the side folded too; the coins are ⌥⌘1-4 only: the two never share a key (designer m_45787)', () => {
    expect([1, 2, 3, 4].map((n) => keyAction(k(`Digit${String(n)}`, { altKey: true }), ctx()))).not.toContainEqual({ screen: 'side-1' });
    for (const n of [1, 2, 3, 4]) expect(keyAction(k(`Digit${String(n)}`, { altKey: true }), ctx())).not.toEqual({ screen: `side-${String(n)}` });
    expect(keyAction(k('Digit0', { altKey: true }), ctx())).toEqual({ go: { screen: 'thread', project: 'a', agent: 'main' } });
    // ⌥⌘1-4 the coins, panel open or not
    expect(keyAction(k('Digit2', { altKey: true, metaKey: true }), ctx({ panelOpen: true }))).toEqual({ screen: 'side-2' });
    expect(keyAction(k('Digit1', { altKey: true, metaKey: true }), ctx())).toEqual({ screen: 'side-1' });
  });

  it('⌘K palette, ⌘D changes / review, ⇧⌘A the artifacts library (as the rail), ⌘E ctrl+y to the screen, ⌘/ the key list (⌘. is the Agent menu’s Stop)', () => {
    expect(keyAction(k('KeyK', { metaKey: true }), ctx())).toEqual({ palette: true });
    expect(keyAction(k('KeyD', { metaKey: true }), ctx())).toEqual({ go: { screen: 'changes', project: 'a' } });
    // an agent's thread: its change in the panel (page pr-guided V2), never a screen
    expect(keyAction(k('KeyD', { metaKey: true }), ctx({ route: { screen: 'thread', project: 'a', agent: 'perf' } }))).toEqual({ change: { project: 'a', what: { agent: 'perf' } } });
    expect(keyAction(k('KeyA', { metaKey: true, shiftKey: true }), ctx())).toEqual({ go: { screen: 'artifacts', project: '' } });
    expect(keyAction(k('KeyE', { metaKey: true }), ctx())).toEqual({ screen: 'fold-all' });
    // the TUI's ctrl+o, typing or not (his cb1f2b44 bug: nothing opened the folds)
    expect(keyAction(k('KeyO', { ctrlKey: true }), ctx())).toEqual({ screen: 'fold-all' });
    expect(keyAction(k('KeyO', { ctrlKey: true }), ctx({ typing: true }))).toEqual({ screen: 'fold-all' });
    // the menu bar's Agent menu has ⌘. (app/menus/registry.ts KEYS): the window's keys leave it
    expect(keyAction(k('Period', { metaKey: true }), ctx())).toBeUndefined();
    expect(keyAction(k('KeyY', { ctrlKey: true }), ctx())).toEqual({ screen: 'copy-code' });
    expect(keyAction(k('Slash', { metaKey: true }), ctx())).toEqual({ help: true });
    // editing keys are not the window's
    expect(keyAction(k('KeyC', { metaKey: true }), ctx())).toBeUndefined();
    expect(keyAction(k('KeyA', { metaKey: true }), ctx())).toBeUndefined();
  });

  it('⌘N New project, ⇧⌘N still a New agent in the project in view, bise\'s own on bise (round 3: no sidebar row for it, the key stays)', () => {
    expect(keyAction(k('KeyN', { metaKey: true }), ctx())).toEqual({ newProject: true });
    expect(keyAction(k('KeyN', { metaKey: true, shiftKey: true }), ctx())).toEqual({ newAgent: 'a' });
    expect(keyAction(k('KeyN', { metaKey: true, shiftKey: true }), ctx({ route: { screen: 'bise' } }))).toEqual({ newAgent: 'h' });
  });

  it('end and ⌥↑↓ only when no text is being typed', () => {
    expect(keyAction({ ...k('End'), key: 'End' }, ctx())).toEqual({ screen: 'to-end' });
    expect(keyAction({ ...k('End'), key: 'End' }, ctx({ typing: true }))).toBeUndefined();
    expect(keyAction({ ...k('ArrowDown', { altKey: true }), key: 'ArrowDown' }, ctx())).toEqual({ go: { screen: 'thread', project: 'a', agent: 'perf' } });
    expect(keyAction({ ...k('ArrowUp', { altKey: true }), key: 'ArrowUp' }, ctx({ route: { screen: 'thread', project: 'a', agent: 'docs' } }))).toEqual({ go: { screen: 'thread', project: 'a', agent: 'perf' } });
    expect(keyAction({ ...k('ArrowDown', { altKey: true }), key: 'ArrowDown' }, ctx({ typing: true }))).toBeUndefined();
  });
});

describe('the key list (amb-web 1e60fbd6)', () => {
  it("'?' never opens it: a message may start with it (main m_22096); ⌘/ and ⌘? do", () => {
    const c = { route: { screen: 'bise' as const }, projects: [], agents: () => [], typing: false };
    const q = { key: '?', code: 'Slash', metaKey: false, altKey: false, ctrlKey: false, shiftKey: true };
    expect(keyAction(q, c)).toBeUndefined();
    expect(keyAction({ ...q, key: '/', metaKey: true, shiftKey: false }, c)).toEqual({ help: true });
    expect(keyAction({ ...q, metaKey: true }, c)).toEqual({ help: true });
  });
});
