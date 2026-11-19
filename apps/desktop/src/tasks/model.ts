// The tasks' pure model for the desktop: the order of the statuses (the Board's columns, signed by
// designer and tasks-ux), the right bar's rows (open first, then a fold for the ongoing ones and the
// ones done this week), its counts, a row's done-when count, a name cut at a word. Pure. Owner: tasks.
import type { Agent, Artifact, LogLine, Part, PrReview, PrState, WorkRow, WorkStatus, WorkTomb } from '@/proto';

/** A task with its project (the bar lists every project's on bise's own). */
export interface ProjWork {
  readonly project: string;
  readonly projectName: string;
  readonly w: WorkRow;
}

/** The Board's column order (signed): Needs you · Working · Waiting · Stalled · To review · Done this week · Paused · Ongoing. */
export const STATUS_ORDER: readonly WorkStatus[] = ['needs_you', 'working', 'waiting', 'stalled', 'to_review', 'done', 'paused', 'ongoing', 'unknown'];

/** a status's rank in the bar's open rows: the Board's order, Paused right after To review (Done and Ongoing fold) */
const OPEN_RANK: Readonly<Record<WorkStatus, number>> = {
  needs_you: 0,
  working: 1,
  waiting: 2,
  stalled: 3,
  to_review: 4,
  paused: 5,
  unknown: 6,
  done: 7,
  ongoing: 8,
};

/** done this week: the bar's fold keeps them, older ones are tombs already (the hub folds them at 7 days) */
const WEEK_MS = 7 * 24 * 3_600_000;

/** Open: not done and not ongoing (an ongoing task is never done, it folds with the done ones). */
export const isOpen = (w: Pick<WorkRow, 'status'>): boolean => w.status !== 'done' && w.status !== 'ongoing';

/** What waits on him: its status is Needs you. */
export const needsHim = (w: Pick<WorkRow, 'status'>): boolean => w.status === 'needs_you';

/** Newest fact first, then the name: a stable order inside a status. */
function byLast(a: ProjWork, b: ProjWork): number {
  return b.w.last_ms - a.w.last_ms || a.w.name.localeCompare(b.w.name);
}

/** The bar's rows: the open ones in the status order; then the fold, ongoing ones then the done this week. Pure. */
export function barRows(all: readonly ProjWork[], now: number): { readonly open: readonly ProjWork[]; readonly ongoing: readonly ProjWork[]; readonly done: readonly ProjWork[] } {
  const open = all.filter((x) => isOpen(x.w)).sort((a, b) => OPEN_RANK[a.w.status] - OPEN_RANK[b.w.status] || byLast(a, b));
  const ongoing = all.filter((x) => x.w.status === 'ongoing').sort(byLast);
  const done = all
    .filter((x) => x.w.status === 'done' && now - (x.w.done?.at_ms ?? x.w.last_ms) < WEEK_MS)
    .sort((a, b) => (b.w.done?.at_ms ?? b.w.last_ms) - (a.w.done?.at_ms ?? a.w.last_ms));
  return { open, ongoing, done };
}

/** A row he checked or unchecked in the todo list, kept where it was while the list is in view
 *  (designer m_66969): `at` its place then, `checked` what his box says until the hub's row agrees. */
export interface Kept {
  readonly x: ProjWork;
  readonly at: number;
  readonly checked: boolean;
}

/** The todo list's rows: the open ones, and the ones he checked stay at their place, checked, so
 *  nothing moves under his pointer (they reach the 'done lately' fold at the next draw); one he
 *  reopened shows unchecked at its place until the hub's row is open again. Done by someone
 *  else: gone at once (barRows). Pure. */
export function listRows(open: readonly ProjWork[], kept: ReadonlyMap<string, Kept>): { readonly x: ProjWork; readonly checked: boolean }[] {
  const key = (x: ProjWork) => `${x.project}/${x.w.id}`;
  const openKeys = new Set(open.map(key));
  const rows = open.filter((x) => kept.get(key(x))?.checked !== true).map((x) => ({ x, checked: false }));
  const extra = [...kept.entries()].filter(([k, v]) => v.checked || !openKeys.has(k)).map(([, v]) => v).sort((a, b) => a.at - b.at);
  for (const k of extra) rows.splice(Math.min(k.at, rows.length), 0, { x: k.x, checked: k.checked });
  return rows;
}

/** The Board's statuses as one string ('p/a=working p/b=done'; ids and statuses have no space):
 *  a change of any status changes it. Pure. */
export function statusSig(all: readonly ProjWork[]): string {
  return all.map((x) => `${x.project}/${x.w.id}=${x.w.status}`).join(' ');
}

