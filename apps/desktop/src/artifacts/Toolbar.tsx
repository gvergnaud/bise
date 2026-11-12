// The toolbar above the list (his m_44961: 'au dessus de la liste, c'est là où je m'attends qu'ils
// soient'; designer's spec D m_44968 and m_45148): one row, 32 high, 12 under the banner, 8 above
// the list, its edges the list rows' edges. Left the filters as they were in the bar (the kinds,
// ds Filters, then 'all projects ⌄'), 8 apart; right the cards/list switch, the search (200, 160
// under a 900 window; its '/' keycap until it has the focus or words, × to clear) and +. Sticky at
// the top of the page's scroller, a 1 px line under it only while stuck. When the row has less room
// than they need, the filters fold into one 'filters ⌄' menu that says how many are on; the search stays. What the
// filters and the keys do stays the screen's (Screen.tsx). Under 900 the library's crumb (its
// column folded, Fold.tsx) leads the row. Owner: m-artifacts (nm-pages-a's page).
import { useLayoutEffect, useRef, useState, type ChangeEvent, type KeyboardEvent, type ReactNode, type Ref } from 'react';
import { AppIcon, Button, cx, Dropdown, Filters, ListBar, ListSearch, Press, sentenceCase, tipAttrs } from '@/ds';
import { library } from '@/proto/words_library.gen';
import type { Facet, When } from './filters';
import type { View } from './gridKeys';
import './toolbar.css';

/** The room between the filters and the right side (--s-2). */
const GAP_PX = 8;

export interface KindOpt {
  readonly id: string;
  readonly word: string;
  readonly n: number;
}

export interface ToolbarProps {
  /** the kinds (all first, then each kind with its count); none: no kind filter (one kind, a stack) */
  readonly kinds: readonly KindOpt[] | null;
  readonly kind: string;
  readonly onKind: (id: string) => void;
  /** first on the left, never folded: under 900 the library's crumb (Fold.tsx), else nothing */
  readonly lead?: ReactNode;
  /** the project scope as drawn wide (Scope.tsx ProjectScope) */
  readonly scope: ReactNode;
  /** the projects for the folded menu, 'all projects' first */
  readonly projects: readonly { readonly project: string; readonly name: string; readonly n: number }[];
  readonly project: string;
  readonly onProject: (project: string) => void;
  readonly view: View;
  readonly onView: (v: View) => void;
  readonly q: string;
  readonly onQ: (e: ChangeEvent<HTMLInputElement>) => void;
  readonly onQKey: (e: KeyboardEvent<HTMLInputElement>) => void;
  readonly onClear: () => void;
  readonly searchRef: Ref<HTMLInputElement>;
  /** + add files (Drops.tsx AddButton) */
  readonly add: ReactNode;
  /** the When chip (filters.ts): its rows with their counts, the one picked; null: not drawn */
  readonly when?: WhenProps | null;
  /** Clear (designer m_61790): only while something is set; never the project or the place */
  readonly onClearAll?: (() => void) | null;
  /** under the field: the by: completion (filters.ts whoOptions), while a token is typed */
  readonly completion?: ReactNode;
  readonly testId: string;
}

export interface WhenProps {
  readonly rows: readonly Facet[];
  readonly on: When | null;
  readonly onPick: (w: When | null) => void;
}

/** The When chip's words. */
export const WHEN_WORD: Record<When, string> = {
  last: library.when_last, today: library.when_today, week: library.when_week, older: library.when_older,
};

/** The When chip (designer m_61790): 'When ▾' quiet; set, its value alone in ink and its × beside. */
export function WhenChip({ rows, on, onPick, testId }: WhenProps & { readonly testId: string }) {
  return (
    <span className={cx('art-when', on !== null && 'on')} data-testid={testId}>
      <Dropdown look="chip" testId={`${testId}.pick`} label={on === null ? library.when : WHEN_WORD[on]}
        groups={[{ id: 'when', rows: rows.map((r) => ({
          id: r.key, name: WHEN_WORD[r.key as When], trailing: String(r.n), current: on === r.key,
          // no dead ends: a value that holds nothing is shown, never picked
          disabled: r.n === 0 && on !== r.key, testId: `${testId}.${r.key}`,
        })) }]}
        onPick={(id) => { onPick(id as When); }} />
      {on !== null && (
        <Press look="art-when-x" aria-label={library.when_clear} onClick={() => { onPick(null); }} data-testid={`${testId}.x`}>
          <AppIcon name="close" size={14} />
        </Press>
      )}
    </span>
  );
}

