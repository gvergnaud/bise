// The review's one reading column (page review-guided, Gabriel's pick B), as data: its sections in
// order, each a head and blocks of cards, for both modes. By kind: the simple mode's parts
// (parts.ts), a folder line over each folder's cards. Guide: the hub's guide (guide.rs), its parts
// in order, each its hunks in reading order as cards (one per run of the same file), then 'The
// rest', then Generated. A guide never hides code (architect m_57259): every hunk is drawn once,
// the weight is a label only; only a path rule (Generated) or his own fold folds. Pure.
import type { DiffFile, Guide, Weight } from '@/proto';
import type { DiffLine as DsLine } from '@/ds';
import type { Diff } from '@/store';
import { fileHead, linesOf } from './model';
import { counts, framed, kindOf, partsOf, type Kind } from './parts';

/** One file's diff card, or one run of its hunks in a guide's part. */
export interface Card {
  readonly key: string;
  /** the file's path: the quote's, the tip's */
  readonly path: string;
  /** what its head says: the path, or its name under a folder line */
  readonly head: string;
  readonly short: boolean;
  readonly add: number;
  readonly del: number;
  /** its lines, none when he marked its file viewed (his fold) */
  readonly lines: readonly DsLine[];
  readonly note: string | null;
  /** its file's viewed key: the path and its hunks' ids (a push that changes it un-views it) */
  readonly file: string;
  readonly viewed: boolean;
}

/** A folder's cards (By kind), or a part's (Guide: no folder line). */
export interface Block {
  readonly folder: string | null;
  readonly cards: readonly Card[];
}

export type SectionKind = Kind | 'part' | 'rest';

export interface Section {
  /** 'code', 'generated', 'part-2', 'rest' */
  readonly key: string;
  readonly kind: SectionKind;
  /** a kind's word or a part's area */
  readonly label: string;
  /** a guide's part: its title and summary */
  readonly title: string | null;
  readonly summary: string | null;
  readonly weight: Weight | null;
  /** '2 files · +21 −5' */
  readonly facts: string;
  /** '2 files', '+21 −5': its head puts a folded one's names between */
  readonly count: string;
  readonly sum: string;
  /** its files' names, for its folded head */
  readonly names: readonly string[];
  readonly folded: boolean;
  readonly blocks: readonly Block[];
}

export interface Words {
  readonly kind: Readonly<Record<Kind, string>>;
  readonly files: (n: number) => string;
  readonly root: string;
  readonly rest: string;
}

const baseName = (p: string) => p.slice(p.lastIndexOf('/') + 1);

/** A file's viewed key: its path and its hunks' ids (none on an older hub: the path alone). */
export function fileKey(f: DiffFile): string {
  return [f.path, ...f.hunks.map((h) => h.id ?? '')].join(' ');
}

function card(f: DiffFile, i: number, run: number, lines: readonly DsLine[], short: boolean, viewed: ReadonlySet<string>): Card {
  const file = fileKey(f);
  const seen = viewed.has(file);
  return { key: `${String(i)}.${String(run)}`, path: f.path, head: fileHead(f, short), short, add: f.add, del: f.del, lines: seen ? [] : lines, note: f.note ?? null, file, viewed: seen };
}

function factsOf(files: readonly DiffFile[], w: Words): { facts: string; count: string; sum: string } {
  const add = files.reduce((n, f) => n + f.add, 0);
  const del = files.reduce((n, f) => n + f.del, 0);
  const count = w.files(files.length);
  const sum = counts({ add, del });
  return { facts: [count, sum].filter(Boolean).join(' · '), count, sum };
}

/** By kind (the simple mode): Code, Tests, Words and docs, Generated; a folder line over each folder. */
export function kindColumn(d: Diff, folded: readonly string[], viewed: ReadonlySet<string>, w: Words): Section[] {
  const parts = partsOf(d.files);
  const short = framed(parts);
  return parts.map((p): Section => {
    const files = p.groups.flatMap((g) => g.files.flatMap((i) => d.files[i] ?? []));
    return {
      key: p.kind,
      kind: p.kind,
      label: w.kind[p.kind],
      title: null,
      summary: null,
      weight: null,
      ...factsOf(files, w),
      names: files.map((f) => baseName(f.path)),
      folded: short && folded.includes(p.kind),
      blocks: p.groups.map((g) => ({
        folder: short ? (g.folder === '' ? w.root : g.folder) : null,
        cards: g.files.flatMap((i) => {
          const f = d.files[i];
          return f === undefined ? [] : [card(f, i, 0, linesOf(f), short, viewed)];
        }),
      })),
    };
  });
}

