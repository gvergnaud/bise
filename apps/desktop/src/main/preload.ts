// The preload: builds `window.bise` (src/shared/api.ts) with contextBridge.
// Sandboxed, so it is bundled as CommonJS and may only use the sandbox's
// modules (electron's contextBridge and ipcRenderer). Owner: amb-mac.
// In test mode it also fences the clipboard (src/main/clipfence.ts): ⌘C ⌘X
// ⌘V and the menu's clipboard items play on a private buffer, never his.
import { contextBridge, ipcRenderer, webUtils, type IpcRendererEvent } from 'electron';
import type { HelperEv } from '../proto';
import { HOME_ARG, IPC, OFFSCREEN_ARG, SURFACE_ARG, TEST_ENV, type A11y, type AppSwapResult, type AppUpdate, type ArtifactLink, type BiseApi, type CrashReport, type CapsuleState, type FileFacts, type MenuAction, type NotifyAuth, type SaveResult, type SkillRead, type Surface, type ThumbAnswer, type TermOpened, type UpdateInfo, type UpdateState, type ViewAnswerWire } from '../shared/api';
import { argValue } from './args';
import { installClipFence, type ClipOp, type ClipSeed, type WinLike } from './clipfence';
import { coreBridge } from './core/bridge';

const test = process.env[TEST_ENV] === '1';
const surface: Surface = argValue(process.argv, SURFACE_ARG) === 'capsule' ? 'capsule' : 'window';
// his home folder from main (windows.ts), read once
const home = argValue(process.argv, HOME_ARG) ?? '';

// what main sends on each channel a page listens to
interface Pushed {
  [IPC.menu]: MenuAction;
  [IPC.helperEv]: HelperEv;
  [IPC.fnVoice]: null;
  [IPC.capsuleState]: CapsuleState;
  [IPC.notifyState]: NotifyAuth;
  [IPC.route]: string;
  [IPC.artifactLink]: ArtifactLink;
  [IPC.termData]: { id: string; bytes: Uint8Array };
  [IPC.termExit]: { id: string; code: number | null };
  [IPC.updateState]: UpdateState;
  [IPC.a11y]: A11y;
  [IPC.agentMenu]: string;
}

// window.bise.saveCopy's answer as main gives it: {to}, {error}, or null
function saved(v: unknown): SaveResult {
  if (typeof v !== 'object' || v === null) return null;
  const r = v as Record<string, unknown>;
  if (typeof r['to'] === 'string') return { to: r['to'] };
  return typeof r['error'] === 'string' ? { error: r['error'] } : null;
}

// one listener on a channel, its unsubscribe
function listen<K extends keyof Pushed>(channel: K, listener: (v: Pushed[K]) => void): () => void {
  const on = (_e: IpcRendererEvent, v: Pushed[K]): void => {
    listener(v);
  };
  ipcRenderer.on(channel, on);
  return () => {
    ipcRenderer.removeListener(channel, on);
  };
}
// the page's window (src/main has no DOM types: the fence names what it uses)
const fence = test ? installClipFence(globalThis as unknown as WinLike, process.platform === 'darwin') : null;
if (fence !== null) {
  ipcRenderer.on(IPC.clip, (_e: IpcRendererEvent, op: ClipOp) => {
    fence.run(op);
  });
  // a new seed while the app runs (main's TEST_CLIP_HOOK): an image for a real ⌘V
  ipcRenderer.on(IPC.clipSet, (_e: IpcRendererEvent, seed: ClipSeed, nonce: number) => {
    fence.seed(seed);
    ipcRenderer.send(IPC.clipSetDone, nonce);
  });
  // the env's seed (BISE_DESKTOP_TEST_CLIPBOARD[_FILE]): main reads the files
  void ipcRenderer.invoke(IPC.clipSeed).then((seed: ClipSeed) => {
    if (seed.text !== undefined || (seed.files ?? []).length > 0) fence.seed(seed);
  });
}

