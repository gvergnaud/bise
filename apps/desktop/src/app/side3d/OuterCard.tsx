// src/app/side3d · the outer card (page outer-side v2, Gabriel's pick m_46173): the Overview out of
// the panel, a card at the window's right (his width by its left border, layout outerW: 292 by default, 260-480, inline; 12 from the edges, the bar's included: its top
// inline, tilestate OUTER_TOP + the tile's room), shown while the panel is folded and the room leaves the column 480 (layout 'card'); a panel
// open gives way to the coins, then to the minis (Host). Its sections head in the folded side's
// style: the section's 3D object at 20, its name, its count, 'all ›'. AGENTS: one line of 28 each,
// in the hub's rank: its mark (the bar's language, ⌥N while ⌥ is held, an ask's in the accent),
// its head at 20, its name and its ψ right after it when it has its own worktree (its tip names
// the worktree and its branch; never under the hover pin, designer m_55493), then right-aligned
// its age while it works or asks and its PR; idle and done step back. A row of the tile's project and its bim light each other (bims/focus,
// his m_47261): the row hovered rings its bim, the bim hovered gives its row the hover tint. A click opens its thread; an ask's click opens it in place,
// its options and 'answer…' / 'write to perf…' under it. The other sections keep the floats' rows
// (FlyoutLists). Each section shows its first rows (Agents 10, the others 5), then '+ N more' adds
// 50 (outerrows.ts); an empty one keeps its empty line. The card scrolls as a whole, a 16 fade at
// the edge that has more, macOS's overlay scrollbar. Owner: nm-side.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type MouseEvent, type ReactNode } from 'react';
import { bimsLit, useBimsLit } from '@/bims/focus';
import { flushSync } from 'react-dom';
import type { Agent, Card } from '@/proto';
import { AppIcon, Press, tipAttrs } from '@/ds';
import { sortCards } from '@/inbox/order';
import { useProjects, useStore } from '@/store';
import { worktreeName } from '@/running/model';
import type { AnswerAt } from '@/store/answered';
import { side } from '@/proto/words_side.gen';
import { panel } from '@/proto/words_panel.gen';
import { waitLen } from '@/thread/wake';
import { AgentSign } from '../Marks';
import { askInbox } from '../inboxAsk';
import { useEscLayer } from '../escape';
import { useEntityMenu } from '../menus/use';
import { roleWords, type BarRow, type ProjAgent } from '../barrows';
import { ThingPic } from './Bubbles';
import { OptButton } from './FlyoutAgents';
import { askButtons, pickIntent } from './model';
import { bubbleThing, type BubbleId } from './objects';
import { groupParts } from './lists';
import { useCalm } from './useCalm';
import { PinButton, PinMark } from '../pinbutton';
import { pinnedIn, usePins } from '../pins';

const keyOfLine = (x: { readonly r: BarRow }): string => x.r.key;
import { OUTER_TOP } from './tilestate';
import { OUTER_W } from './layout';
import { canLess, FIRST_AGENTS, FIRST_ROWS, headLift, moreOf, useShown } from './outerrows';

/** an agent's head in the card, css px */
export const OUTER_HEAD_PX = 20;

/** the composer's field, where 'answer…' puts his words */
const COMPOSER_FIELD = '[data-testid="composer.field"]';

/** A section's head: its 3D object, its name, its count (and what waits on him, in the accent), 'all ›'. */
export function OuterHead({ id, name, count, loud, onAll, onCube }: { id: BubbleId; name: string; count: string; loud?: string; onAll: () => void; onCube?: () => void }) {
  return (
    <header className="s3-ohead" data-testid={`app.outer.${id}.head`}>
      <span className="s3-ohead-pic" aria-hidden="true">
        <ThingPic thing={bubbleThing(id)} px={OUTER_HEAD_PX} tile={id === 'agents' ? 'thread' : id === 'artifacts' ? 'artifacts' : id === 'scheduled' ? 'scheduled' : 'terminal'} />
      </span>
      <b className="s3-ohead-name">{name}</b>
      {count !== '' && <span className="s3-ohead-n">{count}</span>}
      {loud !== undefined && loud !== '' && <span className="s3-ohead-loud" data-testid={`app.outer.${id}.loud`}>· {loud}</span>}
      <Press look="s3-ohead-all" onClick={onAll} data-testid={`app.outer.${id}.all`}>{side.all_short}</Press>
      {onCube !== undefined && (
        <Press look="s3-ohead-cube" aria-label={side.tile_show} {...tipAttrs(side.tile_show)} onClick={onCube} data-testid={`app.outer.${id}.cube`}>
          <AppIcon name="cube" size={14} />
        </Press>
      )}
    </header>
  );
}

