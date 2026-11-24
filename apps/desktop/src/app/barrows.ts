// The right bar's rows (designer's desktop-right-bar v5, desktop-agents-bar): one shape for the four
// sections, glyph, name, a dim second word (the role, or the project's name on bise's thread), the
// end on the right; an agent's row has the TUI's columns instead of an end (its time, its %, its
// worktree's mark: barplaces.ts), and its worktree's group head when it shares one.
// Pure: RightBar.tsx draws these (barrows.ts: a rightbar.ts would clash with RightBar.tsx on macOS's case-blind disk). Owner: amb-win.
import type { Item } from '@/artifacts/model';
import { glyphOf, leaves } from '@/artifacts/model';
import type { Task } from '@/scheduled/model';
import { keyOf, portWords, shortCmd, stateOf, tipOf as procTip, urlWords, whereOf, worktreeName, type ProcItem, type Root } from '@/running/model';
import { howOften, nextWords } from '@/scheduled/model';
import { rankedOf, shortAge, shownName } from '@/agents/model';
import { alarms, pauseLine, waitLen } from '@/thread/wake';
import type { Agent, Card, ComputerAgent, Place } from '@/proto';
import { panel } from '@/proto/words_panel.gen';
import { running } from '@/proto/words_running.gen';
import { asksMerge, livePr, markOf, orphanPlaces, placeLabel, placeOf, sharedPlaces, tipWords, type Mark } from './barplaces';
import { computerDot, signOf, type Sign, type Tone } from './side';
import { archivedAgeTip, madeTip, pctTip, signTip, stateWord, timeTip } from './tipwords';
import { rowState, type RowState } from './rowstate';

/** One agent with its project, across the projects shown. */
export interface ProjAgent {
  readonly project: string;
  readonly projectName: string;
  readonly a: Agent;
}

export interface BarRow {
  readonly key: string;
  readonly project: string;
  /** the agent it opens (agents, archived, waits) */
  readonly agent?: string;
  /** the artifact it opens */
  readonly id?: string;
  /** an agent's row, a worktree's head, an open PR with no agent; none: the other sections' rows */
  readonly kind?: 'agent' | 'group' | 'orphan';
  /** the project's main ('bise'), AGENTS' first row: out of 'AGENTS · N' */
  readonly main?: true;
  readonly glyph: string;
  readonly tone: 'ink' | 'acc' | 'err' | 'dim' | 'mid';
  readonly name: string;
  readonly sub: string;
  readonly end: string;
  readonly endTone: 'dim' | 'ink' | 'acc' | 'err';
  readonly title: string;
  readonly testId: string;
  /** it waits on him: never hidden by a cap (designer v5) */
  readonly waits: boolean;
  /** a feature's group head: the feature it opens */
  readonly feature?: string;
  /** an agent's columns (desktop-agents-bar §2): its turn's or wait's length, its context used; '' empty, the column stays */
  readonly time?: string;
  readonly pct?: string;
  /** its context used, 1-100, for the % column's ring (his ask m_26205); none: no usage, or 0 */
  readonly ctx?: number;
  /** its worktree's mark, last (§3): '↑' '…' 'ψ', tinted, its tip */
  readonly mark?: Mark;
  /** the worktree group it is in (its head's too): the 1 px rule in the margin */
  readonly group?: string;
  /** an agent at work (working, or waiting on an agent or an event): 'show N more''s 'N working' (designer m_39151) */
  readonly busy?: true;
  /** an orphan PR's or a head's branch (an orphan opens it) */
  readonly branch?: string;
  /** at work: the DS's breeze '∿' (a Dot) in the glyph's place */
  readonly working?: boolean;
  /** it uses his computer: the core's mark after its name (S29/L19 W3) */
  readonly cu?: { readonly g: string; readonly tone: Tone; readonly title: string };
  /** a browser-only artifact: '↗' 4 px after its title, text-3 (designer m_21742) */
  readonly leaves?: boolean;
  /** its sub cuts after a whole word with '…' (an alarm's note, agent-waiting-states A) */
  readonly fit?: boolean;
  /** its name is a command: mono (RUNNING's rows) */
  readonly mono?: boolean;
  /** an alarm: its watch's id; the row opens its detail (stop watching) */
  readonly watch?: number;
  /** the hover tips of its compact marks (tipwords.ts): its sign, its end, its time and % columns */
  readonly tips?: { readonly glyph?: string; readonly end?: string; readonly time?: string; readonly pct?: string };
  /** an agent's state in words at the row's right edge (the agents float, page agents-float v1) */
  readonly state?: RowState;
}

