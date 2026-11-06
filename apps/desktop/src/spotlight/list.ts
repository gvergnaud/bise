// The spotlight's list (desktop S8, v9; the capsule's spotlist.js, ported in behavior). Pure.
// Empty: waiting for you (the cards), at work (working agents), projects (designer m_12805; the
// one in front first; what's in front with no project gets its own dim row, no ⌘ number).
// Typed: 1-2 words that name an agent or a project list those (⏎ opens the first); a sentence puts
// 'ask bise' first and keeps the agents it names below, archived ones last (the rule is
// src/search's asksBise / sentenceNames, ⌘K's too: main m_12973).
// The inputs are this module's own view types: the screen maps the store's (generated) shapes to
// them, so a protocol change doesn't reach the list's rules.
import { asksBise, matchScore, sentenceNames } from '@/search/rank';
import { W, cardLine, hostOf, newBrief, oneLine, shortPath, words } from './words';

export type AgentStatus = 'waiting' | 'blocked' | 'working' | 'idle' | 'done' | 'failed';

export interface SpotAgent {
  readonly name: string;
  readonly project: string;
  readonly status: AgentStatus;
  readonly title: string;
  readonly purpose?: string;
  readonly archived?: boolean;
  /** the project's main (its thread is the project's own) */
  readonly main?: boolean;
  /** what it says it does now (`sb status --note`) */
  readonly note?: string;
  /** since when it does what it does now (ms): its live step's start, else its state's */
  readonly since?: number;
  /** the right bar's sign for it (app/side's signOf): the tray menu's submenu shows the same */
  readonly sign?: { readonly g: string; readonly tone: 'ink' | 'acc' | 'err' | 'dim' };
}

export interface SpotProject {
  readonly id: string;
  readonly name: string;
  readonly path: string;
  readonly branch: string;
  readonly agents: number;
  readonly hosts?: readonly string[];
  /** the hub row's counts, as the sidebar's mark reads them (the tray menu's project lines) */
  readonly waits?: number;
  readonly working?: number;
  readonly missing?: boolean;
}

/** bise's home, out of `projects`: its own counts (the tray menu's 'bise' line). */
export interface SpotHome {
  /** its project id (its agents' and cards' `project`) */
  readonly id?: string;
  readonly waits: number;
  readonly working: number;
}

export interface SpotCard {
  readonly id: string;
  /** the hub's card id, its number ('#567', notif-id) */
  readonly card: number;
  readonly agent: string;
  readonly project: string;
  readonly text: string;
  /** ? a question, ! an approval, ✉ drafts to send (ambient m_8793) */
  readonly kind: 'question' | 'approval' | 'drafts';
  /** its project's name when the projects list doesn't hold it (bise's home: 'bise') */
  readonly pname?: string;
}

export interface SpotFront {
  readonly app: string;
  readonly title?: string;
  readonly url?: string;
  readonly path?: string;
}

export interface SpotData {
  readonly agents: readonly SpotAgent[];
  readonly projects: readonly SpotProject[];
  readonly cards: readonly SpotCard[];
  readonly front?: SpotFront | null;
  readonly home?: SpotHome;
}

export type SpotItem =
  | { readonly kind: 'card'; readonly id: string; readonly mark: '?' | '!' | '✉'; readonly agent: string; readonly pname: string; readonly text: string }
  | { readonly kind: 'agent'; readonly id: string; readonly name: string; readonly project: string; readonly pname: string; readonly status: AgentStatus; readonly title: string }
  | { readonly kind: 'env'; readonly id: string; readonly project: string; readonly name: string; readonly sub: string; readonly front: boolean; readonly n: number | null }
  | { readonly kind: 'front'; readonly id: string; readonly name: string; readonly sub: string }
  | { readonly kind: 'new'; readonly id: string; readonly brief: string };

export interface SpotSection {
  readonly label: string | null;
  readonly items: readonly SpotItem[];
}

