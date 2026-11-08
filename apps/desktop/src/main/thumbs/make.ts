// window.bise.thumb's main side (art-thumbs, artifacts-in-folders brief part 2): an artifact
// card's picture, made once per version and kept in the cache (cache.ts).
//   a file    Quick Look (qlmanage -t, 0.33-0.75 s a file, killed at 5 s) for the kinds it
//             draws (LOOKS); a md, txt or code file answers its first 4 KB (the card
//             draws the words); over 50 MB, or a kind Quick Look only has an icon for, its
//             extension and size; a folder its file count; a video or a sound
//             also its length (media.ts).
//   a page    a capture of its first screen in a hidden offscreen window (local urls only;
//             sandboxed, no preload, its own partition with every permission denied, no
//             navigation, window or download, destroyed at 5 s: architect m_39772). Its fence
//             is its own: the app's guard (guard.ts) skips it (isCaptureContents), so nothing
//             a captured page does ever reaches his browser.
// The cache's folder is the core's (cache.ts thumbsDir); before the core said it, nothing is
// made. After each picture made, the cache is pruned past its cap (prune.ts).
// One picture made at a time (a capture or a Quick Look each holds a process busy), a cached one
// answers at once; the window asks for the cards in view first (artifacts/thumbs/queue.ts). Test mode and off-screen runs
// never call Quick Look (no TCC, amb-mac m_21862): an image is decoded here, anything else
// answers its facts. Owner: art-thumbs.
import { execFile } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, mkdtempSync, openSync, readdirSync, readFileSync, readSync, renameSync, rmSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { BrowserWindow, nativeImage, session, type NativeImage } from 'electron';
import type { ThumbAnswer, ThumbReq } from '../../shared/api';
import { CAPTURE, fileFor, fitWidth, MAX_LOOK_BYTES, projectDir, readReq, setThumbsDir, stemOf, TEXT_BYTES, THUMB_W } from './cache';
import { mediaSecs } from './media';
import { prune } from './prune';

/** What making a picture needs from the app (index.ts gives it; tests give fakes). */
export interface ThumbEnv {
  /** the cache's root (cache.ts thumbsDir: the core's), null before the core said it */
  readonly root: () => string | null;
  /** the file fence (reveal.ts realLookable): the realpath when he may read it, else null */
  readonly lookable: (raw: unknown) => string | null;
  /** Quick Look may run (his app, not a test or an off-screen run) */
  readonly quickLook: boolean;
}

/** Drawn from their text: md and txt (the brief), and code, whose Quick Look is as dark and tiny. */
const TEXT = new Set(['md', 'markdown', 'txt', 'log', 'rs', 'py', 'ts', 'tsx', 'js', 'jsx', 'mjs', 'sh', 'go', 'rb', 'swift', 'c', 'h', 'cpp', 'java', 'kt', 'bend', 'sql', 'toml', 'yaml', 'yml', 'json', 'css', 'lua', 'zig']);
/**
 * What Quick Look draws a real picture of. For anything else it answers the system's generic
 * document icon (measured on a .glb), so those never ask it: their tile says the extension and size.
 */
const LOOKS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'heic', 'heif', 'avif', 'tif', 'tiff', 'bmp', 'svg', 'psd', 'pdf', 'doc', 'docx', 'rtf', 'odt', 'pages', 'xls', 'xlsx', 'csv', 'tsv', 'numbers', 'ods', 'ppt', 'pptx', 'key', 'odp', 'html', 'htm', 'mp4', 'm4v', 'mov', 'usdz', 'usda', 'reality', 'epub']);
const IMAGE = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp']);
const TIMED = new Set(['mp4', 'm4v', 'mov', 'm4a', '3gp', 'aac', 'wav', 'mp3', 'webm', 'mkv']);
const FOLDER_FILES_MAX = 5000;
/** A Quick Look or a capture that takes longer is stopped (its process killed, its window destroyed). */
const MAKE_TIMEOUT_MS = 5000;
const PARTITION = 'bise-thumbs';

