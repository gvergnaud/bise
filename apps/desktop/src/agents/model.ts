// The agents list's words and order (bar A.2: the same agents and
// statuses as the TUI's panel, live). Owner: amb-win. Pure: the list
// component reads the store's agents and draws these.
import type { Agent, Status } from '@/proto';
import type { DotState } from '@/ds';
import { pages } from '@/proto/words_pages.gen';

/** A status as the ds's dot: at work, needs him, done, failed, at rest. */
export function dotOf(s: Status): DotState {
  switch (s) {
    case 'working':
      return 'wk';
    case 'waiting':
    case 'blocked':
      return 'nd';
    case 'done':
      return 'ok';
    case 'failed':
      return 'er';
    case 'idle':
      return 'off';
  }
}

/**
 * Its turn runs, for his composer: working, or waiting on a reply inside its turn (sb wait, sb ask,
 * a --then pause), as rows::Agent::turn_running counts it since issue 22. Then tab queues his words
 * for the turn's end (sb-core holds them while its run is busy), ⏎ steers, the send button stops.
 * Pure.
 */
export function turnRuns(s: Status | undefined): boolean {
  return s === 'working' || s === 'waiting';
}

/**
 * A length of time, short, the same everywhere (ambient m_12439: the
 * sidebar's turn time, the head's age, the inbox card's age()): "now",
 * "4 min", "2 h", "3 d". Pure.
 */
export function shortAge(ms: number): string {
  const m = Math.floor(Math.max(0, ms) / 60_000);
  if (m < 1) return 'now';
  if (m < 60) return `${String(m)} min`;
  const h = Math.floor(m / 60);
  return h < 24 ? `${String(h)} h` : `${String(Math.floor(h / 24))} d`;
}

/** How long ago, short (shortAge). */
export function ago(ms: number, now: number): string {
  return shortAge(now - ms);
}

/** A row's end: what waits on him first (accent), else its state or age. */
export function endOf(a: Agent, now: number): { text: string; needs: boolean } {
  // what waits on him says 'waits' in the accent, as the right bar's row (designer m_21173)
  if (a.waits > 0) return { text: 'waits', needs: true };
  // R9/S3: whom it waits on (the hub's waiting_on): him in accent, another agent dim
  if (a.waiting_on?.who === 'you') return { text: 'waiting on you', needs: true };
  if (a.waiting_on?.who === 'agent') return { text: `waiting on ${shownName(a.waiting_on.name)}`, needs: false };
  // wt-disk: its worktree waits for disk; it starts on its own, nothing of his (designer m_67943)
  if (a.phase === 'waiting_disk') return { text: pages.waiting_disk, needs: false };
  switch (a.status) {
    case 'working':
      return { text: ago(a.since_ms, now), needs: false };
    case 'waiting':
    case 'blocked':
      return { text: a.status, needs: true };
    case 'done':
      return { text: 'done', needs: false };
    case 'failed':
      return { text: 'failed', needs: true };
    case 'idle':
      return { text: ago(a.since_ms, now), needs: false };
  }
}

/**
 * An archived agent's end on /archived, dim (designer m_19489): how it ended, then how long ago:
 * '✓ done · 2 d' for one that finished, '– stopped · 3 h' for any other. Pure.
 */
export function archivedEnd(a: Agent, now: number): string {
  return `${a.status === 'done' ? '✓ done' : '– stopped'} · ${ago(a.since_ms, now)}`;
}

/**
 * The live list, main first, then the hub's order (start order): rankedOf
 * numbers it by the hub's rank. Archived agents are not in it (`archivedOf`).
 */
export function liveOf(agents: readonly Agent[]): Agent[] {
  return agents.filter((a) => !a.archived && a.main).concat(agents.filter((a) => !a.archived && !a.main));
}

/** An agent's ⌥ number: main 0, the others from 1. */
export type Slots = ReadonlyMap<string, number>;

