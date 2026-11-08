// The review's simple mode (page review-guided v4, Gabriel's pick B): a diff's files in parts by
// kind (Code, Tests, Words and docs, Generated), then by folder, the most changed first; no model,
// computed from the diff the review already has. The guide (piece 2) gives the same frame better
// names. The rules are review-guided's (m_57256). Pure.
import type { DiffFile } from '@/proto';

/** A file's kind, in the order the review shows them. */
export type Kind = 'code' | 'tests' | 'docs' | 'generated';

export const KINDS: readonly Kind[] = ['code', 'tests', 'docs', 'generated'];

/** One folder's files inside a part: `folder` is cut to its first 4 segments ('' at the root). */
export interface Group {
  readonly folder: string;
  /** indexes into the diff's files, the most changed first */
  readonly files: readonly number[];
  readonly add: number;
  readonly del: number;
}

export interface Part {
  readonly kind: Kind;
  /** its folders, the most changed first */
  readonly groups: readonly Group[];
  readonly files: number;
  readonly add: number;
  readonly del: number;
}

const LOCKS = new Set(['pnpm-lock.yaml', 'Cargo.lock', 'package-lock.json', 'flake.lock', 'yarn.lock']);
const TEST_DIRS = new Set(['test', 'tests', 'spec', '__tests__', 'e2e']);
const DOCS = /\.(md|mdx|txt)$/u;
const TEST_FILE = /\.(test|spec)\.[^/]+$|_test\.[^/]+$|_(e2e|tmux)\.py$/u;

/** A file's kind; the first that matches wins: Generated, Tests, Words and docs, Code. */
export function kindOf(f: Pick<DiffFile, 'path' | 'generated'>): Kind {
  const segs = f.path.split('/');
  const name = segs[segs.length - 1] ?? '';
  const dirs = segs.slice(0, -1);
  if (f.generated === true || name.includes('.gen.') || LOCKS.has(name) || name.endsWith('.snap') || dirs.includes('__snapshots__')) return 'generated';
  if (TEST_FILE.test(name) || dirs.some((d) => TEST_DIRS.has(d))) return 'tests';
  if (DOCS.test(name) || dirs.includes('docs')) return 'docs';
  return 'code';
}

/** The folder a file is grouped by: its folder cut to the first 4 segments ('' at the root). */
export function folderOf(path: string): string {
  return path.split('/').slice(0, -1).slice(0, 4).join('/');
}

const size = (x: { add: number; del: number }) => x.add + x.del;

/** The parts of `files` by kind, in KINDS' order, empty kinds left out; inside each, its folders. */
export function partsOf(files: readonly DiffFile[]): Part[] {
  const byPath = (a: number, b: number) => (files[a]?.path ?? '').localeCompare(files[b]?.path ?? '');
  const out: Part[] = [];
  for (const kind of KINDS) {
    const folders = new Map<string, number[]>();
    files.forEach((f, i) => {
      if (kindOf(f) !== kind) return;
      const k = folderOf(f.path);
      folders.set(k, [...(folders.get(k) ?? []), i]);
    });
    if (folders.size === 0) continue;
    const groups = [...folders].map(([folder, ix]): Group => {
      const sorted = [...ix].sort((a, b) => size(files[b] ?? none) - size(files[a] ?? none) || byPath(a, b));
      return { folder, files: sorted, add: sum(sorted.map((i) => files[i]?.add ?? 0)), del: sum(sorted.map((i) => files[i]?.del ?? 0)) };
    });
    groups.sort((a, b) => size(b) - size(a) || a.folder.localeCompare(b.folder));
    out.push({ kind, groups, files: sum(groups.map((g) => g.files.length)), add: sum(groups.map((g) => g.add)), del: sum(groups.map((g) => g.del)) });
  }
  return out;
}

const none = { add: 0, del: 0 };
const sum = (ns: readonly number[]) => ns.reduce((a, b) => a + b, 0);

/** The files in the order the parts show them (indexes into `files`), a folded kind's left out. */
export function shownOrder(parts: readonly Part[], folded: readonly Kind[]): number[] {
  return parts.filter((p) => !folded.includes(p.kind)).flatMap((p) => p.groups.flatMap((g) => g.files));
}

/** A diff with one kind and one folder shows no part heads: a 1-file diff needs no frame. */
export function framed(parts: readonly Part[]): boolean {
  return parts.length > 1 || (parts[0]?.groups.length ?? 0) > 1;
}

/** '+104 −18', '+2', '−3': the counts that aren't zero ('' for none). */
export function counts(x: { add: number; del: number }): string {
  return [x.add > 0 ? `+${x.add}` : '', x.del > 0 ? `−${x.del}` : ''].filter(Boolean).join(' ');
}
