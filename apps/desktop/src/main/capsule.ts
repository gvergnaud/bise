// The capsule: one panel window over every app, full-screen ones included,
// that never activates bise (the reset's settle-early question, lead m_9434).
// On macOS it is a 'panel' BrowserWindow (Electron adds
// NSWindowStyleMaskNonactivatingPanel): it can take the keys for the
// spotlight without bringing bise in front of his app, and floats on every
// Space and over full-screen apps (setVisibleOnAllWorkspaces with
// visibleOnFullScreen, the screen-saver level). A card shows with
// showInactive (never the keys); the spotlight with show (the keys, still not
// activating the app); it hides when it loses the keys. Its page is the same
// renderer, surface 'capsule' (window.bise.capsule drives it). It shows and
// hides with no AppKit animation (the addon's stillWindow, TrayBadge.m): macOS
// otherwise pops every show open over ~170 ms. Whether macOS
// really keeps his app active is qa-native's VM check: it can't be proven on
// his screen.
//
// Test mode: offscreen, never shown, on a display far from his; show/hide
// only change its state and print CAPSULE_LINE on stdout.
import { BrowserWindow, ipcMain, screen, type Tray } from 'electron';
import { CAPSULE_LINE, IPC, type CapsuleAt, type CapsuleFit, type CapsuleState } from '../shared/api';
import { a11yNow } from './a11y';
import { appendCapsuleFrame, capsuleFrame } from './capsuleframe';
import { glassOf } from './glass';
import { capsuleBounds, fitBounds, OFF_DISPLAY, onScreen, pinnedBounds, pinOf, type Anchor, type DisplayBox, type Pin, type Rect } from './place';
import { loadRenderer, OFF, webPreferences } from './windows';

const START = { width: 640, height: 120 };
/** a corner card's way out: the window's alpha 1 → 0, paired with the page's
 *  CORNER_OUT_MS fade (capsule/machine.ts), since the native glass doesn't fade with the page;
 *  the tray menu's too, alone (no scale: designer m_64430) */
const CORNER_FADE_MS = 120;
const FADE_STEPS = 8;

export class Capsule {
  private readonly win: BrowserWindow;
  private size: { width: number; height: number } = START;
  private glass: Rect | undefined;
  /** where the glass was put at show: it stays there while shown (place.ts) */
  private pin: Pin | null = null;
  /** the work area of the screen it was pinned on: a wider content stays inside it */
  private pinArea: Rect | null = null;
  /** this show grows down from a pinned top-left (the voice view) */
  private fit: CapsuleFit | null = null;
  private state: CapsuleState = { shown: false, at: 'bottom', keys: false, why: 'asked' };
  /** a corner hide's alpha fade, still running (a show cancels it) */
  private fading: ReturnType<typeof setInterval> | null = null;
  tray: Tray | null = null;

  /** off-screen: where this show would sit on his display (capsuleframe.ts), pinned like the real one */
  private would: { pin: Pin; d: DisplayBox } | null = null;

