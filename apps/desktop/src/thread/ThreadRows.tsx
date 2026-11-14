// The thread's list: its rows (entries, his sends not landed yet, the live reply), the folded
// runs of messages between agents, and the first-run block over an empty main thread. Split out
// of ThreadScreen.tsx (the file cap); ThreadScreen mounts it. Owner: amb-win.
import { openWebLink } from '@/app/panel/web';
import { webHistoryNow } from '@/app/panel/webpages';
import { memo, useCallback, useContext, useEffect, useMemo, useRef, useState, useSyncExternalStore, type RefObject } from 'react';
import { Press } from '@/ds';
import type { ComposerActions } from '@/composer';
import { useAgents, useEntry, useProjects, usePending, useStore, useThreadOrder } from '@/store';
import { usePartialText } from '@/shared/partial';
import { LIVE, liveKey, useLive, useStill } from './live';
import { FirstRun, showsFirstRun } from '@/setup';
import { ThreadList, type ThreadListHandle } from './ThreadList';
import { EntryRow, PendingRow, gapOf, type RowCtx } from './rows';
import { entryKey, namedRows, pendingKey, placeSteered, posOfKey, sendInThread } from './entry';
import { useHoldsOf } from './routes';
import { NoProject } from './NoProject';
import { landedEchoes, retryRuns } from './tiers';
import { foldedRuns, runKey, runWords, withRuns } from './runs';
import { turnLines, turnsOf, withTurns, type Turn } from './turns';
import { TurnLine } from './TurnLine';
import { absolute } from './paths';
import { FoldsContext } from './folds';
import { FindMarkContext } from './findmark';
import { GapRow, OlderLine, isGapKey, withGap } from './gap';
import { SyncCue } from './syncCue';
import { useArtifactOpener } from './openArtifact';
import { ArtPeekContext, type PeekSource } from './peek/ArtPeek';
import { IntroRow } from '@/welcome/Welcome';
import { introShows } from '@/welcome/rule';
import { MAIN } from '@/app/route';
import { ViewCards } from '@/views/Card';

/** the lead's intro row's key (never an entry's: entry keys are numbers') */
const INTRO = 'intro';

/** The agent's folder: its worktree, else its project's checkout; none until the rows come. */
function useBase(project: string, agent: string): string | undefined {
  const a = useAgents(project).find((x) => x.name === agent);
  const row = useProjects().find((p) => p.project === project);
  return a?.worktree ?? row?.path;
}

/** A link from an answer: a web tab of bise, his browser with ⌘ or for a site he always opens there (designer m_64680). */
function openLink(url: string, mod = false) {
  openWebLink(url, mod, webHistoryNow().out);
}

/**
 * The first-run text (bar L1 / M18, amb-kit's FirstRun) in main's thread
 * while he has said nothing there and main is the project's only agent: its
 * "show me what you can do" puts the words in the composer, selected (never
 * a send); then its last line says ⏎ tries it. Gone once he said something.
 */
export function FirstRunSlot({ tkey, project, composer, bise }: { tkey: string; project: string; composer: RefObject<ComposerActions | null>; bise: boolean }) {
  const store = useStore();
  const rows = useProjects();
  // bise's own thread before any project: designer's 'no project yet' centre (m_12093), demo only
  const noProject = bise && rows.length > 0 && !rows.some((p) => !p.home);
  const view = useThreadOrder(tkey);
  const pending = usePending(tkey);
  const live = useAgents(project).filter((a) => !a.archived).length;
  const [ready, setReady] = useState(false);
  // his words in the thread: an entry of his, or a send of his on its way
  const said = pending.length > 0 || view.order.some((pos) => store.entry(tkey, pos)?.kind === 'you');
  if (!view.loaded || !showsFirstRun(said, live)) return null;
  // over an empty list: centred in the thread's column, both ways; under entries of main's: in the
  // column, at the list's end
  const centred = view.order.length === 0;
  return (
    <div className={centred ? 'th-first' : 'th-first after'} data-testid="thread.first">
      {noProject ? (
        <NoProject
          onDemo={(text) => {
            composer.current?.fill(text);
          }}
        />
      ) : (
        <FirstRun
          ready={ready}
          onFill={(text) => {
            composer.current?.fill(text);
            setReady(true);
          }}
        />
      )}
    </div>
  );
}