/** The guide is drawable on this diff: every hunk has its id (an older hub sends none). */
export function guidable(d: Diff): boolean {
  return d.files.every((f) => f.hunks.every((h) => h.id != null));
}

/** Guide: its parts in order, 'The rest', then Generated (folded unless he opened it). */
export function guideColumn(d: Diff, g: Guide, folded: readonly string[], viewed: ReadonlySet<string>, w: Words): Section[] {
  // each hunk's file and place
  const at = new Map<string, { fi: number; hi: number }>();
  d.files.forEach((f, fi) => {
    f.hunks.forEach((h, hi) => {
      if (h.id != null) at.set(h.id, { fi, hi });
    });
  });
  const runs = (ids: readonly string[]): Card[] => {
    const out: Card[] = [];
    let cur: { fi: number; his: number[] } | null = null;
    const flush = () => {
      const f = cur === null ? undefined : d.files[cur.fi];
      if (cur === null || f === undefined) return;
      out.push(card(f, cur.fi, out.length, linesOf({ ...f, hunks: cur.his.flatMap((hi) => f.hunks[hi] ?? []) }), false, viewed));
    };
    for (const id of ids) {
      const p = at.get(id);
      if (p === undefined) continue;
      if (cur !== null && cur.fi === p.fi) cur.his.push(p.hi);
      else {
        flush();
        cur = { fi: p.fi, his: [p.hi] };
      }
    }
    flush();
    return out;
  };
  const filesOf = (cards: readonly Card[]) => [...new Set(cards.map((c) => c.path))].flatMap((p) => d.files.find((f) => f.path === p) ?? []);
  const section = (key: string, kind: SectionKind, label: string, cards: Card[], extra: Partial<Section> = {}): Section => {
    const files = filesOf(cards);
    return { key, kind, label, title: null, summary: null, weight: null, ...factsOf(files, w), names: files.map((f) => baseName(f.path)), folded: folded.includes(key), blocks: [{ folder: null, cards }], ...extra };
  };
  const out = g.parts.map((p, i) => section(`part-${String(i + 1)}`, 'part', p.area, runs(p.hunks), { title: p.title, summary: p.summary, weight: p.weight, folded: folded.includes(`part-${String(i + 1)}`) }));
  // the rest: the model's choice, open; a generated file's hunks there go to Generated (a path rule)
  const gen = (id: string) => {
    const p = at.get(id);
    const f = p === undefined ? undefined : d.files[p.fi];
    return f !== undefined && kindOf(f) === 'generated';
  };
  const rest = runs(g.rest.filter((id) => !gen(id)));
  if (rest.length > 0) out.push(section('rest', 'rest', w.rest, rest));
  const generated = runs(g.rest.filter(gen));
  if (generated.length > 0) out.push(section('generated', 'generated', w.kind.generated, generated, { folded: folded.includes('generated') }));
  return out;
}

/** The cards drawn, in order: a folded section's left out (their rows, starts and test ids follow). */
export function shownCards(sections: readonly Section[]): Card[] {
  return sections.filter((s) => !s.folded).flatMap((s) => s.blocks.flatMap((b) => b.cards));
}

/** '01 / 03': a section's place among the numbered ones (a guide's parts, or the kinds); null for the rest and Generated in a guide. */
export function numberOf(sections: readonly Section[], key: string): string | null {
  const numbered = sections.filter((s) => s.kind !== 'rest' && !(s.kind === 'generated' && sections.some((x) => x.kind === 'part')));
  const i = numbered.findIndex((s) => s.key === key);
  const pad = (n: number) => String(n).padStart(2, '0');
  return i < 0 ? null : `${pad(i + 1)} / ${pad(numbered.length)}`;
}

/** The files viewed of the diff's files: '2 of 9 viewed'. */
export function viewedCount(files: readonly DiffFile[], viewed: ReadonlySet<string>): number {
  return files.filter((f) => viewed.has(fileKey(f))).length;
}

/** About how many tokens, for the button: 'about 5k'. */
export function aboutTokens(n: number): string {
  return n < 1000 ? String(n) : `${String(Math.round(n / 1000))}k`;
}