/**
 * The hub's one order as ⌥ numbers (agent-rank, architect m_39150): each live row's `rank`
 * (switchboard proto_view/rank.rs; the TUI's panel reads the same field, rust/tui/src/sb/numbers.rs):
 * main 0, the others 1..n by rank; a row with none (an older hub) after them, in start order.
 * Nothing is ranked here. Pure.
 */
export function rankedOf(live: readonly Agent[], pinned: readonly string[] = []): Map<string, number> {
  // the agents he pinned (app/pins.ts) first, a stable partition: each group keeps the hub's rank
  const pin = (a: Agent) => (pinned.includes(a.name) ? 0 : 1);
  const others = live
    .filter((a) => !a.main)
    .map((a, i) => ({ a, i }))
    .sort((x, y) => pin(x.a) - pin(y.a) || (x.a.rank ?? Number.MAX_SAFE_INTEGER) - (y.a.rank ?? Number.MAX_SAFE_INTEGER) || x.i - y.i);
  const out = new Map<string, number>();
  for (const a of live) if (a.main) out.set(a.name, 0);
  others.forEach(({ a }, k) => out.set(a.name, k + 1));
  return out;
}

/**
 * `next` held to the numbers he sees (`shown`): the ones still there keep theirs, a gone one frees
 * its number, a newcomer goes after the last. Nothing shown yet: `next`. Pure.
 */
export function heldNumbers(shown: Slots | undefined, next: Slots): Map<string, number> {
  if (shown === undefined || shown.size === 0) return new Map(next);
  const out = new Map([...shown].filter(([name]) => next.has(name)));
  let last = Math.max(0, ...out.values());
  for (const [name, n] of next) {
    if (out.has(name)) continue;
    out.set(name, n === 0 ? 0 : ++last);
  }
  return out;
}

/** How long the numbers hold still after an ⌥N jump (his ⌥1, ⌥2, ⌥3 often lets go of ⌥ between keys). */
export const HOLD_MS = 5_000;

/** Whether the numbers hold still: ⌥ held, or an ⌥N jump less than HOLD_MS ago. Pure. */
export function holds(alt: boolean, jumped: number | undefined, now: number): boolean {
  return alt || (jumped !== undefined && now - jumped < HOLD_MS);
}

const shownBy = new Map<string, Slots>();
let jumpedAt: number | undefined;

/** An ⌥N jump now: the numbers hold still HOLD_MS. */
export function noteJump(now = Date.now()): void {
  jumpedAt = now;
}

/**
 * The project's ⌥ numbers now, as the window shows them: the hub's rank (rankedOf), held while ⌥
 * is held (`alt`) and a moment after an ⌥N jump (heldNumbers). The sidebar shows them, ⌥1-9 reads them.
 * `pinned`: the project's pinned agents (app/pins.ts), first: ⌥N is the Nth row he sees.
 */
export function numbersOf(project: string, agents: readonly Agent[], alt = false, now = Date.now(), pinned: readonly string[] = []): Slots {
  const next = rankedOf(liveOf(agents), pinned);
  const out = holds(alt, jumpedAt, now) ? heldNumbers(shownBy.get(project), next) : next;
  shownBy.set(project, out);
  return out;
}

/** The numbers the window shows now (numbersOf's last), without moving them: the tour's words. */
export function shownOf(project: string, agents: readonly Agent[]): Slots {
  return shownBy.get(project) ?? rankedOf(liveOf(agents));
}

/** The archived ones, newest first (/archived). */
export function archivedOf(agents: readonly Agent[]): Agent[] {
  return agents.filter((a) => a.archived).sort((a, b) => b.since_ms - a.since_ms || a.name.localeCompare(b.name));
}

/** One line of what it does: its title, else its purpose. */
export function lineOf(a: Agent): string {
  return a.title !== '' ? a.title : a.purpose;
}

/**
 * An agent's name as the window shows it: a project's main is 'bise' (ambient m_11437, lead
 * m_11431); every other agent keeps its name. Display only: ids, routes, test ids, the hub and
 * the git branch 'main' never change. Pure.
 */
export function shownName(agent: string): string {
  return agent === 'main' ? 'bise' : agent;
}
