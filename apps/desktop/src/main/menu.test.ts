import type { MenuItemConstructorOptions } from 'electron';
import { describe, expect, it } from 'vitest';
import type { GoAction, MenuAction } from '../shared/api';
import { PLACE_KEYS } from '../settings/keys';
import type { ClipOp } from './clipfence';
import { externalUrl } from './external';
import { menuTarget, menuTemplate } from './menu';
import { nativeItems, popupPoint } from './menu-popup';
import { menuWindow } from './agentmenu';
import { AGENT_MENU_IDLE, KEYS, menuOf } from '../app/menus/registry';
import { menus } from '../proto/words_menus.gen';

function flat(items: readonly MenuItemConstructorOptions[]): MenuItemConstructorOptions[] {
  return items.flatMap((i) => [i, ...(Array.isArray(i.submenu) ? flat(i.submenu) : [])]);
}

function build(platform: NodeJS.Platform, dev = false): { items: MenuItemConstructorOptions[]; sent: MenuAction[] } {
  const sent: MenuAction[] = [];
  const t = menuTemplate({ platform, appName: 'bise', dev, send: (a) => sent.push(a) });
  return { items: flat(t), sent };
}

function buildTest(): { items: MenuItemConstructorOptions[]; ops: ClipOp[] } {
  const ops: ClipOp[] = [];
  const t = menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: () => undefined, clip: (op) => ops.push(op) });
  return { items: flat(t), ops };
}

const roles = (items: MenuItemConstructorOptions[]): Set<string> =>
  new Set(items.flatMap((i) => (i.role === undefined ? [] : [i.role])));

