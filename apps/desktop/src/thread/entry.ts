// What a thread row shows of an entry (the hub folds the transcript; this
// only picks words and the ds part). Owner: amb-win. Pure.
import type { Entry } from '@/proto';
import { thread } from '@/proto/words.gen';
import { NAMELESS_KINDS } from './tiers';
import { thinkingWords } from './thinking';
import { rhythmWords } from '@/scheduled/model';

/** A row's key in the thread list: an entry by its pos, a send not confirmed by its cid. */
export const entryKey = (pos: number) => `e:${pos}`;
export const pendingKey = (cid: string) => `p:${cid}`;

/**
 * His send not confirmed yet, drawn in the thread's tail? A queued one waits in the composer's
 * drawer (the hub's Agent.queued) and has no row until the hub says it: its entry then lands at its
 * place (his « pas dans l'historique avant d'avoir été envoyé »). Failed, or held by bise's route,
 * it keeps its row (retry and drop, the route's line). A steer always has its row. Pure.
 */
export function sendInThread(p: { readonly mode: string; readonly state: string }, held: boolean): boolean {
  return p.mode !== 'queued' || p.state === 'failed' || held;
}

/** The pos of an entry key, else undefined. */
export function posOfKey(key: string): number | undefined {
  if (!key.startsWith('e:')) return undefined;
  const n = Number(key.slice(2));
  return Number.isSafeInteger(n) ? n : undefined;
}

/**
 * One Intl.DateTimeFormat per form and zone: making one costs ~0.1 ms, and the rows asked for
 * thousands per render (a day line and a clock per row: thread-perf's profile, 0.4 s of a find).
 */
const formats = new Map<string, Intl.DateTimeFormat>();
function format(locale: string, opts: Intl.DateTimeFormatOptions, tz: string | undefined): Intl.DateTimeFormat {
  const key = `${locale}|${tz ?? ''}|${JSON.stringify(opts)}`;
  let f = formats.get(key);
  if (f === undefined) {
    f = new Intl.DateTimeFormat(locale, { ...opts, timeZone: tz });
    formats.set(key, f);
  }
  return f;
}
const CLOCK: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit', hour12: false };
const YMD: Intl.DateTimeFormatOptions = { year: 'numeric', month: '2-digit', day: '2-digit' };
const DAY_MONTH: Intl.DateTimeFormatOptions = { day: 'numeric', month: 'short' };
const WEEKDAY: Intl.DateTimeFormatOptions = { weekday: 'short', day: 'numeric', month: 'short' };

/** "14:05" in his time zone. */
export function clock(ms: number, tz?: string): string {
  if (ms <= 0) return '';
  return format('en-GB', CLOCK, tz).format(ms);
}

/**
 * A turn's time on hover (M14, the TUI's BISE-271): '14:05' today, '12 Oct 14:05' an
 * older day, '' without a time. Pure (`now` and `tz` given).
 */
export function whenOf(ms: number, now: number, tz?: string): string {
  if (ms <= 0) return '';
  const day = (t: number) => format('en-GB', YMD, tz).format(t);
  if (day(ms) === day(now)) return clock(ms, tz);
  const d = format('en-GB', DAY_MONTH, tz).format(ms);
  return `${d} ${clock(ms, tz)}`;
}

/** An agent's name as shown: main is 'bise' (agents/model shownName, kept local: entry.ts stays pure of the agents module). */
const shown = (agent: string): string => (agent === 'main' ? 'bise' : agent);

/** Who speaks in a message row: his words, the shown agent's (bise's pearl on bise's main), another agent's. */
export function whoOf(e: Entry, ctx: { agent: string; bise: boolean }): { from: 'you' | 'bise' | 'agent'; name: string; indent: boolean } {
  switch (e.kind) {
    case 'you':
      return { from: 'you', name: 'you', indent: false };
    case 'from_agent':
      // batch 3a: a message between two other agents says to whom ('perf → docs')
    {
      // a project's main is 'bise' in every name the window shows (ambient m_11437)
      const from = shown(e.from ?? thread.an_agent);
      return { from: 'agent', name: e.to != null && e.to !== '' && e.to !== ctx.agent ? `${from} → ${shown(e.to)}` : from, indent: true };
    }
    default:
      // every main is bise: its name line carries the 14 px pearl (designer m_12065 (1))
      return ctx.agent === 'main' ? { from: 'bise', name: 'bise', indent: false } : { from: 'agent', name: shown(ctx.agent), indent: false };
  }
}

