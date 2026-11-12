// The action registry (bise fait mac, d4): each entity and state, what shows and what hides (HIG:
// hidden, never dimmed), the keys, the order of d4, and the keys the menu bar leaves free.
import { describe, expect, it } from 'vitest';
import type { Agent, Artifact, Pr, Proc, ProjectRow, ScheduledTask } from '@/proto';
import { menus } from '@/proto/words_menus.gen';
import type { PopupItem } from '@/shared/api';
import { actsOf, KEYS, menuOf, type Act, type MenuFacts } from './registry';

const P = 'acme-1a2b3c4d';

const agent = (over: Partial<Agent> = {}): Agent => ({
  name: 'perf',
  main: false,
  status: 'idle',
  archived: false,
  title: '',
  purpose: 'profile the hub',
  since_ms: 1,
  waits: 0,
  dir: '/h/agents/perf',
  path: '/w/acme/perf',
  branch: 'sb/perf',
  changes: { files: 3, add: 42, del: 18 },
  watching: [],
  ...over,
});

const agentFacts = (a: Agent | undefined, o: { pinned?: boolean; reply?: boolean } = {}): MenuFacts => ({
  e: { kind: 'agent', project: P, agent: a?.name ?? 'gone' },
  row: a,
  pinned: o.pinned ?? false,
  reply: o.reply ?? true,
});

/** The menu as text: labels, '›' for a submenu's items, '─' for a separator. */
function shape(items: readonly PopupItem[], depth = 0): string[] {
  return items.flatMap((i): string[] => {
    if (i.type === 'separator') return ['─'];
    if (i.type === 'submenu') return [`${'  '.repeat(depth)}${i.label} ›`, ...shape(i.items, depth + 1)];
    return [`${'  '.repeat(depth)}${i.label}`];
  });
}

describe('an agent', () => {
  it("a working agent: d4's groups in order, Schedule… hidden (no hub command)", () => {
    expect(shape(menuOf(agentFacts(agent({ status: 'working' }))))).toEqual([
      menus.rename,
      menus.change_model,
      menus.pin,
      '─',
      menus.changes,
      '─',
      `${menus.copy} ›`,
      `  ${menus.copy_name}`,
      `  ${menus.copy_path}`,
      `  ${menus.copy_branch}`,
      `  ${menus.copy_reply}`,
      '─',
      menus.move_right,
      menus.new_window,
      `${menus.open_in} ›`,
      `  ${menus.finder}`,
      `  ${menus.editor}`,
      '─',
      menus.stop,
      menus.archive,
    ]);
  });

  it('stop shows only while it works', () => {
    for (const status of ['idle', 'waiting', 'blocked', 'done', 'failed'] as const) expect(actsOf(menuOf(agentFacts(agent({ status }))))).not.toContain('stop');
    expect(actsOf(menuOf(agentFacts(agent({ status: 'working' }))))).toContain('stop');
  });

  it("an archived one: Restore, never Archive, Rename, Pin or Stop; its Copy and Open stay", () => {
    const acts = actsOf(menuOf(agentFacts(agent({ archived: true, status: 'working' }))));
    expect(acts).toContain('restore');
    for (const no of ['archive', 'rename', 'pin', 'unpin', 'stop'] as const) expect(acts).not.toContain(no);
    expect(acts).toEqual(expect.arrayContaining(['copy-name', 'move-right', 'new-window', 'open-finder']));
  });

  it("main: no Rename, Archive or Changes (the project's own); it still stops", () => {
    const acts = actsOf(menuOf(agentFacts(agent({ name: 'main', main: true, status: 'working' }))));
    for (const no of ['rename', 'archive', 'restore', 'changes'] as const) expect(acts).not.toContain(no);
    expect(acts).toContain('stop');
  });

  it('pinned: Unpin in Pin’s place, the same key', () => {
    const acts = actsOf(menuOf(agentFacts(agent(), { pinned: true })));
    expect(acts).toContain('unpin');
    expect(acts).not.toContain('pin');
    expect(KEYS.unpin).toBe(KEYS.pin);
  });

  it('what it lacks hides its item: no changes, no branch, no folder, no reply held', () => {
    const bare = agent({ changes: null, branch: null, checkout_branch: null, path: '' });
    const acts = actsOf(menuOf(agentFacts(bare, { reply: false })));
    for (const no of ['changes', 'copy-branch', 'copy-path', 'copy-reply', 'open-finder', 'open-editor'] as const) expect(acts).not.toContain(no);
    // its Open in › went whole: no empty submenu
    expect(shape(menuOf(agentFacts(bare, { reply: false })))).not.toContain(`${menus.open_in} ›`);
    // the shared checkout's branch is its branch too
    expect(actsOf(menuOf(agentFacts(agent({ branch: null, checkout_branch: 'main' }))))).toContain('copy-branch');
  });

  it('an agent that left the rows: no menu', () => {
    expect(menuOf(agentFacts(undefined))).toEqual([]);
  });
});

