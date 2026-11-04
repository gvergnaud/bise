// What the menu bar's ':*' shows (designer's bise-presence v10, m_24497): pure, tray.ts draws it.
// Always a template image (black + alpha: macOS tints it white or black for the bar, which on
// macOS 26 follows the wallpaper, not the system theme: Electron can't read it, and two screens can
// have different bars). At rest the ':*'. While agents work, the plain three-branch star turns one
// step per 400 ms (the team's branches, 1b, only from 26 px up: they don't read at 15, designer
// m_24537). When something waits on him, the same template ':*' with a dot at its bottom right
// (his ask, m_53252: like Iru's): the template has a hole there, the dot is drawn over it in its
// own colour by the tray-badge addon (traybadge.ts), the way a native app badges its status item,
// since a template image has no colour. Without the addon, the dot is the tray's title (an ANSI
// red '•' beside the icon). On each new wait two kisses, two beats (1 → 1.25 → 1, 140 ms each,
// 120 ms apart), then still. Under reduced motion no steps and no beats. Its tip says the state in
// words (law 6).
import { PLAIN_ARMS, rasterMark, type Rgba } from '../shared/mark';
import type { Counts } from './counts';
import { common } from '../proto/words_common.gen';
import { panel } from '../proto/words_panel.gen';

/** The icon's box in points (designer: 15 px in the menu bar). */
export const TRAY_PT = 15;
/** One working step. */
export const STEP_MS = 400;
/** Working: the steps of a turn before the star looks the same again. */
export const STEPS = 3;
/** One frame of the two kisses. */
export const BEAT_FRAME_MS = 35;
/** The two kisses as star scales, one per BEAT_FRAME_MS: a 140 ms beat, 120 ms apart, a 140 ms beat. */
export const KISSES: readonly number[] = [1.12, 1.25, 1.12, 1, 1, 1, 1, 1.12, 1.25, 1.12, 1];

export type TrayLook = 'rest' | 'work' | 'wait';

/** The look for the counts. Pure. */
export function trayLook(c: Counts): TrayLook {
  if (c.waiting > 0) return 'wait';
  return c.working > 0 ? 'work' : 'rest';
}

/** Whether a change of counts is a new wait (more things wait than before): the two kisses. Pure. */
export const newWait = (before: Counts, now: Counts): boolean => now.waiting > before.waiting;

/** The tip (law 6: every mark explains itself). Pure. */
export function trayTip(c: Counts): string {
  const tip = (what: string) => panel.tip_of({ who: panel.bise, what });
  if (c.waiting > 0) return tip(panel.tip_things_wait({ n: c.waiting }));
  if (c.working > 0) return tip(panel.tip_working_with({ n: c.working }));
  return tip(common.all_quiet);
}

/** The star's turn in degrees at a working step: a third of its 60° symmetry per step. Pure. */
export const stepTurn = (step: number): number => ((step % STEPS) * 60) / STEPS;

/** The wait's dot, in points from the icon's top left (its 15 pt box): centre, diameter, the
 * clear ring the template cuts around it, its colour (0..1 sRGB). */
export interface TrayDot {
  readonly x: number;
  readonly y: number;
  readonly d: number;
  readonly ring: number;
  readonly r: number;
  readonly g: number;
  readonly b: number;
}

/** The dot (designer m_53288): macOS's red (#ff3b30, the system's 'something waits' badge; our
 * pink would melt into his purple bar), 4.5 pt on the star's corner: it keeps 5 whole arms. */
export const TRAY_DOT: TrayDot = { x: 12.75, y: 12.75, d: 4.5, ring: 1, r: 1, g: 59 / 255, b: 48 / 255 };

/** The dot when the addon can't draw it: the tray's title, an ANSI red bullet beside the icon. */
export const TITLE_DOT = '\x1b[31m•';

/** What the tray shows for a look: the template's hole and the dot over it (`badge`, when the
 * addon draws), or the dot as its title (`title`, the fallback). Always a template. Pure. */
export interface TrayPaint {
  readonly template: true;
  readonly hole: boolean;
  readonly badge: TrayDot | null;
  readonly title: string;
}

/** The paint for a look, with the addon (`addon`: it loaded and found the item) or without. Pure. */
export function trayPaint(look: TrayLook, addon: boolean): TrayPaint {
  if (look !== 'wait') return { template: true, hole: false, badge: null, title: '' };
  return addon ? { template: true, hole: true, badge: TRAY_DOT, title: '' } : { template: true, hole: false, badge: null, title: TITLE_DOT };
}

const BLACK: Rgba = [0, 0, 0, 255];

/** Cut the dot and its ring out of a square RGBA image of `px` pixels at `scale` (antialiased). Pure. */
export function cutHole(rgba: Uint8Array, px: number, scale: number, dot: TrayDot): Uint8Array {
  const r = (dot.d / 2 + dot.ring) * scale;
  const cx = dot.x * scale;
  const cy = dot.y * scale;
  for (let j = 0; j < px; j++) {
    for (let i = 0; i < px; i++) {
      const dist = Math.hypot(i + 0.5 - cx, j + 0.5 - cy) - r;
      const keep = Math.max(0, Math.min(1, dist + 0.5));
      const o = (j * px + i) * 4 + 3;
      rgba[o] = Math.round((rgba[o] ?? 0) * keep);
    }
  }
  return rgba;
}

export interface TrayFrame {
  /** premultiplied BGRA (Electron's raw bitmap on macOS), `px` square */
  readonly bgra: Uint8Array;
  readonly px: number;
}

/** RGBA straight alpha → premultiplied BGRA. Pure. */
export function toBgra(rgba: Uint8Array): Uint8Array {
  const out = new Uint8Array(rgba.length);
  for (let o = 0; o < rgba.length; o += 4) {
    const a = (rgba[o + 3] ?? 0) / 255;
    out[o] = Math.round((rgba[o + 2] ?? 0) * a);
    out[o + 1] = Math.round((rgba[o + 1] ?? 0) * a);
    out[o + 2] = Math.round((rgba[o] ?? 0) * a);
    out[o + 3] = rgba[o + 3] ?? 0;
  }
  return out;
}

/**
 * One frame of the icon at a scale (1, 2), always black + alpha (a template: macOS tints it);
 * `hole`: the dot's place cut out (the addon draws the dot there). Pure.
 */
export function trayFrame(
  look: TrayLook,
  scale: number,
  o: { readonly step?: number; readonly beat?: number; readonly hole?: boolean } = {},
): TrayFrame {
  const px = TRAY_PT * scale;
  const turn = look === 'work' ? stepTurn(o.step ?? 0) : 0;
  const shape = { size: TRAY_PT, arms: PLAIN_ARMS, turn, beat: o.beat ?? 1 };
  const rgba = rasterMark(px, BLACK, BLACK, shape);
  if (o.hole === true) cutHole(rgba, px, scale, TRAY_DOT);
  return { bgra: toBgra(rgba), px };
}
