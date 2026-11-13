// native/mac-keys · bar ⛔2, the app's part (docs/desktop-bar.md); the field's keys are qa-type's
// composer/edit-keys. Passes when each mac key shows in a real menu bar with its key and the
// standard action, the app's menu says bise (not the package's name), and the bise items (⌘, ⌘I)
// reach the window even when no window of bise is focused (the window closed with ⌘W, or the
// spotlight has the keys: the capsule's page doesn't route them), and Quit ends the app and its
// helper.
// What this run can't see: a key pressed on the keyboard reaches NSMenu only through the window
// server with a key window. Offscreen, Playwright's keys (CDP) and webContents.sendInputEvent go to
// the renderer and never to the menu (measured: ⌘I ⌘M ⌘W ⌘, change nothing, before-input-event
// sees the key, the menu doesn't). So here bise's own items are clicked the way NSMenu clicks them
// (MenuItem.click); the native roles (quit, hide, close, minimize, undo...) can't be clicked from
// here at all on macOS (Electron leaves them to NSMenu): their key and action, like every key
// pressed for real, are the VM pass's (native/README.md).
import { test, expect, underLoad } from '../../harness';
import { appMenu, clickMenu, flat, ids, onWorld, sleep } from './world';

/** every key ⛔2 names, where the menu bar must show it, its role and its accelerator */
const KEYS: readonly { path: string; role?: string; key: RegExp }[] = [
  { path: 'bise/Settings…', key: /^(CmdOrCtrl|CommandOrControl|Command|Cmd)\+,$/ },
  { path: 'bise/Hide bise', role: 'hide', key: /^(Command|Cmd|CmdOrCtrl|CommandOrControl)\+H$/i },
  { path: 'bise/Quit bise', role: 'quit', key: /^(CommandOrControl|CmdOrCtrl|Command|Cmd)\+Q$/i },
  // ⌘W closes the panel's tab in front, never bise's window; Close Window is ⇧⌘W, as Safari and
  // Chrome (7da5658be, designer m_53158)
  { path: 'File/Close Tab', key: /^(CommandOrControl|CmdOrCtrl|Command|Cmd)\+W$/i },
  { path: 'File/Close Window', role: 'close', key: /^Shift\+(CommandOrControl|CmdOrCtrl|Command|Cmd)\+W$/i },
  { path: 'Edit/Undo', role: 'undo', key: /^(CommandOrControl|CmdOrCtrl|Command|Cmd)\+Z$/i },
  { path: 'Edit/Redo', role: 'redo', key: /^Shift\+(CommandOrControl|CmdOrCtrl|Command|Cmd)\+Z$/i },
  { path: 'Edit/Cut', key: /^(CmdOrCtrl|CommandOrControl)\+X$/i },
  { path: 'Edit/Copy', key: /^(CmdOrCtrl|CommandOrControl)\+C$/i },
  { path: 'Edit/Paste', key: /^(CmdOrCtrl|CommandOrControl)\+V$/i },
  { path: 'Edit/Select All', role: 'selectall', key: /^(CommandOrControl|CmdOrCtrl)\+A$/i },
  { path: 'Edit/Find', key: /^(CmdOrCtrl|CommandOrControl)\+F$/i },
  { path: 'Go/Inbox', key: /^(CmdOrCtrl|CommandOrControl)\+I$/i },
  { path: 'Window/Minimize', role: 'minimize', key: /^(CommandOrControl|CmdOrCtrl)\+M$/i },
];

test('native/mac-keys: the menu bar is bise\'s, with every mac key and its standard action', async () => {
  await onWorld(async ({ s }) => {
    const menu = await appMenu(s);
    expect.soft(menu[0]?.label, "the app's menu is named bise (today: the package's name)").toBe('bise');
    const all = flat(menu);
    const named = all.filter((e) => /bise-desktop/.test(e.label)).map((e) => e.path);
    expect.soft(named, 'no item says bise-desktop').toEqual([]);
    // the paths with the app's menu called bise, whatever it is called today (only the app menu's
    // first segment is renamed: File/Edit/Go/Window keep theirs)
    const appLabel = menu[0]?.label ?? '';
    const asBise = (p: string): string => (p === appLabel || p.startsWith(`${appLabel}/`) ? `bise${p.slice(appLabel.length)}` : p).replace(/bise-desktop/g, 'bise');
    const byPath = new Map(all.map((e) => [asBise(e.path), e]));
    for (const k of KEYS) {
      const e = byPath.get(k.path);
      expect.soft(e, `${k.path} is in the menu bar`).toBeDefined();
      if (!e) continue;
      expect.soft(e.key ?? '', `${k.path} shows its key`).toMatch(k.key);
      if (k.role !== undefined) expect.soft(e.role, `${k.path} is the standard action`).toBe(k.role);
    }
  });
});

test('native/mac-keys: Settings… and Inbox reach the window with no bise window focused', async () => {
  await onWorld(async ({ s }) => {
    const focused = await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getFocusedWindow() !== null);
    expect(focused, 'this run has no focused window (test mode: never focusable)').toBe(false);
    const menu = await appMenu(s);
    await clickMenu(s, `${menu[0]?.label ?? 'bise'}/Settings…`);
    await expect
      // 1.5 s on an idle Mac, stretched by the load (harness/load.ts: 11.11's pass timed out at load ~15)
      .poll(async () => (await ids(s.page)).some((i) => i.startsWith('settings')), { message: 'bise › Settings… opens settings in the window', timeout: underLoad(1500) })
      .toBe(true);
    await clickMenu(s, 'Go/Inbox');
    await expect
      .poll(async () => (await ids(s.page)).some((i) => i === 'inbox' || i.startsWith('inbox.') || i === 'app.notyet.inbox'), { message: 'Go › Inbox opens the inbox in the window', timeout: underLoad(1500) })
      .toBe(true);
  });
});

test('native/mac-keys: quitting ends the app and tells the helper to quit', async () => {
  await onWorld(async ({ s, helper }) => {
    // Quit is a native role on macOS: its item runs NSApp terminate:, which no click from here can
    // run (MenuItem.click leaves mac roles to NSMenu). terminate: ends in app.quit(), so the quit
    // starts there; the key and the item are the VM pass's.
    const exited = new Promise<number | null>((r) => s.app.process().once('exit', (code) => { r(code); }));
    await s.app.evaluate(({ app }) => { app.quit(); });
    const code = await Promise.race([exited, sleep(3000).then(() => 'still running' as const)]);
    expect(code, 'the app is gone within 3 s').not.toBe('still running');
    expect(helper.cmds().some((c) => c.cmd === 'quit'), 'the helper was told quit (it never outlives the app)').toBe(true);
  });
});
