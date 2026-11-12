// window.bise.thumb's main side (art-thumbs, artifacts-in-folders brief part 2): an artifact
// card's picture, made once per version and kept in the cache (cache.ts).
//   a file    Quick Look's thumbnail, asked of the native helper (helper/look.ts: its
//             QLThumbnailGenerator, a thumbnail only, never the type's icon; docs/plugin-apps.md
//             A6b, architect m_60495), with its length when it has one (a video's, a sound's).
//             Nothing here knows a file type: Quick Look says which files it draws; one it
//             can't, or one over 50 MB, answers its ending and size. A folder its file count.
//   a page    a capture of its first screen in a hidden offscreen window (local urls only;
//             sandboxed, no preload, its own partition with every permission denied, no
//             navigation, window or download, destroyed at 5 s: architect m_39772). Its fence
//             is its own: the app's guard (guard.ts) skips it (isCaptureContents), so nothing
//             a captured page does ever reaches his browser.
// The cache's folder is the core's (cache.ts thumbsDir); before the core said it, nothing is
// made. After each picture made, the cache is pruned past its cap (prune.ts).
// One picture made at a time (a capture or a Quick Look each holds a process busy), a cached one
// answers at once; the window asks for the cards in view first (artifacts/thumbs/queue.ts). Test
// mode and off-screen runs have no helper: Electron decodes what it can (an image), anything else
// answers its facts. Owner: art-thumbs.
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { BrowserWindow, nativeImage, session, type NativeImage } from 'electron';
import type { ThumbAnswer, ThumbReq } from '../../shared/api';
import type { Look } from '../helper/look';
import { CAPTURE, cachedOf, fileFor, fitWidth, MAX_LOOK_BYTES, projectDir, readReq, setThumbsDir, stemOf, THUMB_W } from './cache';
import { prune } from './prune';

/** What making a picture needs from the app (index.ts gives it; tests give fakes). */
export interface ThumbEnv {
  /** the cache's root (cache.ts thumbsDir: the core's), null before the core said it */
  readonly root: () => string | null;
  /** the file fence (reveal.ts realLookable): the realpath when he may read it, else null */
  readonly lookable: (raw: unknown) => string | null;
  /** Quick Look's thumbnail of a file at `out` (helper/look.ts), null while no helper runs (a test, an off-screen run) */
  readonly look: () => ((path: string, out: string, px: number) => Promise<Look | null>) | null;
}

const FOLDER_FILES_MAX = 5000;
/** A Quick Look or a capture that takes longer is stopped (its process killed, its window destroyed). */
const MAKE_TIMEOUT_MS = 5000;
const PARTITION = 'bise-thumbs';

/** The file's ending, the card's word for a file with no picture ('' none). */
const endingOf = (p: string): string => {
  const name = p.split('/').pop() ?? '';
  const dot = name.lastIndexOf('.');
  return dot <= 0 ? '' : name.slice(dot + 1).toLowerCase();
};

const isLocalUrl = (u: string): boolean => /^https?:\/\/(127\.0\.0\.1|localhost|0\.0\.0\.0)(:\d+)?(\/|$)/i.test(u);

/** The files under a folder (.git and node_modules skipped), at most FOLDER_FILES_MAX. */
function countFiles(dir: string): number {
  let n = 0;
  const walk = (d: string): void => {
    for (const e of readdirSync(d, { withFileTypes: true })) {
      if (n >= FOLDER_FILES_MAX) return;
      if (e.isDirectory()) {
        if (e.name !== '.git' && e.name !== 'node_modules') walk(join(d, e.name));
      } else n++;
    }
  };
  walk(dir);
  return n;
}

/** A picture cut to 480 px wide, as PNG bytes; null when empty. */
function png(img: NativeImage): Buffer | null {
  if (img.isEmpty()) return null;
  const { width, height } = img.getSize();
  const fit = fitWidth(width, height);
  return (fit.width === width ? img : img.resize({ ...fit, quality: 'good' })).toPNG();
}

/** The cached picture of this version, as a data URL, or null (none yet). */
function cached(file: string): string | null {
  try {
    return `data:image/png;base64,${readFileSync(file).toString('base64')}`;
  } catch {
    return null;
  }
}

/** Keeps the picture (tmp + rename), its length in its name, and drops this version's older sigs. */
function keep(root: string, r: ThumbReq, bytes: Buffer, ms?: number): void {
  const file = fileFor(root, r, ms);
  mkdirSync(dirname(file), { recursive: true });
  const tmp = `${file}.${String(process.pid)}.tmp`;
  writeFileSync(tmp, bytes);
  renameSync(tmp, file);
  prune(root);
  const stem = stemOf(r);
  for (const f of readdirSync(projectDir(root, r))) {
    if (f.startsWith(stem) && join(projectDir(root, r), f) !== file && cachedOf([f], { ...r, sig: f.slice(stem.length, stem.length + 8) }) !== null) {
      try {
        unlinkSync(join(projectDir(root, r), f));
      } catch {
        // another app's sweep took it first
      }
    }
  }
}

/**
 * A capture window's contents (their session is the capture partition): guard.ts leaves them to
 * capture's own fence. The app's guard reads a main frame leaving its page as his link click and
 * sends it to his browser: on a capture, a local site's 302 (the pages server's /a/<id>/live door)
 * opened that site in his browser at each card drawn, a burst of tabs per project (art-links-burst).
 */
export function isCaptureContents(wc: { readonly session: Electron.Session }): boolean {
  return wc.session === session.fromPartition(PARTITION);
}