  /**
   * `offscreen`: BISE_DESKTOP_OFFSCREEN (offscreen.ts): placed off every display, never shown.
   * `framesDir`: off-screen, where each show and move is recorded (capsuleFramesDir); null records none.
   * `still`: makes its native window show and hide with no AppKit animation (tray.ts stillWith);
   * null in a test or off-screen run.
   */
  constructor(
    private readonly testMode: boolean,
    private readonly offscreen = false,
    private readonly framesDir: string | null = null,
    still: ((handle: Buffer) => boolean) | null = null,
  ) {
    const mac = process.platform === 'darwin';
    const glass = glassOf('capsule', process.platform, testMode);
    const win = new BrowserWindow({
      ...START,
      ...(testMode ? { x: OFF, y: OFF, focusable: false } : offscreen ? { x: OFF, y: OFF } : {}),
      show: false,
      frame: false,
      transparent: true,
      resizable: false,
      movable: false,
      minimizable: false,
      maximizable: false,
      fullscreenable: false,
      skipTaskbar: true,
      hasShadow: false,
      alwaysOnTop: true,
      ...(mac ? { type: 'panel' } : { type: 'toolbar' }),
      // macOS: the vibrancy is the glass (glass.ts), the window is the glass
      ...glass,
      title: 'bise capsule',
      webPreferences: webPreferences('capsule', testMode),
    });
    // what macOS was asked to draw under the page (lead m_17666: the VM pass tells VM from app with it)
    if (glass.vibrancy !== undefined) {
      process.stderr.write(`bise-desktop: glass: capsule vibrancy=${glass.vibrancy} state=${String(glass.visualEffectState)} background=${win.getBackgroundColor()}\n`);
    }
    // no AppKit pop on show (~170 ms) nor fade on hide (~250 ms): the tray's dropdown is there at
    // once, as macOS's own menus (tray-instant); the corner card and the tray menu fade out
    // on their own (hide, fadeOut)
    if (mac && still !== null) still(win.getNativeWindowHandle());
    if (!testMode) {
      win.setAlwaysOnTop(true, 'screen-saver');
      win.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true, skipTransformProcessType: true });
    }
    win.on('blur', () => {
      if (this.state.shown && this.state.keys) this.hide('blur');
    });
    this.win = win;
    loadRenderer(win, 'capsule', undefined);
  }

  get window(): BrowserWindow {
    return this.win;
  }

  show(at: CapsuleAt, keys: boolean, why: CapsuleState['why'] = 'asked', fit: CapsuleFit | null = null): void {
    this.state = { shown: true, at, keys, why, room: this.display().workArea.height };
    // placed (and kept inside the screen) once, here; then pinned
    this.pin = null;
    this.would = null;
    this.fit = at === 'bottom' ? fit : null;
    this.stopFade();
    this.place();
    if (this.testMode) {
      process.stdout.write(`${CAPSULE_LINE} show ${at}${keys ? ' keys' : ''}\n`);
    } else if (this.offscreen) {
      // placed off every display (display()), never ordered in
    } else if (keys) {
      this.win.show();
    } else {
      this.win.showInactive();
    }
    this.tell();
  }


  hide(why: CapsuleState['why'] = 'asked'): void {
    if (!this.state.shown) return;
    // the corner card fades with its page; the tray menu fades as macOS's status menus do, but
    // not under reduced motion (designer m_64430); the rest goes at once
    const at = this.state.at;
    const fades = (): boolean => at === 'corner' || (at === 'menu' && !a11yNow().reduceMotion);
    this.state = { ...this.state, shown: false, keys: false, why };
    this.pin = null;
    this.would = null;
    if (this.testMode) process.stdout.write(`${CAPSULE_LINE} hide\n`);
    else if (!this.offscreen && fades()) this.fadeOut();
    else this.win.hide();
    this.tell();
  }

  /** the corner card (with its page) or the tray menu goes out: alpha 1 → 0 in CORNER_FADE_MS, then hidden at 1 again */
  private fadeOut(): void {
    this.stopFade();
    let n = 0;
    this.fading = setInterval(() => {
      n += 1;
      if (this.win.isDestroyed()) {
        this.stopFade();
        return;
      }
      if (n < FADE_STEPS) {
        this.win.setOpacity(1 - n / FADE_STEPS);
        return;
      }
      this.stopFade();
      this.win.hide();
    }, CORNER_FADE_MS / FADE_STEPS);
  }

  /** a running fade ends (a show, or its last step): the window back at alpha 1 */
  private stopFade(): void {
    if (this.fading === null) return;
    clearInterval(this.fading);
    this.fading = null;
    if (!this.win.isDestroyed()) this.win.setOpacity(1);
  }

  resize(width: number, height: number, glass?: Rect): void {
    if (!(width > 0 && height > 0 && width < 10_000 && height < 10_000)) return;
    this.size = { width, height };
    this.glass = glass;
    this.place();
  }

  /** the tray's click: its dropdown under the pearl, again: gone */
  toggleMenu(): void {
    if (this.state.shown && this.state.at === 'menu') this.hide('tray');
    else this.show('menu', true, 'tray');
  }

  private display(): DisplayBox {
    return this.testMode || this.offscreen ? OFF_DISPLAY : this.realDisplay();
  }

  /** the display under his cursor (the tray's for the menu): only read, nothing goes there off-screen */
  private realDisplay(): DisplayBox {
    const d = this.state.at === 'menu' && this.tray !== null ? screen.getDisplayMatching(this.tray.getBounds()) : screen.getDisplayNearestPoint(screen.getCursorScreenPoint());
    return { bounds: d.bounds, workArea: d.workArea };
  }

  private anchor(): Anchor {
    const at = this.state.at;
    if (at === 'menu') {
      const icon = this.tray?.getBounds();
      // no tray (a test run, or none on this OS): the top of the display
      return icon === undefined || this.testMode ? { at: 'top' } : { at: 'menu', icon };
    }
    return { at };
  }

  /** At show: centred on its anchor, inside the screen, and pinned. Shown: a new size keeps the glass's left and anchored edge. */
  private place(): void {
    if (this.pin !== null) {
      // a wider content (the spotlight's thread pane) grows right, shifted left only as much as
      // keeps it on its screen (place.ts onScreen)
      const kept = onScreen(pinnedBounds(this.size, this.pin, this.glass), this.pin, this.pinArea ?? this.display().workArea);
      this.pin = kept.pin;
      this.win.setBounds(kept.bounds);
    } else {
      const d = this.display();
      const b = this.boundsOn(d);
      this.win.setBounds(b);
      if (this.state.shown) {
        this.pin = this.pinAt(b);
        this.pinArea = d.workArea;
      }
    }
    this.record();
  }

  /** where a show puts the window on `d` */
  private boundsOn(d: DisplayBox): Rect {
    const fit = this.fit;
    return fit !== null ? fitBounds(this.size, d, this.glass, fit.maxHeight) : capsuleBounds(this.size, d, this.anchor(), this.glass);
  }

  /** a fit view keeps its top-left (it grows down), the others their anchored edge */
  private pinAt(b: Rect): Pin {
    return pinOf(b, this.size, this.fit !== null ? { at: 'top' } : this.anchor(), this.glass);
  }

  /** off-screen, shown: where it would sit on his display, appended to framesDir (capsuleframe.ts) */
  private record(): void {
    if (this.framesDir === null || !this.state.shown) return;
    const w = this.would;
    const d = w?.d ?? this.realDisplay();
    let b: Rect;
    if (w !== null) {
      const kept = onScreen(pinnedBounds(this.size, w.pin, this.glass), w.pin, d.workArea);
      this.would = { pin: kept.pin, d };
      b = kept.bounds;
    } else {
      b = this.boundsOn(d);
      this.would = { pin: this.pinAt(b), d };
    }
    appendCapsuleFrame(this.framesDir, capsuleFrame(Date.now(), w === null ? 'show' : 'move', this.state.at, b, d, this.win.getBounds()));
  }

  /** main's own listeners of its state (the spotlight's still, helper/shot.ts) */
  private readonly watchers: ((s: CapsuleState) => void)[] = [];

  /** Calls `listener` with each new state (shown, where). */
  onState(listener: (s: CapsuleState) => void): void {
    this.watchers.push(listener);
  }

  private tell(): void {
    this.win.webContents.send(IPC.capsuleState, this.state);
    for (const w of this.watchers) w(this.state);
  }
}