/** The text ⌘F searches in an entry: its words, plus a tools entry's steps and a card's options. */
export function findText(e: Entry): string {
  const parts = [e.text];
  if (e.tools) for (const t of e.tools.items) parts.push(t.text);
  if (e.card) {
    parts.push(e.card.question);
    for (const o of e.card.options) parts.push(o.label);
  }
  if (e.page) parts.push(e.page.title);
  return parts.join('\n');
}

// ---- the thread's reading (ambient's spec m_11777, his try round 3; amb-home)

/** A name line opens a group when the author changes or after this much silence. */
export const GROUP_GAP_MS = 5 * 60_000;

/** The day a time falls on, in his time zone ('2026-10-06'). */
function dayOf(ms: number, tz?: string): string {
  return format('en-CA', YMD, tz).format(ms);
}

/** Two times on the same day (a time of 0 is never a new day). */
export function sameDay(a: number, b: number, tz?: string): boolean {
  return a <= 0 || b <= 0 || dayOf(a, tz) === dayOf(b, tz);
}

/** The day separator's words: 'today', 'yesterday', 'mon 6 oct'. Pure. */
export function dayLabel(ms: number, now: number, tz?: string): string {
  if (sameDay(ms, now, tz)) return 'today';
  if (sameDay(ms, now - 86_400_000, tz)) return 'yesterday';
  const f = format('en-GB', WEEKDAY, tz).formatToParts(ms);
  const part = (t: string) => f.find((p) => p.type === t)?.value ?? '';
  return `${part('weekday')} ${part('day')} ${part('month')}`.toLowerCase();
}

/** Under this a thinking shows nothing (ambient m_11777 (c), the TUI's: nothing was thought). */
export const THOUGHT_MS = 1000;

/**
 * Whether a thought shows a line, as the TUI draws one: always when it has words (once its
 * signature is cut), at any time ('∴ thought for 0.0s ▸' after a fast '[[think: …]]', qa-flows'
 * rows-batch2 on the real core, ambient-lead m_20357); without words only from 1 s (m_11777 (c):
 * nothing was thought). Pure.
 */
export function thoughtShows(t: { ms: number; text: string } | null | undefined): boolean {
  if (t == null) return false;
  return t.ms >= THOUGHT_MS || thinkingWords(t.text).trim() !== '';
}

/** A thinking row with nothing in it: under 1 s, no words thought and no head words of its own (or only the hub's 'thought for 0.0s'). Pure. */
export function thoughtNothing(e: Entry): boolean {
  return e.kind === 'thinking' && !thoughtShows(e.thinking ?? { ms: 0, text: '' }) && (e.text === '' || /^thought for 0(\.\d+)?s$/.test(e.text));
}

/** Who a row is by, for its group: his words, another agent's, the agent's own words and work; none: a line of the thread's own (a notice, a news line, a separator, a hidden thinking) that opens nothing, so the next message shows its name. */
export function authorOf(e: Entry, ctx: { agent: string; bise: boolean }): string | null {
  if (thoughtNothing(e)) return null;
  switch (e.kind) {
    case 'you':
      return 'you';
    case 'from_agent':
      return `from:${e.from ?? ''}:${e.to ?? ''}`;
    case 'to_agent':
      return `to:${e.to ?? ''}`;
    case 'notice':
    case 'compacting':
    case 'compacted':
    case 'not_delivered':
    case 'unknown':
    case 'card':
    case 'page':
    case 'report':
    case 'stopped':
    case 'landed':
    case 'pr':
    case 'artifact':
    case 'answered':
    case 'approval':
    case 'scheduled':
      return null;
    default:
      return `agent:${ctx.agent}`;
  }
}

/** The row opens a group (its name line shows): a new author, 5 min of silence, a new day, or no row before it. Pure. */
export function groupHead(prev: Entry | undefined, e: Entry, ctx: { agent: string; bise: boolean }, tz?: string): boolean {
  const a = authorOf(e, ctx);
  if (prev === undefined || a === null) return true;
  if (authorOf(prev, ctx) !== a) return true;
  if (e.at_ms > 0 && prev.at_ms > 0 && e.at_ms - prev.at_ms > GROUP_GAP_MS) return true;
  return !sameDay(prev.at_ms, e.at_ms, tz);
}