/** The cards whose status changed between two sigs (tasks-shine batch 5: they fade in in their
 *  new column); a new or a gone task is no move. Pure. */
export function movedKeys(was: string, now: string): ReadonlySet<string> {
  const parse = (s: string) => new Map(s === '' ? [] : s.split(' ').map((l) => [l.slice(0, l.lastIndexOf('=')), l.slice(l.lastIndexOf('=') + 1)] as const));
  const a = parse(was);
  return new Set([...parse(now)].filter(([k, st]) => a.has(k) && a.get(k) !== st).map(([k]) => k));
}

/** A row's done-when count: ticked of all; null when it has none. Pure. */
export function goalsOf(w: Pick<WorkRow, 'goals'>): { readonly done: number; readonly all: number } | null {
  const g = w.goals ?? [];
  if (g.length === 0) return null;
  return { done: g.filter((x) => x.done === true).length, all: g.length };
}

/** A task id as a link reads it: a merged one's, the task it went into (its tombs, followed to the end). Pure. */
export function resolveMerged(tombs: readonly Pick<WorkTomb, 'id' | 'merged_into'>[], id: string): string {
  const into = new Map(tombs.flatMap((t) => (t.merged_into == null ? [] : [[t.id, t.merged_into] as const])));
  let at = id;
  // a chain of merges ends; a loop (never written by the hub) stops after as many steps as tombs
  for (let n = 0; n <= into.size; n++) {
    const next = into.get(at);
    if (next === undefined) return at;
    at = next;
  }
  return at;
}

/** A name cut at a word with '…' when longer than `max` letters (the whole name goes in its tip); a
 *  first word longer than `minWord` letters... is cut inside when no space comes after `minWord`. Pure. */
export function cutAtWord(name: string, max: number, minWord = 8): string {
  if (name.length <= max) return name;
  const head = name.slice(0, max);
  const at = head.lastIndexOf(' ');
  return `${(at >= minWord && at > 0 ? head.slice(0, at) : head).trimEnd()}…`;
}

/** The artifacts that serve task `id` (a merged task's read as the one it went into), archived ones out,
 *  newest first; `mine` the ones for him (an older hub's, none said: his), else all of them. Pure. */
export function artsOf<A extends Pick<Artifact, 'works' | 'at_ms' | 'archived' | 'audience'> & { readonly pr?: Artifact['pr'] }>(arts: readonly A[], id: string, tombs: readonly Pick<WorkTomb, 'id' | 'merged_into'>[]): { readonly all: readonly A[]; readonly mine: readonly A[] } {
  // its PRs are its Changes' rows, never chips too (designer m_66722: nothing twice)
  const all = arts.filter((a) => a.archived !== true && a.pr == null && serves(a, id, tombs)).sort((a, b) => b.at_ms - a.at_ms);
  const his = all.filter((a) => a.audience !== 'agents');
  return { all, mine: his.length > 0 ? his : all };
}

/** An artifact serves task `id`: its works hold it (a merged task's read as the one it went into). Pure. */
function serves(a: Pick<Artifact, 'works'>, id: string, tombs: readonly Pick<WorkTomb, 'id' | 'merged_into'>[]): boolean {
  return (a.works ?? []).some((w) => resolveMerged(tombs, w) === id);
}

/** A PR of a task as its Changes list draws it (his pick on page tasks-fresh, designer m_66712). */
export interface TaskPr {
  readonly id: string;
  /** '#924' (a merge request '!924') */
  readonly tag: string;
  readonly title: string;
  /** the forge's state now, else the last one the hub kept (ArtifactPr.last); unknown: never said */
  readonly state: PrState;
  readonly review: PrReview;
  /** when the state was seen (ms), the last one kept's; null with the live status */
  readonly at_ms: number | null;
  /** the hub follows it and the forge answers late or not at all: its last known state, 'as of' */
  readonly stale: boolean;
}

const PR_RANK: Readonly<Record<PrState, number>> = { open: 0, draft: 0, unknown: 0, merged: 1, closed: 2 };

/** Task `id`'s PRs: the PR artifacts whose works hold it, archived ones too (a merged PR is
 *  archived by the hub), open and draft first, then merged, then closed, newest first in each. Pure. */