/** What AGENTS reads besides the agents: their next wakes ('<project>/<agent>' -> ms), the project's worktrees and cards, the computer rows, the ⌥ numbers. */
export interface AgentExtras {
  readonly wakes?: ReadonlyMap<string, number>;
  /** the project's worktrees and their PRs (hub/agents places); none on bise's thread */
  readonly places?: readonly Place[];
  /** the project's cards: a 'merge' one tints its PR's '↑' pink */
  readonly cards?: readonly Card[];
  readonly computer?: readonly ComputerAgent[];
  /** the ⌥ numbers, the hub's rank (agents/model numbersOf, the TUI's too): the order and ⌥N; none: start order */
  readonly numbers?: ReadonlyMap<string, number>;
  /** bise's home project: on bise's thread, its main is AGENTS' first row */
  readonly home?: string;
}

/**
 * An agent's role words as the bar draws them: never a raw `<tag>` (a
 * model's `<think>` once reached its title, main m_22156; the hub drops it
 * at the source, role.rs `clean`). Such words read 'thinking'.
 */
export function roleWords(t: string): string {
  return /<\/?[a-z][a-z0-9_-]*(\s[^>]*)?>/i.test(t) ? panel.role_thinking : t;
}

const idOf = (all: boolean, project: string, name: string) => (all ? `${project}.${name}` : name);

/**
 * The bar's sign (desktop-agents-bar §2), the TUI's: the breeze while it works, '?' pink when it
 * waits for him, '✓' pink done, '✗' red failed (designer m_26248: '▲' is a cut turn), '…' when it waits on an event or another agent,
 * '○' text-3 at rest (a next wake too: its time is in the hover). No state words. Pure.
 */
export function barSign(s: Sign): { g: string; tone: BarRow['tone']; working: boolean } {
  switch (s.g) {
    case '?':
      return { g: '?', tone: 'acc', working: false };
    case '∿':
      return { g: '∿', tone: 'ink', working: true };
    case '…':
      return { g: '…', tone: 'ink', working: false };
    case '⋯':
      return { g: '…', tone: 'dim', working: false };
    case '✗':
      return { g: '✗', tone: 'err', working: false };
    case '✓':
      return { g: '✓', tone: 'acc', working: false };
    default:
      return { g: '○', tone: 'dim', working: false };
  }
}

/** Its context used, in the % column: '17%'; '' when nothing worth saying (none, 0%). Pure. */
export function pctOf(a: Agent): string {
  const u = a.usage;
  if (u == null || u.window == null || u.window <= 0) return '';
  const n = Math.round((u.context / u.window) * 100);
  return n > 0 ? `${String(Math.min(n, 100))}%` : '';
}

/** The % column shows from this much context used: below it, the row's tip says it (desktop-delight v2). */
export const PCT_LOUD = 70;

/** Whether a '17%' is loud enough for the column. Pure. */
export function pctLoud(pct: string): boolean {
  return pct !== '' && Number.parseInt(pct, 10) >= PCT_LOUD;
}

/** The time column: the turn's length while it works, the wait's while it waits on an event; '' else. Never a state word. */
export function timeOf(a: Agent, now: number): string {
  const pause = pauseLine(a, now);
  if (pause !== null) return pause.len;
  // wt-disk: waiting for disk runs no turn: no time
  if (a.phase === 'waiting_disk') return '';
  if (a.status === 'working') return waitLen(a.turn_ms ?? now - a.since_ms);
  return '';
}

