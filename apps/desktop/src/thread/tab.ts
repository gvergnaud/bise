// The composer's context strip (bise fait mac, mockup 1; it replaced designer's composer-strip A tab):
// the box's first row, where his words go: the addressee, its state while it runs
// (stripState: 'working · 3 min', 'waits for you'; nothing idle) and the branch in mono, and on the
// right the context ring or the screen's own words. No project: the sidebar picks and says it (his
// m_50754).
// Short, it drops whole items: 'lands on main' first, then the right end, then the state; the branch
// stays and 'to perf' never goes (stripFit). Answering a card: '↳ answering search · <question>'
// and 'esc to bise instead' on the right; short, the question goes first, then that end;
// '↳ answering search' never goes (answerFit). Which states make it pink (an item, words opening
// with another agent's @name, an open artifact, an agent that is not main): pinkState. The
// addressee picker's rows: agentPicks.
// Pure: ComposerTab.tsx draws and measures it. Owner: nm-composer.
import type { Agent } from '@/proto';
import { liveOf, rankedOf, shownName } from '@/agents/model';
import { signOf } from '@/app/side';
import { composer } from '../proto/words.gen';
import { MAIN } from '../app/route';

/** Where its words go, as the tab names it (designer m_24790): his checkout by its folder's name,
 *  a worktree by its own folder's name after the ψ; the tip says what it is. */
export interface TabPlace {
  readonly name: string;
  /** a worktree ('worktree' in the strip), else his own checkout ('local') */
  readonly worktree: boolean;
  readonly tip: string;
  /** the branch it works on (its worktree's, else the checkout's); none: not known */
  readonly branch?: string | undefined;
}

/** What the tab says. */
export interface TabWords {
  /** 'to perf', 'answering search · which size…', 'to perf · voice paused', 'to perf · about “…”' */
  readonly label: string;
  /** where it works; undefined: not said (bise's own main) */
  readonly place?: TabPlace | undefined;
  /** 'context 42k · 17%' ('' or undefined: none, as at 0%): the ring's */
  readonly context?: string | undefined;
  /** whose context it is, as the ring's tip names it ('perf', 'bise') */
  readonly who?: string | undefined;
  /** the label's part after its head shows whole or not at all (an open artifact's pink '↳ to perf · about “…”') */
  readonly whole?: boolean | undefined;
}

/** A folder's own name: the last part of its path. Pure. */
export function folderName(path: string): string {
  return path.replace(/\/+$/u, '').split('/').at(-1) ?? path;
}

/** A worktree's name: its folder under ~/.bise/worktrees/<hub>/<name>/<repo>, else its folder's own name. Pure. */
export function worktreeName(path: string): string {
  const parts = path.replace(/\/+$/u, '').split('/');
  const i = parts.lastIndexOf('worktrees');
  return (i >= 0 && parts.length >= i + 3 ? parts[i + 2] : parts.at(-1)) ?? path;
}

/**
 * Where an agent works, as the strip says it (designer m_24790, his m_24782): a worktree by its
 * folder's name with wt-sign's signed tip (`wt`, its Place's path and tip words), on its branch; his
 * checkout by the project folder's name and 'your own folder: <path>', on the checkout's branch;
 * nothing for bise's own main. Pure.
 */
export function tabPlace(
  a: { branch?: string | null; checkout_branch?: string | null; worktree?: string | null } | undefined,
  home: boolean,
  wt: { path?: string | null; tip: string } | undefined,
  project: string | undefined,
  tilde: (path: string) => string,
): TabPlace | undefined {
  if (a === undefined || home) return undefined;
  const own = a.branch != null && a.branch !== '' ? a.branch : undefined;
  if (wt !== undefined && wt.path != null && wt.path !== '') return { name: worktreeName(wt.path), worktree: true, tip: wt.tip, branch: own };
  if (a.worktree != null && a.worktree !== '') return { name: worktreeName(a.worktree), worktree: true, tip: wt?.tip ?? composer.tab_worktree_at({ path: tilde(a.worktree) }), branch: own };
  if (own !== undefined) return { name: own, worktree: true, tip: wt?.tip ?? composer.tab_worktree_on({ branch: own }), branch: own };
  if (project === undefined || project === '') return undefined;
  const co = a.checkout_branch != null && a.checkout_branch !== '' ? a.checkout_branch : undefined;
  return { name: folderName(project), worktree: false, tip: composer.tab_own_folder({ path: tilde(project) }), branch: co };
}

/** Which of the strip's items show. */
export interface StripFit {
  /** the addressee's state while it runs ('working · 3 min', 'waits for you') */
  readonly state: boolean;
  /** the right end: the context ring, or the screen's words */
  readonly end: boolean;
  /** a trunk project's 'lands on main' after the branch (designer m_43996) */
  readonly lands: boolean;
  /** the branch: cut with its ellipsis past the rest, gone whole below BRANCH_MIN */
  readonly branch: boolean;
}

/** The least room the branch keeps: its icon and a few letters; below, it goes whole (ui-review
 *  m_47192: its icon and one clipped letter at 680 with a panel open). */
