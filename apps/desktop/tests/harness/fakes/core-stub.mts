// The scripted stand-in for `bise ambient-core` in a test run: src/main/core spawns
// `<BISE_DESKTOP_BISE> ambient-core --workspace <ws>` (or --home); the harness points
// BISE_DESKTOP_BISE at a sh wrapper that runs this file (fakes/helper.ts), so no bise, no hub, no
// provider runs. Arguments are ignored.
//   stdout  the start world at once, then every line a test sends to the control socket
//           (a core or hub event each: bise-proto's CoreEv | HubEv)
//   stdin   the app's requests (JSON-RPC), each appended to the log as the command it
//           carries (AppCmd | HubCmd): a test reads what the app sent ("a sent message does
//           nothing" is a send that never reaches here)
//   stdout  JSON-RPC: initialize answered, the events as notifications once initialized
// env: BISE_HARNESS_CORE_EVENTS  the start world: a JSONL file of events; unset = every fixture of
//                                rust/proto/fixtures (core_ev.jsonl then hub_ev.jsonl), 'none' = nothing
//      BISE_HARNESS_CORE_SOCK    the unix socket a test writes events to (fakes/core.ts)
//      BISE_HARNESS_CORE_LOG     the file the commands go to
// (src/main/core/env.ts drops BISE_DESKTOP_* from the core's env; BISE_HARNESS_* passes.)
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runStub, type Cmd } from './stub-lib.mts';
import { cardsOf, startRows, withCards, type Card, type Row } from './startrows.mts';

/** A world line (an event, JSON): the fields the stub reads of each kind. */
type Item = { pos: number; head?: string; rule?: string; meta?: string; body?: { text?: string } };
type Ev = {
  ev?: string; project?: unknown; agent?: unknown; before?: number | null; merged?: unknown; id?: unknown;
  entries?: { pos: number }[]; items?: Item[]; slot?: { slot: unknown; req: number }; raw?: unknown;
  cards?: Card[]; others?: Card[]; projects?: Row[];
  [k: string]: unknown;
};
const parse = (l: string): Ev => JSON.parse(l) as Ev;

const FIXTURES = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..', '..', '..', 'rust', 'proto', 'fixtures');
const jsonl = (file: string) => readFileSync(file, 'utf8').split(String.fromCharCode(10)).map((l) => l.trim()).filter(Boolean);

const world = process.env.BISE_HARNESS_CORE_EVENTS;
// The fixtures' `broken` (a core with no runtime: no hub can start) is not part of the healthy
// world: a test that wants it sends it (app/keep, app/core-line went red with its ▲ line on).
// Nor the fixtures' `machine_update` (a bise update 'updating', e2b68728b): a still machine updates
// nothing, and its running line outranks the core's done one in the one status pill, so it hid
// 'bise is back.' (native/crash red since e2b68728b, bisected by desk-checks).
const NOT_HEALTHY = new Set(['broken', 'machine_update']);
// Nor the fixtures' `app_open`: the hub's one-shot answer to his 'app ↗' (a view's tab opens), never
// a state of the world; from the start world it opened a view's tab over every check's thread
// (scroll-steady, compact-fold: its box took their clicks and wheels). A check that wants it sends it.
// Dropped from a check's own world too: running/processes builds its world from the fixtures, and
// the view's failed box in the panel took the click on its row (red alone since 85d2e95f3 left it in).
const ASKED = new Set(['app_open']);
const all = (world === 'none' ? []
  : world ? jsonl(world)
  : [...jsonl(path.join(FIXTURES, 'core_ev.jsonl')), ...jsonl(path.join(FIXTURES, 'hub_ev.jsonl'))].filter((l) => !NOT_HEALTHY.has(parse(l).ev ?? '')))
  .filter((l) => !ASKED.has(parse(l).ev ?? ''));