export function prsOf(arts: readonly Pick<Artifact, 'id' | 'title' | 'works' | 'at_ms' | 'pr'>[], id: string, tombs: readonly Pick<WorkTomb, 'id' | 'merged_into'>[]): TaskPr[] {
  return arts.flatMap((a) => {
    const pr = a.pr;
    if (pr == null || !serves(a, id, tombs)) return [];
    const live = pr.status ?? null;
    const last = pr.last ?? null;
    const p: TaskPr = {
      id: a.id,
      tag: pr.tag ?? `#${String(pr.number)}`,
      title: live?.title != null && live.title !== '' ? live.title : a.title,
      state: live?.state ?? last?.state ?? 'unknown',
      review: live?.review ?? last?.review ?? 'none',
      at_ms: live === null ? (last?.at_ms ?? null) : null,
      stale: live?.stale_ms != null,
    };
    return [{ p, at: a.at_ms }];
  }).sort((x, y) => PR_RANK[x.p.state] - PR_RANK[y.p.state] || y.at - x.at).map((x) => x.p);
}

/** The set's words for a tip ('2 PRs · 1 open · 1 merged') and whether every one is merged (one at
 *  least, none open: the bar's merged mark in place of its n/m; sb-core's own rule, wstatus merged). Pure. */
export function prSet(prs: readonly Pick<TaskPr, 'state'>[]): { readonly n: number; readonly open: number; readonly merged: number; readonly closed: number; readonly allMerged: boolean } {
  const open = prs.filter((p) => p.state === 'open' || p.state === 'draft').length;
  const merged = prs.filter((p) => p.state === 'merged').length;
  const closed = prs.filter((p) => p.state === 'closed').length;
  return { n: prs.length, open, merged, closed, allMerged: merged > 0 && open === 0 };
}

/** A message's first line (his origin is quoted by it). Pure. */
export function firstLine(text: string): string {
  return text.trim().split('\n')[0]?.trim() ?? '';
}

/** His message as the Task tab quotes it: his own words read from its parts (never the model's
 *  template text: tags, paths; his m_65766), and his images; an older entry with no parts, its text. */
export interface Origin {
  readonly words: string;
  readonly images: readonly { readonly name: string; readonly path: string; readonly b64: string; readonly mime: string }[];
}

/** The most letters of his words the quote shows: about two lines of the tab; the rest is in his thread. */
export const ORIGIN_MAX = 160;

/** His first line of words, cut at a word, and his images; null when he wrote no words (an image
 *  alone: no quote line, never an empty one; architect m_65793). Pure. */
export function originOf(e: { readonly text: string; readonly parts?: readonly Part[] | null }): Origin | null {
  const parts = e.parts ?? [];
  const typed = parts.length > 0;
  let said = typed ? '' : e.text;
  const images: Origin['images'][number][] = [];
  for (const p of parts) {
    if (p.kind === 'text') said += p.text;
    else if (p.kind === 'plugin') said += `@${p.name}`;
    else if (p.kind === 'image') images.push({ name: p.name, path: p.source, b64: p.b64_file ?? '', mime: p.mime ?? '' });
  }
  const words = cutAtWord(firstLine(said.replace(/[ \t]{2,}/gu, ' ')), ORIGIN_MAX);
  return words === '' ? null : { words, images };
}

/** A report's first sentence (its kind's prefix out), the Task tab's Now and an agent's line: the
 *  rest is in the Log (tasks-ux m_65775). Pure. */
export function firstSentence(text: string): string {
  const one = firstLine(text.replace(/^\[report: [a-z]+\]\s*/u, ''));
  const m = /^(.+?[.!?])(\s|$)/u.exec(one);
  return (m?.[1] ?? one).trim();
}

/** The most letters of a Now line or an agent's line: two lines at most; the whole one in its tip. */
export const SAID_MAX = 200;

/** Its agents' newest report on THIS task, by agent, from its Log (newest first): an agent on many
 *  tasks says here only what it reported on this one (tasks-ux m_65775). Pure. */
export function reportsOf(lines: readonly Pick<LogLine, 'kind' | 'who' | 'text' | 'at_ms'>[]): ReadonlyMap<string, { readonly text: string; readonly at: number }> {
  const out = new Map<string, { text: string; at: number }>();
  for (const l of lines) {
    const text = (l.text ?? '').trim();
    if (l.kind !== 'report' || text === '') continue;
    const was = out.get(l.who);
    if (was === undefined || l.at_ms > was.at) out.set(l.who, { text, at: l.at_ms });
  }
  return out;
}

/** The agents that serve no other open task than `id`: their status note is about this one. Pure. */
export function soloOn(works: readonly Pick<WorkRow, 'id' | 'status' | 'agents'>[], id: string, names: readonly string[]): ReadonlySet<string> {
  return new Set(names.filter((n) => !works.some((o) => o.id !== id && isOpen(o) && (o.agents ?? []).includes(n))));
}

