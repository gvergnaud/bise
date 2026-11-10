// The app's own update in one line at the top of the window (bar S.6, amb-mac's updater 7140f8e4,
// window.bise.update). 'ready': 'bise <version> is ready · restart', his click swaps the app and
// reopens it on the screen he is on (it also installs at quit or at a quiet moment, main's
// update/run.ts, after asking this window its screen and whether a send is on its way); 'went_bad': the new app
// didn't start right, 'go back' to the previous one. Downloading and failed say nothing to him
// (failed's why is for the log). HubUpdateLine: bise's own /update build (the hub's update event).
// Owner: amb-win.
import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import type { BiseApi, UpdateState } from '@/shared/api';
import type { BiseUpdate, UpdateEv } from '@/proto';
import { settings } from '@/proto/words.gen';
import { useBiseUpdate, useHubUpdate, useProjects, useStore } from '@/store';
import { ShaWords } from './ReloadLine';
import { useRoute } from './router';
import { formatRoute } from './route';
import { RANK, type StatusPart } from './status';
import { shortVersion } from '@/shared/version';

/** The line's words and its one action, or none. Pure. */
export function updateLine(s: UpdateState): { text: string; act: 'restart' | 'go back' } | null {
  switch (s.state) {
    case 'ready':
      // it installs by itself now (desktop-dist v7, designer m_25362/m_25699): the click is
      // 'restart now'; when it installs is the popover's line (the pill never cuts inside a word)
      return { text: `bise ${shortVersion(s.version)} is ready`, act: 'restart' };
    case 'went_bad':
      return { text: "the new bise didn't start right", act: 'go back' };
    default:
      return null;
  }
}

const bise = (): BiseApi | undefined => (window as { bise?: BiseApi }).bise;

function useUpdate(): UpdateState {
  const [s, setS] = useState<UpdateState>({ state: 'none' });
  useEffect(() => {
    const u = bise()?.update;
    if (!u) return;
    let live = true;
    const off = u.onState((next) => {
      live = false;
      setS(next);
    });
    void u.now().then((now) => {
      if (live) setS(now);
    });
    return off;
  }, []);
  return s;
}

/** The app update's candidate for the one status line: 'ready' stays until his click, went_bad is a failure. */
export function useUpdateLine(): StatusPart | null {
  const s = useUpdate();
  const route = useRoute();
  const store = useStore();
  // a quiet moment's install asks first (main's update/run.ts): the screen to reopen, and whether
  // a message of his is on its way (it waits for it)
  const routeNow = useRef(route);
  useEffect(() => {
    routeNow.current = route;
  }, [route]);
  useEffect(() => bise()?.update.onAsk(() => ({ route: formatRoute(routeNow.current), sending: store.sending() })), [store]);
  const [busy, setBusy] = useState(false);
  const line = updateLine(s);
  if (line === null) return null;
  const go = () => {
    const u = bise()?.update;
    if (!u || busy) return;
    setBusy(true);
    const done = line.act === 'restart' ? u.install(formatRoute(route)) : u.revert();
    void done.then((ok) => {
      if (!ok) setBusy(false);
    });
  };
  // went_bad's words in the error tone with '▲', never pink; its one action in the pill (amb-home's
  // TitleStatus draws it, designer m_21067); it wins over core down (status.ts RANK.back)
  const bad = s.state === 'went_bad';
  return {
    rank: bad ? RANK.back : RANK.ready,
    testId: 'app.update',
    state: s.state,
    tone: bad ? 'err' : 'quiet',
    words: bad ? `▲ ${line.text}` : line.text,
    short: bad ? "▲ bise didn't start right" : 'update ready',
    ...(bad
      ? {}
      : {
          more: (
            <div className="app-core-last" data-testid="app.update.when">
              installs by itself when you quit or step away
            </div>
          ),
        }),
    acts: [
      {
        id: line.act === 'restart' ? 'restart' : 'back',
        text: busy ? (line.act === 'restart' ? 'restarting…' : 'going back…') : line.act === 'restart' ? 'restart now' : line.act,
        onClick: go,
        disabled: busy,
      },
    ],
  };
}

/**
 * /update's build in bise's source tree (the hub's update, amb-feed m_16785), on bise's home: while
 * it builds 'updating bise · 1152b33 · 2 min'; failed: the hub's words with the build's last lines
 * dim under them; built: nothing here (the switch says the rest in main's thread). Pure.
 */