describe('the application menu (bar ⛔2)', () => {
  it('has every edit key as the OS action, on mac and elsewhere', () => {
    for (const p of ['darwin', 'win32', 'linux'] as const) {
      const r = roles(build(p).items);
      for (const want of ['undo', 'redo', 'cut', 'copy', 'paste', 'selectAll', 'close']) expect(r, `${p} ${want}`).toContain(want);
    }
  });

  it('has the mac app keys: ⌘H hide, ⌘Q quit, ⇧⌘W close the window, the Window menu', () => {
    const r = roles(build('darwin').items);
    for (const want of ['hide', 'hideOthers', 'quit', 'close', 'windowMenu', 'about']) expect(r).toContain(want);
    expect(build('darwin').items.find((i) => i.role === 'quit')?.accelerator).toBeUndefined();
  });

  it('⌘W never closes the window (cmd-w-artifact, his m_53150, designer m_53158): File › Close Tab sends closeTab, dimmed with nothing to close; Close Window is ⇧⌘W', () => {
    for (const p of ['darwin', 'win32', 'linux'] as const) {
      const { items, sent } = build(p);
      const w = items.filter((i) => i.accelerator === 'CmdOrCtrl+W');
      expect(w.map((i) => [i.label, i.role]), p).toEqual([['Close Tab', undefined]]);
      (w[0]?.click as (() => void) | undefined)?.();
      expect(sent, p).toEqual(['closeTab']);
      expect(w[0]?.enabled, p).toBe(false);
      for (const c of items.filter((i) => i.role === 'close')) expect([c.label, c.accelerator], p).toEqual(['Close Window', 'Shift+CmdOrCtrl+W']);
    }
    const lit = flat(menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: () => undefined, sides: { sidebar: true, panel: true, closeTab: true } }));
    expect(lit.find((i) => i.label === 'Close Tab')?.enabled).toBe(true);
    // the main window's renderer decides (its panel's tab, or nothing); another bise window closes
    expect(menuTarget('closeTab', true, true)).toEqual({ send: 'focused' });
    expect(menuTarget('closeTab', false, true)).toEqual({ close: 'focused' });
    expect(menuTarget('closeTab', false, false)).toEqual({ send: 'focused' });
  });

  it("sends bise's own keys to the renderer: ⌘, settings, ⌘I inbox, ⌘F find, ⌃` terminal, ⌘S save, ⌘N / ⇧⌘N new project / agent, ⌘B / ⌥⌘B the sides, Go's places", () => {
    const { items, sent } = build('darwin');
    // the keyed items only (View › Hide the Bims has no key: outer-world v2, its own test below)
    const keys = new Map(items.filter((i) => i.click !== undefined && i.accelerator !== undefined).map((i) => [i.accelerator, i]));
    expect([...keys.keys()].sort()).toEqual([
      'Alt+CmdOrCtrl+B', 'CmdOrCtrl+,', 'CmdOrCtrl+0', 'CmdOrCtrl+B', 'CmdOrCtrl+Enter', 'CmdOrCtrl+F', 'CmdOrCtrl+I', 'CmdOrCtrl+N', 'CmdOrCtrl+S', 'CmdOrCtrl+W', 'CmdOrCtrl+Y', 'CmdOrCtrl+[', 'CmdOrCtrl+]', 'Ctrl+`',
      'Shift+Alt+CmdOrCtrl+B', 'Shift+CmdOrCtrl+A', 'Shift+CmdOrCtrl+I', 'Shift+CmdOrCtrl+N', 'Shift+CmdOrCtrl+P', 'Shift+CmdOrCtrl+T', 'Shift+CmdOrCtrl+X',
    ]);
    for (const k of ['CmdOrCtrl+,', 'CmdOrCtrl+I', 'CmdOrCtrl+F', 'Ctrl+`', 'CmdOrCtrl+S', 'CmdOrCtrl+N', 'Shift+CmdOrCtrl+N', 'CmdOrCtrl+B', 'Alt+CmdOrCtrl+B', 'Shift+Alt+CmdOrCtrl+B', 'CmdOrCtrl+0', 'Shift+CmdOrCtrl+I', 'CmdOrCtrl+Y', 'Shift+CmdOrCtrl+A', 'Shift+CmdOrCtrl+P', 'Shift+CmdOrCtrl+X', 'Shift+CmdOrCtrl+T', 'CmdOrCtrl+[', 'CmdOrCtrl+]']) {
      const click = keys.get(k)?.click as (() => void) | undefined;
      click?.();
    }
    expect(sent).toEqual(['settings', 'inbox', 'find', 'terminal', 'save', 'newProject', 'newAgent', 'sidebar', 'panel', 'panelFull', 'bise', 'inboxPage', 'scheduled', 'artifacts', 'processes', 'plugins', 'reopenTab', 'back', 'forward']);
  });

  it("Go › Back ⌘[ and Forward ⌘] first, dimmed with nothing on that side; never ⌘← ⌘→, a field's caret moves (history-keys, designer m_58552)", () => {
    const sub = (sides?: { canBack?: boolean; canForward?: boolean }) => {
      const go = menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: () => undefined, ...(sides === undefined ? {} : { sides: { sidebar: true, panel: false, ...sides } }) }).find((i) => i.label === menus.go_menu);
      return Array.isArray(go?.submenu) ? go.submenu : [];
    };
    const [back, forward] = sub();
    expect([back?.label, back?.accelerator, back?.enabled]).toEqual([menus.go_back, 'CmdOrCtrl+[', false]);
    expect([forward?.label, forward?.accelerator, forward?.enabled]).toEqual([menus.go_forward, 'CmdOrCtrl+]', false]);
    const [b2, f2] = sub({ canBack: true });
    expect([b2?.enabled, f2?.enabled]).toEqual([true, false]);
    const all = build('darwin').items.map((i) => i.accelerator);
    expect(all).not.toContain('CmdOrCtrl+Left');
    expect(all).not.toContain('CmdOrCtrl+Right');
  });

  it("File › Reopen Closed Tab ⇧⌘T (his m_46187, designer m_46216), the focused window's own, never a route", () => {
    const file = build('darwin').items;
    expect(file.find((i) => i.label === 'Reopen Closed Tab')?.accelerator).toBe('Shift+CmdOrCtrl+T');
    expect(menuTarget('reopenTab', false)).toEqual({ send: 'focused' });
  });

  it("each Go item has its rail place's key (settings/keys.ts PLACE_KEYS, the glyph the rail shows) and sends its action; ⌘I stays the inbox block (rail-keys)", () => {
    const GO: Record<GoAction, keyof typeof PLACE_KEYS> = { bise: 'bise', inboxPage: 'inbox', scheduled: 'scheduled', artifacts: 'artifacts', processes: 'processes', plugins: 'plugins' };
    const accel = (glyph: string): string => `${glyph.includes('⇧') ? 'Shift+' : ''}CmdOrCtrl+${glyph.slice(-1)}`;
    const sent: MenuAction[] = [];
    const go = menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: (a) => sent.push(a) }).find((i) => i.label === menus.go_menu);
    const sub = (Array.isArray(go?.submenu) ? go.submenu : []);
    for (const [action, place] of Object.entries(GO) as [GoAction, keyof typeof PLACE_KEYS][]) {
      const item = sub.find((i) => i.accelerator === accel(PLACE_KEYS[place]));
      expect(item, action).toBeDefined();
      sent.length = 0;
      (item?.click as (() => void) | undefined)?.();
      expect(sent).toEqual([action]);
    }
    expect(sub.some((i) => i.accelerator === 'CmdOrCtrl+I'), '⌘I the block').toBe(true);
  });

  it('settings once per platform, never twice', () => {
    for (const p of ['darwin', 'win32'] as const) {
      expect(build(p).items.filter((i) => i.accelerator === 'CmdOrCtrl+,')).toHaveLength(1);
    }
  });

  it('no accelerator twice', () => {
    const acc = build('darwin', true).items.flatMap((i) => (typeof i.accelerator === 'string' ? [i.accelerator] : []));
    expect(new Set(acc).size).toBe(acc.length);
  });

  it('test mode: no clipboard role at all, the items play on the private buffer', () => {
    const { items, ops } = buildTest();
    const r = roles(items);
    for (const role of ['cut', 'copy', 'paste', 'pasteAndMatchStyle']) expect(r).not.toContain(role);
    for (const role of ['undo', 'redo', 'selectAll', 'delete']) expect(r).toContain(role);
    for (const k of ['CmdOrCtrl+X', 'CmdOrCtrl+C', 'CmdOrCtrl+V', 'Shift+CmdOrCtrl+V']) {
      const click = items.find((i) => i.accelerator === k)?.click as (() => void) | undefined;
      click?.();
    }
    expect(ops).toEqual(['cut', 'copy', 'paste', 'paste']);
  });

  it('the devtools only in dev builds', () => {
    expect(roles(build('darwin').items)).not.toContain('toggleDevTools');
    expect(roles(build('darwin', true).items)).toContain('toggleDevTools');
  });
});

