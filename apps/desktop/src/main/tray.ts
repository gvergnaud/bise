// The menu bar's ':*' (Electron's Tray; designer's bise-presence v10): what it shows comes from
// traylook.ts (pure), drawn here from the shared geometry (src/shared/mark.ts) as raw bitmaps at
// 1x and 2x, no file. index.ts feeds it the counts (counts.ts) on each change; a new wait plays
// the two kisses once, work steps the star every 400 ms; under reduced motion neither. A click
// opens the capsule's dropdown under it (the page draws it: window.bise.capsule.onState says at
// 'menu', why 'tray'); a right click is the native fallback menu, which works with no page at
// all: open bise ⌘0, settings ⌘,, quit ⌘Q. Never in a test run (a tray icon would sit in his
// menu bar).
import { app, Menu, nativeImage, systemPreferences, Tray, type MenuItemConstructorOptions, type NativeImage } from 'electron';
import type { Capsule } from './capsule';
import type { Counts } from './counts';
import { BEAT_FRAME_MS, KISSES, newWait, STEP_MS, STEPS, trayFrame, trayLook, trayTip, type TrayLook } from './traylook';
import type { MainWindow } from './windows';

export function fallbackMenu(open: (route?: string) => void): MenuItemConstructorOptions[] {
  return [
    { label: 'open bise', accelerator: 'CmdOrCtrl+0', click: () => { open(); } },
    { label: 'settings…', accelerator: 'CmdOrCtrl+,', click: () => { open('settings'); } },
    { type: 'separator' },
    { label: 'quit bise', accelerator: 'CmdOrCtrl+Q', click: () => { app.quit(); } },
  ];
}

function imageOf(look: TrayLook, o: { readonly step?: number; readonly beat?: number }): NativeImage {
  const img = nativeImage.createEmpty();
  for (const scale of [1, 2]) {
    const f = trayFrame(look, scale, o);
    img.addRepresentation({ scaleFactor: scale, width: f.px, height: f.px, buffer: Buffer.from(f.bgra) });
  }
  // black + alpha: macOS tints it for the bar; a wait is a colour image, the only way to show colour
  img.setTemplateImage(look !== 'wait');
  return img;
}

const reduced = (): boolean => process.platform === 'darwin' && systemPreferences.getAnimationSettings().prefersReducedMotion;

/** The tray and its look; `show` takes the counts on each change. */
export class BiseTray {
  readonly tray: Tray;
  private counts: Counts = { waiting: 0, working: 0 };
  private timer: NodeJS.Timeout | null = null;

  constructor(capsule: Capsule, main: MainWindow) {
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

  private draw(kiss: boolean): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
    const look = trayLook(this.counts);
    this.tray.setImage(imageOf(look, {}));
    if (reduced()) return;
    if (look === 'wait' && kiss) {
      let i = 0;
      this.timer = setInterval(() => {
        this.tray.setImage(imageOf(look, { beat: KISSES[i] ?? 1 }));
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
