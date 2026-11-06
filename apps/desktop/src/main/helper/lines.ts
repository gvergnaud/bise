// bise-mac-helper's lines, main's side (bise_proto::helper; the types are
// generated into src/proto). Pure: the stdout splitter, the decode of one
// event line, the encode of a command, which events are state (the last one
// is replayed to a late window), and the restart rule. src/main/helper/run.ts
// is the shell that spawns the process and calls these.
import type { HelperCmd, HelperEv } from '../../proto';

/** Every tag the helper may say (an unknown one, from a newer helper, is logged and dropped). */
export const HELPER_EV_TAGS: ReadonlySet<HelperEv['ev']> = new Set([
  'hello',
  'fn',
  'context',
  'front',
  'perms',
  'quiet',
  'shot',
  'error',
]);

/** The events that describe a state: a window that opens later gets the last of each. */
export const STATE_TAGS: ReadonlySet<HelperEv['ev']> = new Set(['hello', 'front', 'perms', 'quiet']);

/** Chunks of stdout in, whole lines out (a line may span chunks; blank lines are dropped). */
export class LineSplitter {
  private rest = '';

  push(chunk: string): string[] {
    const parts = (this.rest + chunk).split('\n');
    this.rest = parts.pop() ?? '';
    return parts.map((l) => l.trim()).filter((l) => l !== '');
  }
}

export type Decoded = { readonly ok: true; readonly ev: HelperEv } | { readonly ok: false; readonly why: string };

/**
 * One line -> an event. Only the tag is checked here: the helper's own tests
 * pin every shape to the same fixtures (build.sh --test), so a known tag
 * with bad fields is a helper bug, not a version skew.
 */
export function decodeEv(line: string): Decoded {
  let v: unknown;
  try {
    v = JSON.parse(line);
  } catch {
    return { ok: false, why: 'not JSON' };
  }
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return { ok: false, why: 'not an object' };
  const tag = (v as Record<string, unknown>)['ev'];
  if (typeof tag !== 'string') return { ok: false, why: 'no "ev"' };
  if (!HELPER_EV_TAGS.has(tag as HelperEv['ev'])) return { ok: false, why: `unknown ev ${tag}` };
  return { ok: true, ev: v as HelperEv };
}

export function encodeCmd(c: HelperCmd): string {
  return `${JSON.stringify(c)}\n`;
}

/** The commands a window may send (quit is main's alone). */
export function windowMaySend(c: HelperCmd): boolean {
  return c.cmd !== 'quit';
}

/**
 * After the helper exited at `now` (ms): wait this long and start it again,
 * or null to give up. Restarts within the last minute count: 1 s, 2 s, 4 s,
 * 8 s, 16 s, then no more until a minute passes without a crash.
 */
export function restartDelay(crashes: readonly number[], now: number): number | null {
  const recent = crashes.filter((t) => now - t < 60_000).length;
  if (recent >= 5) return null;
  return 1000 * 2 ** recent;
}

/**
 * What a window gets of `ev`: his selection (a context's `selection`, read at fn space only) goes
 * to the capsule alone, where the spotlight shows it as the ❝ chip he can drop; every other window
 * gets the context without it (architect m_53938). Pure.
 */
export function evForWindow(ev: HelperEv, capsule: boolean): HelperEv {
  if (capsule || ev.ev !== 'context' || !('selection' in ev.context)) return ev;
  const context = { ...ev.context };
  delete context.selection;
  return { ...ev, context };
}

/**
 * Whether a talk is open after `ev` (fn talk_start or fn fn opens one; talk_end or cancel ends it).
 * A helper that dies mid-talk sends neither: run.ts ends it for the windows (amb-core m_22285).
 */
export function talkOpen(open: boolean, ev: HelperEv): boolean {
  if (ev.ev !== 'fn') return open;
  if (ev.act === 'talk_start' || ev.act === 'double') return true;
  if (ev.act === 'talk_end' || ev.act === 'cancel') return false;
  return open;
}

/** The last state event of each kind, replayed in order to a window that opens. */
export class HelperState {
  private last = new Map<HelperEv['ev'], HelperEv>();

  take(ev: HelperEv): void {
    if (STATE_TAGS.has(ev.ev)) this.last.set(ev.ev, ev);
  }

  snapshot(): HelperEv[] {
    return [...this.last.values()];
  }

  clear(): void {
    this.last.clear();
  }
}
