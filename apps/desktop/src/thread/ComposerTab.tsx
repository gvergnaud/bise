// The composer's context strip (bise fait mac, mockup 1; it replaced designer's composer-strip A
// tab): the box's first row, 32 high, 12/16 text-2, 14 px icons, 16 between: the addressee, its
// state while it runs and the branch in mono, each a tipped label when there is nothing to pick
// (native-lead m_42159: a picker only where a real choice exists). No project: the sidebar picks
// and says it (his m_50754; its picker here only opened another project's main). On the right
// the context ring, or the screen's own words ('esc to bise instead', the approvals flash). His
// words answering a card (or another agent's @name, an open artifact): the label alone in the
// accent, '↳ answering search · …' (answerFit). His words to an agent that is not main: 'to perf'
// first, in the accent, always kept (his ask m_44704); main's 'to bise' text-2. That name is a
// picker of the project's live agents (his m_47126, designer m_47129: tab.ts agentPicks); a bare
// strip's name stays a label (the card or the artifact decides who). Items that don't fit go whole,
// 'lands on main' first (tab.ts stripFit), measured on a resize or new words, never per keystroke (the
// composer is memoised). Owner: nm-composer.
import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import type { Agent } from '@/proto';
import { AppIcon, PickerPop, PickerTrigger, Ring, RowMark, tipAttrs } from '@/ds';
import { useAgents, useFlow, usePlaces, useProjects } from '@/store';
import { placeOf as placeOfAgent, tilde, worktreeParts } from '@/app/barplaces';
import { AgentSign, WorkingMark } from '@/app/Marks';
import { useNow } from '@/app/clock';
import { composer } from '../proto/words.gen';
import { agentPicks, answerFit, contextOf, RING_CLEAR_PCT, ringTip, stripFit, stripState, tabPlace, type AgentPick, type StripFit, type TabPlace, type TabWords } from './tab';
import { waitLen } from './wake';
import { composerWords } from './head';
import { shownName } from '@/agents/model';

/** his home folder, for '~/…' in the tips (main's window.bise.home) */
const HOME = (globalThis as { bise?: { home?: string } }).bise?.home;

/** Where `a` works, as the strip names it (tab.ts tabPlace): its worktree's Place and his checkout's path. */
export function useTabPlace(project: string, a: Agent | undefined, home: boolean): TabPlace | undefined {
  const places = usePlaces(project);
  const path = useProjects().find((p) => p.project === project)?.path;
  const wt = a === undefined ? undefined : placeOfAgent(a, places);
  const tip = wt === undefined ? '' : worktreeParts(wt, HOME).map((x) => x.t).join('');
  return tabPlace(a, home, wt === undefined ? undefined : { path: wt.path ?? (wt.id.startsWith('pt:') ? wt.id.slice(3) : null), tip }, path, (x) => tilde(x, HOME));
}

/** The strip's words as one object while they hold (a memoised composer never draws again for an
 *  unrelated render of its screen). */
export function useTabWords(label: string, place: TabPlace | undefined, context: string, who: string): TabWords {
  const name = place?.name;
  const worktree = place?.worktree;
  const tip = place?.tip;
  const branch = place?.branch;
  return useMemo(
    () => ({ label, place: name === undefined ? undefined : { name, worktree: worktree === true, tip: tip ?? '', branch }, context, who }),
    [label, name, worktree, tip, branch, context, who],
  );
}

/** The strip's addressee: the agent his words go to ('to perf', whatever the label's rest says),
 *  and what picking another one does (none: a label, as under an open artifact, which decides who). */
export interface StripAgent {
  readonly project: string;
  readonly agent: string;
  readonly onPick?: ((agent: string) => void) | undefined;
}

const ALL: StripFit = { state: true, end: true, lands: true, branch: true };
/** The room a notice at the strip's right keeps at least (Composer's NoticeLine cuts its words to the rest). */
const NOTICE_ROOM = 160;

/** A string's width in a font, by a canvas (null: no canvas, as in unit tests: the css clip then). */
function widthIn(font: string): ((s: string) => number) | null {
  const ctx = document.createElement('canvas').getContext('2d');
  if (ctx === null) return null;
  ctx.font = font;
  return (s) => Math.ceil(ctx.measureText(s).width);
}