function agentRow(x: ProjAgent, now: number, all: boolean, n: number | undefined, alt: boolean, extras: AgentExtras, mark: Mark | undefined, group: string | undefined): BarRow {
  const s = signOf(x.a, now, extras.wakes?.get(`${x.project}/${x.a.name}`));
  const sign = barSign(s);
  // main is ⌥0 (keys.ts), the others ⌥1-⌥9
  const numbered = alt && n !== undefined && n >= (x.a.main ? 0 : 1) && n <= 9;
  const cu = computerDot(extras.computer?.find((r) => r.project === x.project && r.agent === x.a.name));
  const wake = extras.wakes?.get(`${x.project}/${x.a.name}`);
  // its context used: in the column from 70 %, else in its sign's tip only (desktop-delight v2)
  const pct = pctOf(x.a);
  const loud = pctLoud(pct);
  const signWords = signTip(sign.g, x.a, now, wake === undefined ? '' : hm(wake));
  return {
    key: `a:${x.project}/${x.a.name}`,
    kind: 'agent',
    project: x.project,
    agent: x.a.name,
    glyph: numbered ? `⌥${String(n)}` : sign.g,
    tone: numbered ? 'dim' : sign.tone,
    name: shownName(x.a.name),
    // bise's thread: the project's name, but main's own doing line (its project is bise's home)
    // main at rest says what it is, 'team lead' (page new-project-home), never its purpose's words
    sub: all && !x.a.main ? x.projectName : roleWords(x.a.title !== '' ? x.a.title : x.a.main ? panel.role_lead : x.a.purpose),
    end: '',
    endTone: 'dim',
    time: timeOf(x.a, now),
    pct: loud ? pct : '',
    title: s.title,
    tips: { glyph: pct === '' || loud ? signWords : `${signWords} · ${pctTip(x.a, pct)}`, time: timeTip(x.a, now), pct: pctTip(x.a, pct) },
    testId: `app.right.agent.${idOf(all, x.project, x.a.name)}`,
    waits: s.g === '?',
    ...(x.a.main ? { main: true as const } : {}),
    ...(!numbered && sign.working ? { working: true } : {}),
    ...(busyOf(x.a) ? { busy: true as const } : {}),
    ...(cu === null ? {} : { cu }),
    ...(pct === '' ? {} : { ctx: Number.parseInt(pct, 10) }),
    ...(mark === undefined ? {} : { mark }),
    ...(group === undefined ? {} : { group }),
    state: rowState(x.a, now, wake),
  };
}

/** At work, in 'show N more''s words (designer m_39151): working, or waiting on an agent or on an event it resumes on. Pure. */
export function busyOf(a: Agent): boolean {
  return a.status === 'working' || a.status === 'waiting' || a.waiting_on?.who === 'agent' || a.watching.some((w) => w.resumes === true);
}

/**
 * AGENTS (desktop-agents-bar §2-§4), the TUI panel's order (sb.rs blocks, designer m_23651): a
 * project's main first, named 'bise' (on bise's thread, the home project's), then the
 * live agents that share no worktree, by their ⌥ number, the hub's rank (the shared checkout's and the
 * ones alone in theirs, each with its mark), then the open PRs with no live agent (no sign, the
 * branch text-3, its '↑'), then each worktree 2 or more live agents share: a 'ψ <branch>' head with
 * its PR's '↑ #418', its rows with no mark, in the order of their smallest number. A head waits on
 * him when one of its agents does: the cap never hides it. `alt`: ⌥ held, ⌥1-⌥9 for the signs.
 * On bise's thread (`all`) the agents stay in start order, no worktrees. Pure.
 */
export function agentRows(agents: readonly ProjAgent[], now: number, all: boolean, alt = false, extras: AgentExtras = {}): BarRow[] {
  // main first, always, as the TUI's panel (his ask, main m_28881): 'bise', ⌥0, no mark; on bise's
  // thread, bise's own (the home project's: designer m_28913, the row in view there)
  const main = agents.find((x) => x.a.main && !x.a.archived && (!all || x.project === extras.home));
  const head = main === undefined ? [] : [agentRow(main, now, all, 0, alt, extras, undefined, undefined)];
  return [...head, ...taskRows(agents, now, all, alt, extras)];
}

