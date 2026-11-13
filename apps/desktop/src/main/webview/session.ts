// The web tabs' session (architect m_64682 rule 3): persist:bise-web, under the app's sessionData
// (a stand-in home keeps it in the stand-in), never the app's own. Every permission denied (request,
// check, device, display media); every request to bise itself cancelled (guard.ts blockedRequest,
// with main's live ports). A download asks where through macOS's save panel, Downloads its first
// folder, then the folder he picked last (designer m_64684; never a silent write: rule 6); its end
// is a line in the tab (views.ts). Test mode: no panel, the download cancelled and a line on
// stdout. 'And cookies' clears this session only. Owner: newtab-omnibox.
import { basename, dirname, join } from 'node:path';
import { app, session, type DownloadItem, type Session, type WebContents } from 'electron';
import { blockedRequest } from './guard';
import { WEB_PARTITION } from './input';

/** Test mode: a download a web tab started, on stdout instead of a save panel. */
export const DOWNLOAD_LINE = 'BISE_TEST_DOWNLOAD';

/** A download that ended: saved where he picked, or nothing (cancelled, failed). */
export type Saved = { readonly name: string; readonly path: string; readonly folder: string } | null;

let made: Session | null = null;
let lastFolder: string | null = null;

/** The web session, made once: its handlers set. `ports`: bise's loopback ports now; `saved`: a download's end, for its tab. */
export function webSession(o: { readonly ports: () => ReadonlySet<number>; readonly testMode: boolean; readonly saved: (wc: WebContents, s: Saved) => void }): Session {
  if (made !== null) return made;
  const s = session.fromPartition(WEB_PARTITION);
  s.setPermissionRequestHandler((_wc, _p, done) => {
    done(false);
  });
  s.setPermissionCheckHandler(() => false);
  s.setDevicePermissionHandler(() => false);
  s.setDisplayMediaRequestHandler((_req, done) => {
    done({});
  });
  s.webRequest.onBeforeRequest((d, done) => {
    done({ cancel: blockedRequest(d.url, o.ports()) });
  });
  s.on('will-download', (_e, item, wc) => {
    download(item, wc, o);
  });
  made = s;
  return s;
}

function download(item: DownloadItem, wc: WebContents, o: { readonly testMode: boolean; readonly saved: (wc: WebContents, s: Saved) => void }): void {
  if (o.testMode) {
    process.stdout.write(`${DOWNLOAD_LINE} ${item.getFilename()}\n`);
    item.cancel();
    return;
  }
  // his pick in the save panel is his act (rule 6): no setSavePath, Electron shows the panel
  item.setSaveDialogOptions({ defaultPath: join(lastFolder ?? app.getPath('downloads'), item.getFilename()) });
  item.once('done', (_e, state) => {
    const path = item.getSavePath();
    if (state !== 'completed' || path === '') {
      o.saved(wc, null);
      return;
    }
    lastFolder = dirname(path);
    o.saved(wc, { name: basename(path), path, folder: basename(dirname(path)) });
  });
}

/** Settings › Privacy's 'and cookies': this session's cookies and storage, never the app's. */
export async function clearWebSession(): Promise<void> {
  const s = session.fromPartition(WEB_PARTITION);
  await s.clearStorageData();
  await s.clearCache();
}