/**
 * A label's width as MarkedLabel draws it: its first part in the `.who` span's font (bold in the
 * accent strip), the rest in the label's. Measured all in the label's font, a bold head came out
 * short and the css clip cut the question mid-word (designer m_27021: 'next to th…').
 */
function labelWidth(lbl: HTMLElement): ((s: string) => number) | null {
  const reg = widthIn(getComputedStyle(lbl).font);
  if (reg === null) return null;
  const who = lbl.querySelector<HTMLElement>('.who');
  const head = (who === null ? null : widthIn(getComputedStyle(who).font)) ?? reg;
  return (s) => {
    const i = s.indexOf(' · ');
    return i < 0 ? head(s) : head(s.slice(0, i)) + reg(s.slice(i));
  };
}

/** The label with its mark after the first part: '↳ answering perf' ∿ ' · the question'. */
function MarkedLabel({ label, mark }: { label: string; mark: ReactNode }) {
  const cut = label.indexOf(' · ');
  return cut < 0 ? (
    <>
      <span className="who">{label}</span>
      {mark}
    </>
  ) : (
    <>
      <span className="who">{label.slice(0, cut)}</span>
      {mark}
      {label.slice(cut)}
    </>
  );
}

/** The addressee's segment ('to perf ⌄', his m_47126): the accent when his words go to an agent
 *  that is not main, main's text-2; with `onPick`, a button that opens the project's live agents
 *  (designer m_47129: bise first, each with its state's sign, the current one checked). Always kept. */
function AgentItem({ label, accent, who, testId }: { label: string; accent: boolean; who: StripAgent | undefined; testId: string }) {
  const agents = useAgents(who?.project ?? '');
  const now = useNow();
  const trigger = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const cls = accent ? 'it to acc' : 'it to';
  const pick = who?.onPick;
  if (who === undefined || pick === undefined) {
    return (
      <span className={cls} data-item="to" data-testid={`${testId}.to`}>
        <span className="nm">{label}</span>
      </span>
    );
  }
  const rows = open ? agentPicks(agents, now) : [];
  return (
    <>
      <PickerTrigger ref={trigger} extra={`${cls} cmp-ctx-pick`} open={open} label={composer.ctx_agent_pick({ agent: label.replace(/^to /u, '') })}
        tip={composer.ctx_agent_pick({ agent: label.replace(/^to /u, '') })} testId={`${testId}.to`}
        onClick={() => {
          setOpen((o) => !o);
        }}>
        <span className="nm">{label}</span>
      </PickerTrigger>
      {open && (
        <PickerPop
          anchor={trigger}
          box=".box"
          side="above"
          align="start"
          title={composer.ctx_agents}
          groups={[{ id: 'agents', rows: rows.map((r) => ({ id: r.name, name: r.shown, text: r.shown, current: r.name === who.agent, ...(r.sign === '' ? {} : { glyph: <PickSign sign={r.sign} testId={`${testId}.to.pick.${r.name}.sign`} /> }), testId: `${testId}.to.pick.${r.name}` })) }]}
          onPick={(id) => {
            setOpen(false);
            if (id !== who.agent) pick(id);
          }}
          onClose={() => {
            setOpen(false);
          }}
          testId={`${testId}.to.menu`}
        />
      )}
    </>
  );
}

/** An agent's state in the picker's glyph column, as the sidebar draws it: main's breath, the ask
 *  mark in the accent, the alert, the check; a pause's '…' as its text. */
function PickSign({ sign, testId }: { sign: Exclude<AgentPick['sign'], ''>; testId: string }) {
  if (sign === '…') return <span data-testid={testId} data-sign={sign}>…</span>;
  const mark = sign === '∿' ? 'works' : sign === '?' ? 'asks' : sign === '✗' ? 'failed' : 'done';
  return (
    <span className="cmp-ctx-sign" data-testid={testId} data-sign={sign}>
      <AgentSign mark={mark} testId={testId} />
    </span>
  );
}

