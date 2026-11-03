// The processes screen (bise fait mac: page native-mac §6 and §8; designer's running-procs; route
// running): what his agents left running in every project. The page's name is in the window's bar;
// under it the banner (ds PageBanner), the list's row (ds ListBar, his m_52658: the filter, ds Filters
// 'running N' / 'ended today N', and the search at its right; '/' and ⌘F focus it, app/searchkeys.ts;
// it stays over the list, which scrolls in its own box: its line once that box scrolled), a ds GroupLabel per project, one ds ListRow per process (its
// command in mono 12, then agent · address · age; a --c-ok dot while it runs; the chosen one's
// 'open'; its '…' and a right click are its menu, useEntityMenu), and the selected one's output
// beside it (head: meta, find ⌘F, open ↗ / stop; a read-only terminal that follows its end; no
// key line at the foot, the keys in the tips). The floating composer's room at the list's foot. While shown the core asks every running hub for its list every 2 s
// (core cmd processes_all, architect m_38378). Stop asks nothing: 'stopping…', then its hub's
// row says how it ended. A row that ends while the screen is open stays where it was until he
// leaves or switches the filter (designer m_38389). Owner: nm-pages-a (procs-desktop's before).
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { nextHit, type Hit } from './find';
import type { Terminal as Xterm } from '@xterm/xterm';
import { AppIcon, Empty, Field, Filters, GroupLabel, ListBar, ListRow, ListSearch, MoreButton, PageBanner, Press, RowAction, sentenceCase, tipAttrs } from '@/ds';
import { PageVignette, useBannerGutter } from '@/artifacts/hero/vignettes';
import { useEntityMenu } from '@/app/menus/use';
import { Code } from '@/thread/Code';
import { hero } from '@/proto/words_hero.gen';
import { useListKeys } from '@/app/listkeys';
import { usePageSearchKeys } from '@/app/searchkeys';
import { useKeptScroll } from '@/app/keep';
import { pinFirst, pinnedIn, procPinId, usePins, type PinSet } from '@/app/pins';
import { PinMark } from '@/app/pinbutton';
import { useProjects, useStore } from '@/store';
import { useBarAgents } from '@/app/baragents';
import { procRoots } from '@/app/barrows';
import { useRoute } from '@/app/router';
import { useAllProcesses } from './hooks';
import { countsOf, endWords, groupsOf, inScope, isRunning, keyOf, markOf, markTip, matches, metaOf, quietWords, paneWhere, rowCmd, stateOf, tipOf, urlWords, type Filter, type ProcItem, type Root } from './model';
import { Output } from './Output';
import { running } from '@/proto/words_running.gen';
import './running.css';

export interface RunningPorts {
  /** the list is shown: the core asks the running hubs (on), or stops (off) */
  watch: (on: boolean) => void;
  /** stop one (HubCmd proc_stop {project, agent, id}) */
  stop: (x: ProcItem) => void;
  /** open its address in his browser */
  open: (url: string) => void;
}

/** The screen as the shell mounts it: every project's processes, the hubs' commands through the store. */
export function RunningScreen() {
  const store = useStore();
  const items = useAllProcesses();
  useKeptScroll('running', () => document.querySelector<HTMLElement>('[data-testid="running.scroll"]'));
  const ports: RunningPorts = useMemo(
    () => ({
      watch: (on) => {
        store.command({ cmd: 'processes_all', on });
      },
      stop: (x) => {
        store.command({ cmd: 'proc_stop', project: x.project, agent: x.p.agent, id: x.p.id });
      },
      open: (url) => {
        void (window as Partial<Window>).bise?.openExternal(url);
      },
    }),
    [store],
  );
  const home = (window as { bise?: { home?: string } }).bise?.home || undefined;
  const route = useRoute();
  const o = route.screen === 'running' ? route.open : undefined;
  const opened = o === undefined ? undefined : `${o.project}/${o.agent}/${o.id}`;
  // the bar's 'all ›' keeps its scope: a project's, an agent's; none: every project's
  const scope = route.screen === 'running' ? route.scope : undefined;
  const shown = useMemo(() => inScope(items, scope), [items, scope]);
  const projects = useProjects();
  const agents = useBarAgents('');
  const roots = useMemo(() => procRoots(projects, agents), [projects, agents]);
  const pins = usePins('process');
  return <RunningView items={shown} ports={ports} home={home} roots={roots} opened={opened} scoped={scope !== undefined} pins={pins} />;
}