describe('menuTarget (qa-native m_9724)', () => {
  it('settings and the inbox open the main window when it has no keys (closed, or the capsule has them)', () => {
    expect(menuTarget('settings', false)).toEqual({ open: 'settings' });
    expect(menuTarget('inbox', false)).toEqual({ open: 'inbox' });
    expect(menuTarget('terminal', false)).toEqual({ open: 'terminal' });
  });

  it('View > Terminal is ctrl+` (bar T W25) and sends terminal', () => {
    const { items, sent } = build('darwin');
    const t = items.find((i) => i.label === 'Terminal');
    expect(t?.accelerator).toBe('Ctrl+`');
    (t?.click as (() => void) | undefined)?.();
    expect(sent).toEqual(['terminal']);
  });

  it('the main window with the keys gets them itself; find always goes to the focused page', () => {
    expect(menuTarget('settings', true)).toEqual({ send: 'focused' });
    expect(menuTarget('find', false)).toEqual({ send: 'focused' });
  });

  it('⌘S (art-save) goes to the focused page, never opens a route', () => {
    expect(menuTarget('save', false)).toEqual({ send: 'focused' });
    const { items } = build('darwin');
    expect(items.find((i) => i.accelerator === 'CmdOrCtrl+S')?.label).toBe('Save a Copy…');
  });

  it('File › Open in Browser ⌘⏎ (artifact-shortcut) goes to the focused page; dimmed with no artifact in front', () => {
    expect(menuTarget('browser', false)).toEqual({ send: 'focused' });
    const item = (sides?: { sidebar: boolean; panel: boolean; artifact?: boolean }) => {
      const sent: string[] = [];
      const t = menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: (a) => sent.push(a), ...(sides ? { sides } : {}) });
      const sub = (t.find((m) => m.label === 'File')?.submenu ?? []) as MenuItemConstructorOptions[];
      return { i: sub.find((x) => x.accelerator === 'CmdOrCtrl+Enter'), sent };
    };
    expect(item().i?.label).toBe(menus.open_browser);
    expect(item().i?.enabled).toBe(false);
    const { i, sent } = item({ sidebar: true, panel: false, artifact: true });
    expect(i?.enabled).toBe(true);
    (i?.click as (() => void) | undefined)?.();
    expect(sent).toEqual(['browser']);
  });

  it('File: New Project… ⌘N and New Agent… ⇧⌘N (his m_44225) go to the focused window, never a route', () => {
    const { items, sent } = build('darwin');
    const project = items.find((i) => i.label === 'New Project…');
    const agent = items.find((i) => i.label === 'New Agent…');
    expect(project?.accelerator).toBe('CmdOrCtrl+N');
    expect(agent?.accelerator).toBe('Shift+CmdOrCtrl+N');
    (project?.click as (() => void) | undefined)?.();
    (agent?.click as (() => void) | undefined)?.();
    expect(sent).toEqual(['newProject', 'newAgent']);
    expect(menuTarget('newProject', false)).toEqual({ send: 'focused' });
    expect(menuTarget('newAgent', false)).toEqual({ send: 'focused' });
  });

  it('View: Hide/Show Sidebar ⌘B and Hide/Show Panel ⌥⌘B, first, as the focused window says (designer m_45008)', () => {
    const view = (sides?: { sidebar: boolean; panel: boolean }) => {
      const sent: string[] = [];
      const t = menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: (a) => sent.push(a), ...(sides ? { sides } : {}) });
      const sub = (t.find((m) => m.label === 'View')?.submenu ?? []) as MenuItemConstructorOptions[];
      return { sub, sent };
    };
    // none told yet: the sidebar shown, the panel folded
    expect(view().sub.slice(0, 2).map((i) => [i.label, i.accelerator])).toEqual([['Hide Sidebar', 'CmdOrCtrl+B'], ['Show Panel', 'Alt+CmdOrCtrl+B']]);
    expect(view({ sidebar: false, panel: true }).sub.slice(0, 2).map((i) => i.label)).toEqual(['Show Sidebar', 'Hide Panel']);
    const { sub, sent } = view({ sidebar: true, panel: true });
    for (const i of sub.slice(0, 2)) (i.click as (() => void) | undefined)?.();
    expect(sent).toEqual(['sidebar', 'panel']);
    expect(menuTarget('sidebar', false)).toEqual({ send: 'focused' });
    expect(menuTarget('panel', false)).toEqual({ send: 'focused' });
  });

  it('View: Panel Full Width ⇧⌥⌘B right under the panel, checked in full width, dimmed while the panel is a sheet (panel-full, designer m_45445)', () => {
    const third = (sides?: { sidebar: boolean; panel: boolean; panelFull?: boolean; panelFullCan?: boolean }) => {
      const t = menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: () => undefined, ...(sides ? { sides } : {}) });
      const sub = (t.find((m) => m.label === 'View')?.submenu ?? []) as MenuItemConstructorOptions[];
      const i = sub[2];
      return [i?.label, i?.accelerator, i?.type, i?.checked, i?.enabled];
    };
    expect(third()).toEqual(['Panel Full Width', 'Shift+Alt+CmdOrCtrl+B', 'checkbox', false, true]);
    expect(third({ sidebar: true, panel: true, panelFull: true, panelFullCan: true })).toEqual(['Panel Full Width', 'Shift+Alt+CmdOrCtrl+B', 'checkbox', true, true]);
    expect(third({ sidebar: true, panel: true, panelFull: false, panelFullCan: false })[4]).toBe(false);
    expect(menuTarget('panelFull', false)).toEqual({ send: 'focused' });
  });

  it("View: Compact Side / Show Side Card under Panel Full Width, no key, dimmed when the window holds no card (side-mini, designer m_46706), the window's own", () => {
    const fourth = (sides?: { sidebar: boolean; panel: boolean; sideCompact?: boolean; sideCardCant?: boolean }) => {
      const sent: MenuAction[] = [];
      const t = menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: (a) => sent.push(a), ...(sides ? { sides } : {}) });
      const sub = (t.find((m) => m.label === 'View')?.submenu ?? []) as MenuItemConstructorOptions[];
      const i = sub[3];
      (i?.click as (() => void) | undefined)?.();
      return [i?.label, i?.accelerator, i?.enabled, sent];
    };
    expect(fourth()).toEqual(['Compact Side', undefined, true, ['sideCard']]);
    expect(fourth({ sidebar: true, panel: false, sideCompact: true })).toEqual(['Show Side Card', undefined, true, ['sideCard']]);
    expect(fourth({ sidebar: true, panel: false, sideCardCant: true })[2]).toBe(false);
    expect(menuTarget('sideCard', false)).toEqual({ send: 'focused' });
  });

  it("View: Hide the Bims / Show the Bims under Compact Side, as the focused window's tile is (outer-world v2, his pick #799), the window's own", () => {
    const fourth = (sides?: { sidebar: boolean; panel: boolean; bimsHidden?: boolean }) => {
      const sent: MenuAction[] = [];
      const t = menuTemplate({ platform: 'darwin', appName: 'bise', dev: false, send: (a) => sent.push(a), ...(sides ? { sides } : {}) });
      const sub = (t.find((m) => m.label === 'View')?.submenu ?? []) as MenuItemConstructorOptions[];
      const i = sub[4];
      (i?.click as (() => void) | undefined)?.();
      return [i?.label, i?.accelerator, sent];
    };
    expect(fourth()).toEqual(['Hide the Bims', undefined, ['bims']]);
    expect(fourth({ sidebar: true, panel: false, bimsHidden: true })).toEqual(['Show the Bims', undefined, ['bims']]);
    expect(menuTarget('bims', false)).toEqual({ send: 'focused' });
  });
});