/** The capture windows' session: no permission, no download, nothing kept (set once). */
let fenced = false;
function captureSession(): Electron.Session {
  const ses = session.fromPartition(PARTITION);
  if (!fenced) {
    fenced = true;
    ses.setPermissionRequestHandler((_wc, _perm, done) => {
      done(false);
    });
    ses.setPermissionCheckHandler(() => false);
    ses.on('will-download', (e, item) => {
      e.preventDefault();
      item.cancel();
    });
  }
  return ses;
}

/** A local page's first screen, in a hidden offscreen window of its own, destroyed at 5 s at most. */
async function capture(url: string): Promise<NativeImage | null> {
  const win = new BrowserWindow({
    show: false,
    ...CAPTURE,
    webPreferences: { offscreen: true, sandbox: true, contextIsolation: true, nodeIntegration: false, session: captureSession() },
  });
  const wc = win.webContents;
  wc.setAudioMuted(true);
  wc.setWindowOpenHandler(() => ({ action: 'deny' }));
  wc.on('will-navigate', (e) => {
    e.preventDefault();
  });
  wc.on('will-redirect', (e) => {
    if (!isLocalUrl(e.url)) e.preventDefault();
  });
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<null>((res) => {
    timer = setTimeout(() => {
      res(null);
    }, MAKE_TIMEOUT_MS);
  });
  const shot = (async (): Promise<NativeImage | null> => {
    await win.loadURL(url);
    // its fonts and first paint
    await new Promise((res) => setTimeout(res, 400));
    return wc.capturePage();
  })().catch(() => null);
  try {
    return await Promise.race([shot, late]);
  } finally {
    clearTimeout(timer);
    win.destroy();
  }
}

/** This version's cached picture: its file and length, or null. */
function cachedFile(root: string, r: ThumbReq): { readonly file: string; readonly ms?: number } | null {
  let names: string[];
  try {
    names = readdirSync(projectDir(root, r));
  } catch {
    return null;
  }
  const c = cachedOf(names, r);
  if (c === null) return null;
  const file = join(projectDir(root, r), c.name);
  return c.ms === undefined ? { file } : { file, ms: c.ms };
}

/** Quick Look's picture (the helper's, its PNG under the cache), else what Electron decodes itself. */
async function picture(env: ThumbEnv, root: string, p: string): Promise<{ img: NativeImage | null; ms?: number }> {
  const look = env.look();
  if (look === null) return { img: nativeImage.createFromPath(p) };
  mkdirSync(root, { recursive: true });
  const out = join(root, `.ql-${String(process.pid)}-${String(Date.now())}.png`);
  try {
    const l = await look(p, out, THUMB_W);
    const img = l?.ok === true && existsSync(out) ? nativeImage.createFromPath(out) : null;
    return l?.ms === undefined ? { img } : { img, ms: l.ms };
  } finally {
    rmSync(out, { force: true });
  }
}

const secsOf = (ms: number | undefined): { secs?: number } => (ms === undefined ? {} : { secs: ms / 1000 });

/** A file's answer, the cache asked first for its picture. */
async function fileAnswer(env: ThumbEnv, r: ThumbReq, want: boolean): Promise<ThumbAnswer | null> {
  const p = env.lookable(r.src);
  if (p === null) return null;
  const st = statSync(p);
  if (st.isDirectory()) return want ? { files: countFiles(p) } : null;
  const none: ThumbAnswer = { none: { ext: endingOf(p), bytes: st.size } };
  const root = env.root();
  if (st.size > MAX_LOOK_BYTES || root === null) return want ? none : null;
  const hit = cachedFile(root, r);
  if (hit !== null) {
    if (!want) return null;
    const img = cached(hit.file);
    return img === null ? none : { img, ...secsOf(hit.ms) };
  }
  const made = await serial(() => picture(env, root, p));
  const bytes = made.img === null ? null : png(made.img);
  if (bytes === null) return want ? { ...none, ...secsOf(made.ms) } : null;
  keep(root, r, bytes, made.ms);
  return want ? { img: `data:image/png;base64,${bytes.toString('base64')}`, ...secsOf(made.ms) } : null;
}

/** A page's answer: its cached capture, else a new one. */
async function pageAnswer(env: ThumbEnv, r: ThumbReq, want: boolean): Promise<ThumbAnswer | null> {
  const root = env.root();
  if (!isLocalUrl(r.src) || root === null) return null;
  const hit = cachedFile(root, r);
  if (hit !== null) {
    const img = want ? cached(hit.file) : null;
    return img === null ? null : { img };
  }
  const shot = await serial(() => capture(r.src));
  const bytes = shot === null ? null : png(shot);
  if (bytes === null) return null;
  keep(root, r, bytes);
  return want ? { img: `data:image/png;base64,${bytes.toString('base64')}` } : null;
}

// one picture made at a time: each holds Quick Look's or a renderer's process; a cached one or a
// folder answers at once
let line: Promise<unknown> = Promise.resolve();

/** `make` after every picture asked before it is made. */
function serial<T>(make: () => Promise<T>): Promise<T> {
  const next = line.then(make, make);
  line = next.catch(() => undefined);
  return next;
}

let pruned = false;

/** The core's start (core/ipc.ts onInit): its cache folder kept, the cache pruned once per app start. */
export function startThumbs(dir: string): void {
  setThumbsDir(dir);
  if (dir === '' || pruned) return;
  pruned = true;
  prune(dir);
}

/** window.bise.thumb: the request checked, then its answer (null: nothing to draw, or refused). */
export async function thumb(env: ThumbEnv, raw: unknown, want: unknown): Promise<ThumbAnswer | null> {
  const r = readReq(raw);
  if (r === null) return null;
  const w = want === true;
  try {
    return await (r.how === 'page' ? pageAnswer(env, r, w) : fileAnswer(env, r, w));
  } catch {
    return null;
  }
}