export interface SpotList {
  readonly mode: 'empty' | 'names' | 'ask' | 'new';
  readonly sections: readonly SpotSection[];
}

const ORDER: Record<AgentStatus, number> = { waiting: 0, blocked: 1, working: 2, idle: 3, done: 4, failed: 5 };
const MARK = { question: '?', approval: '!', drafts: '✉' } as const;

/** The project in front: his front app's folder or host names it. */
export function frontProject(front: SpotFront | null | undefined, projects: readonly SpotProject[]): SpotProject | null {
  if (!front) return null;
  const host = hostOf(front.url ?? '');
  const path = front.path ?? '';
  return (
    projects.find((p) => p.path !== '' && path !== '' && (path === p.path || path.startsWith(`${p.path.replace(/\/$/u, '')}/`))) ??
    projects.find((p) => host !== '' && (p.hosts ?? []).includes(host)) ??
    projects.find((p) => p.name !== '' && ((host !== '' && host.includes(p.name)) || words(front.title ?? '').includes(p.name.toLowerCase()))) ??
    null
  );
}

export function spotList(d: SpotData, text: string): SpotList {
  const q = text.trim();
  const ql = q.toLowerCase();
  const pname = (id: string): string => d.projects.find((p) => p.id === id)?.name ?? id;
  const waits = new Set(d.cards.map((c) => `${c.project}/${c.agent}`));
  const live = d.agents
    .map((a, i) => ({ a, i, status: waits.has(`${a.project}/${a.name}`) ? ('waiting' as const) : a.status }))
    .sort((x, y) => ORDER[x.status] - ORDER[y.status] || x.i - y.i);
  const agentItem = (a: SpotAgent, status: AgentStatus): SpotItem => ({
    kind: 'agent',
    id: `agent.${a.project}.${a.name}`,
    name: a.name,
    project: a.project,
    pname: pname(a.project),
    status,
    title: oneLine(a.title),
  });
  const front = frontProject(d.front, d.projects);
  const ordered = front ? [front, ...d.projects.filter((p) => p !== front)] : [...d.projects];
  // ⌘1-9 follow the projects' own order, whatever app is in front (ambient m_9196)
  const envs: SpotItem[] = ordered.map((p) => {
    const n = d.projects.indexOf(p) + 1;
    return {
      kind: 'env',
      id: `env.${p.id}`,
      project: p.id,
      name: p.name,
      front: p === front,
      n: n <= 9 ? n : null,
      sub: p === front ? W.inFront(hostOf(d.front?.url ?? '') || shortPath(d.front?.path ?? '') || (d.front?.app ?? '')) : `${W.envSub(p.branch, p.agents)} · ${shortPath(p.path)}`,
    };
  });

  if (q === '') {
    const cards: SpotItem[] = d.cards.map((c) => ({
      kind: 'card',
      id: `card.${c.id}`,
      mark: MARK[c.kind],
      agent: c.agent,
      pname: c.pname ?? pname(c.project),
      text: c.kind === 'question' ? oneLine(c.text) : cardLine(c.text),
    }));
    const f = d.front;
    const fhost = hostOf(f?.url ?? '');
    const frontRow: SpotItem[] =
      f && !front
        ? [{ kind: 'front', id: 'front', name: f.title ?? f.app, sub: W.inFront([fhost || shortPath(f.path ?? ''), f.title ? f.app : ''].filter((x) => x !== '').join(' · ')) }]
        : [];
    const work = live.filter((x) => x.status === 'working' && x.a.archived !== true).map((x) => agentItem(x.a, x.status));
    const sections: SpotSection[] = [];
    if (cards.length > 0) sections.push({ label: W.waiting, items: cards });
    if (work.length > 0) sections.push({ label: W.atWork, items: work });
    if (frontRow.length + envs.length > 0) sections.push({ label: W.envs, items: [...frontRow, ...envs] });
    return { mode: 'empty', sections };
  }

  // '/new …' or 'start an agent: …': one row that starts it (bar A.1)
  const brief = newBrief(q);
  if (brief !== undefined) return { mode: 'new', sections: [{ label: null, items: [{ kind: 'new', id: 'new', brief }] }] };

  // 1-2 words: names (an agent's name, title or purpose, or its project's name; archived ones too),
  // ranked by src/search's rules, the window's ⌘K's (lead m_11844): archived after the live ones,
  // then the best match, then waiting before working before the rest
  const byName = live
    .map((x) => ({ ...x, s: matchScore(ql, { name: [x.a.name], about: [x.a.title, x.a.purpose ?? ''], where: [pname(x.a.project)] }) }))
    .filter((x) => x.s >= 0)
    .sort((x, y) => Number(x.a.archived === true) - Number(y.a.archived === true) || y.s - x.s || ORDER[x.status] - ORDER[y.status] || x.i - y.i)
    .map((x) => agentItem(x.a, x.status));
  const envHit = envs.filter((e) => e.kind === 'env' && (e.name.toLowerCase().startsWith(ql) || (d.projects.find((p) => p.id === e.project)?.path ?? '').toLowerCase().includes(ql)));
  // what ⏎ does is src/search's rule, ⌘K's too (main m_12973): 1-2 words that name something open it
  if (!asksBise(q, byName.length + envHit.length > 0)) {
    return { mode: 'names', sections: [{ label: W.found, items: [...byName, ...envHit] }] };
  }
  // a sentence: a message, ⏎ sends it to bise (Gabriel's v4: no 'ask bise' row); the agents it
  // names stay below (a word is its name, or 2 of its title's long words), archived ones after the
  // live ones; none: no list
  const named = live
    .filter((x) => sentenceNames(q, x.a))
    .sort((x, y) => Number(x.a.archived === true) - Number(y.a.archived === true))
    .map((x) => agentItem(x.a, x.status));
  return { mode: 'ask', sections: named.length === 0 ? [] : [{ label: W.found, items: named }] };
}