describe('externalUrl (window.bise.openExternal)', () => {
  it('opens web and mail links', () => {
    expect(externalUrl('https://github.com/acme/checkout/pull/42')).toBe('https://github.com/acme/checkout/pull/42');
    expect(externalUrl('http://localhost:5173/')).toBe('http://localhost:5173/');
    expect(externalUrl('mailto:camille@example.com')).toBe('mailto:camille@example.com');
  });

  it('refuses every other scheme, junk and non-strings', () => {
    for (const bad of ['file:///etc/passwd', 'javascript:alert(1)', 'vscode://file/x', 'x-apple.systempreferences:a', '', 'not a url', 'https://']) {
      expect(externalUrl(bad), bad).toBeNull();
    }
    expect(externalUrl(42)).toBeNull();
    expect(externalUrl(undefined)).toBeNull();
    expect(externalUrl(`https://a.example/${'x'.repeat(9000)}`)).toBeNull();
  });
});

describe("Help › 'send a test notification' (a0, lead m_14515)", () => {
  it('try and check builds and tests have it; his release app never does', async () => {
    const { hasTestNotify, menuTemplate } = await import('./menu');
    expect(hasTestNotify('bise try', false)).toBe(true);
    expect(hasTestNotify('bise check', false)).toBe(true);
    expect(hasTestNotify('bise', true)).toBe(true);
    expect(hasTestNotify('bise', false)).toBe(false);
    let clicked = 0;
    const base = { platform: 'darwin' as const, appName: 'bise try', dev: false, send: () => undefined };
    const help = menuTemplate({ ...base, testNotify: () => (clicked += 1) }).find((m) => m.role === 'help');
    const item = (help?.submenu as { label?: string; click?: () => void }[] | undefined)?.[0];
    expect(item?.label).toBe('send a test notification');
    item?.click?.();
    expect(clicked).toBe(1);
    expect(menuTemplate(base).some((m) => m.role === 'help')).toBe(false);
  });
});