export interface OuterAgentsProps {
  readonly live: readonly BarRow[];
  readonly agents: readonly ProjAgent[];
  readonly cards: readonly Card[];
  readonly at: AnswerAt | null;
  /** the thread in view: its row is tinted */
  readonly here: { readonly project: string | null; readonly agent: string | null };
  readonly now: number;
  readonly groupOf?: (project: string) => string | undefined;
  readonly onOpen: (r: BarRow) => void;
  readonly onOpenPr: (r: BarRow, e?: MouseEvent) => void;
  /** the project whose office the tile shows (its rows and its bims light each other, bims/focus); null: no tile */
  readonly bims: string | null;
  /** an ask opened in place (its agent) or closed (null): the tile looks at that bim while it is open (designer m_47300); keep it stable */
  readonly onAsk?: (ask: { readonly project: string; readonly agent: string } | null) => void;
}

/**
 * The ψ's tip (designer m_55493): 'worktree <folder> · <branch>', only the folder when the branch
 * is that same word (sb/<folder>) or none; 'works in a worktree' when its folder is unknown. `repo`:
 * its project's own folder name (a bise worktree is `<task>/<repo>`: the task names it). Pure.
 */
export function worktreeTip(a: Pick<Agent, 'worktree' | 'branch'> | undefined, repo: string): string {
  const path = a?.worktree ?? '';
  const folder = path === '' ? '' : worktreeName(path, repo);
  if (folder === '') return side.in_worktree;
  const branch = a?.branch ?? '';
  return branch === '' || branch === folder || branch === `sb/${folder}` ? side.worktree_at({ folder }) : side.worktree_on({ folder, branch });
}

/**
 * The agents' rows past their worktree heads: each with its PR (its own, or its shared worktree's)
 * and, when it works in a worktree, its ψ's tip (worktreeTip; null: the shared checkout). `repoOf`:
 * a project's own folder name. Pure.
 */
export function outerRows(live: readonly BarRow[], agents: readonly ProjAgent[], repoOf: (project: string) => string = () => ''): { r: BarRow; pr: number | undefined; wt: string | null }[] {
  const heads = new Map(live.filter((r) => r.kind === 'group' && r.group !== undefined).map((r) => [r.group ?? '', r]));
  const byKey = new Map(agents.map((x) => [`${x.project}/${x.a.name}`, x.a]));
  return live.filter((r) => r.kind === 'agent').map((r) => {
    const head = r.group === undefined ? undefined : heads.get(r.group);
    const a = byKey.get(`${r.project}/${r.agent ?? ''}`);
    const inWt = r.group !== undefined || a?.mode === 'worktree';
    return { r, pr: r.mark?.pr ?? head?.mark?.pr ?? undefined, wt: inWt ? worktreeTip(a, repoOf(r.project)) : null };
  });
}

/** A project's own folder name, from its path ('' when unknown). Pure. */
export const repoName = (path: string): string => path.split('/').filter((x) => x !== '').pop() ?? '';

