// The review column's state (page review-guided, pick B): the guide asked and swapped in place
// (useGuideState), the files he marked viewed (useViewed, kept: a review goes on tomorrow), and
// where he is among the sections (useSectionNav: the bar's place, ‹ › and n / p). DiffFiles uses
// them; the rules they keep are the page's and architect m_57259's.
import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import type { Guide, GuideTarget } from '@/proto';
import { keep } from '@/app/keep';
import { useGuide, useStore, type Diff, type GuideEv } from '@/store';
import { guidable, type Section } from './column';
import type { Mode } from './ReviewBar';

/** He has scrolled: a guide that comes now waits for his 'Show it' (it never moves what he reads). */
const READING_PX = 24;

export interface GuideState {
  readonly ev: GuideEv | null;
  /** the guide drawn now (null: By kind) */
  readonly shown: Guide | null;
  readonly mode: Mode;
  /** a guide is here, drawable on this diff */
  readonly ready: boolean;
  readonly setMode: (m: Mode) => void;
  /** his click: write it (his main model too) or try again */
  readonly run: () => void;
  /** 'Show it': the guide, at the part holding the file he was reading */
  readonly show: () => void;
  /** the column scrolled to `top` */
  readonly noteScroll: (top: number) => void;
  /** code scrolls under the bar (its hairline) */
  readonly under: boolean;
}

export function useGuideState(project: string, diff: Diff | null, target: GuideTarget | undefined, body: RefObject<HTMLElement | null>): GuideState {
  const store = useStore();
  const hash = diff?.hash ?? null;
  const ev = useGuide(project, target === undefined ? null : hash);
  const [mode, setModeState] = useState<Mode>('guide');
  const [shown, setShown] = useState<Guide | null>(null);
  const [scrolled, setScrolled] = useState(false);
  // the diff's guide, asked once per diff (a new push is a new hash)
  useEffect(() => {
    if (target !== undefined && hash !== null) store.command({ cmd: 'guide', project, target });
  }, [store, project, target, hash]);
  // another diff (a push): its own guide, none shown until it comes
  const [hashSeen, setHashSeen] = useState(hash);
  if (hash !== hashSeen) {
    setHashSeen(hash);
    setShown(null);
  }
  const guide = ev?.state === 'ready' && ev.guide != null && diff !== null && guidable(diff) ? ev.guide : null;
  // ready while he hasn't scrolled: in place at once; else his 'Show it' (never moves what he reads)
  if (guide !== null && shown === null && mode === 'guide' && !scrolled) setShown(guide);
  const [under, setUnder] = useState(false);
  const noteScroll = useCallback((top: number) => {
    setScrolled(top > READING_PX);
    setUnder(top > 0);
  }, []);
  const run = useCallback(() => {
    if (target !== undefined) store.command({ cmd: 'guide', project, target, run: true });
  }, [store, project, target]);
  const show = useCallback(() => {
    if (guide === null) return;
    const path = readingPath(body.current);
    setModeState('guide');
    setShown(guide);
    requestAnimationFrame(() => {
      const el = path === null ? null : body.current?.querySelector<HTMLElement>(`.rv-file[data-path="${CSS.escape(path)}"]`);
      (el?.closest<HTMLElement>('.rv-part') ?? null)?.scrollIntoView({ block: 'start' });
    });
  }, [guide, body]);
  const setMode = useCallback(
    (m: Mode) => {
      setModeState(m);
      if (m === 'guide' && guide !== null) setShown(guide);
    },
    [guide],
  );
  return { ev, shown, mode, ready: guide !== null, setMode, run, show, noteScroll, under };
}

/** The path of the file card at the column's top (what he is reading), or null. */
function readingPath(el: HTMLElement | null): string | null {
  if (el === null) return null;
  const top = el.getBoundingClientRect().top;
  const files = [...el.querySelectorAll<HTMLElement>('.rv-file')];
  const at = files.find((f) => f.getBoundingClientRect().bottom > top + 48) ?? null;
  return at?.dataset['path'] ?? null;
}

/** The newest marks kept per project. */
const KEEP_MARKS = 500;

export interface Viewed {
  readonly set: ReadonlySet<string>;
  readonly mark: (file: string, on: boolean) => void;
}

/** The files he marked viewed, by their key (column.ts fileKey), kept across windows and days. */
export function useViewed(project: string): Viewed {
  const key = `viewed:${project}`;
  const [marks, setMarks] = useState<Readonly<Record<string, number>>>(() => readMarks(keep.view(key)));
  const set = useMemo(() => new Set(Object.keys(marks)), [marks]);
  const mark = useCallback(
    (file: string, on: boolean) => {
      setMarks((m) => {
        const others = Object.entries(m).filter(([k]) => k !== file);
        const next: [string, number][] = on ? [[file, Date.now()], ...others] : others;
        const kept = Object.fromEntries(next.sort((a, b) => b[1] - a[1]).slice(0, KEEP_MARKS));
        keep.setView(key, kept);
        return kept;
      });
    },
    [key],
  );
  return { set, mark };
}

function readMarks(v: unknown): Record<string, number> {
  if (typeof v !== 'object' || v === null || Array.isArray(v)) return {};
  return Object.fromEntries(Object.entries(v).filter((e): e is [string, number] => typeof e[1] === 'number'));
}

export interface SectionNav {
  /** the section at the column's top */
  readonly cur: string;
  readonly onScroll: () => void;
  /** the next (1) or previous (-1) section */
  readonly go: (d: 1 | -1) => void;
  readonly to: (key: string) => void;
}

/** Where he is among the sections: the bar says it, ‹ › and n / p move by one. */
export function useSectionNav(body: RefObject<HTMLElement | null>, sections: readonly Section[]): SectionNav {
  const first = sections[0]?.key ?? '';
  const [at, setCur] = useState(first);
  const frame = useRef(0);
  const keys = sections.map((s) => s.key).join(' ');
  // another column (the guide came, a mode): the first section until he moves
  const cur = keys.split(' ').includes(at) ? at : first;
  const onScroll = useCallback(() => {
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => {
      const el = body.current;
      if (el === null) return;
      const line = el.getBoundingClientRect().top + (el.querySelector('.rv-bar')?.getBoundingClientRect().height ?? 0) + 8;
      const heads = [...el.querySelectorAll<HTMLElement>('.rv-part[data-section]')];
      const at = heads.filter((h) => h.getBoundingClientRect().top <= line).pop() ?? heads[0];
      const k = at?.dataset['section'];
      if (k !== undefined) setCur(k);
    });
  }, [body]);
  const to = useCallback(
    (key: string) => {
      body.current?.querySelector<HTMLElement>(`.rv-part[data-section="${CSS.escape(key)}"]`)?.scrollIntoView({ block: 'start' });
      setCur(key);
    },
    [body],
  );
  const go = useCallback(
    (d: 1 | -1) => {
      const ks = keys.split(' ');
      const next = ks[Math.max(0, Math.min(ks.length - 1, ks.indexOf(cur) + d))];
      if (next !== undefined && next !== '') to(next);
    },
    [keys, cur, to],
  );
  return { cur, onScroll, go, to };
}
