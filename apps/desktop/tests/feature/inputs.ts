// A feature test's inputs (fast-try p5, architect m_29407), typed from bise-proto's generated
// types: the core's and the hubs' events, the window-local answers main sends the window that
// asked (src/main/core/ipc.ts), and his gestures. Every default comes from that tag's first line
// in src/proto/fixtures.gen.ts (rust/proto/fixtures), never a hand-kept value, so a proto change
// breaks a builder or a test at typecheck and the compiler shows what to fix. Test-only (never in
// the bundle): a feature test imports this and run.ts, nothing else of the app (eslint).
import type { AnswerResult, Artifact, Card, Folder, CoreEv, CoreCmd, HubEv, Mode, Part, ProjectRow, RailPlace, Route, SendReason } from './app';
import { core_ev, hub_ev } from './app';

export type Ev = HubEv | CoreEv;
export type Tag = Ev['ev'];
export type EvOf<T extends Tag> = Extract<Ev, { ev: T }>;
/** An event's fields over its fixture's (Partial distributes over a tag with several shapes). */
export type Over<T extends Tag> = Partial<Omit<EvOf<T>, 'ev'>>;

const FIXTURES: readonly Ev[] = [...hub_ev, ...core_ev];

/** The first fixture line of `tag` (a tag without one has no defaults: give them all). */
function fixtureOf<T extends Tag>(tag: T): EvOf<T> | null {
  return (FIXTURES.find((e) => e.ev === tag) as EvOf<T> | undefined) ?? null;
}

/**
 * One event of the core or a hub: its fixture's fields, then `over`'s. `ev('cards', { project,
 * cards: [card({ id: 9 })], others: [] })`. A tag no fixture has: `over` must hold every field
 * (the cast is checked by the fixtures test: every Tag has a fixture today).
 */
export function ev<T extends Tag>(tag: T, over: Over<T> = {}): EvOf<T> {
  return { ...(fixtureOf(tag) ?? {}), ...over, ev: tag } as EvOf<T>;
}

/** The first card of the fixtures' `cards` event. */
const CARD: Card = (() => {
  const c = fixtureOf('cards')?.cards[0];
  if (c === undefined) throw new Error('rust/proto/fixtures has no cards event with a card');
  return c;
})();

/** A card: the fixtures' first, then `over` (give `project` and `id`). */
export function card(over: Partial<Card> & Pick<Card, 'project' | 'id'>): Card {
  return { ...CARD, ...over };
}

/** The fixtures' first project row that has cards (the ProjectRow every default comes from), without
 * its pulse: a default row is a hub that sends none yet; a test that wants one gives it. */
const ROW: ProjectRow = (() => {
  for (const e of core_ev) {
    if (e.ev !== 'projects') continue;
    const r = e.projects.find((p) => p.cards !== undefined);
    if (r !== undefined) return { ...r, pulse: null };
  }
  throw new Error('rust/proto/fixtures has no projects row with cards');
})();

/** A project row (the sidebar's and the inbox's source): the fixtures' first with cards, then `over`. */
export function row(over: Partial<ProjectRow> & Pick<ProjectRow, 'project' | 'name'>): ProjectRow {
  return { ...ROW, cards: [], waits: 0, home: false, ...over };
}

/** The core's `projects` event with these rows. */
export const projects = (...rows: ProjectRow[]): EvOf<'projects'> => ev('projects', { projects: rows });

/** The first artifact of the fixtures' `artifacts` event. */
const ARTIFACT: Artifact = (() => {
  const a = fixtureOf('artifacts')?.items[0];
  if (a === undefined) throw new Error('rust/proto/fixtures has no artifacts event with an item');
  return a;
})();

/** An artifact: the fixtures' first, then `over` (give `id`); one version, no history, not new. */
export function artifact(over: Partial<Artifact> & Pick<Artifact, 'id'>): Artifact {
  return { ...ARTIFACT, versions: [], path: null, version: 1, new: false, title: over.id, ...over };
}

/** A hub's `artifacts` event: every artifact of that project. */
export const artifacts = (project: string, ...items: Artifact[]): EvOf<'artifacts'> => ev('artifacts', { project, items });