/** AGENTS in the card: its first 10 lines, then '+ N more' adds 50. */
export function OuterAgents({ live, agents, cards, at, here, now, groupOf, onOpen, onOpenPr, bims, onAsk }: OuterAgentsProps) {
  const projects = useProjects();
  const rows = outerRows(live, agents, (p) => repoName(projects.find((x) => x.project === p)?.path ?? ''));
  const pins = usePins('agent');
  const byKey = new Map(agents.map((x) => [`${x.project}/${x.a.name}`, x.a]));
  const [open, setOpen] = useState<string | null>(null);
  const asked = open === null ? undefined : rows.find((x) => x.r.key === open)?.r;
  const askProject = asked?.project ?? null;
  const askAgent = asked?.agent ?? null;
  useEffect(() => {
    if (onAsk === undefined) return;
    onAsk(askProject === null || askAgent === null ? null : { project: askProject, agent: askAgent });
    return () => {
      onAsk(null);
    };
  }, [askProject, askAgent, onAsk]);
  // esc folds an ask open in place (app/escape.ts row 4, designer m_47321), the focus back on its row
  const box = useRef<HTMLDivElement>(null);
  const fold = useCallback(() => {
    box.current?.querySelector<HTMLElement>('.s3-orow[data-open] [data-row]')?.focus();
    setOpen(null);
  }, []);
  useEscLayer('ask', open !== null, fold);
  const { shown, more, less } = useShown(FIRST_AGENTS);
  // the row <-> bim link (bims/focus): a bim hovered tints its row; a row hovered lights its bim
  const lit = useBimsLit();
  const litBim = lit?.from === 'bim' ? lit.name : null;
  useEffect(() => () => {
    bimsLit.set(null, 'row');
  }, []);
  const linked = (r: BarRow): boolean => bims !== null && r.project === bims && r.agent !== undefined && r.main !== true;
  // the order holds under the pointer and slides when it moves (useCalm, designer m_47454)
  const calm = useCalm(rows, keyOfLine, box);
  return (
    <div ref={box} className="s3-orows" {...calm.bind} data-testid="app.outer.agents.rows">
      {calm.rows.slice(0, shown).map(({ r, pr, wt }) => {
        const a = byKey.get(`${r.project}/${r.agent ?? ''}`);
        const card = sortCards(cards.filter((c) => c.agent === r.agent && c.project === r.project))[0];
        return (
          <AgentLine key={r.key} r={r} pr={pr} wt={wt} job={a === undefined ? r.sub : jobOf(a)} card={card} at={at} now={now}
            here={here.project === r.project && here.agent === r.agent} group={groupOf?.(r.project)} open={open === r.key}
            pinned={r.agent !== undefined && pinnedIn(pins, r.project, r.agent)}
            lit={linked(r) && r.agent === litBim}
            onLit={linked(r) ? (on) => {
              bimsLit.set(on ? { name: r.agent ?? '', from: 'row' } : null, 'row');
            } : undefined}
            onToggle={() => {
              setOpen((o) => (o === r.key ? null : r.key));
            }} onOpen={onOpen} onOpenPr={onOpenPr} />
        );
      })}
      <MoreRow id="agents" first={FIRST_AGENTS} shown={shown} total={rows.length} onMore={more} onLess={less} />
    </div>
  );
}

/**
 * Under a section's rows: '+ N more' at the left (N more show), 'show less' at the right once it
 * unfolded (50 fold back, down to its first; designer m_47011: always at the right, so a second
 * click finds it). After a fold that left the section's head above the card's view, the card
 * scrolls it back to the top: a fold never drops him into the next section. Nothing when the first
 * rows hold them all.
 */
function MoreRow({ id, first, shown, total, onMore, onLess }: { id: BubbleId; first: number; shown: number; total: number; onMore: () => void; onLess: () => void }) {
  const line = useRef<HTMLDivElement>(null);
  const n = moreOf(shown, total);
  const lessToo = canLess(shown, total, first);
  if (n === 0 && !lessToo) return null;
  const fold = () => {
    flushSync(onLess);
    const sec = line.current?.closest('.s3-osec');
    const card = line.current?.closest<HTMLElement>('.s3-outer');
    if (sec == null || card == null) return;
    const lift = headLift(sec.getBoundingClientRect().top, card.getBoundingClientRect().top);
    if (lift > 0) card.scrollTop -= lift;
  };
  return (
    <div ref={line} className="s3-omores">
      {n > 0 && (
        <Press look="s3-omore" onClick={onMore} data-testid={`app.outer.${id}.more`}>
          {side.more_rows({ n })}
        </Press>
      )}
      {lessToo && (
        <Press look="s3-omore s3-oless" onClick={fold} data-testid={`app.outer.${id}.less`}>
          {side.show_less}
        </Press>
      )}
    </div>
  );
}

