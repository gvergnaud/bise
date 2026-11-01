// What the menu bar's ':*' shows (designer's bise-presence v10, m_24497): pure, tray.ts draws it.
// At rest the ink ':*', a template image (black + alpha: macOS tints it for the menu bar). While
// agents work, the plain three-branch star turns one step per 400 ms (template frames; the team's
// branches, 1b, only from 26 px up: they don't read at 15, designer m_24537). When something waits on him, a colour image (macOS shows colour only
// in a non-template one): the whole ':*' in one pink, WAIT_PINK, the same on any bar (designer
// m_51405): the macOS 26 menu bar follows the wallpaper, not the system theme, Electron can't read
// it, and two screens can have different bars, so the colour image can't follow it. On each new
// wait two kisses, two beats (1 → 1.25 → 1, 140 ms each, 120 ms apart), then still. Under reduced
// motion no steps and no beats: the shape and the colour only. Its tip says the state in words (law 6).
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

/** A wait's pink (#d03a72, pink = waits on him): eyes and star, read on a light, a pink and a dark bar. */
export const WAIT_PINK: Rgba = [208, 58, 114, 255];
const BLACK: Rgba = [0, 0, 0, 255];

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
 * One frame of the icon at a scale (1, 2): a template look draws black (macOS tints it), a wait
 * draws it all in WAIT_PINK. Pure.
 */
export function trayFrame(look: TrayLook, scale: number, o: { readonly step?: number; readonly beat?: number } = {}): TrayFrame {
  const px = TRAY_PT * scale;
  const turn = look === 'work' ? stepTurn(o.step ?? 0) : 0;
  const shape = { size: TRAY_PT, arms: PLAIN_ARMS, turn, beat: o.beat ?? 1 };
  const ink = look === 'wait' ? WAIT_PINK : BLACK;
  const rgba = rasterMark(px, ink, ink, shape);
  return { bgra: toBgra(rgba), px };
}
