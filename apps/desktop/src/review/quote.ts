// V7 / L11: quote a diff's lines (the TUI's diffquote.rs, one behavior with the thread's quote):
// drag or shift+↑↓ over lines in the review selects whole lines, then a typed key puts them in
// the composer of the agent in view as quotes, one per file the selection touches, each saying
// where ('src/a.rs:12-14 · 3 lines'). The review has no composer: it opens the agent's thread and
// the quotes go in there with the key (the TUI closes its full-screen panel the same way).
// Pure, except the one pending hand-off to the thread. Owner: amb-win.
import type { DiffLine as DsLine } from '@/ds';
import type { QuoteAt } from '@/composer/message';

/** A row of the review: file `f`, line `l` of linesOf(file). */
export interface RowAt {
  readonly f: number;
  readonly l: number;
}

/** The selection: rows `anchor..=head` of the flat list (either way). */
export interface Sel {
  readonly anchor: number;
  readonly head: number;
}

export interface DiffQuote {
  readonly at: QuoteAt;
  readonly text: string;
}

/** Every file's lines in one list, in order. Pure. */
export function flatRows(files: readonly (readonly DsLine[])[]): RowAt[] {
  return files.flatMap((ls, f) => ls.map((_, l) => ({ f, l })));
}

export function range(s: Sel): [number, number] {
  return s.anchor <= s.head ? [s.anchor, s.head] : [s.head, s.anchor];
}

/** `12`, `12-14`, '' for none (diffquote.rs span). */
export function span(nums: readonly number[]): string {
  if (nums.length === 0) return '';
  const a = Math.min(...nums);
  const b = Math.max(...nums);
  return a === b ? String(a) : `${String(a)}-${String(b)}`;
}

const isCode = (l: DsLine): boolean => l.old != null || l.new != null;

/** The selected rows as quotes, one per file they touch, in order; hunk heads left out (diffquote.rs quotes). Pure. */
export function quotesOf(paths: readonly string[], files: readonly (readonly DsLine[])[], rows: readonly RowAt[], s: Sel): DiffQuote[] {
  const [a, b] = range(s);
  const out: { f: number; lines: DsLine[] }[] = [];
  for (const r of rows.slice(a, b + 1)) {
    const line = files[r.f]?.[r.l];
    if (line === undefined || !isCode(line)) continue;
    const last = out.at(-1);
    if (last !== undefined && last.f === r.f) last.lines.push(line);
    else out.push({ f: r.f, lines: [line] });
  }
  return out.map(({ f, lines }) => ({
    at: {
      file: paths[f] ?? '',
      new: span(lines.flatMap((l) => (l.new == null ? [] : [l.new]))),
      old: span(lines.flatMap((l) => (l.old == null ? [] : [l.old]))),
    },
    text: lines.map((l) => `${l.sign === '+' || l.sign === '-' ? l.sign : ' '}${l.text}`.replace(/\r?\n$/u, '')).join('\n'),
  }));
}

/** Whether row `l` of file `f` is selected. Pure. */
export function picked(rows: readonly RowAt[], s: Sel | null, f: number, l: number): boolean {
  if (s === null) return false;
  const [a, b] = range(s);
  for (let i = a; i <= b; i++) {
    const r = rows[i];
    if (r !== undefined && r.f === f && r.l === l) return true;
  }
  return false;
}

/** The quotes waiting for the thread the review opened, with the key he typed. */
export interface Pending {
  readonly project: string;
  readonly agent: string;
  readonly from: string;
  readonly quotes: readonly DiffQuote[];
  readonly key: string;
}

let pending: Pending | null = null;
/** the threads already shown (the review in the panel, its thread in the centre: page pr-guided) */
const shown = new Set<() => void>();

/** The review hands its quotes to the thread it opens; a thread already shown is told at once. */
export function handQuotes(p: Pending): void {
  pending = p;
  for (const tell of [...shown]) tell();
}

/** A shown thread hears of quotes handed after it mounted (it takes its own with takeQuotes). */
export function onQuotes(tell: () => void): () => void {
  shown.add(tell);
  return () => {
    shown.delete(tell);
  };
}

/** The thread takes the quotes meant for it (once). */
export function takeQuotes(project: string, agent: string): Pending | null {
  const p = pending;
  if (p === null || p.project !== project || p.agent !== agent) return null;
  pending = null;
  return p;
}
