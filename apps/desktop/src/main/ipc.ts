// Main's side of window.bise (src/shared/api.ts, built by preload.ts): the
// links he opens, the clipboard, the menu's own keys. In test mode nothing
// leaves the app: a link and a copy become stdout lines the harness reads
// (EXTERNAL_LINE, COPY_LINE), never his browser or his clipboard.
import { appendFile, existsSync, readFileSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, extname, isAbsolute, join } from 'node:path';
import { app, BrowserWindow, clipboard, dialog, ipcMain, nativeImage, session, shell, type OpenDialogOptions } from 'electron';
import { COPY_LINE, EXTERNAL_LINE, IPC, MIC_SETTINGS_URL, OPEN_PATH_LINE, QUICKLOOK_LINE, REVEAL_LINE, SAVE_COPY_LINE, TEST_CLIP_FILE_ENV, TEST_CLIP_HOOK, TEST_CLIP_TEXT_ENV, TEST_PICK_FILES_ENV, TEST_PICK_FOLDER_ENV, TEST_SAVE_TO_ENV, type A11y, type FileFacts, type MenuAction, type PopupAt, type PopupItem, type SaveResult, type SkillRead, type ThumbAnswer } from '../shared/api';
import { saveCopy } from './savecopy';
import { a11yNow } from './a11y';
import { popupMenu } from './menu-popup';
import { setAgentMenu, setViewMenu } from './agentmenu';
import { cleanLine, cleanPath, openPlan } from './openpath';
import type { ClipFile, ClipOp, ClipSeed } from './clipfence';
import { menuTarget } from './menu';
import { externalUrl } from './external';
import { isOffscreen, noAsks, offscreenGrants } from './offscreen';
import { imageDir, saveImage } from './images';
import { pickedFilesInTest, pickedInTest } from './pick';
import { allowedPath, realLookable } from './reveal';
import { probePage } from './probe';
import { readSkill, skillPaths } from './skillfile';
import { onCoreEvent } from './core/ipc';
import { thumbsDir } from './thumbs/cache';
import { thumb, type ThumbEnv } from './thumbs/make';
import { viewPermissionIn } from './viewframes';

// the page's async clipboard API (navigator.clipboard): its permissions
const CLIPBOARD_PERMS = new Set(['clipboard-read', 'clipboard-sanitized-write']);

const MAX_COPY = 1_000_000;

const MIME: Readonly<Record<string, string>> = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.pdf': 'application/pdf',
  '.txt': 'text/plain',
};

/** Test mode: a file's bytes for a seed (unreadable: null, said on stderr). */
function readSeedFile(what: string, p: string | undefined): Buffer | null {
  if (p === undefined || p === '') return null;
  try {
    return readFileSync(p);
  } catch (e) {
    process.stderr.write(`clip seed: can't read ${what}=${p}: ${String(e)}\n`);
    return null;
  }
}

/** A seed's file item from its path and bytes: its name and its type from the extension (an image is image/png...). */
export function clipFileOf(path: string, bytes: Uint8Array): ClipFile {
  return { name: basename(path), type: MIME[extname(path).toLowerCase()] ?? 'application/octet-stream', bytes };
}

/** Test mode: the clipboard's seed from the env's files (unreadable: left out, said on stderr). */
function clipSeed(): ClipSeed {
  const text = readSeedFile(TEST_CLIP_TEXT_ENV, process.env[TEST_CLIP_TEXT_ENV])?.toString('utf8');
  const path = process.env[TEST_CLIP_FILE_ENV] ?? '';
  const bytes = readSeedFile(TEST_CLIP_FILE_ENV, path);
  const files: ClipFile[] = bytes === null ? [] : [clipFileOf(path, new Uint8Array(bytes))];
  return { ...(text === undefined ? {} : { text }), files };
}

let clipNonce = 0;

/**
 * Test mode: TEST_CLIP_HOOK's seed ({file?: absolute path, text?}) put on every window's private
 * clipboard at once (lead m_12801: a check stages an image for a real ⌘V while the app runs).
 * False when nothing usable was given or the file can't be read.
 */