export function ComposerTab({ words, mark, end, project, who, bare = false, newPlace = false, to = false, asks = false, at, testId }: {
  words: TabWords;
  /** his words go to an agent that is not main (pinkState's quiet, or an open item only its options
   *  decide): 'to perf' first, in the accent; bare, the label's rest text-3 (designer m_44890) */
  to?: boolean;
  /** bare, an open question from main: the box stays plain, the label in the accent (designer m_47129) */
  asks?: boolean;
  /** the addressee and its picker (not bare); none: its name a label */
  who?: StripAgent | undefined;
  /** after the label's first part (bare): the addressee's working gust */
  mark?: ReactNode;
  /** the right end instead of the context ring */
  end?: ReactNode;
  /** the project's hub id: its branch and flow ('lands on main'); its name is never said here, the
   *  sidebar picks and says it (his m_50754) */
  project?: string | undefined;
  /** the label alone (answering, an @name, an open artifact): no project, place or branch */
  bare?: boolean;
  /** the empty home: the branch is the project's own (main starts its worktrees from it) */
  newPlace?: boolean;
  /** the addressee's row: its state while it runs (tab.ts stripState); none or idle: nothing said */
  at?: Agent | undefined;
  testId: string;
}) {
  const ref = useRef<HTMLSpanElement>(null);
  // the strip's own minute clock: 'working · 3 min' moves, the memoised field above never redraws
  const now = useNow();
  const state = bare ? null : stripState(at, now);
  // the empty home: the project's own branch (where main starts its worktrees from)
  const projectBranch = useProjects().find((p) => p.project === project)?.branch ?? undefined;
  const flow = useFlow(project ?? '');
  const [fit, setFit] = useState<StripFit>(ALL);
  // the label cut after a whole word when even alone it doesn't fit (designer m_24325); null: whole
  const [cut, setCut] = useState<string | null>(null);
  // answering, the end ('esc to bise instead') goes once the question has gone (answerFit)
  const [endIn, setEndIn] = useState(true);
  const context = words.context ?? '';
  const ring = contextOf(context);
  const place = words.place;
  const key = `${words.label}|${place?.name ?? '-'}|${place?.branch ?? ''}|${project ?? ''}|${context}|${end === undefined ? '' : 'end'}|${bare ? 'b' : ''}|${to ? 't' : ''}`;
  useLayoutEffect(() => {
    const host = ref.current?.closest<HTMLElement>('.head');
    if (host == null) return;
    const measure = () => {
      const q = (sel: string) => host.querySelector<HTMLElement>(sel);
      const nat = (e: HTMLElement | null) => (e === null ? 0 : Math.ceil(e.scrollWidth));
      const cs = getComputedStyle(host);
      const gap = parseFloat(cs.columnGap) || 0;
      const inner = host.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight) - (q(':scope > .notice') === null ? 0 : NOTICE_ROOM + gap);
      if (bare) {
        const lbl = q('[data-item="label"]');
        const markEl = lbl?.querySelector<HTMLElement>('[data-testid="thread.mark"]') ?? null;
        const markW = markEl === null ? 0 : Math.ceil(markEl.getBoundingClientRect().width + (parseFloat(getComputedStyle(markEl).marginLeft) || 0));
        const width = lbl === null ? null : labelWidth(lbl);
        const endEl = q('[data-item="end"]');
        const endW = endEl === null ? 0 : nat(endEl) + gap;
        if (width === null) return;
        // the question first, then the end; '↳ answering search' never goes; an open artifact's
        // '· about “…”' whole or gone (designer m_27021)
        const f = answerFit(words.label, inner - markW, endW, width, words.whole === true);
        const nextCut = f.label === words.label ? null : f.label;
        setCut((c) => (c === nextCut ? c : nextCut));
        setEndIn(f.end);
        return;
      }
      setEndIn(true);
      // the branch's whole width even while its css cuts it: its box less what its words show, plus
      // all of its words (ui-review m_47192: read cut, 40 px kept 's…' under BRANCH_MIN)
      const br = q('[data-item="branch"]');
      const brWords = br?.querySelector<HTMLElement>('.nm') ?? null;
      const branchW = br === null ? 0 : nat(br) + (brWords === null ? 0 : Math.max(0, Math.ceil(brWords.scrollWidth) - Math.ceil(brWords.clientWidth)));
      const w = { state: nat(q('[data-item="state"]')), branch: branchW, end: nat(q('[data-item="end"]')), lands: nat(q('[data-item="lands"]')) };
      // 'to perf' always stays: the rest fits in what it leaves
      const toEl = q(`[data-testid="${testId}.to"]`);
      const next = stripFit({ avail: inner - (toEl === null ? 0 : nat(toEl) + gap), gap, ...w });
      setFit((f) => (f.state === next.state && f.end === next.end && f.lands === next.lands && f.branch === next.branch ? f : next));
      // the label is only drawn bare; a plain strip never cuts one
      setCut((c) => (c === null ? c : null));
    };
    measure();
    // a font that loads after the first measure widens the items, not the host: measure again then
    // (designer m_50840: main at 900 kept 'lands on main' in one theme's run, dropped it in the other's)
    const fonts = (document as { fonts?: FontFaceSet }).fonts;
    let live = true;
    const again = () => {
      if (live) measure();
    };
    void fonts?.ready.then(again);
    fonts?.addEventListener('loadingdone', again);
    const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    ro?.observe(host);
    return () => {
      live = false;
      fonts?.removeEventListener('loadingdone', again);
      ro?.disconnect();
    };
  }, [key, words.label, words.whole, bare, to, testId]);
  const endEl =
    end !== undefined ? (
      <span className={endIn && fit.end ? 'end' : 'end gone'} data-item="end" data-testid={`${testId}.end`} aria-hidden={!(endIn && fit.end) || undefined}>
        {end}
      </span>
    ) : (
      !bare &&
      ring !== null && (
        // the tokens and the ring, its words in the tip; past 80% the percent shows in clear too
        <span className={fit.end ? 'end ctx' : 'end ctx gone'} data-item="end" data-testid={`${testId}.context`} {...tipAttrs(ringTip(words.who ?? 'its', ring))} aria-label={ringTip(words.who ?? 'its', ring)}
          aria-hidden={!fit.end || undefined} data-pct={Math.round(ring.pct)}>
          {/* the tokens used left of the ring (his ask m_26205: '84k'), past 80% its percent too */}
          <span className="num">{ring.pct > RING_CLEAR_PCT ? `${ring.tokens} · ${String(Math.round(ring.pct))}%` : ring.tokens}</span>
          <Ring pct={ring.pct} />
        </span>
      )
    );
  if (bare) {
    return (
      <span ref={ref} className="cmp-ctx bare" data-testid={testId}>
        <span className={to ? 'lbl to' : asks ? 'lbl ask' : 'lbl'} data-item="label" data-testid={`${testId}.to`}>
          <MarkedLabel label={cut ?? words.label} mark={mark} />
        </span>
        {endEl}
      </span>
    );
  }
  const branch = newPlace ? projectBranch : place?.branch;
  // a trunk project's fact, where the work lands, text-3 after the branch (designer m_43996): never
  // a loose line between the thread and the dock
  const lands = flow === 'trunk' && branch !== undefined ? composer.ctx_lands({ branch: projectBranch ?? 'main' }) : null;
  return (
    <span ref={ref} className="cmp-ctx" data-testid={testId}>
      {/* whom his words go to, first: 'to perf' in the accent, as main's tab had it (agent-target,
          designer m_28208; his ask m_44704: clear at a glance), main's 'to bise' text-2; a picker
          of the project's agents (his m_47126) */}
      <AgentItem label={who === undefined ? words.label : composerWords(shownName(who.agent)).label} accent={to} who={who} testId={testId} />
      {/* where 'local' was (his m_44719: the only option, no control): the addressee's state while
          it runs, main's breath and how long, or the ask mark in the accent; nothing when idle */}
      {state !== null && (
        <span className={fit.state ? 'it st' : 'it st gone'} data-item="state" data-state={state.kind} data-testid={`${testId}.state`} aria-hidden={!fit.state || undefined}>
          {state.kind === 'working' ? <WorkingMark testId={`${testId}.state.mark`} /> : <RowMark icon="ask" tone="accent" label={composer.ctx_waits} />}
          <span className="nm">{state.kind === 'working' ? composer.ctx_working({ dur: waitLen(state.ms) }) : composer.ctx_waits}</span>
        </span>
      )}
      {branch !== undefined && (
        <span className={fit.branch ? 'it br' : 'it br gone'} data-item="branch" data-testid={`${testId}.branch`} {...tipAttrs(composer.ctx_branch_tip({ branch }))} aria-hidden={!fit.branch || undefined}>
          <AppIcon name="branch" size={14} />
          <span className="nm mono">{branch}</span>
        </span>
      )}
      {lands !== null && (
        <span className={fit.lands ? 'it lands' : 'it lands gone'} data-item="lands" data-testid={`${testId}.lands`} aria-hidden={!fit.lands || undefined}>
          <span className="nm">{lands}</span>
        </span>
      )}
      {endEl}
    </span>
  );
}