/** An agent's line on this task: its newest report on it (first sentence), else its status note
 *  when it serves only this task, else nothing; cut at a word. Pure. */
export function agentLineOf(name: string, reports: ReadonlyMap<string, { readonly text: string }>, a: Pick<Agent, 'note' | 'title' | 'status'> | undefined, solo: boolean): string {
  const r = reports.get(name);
  const said = r !== undefined ? firstSentence(r.text) : a !== undefined && solo ? doingOf(a) : '';
  return cutAtWord(said, SAID_MAX);
}

/** The task's Now: its agents' newest report on it (first sentence), else a note of an agent on it
 *  alone, else null; cut at a word. Pure. */
export function nowOfTask(reports: ReadonlyMap<string, { readonly text: string; readonly at: number }>, solos: readonly Pick<Agent, 'note'>[]): string | null {
  const newest = [...reports.values()].sort((a, b) => b.at - a.at)[0];
  const said = newest !== undefined ? firstSentence(newest.text) : (solos.map((a) => (a.note ?? '').trim()).find((n) => n !== '') ?? '');
  return said === '' ? null : cutAtWord(said, SAID_MAX);
}

/** A Board card's agents: the first one and how many more, never main (designer m_65805:
 *  'auth-fix +2', never every name); '' none. Pure. */
export function cardCrew(names: readonly string[]): string {
  const rest = names.filter((n) => n !== 'main');
  const first = rest[0];
  return first === undefined ? '' : rest.length === 1 ? first : `${first} +${String(rest.length - 1)}`;
}

/** A Board card's sentence: the newest report of an agent on this task alone (a report of an agent
 *  on many tasks may be another's: only the Task tab, which reads this task's Log, shows it;
 *  tasks-ui-qa m_65799 #4), with its time; null: none said. Pure. */
export function cardSaid(agents: readonly (Pick<Agent, 'name' | 'report' | 'note'>)[], solo: ReadonlySet<string>): { readonly text: string; readonly at: number | null } | null {
  const said = saidOf(agents.filter((a) => solo.has(a.name)));
  return said === null ? null : { text: firstSentence(said.text), at: said.at };
}

/** An agent's state on this task: waits for him only for an open card of this task from it
 *  (tasks-ux m_65775: never another task's card). Pure. */
export function agentStateOf(a: Pick<Agent, 'status'> | undefined, waitsHere: boolean): 'archived' | 'working' | 'waits' | 'idle' {
  if (a === undefined) return 'archived';
  if (a.status === 'working') return 'working';
  return waitsHere ? 'waits' : 'idle';
}

/** The agents a Task tab lists (tasks-ux m_64959): main starts the work, it is not on the task, so it
 *  shows only when it is the task's only agent or in a turn for it. Pure. */
export function crewOf(names: readonly string[], working: (name: string) => boolean): string[] {
  const rest = names.filter((n) => n !== 'main');
  return names.filter((n) => n !== 'main' || rest.length === 0 || working(n));
}

/** What an agent does now, one line: its status note, else its turn's title, else nothing. Pure. */
export function doingOf(a: Pick<Agent, 'note' | 'title' | 'status'>): string {
  const note = (a.note ?? '').trim();
  if (note !== '') return note;
  return a.status === 'working' ? a.title.trim() : '';
}

/** A task's Now with its time: the newest report of its agents (`at` its time), else a status note
 *  (no time); null: nothing said. Pure. */
export function saidOf(agents: readonly Pick<Agent, 'report' | 'note'>[]): { readonly text: string; readonly at: number | null } | null {
  const reps = agents.flatMap((a) => (a.report == null || a.report.text.trim() === '' ? [] : [a.report])).sort((a, b) => b.at_ms - a.at_ms);
  const r = reps[0];
  if (r !== undefined) return { text: firstLine(r.text.replace(/^\[report: [a-z]+\]\s*/, '')), at: r.at_ms };
  const note = agents.map((a) => (a.note ?? '').trim()).find((n) => n !== '');
  return note === undefined ? null : { text: note, at: null };
}

/** A task's Now: the newest report of its agents, else a status note; null: nothing said. Pure. */
export function nowOf(agents: readonly Pick<Agent, 'report' | 'note'>[]): string | null {
  return saidOf(agents)?.text ?? null;
}

/** A task line's part past its status word ('waiting · no agent yet' → 'no agent yet'); '' when the
 *  line is the status word alone (the pill says it). Pure. */
export function aboutOf(line: string): string {
  const at = line.indexOf(' · ');
  return at < 0 ? '' : line.slice(at + 3);
}
