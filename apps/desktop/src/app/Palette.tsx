// ⌘K: one search over the window (designer's signed block s-k, desktop-screens v7; lead m_11844):
// every agent of every project, archived ones too, the artifacts and the scheduled tasks, grouped,
// keyboard first. The ranking is src/search's, the bar's spotlight finds with the same rules.
// No query: 'waiting for you', then 'recent'. tab / shift+tab cycle the scope (all · agents ·
// artifacts · scheduled), ↑↓ ctrl+n/p move, ⏎ opens (a waiting agent: answers in its thread),
// ⌘⏎ restores an archived agent then opens it; one or two words that name nothing, or a sentence:
// ⏎ asks bise (a sentence: 'ask bise' is the first row, what it names one row down, main m_12973,
// the spotlight's rule from src/search's asksBise); esc closes. Its own field: the composer keeps its draft.
// Bise fait mac keeps main's palette whole (r2-palette, Gabriel m_44243): its glyphs, its rows,
// no chips. Only the rows in view are drawn, the others are gaps of their height (paletteWindow.ts,
// cmdk-perf): a one-letter query can match a thousand rows. Owner: amb-web (lead m_11864), in amb-win's shell.
import { useCoverWhile } from '@/ds';
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { useCoreEvent, useIndex, useProjects, useStore } from '@/store';
import { caretAtEnd, keep, useKeptText } from '@/app/keep';
import type { Ev } from '@/store/store';
import { flatHits, marks, nextScope, SCOPES, searchList, type Hit, type Scope, type SearchList } from '@/search/rank';
import { searchDataOf } from '@/search/data';
import { useGoArtifact } from '@/artifacts/leave';
import { PAGES } from '@/settings';
import { useNow } from './clock';
import { inboxFromMenu } from './inboxAsk';
import { UNMEASURED, windowOf, type Measures } from './paletteWindow';
import { HELP_EVENT } from './keys';
import { appPages } from './pages';
import { MAIN, parseRoute } from './route';
import { useRoute, useRouter } from './router';
import './palette.css';
import { Input, Press } from '@/ds';

/** The kept view bit: ⌘K open (lead m_11997, keep.ts: an open panel, no age). */
const OPEN_KEY = 'palette.open';

/** Whether ⌘K is open, kept across a reload or a relaunch like his query (Shell's state). */
export function usePaletteOpen(): readonly [boolean, (open: boolean) => void] {
  const [open, setOpen] = useState(() => keep.view(OPEN_KEY) === true);
  const set = useCallback((on: boolean) => {
    keep.setView(OPEN_KEY, on ? true : undefined);
    setOpen(on);
  }, []);
  return [open, set];
}

export const PW = {
  ph: 'search agents, artifacts, scheduled tasks',
  none: (q: string): string => `nothing matches “${q}”. archived agents, artifacts and scheduled tasks were searched too.`,
  ask: (q: string): string => `ask bise “${q}”`,
  blank: 'nothing to search yet. your agents, what they make and their scheduled tasks show up here as you work.',
  keys: '↑↓ choose   ⏎ open   tab all · agents · artifacts · scheduled   esc close',
  keysArchived: (scope: Scope): string => `↑↓ choose   ⏎ open   ⌘⏎ restore   ${scope === 'all' ? 'tab agents only' : 'tab all · agents · artifacts · scheduled'}   esc close`,
  keysNone: '⏎ ask bise   esc close',
  keysAsk: '↑↓ choose   ⏎ ask bise   esc close',
  hintAsk: '⏎ ask',
  keysBlank: 'esc close',
  tabOnly: 'tab agents only',
  hint: (h: Hit): string => (h.archived ? '⏎ open   ⌘⏎ restore' : h.waiting ? '⏎ answer' : '⏎ open'),
} as const;

/** The key line for a list and its selected row. */
export function keyLine(l: SearchList, at: Hit | undefined, scope: Scope): string {
  if (l.mode === 'none') return PW.keysNone;
  // a sentence: the 'ask bise' row is chosen (no hit)
  if (l.mode === 'ask' && at === undefined) return PW.keysAsk;
  if (l.mode === 'blank') return PW.keysBlank;
  return at?.archived === true ? PW.keysArchived(scope) : PW.keys;
}

/** Key pairs ('⏎ open   esc close') drawn with the TUI's 3-space gap (about 1.2em, designer
 * m_11923): each gap is its own span, so the text still reads with its 3 spaces. */
