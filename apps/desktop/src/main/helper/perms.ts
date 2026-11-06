// The helper's permissions to the core (settings · Setup's gauge, architect m_53994): main owns the
// helper and the core, so main relays the fact, never a window (none open, or several, would break
// it). Sent at each helper `perms` and again with the last one when a core comes up; an off-screen
// run has no helper and relays run.ts's GRANTED, like the pages see. A fact only: the core decides
// what it means. Owner: setup-page.
import type { AppCmd, HelperEv } from '../../proto';

/** The core's `helper_perms` for a helper event; null for any other event. Pure. */
export function permsCmd(ev: HelperEv): AppCmd | null {
  if (ev.ev !== 'perms') return null;
  return { cmd: 'helper_perms', mic: ev.mic, accessibility: ev.accessibility, screen: ev.screen };
}

/** Keeps the last perms and sends them: at each new one, and at `coreUp`. */
export class PermsRelay {
  private last: AppCmd | null = null;

  constructor(private readonly send: (cmd: AppCmd) => boolean) {}

  helperEv(ev: HelperEv): void {
    const c = permsCmd(ev);
    if (c === null) return;
    this.last = c;
    this.send(c);
  }

  /** A core (re)started: it starts from nothing, so it hears the last perms again. */
  coreUp(): void {
    if (this.last !== null) this.send(this.last);
  }
}