export const Rows = memo(function Rows({ tkey, project, agent, bise, listRef, onReading }: { tkey: string; project: string; agent: string; bise: boolean; listRef: RefObject<ThreadListHandle | null>; onReading?: (reading: boolean) => void }) {
  const store = useStore();
  const view = useThreadOrder(tkey);
  const pending = usePending(tkey);
  // entries only: his sends not landed yet are the list's tail, outside the
  // virtual list, so his row is drawn in ⏎'s own frame (bar ⛔3: < 100 ms)
  // R13: more than 3 agent-to-agent messages in a row fold into '▸ 4 messages between 2 agents'
  // ctrl+o / ⌘E open and close every fold, the runs of messages between agents too (designer's
  // desktop-notices (c)): a press (its version) sets them all; a click still opens one in place
  const folds = useContext(FoldsContext);
  const foldAll = useSyncExternalStore(
    (l) => folds.subscribe(l),
    () => folds.get(),
  );
  const [runsOpen, setRunsOpen] = useState<{ open: ReadonlySet<number>; v: number }>(() => ({ open: new Set(), v: foldAll.v }));
  const openRuns = runsOpen.open;
  const setOpenRuns = useCallback((f: (s: ReadonlySet<number>) => ReadonlySet<number>) => {
    setRunsOpen((x) => ({ open: f(x.open), v: x.v }));
  }, []);
  // a steer shows where the agent read it (its receipt's pos), not where it was written
  // (the store bumps the order when a steered_at comes, so this recomputes)
  // one land per sha (designer m_19387): the hub's line that says again a typed land leaves the list
  const placed = useMemo(() => {
    const echoes = landedEchoes(view.order, (pos) => store.entry(tkey, pos));
    const order = echoes.size === 0 ? view.order : view.order.filter((pos) => !echoes.has(pos));
    return placeSteered(order, (pos) => store.entry(tkey, pos)?.steered_at);
  }, [view, store, tkey]);
  // H9 (designer m_19832): a model call's retries, one row that updates in place; the tries before
  // the last leave the list, their words open under its ▸
  const retries = useMemo(() => retryRuns(placed, (pos) => store.entry(tkey, pos)), [placed, store, tkey]);
  const shown = useMemo(() => (retries.hidden.size === 0 ? placed : placed.filter((pos) => !retries.hidden.has(pos))), [placed, retries]);
  const runs = useMemo(() => foldedRuns(shown, (pos) => store.entry(tkey, pos), agent), [shown, store, tkey, agent]);
  // each turn that did work: 'worked for 3m 10s ›' at its start, its thinking folded (turns.ts)
  const turns = useMemo(() => turnsOf(shown, (pos) => store.entry(tkey, pos)), [shown, store, tkey]);
  const [turnsOpen, setTurnsOpen] = useState<ReadonlySet<number>>(() => new Set());
  // a press since the last render: every run and every turn listed now opens, or every one folds
  if (runsOpen.v !== foldAll.v) {
    setRunsOpen({ open: foldAll.open ? new Set(runs.map((r) => r.start)) : new Set(), v: foldAll.v });
    setTurnsOpen(foldAll.open ? new Set(turns.map((t) => t.start)) : new Set());
  }
  // ⌘F's current match in a folded run: the run opens, so its row is there to reveal and mark
  // (find-scroll: ⏎ went to it and nothing moved, his 'ça reste au premier match')
  const mark = useContext(FindMarkContext);
  const findAt = useSyncExternalStore(
    (l) => mark.subscribe(l),
    () => mark.get(),
  );
  // once per match he goes to: closed again by his click, the fold stays closed
  const [openedFor, setOpenedFor] = useState<number | undefined>(undefined);
  const holder = findAt === undefined || findAt === openedFor ? undefined : runs.find((r) => !openRuns.has(r.start) && r.members.includes(findAt));
  if (holder !== undefined) {
    setOpenedFor(findAt);
    setOpenRuns((open) => new Set([...open, holder.start]));
  }
  // a window around a far entry (his answer weeks back, store/window.ts): the gap row between it
  // and the latest, under the window's last row
  const gap = view.gap;
  const listed = useMemo(() => withGap(withTurns(withRuns(shown, runs, openRuns, entryKey), turns, turnsOpen, entryKey), gap?.hi), [shown, runs, openRuns, turns, turnsOpen, gap?.hi]);
  // the row each turn's line sits on (under its day line): it starts right under the line's 12
  const lineOn = useMemo(() => turnLines(turns, turnsOpen), [turns, turnsOpen]);
  const toggleTurn = useCallback((t: Turn) => {
    setTurnsOpen((s) => {
      const next = new Set(s);
      if (next.has(t.start)) next.delete(t.start);
      else next.add(t.start);
      return next;
    });
  }, []);
  // the reply being written (streaming): a live row at the end, drawn as the entry it becomes,
  // which takes its place with the same key (live.ts, text-appearing v2)
  const partial = usePartialText(project, agent);
  const lastPos = view.order.at(-1) ?? -1;
  const lastKind = useEntry(tkey, lastPos)?.kind;
  const live = useLive(partial, lastPos, lastKind);
  const still = useStill();
  // a project's main: the lead's intro is its first row once he wrote (welcome/Welcome.tsx IntroRow)
  const hasYou = !bise && agent === MAIN && view.order.some((pos) => store.entry(tkey, pos)?.kind === 'you');
  const intro = !bise && agent === MAIN && introShows(view, hasYou, pending.length);
  const keys = useMemo(() => {
    const withLive = live.entry === null ? listed : [...listed, LIVE];
    return intro ? [INTRO, ...withLive] : withLive;
  }, [listed, live.entry, intro]);
  const liveCall = live.call;
  const handed = live.handed;
  const reactKey = useCallback(
    (key: string) => {
      if (key === LIVE) return liveCall === null ? key : liveKey(liveCall);
      const pos = posOfKey(key);
      return (pos === undefined ? undefined : handed.get(pos)) ?? key;
    },
    [liveCall, handed],
  );
  // relative paths in its answers are under the agent's folder: its worktree, else the project's
  const base = useBase(project, agent);
  const goArtifact = useArtifactOpener();
  const ctx = useMemo<RowCtx>(
    () => ({
      tkey,
      project,
      agent,
      bise,
      onLink: openLink,
      onArtifact: (id: string) => {
        goArtifact(project, id);
      },
      ...(base === undefined
        ? {}
        : {
            onPath: (path: string, line?: number) => {
              void (window as Partial<Window>).bise?.openPath(absolute(path, base), line === undefined ? {} : { line });
            },
          }),
    }),
    [tkey, project, agent, bise, base, goArtifact],
  );
  const peek = useMemo<PeekSource>(
    () => ({
      project,
      open: (id: string) => {
        goArtifact(project, id);
      },
    }),
    [project, goArtifact],
  );
  // each row reads the one before it: its group and its day (ambient m_11777)
  const prevOf = useMemo(() => new Map(shown.map((p, i) => [p, shown[i - 1]])), [shown]);
  // and the one after it: his last bubble of a run shows its time (messages-right v1)
  const nextOf = useMemo(() => new Map(shown.map((p, i) => [p, shown[i + 1]])), [shown]);
  // H2: one name line per author run, never on a tool row (entry.ts namedRows)
  const heads = useMemo(() => namedRows(shown, (pos) => store.entry(tkey, pos), { agent, bise }), [shown, store, tkey, agent, bise]);
  // H1: the row under a folded run follows the fold's quiet row
  const afterFold = useMemo(() => {
    const out = new Set<number>();
    for (const r of runs) {
      const last = r.members.at(-1);
      const after = openRuns.has(r.start) || last === undefined ? undefined : nextOf.get(last);
      if (after !== undefined) out.add(after);
    }
    return out;
  }, [runs, openRuns, nextOf]);
  // the first message of an open run sits 8 px under its fold row (designer m_19912 (2))
  const underFold = useMemo(() => new Set(runs.filter((r) => openRuns.has(r.start)).map((r) => r.start)), [runs, openRuns]);
  // H1: the fold is a quiet row: 4 px under another, 12 under a message or prose
  const foldGap = useCallback(
    (start: number) => {
      const before = prevOf.get(start);
      const p = before === undefined ? undefined : store.entry(tkey, before);
      return p === undefined ? undefined : gapOf(p, { kind: 'fold' });
    },
    [prevOf, store, tkey],
  );
  const row = useCallback(
    (key: string) => {
      const run = key.startsWith('r:') ? runs.find((r) => runKey(r.start) === key) : undefined;
      if (run !== undefined) {
        const open = openRuns.has(run.start);
        return (
          <Press
            look="th-run"
            aria-expanded={open}
            onClick={() => {
              setOpenRuns((s) => {
                const next = new Set(s);
                if (next.has(run.start)) next.delete(run.start);
                else next.add(run.start);
                return next;
              });
            }}
            data-testid={`thread.run.${String(run.start)}`}
            data-gap={foldGap(run.start)}
          >
            {runWords(run, open)}
          </Press>
        );
      }
      if (isGapKey(key)) return <GapRow project={project} agent={agent} />;
      if (key === INTRO) return <IntroRow project={project} />;
      if (key === LIVE) return live.entry === null ? null : <EntryRow ctx={ctx} pos={-1} prev={lastPos < 0 ? undefined : lastPos} live={live.entry} still={still} carry={heads.carry} />;
      const pos = posOfKey(key);
      const turn = pos === undefined ? undefined : lineOn.get(pos);
      const turnLine =
        turn === undefined ? undefined : (
          <TurnLine
            turn={turn}
            open={turnsOpen.has(turn.start)}
            onToggle={() => {
              toggleTurn(turn);
            }}
          />
        );
      return pos === undefined ? null : (
        <>
          <EntryRow ctx={ctx} pos={pos} prev={prevOf.get(pos)} next={nextOf.get(pos)} still={still} named={heads.named.has(pos)} afterFold={afterFold.has(pos)} underFold={underFold.has(pos)} turnLine={turnLine} retries={retries.runs.get(pos)} />
          {/* MCP Apps: a tool call's view card under its row (src/views, mcp-apps-host) */}
          <ViewCards tkey={tkey} pos={pos} project={project} agent={agent} />
        </>
      );
    },
    [ctx, prevOf, nextOf, runs, openRuns, setOpenRuns, turnsOpen, lineOn, toggleTurn, live.entry, lastPos, still, heads, afterFold, underFold, foldGap, retries, project, agent, tkey],
  );
  const onOlder = useCallback(() => {
    store.olderPage(project, agent);
  }, [store, project, agent]);
  // scrolled up: the store keeps the entries near the one he reads (trim.ts), the far ones are paged back
  const onRead = useCallback(
    (key: string) => {
      // an entry's row, or a folded run's (its first entry)
      const pos = posOfKey(key) ?? (key.startsWith('r:') ? Number(key.slice(2)) : undefined);
      if (pos !== undefined) store.reading(project, agent, pos);
    },
    [store, project, agent],
  );
  // the row his sends follow: the reply being written, else the list's last entry (rowLayout)
  const lastShown = useEntry(tkey, shown.at(-1) ?? -1);
  const tailPrev = live.entry ?? lastShown;
  // his messages on their way; a queued one is the composer's drawer (QueueDrawer.tsx) until the
  // hub says it, never a bubble here: then its entry lands as his normal bubble, at its place
  const holds = useHoldsOf(store, tkey);
  const going = useMemo(() => pending.filter((p) => sendInThread(p, holds.some((h) => h.cid === p.cid))), [pending, holds]);
  // his own send goes to the bottom even when he had scrolled up
  const sent = going.length;
  const last = useRef(sent);
  useEffect(() => {
    if (sent > last.current) listRef.current?.scrollToEnd();
    last.current = sent;
  }, [sent, listRef]);
  const tail = going.length === 0 ? undefined : (
    <div className="th-tail">
      {going.map((p, i) => (
        <PendingRow key={pendingKey(String(p.cid))} ctx={ctx} cid={p.cid} after={i === 0 ? tailPrev : undefined} />
      ))}
    </div>
  );
  // his thread's newest page on its way: the cue under its last row (thread-loading, syncCue.tsx)
  const cue = <SyncCue tkey={tkey} />;
  if (!view.loaded && keys.length === 0 && tail === undefined)
    return (
      <div className="th-list th-loading" data-testid="thread.loading">
        {cue}
      </div>
    );
  // one list per thread (key): another agent's thread is a new mount, so it opens at its end or
  // where he left it (keep.ts), never with the last thread's scroll, stick or opened rows
  // its artifact pills show their preview on hover (peek/ArtPeek)
  return (
    <ArtPeekContext.Provider value={peek}>
      <ThreadList key={tkey} ref={listRef} keys={keys} row={row} more={view.more} before={view.before} onOlder={onOlder} head={<OlderLine />} tail={tail} cue={cue} testId="thread.list" keepKey={tkey} holdPart=".th-row" reactKey={reactKey} onRead={onRead} {...(onReading === undefined ? {} : { onReading })} />
    </ArtPeekContext.Provider>
  );
});
