// The sidebar (bise fait mac, page native-mac §6, mockup 1): beside the rail under the bar, 240
// wide (200-320 by its border, kept: frame-state.ts), --p-raised 82% with a 12 top-left corner.
// Its head, in his order (round 3, main m_44657): bise's own row (its ':*', what waits in all his
// projects, ⌘0; as on main), 'Search ⌘K', 'New project ⌘N' (no New agent row: he talks to main;
// ⇧⌘N stays in keys.ts, the File menu, the project menu and ⌘K). Then 'Projects' (no 'Pinned':
// his pinned agents lead the right side's rows only, his m_52954): one
// row per project, a click opens it (its main), no agent under it (round 2, his r2-img11: the
// agents live on the right side), as Gabriel's H1 (round 3, designer m_44783): its name and the
// inbox pill or its age, one plain sentence and its week's day bars, a quiet one on one line; ⌘
// held, its ⌘1-9 digit: siderows.ts and projectfacts.ts decide, this draws; the project he looks at is
// marked seen (seen.ts), and a row with an entry for him he hasn't seen has the unread dot after its
// name (proj-unread). A project row dragged up or down (or ⌥⌘↑ ⌥⌘↓) moves it in his order, the
// core's (sidedrag.ts, his m_49107). Its head stays, its list scrolls;
// a right click on a project opens its menu (useEntityMenu, nm-menus'). On the artifacts
// library page (decision d2) it shows '← Projects' and the library (artifacts' ArtLibrary) at the
// same width, a 160 ms crossfade; an artifact open in a panel keeps the projects (sideshows.ts says
// which, his m_46395). Owner: r3-projects this round (nm-frame's, archived).
//
// Each project row reads its own project's agents (useAgents): an event re-renders
// the row it touches, never the sidebar, and a thread's entries never do.
import { memo, useCallback, useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from 'react';
import { AppIcon, Input, tipAttrs, type AppIconName, Press } from '@/ds';
import { reducedMotion } from '@/ds/hooks';
import type { ProjectRow } from '@/proto';
import { frame } from '@/proto/words_frame.gen';
import { panel } from '@/proto/words_panel.gen';
import { useAgents, useProjects, useStore } from '@/store';
import { ArtLibrary } from '@/artifacts';
import { SettingsNav } from '@/settings/nav';
import { PLACE_KEYS } from '@/settings/keys';
import { useProjectWaiting, useWaitingCount } from '@/inbox/waiting';
import { useEntityMenu } from './menus/use';
import { useEntityAction } from './menus/actions';
import { MAIN, projectOf } from './route';
import { useRoute, useRouter } from './router';
import { sideShows } from './sideshows';
import { useKeptScroll } from './keep';
import { useModHeld } from './held';
import { LiveBiseMark } from './Marks';
import { addProject } from './addProject';
import { projectRowOf, projectsOf } from './siderows';
import { BAR_GAP, BAR_H, BAR_W, rowBars, type DayBar, type ProjectFacts } from './projectfacts';
import { newestOf, useLookedAt, useMarkSeen, useSeen } from './seen';
import { useNow } from './clock';
import { useSpring, type SpringRow } from './spring';
import { projects } from '@/proto/words_projects.gen';
import { waitsTip } from './tipwords';
import { DRAG_ATTR, dropMark, slideOf, useProjectDrag } from './sidedrag';

/** What waits on him, one number everywhere: the inbox's own list (designer m_25690). */
export function useWaitingTotal(): number {
  return useWaitingCount();
}

/** A row's name; cut ('…'), it carries the ds tip with the whole name (designer m_38093). */
function CutName({ name }: { name: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (el === null) return undefined;
    const sync = () => {
      if (el.scrollWidth > el.clientWidth) el.setAttribute('data-tip', name);
      else el.removeAttribute('data-tip');
    };
    sync();
    if (typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [name]);
  return (
    <span ref={ref} className="nm">
      {name}
    </span>
  );
}

/** A bar's shape: bottom-aligned, its top corners rounded 1 (a stub of 1 px stays square). */
function barPath(b: DayBar): string {
  const y = BAR_H - b.h;
  const r = b.h >= 2 ? 1 : 0;
  return `M${String(b.x)} ${String(BAR_H)}V${String(y + r)}Q${String(b.x)} ${String(y)} ${String(b.x + r)} ${String(y)}H${String(b.x + BAR_W - r)}Q${String(b.x + BAR_W)} ${String(y)} ${String(b.x + BAR_W)} ${String(y + r)}V${String(BAR_H)}Z`;
}

/**
 * A project's week (H1, designer m_45992): 7 day bars in 40 x 12, text-3, today's in text-2; the
 * row's hover wakes them all to text-2, the bar under the pointer goes to ink and its tip (its day,
 * its work) shows instead of the row's, off the row as the row's own (clear 'outer', ui-review
 * m_46014). Each bar's hover column is its full height and half of each gap beside it. A week not
 * known yet: 7 flat stubs (rowBars), so every row draws the same week.
 */
function Week({ week, now, testId }: { week: readonly number[] | null; now: number; testId: string }) {
  const bars = rowBars(week, now);
  const width = bars.length * (BAR_W + BAR_GAP) - BAR_GAP;
  return (
    <svg className="app-s-week" viewBox={`0 0 ${String(width)} ${String(BAR_H)}`} role="img" aria-label={projects.week_tip} data-testid={testId}>
      {bars.map((b, i) => (
        <g key={i} className={`app-s-day${b.today ? ' today' : ''}`} {...tipAttrs(b.tip, undefined, undefined, 'outer')} data-testid={`${testId}.${String(i)}`}>
          <rect x={b.hx} y={0} width={b.hw} height={BAR_H} />
          <path d={barPath(b)} />
        </g>
      ))}
    </svg>
  );
}

/** A project's line-1 end: ⌘ held, its digit; else the inbox pill (what waits on him), else its age. */
function ProjectEnd({ facts, digit, testId }: { facts: ProjectFacts; digit: number | undefined; testId: string }) {
  if (digit !== undefined) return <kbd data-testid={`${testId}.digit`}>{`⌘${String(digit)}`}</kbd>;
  if (facts.pill !== null)
    return (
      <span className="app-s-pill" {...tipAttrs(projects.pill_tip({ n: facts.pill }))} data-testid={`${testId}.pill`}>
        {facts.pill}
      </span>
    );
  if (facts.age === null) return null;
  return (
    <span className="app-s-age" data-testid={`${testId}.age`}>
      {facts.age}
    </span>
  );
}

/**
 * A project's row, Gabriel's H1 (designer m_44783): two calm lines, its name and its pill or age,
 * then one plain sentence and its week. Every project draws these same two lines at the same
 * height, busy or not (his m_48450, designer m_48464): a quiet one is its name in text-2, its last
 * news (else 'all quiet') and its week, flat stubs while unknown. No icon, no mark, no border: the words say it, the tip says it all.
 */
function ProjectRowView({ name, facts, on, digit, now, onClick, onContextMenu, spring, index, lift, slide, drop, testId }: {
  name: string;
  /** its place in the list, the drag's handle on it (sidedrag.ts) */
  index: number;
  /** dragged: how far it follows the pointer (px) */
  lift: number | undefined;
  /** another row dragged: how far this one slides to open the gap (px, sidedrag slideOf) */
  slide: number;
  /** the drop line, before or after it */
  drop: 'before' | 'after' | undefined;
  facts: ProjectFacts;
  on: boolean;
  /** the week's bars name their days from it */
  now: number;
  digit: number | undefined;
  onClick: () => void;
  onContextMenu: (e: MouseEvent<HTMLElement>) => void;
  /** a file dragged over it: lit, then it blinks and opens (spring.ts) */
  spring: SpringRow;
  testId: string;
}) {
  return (
    <Press
      look={`app-s-row project${facts.quiet ? ' quiet' : ''}${on ? ' on' : ''}`}
      aria-current={on ? 'page' : undefined}
      {...tipAttrs(facts.tip)}
      onClick={onClick}
      onContextMenu={onContextMenu}
      {...spring.drag}
      data-spring={spring.spring ?? undefined}
      {...{ [DRAG_ATTR]: index }}
      data-lifted={lift !== undefined || undefined}
      data-drop={drop}
      style={lift !== undefined ? { transform: `translateY(${String(lift)}px)` } : slide !== 0 ? { transform: `translateY(${String(slide)}px)` } : undefined}
      data-testid={testId}
    >
      <span className="app-s-l1">
        <CutName name={name} />
        <UnreadDot on={facts.unread} testId={`${testId}.unread`} />
        <span className="app-s-end">
          <ProjectEnd facts={facts} digit={digit} testId={testId} />
        </span>
      </span>
      <span className="app-s-l2">
        <span className={`app-s-say${facts.sentence.err ? ' err' : ''}`} data-testid={`${testId}.say`}>
          {facts.sentence.text}
        </span>
        <Week week={facts.week} now={now} testId={`${testId}.week`} />
      </span>
    </Press>
  );
}

/** The unread fade's length (designer m_47372: 150 ms; none under reduced motion). */
export const UNREAD_FADE_MS = 150;

/**
 * Something written for him he hasn't seen (designer m_47372): the accent dot after the name, no
 * tip of its own (the row's says 'N unread'). It shows still; once seen it fades out, then goes.
 */
function UnreadDot({ on, testId }: { on: boolean; testId: string }) {
  const [leaving, setLeaving] = useState(false);
  const [was, setWas] = useState(on);
  if (was !== on) {
    setWas(on);
    setLeaving(!on && !reducedMotion());
  }
  useEffect(() => {
    if (!leaving) return undefined;
    const t = window.setTimeout(() => {
      setLeaving(false);
    }, UNREAD_FADE_MS);
    return () => {
      window.clearTimeout(t);
    };
  }, [leaving]);
  if (!on && !leaving) return null;
  return <span className={`app-s-unread${on ? '' : ' out'}`} aria-hidden="true" data-testid={testId} />;
}

function Row({ kind, icon, name, end, on, tip, onClick, onContextMenu, testId }: {
  kind: 'top' | 'back';
  icon?: AppIconName;
  name: string;
  end?: ReactNode;
  on?: boolean;
  /** the row's whole story on hover */
  tip?: string;
  onClick: () => void;
  onContextMenu?: (e: MouseEvent<HTMLElement>) => void;
  testId: string;
}) {
  return (
    <Press
      look={`app-s-row ${kind}${on === true ? ' on' : ''}`}
      aria-current={on === true ? 'page' : undefined}
      {...tipAttrs(tip)}
      onClick={onClick}
      onContextMenu={onContextMenu}
      data-testid={testId}
    >
      {icon !== undefined && <AppIcon name={icon} size={16} />}
      <CutName name={name} />
      {end != null && <span className="app-s-end">{end}</span>}
    </Press>
  );
}

function Label({ children, end, testId }: { children: string; end?: ReactNode; testId: string }) {
  return (
    <div className="app-s-label" role="heading" aria-level={2} data-testid={testId}>
      <span>{children}</span>
      {end}
    </div>
  );
}

export function Sidebar({ onSearch }: { onSearch: () => void }) {
  const route = useRoute();
  const router = useRouter();
  // d2: the artifacts library takes the projects' place on the library page, at the same width; an
  // artifact open in a panel keeps his projects (sideshows.ts)
  const shows = sideShows(route);
  const library = shows === 'library';
  return (
    <aside className="app-side" data-testid="app.sidebar" data-library={library || undefined}>
      {/* on settings the sidebar is the settings nav alone (designer m_44447) */}
      {shows !== 'settings' && (
        <div className="app-s-head">
          <BiseRow />
          <Row kind="top" icon="search" name={frame.search} end={<kbd>{PLACE_KEYS.search}</kbd>} onClick={onSearch} testId="app.side.search" />
          <Row
            kind="top"
            icon="folderPlus"
            name={frame.new_project}
            end={<kbd>{PLACE_KEYS.newProject}</kbd>}
            onClick={() => {
              addProject(router);
            }}
            testId="app.side.new-project"
          />
        </div>
      )}
      {/* the swap is a crossfade (app.css .app-s-swap): a new key fades the other list in */}
      <div className="app-s-swap" key={shows}>
        {shows === 'library' ? <LibraryList /> : shows === 'settings' ? <SettingsNav /> : <ProjectList />}
      </div>
    </aside>
  );
}

/** d2: '← Projects' back (esc and ⌘[ too: router's back), then the library (nm-pages-a's ArtLibrary);
 *  ⌘ held, its rows' ⌘1-9 as the projects' (keys.ts opens them; rail-keys). */
function LibraryList() {
  const router = useRouter();
  const cmd = useModHeld('Meta');
  return (
    <div className="app-s-list" data-testid="app.side.library">
      <Row
        kind="back"
        icon="back"
        name={frame.projects}
        onClick={() => {
          if (router.canBack()) router.back();
          else router.go({ screen: 'bise' });
        }}
        testId="app.side.back"
      />
      <ArtLibrary digits={cmd} />
    </div>
  );
}

function ProjectList() {
  const rows = useProjects();
  const store = useStore();
  // ⌘ held: each project row shows its ⌘1-9 (keys.ts opens it), the held hints' delay (held.ts)
  const cmd = useModHeld('Meta');
  const route = useRoute();
  // where he was in a long sidebar comes back after a reload (keep.ts)
  useKeptScroll('sidebar', () => document.querySelector<HTMLElement>('[data-testid="app.side.list"]'));
  // kept while the core's rows stay: the drag's held order waits for them to change (sidedrag.ts)
  const ordered = useMemo(() => projectsOf(rows), [rows]);
  const root = useRef<HTMLDivElement>(null);
  const move = useCallback(
    (project: string, at: number) => {
      store.command({ cmd: 'project_move', project, order: at });
    },
    [store],
  );
  const { shown: list, drag, quiet, handlers } = useProjectDrag(ordered, root, move);
  // the rows slide open where it lands; under reduced motion they stay, the 2 px line says it (designer m_49316)
  const calm = drag !== null && reducedMotion();
  const here = projectOf(route);
  // the project he looks at (its main at its newest row, the window in front): its news, artifacts
  // and unread are seen, and its row counts nothing new while he looks (seen.ts)
  const looked = useLookedAt();
  useMarkSeen(looked, newestOf(list.find((p) => p.project === looked)?.pulse));
  const seenOf = useSeen();
  const seen = (project: string): number | undefined => (project === looked ? Infinity : seenOf(project));
  const now = useNow();
  return (
    <div ref={root} className="app-s-list" data-testid="app.side.list" data-drag={drag !== null || undefined} data-tip-quiet={quiet || undefined} {...handlers}>
      <Label testId="app.side.label.projects" end={<AddProject />}>
        {frame.projects}
      </Label>
      {list.map((p, i) => (
        <ProjectItem
          key={p.project}
          p={p}
          index={i}
          on={p.project === here}
          cmd={cmd}
          seen={seen(p.project)}
          now={now}
          lift={drag?.from === i ? drag.dy : undefined}
          slide={calm ? 0 : slideOf(drag, i)}
          drop={calm ? dropMark(drag, i, list.length) : undefined}
        />
      ))}
    </div>
  );
}

/**
 * bise's own row, the sidebar's first (his r3 order: bise, Search, New project): its live ':*' mark,
 * 'bise', what waits on him in all his projects (the inbox's one number, the projects' accent pill); ⌘ held, ⌘0.
 * It opens bise's place; marked there and on bise's own main (the home project stays out of the list).
 */
function BiseRow() {
  const cmd = useModHeld('Meta');
  const route = useRoute();
  const router = useRouter();
  const waits = useWaitingTotal();
  const home = useProjects().find((p) => p.home)?.project;
  const on = route.screen === 'bise' || (home !== undefined && route.screen === 'thread' && route.project === home && route.agent === MAIN);
  const id = 'app.side.bise';
  return (
    <Press
      look={`app-s-row bise${on ? ' on' : ''}`}
      aria-current={on ? 'page' : undefined}
      onClick={() => {
        router.go({ screen: 'bise' });
      }}
      data-testid={id}
    >
      <LiveBiseMark size={16} testId={`${id}.mark`} />
      <CutName name={panel.bise} />
      {cmd ? (
        <span className="app-s-end">
          <kbd data-testid={`${id}.digit`}>{PLACE_KEYS.bise}</kbd>
        </span>
      ) : (
        waits > 0 && (
          <span className="app-s-end">
            <span className="app-s-pill" {...tipAttrs(waitsTip(waits, true))} data-testid={`${id}.waits`}>
              {waits}
            </span>
          </span>
        )
      )}
    </Press>
  );
}

/** '+' at the Projects label's end: add a project (the add-a-project sheet). */
function AddProject() {
  const router = useRouter();
  return (
    <Press
      look="app-s-add"
      aria-label={frame.add_project}
      {...tipAttrs(frame.add_project)}
      onClick={() => {
        addProject(router);
      }}
      data-testid="app.side.add"
    >
      <AppIcon name="plus" size={14} />
    </Press>
  );
}

/** One project's row: it reads its own agents and its waiting count, so an event re-renders it alone. */
const ProjectItem = memo(function ProjectItem({ p, index, on, cmd, seen, now, lift, slide, drop }: {
  p: ProjectRow;
  index: number;
  on: boolean;
  cmd: boolean;
  seen: number | undefined;
  now: number;
  /** dragged: how far it follows the pointer (px) */
  lift: number | undefined;
  /** another row dragged: how far this one slides (px) */
  slide: number;
  /** the drop line, before or after it */
  drop: 'before' | 'after' | undefined;
}) {
  const store = useStore();
  const router = useRouter();
  const agents = useAgents(p.project);
  // what waits on him: the inbox's own count for this project (what he just answered left out)
  const waits = useProjectWaiting(p);
  const r = projectRowOf({ p, agents, waits, index, on, cmd, seen, now });
  const menu = useEntityMenu({ kind: 'project', project: p.project });
  const [renaming, setRenaming] = useState(false);
  // the menu's Rename takes its new name here, inline (app/menus/actions.ts)
  useEntityAction('rename', (e) => {
    if (e.kind !== 'project' || e.project !== p.project) return false;
    setRenaming(true);
    return true;
  });
  const id = `app.side.project.${p.project}`;
  const go = () => {
    router.go({ screen: 'thread', project: p.project, agent: MAIN });
  };
  // a file held over the row opens its project (spring.ts); the one in view never springs
  const spring = useSpring(on, go);
  if (renaming) {
    const done = (name: string | null) => {
      setRenaming(false);
      const n = name?.trim() ?? '';
      if (n !== '' && n !== p.name) store.command({ cmd: 'project_rename', project: p.project, name: n });
    };
    return (
      <div className="app-s-row project quiet" data-testid={`${id}.rename`}>
        <Input
          look="app-s-rename"
          autoFocus
          defaultValue={p.name}
          aria-label={p.name}
          onFocus={(e) => {
            e.currentTarget.select();
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') done(e.currentTarget.value);
            else if (e.key === 'Escape') {
              e.stopPropagation();
              done(null);
            }
          }}
          onBlur={(e) => {
            done(e.currentTarget.value);
          }}
          data-testid={`${id}.rename.field`}
        />
      </div>
    );
  }
  return (
    <ProjectRowView
      name={p.name}
      facts={r.facts}
      on={r.on}
      digit={r.digit}
      now={now}
      onClick={go}
      onContextMenu={menu.onContextMenu}
      spring={spring}
      index={index}
      lift={lift}
      slide={slide}
      drop={drop}
      testId={id}
    />
  );
});