/**
 * The rows whose name line shows (tui-parity H2, designer m_19832): one head per run of the same
 * author, opened by a new author, his message or 5 min of silence; on the run's first row, a tool or
 * a thinking row too (designer m_21404 (3): a run never shows rows before its author; this replaces
 * m_19832's 'first row that is not work'). `carry`: kept for the live row's API, now always false
 * at the end (no run owes its head any more). News and plumbing rows draw no name and close a run.
 * Pure.
 */
export function namedRows(
  order: readonly number[],
  entry: (pos: number) => Entry | undefined,
  ctx: { agent: string; bise: boolean },
  tz?: string,
): { named: Set<number>; carry: boolean } {
  const named = new Set<number>();
  let prev: Entry | undefined;
  for (const pos of order) {
    const e = entry(pos);
    if (e === undefined) continue;
    if (!NAMELESS_KINDS.has(e.kind) && groupHead(prev, e, ctx, tz)) named.add(pos);
    prev = e;
  }
  return { named, carry: false };
}

const CONTEXT_BLOCK = /\n*<screen_context\b[^>]*>[\s\S]*?<\/screen_context>\s*/g;

/** His words without the fn context block the hub appended (the window shows a chip instead). Pure. */
export function stripContext(text: string): string {
  return text.replace(CONTEXT_BLOCK, '\n').trim();
}