describe('the other things', () => {
  const project: ProjectRow = { project: P, name: 'acme', path: '/w/acme' } as ProjectRow;
  const artifact = (over: Partial<Artifact> = {}): Artifact => ({ id: 'a1', kind: 'page', title: 'Perf notes', agent: 'perf', version: 2, at_ms: 1, url: 'http://127.0.0.1:4000/p/a1', new: false, ...over });
  const proc = (over: Partial<Proc> = {}): Proc =>
    ({ id: 'p1', agent: 'perf', kind: 'dev', cmd: 'pnpm dev', pid: 9, started_ms: 1, ports: [5173], state: { is: 'running' }, cpu: 0, rss_mb: 1, procs: 1, out: 'pty', ...over }) as Proc;
  const task = (over: Partial<ScheduledTask> = {}): ScheduledTask => ({ id: 7, agent: 'perf', by: 'perf', words: 'check the build', every: '10m', done: 0, ...over });

  it('a project: Rename · Copy › · Open in new window, Open in ›', () => {
    expect(actsOf(menuOf({ e: { kind: 'project', project: P }, row: project }))).toEqual(['rename', 'copy-name', 'copy-path', 'new-window', 'open-finder', 'open-editor']);
    expect(menuOf({ e: { kind: 'project', project: P }, row: undefined })).toEqual([]);
  });

  it("Open with: the apps (the hub's pick first), 'your Mac' for a file, then Always open; a file no app opens: 'your Mac' alone (docs/plugin-apps.md A6b)", () => {
    const e = { kind: 'artifact', project: P, id: 'a1' } as const;
    const sheets = { plugin: 'sheets', tool: 'open_sheet', app: 'Sheets', type: '*.xlsx', type_title: 'Excel workbook' };
    const grid = { ...sheets, plugin: 'grid', tool: 'open', app: 'Grid' };
    const withApps = actsOf(menuOf({ e, row: artifact({ kind: 'file', path: '/w/acme/q3.xlsx', opener: sheets, openers: [grid] }) }));
    expect(withApps.slice(1, 6)).toEqual(['open-with:0', 'open-with:1', 'open-mac', 'always-open:0', 'always-open:1']);
    expect(actsOf(menuOf({ e, row: artifact({ kind: 'file', path: '/w/acme/logs.zip' }) }))).toContain('open-mac');
    // nothing on disk (a page, a gone file): no 'your Mac'
    expect(actsOf(menuOf({ e, row: artifact() }))).not.toContain('open-mac');
  });

  it('an artifact: its file items only with a file there', () => {
    const e = { kind: 'artifact', project: P, id: 'a1' } as const;
    // a file no app opens: Open with › your Mac (never a dead click)
    expect(actsOf(menuOf({ e, row: artifact({ path: '/w/acme/notes.md' }) }))).toEqual(['pin', 'open-mac', 'move-right', 'new-window', 'quick-look', 'show-in-finder', 'copy-link', 'copy-path']);
    expect(actsOf(menuOf({ e, row: artifact({ path: '/w/acme/notes.md', gone: true }) }))).toEqual(['pin', 'move-right', 'new-window', 'copy-link']);
    // pinned (pins.ts, his m_49100): Unpin in Pin's place
    expect(actsOf(menuOf({ e, row: artifact({ path: '/w/acme/notes.md' }), pinned: true }))[0]).toBe('unpin');
  });

  it('a PR (its row or its tab): Copy › Link, and Branch once the hub sent it (pr-integration A rule 7)', () => {
    const url = 'https://github.com/acme/web/pull/412';
    const status: Pr = { number: 412, url, branch: 'sb/perf', agents: ['perf'], state: 'open', checks: 'pass', review: 'approved', words: '', text: '' };
    const row = artifact({ id: 'pr-412', kind: 'pr', url, pr: { repo: 'acme/web', number: 412, tag: '#412', status } });
    expect(actsOf(menuOf({ e: { kind: 'artifact', project: P, id: 'pr-412' }, row }))).toEqual(['pin', 'move-right', 'new-window', 'copy-link', 'copy-branch']);
    expect(actsOf(menuOf({ e: { kind: 'pr', project: P, url }, row }))).toEqual(['move-right', 'new-window', 'copy-link', 'copy-branch']);
    expect(actsOf(menuOf({ e: { kind: 'pr', project: P, url }, row: undefined }))).toEqual(['move-right', 'new-window', 'copy-link']);
  });

  it("a Copy › of one item is that item flat ('Copy Link'), never a submenu of one", () => {
    const e = { kind: 'process', project: P, agent: 'perf', id: 'p1' } as const;
    expect(shape(menuOf({ e, row: proc() }))).toContain(menus.copy_one({ what: menus.copy_link }));
    expect(shape(menuOf({ e, row: proc() }))).not.toContain(`${menus.copy} ›`);
    // two items stay a submenu
    expect(shape(menuOf({ e: { kind: 'artifact', project: P, id: 'a1' }, row: artifact({ path: '/w/acme/notes.md' }) }))).toContain(`${menus.copy} ›`);
  });

  it('a process: Stop process only while it runs; its browser items only with a page', () => {
    const e = { kind: 'process', project: P, agent: 'perf', id: 'p1' } as const;
    expect(actsOf(menuOf({ e, row: proc() }))).toEqual(['pin', 'move-right', 'open-browser', 'copy-link', 'stop-process']);
    expect(actsOf(menuOf({ e, row: proc({ state: { is: 'exited', rc: 0 }, ports: [] }), pinned: true }))).toEqual(['unpin', 'move-right']);
  });

  it('a scheduled task: Pin · Stop schedule until it ended', () => {
    const e = { kind: 'scheduled', project: P, id: 7 } as const;
    expect(actsOf(menuOf({ e, row: task() }))).toEqual(['pin', 'stop-scheduled']);
    expect(actsOf(menuOf({ e, row: task(), pinned: true }))).toEqual(['unpin', 'stop-scheduled']);
    expect(menuOf({ e, row: task({ ended_ms: 5 }) })).toEqual([]);
  });

  it("a tab: its thing's menu (no Move to right pane, it is there), then the tab's own", () => {
    const inner = agentFacts(agent({ status: 'working' }));
    const acts = actsOf(menuOf({ e: { kind: 'tab', id: 'agent:acme/perf' }, inner, moveable: false, closable: true, others: true }));
    expect(acts).not.toContain('move-right');
    expect(acts.slice(-2)).toEqual(['close-tab', 'close-others']);
    expect(acts).toContain('archive');
    // a terminal: it moves to the bottom; alone, no Close other tabs
    expect(actsOf(menuOf({ e: { kind: 'tab', id: 'term:acme#1' }, inner: undefined, moveable: true, closable: true, others: false }))).toEqual(['move-bottom', 'close-tab']);
    // Overview never closes (nm-panel's closable): no Close Tab, Close Other Tabs stays
    expect(actsOf(menuOf({ e: { kind: 'tab', id: 'overview' }, inner: undefined, moveable: false, closable: false, others: true }))).toEqual(['close-others']);
  });
});

