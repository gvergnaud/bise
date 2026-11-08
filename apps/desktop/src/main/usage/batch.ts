// What main has to tell the core about the app's usage (page usage-metrics-plan, step 3): the
// clock's seconds and the gestures since the last usage_time that the core took. A send the core
// didn't take (it was down, restarting) keeps it all for the next one. Gestures are closed names
// (bise_proto's AppGesture), checked here: a window's anything else is dropped. Pure.
import type { AppCmd, AppGesture } from '../../proto';
import type { Spent } from './clock';

/** Every gesture a window or main may count; the record makes a new AppGesture a compile error here. */
const GESTURES: Readonly<Record<Exclude<AppGesture, 'unknown'>, true>> = {
  inbox_opened: true,
  artifact_page: true,
  artifact_doc: true,
  artifact_sheet: true,
  artifact_slides: true,
  artifact_image: true,
  artifact_link: true,
  artifact_code: true,
  artifact_other: true,
  setup_started: true,
  crash_renderer: true,
  crash_child: true,
  crash_core: true,
};

/** At most this many gestures wait for the core (a window gone wild can't grow main's memory). */
export const GESTURES_MAX = 500;

/** Whether `g` is a gesture name (a window's IPC argument is untrusted). */
export function isGesture(g: unknown): g is AppGesture {
  return typeof g === 'string' && Object.hasOwn(GESTURES, g);
}

export type UsageTime = Extract<AppCmd, { cmd: 'usage_time' }>;

export class UsageBatch {
  private open = 0;
  private focused = 0;
  private engaged = 0;
  private gestures: AppGesture[] = [];

  /** The clock's seconds since its last take. */
  add(s: Spent): void {
    this.open += s.open_s;
    this.focused += s.focused_s;
    this.engaged += s.engaged_s;
  }

  /** One gesture; a name that isn't one, or past GESTURES_MAX, is dropped. */
  gesture(g: unknown): void {
    if (isGesture(g) && this.gestures.length < GESTURES_MAX) this.gestures.push(g);
  }

  /** The command for what waits, or null when nothing does. */
  cmd(): UsageTime | null {
    if (this.open === 0 && this.focused === 0 && this.engaged === 0 && this.gestures.length === 0) return null;
    return {
      cmd: 'usage_time',
      open_s: this.open,
      focused_s: this.focused,
      engaged_s: this.engaged,
      ...(this.gestures.length === 0 ? {} : { gestures: [...this.gestures] }),
    };
  }

  /** The core took cmd(): it starts over. */
  sent(): void {
    this.open = 0;
    this.focused = 0;
    this.engaged = 0;
    this.gestures = [];
  }
}