/** The fn context chip: 'with Chrome · localhost:4801/sett…' (the app and the host whole, the path cut), what a click opens, and the other sources counted. Pure. */
export function chipOf(c: { app?: string | null; url?: string | null; title?: string | null; file?: string | null; shot?: string | null }): {
  label: string;
  open: { url: string } | { file: string } | null;
  tip: string | undefined;
} | null {
  const app = c.app ?? '';
  const sources: { what: string; open: { url: string } | { file: string } | null }[] = [];
  if (c.url) {
    const m = /^[a-z]+:\/\/([^/?#]+)(.*)$/i.exec(c.url);
    const host = m?.[1] ?? c.url;
    const rest = m?.[2] ?? '';
    const path = rest.length > 6 ? `${rest.slice(0, 5)}…` : rest;
    sources.push({ what: `${host}${path}`, open: { url: c.url } });
  }
  if (c.file) {
    const name = c.file.split('/').pop() ?? c.file;
    sources.push({ what: name.length > 24 ? `${name.slice(0, 23)}…` : name, open: { file: c.file } });
  }
  if (sources.length === 0 && c.title) sources.push({ what: c.title.length > 28 ? `${c.title.slice(0, 27)}…` : c.title, open: null });
  if (app === '' && sources.length === 0 && !c.shot) return null;
  const first = sources[0];
  const more = sources.length > 1 ? ` +${String(sources.length - 1)}` : '';
  const label = `${thread.ctx_with({ app: app || thread.ctx_screen })}${first ? ` · ${first.what}` : ''}${more}`;
  const shotOnly = sources.length === 0 && Boolean(c.shot);
  return { label, open: first?.open ?? null, tip: shotOnly ? thread.ctx_shot_gone : undefined };
}

/** His agents often start with their own name ('perf: the parsed copy…'): the name line already
 *  says it, so the row drops that prefix (ambient m_11934). Pure. */
export function dropAuthor(text: string, name: string): string {
  if (name === '' || !text.startsWith(`${name}:`)) return text;
  return text.slice(name.length + 1).trimStart();
}

// ---- the agent view's tool rows (designer's desktop-screens v6 (2); amb-home)

/** A step's length, as designer's rows say it: '38 s', '1 m 12 s', '2 m'. Under 1 s: none. Pure. */
export function stepTime(ms: number): string {
  const s = Math.floor(ms / 1000);
  if (s < 1) return '';
  if (s < 60) return `${String(s)} s`;
  const m = Math.floor(s / 60);
  const r = s % 60;
  return r === 0 ? `${String(m)} m` : `${String(m)} m ${String(r)} s`;
}

/**
 * A scheduled task's line without its number ('main scheduled #1 for dashboards · …' reads 'bise
 * scheduled dashboards · …'; designer N2), its agents named as the window names them (`shown`: main
 * is 'bise', designer m_40683; the TUI keeps 'main'). Pure.
 */
export function schedHead(head: string, agent: string | null = null): string {
  // never the internal number (designer m_14246): 'scheduled #48 ended' reads 'scheduled perf ended' in
  // that agent's thread, else 'a scheduled task ended'
  // its rhythm in the app's one form, as the right bar's: 'every 2m' reads 'every 2 min' (designer m_22403)
  return (
    rhythmWords(head)
      .replace(/\bscheduled #\d+ for /, 'scheduled ')
      .replace(/\bscheduled #\d+ (?=ended|stopped|failed)/, agent === null ? 'a scheduled task ' : `scheduled ${agent} `)
      // the hub's set_own '{agent} scheduled {title}' and set_for '… {title} for {agent}' (bise_proto::words::scheduled)
      .replace(/^(\S+)(?= scheduled )/, (a: string) => shown(a))
      .replace(/(?<= for )(\S+)(?= ·|$)/, (a: string) => shown(a))
  );
}

/** The argument a tool is about, in the order the args usually name it. */
const MAIN_ARGS = ['file_path', 'path', 'command', 'cmd', 'pattern', 'query', 'url', 'file', 'name', 'id', 'text'];

/**
 * A tool step's words, never its args object (ambient-pm m_12810, designer D15): the hub's
 * '<tool>: {"file_path":"panels.md"}' reads '<tool> panels.md' (its main argument in words, else its
 * first string one, else the tool alone); a bash step drops 'bash: ' ('$' says it already). Words
 * that aren't a tool and JSON stay as they are. Pure.
 */
export function toolWords(text: string): string {
  const t = text.trim();
  const m = /^([A-Za-z_][\w.-]*):\s*(\{[\s\S]*\})$/.exec(t);
  if (m !== null) {
    const tool = m[1] ?? '';
    let args: unknown;
    try {
      args = JSON.parse(m[2] ?? '');
    } catch {
      args = undefined;
    }
    if (args !== null && typeof args === 'object' && !Array.isArray(args)) {
      const o = args as Record<string, unknown>;
      const key = MAIN_ARGS.find((k) => typeof o[k] === 'string' && o[k] !== '') ?? Object.keys(o).find((k) => typeof o[k] === 'string' && o[k] !== '');
      const arg = key === undefined ? '' : String(o[key]).split('\n')[0]?.trim() ?? '';
      if (tool === 'bash') return arg !== '' ? arg : tool;
      return arg !== '' ? `${tool} ${arg}` : tool;
    }
  }
  return t.replace(/^bash:\s+/, '');
}

/**
 * A report as the TUI reads it (render.rs report_parts: '[report: <kind>] <body>'), never shown in its
 * bracket form (designer D14): a status line '<kind> · <first line>'; a blocked agent waiting on his
 * answer says only that ('blocked · waiting for your answer'): the question card under it says the
 * question. Words that already start with the kind ('blocked: signing key …') say it once, never
 * 'blocked · blocked: signing key …' (designer m_52857). null: not a report. Pure.
 */
export function reportLine(text: string, kind?: string | null): string | null {
  const m = /^\[report:\s*([^\]]+)\]\s*([\s\S]*)$/.exec(text.trim());
  const k = (m?.[1] ?? kind ?? '').trim();
  if (m === null && (kind == null || kind === '')) return null;
  const body = (m === null ? text : (m[2] ?? '')).trim();
  const first = body.split('\n')[0]?.trim() ?? '';
  const waiting = /^waiting for your answer\b/i.exec(first);
  const words = waiting !== null ? thread.report_waiting : first;
  if (words === '') return k;
  return k !== '' && saysKind(words, k) ? words : `${k} · ${words}`;
}

/** The words open on the report's kind, as a word of their own ('blocked: …', 'Done · …', 'failed'). */
function saysKind(words: string, k: string): boolean {
  const head = words.slice(0, k.length);
  return head.toLowerCase() === k.toLowerCase() && /^(?:$|[\s:·.,—-])/.test(words.slice(k.length));
}

/** A report's words without its '[report: kind]' tag (the whole of them, every line). Pure. */
export function reportBody(text: string): string {
  return text.trim().replace(/^\[report:\s*[^\]]+\]\s*/, '').trim();
}

const plain = (t: string) => t.toLowerCase().replace(/[`*_>#]/g, '').replace(/\s+/g, ' ').replace(/[.\s]+$/, '').trim();

/**
 * The message under a report says the same words (tui-parity H8: the TUI folds the report to one
 * line, the app drew both loud): its first line is in the message. Pure.
 */
export function repeatsReport(body: string, next: string): boolean {
  const first = plain(body.split('\n')[0] ?? '');
  return first.length >= 12 && plain(next).includes(first);
}

/**
 * A report the message right under it repeats (tui-parity H8): drawn as one quiet row
 * '✓ <summary>… ▸ report', so the rows around it sit as quiet rows do (4 px). Pure.
 */
export function repeatedReport(e: Pick<Entry, 'kind' | 'text' | 'report'>, next: Pick<Entry, 'kind' | 'text'> | undefined): boolean {
  if (next === undefined || !['report', 'from_agent', 'to_agent', 'agent'].includes(e.kind)) return false;
  if (next.kind !== 'agent' && next.kind !== 'from_agent' && next.kind !== 'to_agent') return false;
  if (reportStatus(e.text, e.kind === 'report' ? e.report?.kind : undefined) === null) return false;
  return repeatsReport(reportBody(e.text), next.text);
}

/** A report's status line in a thread, with its kind's mark as the TUI's report line: '✓ done · …', '✗ failed · …' (designer: '✓ done · <summary>'); the spotlight's preview marks its own lines. Pure. */
export function reportStatus(text: string, kind?: string | null): string | null {
  const line = reportLine(text, kind);
  if (line === null) return null;
  // the kind is the line's first word ('done · …', and said once: 'done: …')
  const k = (/^[^\s·:.,]+/.exec(line)?.[0] ?? '').toLowerCase();
  return `${k === 'done' ? '✓ ' : k.includes('fail') ? '✗ ' : ''}${line}`;
}

/** At most this many command and edit lines show; the rest fold into the counted line. */
export const TOOL_LINES = 6;

/** A tool call as the hub sends it (proto ToolItem): state, time, exit, error, code and output are R11's. */
export interface ToolLike {
  pos: number;
  at_ms: number;
  /** the tool's name ('bash', 'run_typescript'): which language its code is */
  name?: string | null;
  /** the model's one line for it (its 'description'): words, not machine text */
  intent?: string | null;
  text: string;
  kind: string;
  state?: string;
  ms?: number | null;
  exit?: number | null;
  err?: string | null;
  code?: string | null;
  out?: string | null;
  /** an edit's files with their line counts (hub-rows: ToolItem.files; a move's path is 'old → new') */
  files?: readonly { path: string; add: number; del: number }[] | null;
}

/** A shown call: its mark, words, its state's words on the right ('✓ 0.3 s', '✗ exit 1 · 2.1 s'), its first error line, and what opens under it. */
export interface ToolLine {
  pos: number;
  /** '$' a command, 'ƒ' a TypeScript run (the TUI's), '±' an edit, '' a read, search or other step (its verb says it) */
  mark: '$' | 'ƒ' | '±' | '';
  text: string;
  /** its words are the model's description (sans), not a command or a path (mono): designer m_22284 */
  said: boolean;
  /** the step's length until the next one (a replay, no state) */
  time: string;
  state: 'run' | 'ok' | 'err' | '';
  end: string;
  err: string;
  /** the source it ran, decoded (toolSource): never its args' JSON */
  code: string;
  /** that source's language for its colours: 'typescript', 'bash', or '' (plain) */
  lang: string;
  out: string;
  /** an edit's line counts, '+18 −6' (dim after its words); '' when the hub has none */
  counts: string;
}

const base = (p: string): string => p.split('/').filter(Boolean).pop() ?? p;

/**
 * An edit's words from its files (designer m_12065, '± Hero.tsx +18 −6'): the file's name (a move
 * 'old.rs → new.rs'), 'and N more' for several, and the lines added and deleted over all of them.
 * None or no counts: null (the call's own words stay). Pure.
 */
export function editWords(files: ToolLike['files']): { text: string; counts: string } | null {
  if (files == null || files.length === 0) return null;
  const first = files[0];
  if (first === undefined) return null;
  const name = first.path.includes(' → ') ? first.path.split(' → ').map(base).join(' → ') : base(first.path);
  const more = files.length > 1 ? ` and ${String(files.length - 1)} more` : '';
  const add = files.reduce((n, f) => n + f.add, 0);
  const del = files.reduce((n, f) => n + f.del, 0);
  return { text: `${name}${more}`, counts: add === 0 && del === 0 ? '' : `+${String(add)} −${String(del)}` };
}

/**
 * A tools row's line lines added and deleted by its edits (QA §4: 'edited 3 files +42 −18 ›', the
 * hub's counts summed, the window's sum like the folds'); none when no edit counted a line. Pure.
 */
export function editCounts(items: readonly Pick<ToolLike, 'kind' | 'files'>[]): { add: number; del: number } | null {
  let add = 0;
  let del = 0;
  for (const t of items) {
    if (t.kind !== 'edit') continue;
    for (const f of t.files ?? []) {
      add += f.add;
      del += f.del;
    }
  }
  return add === 0 && del === 0 ? null : { add, del };
}

/** A JSON string field's value, decoded, read even from a capped payload (the TUI's render.rs
 *  json_str_field): a cut string keeps what came. Pure. */
export function jsonStrField(s: string, field: string): string | null {
  const m = new RegExp(`"${field}"\\s*:\\s*"`, 'u').exec(s);
  if (m === null) return null;
  let out = '';
  const esc: Record<string, string> = { n: '\n', t: '\t', r: '\r', '"': '"', '\\': '\\', '/': '/' };
  for (let i = m.index + m[0].length; i < s.length; i++) {
    const c = s[i] ?? '';
    if (c === '"') return out;
    if (c !== '\\') {
      out += c;
      continue;
    }
    const n = s[i + 1];
    i++;
    if (n === undefined) return out + '\\';
    if (n === 'u' && /^[0-9a-f]{4}$/iu.test(s.slice(i + 1, i + 5))) {
      out += String.fromCharCode(parseInt(s.slice(i + 1, i + 5), 16));
      i += 4;
    } else out += esc[n] ?? `\\${n}`;
  }
  return out;
}

/**
 * The source a code tool ran, as the TUI shows it in its box (code.rs tool_source): run_typescript's
 * 'code' field, bash's command, decoded, never the args' JSON (his shot, main m_22134); its
 * description is the row's words already. Anything else: its code as it came. Pure.
 */
export function toolSource(name: string | null | undefined, code: string): { code: string; lang: string } {
  const json = code.trimStart().startsWith('{');
  if (name === 'run_typescript' || (json && name == null && /"code"\s*:/u.test(code))) return { code: (json ? jsonStrField(code, 'code') : null) ?? code, lang: 'typescript' };
  if (name === 'bash') {
    const cmd = json ? (jsonStrField(code, 'command') ?? jsonStrField(code, 'arg') ?? jsonStrField(code, 'cmd')) : null;
    return { code: cmd ?? code, lang: 'bash' };
  }
  return { code, lang: '' };
}

/** A call's own time, the TUI's: '0.3 s', '4.1 s', '12 s', '1 m 4 s'. Pure. */
export function toolTime(ms: number): string {
  // rounded to the tenth as the TUI's {:.1} (a 60 ms call reads '0.1 s' in both, qa-flows m_14191)
  if (ms < 10_000) return `${(ms / 1000).toFixed(1)} s`;
  return stepTime(ms);
}

/** What a call's line says on its right, as the TUI's tool row: ok '✓ 0.3 s' (dim), failed '✗ exit 1 · 0.3 s' (err), a replay its step's length. Pure. */
export function toolEnd(t: ToolLike, time: string): string {
  // a call that rounds to 0.0 s says '✓' alone (designer m_15569: never '✓ 0.0 s')
  const took = t.ms != null && t.ms >= 50 ? toolTime(t.ms) : '';
  if (t.state === 'ok') return took === '' ? '✓' : `✓ ${took}`;
  if (t.state === 'err') {
    const exit = t.exit != null && t.exit !== 0 ? `exit ${String(t.exit)}` : '';
    return ['✗', [exit, took].filter((x) => x !== '').join(' · ')].filter((x) => x !== '').join(' ');
  }
  if (t.state === 'run') return '';
  return time;
}

/** A turn's tools: its commands ('$ …') and edits ('± …') as lines with their length (until the next step), and the counted words of the rest ('read 6 files'), which fold. Pure. */
export function toolLines(items: readonly ToolLike[]): {
  lines: ToolLine[];
  rest: ToolLine[];
  restWords: string;
} {
  const lines: ToolLine[] = [];
  const rest: ToolLine[] = [];
  items.forEach((t, i) => {
    const next = items[i + 1];
    const time = next !== undefined && t.at_ms > 0 && next.at_ms > t.at_ms ? stepTime(next.at_ms - t.at_ms) : '';
    // R11/S9: a failed call always shows as its own line, whatever its kind (the TUI's ✗ row)
    const failed = t.state === 'err';
    const edit = t.kind === 'edit' ? editWords(t.files) : null;
    const line: ToolLine = {
      pos: t.pos,
      mark: t.kind === 'run' ? (t.name === 'run_typescript' ? 'ƒ' : '$') : t.kind === 'edit' ? '±' : '',
      said: t.intent != null && t.intent.trim() !== '' && t.kind !== 'edit' && t.kind !== 'read' && t.kind !== 'search',
      // a read or search says the TUI's verb, like its fold ('read gone.rs', designer m_14103/m_14269)
      text: t.kind === 'read' || t.kind === 'search' ? `${t.kind} ${toolWords(t.text).replace(/^\S+\s+/, '')}`.trim() : t.kind === 'edit' ? (edit?.text ?? toolWords(t.text)) : toolWords(t.text),
      counts: t.kind === 'edit' ? (edit?.counts ?? '') : '',
      time,
      state: t.state === 'run' || t.state === 'ok' || t.state === 'err' ? t.state : '',
      end: toolEnd(t, time),
      err: failed ? (t.err ?? '').trim() : '',
      ...toolSource(t.name, t.code ?? ''),
      out: t.out ?? '',
    };
    if (((t.kind === 'run' || t.kind === 'edit') && lines.length < TOOL_LINES) || failed) lines.push(line);
    // the folded ones open to the same rows as the lines (designer m_14269: no bullets, no fill)
    else rest.push(line);
  });
  const lined = new Set(lines.filter((l) => l.mark === '').map((l) => l.pos));
  const n = (k: string) => items.filter((t) => t.kind === k && !lined.has(t.pos)).length;
  const shownRun = lines.filter((l) => l.mark === '$' || l.mark === 'ƒ').length;
  const shownEdit = lines.filter((l) => l.mark === '±').length;
  // words::summary's words, without the steps shown as lines
  const parts: string[] = [];
  if (n('read') > 0) parts.push(thread.did_read({ n: n('read') }));
  if (n('edit') - shownEdit > 0) parts.push(thread.did_edit({ n: n('edit') - shownEdit }));
  if (n('run') - shownRun > 0) parts.push(thread.did_run({ n: n('run') - shownRun }));
  if (n('search') > 0) parts.push(n('search') === 1 ? thread.did_search_once : thread.did_search({ n: n('search') }));
  if (n('other') > 0) parts.push(thread.did_other({ n: n('other') }));
  return { lines, rest, restWords: parts.join(', ') };
}

/**
 * A tools row's line (his m_49672): the last description a command there has (its intent: 'start
 * the dev server'), one command or a group; none: the hub's counted words ('ran 3 commands').
 * `count`: a group's commands after its description (designer m_49693: 'run the hub tests · 3
 * commands'), '' for one command or the counted words. `drawn`: the call at work whose intent the
 * step fold already says, never said twice. Pure.
 */
export function toolsWords(items: readonly ToolLike[], counted: string, drawn?: number): { words: string; count: string } {
  const runs = items.filter((t) => t.kind === 'run');
  const said = runs.filter((t) => t.pos !== drawn && (t.intent ?? '').trim() !== '').at(-1);
  if (said === undefined) return { words: counted, count: '' };
  return { words: (said.intent ?? '').trim(), count: runs.length > 1 ? thread.run_count({ n: runs.length }) : '' };
}

/** A handoff in a main's thread (the hub's spawn notice '✚ main → new agent @perf : <brief>'): who it went to and the brief's first line; none for any other notice. Pure. */
export function handoffOf(text: string): { agent: string; brief: string } | null {
  const m = /^✚ \S+ → (?:new agent|new task|nouvelle tâche) @([A-Za-z0-9_-]+)(?: : ([\s\S]*))?$/.exec(text.trim());
  if (m === null) return null;
  return { agent: m[1] ?? '', brief: (m[2] ?? '').split('\n')[0]?.trim() ?? '' };
}

/**
 * The hub's raw line when he answered main's own notification (sb-core's notify_main, until amb-feed
 * folds it to an approval entry): 'the user answered notification #1 ("merge checkout into main? (2 files, tests
 * pass) 1. merge 2. not yet"): "[Image #1] this one"'. Its card, the question without its numbered
 * options, and his words; none for any other text. Designer's must-fix (m_15195): never the raw
 * line or the old product name. Pure.
 */
export function answeredCardOf(text: string): { card: number; question: string; words: string } | null {
  // 'card' in transcripts written before notif-rename, read for good. TODO(answer-route): read the
  // route line's typed card instead of these words.
  const m = /^the user answered (?:notification|card) #(\d+) \("([\s\S]*)"\):\s*"([\s\S]*)"\s*$/.exec(text.trim());
  if (m === null) return null;
  const q = (m[2] ?? '').replace(/\s+/gu, ' ').trim();
  // the card's options ride after its question: ' 1. merge 2. not yet'
  const question = q.replace(/\s+1\.\s[\s\S]*$/u, '').trim();
  return { card: Number(m[1]), question, words: (m[3] ?? '').trim() };
}

/**
 * Which side of the column a row sits on (designer's messages-right v1): everything he said goes
 * right (his messages, his answers to cards, the hub's raw line of his answer to main's card);
 * what bise or an agent said or did stays left (a task's brief too: the hub wrote it). Pure.
 */
export function sideOf(e: Entry | undefined): 'you' | 'them' {
  if (e === undefined) return 'them';
  if (e.kind === 'you') return briefOf(e.text) === null ? 'you' : 'them';
  if (e.kind === 'approval') return 'you';
  if ((e.kind === 'from_agent' || e.kind === 'agent') && answeredCardOf(e.text) !== null) return 'you';
  return 'them';
}

/**
 * Where each row shows (amb-feed 9633cd2a, designer m_16435): a message the agent read mid-turn
 * (a steer, `steered_at` = the pos of the hub's receipt, the same for a bundled steer) sits right
 * before the first row whose pos is past that receipt (after the work it read it under), else at
 * the end; every other row keeps its pos order. A steered_at at or before its own pos moves
 * nothing. Pure.
 */
export function placeSteered(order: readonly number[], steeredAt: (pos: number) => number | null | undefined): readonly number[] {
  const moved = new Map<number, number>();
  for (const pos of order) {
    const at = steeredAt(pos);
    if (at != null && at > pos) moved.set(pos, at);
  }
  if (moved.size === 0) return order;
  const rest = order.filter((pos) => !moved.has(pos));
  const out: number[] = [];
  const waiting = [...moved.entries()];
  for (const pos of rest) {
    // the steers whose receipt is before this row go in front of it, in their own order
    for (let i = 0; i < waiting.length; ) {
      const w = waiting[i];
      if (w !== undefined && w[1] < pos) {
        out.push(w[0]);
        waiting.splice(i, 1);
      } else i += 1;
    }
    out.push(pos);
  }
  for (const [pos] of waiting) out.push(pos);
  return out;
}

/** His bubbles closer than this make one run: its time shows under the last one only. */
export const RUN_MS = 2 * 60_000;

/** Is this bubble of his the last of its run (the next row isn't his, or comes 2 min later)? Pure. */
export function lastOfRun(e: Entry, next: Entry | undefined): boolean {
  if (next === undefined || sideOf(next) !== 'you') return true;
  return e.at_ms <= 0 || next.at_ms <= 0 || next.at_ms - e.at_ms > RUN_MS;
}

/**
 * His message's delivery mark after the time under its bubble (designer's messages-right v3, 'got it,
 * read it'; the TUI's render.rs mark_span): '✓' faint when the agent got it, '✓✓' dim when it read
 * it, never the accent; a tooltip says who. A failed one says '▲ not sent' in the error tone (its
 * 'send again' is the not-delivered row's). Sent, unknown or none: no mark, the time alone. Pure.
 */
export function deliveryMark(d: Entry['delivery'], who: string): { glyph: string; tip: string; tone: 'got' | 'read' | 'failed' } | null {
  switch (d) {
    case 'received':
      return { glyph: '✓', tip: thread.got_it({ who }), tone: 'got' };
    case 'read':
      return { glyph: '✓✓', tip: thread.read_it({ who }), tone: 'read' };
    case 'failed':
      return { glyph: thread.not_sent, tip: thread.didnt_get_it({ who }), tone: 'failed' };
    default:
      return null;
  }
}

/** The hub's brief, the first message of a task's thread ('# Task `gift-ui`', then 'Objective: …'; the TUI's is_brief): its name, objective and body without the heading; none for any other text. Pure. */
export function briefOf(text: string): { name: string; objective: string; body: string } | null {
  const m = /^# Task `([^`]+)`[^\n]*\n?/.exec(text);
  if (m === null) return null;
  const body = text.slice(m[0].length).replace(/^\n+/, '').trimEnd();
  const objective = /^Objective:\s*(.*)$/m.exec(body)?.[1]?.trim() ?? '';
  return { name: m[1] ?? '', objective, body };
}