describe("the Agent menu (bise fait mac, d4): the agent in view's '…' in the menu bar", () => {
  const perf = { name: 'perf', main: false, status: 'working', archived: false, title: '', purpose: '', since_ms: 1, waits: 0, dir: '/h/perf', path: '/w/perf', changes: { files: 1, add: 1, del: 0 }, watching: [] as never[] } as const;
  const items = menuOf({ e: { kind: 'agent', project: 'acme', agent: 'perf' }, row: perf, pinned: false, reply: false });
  const base = { platform: 'darwin' as const, appName: 'bise', dev: true, send: () => undefined };

  it("with no agent in view: the same menu, every item dimmed (designer m_42681); with one, its items, keys live, a pick says its id", () => {
    const idle = menuTemplate({ ...base, agent: nativeItems(AGENT_MENU_IDLE, () => undefined, () => undefined, true, true) }).find((m) => m.label === menus.agent_menu);
    const idleItems = flat((idle?.submenu ?? []) as MenuItemConstructorOptions[]).filter((i) => i.type !== 'separator');
    expect(idleItems.length).toBeGreaterThan(0);
    expect(idleItems.every((i) => i.enabled === false)).toBe(true);
    // d4's items, none hidden for want of an agent
    expect(idleItems.map((i) => i.id).filter((id) => id !== undefined)).toEqual(expect.arrayContaining(['rename', 'pin', 'changes', 'move-right', 'new-window', 'stop', 'archive']));
    const picked: string[] = [];
    const t = menuTemplate({ ...base, agent: nativeItems(items, (id) => picked.push(id), () => undefined, true) });
    const agent = t.find((m) => m.label === menus.agent_menu);
    const sub = flat((agent?.submenu ?? []) as MenuItemConstructorOptions[]);
    const stop = sub.find((i) => i.id === 'stop');
    expect(stop?.accelerator).toBe(KEYS.stop);
    expect(stop?.registerAccelerator).toBe(true);
    (stop?.click as (() => void) | undefined)?.();
    expect(picked).toEqual(['stop']);
    // nothing dimmed: an item it can't do is not there
    expect(sub.some((i) => i.enabled === false)).toBe(false);
    // its place: after View, before Window (macOS's order)
    const labels = t.map((m) => m.label ?? m.role);
    expect(labels.indexOf(menus.agent_menu)).toBe(labels.indexOf('View') + 1);
  });

  it('no accelerator twice across the whole bar; ⌥⌘0-4 left to the panel and the bubbles; ⌘B and ⌥⌘B the View menu\'s only', () => {
    const all = flat(menuTemplate({ ...base, agent: nativeItems(items, () => undefined, () => undefined, true) }));
    const acc = all.flatMap((i) => (typeof i.accelerator === 'string' ? [i.accelerator] : []));
    expect(acc).toContain('CmdOrCtrl+.');
    expect(new Set(acc).size).toBe(acc.length);
    for (const n of [0, 1, 2, 3, 4]) expect(acc).not.toContain(`Alt+CmdOrCtrl+${String(n)}`);
    expect(acc.filter((a) => a === 'CmdOrCtrl+B' || a === 'Alt+CmdOrCtrl+B' || a === 'Ctrl+CmdOrCtrl+S')).toEqual(['CmdOrCtrl+B', 'Alt+CmdOrCtrl+B']);
  });

  it("a popup's keys are shown, never registered (the menu bar's are the live ones)", () => {
    const pop = flat(nativeItems(items, () => undefined, () => undefined, false));
    expect(pop.filter((i) => i.accelerator !== undefined).every((i) => i.registerAccelerator === false)).toBe(true);
  });

  it("the '…' opens its menu under the button, at its left edge; a right click at the pointer", () => {
    expect(popupPoint({ x: 100.4, y: 40, width: 28, height: 28 })).toEqual({ x: 100, y: 68 });
    expect(popupPoint(undefined)).toBeUndefined();
  });
});

describe("the window the Agent menu speaks for (ui-review m_42540)", () => {
  it('the focused one; none focused (another app, an off-screen window): the last focused, unless it closed', () => {
    const gone = (w: string) => w === 'closed';
    expect(menuWindow('a', 'b', gone)).toBe('a');
    expect(menuWindow(null, 'b', gone)).toBe('b');
    expect(menuWindow(null, 'closed', gone)).toBeNull();
    expect(menuWindow<string>(null, null, gone)).toBeNull();
  });
});