/** AGENTS' rows but main's (agentRows). Pure. */
function taskRows(agents: readonly ProjAgent[], now: number, all: boolean, alt: boolean, extras: AgentExtras): BarRow[] {
  const live = agents.filter((x) => !x.a.archived && !x.a.main);
  const num = (name: string) => extras.numbers?.get(name);
  const byNum = (a: ProjAgent, b: ProjAgent) => (num(a.a.name) ?? Number.MAX_SAFE_INTEGER) - (num(b.a.name) ?? Number.MAX_SAFE_INTEGER);
  const places = all ? [] : (extras.places ?? []);
  const cards = extras.cards ?? [];
  if (places.length === 0) {
    const order = all ? acrossProjects(live, extras.home) : extras.numbers === undefined ? live : live.slice().sort(byNum);
    return order.map((x, i) => agentRow(x, now, all, num(x.a.name) ?? i + 1, alt, extras, undefined, undefined));
  }
  const liveA = live.map((x) => x.a);
  const shared = sharedPlaces(places, liveA);
  const project = live[0]?.project ?? agents[0]?.project ?? '';
  // the hub's rank leads (agent-rank, his m_45828: keep the order): each agent alone at its number,
  // each worktree two or more share as one unit at its first member's number, its agents under it
  const groups = places.flatMap((p) => {
    const members = live.filter((x) => shared.get(p.id)?.some((a) => a.name === x.a.name) === true).sort(byNum);
    return members.length >= 2 ? [{ p, members }] : [];
  });
  const inGroup = new Set(groups.flatMap((g) => g.members.map((x) => x.a.name)));
  type Unit = { readonly at: number; readonly rows: BarRow[] };
  const units: Unit[] = live
    .filter((x) => !inGroup.has(x.a.name))
    .map((x) => {
      const p = placeOf(x.a, places);
      return { at: num(x.a.name) ?? Number.MAX_SAFE_INTEGER, rows: [agentRow(x, now, all, num(x.a.name), alt, extras, p === undefined ? undefined : markOf(p, asksMerge(p, cards)), undefined)] };
    });
  for (const { p, members } of groups) {
    const head = markOf(p, asksMerge(p, cards));
    const memberRows = members.map((x) => agentRow(x, now, all, num(x.a.name), alt, extras, undefined, p.id));
    const headRow: BarRow = {
      key: `ψ:${project}/${p.id}`,
      kind: 'group',
      project,
      group: p.id,
      agent: members[0]?.a.name,
      branch: p.branch ?? undefined,
      ...(p.feature === true ? { feature: p.id.replace(/^feature:/, '') } : {}),
      glyph: p.feature === true && p.trying === true ? 'Δ' : 'ψ',
      tone: 'mid',
      name: placeLabel(p),
      sub: '',
      end: '',
      endTone: 'dim',
      title: tipWords(p),
      testId: `app.right.group.${placeLabel(p)}`,
      // one of its agents waits on him: the head shows past the cap too
      waits: memberRows.some((r) => r.waits),
      // the head's mark: the PR's '↑' with its number, or its land's '…'; nothing for a lone 'ψ'
      ...(head.g === 'ψ' || head.g === 'Δ' ? {} : { mark: head }),
    };
    units.push({ at: num(members[0]?.a.name ?? '') ?? Number.MAX_SAFE_INTEGER, rows: [headRow, ...memberRows] });
  }
  // a stable sort: equal numbers (none, an older hub) keep the agents' start order
  const rows = units.sort((x, y) => x.at - y.at).flatMap((u) => u.rows);
  // the open PRs with no live agent, after the agents
  for (const p of orphanPlaces(places, liveA)) {
    const label = placeLabel(p);
    rows.push({
      key: `pr:${project}/${p.id}`,
      kind: 'orphan',
      project,
      branch: p.branch ?? undefined,
      glyph: '',
      tone: 'dim',
      name: label,
      sub: '',
      end: '',
      endTone: 'dim',
      title: tipWords(p),
      testId: `app.right.pr.${String(livePr(p)?.number ?? label)}`,
      waits: false,
      mark: markOf(p, asksMerge(p, cards)),
    });
  }
  return rows;
}

/**
 * Every project's agents on bise's own screen (no ⌥ numbers there): project by project, the home's
 * first, the others as they come; in each, the hub's rank (agents/model rankedOf, as the folded
 * side's lists.ts did). Pure.
 */
export function acrossProjects(live: readonly ProjAgent[], home: string | undefined): ProjAgent[] {
  const projects = [...new Set(live.map((x) => x.project))].sort((a, b) => Number(b === home) - Number(a === home));
  return projects.flatMap((p) => {
    const here = live.filter((x) => x.project === p);
    const slots = rankedOf(here.map((x) => x.a));
    return here.slice().sort((x, y) => (slots.get(x.a.name) ?? 0) - (slots.get(y.a.name) ?? 0));
  });
}