async function setClip(raw: unknown): Promise<boolean> {
  const o = typeof raw === 'object' && raw !== null ? (raw as Record<string, unknown>) : {};
  const text = typeof o['text'] === 'string' ? o['text'] : undefined;
  const path = typeof o['file'] === 'string' && isAbsolute(o['file']) ? o['file'] : undefined;
  const bytes = path === undefined ? null : readSeedFile('file', path);
  if (path !== undefined && bytes === null) return false;
  const files = path === undefined || bytes === null ? [] : [clipFileOf(path, new Uint8Array(bytes))];
  if (text === undefined && files.length === 0) return false;
  const seed: ClipSeed = { ...(text === undefined ? {} : { text }), files };
  const wins = BrowserWindow.getAllWindows().filter((w) => !w.isDestroyed());
  const nonce = ++clipNonce;
  // every window answers after its fence has the seed (the IPC is async: a ⌘V right after
  // the hook would otherwise paste the old buffer)
  const done = new Promise<boolean>((resolve) => {
    let left = wins.length;
    const on = (_e: unknown, n: unknown): void => {
      if (n !== nonce) return;
      left -= 1;
      if (left <= 0) finish(true);
    };
    const timer = setTimeout(() => {
      finish(false);
    }, 2000);
    const finish = (ok: boolean): void => {
      clearTimeout(timer);
      ipcMain.removeListener(IPC.clipSetDone, on);
      resolve(ok);
    };
    ipcMain.on(IPC.clipSetDone, on);
    if (left === 0) finish(true);
  });
  for (const w of wins) w.webContents.send(IPC.clipSet, seed, nonce);
  return done;
}

