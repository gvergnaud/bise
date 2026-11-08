// The app's usage clock (page usage-metrics-plan, step 3; architect m_53623 d.3): how long the app
// was open, focused and engaged, from main's own events only: its windows' focus and blur,
// powerMonitor's suspend, resume, lock and unlock, and the renderer's input signal (no content: a
// signal that he typed, clicked or scrolled). These states stay in main; the renderer keeps none.
//
// open: the app runs and the Mac is awake and unlocked. focused: open, with one of its windows in
// front. engaged: focused, with his input in the last ENGAGED_MS (the reco's 60 s).
//
// Pure: the times come in as arguments (ms). take() gives the whole seconds since the last take and
// keeps the rest, so nothing is lost or counted twice between two usage_time commands.

/** Input this recent makes a focused moment engaged (usage-metrics-reco: 60 s). */
export const ENGAGED_MS = 60_000;

export type ClockEvent = 'focus' | 'blur' | 'suspend' | 'resume' | 'lock' | 'unlock' | 'input';

/** Seconds of each kind of time. */
export interface Spent {
  readonly open_s: number;
  readonly focused_s: number;
  readonly engaged_s: number;
}

export class UsageClock {
  private at: number;
  private focused = false;
  private suspended = false;
  private locked = false;
  private lastInput: number | null = null;
  private open = 0;
  private front = 0;
  private engaged = 0;

  constructor(now: number) {
    this.at = now;
  }

  /** One of main's events at `now`. */
  on(e: ClockEvent, now: number): void {
    this.advance(now);
    switch (e) {
      case 'focus':
        this.focused = true;
        return;
      case 'blur':
        this.focused = false;
        return;
      case 'suspend':
        this.suspended = true;
        return;
      case 'resume':
        this.suspended = false;
        return;
      case 'lock':
        this.locked = true;
        return;
      case 'unlock':
        this.locked = false;
        return;
      case 'input':
        this.lastInput = now;
        return;
    }
  }

  /** The whole seconds spent since the last take; the rest stays for the next one. */
  take(now: number): Spent {
    this.advance(now);
    const whole = (ms: number): number => Math.floor(ms / 1000);
    const out: Spent = { open_s: whole(this.open), focused_s: whole(this.front), engaged_s: whole(this.engaged) };
    this.open -= out.open_s * 1000;
    this.front -= out.focused_s * 1000;
    this.engaged -= out.engaged_s * 1000;
    return out;
  }

  /** Counts the time from the last event to `now` in the state it was in. */
  private advance(now: number): void {
    const from = this.at;
    const to = Math.max(now, from);
    this.at = to;
    if (this.suspended || this.locked) return;
    this.open += to - from;
    if (!this.focused) return;
    this.front += to - from;
    if (this.lastInput === null) return;
    const until = Math.min(to, this.lastInput + ENGAGED_MS);
    this.engaged += Math.max(0, until - from);
  }
}