/** A chip in the field's plain text (the composer's pmtext OBJ). */
const CHIP = '\uFFFC';

/**
 * The list for the field's plain text (Gabriel's v4, spotlight-composer#choix): a search first. A
 * name or two words that find an agent or a project are a search, ⏎ opens the first; a sentence,
 * a line break or a chip (@ agent, ▤ paste, image, file) make a message: ⏎ sends it to bise, only
 * the agents it names stay listed ('found'); none: no list, the field alone. Pure.
 */
export function spotView(d: SpotData, text: string): SpotList {
  const flatText = text.split(CHIP).join(' ').replace(/\s+/gu, ' ').trim();
  const base = spotList(d, flatText);
  const message = text.includes('\n') || text.includes(CHIP);
  if (!message || base.mode === 'ask') return base;
  // a chip or a line break: a message whatever its words say (chips alone name nothing)
  const named = flatText === '' ? [] : flat(base).filter((x) => x.kind === 'agent' || x.kind === 'env');
  return { mode: 'ask', sections: named.length === 0 ? [] : [{ label: W.found, items: named }] };
}

/** While a routing hold runs: the other projects he can send it to instead (⏎ corrects the route). */
export function routeList(d: SpotData, to: string): SpotList {
  const items: SpotItem[] = d.projects
    .filter((p) => p.id !== to)
    .map((p, i) => ({ kind: 'env', id: `env.${p.id}`, project: p.id, name: p.name, front: false, n: i < 9 ? i + 1 : null, sub: shortPath(p.path) }));
  return { mode: 'names', sections: items.length === 0 ? [] : [{ label: W.anotherProject, items }] };
}

/** The list's items in keyboard order. */
export const flat = (l: SpotList): SpotItem[] => l.sections.flatMap((s) => s.items);
