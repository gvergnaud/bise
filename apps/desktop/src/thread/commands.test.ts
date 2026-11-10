// The composer's / list in the window and how a command line runs (amb-core's C/A/B 40f14a12).
import { describe, expect, it } from 'vitest';
import type { PickCommand as CoreCommand } from '@/proto';
import { pickCommands, slashLine, slashOf, windowCommands, windowRun } from './commands';

const P = 'shop-1a2b3c4d';
const at = { project: P, agent: 'perf' };
const core: CoreCommand[] = [
  { name: '/new', desc: 'start an agent', args: [{ kind: 'words', words: [{ value: '-w', desc: 'in its own git worktree' }] }, { kind: 'text' }], runs: 'hub', runnable: true },
  { name: '/compact', desc: 'compact its context', args: [], runs: 'hub', runnable: false },
  { name: '/inbox', desc: 'what waits for you', args: [], runs: 'window', runnable: false },
  { name: '/scheduled', desc: 'its timers', args: [], runs: 'window', runnable: false },
  { name: '/rename', desc: 'rename an agent', args: [{ kind: 'agent' }, { kind: 'note' }], runs: 'hub', runnable: true },
  { name: '/agents', desc: 'the agents', args: [], runs: 'window', runnable: false },
  { name: '/quit', desc: 'quit', args: [], runs: 'window', runnable: false },
];

describe("the / list: the TUI's commands in its order, never a silent gap", () => {
  it("keeps the core's order; a window command is ready when its screen exists, every hub one (its line goes as typed)", () => {
    const rows = pickCommands(core);
    expect(rows.slice(0, core.length).map((r) => r.name)).toEqual(core.map((c) => c.name));
    const ready = Object.fromEntries(rows.map((r) => [r.name, r.ready]));
    expect(ready).toMatchObject({ '/new': true, '/compact': true, '/inbox': true, '/scheduled': true, '/rename': true, '/agents': true, '/quit': true });
  });
  it("maps the arguments the composer completes (a note is free text) and adds the window's own the TUI lacks", () => {
    const rows = pickCommands(core);
    expect(rows.find((r) => r.name === '/rename')).toMatchObject({ args: true, argKinds: ['agent', 'text'] });
    expect(rows.find((r) => r.name === '/new')?.argKinds?.[0]).toEqual({ words: [{ value: '-w', desc: 'in its own git worktree' }] });
    expect(rows.find((r) => r.name === '/inbox')).toMatchObject({ args: false });
    const extra = rows.slice(core.length).map((r) => r.name);
    expect(extra).toContain('prs');
    expect(extra, 'one row per command').not.toContain('agents');
  });
  it('leaves /version out of the window\'s list (a terminal dev tool), whatever the core sends', () => {
    const rows = pickCommands([...core, { name: '/version', desc: 'the version' } as (typeof core)[number]]);
    expect(rows.map((r) => r.name)).not.toContain('/version');
    expect(rows.map((r) => r.name)).not.toContain('version');
  });
  it("before the core answers: the window's own alone", () => {
    expect(pickCommands([])).toEqual(windowCommands);
  });
});