/** A folder of the library: a theme (no parent) or a task folder in one; main's unless `by`. */
export function folder(over: Partial<Folder> & Pick<Folder, 'id' | 'name'>): Folder {
  return { by: 'main', named_by_you: false, ...over };
}

/** A hub's `artifacts` event carrying the library's folders (each hub sends the one list). */
export const withFolders = (e: EvOf<'artifacts'>, folders: Folder[]): EvOf<'artifacts'> => ({ ...e, folders });

/** The core's `library_folders`: the folder list after his edit. */
export const libraryFolders = (folders: Folder[]): EvOf<'library_folders'> => ev('library_folders', { folders });

// ---- the window-local answers (main's ipc.ts sends them to the window that asked; not in the
// core's protocol, so typed here from the proto's result types)

/** The hub took his answer to `card` (card/answer's AnswerResult; no `answer`: a page took it). */
export interface CardAnsweredEv {
  readonly ev: 'card_answered';
  readonly project: string;
  readonly card: number;
  readonly answer?: AnswerResult['answer'];
}
/** The core or its hub refused his answer or close of that card, with its words. */
export interface CardRefusedEv {
  readonly ev: 'card_refused';
  readonly project: string;
  readonly card: number;
  readonly text: string;
}
/** His send's result: the message id the hub gave it, and why none (null both: an older hub). */
export interface SentEv {
  readonly ev: 'sent';
  readonly project: string;
  readonly agent: string;
  readonly cid: number;
  readonly msg_id: number | null;
  readonly reason: SendReason | null;
}
export type LocalEv = CardAnsweredEv | CardRefusedEv | SentEv;

export const cardAnswered = (project: string, card: number, answer?: AnswerResult['answer']): CardAnsweredEv =>
  ({ ev: 'card_answered', project, card, ...(answer === undefined ? {} : { answer }) });
export const cardRefused = (project: string, card: number, text: string): CardRefusedEv => ({ ev: 'card_refused', project, card, text });
/** A send answered: with an id, its message made; without, `reason` says why none (null: an older hub). */
export const sent = (project: string, agent: string, cid: number, msgId: number | null = null, reason: SendReason | null = msgId === null ? null : 'made'): SentEv =>
  ({ ev: 'sent', project, agent, cid, msg_id: msgId, reason });

/** Main says the core's state (CorePort.onState: up, or down after it exited). */
export interface CoreStateIn {
  readonly ev: 'core_state';
  readonly state: 'up' | 'down';
}
export const coreDown = (): CoreStateIn => ({ ev: 'core_state', state: 'down' });
export const coreUp = (): CoreStateIn => ({ ev: 'core_state', state: 'up' });

/** Anything given to the window: a core or hub event, main's window-local answer, or the core's state. */
export type Input = Ev | LocalEv | CoreStateIn;

/** His own message as the hub writes it in the thread (his echo): `text`, its message id (null: an older transcript). */
export function said(project: string, agent: string, pos: number, text: string, msgId: number | null, atMs: number): EvOf<'entry'> {
  return ev('entry', { project, agent, entry: { kind: 'you', pos, at_ms: atMs, text, ...(msgId === null ? {} : { msg_id: msgId }) } });
}

/** The hub (or the core for it) refused his send: with its cid (G), or none (an older core: the project's oldest). */
export function sendError(project: string, text: string, cid: number | null, reason?: 'refused' | 'undelivered'): EvOf<'error'> {
  return ev('error', { project, cmd: 'send', text, ...(cid === null ? {} : { cid }), ...(reason === undefined ? {} : { reason }) });
}

// ---- his gestures (what he does; run.ts turns each into the app's own call)

/** Where he is when he answers: a project's agent's thread, or the inbox screen. */
export type Here = { readonly project: string; readonly agent: string } | 'inbox';