const extOf = (p: string): string => {
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

/** A video's or a sound's length from its header, or undefined. */
function secsOf(p: string, ext: string, size: number): number | undefined {
  if (!TIMED.has(ext)) return undefined;
  const fd = openSync(p, 'r');
  try {
    const read = (at: number, len: number): Uint8Array => {
      const b = Buffer.alloc(Math.max(0, Math.min(len, size - at)));
      const got = b.length === 0 ? 0 : readSync(fd, b, 0, b.length, at);
      return b.subarray(0, got);
    };
    return mediaSecs(ext, read, size) ?? undefined;
  } finally {
    closeSync(fd);
  }
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

/** Keeps the picture (tmp + rename) and drops this version's older sigs. */
function keep(root: string, r: ThumbReq, bytes: Buffer): void {
  const file = fileFor(root, r);
  mkdirSync(dirname(file), { recursive: true });
  const tmp = `${file}.${String(process.pid)}.tmp`;
  writeFileSync(tmp, bytes);
  renameSync(tmp, file);
  prune(root);
  const stem = stemOf(r);
  for (const f of readdirSync(projectDir(root, r))) {
    if (f.startsWith(stem) && f !== `${stem}${r.sig}.png` && f.endsWith('.png')) {
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

/** Quick Look's thumbnail of a file (qlmanage -t, killed at 5 s), in a folder of its own under the cache. */
async function quickLook(root: string, p: string): Promise<NativeImage | null> {
  mkdirSync(root, { recursive: true });
  const out = mkdtempSync(join(root, '.ql-'));
  try {
    // true when qlmanage ended well (a timeout kills it: false)
    const ok = await new Promise<boolean>((res) => {
      execFile('/usr/bin/qlmanage', ['-t', '-s', String(THUMB_W), '-o', out, p], { timeout: MAKE_TIMEOUT_MS, killSignal: 'SIGKILL' }, (e) => {
        res(e === null);
      });
    });
    const made = join(out, `${basename(p)}.png`);
    return ok && existsSync(made) ? nativeImage.createFromPath(made) : null;
  } catch {
    return null;
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}

/** A file's answer, the cache asked first for its picture. */
async function fileAnswer(env: ThumbEnv, r: ThumbReq, want: boolean): Promise<ThumbAnswer | null> {
  const p = env.lookable(r.src);
  if (p === null) return null;
  const st = statSync(p);
  if (st.isDirectory()) return want ? { files: countFiles(p) } : null;
  const ext = extOf(p);
  if (TEXT.has(ext)) {
    if (!want) return null;
    const fd = openSync(p, 'r');
    try {
      const b = Buffer.alloc(Math.min(TEXT_BYTES, st.size));
      const n = b.length === 0 ? 0 : readSync(fd, b, 0, b.length, 0);
      return { text: b.subarray(0, n).toString('utf8').replace(/\uFFFD+$/u, '') };
    } finally {
      closeSync(fd);
    }
  }
  const secs = secsOf(p, ext, st.size);
  const timed = secs === undefined ? {} : { secs };
  const none: ThumbAnswer = { none: { ext, bytes: st.size }, ...timed };
  if (st.size > MAX_LOOK_BYTES || !LOOKS.has(ext)) return want ? none : null;
  const root = env.root();
  if (root === null) return want ? none : null;
  const file = fileFor(root, r);
  if (existsSync(file)) {
    if (!want) return null;
    const img = cached(file);
    return img === null ? none : { img, ...timed };
  }
  let img: NativeImage | null = null;
  if (env.quickLook) img = await serial(() => quickLook(root, p));
  else if (IMAGE.has(ext)) img = nativeImage.createFromPath(p);
  const bytes = img === null ? null : png(img);
  if (bytes === null) return want ? none : null;
  keep(root, r, bytes);
  return want ? { img: `data:image/png;base64,${bytes.toString('base64')}`, ...timed } : null;
}

/** A page's answer: its cached capture, else a new one. */
async function pageAnswer(env: ThumbEnv, r: ThumbReq, want: boolean): Promise<ThumbAnswer | null> {
  const root = env.root();
  if (!isLocalUrl(r.src) || root === null) return null;
  const file = fileFor(root, r);
  if (existsSync(file)) {
    const img = want ? cached(file) : null;
    return img === null ? null : { img };
  }
  const shot = await serial(() => capture(r.src));
  const bytes = shot === null ? null : png(shot);
  if (bytes === null) return null;
  keep(root, r, bytes);
  return want ? { img: `data:image/png;base64,${bytes.toString('base64')}` } : null;
}

// one picture made at a time: each holds Quick Look's or a renderer's process; a cached one, a
// text or a folder answers at once
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