/** How many filters are on (the folded menu's count). Pure. */
export function filtersOn(kind: string, project: string): number {
  return (kind === 'all' ? 0 : 1) + (project === '' ? 0 : 1);
}

export function Toolbar({ lead, kinds: kindOpts, kind, onKind, scope, projects, project, onProject, view, onView, q, onQ, onQKey, onClear, searchRef, add, when = null, onClearAll = null, completion = null, testId }: ToolbarProps) {
  const box = useRef<HTMLDivElement>(null);
  const [folded, setFolded] = useState(false);
  // the filters fold once the row has less room than they need beside the right side (never an
  // overflow, ui-review m_45291): what they need is measured while whole and kept while folded; new
  // kinds or another project measure again. An unmeasured box (a test's DOM) stays whole.
  const left = useRef<HTMLSpanElement>(null);
  const right = useRef<HTMLSpanElement>(null);
  const need = useRef(0);
  const isFolded = useRef(false);
  const sig = `${(kindOpts ?? []).map((k) => `${k.id}${String(k.n)}`).join()}|${project}`;
  useLayoutEffect(() => {
    const el = box.current;
    if (el === null) return undefined;
    isFolded.current = false;
    setFolded(false);
    const measure = () => {
      const w = el.clientWidth;
      if (w === 0) return;
      if (!isFolded.current) {
        const l = left.current;
        const r = right.current;
        if (l === null || r === null) return;
        need.current = l.scrollWidth + r.offsetWidth + GAP_PX;
      }
      const fold = w < need.current;
      if (fold !== isFolded.current) {
        isFolded.current = fold;
        setFolded(fold);
      }
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => { ro.disconnect(); };
  }, [sig]);
  const kinds = kindOpts !== null && <KindFilters kinds={kindOpts} on={kind} onPick={onKind} />;
  const on = filtersOn(kind, project);
  return (
    <ListBar ref={box} startRef={left} endRef={right} folded={folded} testId={testId}
      end={
        <>
          <span className="art-views">
            <Press look="art-view" aria-pressed={view === 'grid'} aria-label={library.tip_cards} onClick={() => { onView('grid'); }} data-testid="artifacts.view.grid" {...tipAttrs(library.tip_cards)}><AppIcon name="grid" size={16} /></Press>
            <Press look="art-view" aria-pressed={view === 'list'} aria-label={library.tip_list} onClick={() => { onView('list'); }} data-testid="artifacts.view.list" {...tipAttrs(library.tip_list)}><AppIcon name="list" size={16} /></Press>
          </span>
          <ListSearch ref={searchRef} value={q} placeholder={library.search_placeholder} onChange={onQ} onKeyDown={onQKey} onClear={onClear} testId="artifacts.search" />
          {completion}
          {add}
        </>
      }>
      {lead}
      {folded ? (
        <Dropdown testId={`${testId}.fold`}
          label={<>{sentenceCase(library.filters)}{on > 0 && <span className="art-n">{on}</span>}</>}
          groups={[
            ...(kindOpts === null ? [] : [{ id: 'kinds', head: library.kinds_label, rows: kindOpts.map((k) => ({ id: `k:${k.id}`, name: k.word, trailing: String(k.n), current: kind === k.id, testId: `${testId}.fold.k.${k.id}` })) }]),
            { id: 'projects', rows: projects.map((r) => ({ id: `p:${r.project}`, name: r.name, trailing: String(r.n), current: project === r.project, testId: `${testId}.fold.p.${r.project === '' ? 'all' : r.project}` })) },
          ]}
          onPick={(id) => { if (id.startsWith('k:')) onKind(id.slice(2)); else onProject(id.slice(2)); }} />
      ) : (
        <>
          {kinds}
          {scope}
        </>
      )}
      {when !== null && <WhenChip {...when} testId="artifacts.when" />}
      {onClearAll !== null && <Button kind="ghost" onClick={onClearAll} testId="artifacts.clear">{library.clear}</Button>}
    </ListBar>
  );
}

/** The kinds as the bar had them (ds Filters): 'All 866', then each kind with its count. */
function KindFilters({ kinds, on, onPick }: { kinds: readonly KindOpt[]; on: string; onPick: (id: string) => void }) {
  return (
    <Filters label={library.kinds_label} on={on} testId="artifacts.kinds"
      items={kinds.map((k) => ({ id: k.id, label: <>{k.word}<span className="art-n">{k.n}</span></> }))}
      onPick={onPick} />
  );
}