export const BRANCH_MIN = 56;

/**
 * The strip's items that fit in `avail` px, each whole or not at all: 'lands on main' goes first,
 * then the right end, then the state; the branch stays, cut by its css past that, until even BRANCH_MIN doesn't fit. `avail` is what 'to perf' leaves: it never
 * goes. `gap` is the space between two items (16); a width of 0 is an item that isn't there.
 * Every item is counted: one left out (ui-review m_47192: 'lands' was) is squeezed to a sliver by
 * the css while the ones after it stay. Pure.
 */
export function stripFit(w: { avail: number; gap: number; state: number; branch: number; end: number; lands?: number }): StripFit {
  const sum = (on: StripFit): number => {
    const items = [on.state ? w.state : 0, on.branch ? w.branch : 0, on.lands ? (w.lands ?? 0) : 0, on.end ? w.end : 0].filter((x) => x > 0);
    return items.reduce((n, x) => n + x, 0) + w.gap * Math.max(0, items.length - 1);
  };
  const tries: StripFit[] = [
    { state: true, end: true, lands: true, branch: true },
    { state: true, end: true, lands: false, branch: true },
    { state: true, end: false, lands: false, branch: true },
  ];
  for (const t of tries) if (sum(t) <= w.avail) return t;
  return { state: false, end: false, lands: false, branch: w.branch === 0 || w.avail >= Math.min(w.branch, BRANCH_MIN) };
}

/** The addressee's state in the strip (his m_44719): it waits for him, or it works and for how long; null idle. */
export type StripState = { readonly kind: 'waits' } | { readonly kind: 'working'; readonly ms: number };

/**
 * What the strip says of the agent his words go to, where 'local' was (his m_44719: the only
 * option, so no control): something of its waits on him first, then its turn running (how long:
 * the hub's turn_ms, else since its status changed); idle, stopped or unknown: nothing. Pure.
 */
export function stripState(a: { status: string; waits: number; since_ms: number; turn_ms?: number | null; waiting_on?: { who: string } | null } | undefined, now: number): StripState | null {
  if (a === undefined) return null;
  if (a.waits > 0 || a.waiting_on?.who === 'you') return { kind: 'waits' };
  if (a.status === 'working') return { kind: 'working', ms: a.turn_ms ?? Math.max(0, now - a.since_ms) };
  return null;
}

/**
 * The label cut to fit `room` px after a whole word, with the one '…' (designer m_24325: never
 * mid-word): the whole label when it fits; the longest run of whole words that fits with the '…'
 * (a trailing '·' or ',' goes with the cut); its first word and the '…' when not even that fits
 * (the tab's own clip then). `width` measures a string in the label's font. Pure.
 */
export function wordCut(label: string, room: number, width: (s: string) => number): string {
  if (width(label) <= room) return label;
  const words = label.split(' ');
  for (let n = words.length - 1; n >= 1; n--) {
    const head = words.slice(0, n).join(' ').replace(/[\s·,:;]+$/u, '');
    if (head === '') continue;
    const cut = `${head}…`;
    if (width(cut) <= room) return cut;
  }
  return `${words[0] ?? ''}…`;
}

/** The answering tab's head ('↳ answering search'), the label up to its first ' · ': never cut. */
export function answerHead(label: string): string {
  const i = label.indexOf(' · ');
  return i < 0 ? label : label.slice(0, i);
}

/**
 * The answering tab in `room` px (composer-target A, designer m_26531): the label and its end ('esc
 * to bise instead', `endW` px with its gap) both whole when they fit; short, the question goes first
 * (cut after a whole word, down to '↳ answering search…'), then the end; the head itself never goes
 * (past that, the tab's own clip). Pure.
 */
export function answerFit(label: string, room: number, endW: number, width: (s: string) => number, whole = false): { label: string; end: boolean } {
  const head = answerHead(label);
  // `whole` (an open artifact's '↳ to perf · about “…”', designer m_27021): the part after the head
  // shows whole or drops whole, never 'about…'; the end goes only once the head alone needs its room
  if (whole) {
    if (width(label) <= room - endW) return { label, end: true };
    return { label: head, end: width(head) <= room - endW };
  }
  const keeps = (cut: string) => cut === label || (cut.startsWith(head) && Array.from(cut).length > Array.from(head).length);
  const withEnd = wordCut(label, room - endW, width);
  if (keeps(withEnd) && width(withEnd) <= room - endW) return { label: withEnd, end: true };
  const alone = wordCut(label, room, width);
  return { label: keeps(alone) ? alone : head, end: false };
}

/** A row of the strip's addressee picker: the agent, the name the window gives it, its state's
 *  sign as the sidebar draws it ('?' asks him, '∿' works, '…' paused or waiting, '✓' done, '✗'
 *  failed; '' at rest). */
export interface AgentPick {
  readonly name: string;
  readonly shown: string;
  readonly sign: '?' | '∿' | '…' | '✓' | '✗' | '';
}

