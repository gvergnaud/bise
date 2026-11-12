// A file's Quick Look thumbnail, asked of the helper ({cmd:'thumb', id, path, out, px}:
// Thumb.swift's QLThumbnailGenerator, a thumbnail only, never the type's icon; docs/plugin-apps.md
// A6b, architect m_60495) for an artifact card's picture (thumbs/make.ts). Nothing here knows a
// file type: Quick Look says itself which files it draws, so an answer without `ok` means "ask
// the file's app". The path is main's (the store's copy or the root's file behind the realpath
// fence), never the renderer's. Not answered in 5 s: given up (null); its late PNG, if any, is
// deleted. An off-screen or test run has no helper: no Looks, the maker decodes what Electron can.
import type { HelperCmd, HelperEv } from '../../proto';

/** The helper's answer: its picture written at `out` (`ok`), its length when it has one. */
export interface Look {
  readonly ok: boolean;
  readonly ms?: number;
}

export interface LookPorts {
  /** a command to the helper */
  readonly helper: (c: Extract<HelperCmd, { cmd: 'thumb' }>) => void;
  /** calls `f` after `ms`; returns its cancel */
  readonly later: (ms: number, f: () => void) => () => void;
  /** deletes a PNG nobody waits for (a late answer's) */
  readonly drop: (path: string) => void;
}

/** A Quick Look that takes longer is given up (the card says its ending meanwhile). */
export const LOOK_TIMEOUT_MS = 5000;

interface Waiting {
  readonly out: string;
  readonly done: (l: Look | null) => void;
  readonly cancel: () => void;
}

export class Looks {
  private n = 0;
  private readonly waiting = new Map<string, Waiting>();
  /** the given-up asks' PNGs, deleted if their answer still comes */
  private readonly late = new Map<string, string>();

  constructor(private readonly ports: LookPorts) {}

  /** Quick Look's picture of `path` at `out`, `px` wide at most; null: given up. */
  look(path: string, out: string, px: number): Promise<Look | null> {
    this.n += 1;
    const id = `t${String(this.n)}`;
    return new Promise((done) => {
      const cancel = this.ports.later(LOOK_TIMEOUT_MS, () => {
        this.waiting.delete(id);
        this.late.set(id, out);
        done(null);
      });
      this.waiting.set(id, { out, done, cancel });
      this.ports.helper({ cmd: 'thumb', id, path, out, px });
    });
  }

  /** Every helper event: a `thumb` answers its ask. */
  ev(ev: HelperEv): void {
    if (ev.ev !== 'thumb') return;
    const w = this.waiting.get(ev.id);
    if (w === undefined) {
      const out = this.late.get(ev.id);
      this.late.delete(ev.id);
      if (out !== undefined && ev.ok === true) this.ports.drop(out);
      return;
    }
    this.waiting.delete(ev.id);
    w.cancel();
    const ms = ev.ms ?? undefined;
    w.done(ms === undefined ? { ok: ev.ok === true } : { ok: ev.ok === true, ms });
  }
}