interface LineProps {
  readonly r: BarRow;
  readonly pr: number | undefined;
  /** its ψ's tip when it works in a worktree (worktreeTip); null: none */
  readonly wt: string | null;
  readonly job: string;
  readonly card: Card | undefined;
  readonly at: AnswerAt | null;
  readonly now: number;
  readonly here: boolean;
  readonly group: string | undefined;
  /** he pinned it (pins.ts): the pin mark at rest, 'Unpin' on hover */
  readonly pinned: boolean;
  readonly open: boolean;
  readonly onToggle: () => void;
  readonly onOpen: (r: BarRow) => void;
  readonly onOpenPr: (r: BarRow, e?: MouseEvent) => void;
  /** its bim is hovered in the tile: the rows' hover tint */
  readonly lit: boolean;
  /** the pointer comes on the row (true) or leaves it: its bim lights; none, the row has no bim in the tile */
  readonly onLit: ((on: boolean) => void) | undefined;
}

/** An agent's job words: what it does now, else its title, else its purpose; never main's purpose (main at rest is the team lead, barrows.ts). Pure. */
export function jobOf(a: Pick<Agent, 'main' | 'note' | 'title' | 'purpose'>): string {
  return roleWords(a.note || a.title || (a.main ? '' : a.purpose));
}

/** A line's tip: its name and its job's words; main at rest says what it is, 'team lead' (native-lead m_47965). Pure. */
export function lineTip(r: Pick<BarRow, 'name' | 'main'>, job: string): string {
  if (job !== '') return `${r.name} · ${job}`;
  return r.main === true ? `${r.name} · ${panel.role_lead}` : r.name;
}

/** One agent's line: mark · head · name · ψ, then the end's column: pin mark · age · PR; an ask opens in place under it. */
function AgentLine({ r, pr, wt, job, card, at, now, here, group, pinned, open, onToggle, onOpen, onOpenPr, lit, onLit }: LineProps) {
  const store = useStore();
  const menu = useEntityMenu({ kind: 'agent', project: r.project, agent: r.agent ?? '' });
  const st = r.state;
  const asks = card !== undefined;
  const id = `app.outer.agents.${r.project}.${r.agent ?? ''}`;
  const g = groupParts(group, r.name);
  const key = r.glyph.startsWith('⌥') ? r.glyph : null;
  const age = asks ? waitLen(now - card.since_ms) : st?.tone === 'work' ? (r.time ?? '') : '';
  const b = card === undefined ? null : askButtons(card);
  const pinnable = r.main !== true && r.agent !== undefined;
  return (
    <div className="s3-orow" data-asks={asks || undefined} data-here={here || undefined} data-back={st?.back === true || undefined} data-open={open || undefined}
      data-lit={lit || undefined} data-calm={r.key} onContextMenu={menu.onContextMenu} data-testid={id}
      onPointerEnter={onLit && (() => {
        onLit(true);
      })}
      onPointerLeave={onLit && (() => {
        onLit(false);
      })}>
      <Press look="s3-orow-open" aria-label={r.title} aria-expanded={asks ? open : undefined} {...tipAttrs(lineTip(r, job), undefined, 'left')} onClick={() => {
        if (asks) onToggle();
        else onOpen(r);
      }} data-row="">
        <span className="s3-orow-mark" data-tone={st?.tone} data-testid={`${id}.mark`}>
          {key !== null ? <span className="s3-orow-key">{key}</span> : <Mark g={st?.g ?? ''} id={id} />}
        </span>
        <span className="s3-orow-head" aria-hidden="true">
          <ThingPic thing={{ head: r.agent ?? '', main: r.main === true }} px={OUTER_HEAD_PX} tile="thread" />
        </span>
        <span className="s3-orow-name" data-testid={`${id}.name`}>{g.project !== '' && <span className="s3-group">{g.project}</span>}{g.name}</span>
        {/* the ψ says where it works: right after the name, never under the hover pin (designer m_55493) */}
        {wt !== null && <span className="s3-orow-wt" {...tipAttrs(wt)} data-testid={`${id}.wt`}>ψ</span>}
        {/* the end's column (pin mark, age, PR) stays right-aligned past this */}
        <span className="s3-orow-gap" aria-hidden="true" />
        {pinned && <PinMark testId={`${id}.pinned`} />}
        {age !== '' && <span className="s3-orow-age" {...tipAttrs(st?.tip)} data-testid={`${id}.age`}>{age}</span>}
        {pr !== undefined && (
          <span className="s3-orow-pr" role="button" {...tipAttrs(r.mark?.tip ?? `#${String(pr)}`)} onClick={(e) => {
            e.stopPropagation();
            onOpenPr(r);
          }} data-testid={`${id}.pr`}><AppIcon name="branch" size={14} />#{String(pr)}</span>
        )}
      </Press>
      {/* the hover pin over the line's last thing, no room kept at rest (designer m_49518); main leads anyway, never pinned here */}
      {pinnable && <PinButton pin={{ kind: 'agent', project: r.project, id: r.agent ?? '', on: pinned }} look="s3-pin s3-opin" testId={`${id}.pin`} />}
      {open && card !== undefined && b !== null && (
        <div className="s3-orow-ask" data-testid={`${id}.ask`}>
          <span className="s3-orow-q">{card.question}</span>
          <span className="s3-orow-acts">
            {b.opts.map((o) => (
              <OptButton key={o.n} label={o.label} onPick={() => store.do(pickIntent(card, o.n, at))} testId={`${id}.opt.${String(o.n)}`} />
            ))}
            <Press look="s3-arow-answer" onClick={() => {
              askInbox({ project: card.project, card: card.id });
              requestAnimationFrame(() => {
                document.querySelector<HTMLElement>(COMPOSER_FIELD)?.focus();
              });
            }} data-testid={`${id}.answer`}>
              {card.approval === true ? side.write_to({ agent: r.name }) : side.answer}
            </Press>
          </span>
        </div>
      )}
    </div>
  );
}