export type Gesture =
  | { readonly g: 'answer'; readonly project: string; readonly card: number; readonly reply: string; readonly from?: Here }
  | { readonly g: 'close'; readonly project: string; readonly card: number }
  | { readonly g: 'retry'; readonly project: string; readonly card: number }
  | { readonly g: 'pick'; readonly project: string; readonly card: number; readonly n: number; readonly from?: Here }
  | { readonly g: 'send'; readonly project: string; readonly agent: string; readonly text: string; readonly mode?: Mode; readonly parts?: readonly Part[] }
  | { readonly g: 'open'; readonly project: string; readonly agent: string }
  | { readonly g: 'resend' | 'dismiss'; readonly project: string; readonly agent: string; readonly cid: number }
  | { readonly g: 'art'; readonly project: string; readonly id: string }
  | { readonly g: 'chip'; readonly card: Card }
  | { readonly g: 'go'; readonly route: Route }
  | { readonly g: 'rail'; readonly place: RailPlace }
  | { readonly g: 'win'; readonly width: number }
  | { readonly g: 'tab'; readonly project: string; readonly id: string }
  | { readonly g: 'untab'; readonly how: 'x' | 'esc'; readonly forget?: true }
  | { readonly g: 'outer'; readonly want: number | null };

export const answer = (project: string, card: number, reply: string, from?: Here): Gesture =>
  ({ g: 'answer', project, card, reply, ...(from === undefined ? {} : { from }) });
export const close = (project: string, card: number): Gesture => ({ g: 'close', project, card });
export const retry = (project: string, card: number): Gesture => ({ g: 'retry', project, card });
/** He picks option `n` of a card (a digit, a click on it). */
export const pick = (project: string, card: number, n: number, from?: Here): Gesture => ({ g: 'pick', project, card, n, ...(from === undefined ? {} : { from }) });
/** He sends `text` to that agent: now (the default) or queued after its turn, with the parts he made (a file he attached is a file part). */
export const send = (project: string, agent: string, text: string, opts: { readonly mode?: Mode; readonly parts?: readonly Part[] } = {}): Gesture =>
  ({ g: 'send', project, agent, text, ...opts });
/** He switches to that project's agent's thread (the sidebar, a link, ⌘1-9). */
export const open = (project: string, agent = 'main'): Gesture => ({ g: 'open', project, agent });
/** 'try again' on his failed message (its row's cid, as the row shows it). */
export const resend = (project: string, agent: string, cid: number): Gesture => ({ g: 'resend', project, agent, cid });
/** He clicks an artifact's name (an inbox item's line, a thread's chip, the right side's list). */
export const openArtifact = (project: string, id: string): Gesture => ({ g: 'art', project, id });
/** He clicks the artifact chip of an open card (or ⌘O on it): what it is about. */
export const openChip = (c: Card): Gesture => ({ g: 'chip', card: c });
/** He goes to that route (the rail, ⌘K, a link). */
export const goTo = (route: Route): Gesture => ({ g: 'go', route });
/** He clicks that button of the left rail (bise's ':*', inbox, scheduled, artifacts, plugins, settings). */
export const rail = (place: RailPlace): Gesture => ({ g: 'rail', place });
/** He makes the window this wide (the rail and a 240 sidebar on its left). */
export const windowAt = (width: number): Gesture => ({ g: 'win', width });
/** An artifact opens in the panel's tab (its route, a chip, the side's list), the panel unfolding. */
export const tabArtifact = (project: string, id: string): Gesture => ({ g: 'tab', project, id });
/** He closes the panel's tab in front: its × ('x'), or esc ('esc'; `forget`: what was in front before it came is not known, after a relaunch or another door). */
export const closeFrontTab = (how: 'x' | 'esc', forget = false): Gesture => ({ g: 'untab', how, ...(forget ? { forget: true as const } : {}) });
/** His drag (or ←/→) on the outer card's left border asks this width (the Splitter's want). */
export const dragOuter = (want: number): Gesture => ({ g: 'outer', want });
/** A double-click (or Enter) on the outer card's left border. */
export const resetOuter = (): Gesture => ({ g: 'outer', want: null });
/** He drops his failed message. */
export const dismiss = (project: string, agent: string, cid: number): Gesture => ({ g: 'dismiss', project, agent, cid });

/** A command that left the window for the core (bise-proto's AppCmd | HubCmd). */
export type Cmd = CoreCmd;
