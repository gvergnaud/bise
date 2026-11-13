// The web history's one writer (webhistory.ts is its pure part; architect m_64583): read once
// from userData/web-history.json at start (a broken file moved aside with one app.log line, then
// empty), a visit noted by openLink (ipc.ts), the window's four calls (list, remove one, clear, his
// switch), each change written whole (keep.ts writeAtomic: tmp, fsync, rename, 0600) and pushed
// whole to every window. Main checks every call: an http(s) URL under URL_MAX, a boolean switch,
// nothing else taken. A test copy of the app has its own userData (index.ts), so it never reads
// his history. Owner: newtab-omnibox.
import { renameSync } from 'node:fs';
import { BrowserWindow, ipcMain } from 'electron';
import { IPC, type WebHistory } from '../shared/api';
import { readKeep, writeAtomic } from './keep';
import { cleared, EMPTY_HISTORY, webHistoryFile, historyText, noteVisit, parseHistory, pruned, recordable, withKeep, withoutUrl } from './webhistory';

let file: string | null = null;
let h: WebHistory = EMPTY_HISTORY;

function changed(next: WebHistory): void {
  if (next === h) return;
  h = next;
  if (file !== null) writeAtomic(file, historyText(h));
  for (const w of BrowserWindow.getAllWindows()) {
    if (!w.isDestroyed()) w.webContents.send(IPC.webHistoryChanged, h);
  }
}

/** A link openLink let out: noted when it is recordable (webhistory.ts recordable) and his switch is on. */
export function noteLink(url: string, typed: boolean): void {
  if (file === null) return;
  const u = recordable(url);
  if (u !== null) changed(noteVisit(h, u, Date.now(), typed));
}

/** Reads `dir`'s history and answers the window's calls; `log` gets one line when the file was broken. */
export function installWebHistory(dir: string, log: (line: string) => void): void {
  file = webHistoryFile(dir);
  const read = parseHistory(readKeep(file));
  if (read.broken) {
    const aside = `${file}.broken-${String(Date.now())}`;
    try {
      renameSync(file, aside);
    } catch {
      // gone meanwhile: nothing to keep aside
    }
    log(`web history: ${file} was unreadable, moved to ${aside}; starting empty`);
  }
  h = pruned(read.h, Date.now());
  ipcMain.handle(IPC.webHistoryList, () => {
    changed(pruned(h, Date.now()));
    return h;
  });
  ipcMain.handle(IPC.webHistoryRemove, (_e, raw: unknown) => {
    const u = recordable(raw);
    if (u !== null) changed(withoutUrl(h, u));
  });
  ipcMain.handle(IPC.webHistoryClear, () => {
    changed(cleared(h));
  });
  ipcMain.handle(IPC.webHistoryKeep, (_e, on: unknown) => {
    if (typeof on === 'boolean') changed(withKeep(h, on));
  });
}
