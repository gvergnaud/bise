// The menu bar's ':*' (Electron's Tray; designer's bise-presence v10): what it shows comes from
// traylook.ts (pure), drawn here from the shared geometry (src/shared/mark.ts) as raw bitmaps at
// 1x and 2x, no file, always a template image. index.ts feeds it the counts (counts.ts) on each
// change; a wait cuts the dot's hole and lays the dot over it with the tray-badge addon
// (traybadge.ts; appAddon loads it once for the tray and the capsule), again after each setImage
// (Electron may rebuild the button), or shows the dot
// as the title when the addon can't; a new wait plays the two kisses once, work steps the star
// every 400 ms; under reduced motion neither. A click
// opens the capsule's dropdown under it (the page draws it: window.bise.capsule.onState says at
// 'menu', why 'tray'); a right click is the native fallback menu, which works with no page at
// all: open bise ⌘0, settings ⌘,, quit ⌘Q. Never in a test run (a tray icon would sit in his
// menu bar).
import { app, Menu, nativeImage, systemPreferences, Tray, type MenuItemConstructorOptions, type NativeImage } from 'electron';
import type { Capsule } from './capsule';
import type { Counts } from './counts';
import { badgePath, loadBadge, stillWindow, TrayBadge, type Addon } from './traybadge';
import { BEAT_FRAME_MS, KISSES, newWait, STEP_MS, STEPS, trayFrame, trayLook, trayPaint, trayTip, type TrayLook } from './traylook';
import type { MainWindow } from './windows';

export function fallbackMenu(open: (route?: string) => void): MenuItemConstructorOptions[] {
  return [
    { label: 'open bise', accelerator: 'CmdOrCtrl+0', click: () => { open(); } },
    { label: 'settings…', accelerator: 'CmdOrCtrl+,', click: () => { open('settings'); } },
    { type: 'separator' },
    { label: 'quit bise', accelerator: 'CmdOrCtrl+Q', click: () => { app.quit(); } },
  ];
}

function imageOf(look: TrayLook, o: { readonly step?: number; readonly beat?: number; readonly hole?: boolean }): NativeImage {
  const img = nativeImage.createEmpty();
  for (const scale of [1, 2]) {
    const f = trayFrame(look, scale, o);
    img.addRepresentation({ scaleFactor: scale, width: f.px, height: f.px, buffer: Buffer.from(f.bgra) });
  }
  // black + alpha: macOS tints it for the bar (the wait's dot is the addon's layer, over it)
  img.setTemplateImage(true);
  return img;
}

const log = (why: string): void => { console.log(why); };

/** The in-process AppKit addon (native/mac/badge/TrayBadge.m), loaded once by index.ts: the tray's dot and the capsule's still window. */
export function appAddon(): Addon | null {
  const path = badgePath({ platform: process.platform, env: process.env, packaged: app.isPackaged, resources: process.resourcesPath });
  const load = (p: string): unknown => {
    const m = { exports: {} as unknown };
    process.dlopen(m, p);
    return m.exports;
  };
  return loadBadge(path, load, log);
}

/** The capsule's window shows and hides at once (traybadge.ts stillWindow), logging why not. */
export const stillWith = (addon: Addon | null) => (handle: Buffer): boolean => stillWindow(addon, handle, log);

const reduced = (): boolean => process.platform === 'darwin' && systemPreferences.getAnimationSettings().prefersReducedMotion;

/** The tray and its look; `show` takes the counts on each change. */
export class BiseTray {
  readonly tray: Tray;
  private counts: Counts = { waiting: 0, working: 0 };
  private timer: NodeJS.Timeout | null = null;
  private readonly badge: TrayBadge;

  constructor(capsule: Capsule, main: MainWindow, addon: Addon | null) {
    this.badge = new TrayBadge(addon, log);
    this.tray = new Tray(imageOf('rest', {}));
    this.tray.setToolTip(trayTip(this.counts));
    const menu = Menu.buildFromTemplate(fallbackMenu((route) => { main.open(route); }));
    this.tray.on('click', () => {
      capsule.toggleMenu();
    });
    this.tray.on('right-click', () => {
      this.tray.popUpContextMenu(menu);
    });
    capsule.tray = this.tray;
  }

  show(c: Counts): void {
    const kiss = newWait(this.counts, c);
    this.counts = c;
    this.tray.setToolTip(trayTip(c));
    this.draw(kiss);
  }

  /** One image, then the dot over it (the addon) or as the title (its fallback). */
  private paint(look: TrayLook, o: { readonly step?: number; readonly beat?: number }): void {
    const want = trayPaint(look, this.badge.loaded);
    this.tray.setImage(imageOf(look, { ...o, hole: want.hole }));
    if (want.badge === null) {
      this.badge.set(null);
      this.title(want.title);
      return;
    }
    if (this.badge.set(want.badge)) {
      this.title('');
      return;
    }
    // the addon found no item: the plain icon and the title's dot
    const fallback = trayPaint(look, false);
    this.tray.setImage(imageOf(look, o));
    this.title(fallback.title);
  }

  private title(t: string): void {
    if (this.tray.getTitle() !== t) this.tray.setTitle(t);
  }

  private draw(kiss: boolean): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
    const look = trayLook(this.counts);
    this.paint(look, {});
    if (reduced()) return;
    if (look === 'wait' && kiss) {
      let i = 0;
      this.timer = setInterval(() => {
        this.paint(look, { beat: KISSES[i] ?? 1 });
        i += 1;
        if (i >= KISSES.length && this.timer !== null) {
          clearInterval(this.timer);
          this.timer = null;
        }
      }, BEAT_FRAME_MS);
    } else if (look === 'work') {
      const frames = Array.from({ length: STEPS }, (_, step) => imageOf(look, { step }));
      let step = 0;
      this.timer = setInterval(() => {
        step = (step + 1) % STEPS;
        const f = frames[step];
        if (f !== undefined) this.tray.setImage(f);
      }, STEP_MS);
    }
  }
}