/**
 * ARCHIVED (desktop-agents-bar): its sign, its name, its age, newest first: '✓ release-32 2 h' done,
 * '✗ import 1 d' red failed, '– old 3 d' stopped; no state words (the hover says it). Pure.
 */
export function archivedRows(agents: readonly ProjAgent[], now: number, all: boolean): BarRow[] {
  return agents
    .filter((x) => x.a.archived)
    .slice()
    .sort((a, b) => b.a.since_ms - a.a.since_ms)
    .map((x) => {
      const st = x.a.status === 'done' ? 'done' : x.a.status === 'failed' ? 'failed' : 'stopped';
      return {
        key: `r:${x.project}/${x.a.name}`,
        project: x.project,
        agent: x.a.name,
        glyph: st === 'done' ? '✓' : st === 'failed' ? '✗' : '–',
        tone: st === 'failed' ? 'err' : 'dim',
        name: x.a.name,
        sub: all ? x.projectName : '',
        end: shortAge(now - x.a.since_ms),
        endTone: 'dim',
        title: `${stateWord(st)} · ${x.a.title !== '' ? x.a.title : x.a.purpose}`,
        testId: `app.right.archived.${idOf(all, x.project, x.a.name)}`,
        waits: false,
        tips: { glyph: `${st === 'done' ? '✓' : st === 'failed' ? '✗' : '–'} ${(st === 'done' ? panel.tip_done : st === 'failed' ? panel.tip_failed : panel.tip_stopped)({ agent: x.a.name })}`, end: archivedAgeTip(st, now - x.a.since_ms) },
      };
    });
}

const hm = (ms: number) => {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
};

/** ARTIFACTS: newest first, its kind's glyph, 'new' in ink, else when (today's time, else its age); `all` (bise's thread): its project under it. Pure. */
export function artifactRows(items: readonly Item[], now: number, all = false): BarRow[] {
  return items
    .filter((it) => it.a.archived_at == null)
    .slice()
    .sort((a, b) => b.a.at_ms - a.a.at_ms)
    .map((it) => ({
      key: `f:${it.project}/${it.a.id}`,
      project: it.project,
      id: it.a.id,
      glyph: glyphOf(it.a.kind),
      tone: 'dim',
      name: it.a.title,
      // on bise's thread its project, as an agent's row says it (designer m_45706)
      sub: all ? it.projectName : '',
      leaves: leaves(it.a),
      end: it.a.new ? panel.new_chip : now - it.a.at_ms < 86_400_000 ? hm(it.a.at_ms) : shortAge(now - it.a.at_ms),
      endTone: it.a.new ? 'ink' : 'dim',
      title: `${it.a.title} · ${it.projectName}`,
      testId: `app.right.artifact.${it.project}.${it.a.id}`,
      waits: false,
      tips: { end: it.a.new ? panel.tip_new_chip : madeTip(now - it.a.at_ms < 86_400_000 ? hm(it.a.at_ms) : shortAge(now - it.a.at_ms)) },
    }));
}

/** SCHEDULED: ◷, its name (else its words), how often, its next run; `all` (bise's thread): its project first. Pure. */
export function scheduledRows(tasks: readonly Task[], now: number, all = false): BarRow[] {
  return tasks.map((x) => ({
    key: `s:${x.project}/${String(x.t.id)}`,
    project: x.project,
    glyph: '◷',
    tone: 'dim',
    name: x.t.name != null && x.t.name !== '' ? x.t.name : (x.t.words.split('\n')[0] ?? ''),
    sub: all ? [x.projectName, howOften(x.t)].join(' · ') : howOften(x.t),
    // cut on a whole word, '…' glued to it ('every day…', designer m_25211), as an alarm's note
    fit: true,
    end: nextWords(x.t, now),
    endTone: 'dim',
    title: x.t.words,
    testId: `app.right.scheduled.${x.project}.${String(x.t.id)}`,
    waits: false,
  }));
}

/**
 * PROCESSES (designer's running-procs, its name m_38642; Gabriel's test, procs-bar): what the bar's
 * scope left running, oldest first (barProcs): a dim '∿', its program and arguments, short and mono
 * ('python3 -m http.server 8000'), then where it runs, dim: its folder past the project's root or
 * its worktree's name (whereOf: 'apps/desktop', 'perf'), on bise's thread 'project › where'; at the
 * end 'localhost:5173 ↗' only when its hub found a web page there (`url`), else its port plain
 * (':9229'). The tip says its id, start, folder and whose. A click opens its output on the
 * processes screen; the address its browser tab. `roots`: each project's root and its agents'
 * worktrees, by project. Pure.
 */