describe('every menu', () => {
  const all: MenuFacts[] = [
    agentFacts(agent({ status: 'working' })),
    agentFacts(agent({ archived: true })),
    agentFacts(agent({ main: true, name: 'main' }), { pinned: true }),
    { e: { kind: 'project', project: P }, row: { project: P, name: 'acme', path: '/w/acme' } as ProjectRow },
  ];

  it('no dimmed item (hidden instead), nothing destructive (archive is undone by restore), no separator at an end or twice', () => {
    for (const f of all) {
      const items = menuOf(f);
      const flat = (xs: readonly PopupItem[]): PopupItem[] => xs.flatMap((i) => (i.type === 'submenu' ? [i, ...flat(i.items)] : [i]));
      for (const i of flat(items)) {
        expect(i).not.toHaveProperty('enabled');
        expect(i).not.toHaveProperty('destructive');
      }
      expect(items[0]?.type).not.toBe('separator');
      expect(items[items.length - 1]?.type).not.toBe('separator');
      items.forEach((i, n) => {
        if (i.type === 'separator') expect(items[n + 1]?.type).not.toBe('separator');
      });
    }
  });

  it("a key once per menu; ⌥⌘0-4 left free (the panel's ⌥⌘0, nm-side's bubbles ⌥⌘1-4); Archive has none (⌘⇧A is the artifacts page)", () => {
    const keys = Object.entries(KEYS).filter(([act]) => act !== 'unpin');
    const accs = keys.map(([, k]) => k);
    expect(new Set(accs).size).toBe(accs.length);
    for (const n of [0, 1, 2, 3, 4]) expect(accs).not.toContain(`Alt+CmdOrCtrl+${String(n)}`);
    // ⌘B, ⌥⌘B the two sides, ⌃⌘S their hidden alias (app/keys.ts FRAME_KEYS): no menu item takes them
    for (const a of ['CmdOrCtrl+B', 'Alt+CmdOrCtrl+B', 'Ctrl+CmdOrCtrl+S']) expect(accs).not.toContain(a);
    expect(KEYS.archive).toBeUndefined();
    for (const f of all) {
      const shown = menuOf(f).flatMap((i) => (i.type === 'item' || i.type === undefined ? (i.accelerator === undefined ? [] : [i.accelerator]) : []));
      expect(new Set(shown).size).toBe(shown.length);
    }
  });

  it('every item has its words from the catalog (no empty label)', () => {
    for (const f of all) for (const act of actsOf(menuOf(f))) expect(act satisfies Act).not.toBe('');
    for (const f of all) expect(shape(menuOf(f)).every((l) => l.trim() !== '')).toBe(true);
  });
});