/** An agent's mark in the bar's language: the circled ? (asks), the breath (works), '…', '✓', '✗', '◷'; nothing at rest. */
function Mark({ g, id }: { g: string; id: string }): ReactNode {
  if (g === '?') return <AgentSign mark="asks" testId={`${id}.sign`} />;
  if (g === '∿') return <AgentSign mark="works" testId={`${id}.sign`} />;
  if (g === '') return null;
  return <span className="s3-orow-g">{g}</span>;
}

interface SectionProps {
  readonly id: BubbleId;
  readonly name: string;
  readonly count: string;
  readonly loud?: string;
  readonly onAll: () => void;
  readonly onCube?: () => void;
}

/** One section of the card: its head and what is under it (Agents: OuterAgents). */
export function OuterSection({ id, name, count, loud, onAll, onCube, children }: SectionProps & { children?: ReactNode }) {
  return (
    <section className="s3-osec" data-testid={`app.outer.${id}`}>
      <OuterHead id={id} name={name} count={count} {...(loud === undefined ? {} : { loud })} onAll={onAll} {...(onCube === undefined ? {} : { onCube })} />
      {children}
    </section>
  );
}

/** A section of rows: its first 5, then '+ N more' adds 50; `rows(n)` draws the first n; none: its empty line. */
export function OuterList({ total, rows, empty, ...head }: SectionProps & { total: number; rows: (n: number) => ReactNode; empty: ReactNode }) {
  const { shown, more, less } = useShown(FIRST_ROWS);
  return (
    <OuterSection {...head}>
      {total === 0 ? empty : rows(shown)}
      <MoreRow id={head.id} first={FIRST_ROWS} shown={shown} total={total} onMore={more} onLess={less} />
    </OuterSection>
  );
}

/** Where the card's scroll is: more above, more below; each edge with more fades. Pure. */
export function scrollEdges(top: number, view: number, height: number): { readonly above: boolean; readonly below: boolean } {
  return { above: top > 0, below: top + view < height - 1 };
}

/**
 * The card: its sections, one scroll for all when they pass its height (no list scrolls inside
 * it), a 16 fade at each edge that has more. `tile`: the room the bims' tile and its handle take
 * above it (0: hidden).
 */
export function OuterCard({ tile = 0, w = OUTER_W, children }: { tile?: number; w?: number; children: ReactNode }) {
  const box = useRef<HTMLElement>(null);
  const [edges, setEdges] = useState({ above: false, below: false });
  const measure = () => {
    const el = box.current;
    if (el === null) return;
    const next = scrollEdges(el.scrollTop, el.clientHeight, el.scrollHeight);
    setEdges((e) => (e.above === next.above && e.below === next.below ? e : next));
  };
  // after each render: rows came or went ('+ N more', a new artifact), the window's height
  useLayoutEffect(measure);
  return (
    <aside ref={box} className="s3-outer" style={{ top: OUTER_TOP + tile, width: w }} onScroll={measure}
      data-above={edges.above || undefined} data-below={edges.below || undefined} data-tip-box="" data-testid="app.outer">
      {children}
    </aside>
  );
}