export function procRows(items: readonly ProcItem[], scope: 'agent' | 'project' | 'all', roots: ReadonlyMap<string, readonly Root[]> = new Map(), home?: string): BarRow[] {
  return items.map((x) => {
    const st = stateOf(x.p);
    const url = urlWords(x.p, st);
    const where = whereOf(x.p.cwd, roots.get(x.project) ?? []);
    return {
      key: `p:${keyOf(x)}`,
      project: x.project,
      agent: x.p.agent,
      glyph: '∿',
      tone: 'dim',
      name: shortCmd(x.p.cmd),
      sub: scope === 'all' ? (where === '' ? x.projectName : running.group({ project: x.projectName, agent: where })) : where,
      end: url !== null ? url.label : portWords(x.p, st),
      endTone: 'dim',
      title: scope === 'agent' ? procTip(x.p, home) : `${procTip(x.p, home)}
${running.by({ agent: x.p.agent })}`,
      testId: `app.right.running.${keyOf(x)}`,
      waits: false,
      mono: true,
    };
  });
}

/**
 * WATCHING (agent-waiting-states A): an alarm, its agent free; '◌' (set, not at work), its agent,
 * what wakes it ('if the nightly gate goes red', cut on a word), when it stops watching ('until
 * 08:07', a clock ahead); the hover says who bise wakes, when it was set, when it stops. A pause is
 * no row here: it shows over its composer and in AGENTS. `off`: each instant's offset. Pure.
 */
export function watchRows(agents: readonly ProjAgent[], now: number, all: boolean, off?: (ms: number) => number): BarRow[] {
  return agents
    .filter((x) => !x.a.archived)
    .flatMap((x) =>
      alarms(x.a, now, off).map((al) => ({
        key: `w:${x.project}/${String(al.id)}`,
        project: x.project,
        agent: x.a.name,
        glyph: '◌',
        tone: 'dim' as const,
        name: x.a.name,
        sub: all ? [x.projectName, al.note].filter((w) => w !== '').join(' · ') : al.note,
        fit: true,
        end: al.end === '' ? '' : panel.until({ at: al.end }),
        endTone: 'dim' as const,
        title: al.title,
        testId: `app.right.watching.${idOf(all, x.project, x.a.name)}.${String(al.id)}`,
        waits: false,
        watch: al.id,
      })),
    );
}

/** How many rows one click on 'show 50 more' adds (his m_26894: hundreds of artifacts). */
export const PAGE = 50;

/** One run of a 'show N more' row's words and its tone: the click text-2, the folded idle text-3, ' · ' faint (designer m_39151). */
export interface MorePart {
  readonly text: string;
  readonly tone: 'mid' | 'dim' | 'faint';
}

/** A 'show N more' row's words in parts and its tip (AGENTS'); none: `more` alone. */
export interface MoreWords {
  readonly parts: readonly MorePart[];
  readonly tip: string;
}

/** A section's rows under its cap and the pages he opened past it, and its last row's words. */
export interface Paged {
  readonly shown: BarRow[];
  /** the rows still folded */
  readonly hidden: number;
  /** the pages open past the cap, clamped to what the list has (a list that shrank) */
  readonly pages: number;
  /** the row that shows the next page, the cap's too: 'show 50 more · 365 left', 'show 15 more' under 50 ('' none): it says what a click does (designer m_27020) */
  readonly more: string;
  /** AGENTS': `more` in parts with their tones, and its tip (agentsMore) */
  readonly words?: MoreWords;
  /** 'show less' is there: some page is open past the cap */
  readonly less: boolean;
}

/**
 * The rows a section shows: its cap (8 for AGENTS, 5 for the others), then PAGE more for each page
 * he opened. A row that waits on him is never hidden, it shows in its place past the limit and the
 * count leaves it out (designer v5, ambient-lead m_20091). `say`: the section's own words for the
 * row (AGENTS': agentsMore). Pure.
 */