export function installIpc(testMode: boolean): void {
  if (testMode) {
    ipcMain.handle(IPC.clipSeed, () => clipSeed());
    (globalThis as unknown as Record<string, unknown>)[TEST_CLIP_HOOK] = setClip;
  }
  // the app's own pages: an off-screen run denies a page's permission (mic, camera, screen,
  // notifications...) before macOS can ask him, only the clipboard passes (offscreen.ts); test mode:
  // navigator.clipboard can never read or write his clipboard; his run: electron's default, granted
  const quiet = noAsks(process.env, testMode);
  const base = (permission: string): boolean => (quiet ? offscreenGrants(permission) : testMode ? !CLIPBOARD_PERMS.has(permission) : true);
  // an MCP Apps view's (viewrules.ts): only what its frame was granted, before the rule above; a
  // subframe that isn't the app's own (file:, the pages server), a frame nested in a view: denied
  const s = session.defaultSession;
  s.setPermissionCheckHandler((wc, permission, requestingOrigin, d) => viewPermissionIn(wc, permission, { ...d, requestingOrigin }) ?? base(permission));
  s.setPermissionRequestHandler((wc, permission, done, d) => {
    done(viewPermissionIn(wc, permission, d) ?? base(permission));
  });
  ipcMain.handle(IPC.openExternal, (_e, raw: unknown): Promise<boolean> => openLink(raw, testMode));
  // voice mode's failed line: System Settings › Privacy › Microphone, the one settings URL a window may open
  ipcMain.on(IPC.micSettings, () => {
    if (testMode) {
      process.stdout.write(`${EXTERNAL_LINE} ${MIC_SETTINGS_URL}
`);
      return;
    }
    void shell.openExternal(MIC_SETTINGS_URL);
  });
  // a window's line for app.log, next to core.log (his dictation's decisions; we were blind on his Mac)
  ipcMain.on(IPC.appLog, (_e, raw: unknown) => {
    const line = appLogLine(raw, new Date());
    if (line !== null) appendFile(join(app.getPath('userData'), 'app.log'), line, () => undefined);
  });
  ipcMain.handle(IPC.openPath, (_e, raw: unknown, opts: unknown): Promise<boolean> => openFile(raw, opts, testMode));
  ipcMain.handle(IPC.quickLook, (e, raw: unknown): boolean => quickLook(BrowserWindow.fromWebContents(e.sender), raw, testMode));
  ipcMain.handle(IPC.showInFinder, (_e, raw: unknown): boolean => showInFinder(raw, testMode));
  // page-unreach: does an artifact's page server answer, before the panel frames it (probe.ts)
  ipcMain.handle(IPC.probePage, (_e, raw: unknown): Promise<'up' | 'down' | 'gone'> => probePage(raw));
  // the skill tab (nm-pages-b): a skill's SKILL.md and its folder, fenced to the latest plugins rows' skill paths
  // kept per project: a later plugins event of another project (or of none) never wipes the paths a
  // tab open on this one reads (a built-in skill gave not_allowed; designer m_43933 #12)
  const skillsBy = new Map<string, string[]>();
  onCoreEvent((ev) => {
    if (ev['ev'] !== 'plugins') return;
    const project = ev['project'];
    skillsBy.set(typeof project === 'string' ? project : '', skillPaths(ev));
  });
  ipcMain.handle(IPC.readSkill, (_e, raw: unknown, file: unknown): Promise<SkillRead> => readSkill(raw, file, [...skillsBy.values()].flat()));
  // art-save: an artifact's file copied where he says (savecopy.ts), behind the same fence
  ipcMain.handle(IPC.saveCopy, (e, raw: unknown, name: unknown): Promise<SaveResult> => {
    const p = lookable(raw);
    if (p === null) return Promise.resolve(null);
    return saveCopy(BrowserWindow.fromWebContents(e.sender), p, name, {
      testMode,
      testTo: process.env[TEST_SAVE_TO_ENV],
      downloads: () => app.getPath('downloads'),
      panel: (win, opts) => panelOpen(() => (win === null ? dialog.showSaveDialog(opts) : dialog.showSaveDialog(win, opts))),
      line: (rec) => {
        process.stdout.write(`${SAVE_COPY_LINE} ${JSON.stringify(rec)}
`);
      },
    });
  });
  ipcMain.handle(IPC.fileIcon, (_e, raw: unknown): Promise<string | null> => fileIcon(raw, testMode));
  ipcMain.handle(IPC.fileFacts, (_e, raw: unknown): FileFacts | null => fileFacts(raw));
  // art-thumbs: an artifact card's picture, kept per version in bise's thumbs cache (thumbs/make.ts); its
  // reads behind the realpath fence (architect m_39772: a symlink in a project can't reach out)
  const thumbEnv: ThumbEnv = { root: thumbsDir, lookable: (raw) => realLookable(raw, [homedir(), ...roots]), quickLook: !testMode && !isOffscreen(process.env, testMode) };
  ipcMain.handle(IPC.thumb, (_e, raw: unknown, want: unknown): Promise<ThumbAnswer | null> => thumb(thumbEnv, raw, want));
  ipcMain.handle(IPC.pickFolder, (e): Promise<string | null> => pickFolder(BrowserWindow.fromWebContents(e.sender), testMode));
  ipcMain.handle(IPC.pickFiles, (e): Promise<readonly string[]> => pickFiles(BrowserWindow.fromWebContents(e.sender), testMode));
  ipcMain.handle(IPC.popupMenu, (e, items: unknown, at: unknown): Promise<string | null> => popupMenu(BrowserWindow.fromWebContents(e.sender), Array.isArray(items) ? (items as PopupItem[]) : [], (at ?? undefined) as PopupAt | undefined));
  ipcMain.handle(IPC.a11y, (): A11y => a11yNow());
  ipcMain.on(IPC.agentMenu, (e, items: unknown) => { setAgentMenu(e.sender, items); });
  ipcMain.on(IPC.viewMenu, (e, sides: unknown) => { setViewMenu(e.sender, sides); });
  // R41: a pasted or dropped image into the TUI's image store (the test home's under test)
  ipcMain.handle(IPC.saveImage, (_e, bytes: unknown): string | null => saveImage(imageDir(process.env), bytes));
  ipcMain.handle(IPC.copy, async (_e, text: unknown): Promise<void> => {
    if (typeof text !== 'string' || text.length > MAX_COPY) return;
    if (testMode) {
      process.stdout.write(`${COPY_LINE} ${JSON.stringify(text)}\n`);
      return;
    }
    await clipboard.writeText(text);
  });
}