/** How often the shown screen says it again: the core lets its hold go 30 s after the last (LISTS_LEASE). */
export const RENEW_MS = 10_000;

const FILTERS: readonly Filter[] = ['running', 'ended'];

/** The screen from props (tests, shots). */
export function RunningView({
  items,
  ports,
  now: pinned,
  home,
  roots,
  opened,
  scoped = false,
  pins,
}: {
  /** what he pinned (app/pins.ts): first in its project's group, the pin mark on its row */
  pins?: PinSet;
  items: readonly ProcItem[];
  ports: RunningPorts;
  now?: number;
  home?: string | undefined;
  /** where each project's processes run (procRoots): the pane says a folder past its worktree */
  roots?: ReadonlyMap<string, readonly Root[]> | undefined;
  /** the bar's scope ('all ›' from a project's or an agent's thread): its head names no projects */
  scoped?: boolean;
  /** the process the route opens ('project/agent/id': the bar's row, a thread's backgrounded call) */
  opened?: string | undefined;
}) {
  const now = useNow(pinned);
  // shown: the core asks every running hub, said again within its lease; gone: off
  useEffect(() => {
    ports.watch(true);
    const t = setInterval(() => {
      ports.watch(true);
    }, RENEW_MS);
    return () => {
      clearInterval(t);
      ports.watch(false);
    };
  }, [ports]);
  // an ended one the route opens is under 'ended today'
  const endedOne = (k: string | undefined): boolean => {
    const x = k === undefined ? undefined : items.find((y) => keyOf(y) === k);
    return x !== undefined && !isRunning(x.p);
  };
  const [filter, setFilter] = useState<Filter>(() => (endedOne(opened) ? 'ended' : 'running'));
  const [q, setQ] = useState('');
  const searchRef = useRef<HTMLInputElement>(null);
  // the list's own scroller, under its row: the row's line shows once it scrolled
  const listBox = useRef<HTMLDivElement>(null);
  const gutter = useBannerGutter();
  // what he stopped from here, until its hub says it ended
  const [stopping, setStopping] = useState<ReadonlySet<string>>(new Set());
  // the rows seen running while 'running' is shown: one that ends stays in place (designer m_38389)
  const [kept, setKept] = useState<ReadonlySet<string>>(() => new Set());
  const fresh = filter === 'running' ? items.filter((x) => isRunning(x.p) && !kept.has(keyOf(x))).map(keyOf) : [];
  if (fresh.length > 0) setKept(new Set([...kept, ...fresh]));
  const [open, setOpen] = useState<string | null>(opened ?? null);
  // the route opens another one: its output, and its filter; a row that ends later stays where it is
  const [routed, setRouted] = useState(opened);
  if (opened !== routed) {
    setRouted(opened);
    if (opened !== undefined) {
      setOpen(opened);
      if (endedOne(opened) && !kept.has(opened)) setFilter('ended');
    }
  }
  const counts = countsOf(items, now);
  // what he pinned leads its project's group (app/pins.ts: a stable partition)
  const isPin = (x: ProcItem) => pins !== undefined && pinnedIn(pins, x.project, procPinId(x.p.agent, x.p.id));
  const groups = groupsOf(items.filter((x) => matches(x, q)), filter, now, kept).map((g) => ({ ...g, items: pinFirst(g.items, isPin) }));
  const order = groups.flatMap((g) => g.items.map(keyOf));
  const term = useRef<Xterm | null>(null);
  const onTerm = useCallback((t: Xterm | null) => {
    term.current = t;
  }, []);
  // ⌘F: the find field over the output
  const [finding, setFinding] = useState(false);

  const byKey = new Map(items.map((x) => [keyOf(x), x]));
  const stop = (x: ProcItem) => {
    if (!isRunning(x.p) || stopping.has(keyOf(x))) return;
    setStopping((s) => new Set([...s, keyOf(x)]));
    ports.stop(x);
  };
  const openUrl = (x: ProcItem) => {
    const u = urlWords(x.p, stateOf(x.p));
    if (u !== null) ports.open(u.url);
  };
  const pick = (f: Filter) => {
    setKept(new Set());
    setFilter(f);
  };
  // the TUI's /running keys: s stop, o open; esc closes the output, then clears the search
  const screenKey = (e: KeyboardEvent, k: string | null): boolean => {
    if (e.metaKey && e.key === 'f' && open !== null) {
      setFinding(true);
      return true;
    }
    if (e.metaKey && e.key === 'c') {
      const sel = term.current?.getSelection() ?? '';
      if (sel === '') return false;
      void navigator.clipboard.writeText(sel);
      return true;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return false;
    const x = byKey.get(open ?? k ?? '');
    if (e.key === 'Escape') {
      if (open !== null) setOpen(null);
      else if (q !== '') setQ('');
      else return false;
    } else if (e.key === 's' && x) stop(x);
    else if (e.key === 'o' && x) openUrl(x);
    else return false;
    return true;
  };
  const { sel, setSel, ref, onKeyDown, move } = useListKeys({ order, keepAs: 'running.sel', onOpen: setOpen, onKey: screenKey });
  // '/' and ⌘F focus the search; ⌘F with an output open is its find (the output's own)
  usePageSearchKeys(searchRef, () => {
    if (open === null) return false;
    setFinding(true);
    return true;
  });
  const searchKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const back = () => {
      e.preventDefault();
      ref.current?.focus({ preventScroll: true });
    };
    if (e.key === 'Escape') {
      if (q !== '') {
        e.preventDefault();
        setQ('');
      } else back();
    } else if (e.key === 'Enter') back();
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      back();
      move(e.key === 'ArrowDown' ? 1 : -1);
    }
  };
  const cur = open === null ? null : (byKey.get(open) ?? null);
  return (
    <section ref={ref} className={`run-screen${cur ? ' has-pane' : ''}`} data-testid="running.screen" tabIndex={-1} onKeyDown={onKeyDown}>
      {/* the banner over the whole content area, the list and the output under it (designer m_43401) */}
      <PageBanner
          title={hero.processes_title}
          sentence={hero.processes_say}
          visual={<PageVignette page="processes" />}
          icon="terminal"
          // its list is a full-width column beside the output (--pad-column): the title 32 in
          edge="page"
          gutter={gutter}
          // 'in N projects' only on every project's list: a scoped one is one place (designer m_38700)
          facts={<span data-testid="running.sub">{`${scoped ? running.count_running({ n: counts.running }) : running.running_in({ count: counts.running, n: counts.projects })} · ${running.count_listening({ n: counts.listening })}`}</span>}
          testId="running.lead"
        />
      <div className="run-body">
      <div className="run-main">
        {/* the list's row stays over the list, which scrolls in its own box under it: the row's
            1 px line once it scrolled (designer m_52731); its foot keeps the floating composer's
            room (--float-room) */}
        <ListBar inset scroller={listBox} testId="running.filters"
          end={<ListSearch ref={searchRef} value={q} onChange={(e) => { setQ(e.currentTarget.value); }} onKeyDown={searchKey} onClear={() => { setQ(''); searchRef.current?.focus(); }} testId="running.search" />}>
          <Filters
            items={FILTERS.map((f) => ({ id: f, label: <>{sentenceCase(f === 'running' ? running.state_running : running.ended_today)}<span className="run-n">{f === 'running' ? counts.running : counts.ended}</span></> }))}
            on={filter}
            label={running.filter_label}
            onPick={(id) => { pick(id === 'ended' ? 'ended' : 'running'); }}
            testId="running.filter"
          />
        </ListBar>
        <div ref={listBox} className="run-scroll" data-testid="running.scroll">
        {order.length === 0 ? (
          <Empty testId="running.empty" title={q !== '' ? running.none_match : filter === 'running' ? running.empty : running.empty_ended} />
        ) : (
          <div data-testid="running.list">
            {groups.map((g) => (
              <div key={g.key} role="group" aria-label={g.projectName} data-testid={`running.group.${g.key}`}>
                {/* one label per project; none with one project: the lead says it (designer m_41694 #2) */}
                {groups.length > 1 && <GroupLabel>{g.projectName}</GroupLabel>}
                {g.items.map((x) => {
                  const k = keyOf(x);
                  return (
                    <ProcRow key={k} x={x} k={k} on={k === sel} s={stateOf(x.p, stopping.has(k))} now={now} home={home} pinned={isPin(x)}
                      onClick={() => {
                        setSel(k);
                        setOpen(k);
                        ref.current?.focus({ preventScroll: true });
                      }}
                      onOpen={(url) => { ports.open(url); }} />
                  );
                })}
              </div>
            ))}
          </div>
        )}
        </div>
      </div>
      {cur ? (
        <Pane
          key={keyOf(cur)}
          x={cur}
          st={stateOf(cur.p, stopping.has(keyOf(cur)))}
          now={now}
          where={paneWhere(cur, roots?.get(cur.project) ?? [], home)}
          onTerm={onTerm}
          onBack={() => { setOpen(null); }}
          onStop={() => { stop(cur); }}
          onOpen={() => { openUrl(cur); }}
          onFind={() => { setFinding(true); }}
          find={finding ? <Find term={term} onDone={() => { setFinding(false); ref.current?.focus({ preventScroll: true }); }} /> : null}
        />
      ) : (
        // none picked: the pane says what it is for, never a blank (designer m_41694 #12)
        <aside className="run-pane none" data-testid="running.pane.none">
          <Empty title={running.pick_one} />
        </aside>
      )}
      </div>
    </section>
  );
}