export function pageRows(rows: readonly BarRow[], cap: number, pages = 0, say?: (folded: readonly BarRow[], next: number) => MoreWords): Paged {
  const over = rows.filter((r, i) => i >= cap && !r.waits).length;
  const p = Math.min(Math.max(0, Math.floor(pages)), Math.ceil(over / PAGE));
  const limit = cap + p * PAGE;
  const shown = rows.filter((r, i) => i < limit || r.waits);
  const hidden = rows.length - shown.length;
  if (hidden > 0 && say !== undefined) {
    const words = say(rows.filter((r, i) => !(i < limit || r.waits)), Math.min(hidden, PAGE));
    return { shown, hidden, pages: p, more: words.parts.map((x) => x.text).join(''), words, less: p > 0 };
  }
  const more = hidden === 0 ? '' : hidden > PAGE ? panel.show_more_left({ n: PAGE, left: hidden }) : panel.show_more({ n: hidden });
  return { shown, hidden, pages: p, more, less: p > 0 };
}

/**
 * AGENTS' 'show N more' (designer m_39151): what a click shows, then how many of the folded agents
 * work and rest: 'show 14 more · 3 working · 11 idle', 'show 11 more · all idle', 'show 3 more · all
 * working'; past a page 'show 50 more of 365 · 3 working'. Its tip: '14 agents not shown: 3 working
 * (perf, docs, search), 11 idle', the names cut after 3 with '+ n'. Never the accent: what needs
 * him is never folded. Pure.
 */
export function agentsMore(folded: readonly BarRow[], next: number): MoreWords {
  const agents = folded.filter((r) => r.kind === 'agent');
  const busy = agents.filter((r) => r.busy === true);
  const idle = agents.length - busy.length;
  const dot: MorePart = { text: ' · ', tone: 'faint' };
  const parts: MorePart[] = [{ text: panel.show_more({ n: next }), tone: 'mid' }];
  if (folded.length > next) parts.push({ text: ` ${panel.more_of({ left: folded.length })}`, tone: 'dim' });
  if (busy.length === 0) parts.push(dot, { text: panel.more_all_idle, tone: 'dim' });
  else if (idle === 0) parts.push(dot, { text: panel.more_all_working, tone: 'mid' });
  else {
    parts.push(dot, { text: panel.count_working({ n: busy.length }), tone: 'mid' });
    if (folded.length <= next) parts.push(dot, { text: panel.more_idle({ n: idle }), tone: 'dim' });
  }
  const names = busy.slice(0, 3).map((r) => r.name);
  const plus = busy.length > 3 ? [panel.more_tip_plus({ n: busy.length - 3 })] : [];
  const say = [
    ...(busy.length > 0 ? [panel.more_tip_working({ n: busy.length, names: [...names, ...plus].join(', ') })] : []),
    ...(idle > 0 ? [panel.more_idle({ n: idle })] : []),
  ];
  return { parts, tip: `${panel.more_tip({ n: agents.length })}: ${say.join(', ')}` };
}

/**
 * Where a project's processes run (whereOf's roots, procs-bar): its root, named '' (a folder past
 * it says only its path inside), and each of its agents' own worktrees by name. Pure.
 */
export function procRoots(projects: readonly { readonly project: string; readonly path: string }[], agents: readonly ProjAgent[]): Map<string, Root[]> {
  const m = new Map<string, Root[]>();
  for (const p of projects) m.set(p.project, p.path === '' ? [] : [{ path: p.path, name: '' }]);
  for (const x of agents) {
    const wt = x.a.worktree ?? '';
    if (wt === '') continue;
    const repo = projects.find((p) => p.project === x.project)?.path.split('/').filter((y) => y !== '').pop() ?? '';
    const list = m.get(x.project) ?? [];
    if (!list.some((r) => r.path === wt)) list.push({ path: wt, name: worktreeName(wt, repo) });
    m.set(x.project, list);
  }
  return m;
}

/** Each agent's next wake (its scheduled tasks' soonest run), '<project>/<agent>' -> ms. Pure. */
export function wakesOf(tasks: readonly { project: string; t: { agent: string; next_ms?: number | null } }[]): Map<string, number> {
  const soonest = new Map<string, number>();
  for (const x of tasks) {
    if (x.t.next_ms == null) continue;
    const k = `${x.project}/${x.t.agent}`;
    const was = soonest.get(k);
    if (was === undefined || x.t.next_ms < was) soonest.set(k, x.t.next_ms);
  }
  return soonest;
}