/**
 * The strip's addressee picker's rows (his m_47126, designer m_47129): the project's live agents,
 * main first ('bise'), then the others in the hub's rank (the sidebar's ⌥ order), each with its
 * state's sign. Archived agents never. Pure.
 */
export function agentPicks(agents: readonly Agent[], now: number): AgentPick[] {
  const live = liveOf(agents);
  const rank = rankedOf(live);
  const at = (a: Agent) => rank.get(a.name) ?? Number.MAX_SAFE_INTEGER;
  return live
    .map((a, i) => ({ a, i }))
    .sort((x, y) => at(x.a) - at(y.a) || x.i - y.i)
    .map(({ a }) => ({ name: a.name, shown: shownName(a.name), sign: pickSign(signOf(a, now).g) }));
}

function pickSign(g: string): AgentPick['sign'] {
  if (g === '?' || g === '∿' || g === '✓' || g === '✗') return g;
  if (g === '…' || g === '⋯') return '…';
  return '';
}

/** What makes the composer pink, and its border's tip; `tip` undefined: plain. */
export interface PinkState {
  readonly pink: boolean;
  readonly tip: string | undefined;
  /** the tab holds only its label (an open item, the pink artifact): no place, no context */
  readonly bare: boolean;
  /** the label's part after its head whole or gone (the pink artifact's '· about “…”') */
  readonly whole: boolean;
  /** the border alone, no head tint, the send plain until he types (an agent that is not main,
   *  designer m_28208; the border his m_47126) */
  readonly quiet: boolean;
  /** main's open question: the box plain (his words go to main), its label still in the accent
   *  (designer m_47129) */
  readonly asks: boolean;
}

const PLAIN: PinkState = { pink: false, tip: undefined, bare: false, whole: false, quiet: false, asks: false };

/**
 * The composer's one pink state (composer-target A's accent tab, 1.5 px border and send), whoever
 * sets it. The rule (his m_47126): the border is pink whenever his words go to an agent that is not
 * the project's main, never for main. An open item wins: a question pink with its tip (main's
 * plain, its label in the accent: `asks`), a closed one or an approval the border alone when its
 * asker is not main, else plain. Then an open artifact: its maker's pink (`aboutPink`, art-target),
 * main's or an archived maker's plain. Then a thread or a bottom block whose words go to an agent
 * that is not main (agent-target, his ask: he wrote to the wrong agent without seeing it): the
 * border alone (`quiet`: no head tint, designer m_28208, m_47129), its place and context kept.
 * Voice typing stays plain, as art-target. His words opening with `@<agent>` (at-start,
 * thread/lead.ts), read only with no item and no artifact open: that agent's pink, bare, with
 * `leadTip`. Pure.
 */
export function pinkState(o: {
  answer: { asker?: string; closed?: boolean; free?: 'deny' | 'steer'; tip?: string } | null;
  /** the border's tip while his words open with another agent's `@name` (lead.ts); undefined: they don't */
  leadTip?: string | undefined;
  aboutPink: string | undefined;
  about: boolean;
  agent: string;
  voiceTyping: boolean;
  otherTip: (agent: string) => string;
}): PinkState {
  const a = o.answer;
  if (a !== null) {
    const toMain = a.asker === MAIN;
    if (a.closed !== true && a.free === undefined) return toMain ? { ...PLAIN, bare: true, asks: true } : { ...PLAIN, pink: true, tip: a.tip, bare: true };
    if (a.asker === undefined || toMain) return { ...PLAIN, bare: true };
    return { ...PLAIN, pink: true, tip: o.otherTip(a.asker), bare: true, quiet: true };
  }
  if (o.leadTip !== undefined) return { ...PLAIN, pink: true, tip: o.leadTip, bare: true };
  if (o.aboutPink !== undefined) return { ...PLAIN, pink: true, tip: o.aboutPink, bare: true, whole: true };
  if (o.about || o.voiceTyping || o.agent === MAIN) return PLAIN;
  return { ...PLAIN, pink: true, tip: o.otherTip(o.agent), quiet: true };
}

/** The context as the hub says it ('42k · 17%', or named 'context 42k · 17%'): its tokens and its
 *  percent; null when it doesn't read so. Pure. */
export function contextOf(words: string | undefined): { tokens: string; pct: number } | null {
  const m = /^(?:context\s+)?(\S+)\s*·\s*(\d+(?:\.\d+)?)%$/u.exec((words ?? '').trim());
  if (m === null) return null;
  const pct = Number(m[2]);
  return Number.isFinite(pct) ? { tokens: m[1] ?? '', pct: Math.min(100, Math.max(0, pct)) } : null;
}

/** Above this percent the ring's numbers show in clear beside it (designer m_24404 touch 4). */
export const RING_CLEAR_PCT = 80;

/** The ring's tip: '17% of perf's context used · 42k tokens · compacts at 90%'. Pure. */
export function ringTip(who: string, c: { tokens: string; pct: number }): string {
  return composer.ring_tip({ pct: Math.round(c.pct), agent: who, tokens: c.tokens });
}