/** ⌘F's field over the output: ⏎ the next match (selected, its line in view), esc closes. */
function Find({ term, onDone }: { term: { readonly current: Xterm | null }; onDone: () => void }) {
  const [q, setQ] = useState('');
  const last = useRef<Hit | null>(null);
  const [none, setNone] = useState(false);
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      term.current?.clearSelection();
      onDone();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const t = term.current;
      if (t === null) return;
      const buf = t.buffer.active;
      const lines = Array.from({ length: buf.length }, (_, i) => buf.getLine(i)?.translateToString(true) ?? '');
      const hit = nextHit(lines, q, last.current);
      last.current = hit;
      setNone(hit === null);
      if (hit === null) return;
      t.select(hit.col, hit.row, hit.len);
      t.scrollToLine(Math.max(0, hit.row - Math.floor(t.rows / 2)));
    }
  };
  return (
    <span className="run-find">
      <Field autoFocus placeholder={running.key_find} value={q} onChange={(e) => { setQ(e.currentTarget.value); last.current = null; setNone(false); }} onKeyDown={onKey} testId="running.find" />
      {none && <span className="run-find-none" data-testid="running.find.none">{running.none_match}</span>}
    </span>
  );
}

function Pane({ x, st, now, where, onTerm, onBack, onStop, onOpen, onFind, find }: { x: ProcItem; st: ReturnType<typeof stateOf>; now: number; where: string; onTerm: (t: Xterm | null) => void; onBack: () => void; onStop: () => void; onOpen: () => void; onFind: () => void; find: ReactNode }) {
  const u = urlWords(x.p, st);
  const live = st.k === 'running' || st.k === 'stopping';
  // stop is the only danger: a first click asks, the second stops (designer m_41694 #8)
  const [asking, setAsking] = useState(false);
  const cwd = x.p.cwd ?? '';
  return (
    <aside className="run-pane" data-testid="running.pane">
      <div className="run-phead">
        <Press look="run-back" onClick={onBack} data-testid="running.back"><AppIcon name="back" size={14} />{running.heading}</Press>
        <div className="run-meta" data-testid="running.pane.meta">
          {metaOf(x, st, now, where).map((m) => (
            <span key={m.k} className={`run-m-${m.k}`} {...tipAttrs(m.k === 'where' ? cwd : undefined)}>{m.w}</span>
          ))}
        </div>
        <span className="run-pacts">
          {/* ⌘F's find over the output: the key in its tip (no key line at the foot, bise fait mac §6) */}
          <Press look="run-icon" onClick={onFind} aria-label={running.key_find} data-testid="running.pane.find" {...tipAttrs(running.find_tip)}><AppIcon name="search" size={16} /></Press>
          {u !== null && (
            <Press look="run-act" onClick={onOpen} data-testid="running.pane.open" {...tipAttrs(u.tip)}>{running.open}<AppIcon name="open" size={14} /></Press>
          )}
          {st.k === 'running' && (
            <Press
              look={`run-act${asking ? ' ask' : ''}`}
              onClick={() => {
                if (asking) onStop();
                else setAsking(true);
              }}
              onBlur={() => { setAsking(false); }}
              data-testid="running.pane.stop"
            >
              {asking ? running.stop_confirm : running.stop}
            </Press>
          )}
        </span>
      </div>
      {/* the whole command, wrapped, highlighted as the thread's '$' rows are (Gabriel's ask) */}
      <pre className="run-pcmd" data-testid="running.pane.cmd">
        <Code text={x.p.cmd} lang="bash" />
      </pre>
      {find}
      <Output project={x.project} agent={x.p.agent} id={x.p.id} none={x.p.out === 'none' || x.p.out === 'unknown'} live={live} quiet={quietWords(x.p, st, now)} onTerm={onTerm} />
    </aside>
  );
}

