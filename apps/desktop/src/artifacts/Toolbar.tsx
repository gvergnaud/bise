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
import { AppIcon, Button, cx, Dropdown, Filters, ListBar, ListSearch, Press, tipAttrs } from '@/ds';
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

/** The app tabs (apps.ts appTabsOf, faceted): All and its count, five tabs at most, the rest under More. */
export interface AppsOpt {
  readonly total: number;
  readonly tabs: readonly KindOpt[];
  readonly more: readonly KindOpt[];
}

export interface ToolbarProps {
  /** the app tabs (docs/plugin-apps.md A6b, his pick A); none: no app filter (one app, a stack) */
  readonly apps: AppsOpt | null;
  /** the `app:` keys he picked (OR); none: All */
  readonly appsOn: readonly string[];
  /** a tab, More's row or All ('all'): `add` is ⌘-click (adds it or takes it off) */
  readonly onApp: (id: string, add: boolean) => void;
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
  /** For you | All (Audience.tsx), between the project and When; null: not drawn (a slot's place) */
  readonly audience?: ReactNode;
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
export function filtersOn(apps: number, project: string): number {
  return apps + (project === '' ? 0 : 1);
}

export function Toolbar({ lead, apps, appsOn, onApp, scope, projects, project, onProject, view, onView, q, onQ, onQKey, onClear, searchRef, add, audience = null, when = null, onClearAll = null, completion = null, testId }: ToolbarProps) {
  const box = useRef<HTMLDivElement>(null);
  // the filters fold once the row has less room than they need beside the right side (never an
  // overflow, ui-review m_45291), in two steps (his pick A): first the app tabs into one 'All ▾'
  // (stage 1), then every filter into one 'Filters ▾' (stage 2); the search stays. What a stage
  // needs is measured while in it and kept; new tabs or another project measure again. An
  // unmeasured box (a test's DOM) stays whole.
  const [stage, setStage] = useState<Stage>(0);
  const left = useRef<HTMLSpanElement>(null);
  const right = useRef<HTMLSpanElement>(null);
  const needs = useRef<number[]>([0, 0, 0]);
  const at = useRef<Stage>(0);
  const measure = useRef<() => void>(() => undefined);
  const sig = `${apps === null ? '' : [...apps.tabs, ...apps.more].map((k) => `${k.id}${String(k.n)}`).join()}|${appsOn.join()}|${project}`;
  useLayoutEffect(() => {
    const el = box.current;
    if (el === null) return undefined;
    at.current = 0;
    needs.current = [0, 0, 0];
    setStage(0);
    measure.current = () => {
      const w = el.clientWidth;
      const l = left.current;
      const r = right.current;
      if (w === 0 || l === null || r === null) return;
      needs.current[at.current] = l.scrollWidth + r.offsetWidth + GAP_PX;
      const next = nextStage(at.current, w, needs.current);
      if (next !== at.current) {
        at.current = next;
        setStage(next);
      }
    };
    measure.current();
    if (typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver(() => { measure.current(); });
    ro.observe(el);
    return () => { ro.disconnect(); };
  }, [sig]);
  // a new stage measures what it needs (and folds once more if that is still too wide)
  useLayoutEffect(() => { measure.current(); }, [stage]);
  const folded = stage === 2;
  const tabs = apps !== null && (stage === 0
    ? <AppTabs apps={apps} on={appsOn} onPick={onApp} />
    : <AppsFolded apps={apps} on={appsOn} onPick={onApp} testId={`${testId}.apps`} />);
  const on = filtersOn(appsOn.length, project);
  const appRows = apps === null ? [] : [{ id: 'all', word: library.all_kinds, n: apps.total }, ...apps.tabs, ...apps.more];
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
          // the menu says the app it holds, 'All apps ▾' or 'Sheets ▾', never 'Filters ▾' (designer m_63720)
          label={<>{foldWord(appRows, appsOn)}{on > 0 && <span className="art-n">{on}</span>}</>}
          groups={[
            ...(apps === null ? [] : [{ id: 'apps', head: library.apps_label, rows: appRows.map((k) => ({ id: `k:${k.id}`, name: k.word, trailing: String(k.n), current: isOn(appsOn, k.id), testId: `${testId}.fold.k.${k.id}` })) }]),
            { id: 'projects', rows: projects.map((r) => ({ id: `p:${r.project}`, name: r.name, trailing: String(r.n), current: project === r.project, testId: `${testId}.fold.p.${r.project === '' ? 'all' : r.project}` })) },
          ]}
          onPick={(id) => { if (id.startsWith('k:')) onApp(id.slice(2), false); else onProject(id.slice(2)); }} />
      ) : scope}
      {/* designer m_63213's order: project · For you | All · When · app tabs · search; no toggle in a slot's place */}
      {audience}
      {when !== null && <WhenChip {...when} testId="artifacts.when" />}
      {!folded && tabs}
      {onClearAll !== null && <Button kind="ghost" onClick={onClearAll} testId="artifacts.clear">{library.clear}</Button>}
    </ListBar>
  );
}