/**
 * A link out of the app: http, https or mailto in his browser or mail app
 * (externalUrl), refused otherwise; in test mode only EXTERNAL_LINE on stdout.
 * The one path for window.bise.openExternal and the guard's links.
 */
/** A window's app.log line: its time, one line, 300 chars at most; null when it isn't text. Pure. */
export function appLogLine(raw: unknown, at: Date): string | null {
  if (typeof raw !== 'string' || raw.trim() === '') return null;
  return `${at.toISOString()} ${raw.replace(/\s+/gu, ' ').trim().slice(0, 300)}\n`;
}

export async function openLink(raw: unknown, testMode: boolean): Promise<boolean> {
  const url = externalUrl(raw);
  if (url === null) return false;
  if (testMode) {
    process.stdout.write(`${EXTERNAL_LINE} ${url}\n`);
    return true;
  }
  await shell.openExternal(url);
  return true;
}

/**
 * A file or folder of an answer in his editor at its line, else in Finder
 * (openpath.ts); refused when not absolute or missing. In test mode only
 * OPEN_PATH_LINE on stdout (the plan as if his machine had no editor
 * installed: the harness's machine says nothing about his).
 */
export async function openFile(raw: unknown, opts: unknown, testMode: boolean): Promise<boolean> {
  const path = cleanPath(raw);
  if (path === null || !existsSync(path)) return false;
  const line = cleanLine(opts);
  if (testMode) {
    const plan = openPlan(path, line, process.env, () => false);
    process.stdout.write(`${OPEN_PATH_LINE} ${JSON.stringify({ ...plan, ...(line === undefined ? {} : { line }) })}
`);
    return true;
  }
  const plan = openPlan(path, line, process.env, (scheme) => app.getApplicationNameForProtocol(`${scheme}://`) !== '');
  if (plan.how === 'editor') await shell.openExternal(plan.url);
  else shell.showItemInFolder(plan.path);
  return true;
}

/** His home and project folders: where quickLook and showInFinder may look (reveal.ts). */
let roots: readonly string[] = [];
/** The core's projects rows changed (index.ts). */
export function setProjectRoots(paths: readonly string[]): void {
  roots = paths;
}
const lookable = (raw: unknown): string | null => {
  const p = allowedPath(raw, [homedir(), ...roots]);
  return p !== null && existsSync(p) ? p : null;
};

/** window.bise.quickLook: macOS's Quick Look panel of `win` on the file; test mode a stdout line. */
export function quickLook(win: BrowserWindow | null, raw: unknown, testMode: boolean): boolean {
  const p = lookable(raw);
  if (p === null) return false;
  if (testMode) {
    process.stdout.write(`${QUICKLOOK_LINE} ${p}\n`);
    return true;
  }
  if (win === null || process.platform !== 'darwin') return false;
  win.previewFile(p);
  return true;
}

/** window.bise.showInFinder: the file selected in a Finder window; test mode a stdout line. */
export function showInFinder(raw: unknown, testMode: boolean): boolean {
  const p = lookable(raw);
  if (p === null) return false;
  if (testMode) {
    process.stdout.write(`${REVEAL_LINE} ${p}\n`);
    return true;
  }
  shell.showItemInFolder(p);
  return true;
}

/** window.bise.fileIcon: the file's small system icon as a data URL; null in test mode and
 *  off-screen (no TCC call there, amb-mac m_21862: getFileIcon reads the file), out of the fence,
 *  or when the system has none (the chip draws its ▯ glyph). Only his own attachments ask. */
