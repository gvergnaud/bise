// The tray-badge addon (apps/desktop/native/mac/badge/TrayBadge.m), main's side: the dot over the
// menu bar's template ':*' when something waits (traylook.ts trayPaint decides; the addon only
// draws what it is given). darwin only, loaded once and lazily, never in the renderer. It fails
// soft: no addon (a dev run, another OS, a load error) or no status item found, and set() says
// false, so the tray shows its fallback (the title dot) with one log line, never a crash.
// The same addon makes the capsule's window show and hide with no AppKit animation
// (stillWindow, tray-instant), failing soft the same way: the animation stays.
// Where it is: BISE_DESKTOP_TRAY_BADGE, else the packed app's Contents/Resources/tray-badge.node
// (electron-builder.mjs, built next to the helper by native/mac/build.sh).
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import type { TrayDot } from './traylook';

export const BADGE_ENV = 'BISE_DESKTOP_TRAY_BADGE';
export const BADGE_FILE = 'tray-badge.node';

export interface Addon {
  setBadge(dot: TrayDot | null): boolean;
  /** the window of that NSView* shows and hides with no AppKit animation; absent in an older build */
  stillWindow?(handle: Buffer): boolean;
}

/**
 * The capsule shows and hides at once, with no AppKit pop or fade (TrayBadge.m stillWindow);
 * false when it couldn't, with one log line: no addon (a dev run), an older addon, no window.
 */
export function stillWindow(addon: Addon | null, handle: Buffer, log: (why: string) => void): boolean {
  if (addon === null) {
    log('appkit addon: none in this run, the capsule keeps macOS open animation');
    return false;
  }
  if (typeof addon.stillWindow !== 'function') {
    log('appkit addon: no stillWindow in this build, the capsule keeps macOS open animation');
    return false;
  }
  try {
    const ok = addon.stillWindow(handle);
    if (!ok) log('appkit addon: stillWindow found no window, the capsule keeps macOS open animation');
    return ok;
  } catch (e) {
    log(`appkit addon: stillWindow failed (${e instanceof Error ? e.message : String(e)})`);
    return false;
  }
}

/** Where the addon is for this run, or null (see the header). Pure. */
export function badgePath(o: { platform: string; env: Record<string, string | undefined>; packaged: boolean; resources: string }): string | null {
  if (o.platform !== 'darwin') return null;
  const set = o.env[BADGE_ENV];
  if (set !== undefined && set !== '') return set;
  return o.packaged ? join(o.resources, BADGE_FILE) : null;
}

/** The addon at a path, or null when it can't load; `why` says why (one log line). */
export function loadBadge(path: string | null, load: (p: string) => unknown, log: (why: string) => void): Addon | null {
  if (path === null) return null;
  if (!existsSync(path)) {
    log(`tray badge: no addon at ${path}, the dot is the title`);
    return null;
  }
  try {
    const m = load(path) as Partial<Addon> | null;
    if (m !== null && typeof m.setBadge === 'function') return m as Addon;
    log(`tray badge: ${path} has no setBadge, the dot is the title`);
  } catch (e) {
    log(`tray badge: ${path} did not load (${e instanceof Error ? e.message : String(e)}), the dot is the title`);
  }
  return null;
}

/** The dot over the tray icon: `set` says whether the addon drew (or cleared) it. */
export class TrayBadge {
  private warned = false;
  constructor(private readonly addon: Addon | null, private readonly log: (why: string) => void) {}

  get loaded(): boolean {
    return this.addon !== null;
  }

  set(dot: TrayDot | null): boolean {
    if (this.addon === null) return false;
    try {
      const ok = this.addon.setBadge(dot);
      if (!ok && dot !== null && !this.warned) {
        this.warned = true;
        this.log('tray badge: no status item found, the dot is the title');
      }
      return ok;
    } catch (e) {
      if (!this.warned) {
        this.warned = true;
        this.log(`tray badge: setBadge failed (${e instanceof Error ? e.message : String(e)}), the dot is the title`);
      }
      return false;
    }
  }
}