const api: BiseApi = {
  platform: process.platform,
  home,
  // only in an off-screen run: his window.bise has no 'offscreen' key at all
  ...(process.argv.includes(OFFSCREEN_ARG) ? { offscreen: true as const } : {}),
  test,
  update: {
    now: () => ipcRenderer.invoke(IPC.updateNow) as Promise<UpdateState>,
    onState: (listener) => listen(IPC.updateState, listener),
    install: async (route) => (await ipcRenderer.invoke(IPC.updateInstall, route)) === true,
    revert: async () => (await ipcRenderer.invoke(IPC.updateRevert)) === true,
    move: async () => (await ipcRenderer.invoke(IPC.updateMove)) === true,
    info: () => ipcRenderer.invoke(IPC.updateInfo) as Promise<UpdateInfo>,
    onAsk: (answer) => {
      const h = (_e: unknown, id: unknown): void => {
        ipcRenderer.send(IPC.updateAnswer, id, answer());
      };
      ipcRenderer.on(IPC.updateAsk, h);
      return () => {
        ipcRenderer.removeListener(IPC.updateAsk, h);
      };
    },
  },
  app: {
    update: () => ipcRenderer.invoke(IPC.appUpdate) as Promise<AppUpdate>,
    swap: (route) => ipcRenderer.invoke(IPC.appSwap, route) as Promise<AppSwapResult>,
    restartLog: () => ipcRenderer.invoke(IPC.appRestartLog) as Promise<CrashReport | null>,
  },
  openExternal: async (url) => (await ipcRenderer.invoke(IPC.openExternal, url)) === true,
  views: {
    call: (cmd) => ipcRenderer.invoke(IPC.viewsCall, cmd) as Promise<ViewAnswerWire>,
    frames: (list) => {
      ipcRenderer.send(IPC.viewsFrames, list);
    },
    gesture: async (ref) => (await ipcRenderer.invoke(IPC.viewsGesture, ref)) === true,
  },
  openPath: async (path, opts) => (await ipcRenderer.invoke(IPC.openPath, path, opts)) === true,
  quickLook: async (path) => (await ipcRenderer.invoke(IPC.quickLook, path)) === true,
  showInFinder: async (path) => (await ipcRenderer.invoke(IPC.showInFinder, path)) === true,
  probePage: async (url) => {
    const r: unknown = await ipcRenderer.invoke(IPC.probePage, url);
    return r === 'down' || r === 'gone' ? r : 'up';
  },
  readSkill: (path, file) => ipcRenderer.invoke(IPC.readSkill, path, file) as Promise<SkillRead>,
  saveCopy: async (path, name) => saved(await ipcRenderer.invoke(IPC.saveCopy, path, name)),
  fileIcon: async (path) => {
    const u: unknown = await ipcRenderer.invoke(IPC.fileIcon, path);
    return typeof u === 'string' ? u : null;
  },
  fileFacts: async (path) => {
    const f: unknown = await ipcRenderer.invoke(IPC.fileFacts, path);
    return typeof f === 'object' && f !== null && typeof (f as FileFacts).bytes === 'number' ? (f as FileFacts) : null;
  },
  thumb: async (req, want) => {
    const t: unknown = await ipcRenderer.invoke(IPC.thumb, req, want);
    return typeof t === 'object' && t !== null ? (t as ThumbAnswer) : null;
  },
  saveImage: async (bytes) => {
    const p: unknown = await ipcRenderer.invoke(IPC.saveImage, bytes);
    return typeof p === 'string' ? p : null;
  },
  filePath: (file) => {
    try {
      return webUtils.getPathForFile(file);
    } catch {
      return '';
    }
  },
  pickFolder: async () => {
    const p: unknown = await ipcRenderer.invoke(IPC.pickFolder);
    return typeof p === 'string' ? p : null;
  },
  pickFiles: async () => {
    const p: unknown = await ipcRenderer.invoke(IPC.pickFiles);
    return Array.isArray(p) ? p.filter((x): x is string => typeof x === 'string') : [];
  },
  copy: async (text) => {
    fence?.put(text);
    await ipcRenderer.invoke(IPC.copy, text);
  },
  onMenu: (listener) => listen(IPC.menu, listener),
  onHelper: (listener) => listen(IPC.helperEv, listener),
  onFnVoice: (listener) =>
    listen(IPC.fnVoice, () => {
      listener();
    }),
  helperState: async () => (await ipcRenderer.invoke(IPC.helperState)) as HelperEv[],
  helper: (cmd) => {
    ipcRenderer.send(IPC.helperCmd, cmd);
  },
  surface,
  // the renderer's keep on disk (amb-win, src/main/keep.ts)
  keep: {
    read: () => {
      const t: unknown = ipcRenderer.sendSync(IPC.keepRead, surface);
      return typeof t === 'string' ? t : null;
    },
    write: (text) => {
      ipcRenderer.send(IPC.keepWrite, surface, text);
    },
  },
  capsule: {
    show: (at, keys, fit) => {
      ipcRenderer.send(IPC.capsuleShow, at, keys, fit);
    },
    hide: () => {
      ipcRenderer.send(IPC.capsuleHide);
    },
    size: (width, height, glass) => {
      ipcRenderer.send(IPC.capsuleSize, width, height, glass);
    },
    onState: (listener) => listen(IPC.capsuleState, listener),
  },
  notify: {
    state: async () => (await ipcRenderer.invoke(IPC.notifyState)) as NotifyAuth,
    onState: (listener) => listen(IPC.notifyState, listener),
    ask: async () => (await ipcRenderer.invoke(IPC.notifyAsk)) as NotifyAuth,
    openSettings: () => {
      ipcRenderer.send(IPC.notifySettings);
    },
    listed: (on) => {
      ipcRenderer.send(IPC.notifyListed, on);
    },
  },
  usage: {
    // a signal and a closed name only, never what he typed or opened (main/usage)
    input: () => {
      ipcRenderer.send(IPC.usageInput);
    },
    gesture: (g) => {
      ipcRenderer.send(IPC.usageGesture, g);
    },
  },
  voice: {
    openMicSettings: () => {
      ipcRenderer.send(IPC.micSettings);
    },
    log: (line: string) => {
      ipcRenderer.send(IPC.appLog, line);
    },
  },
  quit: () => {
    ipcRenderer.send(IPC.quit);
  },
  openWindow: (to) => {
    ipcRenderer.send(IPC.openWindow, to);
  },
  barDoubleClick: () => {
    ipcRenderer.send(IPC.barDoubleClick);
  },
  popupMenu: async (items, at) => {
    const id: unknown = await ipcRenderer.invoke(IPC.popupMenu, items, at);
    return typeof id === 'string' ? id : null;
  },
  agentMenu: (items) => {
    ipcRenderer.send(IPC.agentMenu, items);
  },
  onAgentMenu: (listener) => listen(IPC.agentMenu, listener),
  viewMenu: (sides) => {
    ipcRenderer.send(IPC.viewMenu, sides);
  },
  a11y: async () => (await ipcRenderer.invoke(IPC.a11y)) as A11y,
  onA11y: (listener) => listen(IPC.a11y, listener),
  onRoute: (listener) => listen(IPC.route, listener),
  onArtifactLink: (listener) => listen(IPC.artifactLink, listener),
  core: coreBridge(ipcRenderer),
  term: {
    open: async (project, cols, rows, n) => (await ipcRenderer.invoke(IPC.termOpen, project, cols, rows, n ?? 0)) as TermOpened,
    write: (id, data) => {
      ipcRenderer.send(IPC.termWrite, id, data);
    },
    resize: (id, cols, rows) => {
      ipcRenderer.send(IPC.termResize, id, cols, rows);
    },
    kill: (id) => {
      ipcRenderer.send(IPC.termKill, id);
    },
    onData: (listener) =>
      listen(IPC.termData, (v) => {
        listener(v.id, v.bytes);
      }),
    onExit: (listener) =>
      listen(IPC.termExit, (v) => {
        listener(v.id, v.code);
      }),
  },
};

contextBridge.exposeInMainWorld('bise', api);