/** How far the filters folded: 0 whole, 1 the app tabs in 'All ▾', 2 every filter in 'Filters ▾'. */
type Stage = 0 | 1 | 2;

/**
 * The next stage at width `w` from what each stage needs (0: not measured yet): one step more
 * folded while too narrow, one step back once the less folded one fits again. Pure.
 */
export function nextStage(s: Stage, w: number, needs: readonly number[]): Stage {
  if (s < 2 && w < (needs[s] ?? 0)) return (s + 1) as Stage;
  const back = needs[s - 1] ?? 0;
  if (s > 0 && back > 0 && w >= back) return (s - 1) as Stage;
  return s;
}

/** A tab is on: one of the picked keys; none picked: All. Pure. */
const isOn = (on: readonly string[], id: string): boolean => (on.length === 0 ? id === 'all' : on.includes(id));

/** The app tabs (ds Filters, several on): 'All 866', each app with its count, then 'More ▾' past five. */
function AppTabs({ apps, on, onPick }: { apps: AppsOpt; on: readonly string[]; onPick: (id: string, add: boolean) => void }) {
  return (
    <>
      <Filters label={library.apps_label} on={on.length === 0 ? 'all' : on} testId="artifacts.apps"
        items={[{ id: 'all', word: library.all_kinds, n: apps.total }, ...apps.tabs].map((k) => ({ id: k.id, label: <>{k.word}<span className="art-n">{k.n}</span></> }))}
        onPick={(id, how) => { onPick(id, how?.add === true); }} />
      {apps.more.length > 0 && (
        <Dropdown look="chip" testId="artifacts.apps.more" label={library.apps_more}
          groups={[{ id: 'more', rows: apps.more.map((k) => ({ id: k.id, name: k.word, trailing: String(k.n), current: on.includes(k.id), testId: `artifacts.apps.more.${k.id}` })) }]}
          onPick={(id) => { onPick(id, false); }} />
      )}
    </>
  );
}

/** A folded menu's words: the apps picked ('Sheets', 'Sheets + Slides'), else 'All apps'. Pure. */
export function foldWord(rows: readonly { readonly id: string; readonly word: string }[], on: readonly string[]): string {
  const words = rows.filter((k) => k.id !== 'all' && on.includes(k.id)).map((k) => k.word);
  return words.length === 0 ? library.all_kinds : words.join(' + ');
}

/** The app tabs folded (stage 1): one 'All ▾' (or the picked apps' words) holding All, the tabs and More's. */
function AppsFolded({ apps, on, onPick, testId }: { apps: AppsOpt; on: readonly string[]; onPick: (id: string, add: boolean) => void; testId: string }) {
  const rows = [{ id: 'all', word: library.all_kinds, n: apps.total }, ...apps.tabs, ...apps.more];
  const words = rows.filter((k) => k.id !== 'all' && on.includes(k.id)).map((k) => k.word);
  return (
    <Dropdown look="chip" testId={testId} label={words.length === 0 ? library.all_kinds : words.join(' + ')}
      groups={[{ id: 'apps', rows: rows.map((k) => ({ id: k.id, name: k.word, trailing: String(k.n), current: isOn(on, k.id), testId: `${testId}.${k.id}` })) }]}
      onPick={(id) => { onPick(id, false); }} />
  );
}
