// Does the open artifact's page answer (page-unreach, Benjamin's blank panel)? A page lives on its
// project's pages server, inside that project's hub: a hub that stopped (idle-exit), is starting or
// didn't start left the frame on Chromium's refused page, blank in the panel, never retried. So the
// panel asks main first (bise.probePage, main/probe.ts) and frames the page only once it answers;
// meanwhile it shows a loading state (LOAD_GRACE_MS: a server still starting refuses the first
// asks, never an error for that, artifact-loading), then says why, and asks again every REPROBE_MS: opening the artifact shows its project,
// which makes the core start that project's hub (architect m_38135), so the page usually comes
// back by itself. The words: downWords (pure). Owner: page-unreach (the artifact view, Open.tsx).
import { useCallback, useEffect, useRef, useState } from 'react';

/** How often a page that doesn't answer is asked again while the panel shows it. */
export const REPROBE_MS = 2000;

/** 'gone': the server answered 404, the artifact's file is gone (designer m_52746: its own state, Open.tsx FileGone) */
export type Reach = 'checking' | 'up' | 'down' | 'gone';

type ProbeBridge = { probePage?: (url: string) => Promise<'up' | 'down' | 'gone'> };

/** Pure: is `src` a url of the local pages server (the one kind we probe)? */
export function probed(src: string | null): src is string {
  return src !== null && /^http:\/\/127\.0\.0\.1:\d+\//u.test(src);
}

/** Pure: the port a pages-server url names ('47415'), else ''. */
export function portOf(src: string): string {
  return /^http:\/\/127\.0\.0\.1:(\d+)\//u.exec(src)?.[1] ?? '';
}

/** What the project's row says of its hub (the sidebar's row). */
export interface HubRow {
  readonly name: string;
  readonly running: boolean;
  /** its hub couldn't start, in plain words */
  readonly down?: string | null;
}

/** A piece of the panel's line: words, or mono text (an address, a path; `reveal`: shown in Finder on a click). */
export type Part = { readonly t: string } | { readonly mono: string; readonly reveal?: string };

/**
 * The panel's words when the page doesn't answer (designer m_38170): its title, its line, and
 * whether a hub is starting (the working mark '∿' before the title, no 'try again': it asks
 * again by itself).
 */
export interface DownWords {
  readonly title: string;
  readonly parts: readonly Part[];
  readonly starting: boolean;
}

/** A path as he reads it: ~/… under his home. Pure. */
const tildeOf = (p: string, home: string | undefined): string => (home !== undefined && home !== '' && p.startsWith(`${home}/`) ? `~${p.slice(home.length)}` : p);

/**
 * Pure: the row's words of a hub that didn't start ('the hub did not start: <cause> (its log:
 * <path>)', client.rs not_started) without their 'did not start' (the title says it once), the
 * log as a path he can open.
 */
export function hubDownParts(down: string, home?: string): Part[] {
  const m = /^(.*?)\s*\(its log: ([^)]+)\)\.?$/u.exec(down.trim());
  const cause = (m?.[1] ?? down)
    .trim()
    .replace(/^the hub did not start: /u, '')
    .replace(/^the hub did not start in (\d+) s$/u, 'no answer in $1 s')
    .replace(/\.$/u, '');
  const end: Part = { t: '. its page shows here once the hub runs.' };
  const log = m?.[2];
  if (log === undefined) return [{ t: cause }, end];
  return [{ t: `${cause}. its log: ` }, { mono: tildeOf(log, home), reveal: log }, end];
}

/**
 * Pure: why the page doesn't answer, as the project's row tells it. Its hub not running: bise is
 * starting it (opening the artifact holds its project). Its hub didn't start: its words. Its hub
 * runs and still nothing answers: the address, plainly.
 */
export function downWords(row: HubRow | undefined, src: string, home?: string): DownWords {
  const name = row?.name ?? 'its project';
  const down = row?.down ?? '';
  if (down !== '') return { title: `${name}'s hub didn't start`, parts: hubDownParts(down, home), starting: false };
  if (row !== undefined && !row.running) {
    return { title: `∿ starting ${name}'s hub…`, parts: [{ t: `${name}'s hub had stopped, and its pages with it. this one shows here as soon as the hub is up.` }], starting: true };
  }
  return {
    title: "this page doesn't answer",
    parts: [{ t: 'nothing answers at ' }, { mono: `127.0.0.1:${portOf(src)}` }, { t: `, where ${name}'s hub serves its pages. try again; if it stays like this, quit and reopen bise.` }],
    starting: false,
  };
}

// the loading grace's numbers (grace.ts: the views store reads them too, with no React)
import { LOAD_GRACE_MS, LOAD_RETRY_MS } from './grace';
export { LOAD_GRACE_MS, LOAD_RETRY_MS };

/**
 * The page's reach, for `src`: 'up' at once for anything not on the pages server (or with no
 * bridge: a plain browser, a test without one); else 'checking', then what the probe says. A
 * 'down' inside LOAD_GRACE_MS of the open (or of his 'try again') stays 'checking', asked again
 * every LOAD_RETRY_MS; past it, 'down', asked again every REPROBE_MS. 'up' and 'gone' are real
 * answers: at once. `again()`: ask now, loading again for a new LOAD_GRACE_MS (his 'try again').
 */
export function useReach(src: string | null): { reach: Reach; again: () => void } {
  const probe = (globalThis as { bise?: ProbeBridge }).bise?.probePage;
  const asks = probed(src) && probe !== undefined;
  const [state, setState] = useState<{ src: string | null; reach: Reach }>({ src, reach: asks ? 'checking' : 'up' });
  const [tick, setTick] = useState(0);
  // each open of `src` and each 'try again' starts a grace; `late`: the one that ran out
  const [epoch, setEpoch] = useState(0);
  const [late, setLate] = useState<{ src: string | null; epoch: number } | null>(null);
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);
  useEffect(() => {
    if (!asks) return;
    let gone = false;
    void probe(src).then((r) => {
      if (gone || !live.current) return;
      setState((s) => (s.src === src && s.reach === r ? s : { src, reach: r }));
    });
    return () => {
      gone = true;
    };
  }, [asks, probe, src, tick]);
  useEffect(() => {
    if (!asks) return;
    const t = setTimeout(() => {
      setLate({ src, epoch });
    }, LOAD_GRACE_MS);
    return () => {
      clearTimeout(t);
    };
  }, [asks, src, epoch]);
  const asked: Reach = state.src === src ? state.reach : 'checking';
  const loading = late?.src !== src || late.epoch !== epoch;
  const reach: Reach = !asks ? 'up' : asked === 'down' && loading ? 'checking' : asked;
  useEffect(() => {
    if (asked !== 'down') return;
    const t = setTimeout(() => {
      setTick((n) => n + 1);
    }, loading ? LOAD_RETRY_MS : REPROBE_MS);
    return () => {
      clearTimeout(t);
    };
  }, [asked, loading, tick]);
  const again = useCallback(() => {
    setEpoch((n) => n + 1);
    setTick((n) => n + 1);
  }, []);
  return { reach, again };
}