/** A row's state column, as the terminal's /processes: '∿' in text-3 while it runs (green is only a
 *  diff's +: designer m_43401), a check in ink when it ended well, an x in --c-err when it failed,
 *  a dash otherwise; its tip says which. */
function Mark({ s }: { s: ReturnType<typeof stateOf> }) {
  const m = markOf(s);
  if (s.k === 'running' || s.k === 'stopping') return <span className="run-wave">{m.glyph}</span>;
  if (m.tone === 'ink') return <AppIcon name="check" size={14} />;
  if (m.tone === 'err') return <span className="run-err"><AppIcon name="close" size={14} /></span>;
  return <AppIcon name="fold" size={14} />;
}

/** One process (ds ListRow): its program in mono and its arguments dimmer, then its agent, its
 *  address and its age; the chosen one's 'open' (its address); its '…' and a right click are its
 *  menu (useEntityMenu: stop, open in a tab...). */
function ProcRow({ x, k, on, s, now, home, pinned, onClick, onOpen }: {
  x: ProcItem; k: string; on: boolean; s: ReturnType<typeof stateOf>; now: number; home: string | undefined; pinned: boolean;
  onClick: () => void; onOpen: (url: string) => void;
}) {
  const menu = useEntityMenu({ kind: 'process', project: x.project, agent: x.p.agent, id: x.p.id });
  const u = urlWords(x.p, s);
  const c = rowCmd(x.p.cmd, x.p.cwd);
  return (
    <div className="run-row" data-row={k} data-state={s.k}>
      <ListRow on={on} testId={`running.row.${k}`} onClick={onClick} onContextMenu={menu.onContextMenu}
        // the whole command in its tip (designer m_41694 #1)
        tip={`${x.p.cmd}\n${tipOf(x.p, home)}`}
        state={<span className="run-mark" data-testid={`running.row.${k}.mark`} {...tipAttrs(markTip(x.p, s, now))}><Mark s={s} /></span>}
        title={<span className="run-cmd" data-testid={`running.row.${k}.cmd`}><span className="run-prog">{c.prog}</span>{c.args !== '' && <span className="run-args">{` ${c.args}`}</span>}</span>}
        sub={<>
          {x.p.agent}
          {u !== null && <span className="run-url" data-testid={`running.row.${k}.url`}>{` · ${u.row}`}<AppIcon name="open" size={14} /></span>}
          {' · '}<span data-testid={`running.row.${k}.end`}>{endWords(x.p, s, now)}</span>
        </>}
        end={<>
          {pinned && <PinMark testId={`running.row.${k}.pinned`} />}
          {on && u !== null && <RowAction onClick={() => { onOpen(u.url); }} tip={u.tip} testId={`running.row.${k}.go`}>{running.open}<AppIcon name="open" size={14} /></RowAction>}
          <MoreButton menu={menu} testId={`running.row.${k}.more`} />
        </>} />
    </div>
  );
}

/** The clock for the ages: pinned in tests and shots, else ticking every 2 s (the hubs' refresh). */
function useNow(pinned?: number): number {
  const [now, setNow] = useState(() => pinned ?? Date.now());
  useEffect(() => {
    if (pinned !== undefined) return;
    const t = setInterval(() => {
      setNow(Date.now());
    }, 2_000);
    return () => {
      clearInterval(t);
    };
  }, [pinned]);
  return pinned ?? now;
}