export async function fileIcon(raw: unknown, testMode: boolean): Promise<string | null> {
  if (testMode || isOffscreen(process.env, testMode)) return null;
  const p = lookable(raw);
  if (p === null) return null;
  try {
    const img = await app.getFileIcon(p, { size: 'small' });
    return img.isEmpty() ? null : img.toDataURL();
  } catch {
    return null;
  }
}

/** window.bise.fileFacts (at-chips): a picked file's size, and an image's pixels and 48 px
 *  thumbnail (decoded here, never QuickLook); null out of the fence or unreadable. Test mode
 *  reads too: the harness's files are its own. */
export function fileFacts(raw: unknown): FileFacts | null {
  const p = lookable(raw);
  if (p === null) return null;
  try {
    const bytes = statSync(p).size;
    if (MIME[extname(p).toLowerCase()]?.startsWith('image/') !== true) return { bytes };
    const img = nativeImage.createFromPath(p);
    if (img.isEmpty()) return { bytes };
    const { width: w, height: h } = img.getSize();
    const k = Math.min(1, 48 / Math.max(w, h));
    const thumb = img.resize({ width: Math.max(1, Math.round(w * k)), height: Math.max(1, Math.round(h * k)) }).toDataURL();
    return { bytes, w, h, thumb };
  } catch {
    return null;
  }
}

/**
 * His folder pick (window.bise.pickFolder): macOS's open panel for folders,
 * attached to `win`, starting in his home; null on cancel. Test mode: the
 * env's absolute TEST_PICK_FOLDER_ENV at once, else null, never a panel.
 */
export async function pickFolder(win: BrowserWindow | null, testMode: boolean): Promise<string | null> {
  if (testMode) return pickedInTest(process.env[TEST_PICK_FOLDER_ENV]);
  const opts: OpenDialogOptions = { properties: ['openDirectory', 'createDirectory'], defaultPath: homedir() };
  const r = await panelOpen(() => (win === null ? dialog.showOpenDialog(opts) : dialog.showOpenDialog(win, opts)));
  return r.canceled ? null : (r.filePaths[0] ?? null);
}

/** The open panels now (his picks): the app's update never installs under one (update/moment.ts). */
let panels = 0;

/** A panel is open. */
export function panelShown(): boolean {
  return panels > 0;
}

async function panelOpen<T>(show: () => Promise<T>): Promise<T> {
  panels += 1;
  try {
    return await show();
  } finally {
    panels -= 1;
  }
}

/**
 * His files pick (window.bise.pickFiles, the composer's '+'): macOS's open panel for files, several
 * at once, attached to `win`, starting in his home; [] on cancel. Test mode: the env's
 * TEST_PICK_FILES_ENV paths at once, never a panel.
 */
export async function pickFiles(win: BrowserWindow | null, testMode: boolean): Promise<readonly string[]> {
  if (testMode) return pickedFilesInTest(process.env[TEST_PICK_FILES_ENV]);
  const opts: OpenDialogOptions = { properties: ['openFile', 'multiSelections'], defaultPath: homedir() };
  const r = await panelOpen(() => (win === null ? dialog.showOpenDialog(opts) : dialog.showOpenDialog(win, opts)));
  return r.canceled ? [] : r.filePaths;
}

/** Test mode: a clipboard item of the menu, played in the focused window's fence. */
export function sendClip(op: ClipOp): void {
  BrowserWindow.getFocusedWindow()?.webContents.send(IPC.clip, op);
}

/** A bise item of the menu, sent where menuTarget says. */
export function sendMenu(action: MenuAction, main: { readonly window: BrowserWindow | null; open: (route?: string) => void }): void {
  const focused = BrowserWindow.getFocusedWindow();
  const t = menuTarget(action, focused !== null && focused === main.window, focused !== null);
  if ('open' in t) main.open(t.open);
  else if ('close' in t) focused?.close();
  // nothing of bise's has the keys (his app does, or a test run): the main window's
  else (focused ?? main.window)?.webContents.send(IPC.menu, action);
}