/** The capsule's IPC: any window may show, hide or size it. */
export function installCapsuleIpc(c: Capsule): void {
  const ats: ReadonlySet<string> = new Set(['top', 'bottom', 'menu', 'corner']);
  ipcMain.on(IPC.capsuleShow, (_e, at: unknown, keys: unknown, fit: unknown) => {
    if (typeof at === 'string' && ats.has(at)) c.show(at as CapsuleAt, keys === true, 'asked', isFit(fit) ? fit : null);
  });
  ipcMain.on(IPC.capsuleHide, () => {
    c.hide();
  });
  ipcMain.on(IPC.capsuleSize, (_e, width: unknown, height: unknown, glass: unknown) => {
    if (typeof width !== 'number' || typeof height !== 'number') return;
    c.resize(width, height, isRect(glass) ? glass : undefined);
  });
}

function isFit(v: unknown): v is CapsuleFit {
  if (typeof v !== 'object' || v === null) return false;
  const f = v as Record<string, unknown>;
  return f['grow'] === 'down' && typeof f['maxHeight'] === 'number' && f['maxHeight'] > 0 && f['maxHeight'] < 10_000;
}

function isRect(v: unknown): v is Rect {
  if (typeof v !== 'object' || v === null) return false;
  const r = v as Record<string, unknown>;
  return ['x', 'y', 'width', 'height'].every((k) => typeof r[k] === 'number');
}