function Pairs({ text }: { text: string }): ReactNode {
  const parts = text.split(/ {3}/);
  const out: ReactNode[] = [];
  parts.forEach((p, i) => {
    if (i > 0)
      out.push(
        <span className="kgap" key={`g${i}`}>
          {'   '}
        </span>,
      );
    out.push(<span key={`p${i}`}>{p}</span>);
  });
  return out;
}

/** The events that change what the search reads. */
const READS = new Set<Ev['ev']>(['agents', 'artifacts', 'cards', 'projects', 'scheduled']);

/** A name with the query's matches bold and underlined. */
function Marked({ text, q }: { text: string; q: string }): ReactNode {
  const ms = marks(text, q);
  if (ms.length === 0) return text;
  const out: ReactNode[] = [];
  let i = 0;
  for (const [a, b] of ms) {
    if (a > i) out.push(text.slice(i, a));
    out.push(
      <span className="mt" key={a}>
        {text.slice(a, b)}
      </span>,
    );
    i = b;
  }
  if (i < text.length) out.push(text.slice(i));
  return out;
}

export function Palette({ onClose }: { onClose: () => void }) {
  const store = useStore();
  const router = useRouter();
  const goArtifact = useGoArtifact();
  const projects = useProjects();
  const index = useIndex();
  const now = useNow();
  // kept across a reload or an update swap (amb-win's keep, lead m_11756): dropped once used or on esc
  const [q, setQ, dropQ] = useKeptText('palette');
  const [scope, setScope] = useState<Scope>('all');
  const [at, setAt] = useState(0);
  const [version, setVersion] = useState(0);
  const field = useRef<HTMLInputElement>(null);
  // a web tab's page draws above the window's page: it hides while the palette shows (ds/cover.ts; ui-review m_64756)
  useCoverWhile(true);
  useCoreEvent(
    useCallback((ev: Ev) => {
      if (READS.has(ev.ev)) setVersion((v) => v + 1);
    }, []),
  );
  // the projects the core doesn't hold: asked once per opening, re-sent by the core on change
  useEffect(() => {
    store.askIndex();
  }, [store]);
  const route = useRoute();
  const data = useMemo(() => {
    const d = searchDataOf({ projects, agents: (p) => store.agents(p), artifacts: (p) => store.artifacts(p), cards: (p) => store.cards(p), scheduled: (p) => store.scheduled(p), index });
    // the app's pages by name (ambient-lead m_21499): the in-view project's first, the in-view agent's log
    return { ...d, pages: appPages({ projects: d.projects, route, settings: PAGES }) };
    // version: a held project's agents, cards or artifacts changed
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projects, index, store, version, route]);
  const list = useMemo(() => searchList(data, q, scope, now), [data, q, scope, now]);
  const hits = useMemo(() => flatHits(list), [list]);
  // a sentence (main m_12973): 'ask bise' is the first row, ⏎ asks; the hits come one row down
  const askRow = list.mode === 'ask';
  const rows = hits.length + (askRow ? 1 : 0);
  const cur = Math.min(at, Math.max(0, rows - 1));
  const onAsk = list.mode === 'none' || (askRow && cur === 0);
  const sel = askRow ? (cur === 0 ? undefined : hits[cur - 1]) : hits[cur];
  const home = projects.find((p) => p.home)?.project;

  useEffect(() => {
    field.current?.focus();
    caretAtEnd(field.current);
  }, []);
  useEffect(() => {
    document.querySelector(`[data-testid="app.palette.item.${String(cur)}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [cur]);
  // only the rows in view are drawn (paletteWindow.ts): the box's measures and scroll, read after
  // each render (a state change only when they moved), on a resize, and on a scroll at once
  // (flushSync: the rows are there before the frame paints, ui-review m_46875)
  const body = useRef<HTMLDivElement>(null);
  const [m, setM] = useState<Measures>(UNMEASURED);
  const [box, setBox] = useState<{ top: number; view: number; dir: -1 | 0 | 1; step: number }>({ top: 0, view: 0, dir: 0, step: 0 });
  const readBox = useCallback(() => {
    const b = body.current;
    if (b === null) return;
    const top = b.scrollTop;
    const view = b.clientHeight;
    setBox((x) => (x.top === top && x.view === view ? x : { top, view, dir: top > x.top ? 1 : top < x.top ? -1 : x.dir, step: Math.abs(top - x.top) }));
  }, []);
  const onScroll = useCallback(() => {
    flushSync(readBox);
  }, [readBox]);
  useLayoutEffect(() => {
    const b = body.current;
    if (b === null) return;
    const row = b.querySelector<HTMLElement>('.app-k-row')?.offsetHeight ?? 0;
    const head = b.querySelector<HTMLElement>('.app-k-head')?.offsetHeight ?? 0;
    const pad = parseFloat(getComputedStyle(b).paddingTop) || 0;
    setM((x) => {
      const n = { row: row > 0 ? row : x.row, head: head > 0 ? head : x.head, pad };
      return n.row === x.row && n.head === x.head && n.pad === x.pad ? x : n;
    });
    readBox();
  }, [list, readBox]);
  useEffect(() => {
    const b = body.current;
    if (b === null || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(readBox);
    ro.observe(b);
    return () => {
      ro.disconnect();
    };
  }, [readBox]);
  const parts = windowOf({ sizes: list.groups.map((g) => g.hits.length), askRow, m, scrollTop: box.top, view: box.view, cur, dir: box.dir, step: box.step });

  const open = (h: Hit | undefined, restore = false) => {
    if (h === undefined) return;
    dropQ();
    onClose();
    if (h.open.to === 'artifact') {
      // a browser-only one (a PR, a link) opens his browser, no route (leave.ts)
      goArtifact(h.open.project, h.open.id);
      return;
    }
    // a page: as its key does (⌘I the inbox in place, ⌘/ the key list, the rest a route)
    if (h.open.to === 'page') {
      const go = h.open.go;
      if ('inbox' in go)
        inboxFromMenu((to) => {
          router.go(to);
        });
      else if ('help' in go) window.dispatchEvent(new Event(HELP_EVENT));
      else router.go(parseRoute(go.route));
      return;
    }
    if (restore && h.archived) store.command({ cmd: 'unarchive', project: h.open.project, agent: h.open.agent });
    const agent = h.open.agent;
    const main = store.agents(h.open.project).find((a) => a.name === agent)?.main === true;
    router.go({ screen: 'thread', project: h.open.project, agent: main ? MAIN : agent });
  };
  const ask = () => {
    const text = q.trim();
    if (text === '' || home === undefined) return;
    dropQ();
    onClose();
    store.send({ project: home, agent: 'main', text });
    router.go({ screen: 'thread', project: home, agent: MAIN });
  };

  const onKey = (e: KeyboardEvent) => {
    const down = e.key === 'ArrowDown' || (e.ctrlKey && e.key === 'n');
    const up = e.key === 'ArrowUp' || (e.ctrlKey && e.key === 'p');
    if (down || up) {
      e.preventDefault();
      setAt(Math.max(0, Math.min(rows - 1, cur + (down ? 1 : -1))));
    } else if (e.key === 'Tab') {
      e.preventDefault();
      setScope((s) => nextScope(s, e.shiftKey));
      setAt(0);
    } else if (e.key === 'Enter' && e.metaKey) {
      e.preventDefault();
      if (onAsk) ask();
      else if (sel?.archived === true) open(sel, true);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (onAsk) ask();
      else open(sel);
    } else if (e.key === 'Escape' || (e.ctrlKey && e.key === 'c')) {
      e.preventDefault();
      e.stopPropagation();
      dropQ();
      onClose();
    }
  };

  // each group's first row's flat index (the ask row is 0 when there is one)
  const firsts = list.groups.reduce<number[]>((acc, _g, i) => [...acc, i === 0 ? (askRow ? 1 : 0) : (acc[i - 1] ?? 0) + (list.groups[i - 1]?.hits.length ?? 0)], []);
  return (
    <div
      className="app-palette"
      data-testid="app.palette"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="app-k" onKeyDown={onKey} role="dialog" aria-label="search">
        <label className="app-k-field">
          <span aria-hidden>⌕</span>
          <Input
            ref={field}
            value={q}
            placeholder={PW.ph}
            aria-label={PW.ph}
            spellCheck={false}
            onChange={(e) => {
              setQ(e.currentTarget.value);
              setAt(0);
            }}
            data-testid="app.palette.field"
          />
        </label>
        <div className="app-k-scopes" role="tablist">
          {SCOPES.map((s) => (
            <Press
              key={s}
              role="tab"
              aria-selected={s === scope}
              look={s === scope ? 'app-k-scope on' : 'app-k-scope'}
              tabIndex={-1}
              onMouseDown={(e) => {
                e.preventDefault();
              }}
              onClick={() => {
                setScope(s);
                setAt(0);
              }}
              data-testid={`app.palette.scope.${s}`}
            >
              {s}
            </Press>
          ))}
        </div>
        <div ref={body} className="app-k-body" role="listbox" aria-label="results" onScroll={onScroll}>
          {list.mode === 'blank' && (
            <div className="app-k-empty" data-testid="app.palette.blank">
              <p>{PW.blank}</p>
            </div>
          )}
          {list.mode === 'none' && (
            <div className="app-k-empty" data-testid="app.palette.none">
              <span className="big" aria-hidden>
                ⌕
              </span>
              <p>{PW.none(q.trim())}</p>
              <Press look="app-k-ask" onClick={ask} data-testid="app.palette.ask">
                {PW.ask(q.trim())} <kbd>⏎</kbd>
              </Press>
            </div>
          )}
          {askRow && (
            <div
              role="option"
              aria-selected={cur === 0}
              className={['app-k-row', cur === 0 ? 'on' : ''].filter((c) => c !== '').join(' ')}
              data-kind="ask"
              onMouseMove={() => {
                if (cur !== 0) setAt(0);
              }}
              onClick={ask}
              data-testid="app.palette.item.0"
            >
              <span className="g pearl" aria-hidden>
                <i className="app-k-pearl bise" data-testid="app.palette.ask.pearl" />
              </span>
              <span className="nm">{PW.ask(q.trim())}</span>
              <span className="rt">{cur === 0 && <span className="hint"><Pairs text={PW.hintAsk} /></span>}</span>
            </div>
          )}
          {list.groups.map((g, gi) => {
            const first = firsts[gi] ?? 0;
            return (
            <div key={g.id} role="group" aria-label={g.label} data-testid={`app.palette.group.${g.id}`}>
              <div className="app-k-head">
                <span>{g.label}</span>
                {g.id !== 'recent' && <span className="x">&nbsp;· {g.hits.length}</span>}
                <span className="sp" />
                <span className="x">{scope === 'all' && g.id === 'agents' && sel !== undefined && g.hits.includes(sel) ? PW.tabOnly : ''}</span>
              </div>
              {(parts[gi] ?? []).flatMap((p, pi) =>
                'gap' in p
                  ? [<div key={`gap.${String(pi)}`} aria-hidden style={{ height: p.gap * m.row }} />]
                  : g.hits.slice(p.draw[0], p.draw[1]).map((h, j) => {
                const n = first + p.draw[0] + j;
                const on = n === cur;
                return (
                  <div
                    key={h.id}
                    role="option"
                    aria-selected={on}
                    className={['app-k-row', on ? 'on' : '', h.archived ? 'arch' : ''].filter((c) => c !== '').join(' ')}
                    data-kind={h.kind}
                    data-archived={h.archived || undefined}
                    onMouseMove={() => {
                      if (!on) setAt(n);
                    }}
                    onClick={() => {
                      open(h);
                    }}
                    data-testid={`app.palette.item.${String(n)}`}
                  >
                    <span className={`g ${h.tone}`} aria-hidden>
                      {h.tone === 'pearl' ? <i className="app-k-pearl" /> : h.tone === 'bise' ? <i className="app-k-pearl bise" data-testid={`app.palette.item.${String(n)}.pink`} /> : h.glyph}
                    </span>
                    <span className="nm">
                      {h.parent !== undefined && <span className="up">{h.parent} › </span>}
                      <Marked text={h.name} q={q} />
                      {h.of !== undefined && <span className="up"> · {h.of}</span>}
                      {h.detail !== '' && <span className="d">{h.detail}</span>}
                    </span>
                    <span className="rt">
                      {h.where !== '' && <span className="pj">{h.where}</span>}
                      {h.when !== '' && <span>{h.when}</span>}
                      {h.key !== undefined && <kbd className="b-kbd">{h.key}</kbd>}
                      {on && <span className="hint"><Pairs text={PW.hint(h)} /></span>}
                    </span>
                  </div>
                );
              }),
              )}
            </div>
            );
          })}
        </div>
        <div className="app-k-keys" data-testid="app.palette.keys">
          <Pairs text={keyLine(list, sel, scope)} />
        </div>
      </div>
    </div>
  );
}