export function hubUpdateLine(u: UpdateEv | null): { text: string; tail: readonly string[]; failed: boolean } | null {
  if (u === null) return null;
  if (u.state === 'building') {
    const s = u.elapsed ?? 0;
    const took = s >= 60 ? ` · ${String(Math.floor(s / 60))} min` : s > 0 ? ` · ${String(s)} s` : '';
    return { text: `updating bise · ${u.rev}${took}`, tail: [], failed: false };
  }
  if (u.state === 'failed') return { text: `▲ ${u.text ?? `couldn't build ${u.rev}`}`, tail: (u.tail ?? []).slice(-3), failed: true };
  return null;
}

/** The window's one status line draws this (StatusLine.tsx, amb-home). */
/** /update's build as a candidate for the one status pill: building runs, failed is a failure (the build's last lines in its popover). */
export function useHubUpdateLine(): StatusPart | null {
  const home = useProjects().find((p) => p.home)?.project ?? '';
  const u = useHubUpdate(home);
  const line = hubUpdateLine(u);
  if (line === null || u === null) return null;
  return {
    rank: line.failed ? RANK.failed : RANK.running,
    testId: 'app.hubupdate',
    state: line.failed ? 'failed' : 'building',
    tone: line.failed ? 'err' : 'running',
    words: <ShaWords text={line.text} shas={[u.rev]} />,
    short: line.failed ? "▲ couldn't update" : 'updating…',
    ...(line.tail.length > 0
      ? {
          more: (
            <div className="app-core-last" data-testid="app.hubupdate.tail">
              {line.tail.join('\n')}
            </div>
          ),
        }
      : {}),
  };
}

/**
 * update-flow (designer m_45087), update-dot (Gabriel m_46040, designer m_46058): a newer bise for
 * the machine (the core's fold of every project's hub/bise_update) in the one status pill once it
 * runs; ready, it is the inbox's update card (inbox/Release.tsx), never the pill. Pure: the pill's
 * words, its action and its popover lines; `done` the version it just reached (4 s of '✓ updated
 * to …').
 */
export function biseUpdateLine(
  u: BiseUpdate | null,
  done: string | null,
): { tone: StatusPart['tone']; state: string; words: string; short: string; act: string | null; actCard?: string; lines: readonly string[]; copy?: string } | null {
  if (u === null || u.state === 'unknown') {
    if (done === null) return null;
    const words = `✓ ${settings.bise_updated({ version: done })}`;
    return { tone: 'done', state: 'done', words, short: words, act: null, lines: [] };
  }
  const version = u.version;
  switch (u.state) {
    case 'ready':
      // the inbox's update card asks (designer m_46058)
      return null;
    case 'installing':
    case 'updating': {
      const left = u.waiting?.length ?? 0;
      return {
        tone: 'running',
        state: 'updating',
        words: settings.bise_updating({ version }),
        short: 'updating…',
        act: null,
        lines: left > 0 ? [u.waiting?.join(', ') ?? '', settings.bise_updating_left({ n: left })] : [],
      };
    }
    case 'failed':
      return {
        tone: 'err',
        state: 'failed',
        words: `▲ ${settings.bise_update_failed({ version })}`,
        short: "▲ couldn't update",
        act: settings.bise_update_again,
        actCard: settings.bise_update_again_card,
        lines: [...(u.why ? [u.why] : []), settings.bise_update_unchanged({ running: u.running })],
        // his report to send us (designer m_59048): the quiet act after the main one, card only
        copy: settings.bise_update_copy,
      };
  }
}

/**
 * The failure's words split (designer m_59048): the head as it reads ('the hub stopped (crash):')
 * and the new version's own raw words after it, drawn mono text-3 on one line, whole in its tip;
 * no raw part: the whole reason is the head. Pure.
 */
export function causeParts(why: string): { head: string; tail: string } {
  const m = /^(the hub stopped \(crash\)(?: after)?):\s*(\S.*)$/su.exec(why);
  return m === null ? { head: why, tail: '' } : { head: `${m[1] ?? ''}:`, tail: m[2] ?? '' };
}

/**
 * What 'Copy report' copies (designer m_59048), plain text ready to paste to bise or into an issue:
 * 'bise <from> → <to> couldn't start', the cause, the new version's last log lines, the macOS
 * version. Pure.
 */
export function updateReport(u: BiseUpdate, macos: string): string {
  const log = u.log ?? [];
  return [settings.bise_update_report_head({ from: u.running, to: u.version }), ...(u.why ? [u.why] : []), ...(log.length > 0 ? ['', ...log] : []), '', `macOS ${macos}`].join('\n');
}

/** How long the copy act says 'Copied' (designer m_59191). */
const COPIED_MS = 1500;

/** How long '✓ updated to …' stays (designer m_45087). */
const DONE_MS = 4000;

/** He answered '1 update now' on the update card in this window (inbox/Block.tsx): the app's own
 *  update goes with the machine's once its line moves past ready. Cleared when it ends. */
let askedNow = false;
const askedSubs = new Set<() => void>();
const setAskedNow = (v: boolean): void => {
  if (askedNow === v) return;
  askedNow = v;
  for (const f of askedSubs) f();
};
export const askedBiseUpdate = (): void => {
  setAskedNow(true);
};
function useAskedNow(): boolean {
  return useSyncExternalStore(
    (f) => {
      askedSubs.add(f);
      return () => {
        askedSubs.delete(f);
      };
    },
    () => askedNow,
  );
}

/**
 * The bise update as a candidate for the one status pill, once it runs (his '1' on the update
 * card: one hub installs, the machine's flock dedupes, every project switches in turn); once the
 * core's line moves past 'ready', the app's own update, when one is ready, installs and reopens the
 * window on the screen he is on. Failed: 'try again' asks the core again.
 */
export function useBiseUpdateLine(): StatusPart | null {
  const u = useBiseUpdate();
  const store = useStore();
  const app = useUpdate();
  const route = useRoute();
  const asked = useAskedNow();
  const setAsked = setAskedNow;
  const [done, setDone] = useState<string | null>(null);
  const was = useRef<BiseUpdate | null>(null);
  // updating then gone: '✓ updated to …' for 4 s
  useEffect(() => {
    const before = was.current;
    was.current = u;
    if (u === null && before !== null && before.state !== 'ready' && before.state !== 'failed') {
      setDone(before.version);
      setAsked(false);
      const t = window.setTimeout(() => {
        setDone(null);
      }, DONE_MS);
      return () => {
        window.clearTimeout(t);
      };
    }
    if (u?.state === 'failed') setAsked(false);
    return undefined;
  }, [u, setAsked]);
  // the core took it (its line left 'ready'): the app's own update goes with it
  const moved = u !== null && (u.state === 'installing' || u.state === 'updating');
  const appReady = app.state === 'ready';
  const installed = useRef(false);
  useEffect(() => {
    if (!asked || !moved || !appReady || installed.current) return;
    installed.current = true;
    void bise()?.update.install(formatRoute(route));
  }, [asked, moved, appReady, route]);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return undefined;
    const t = window.setTimeout(() => {
      setCopied(false);
    }, COPIED_MS);
    return () => {
      window.clearTimeout(t);
    };
  }, [copied]);
  const line = biseUpdateLine(u, done);
  if (line === null) return null;
  const copy = () => {
    const api = bise();
    if (u === null || api === undefined) return;
    void api.copy(updateReport(u, api.osVersion)).then(() => {
      setCopied(true);
    });
  };
  const cause = line.state === 'failed' && u?.why ? causeParts(u.why) : null;
  const go = () => {
    setAsked(true);
    store.command({ cmd: 'bise_update' });
  };
  return {
    rank: line.tone === 'err' ? RANK.failed : line.tone === 'running' ? RANK.running : line.tone === 'done' ? RANK.done : RANK.ready,
    testId: 'app.bise',
    state: line.state,
    tone: line.tone,
    // an offer he can ignore (designer m_45423): the neutral pill, its failure's ▲ alone in --c-err
    calm: true,
    words:
      line.tone === 'err' ? (
        <>
          <span className="app-ts-err">▲</span> {line.words.replace(/^▲\s*/u, '')}
        </>
      ) : (
        line.words
      ),
    short: line.short,
    ...(line.lines.length > 0
      ? {
          more:
            // a failure: bise's sentences in sans text-2, only the new version's raw tail mono
            // text-3 on one line (designer m_59191)
            line.state === 'failed' && cause !== null ? (
              <div className="app-bise-why" data-testid="app.bise.tip">
                <div>{cause.head}</div>
                {cause.tail !== '' && (
                  <div className="app-bise-cause" data-testid="app.bise.cause" title={cause.tail}>
                    {cause.tail}
                  </div>
                )}
                {line.lines.slice(1).map((l) => (
                  <div key={l}>{l}</div>
                ))}
              </div>
            ) : (
              <div className="app-core-last" data-testid="app.bise.tip">
                {line.lines.join('\n')}
              </div>
            ),
        }
      : {}),
    ...(line.act === null
      ? {}
      : {
          acts: [
            { id: 'update', text: line.act, ...(line.actCard === undefined ? {} : { cardText: line.actCard }), onClick: go },
            ...(line.copy === undefined ? [] : [{ id: 'copy', text: copied ? settings.bise_update_copied : line.copy, onClick: copy, quiet: true }]),
          ],
        }),
  };
}