describe('a command line runs as the TUI runs it', () => {
  it('window commands open their screens, /help the key list', () => {
    expect(windowRun('/inbox', '', at)).toEqual({ inbox: true });
    expect(windowRun('/help', '', at)).toEqual({ help: true });
    expect(windowRun('/shortcuts', '', at)).toEqual({ help: true });
    expect(windowRun('/plugins', '', at)).toEqual({ go: { screen: 'plugins' } });
    expect(windowRun('/skills', '', at)).toEqual({ go: { screen: 'plugins', kind: 'skills' } });
    expect(windowRun('/skills', 'off bend', at)).toEqual({ app: { cmd: 'plugin_set', name: 'bend', on: false } });
    expect(windowRun('/welcome', '', at)).toEqual({ go: { screen: 'onboarding' } });
    expect(windowRun('/scheduled', '', at), 'R4').toEqual({ go: { screen: 'scheduled' } });
    // N1: /keychain on|off moves the secrets as settings' switch does; bare opens that page
    expect(windowRun('/keychain', 'on', at)).toEqual({ app: { cmd: 'keychain', to: 'on' } });
    expect(windowRun('/keychain', 'off', at)).toEqual({ app: { cmd: 'keychain', to: 'off' } });
    expect(windowRun('/keychain', '', at)).toEqual({ go: { screen: 'settings', page: 'keys' } });
    expect(windowRun('/quit', '', at), 'R5: quits, the agents keep running').toEqual({ quit: true });
    // a change opens in the panel (page pr-guided V2: review/openChange.ts)
    expect(windowRun('/diff', 'sb/x', at), 'R7: a branch, with or without an agent').toEqual({ change: { branch: 'sb/x' } });
    expect(windowRun('/diff', 'feat/a/b', at), 'its slashes kept').toEqual({ change: { branch: 'feat/a/b' } });
    expect(windowRun('/diff', 'perf', at), 'an agent: its change').toEqual({ change: { agent: 'perf' } });
    expect(windowRun('/new', 'x', at)).toBeUndefined();
  });
  it("a line he typed that isn't a window screen goes to the hub as typed (architect m_11011), never parsed here", () => {
    const slash = (line: string) => ({ cmd: 'slash', project: P, agent: 'perf', line });
    expect(slashOf('/rename', 'perf p2', at), 'a typed /rename is a slash, not a typed rename').toEqual(slash('/rename perf p2'));
    expect(slashOf('/new', '-w --with-changes x: do it', at), 'C3').toEqual(slash('/new -w --with-changes x: do it'));
    expect(slashOf('/undo', '', at), 'C40').toEqual(slash('/undo'));
    expect(slashOf('/flow', 'pr', at), 'C42').toEqual(slash('/flow pr'));
    expect(slashOf('compact', '', at)).toEqual(slash('/compact'));
  });
  it("a window command's words act as its settings switch (R2, R3): the same AppCmd settings sends", () => {
    expect(windowRun('/plugins', 'off notes', at)).toEqual({ app: { cmd: 'plugin_set', name: 'notes', on: false } });
    expect(windowRun('/plugins', 'on computer', at)).toEqual({ app: { cmd: 'plugin_set', name: 'computer', on: true } });
    expect(windowRun('/plugins', 'login linear', at)).toEqual({ app: { cmd: 'plugin_login', name: 'linear' } });
    expect(windowRun('/plugins', 'logout linear', at)).toEqual({ app: { cmd: 'plugin_logout', name: 'linear' } });
    expect(windowRun('/plugins', 'list', at), 'list: the place').toEqual({ go: { screen: 'plugins' } });
    expect(windowRun('/plugins', 'login', at), 'no name: the hub lists them').toBeUndefined();
    expect(windowRun('/plugins', 'frob x', at), 'unknown: the hub says the usage').toBeUndefined();
    expect(windowRun('/theme', 'light', at)).toEqual({ app: { cmd: 'prefs_set', key: 'theme', value: 'light' } });
    expect(windowRun('/theme', 'dark', at)).toEqual({ app: { cmd: 'prefs_set', key: 'theme', value: 'dark' } });
    expect(windowRun('/theme', 'auto', at), "the TUI's auto is the system's").toEqual({ app: { cmd: 'prefs_set', key: 'theme', value: 'system' } });
    expect(windowRun('/theme', '', at), 'bare: the page').toEqual({ go: { screen: 'settings', page: 'general' } });
    expect(windowRun('/computer-use', 'off', at)).toEqual({ app: { cmd: 'computer_use', act: 'off' } });
    expect(windowRun('/computer-use', 'uninstall', at)).toEqual({ app: { cmd: 'computer_use', act: 'uninstall' } });
    expect(windowRun('/computer-use', '', at), 'bare: the page sets it up').toEqual({ go: { screen: 'settings', page: 'computer' } });
  });
  it('a window command with words of its own goes to the hub: /artifacts add <url> (C13)', () => {
    expect(windowRun('/artifacts', '', at)).toEqual({ go: { screen: 'artifacts', project: P } });
    expect(windowRun('/artifacts', 'add https://x.test', at), 'C13: not the screen').toBeUndefined();
    expect(slashOf('/artifacts', 'add https://x.test', at)).toEqual({ cmd: 'slash', project: P, agent: 'perf', line: '/artifacts add https://x.test' });
  });
});

describe('a slash line the list does not know (qa-flows d50c36c5)', () => {
  it('/undo, /cancel, /flow pr are commands for the hub; a path or several lines stay a message', () => {
    expect(slashLine('/undo')).toEqual({ name: '/undo', args: '' });
    expect(slashLine(' /flow pr ')).toEqual({ name: '/flow', args: 'pr' });
    expect(slashLine('/cancel')).toEqual({ name: '/cancel', args: '' });
    expect(slashLine('/usr/bin is odd')).toBeNull();
    expect(slashLine('/flow pr\nand more')).toBeNull();
    expect(slashLine('why /flow?')).toBeNull();
    expect(slashOf('/flow', 'pr', { project: 'p', agent: 'main' })).toEqual({ cmd: 'slash', project: 'p', agent: 'main', line: '/flow pr' });
  });
});