// A restart (FakeCore.exit: the app's main spawns the stub again) sends the start world again, but
// never the hub's one-shot ask: a `confirm` goes to the connection whose command asked it, and a
// real core restart is a new connection, so it is gone (app/keep-all :166 red: the replayed
// 'archive it anyway? (y/n)' bar took the keys from Tune's half-typed model id).
const started = process.env.BISE_HARNESS_CORE_LOG ? `${process.env.BISE_HARNESS_CORE_LOG}.started` : null;
const again = started !== null && existsSync(started);
if (started !== null) writeFileSync(started, '');
// Nor his voice answer (bar N14): a `voice_answer` is what the core heard once; the real core never
// says it again after a restart, and the inbox block, still mounted, took the replayed one for a new
// answer to card 9 and dropped it (app/keep-all's ⌘I case: '2 waiting' after the restart, not 3).
const ONCE = new Set(['confirm', 'voice_answer']);
const start = again ? all.filter((l) => !ONCE.has(parse(l).ev ?? '')) : all;

// The world is written at spawn: the app's main keeps the newest state events and replays them to
// a window that loads later (amb-core 162ec18b). A thread's lines aren't state: like the real core,
// the stub sends them when the window asks (subscribe, page) and answers the per-project asks
// (diff, worktrees, dev_servers, merged, processes, proc_out, inbox_history) from the same world.
const evs = start.map(parse).filter((ev) => ev.ev !== 'transcript');
// A world may give an agent's whole thread as one `{ev: 'transcript', project, agent, entries}` (the
// harness's own line, never sent to the app): then subscribe and page answer as the hub does,
// paged by pos (thread/subscribe: the newest `limit`, 40; thread/page: the newest `limit` before
// `before`), so a test can reach an entry pages back (inbox-open: the window around his answer).
const transcripts = start.map(parse).filter((ev) => ev.ev === 'transcript');
function paged(cmd: Cmd): Ev[] | null {
  const t = transcripts.find((ev) => same(ev, cmd));
  if (t === undefined) return null;
  const under = (t.entries ?? []).filter((e) => cmd.before === undefined || cmd.before === null || e.pos < cmd.before);
  const n = Math.max(1, Math.min(200, cmd.limit ?? 40));
  const entries = under.slice(-n);
  const more = under.length > entries.length;
  return [{ ev: 'thread', project: cmd.project, agent: cmd.agent, entries, before: more ? (entries[0]?.pos ?? null) : null, more }];
}
const same = (ev: Ev, cmd: Cmd) => ev.project === cmd.project && (cmd.agent === undefined || ev.agent === undefined || ev.agent === cmd.agent);
function answer(cmd: Cmd): Ev[] {
  const of = (kinds: string[], more: (ev: Ev) => boolean = () => true) => evs.filter((ev) => kinds.includes(ev.ev ?? '') && same(ev, cmd) && more(ev));
  const t = cmd.cmd === 'subscribe' || cmd.cmd === 'page' ? paged(cmd) : null;
  if (t !== null) return t;
  switch (cmd.cmd) {
    case 'subscribe': return of(['thread', 'entry', 'typing'], (ev) => ev.ev !== 'thread' || ev.before === null || ev.before === undefined);
    case 'page': return of(['thread'], (ev) => ev.before !== null && ev.before !== undefined);
    // a PR tab's Changes (pr-artifacts): the world's agent diff, answered by its req (the hub's
    // pick of place or forge is the hub's; the window knows the answer by its req alone)
    case 'diff':
      if (cmd.pr_url) return of(['diff'], (ev) => !ev.merged && ev.agent === 'perf').slice(0, 1).map((ev) => ({ ...ev, req: cmd.req }));
      return of(['diff'], (ev) => (cmd.commit ? ev.merged === cmd.commit : !ev.merged));
    // a PR tab's Summary and a check's log (pr/detail, pr/checkLog): the world's answer with a
    // detail (or lines) for that PR, else none (the tab says what it waits for), echoing the req;
    // his Merge or Close: done (pr/act)
    case 'pr_detail': return of(['pr_detail'], (ev) => ev.url === cmd.url && ev.detail != null).slice(-1).map((ev) => ({ ...ev, req: cmd.req }));
    case 'pr_check_log': return of(['pr_check_log'], (ev) => ev.url === cmd.url && ev.lines != null).slice(-1).map((ev) => ({ ...ev, check: cmd.check, req: cmd.req }));
    case 'pr_act': return [{ ev: 'pr_acted', project: cmd.project, req: cmd.req, url: cmd.url, act: cmd.act }];
    case 'worktrees': case 'dev_servers': case 'merged': case 'processes': return of([cmd.cmd]);
    // a process's output (the running screen's terminal): the world's ranges of that one, in order
    case 'proc_out': return of(['proc_out'], (ev) => ev.id === cmd.id);
    // an @ query (at-chips): the world's newest `files` list of that project, whatever the query,
    // answered with the ask's own rid and query
    case 'files': return of(['files']).slice(-1).map((ev) => ({ ...ev, q: cmd.q, rid: cmd.rid }));
    // the inbox screen's history (inbox/history): the world's page of that project; an older one
    // (before) has none
    case 'inbox_history': return cmd.before == null ? of(['inbox_history']) : [];
    // /debug history: the world's pages of that agent's session, the newest (no before) or the one
    // before a pos; a find: the pos of every row of every page that holds it (any case), no rows
    case 'log': {
      if (cmd.find !== undefined) {
        const n = (typeof cmd.find === 'string' ? cmd.find : JSON.stringify(cmd.find)).toLowerCase();
        const rows = of(['log']).flatMap((ev) => ev.items ?? []);
        const has = (it: Item) => [it.head, it.rule ?? '', it.meta ?? '', it.body?.text ?? ''].join('\n').toLowerCase().includes(n);
        const hits = [...new Set(rows.filter(has).map((it) => it.pos))].sort((a, b) => a - b);
        return [{ ev: 'log', project: cmd.project, agent: cmd.agent, items: [], more: false, hits }];
      }
      const newest = Math.max(...of(['log']).map((e) => e.items?.[0]?.pos ?? 0));
      const before = cmd.before;
      return of(['log'], (ev) => (before === undefined || before === null ? (ev.items?.[0]?.pos ?? 0) >= newest : (ev.items?.at(-1)?.pos ?? 0) < before)).slice(-1);
    }
    // /debug completion: the world's request in that slot, else its newest; raw: the world's raw
    // one when it has it
    case 'completion': {
      const all = of(['completion'], (ev) => (cmd.slot === undefined || ev.slot?.slot === cmd.slot) && (ev.raw === true) === (cmd.raw === true));
      return [...all].sort((a, b) => (a.slot?.req ?? 0) - (b.slot?.req ?? 0)).slice(-1).map((ev) => Object.fromEntries(Object.entries(ev).filter(([k]) => k !== 'raw')));
    }
    default: return [];
  }
}
// The start rows list each project's cards as the real core does (startrows.mts: a project's first
// `cards` line), and a `cards` line a test sends later is followed by the rows it changes.
const startLines = startRows(start.filter((l) => parse(l).ev !== 'transcript'), evs);
let rows: Row[] | null = null;
let seen = 0;
function follow(line: string): string[] {
  seen += 1;
  let ev: Ev | null;
  // a test's line may be any JSON, null too
  try { ev = JSON.parse(line) as Ev | null; } catch { return []; }
  if (ev?.ev === 'projects' && Array.isArray(ev.projects)) {
    rows = ev.projects;
    return [];
  }
  // the start world's own cards are in its rows already
  if (seen <= startLines.length) return [];
  if (ev?.ev !== 'cards' || typeof ev.project !== 'string' || !Array.isArray(ev.cards) || rows === null) return [];
  const project = ev.project;
  if (!rows.some((r) => r.project === project)) return [];
  const next = withCards(rows, new Map([[ev.project, cardsOf(ev)]]));
  if (JSON.stringify(next) === JSON.stringify(rows)) return [];
  rows = next;
  return [JSON.stringify({ ev: 'projects', projects: rows })];
}

// JSON-RPC like `ambient-core`: the HubEv tags (hub/<tag>) from the generated names
const NAMES = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'src', 'proto', 'app_methods.gen.ts');
const listed = /export const NOTIFICATIONS = (\[.*\]) as const;/.exec(readFileSync(NAMES, 'utf8'))?.[1];
if (!listed) throw new Error(`no NOTIFICATIONS in ${NAMES}`);
const notes = JSON.parse(listed) as string[];
const hubEvs = notes.filter((n) => n.startsWith('hub/')).map((n) => n.slice(4));
runStub({ start: startLines, sock: process.env.BISE_HARNESS_CORE_SOCK, log: process.env.BISE_HARNESS_CORE_LOG, answer, rpc: { hubEvs }, follow });
