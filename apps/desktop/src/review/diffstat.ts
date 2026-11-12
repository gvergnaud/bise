// A change's diffstat, as GitHub draws it at a PR's top (his m_62199: a measure of its size): the
// lines added and removed over its whole diff, and five squares, green for the added share, red for
// the removed one, each rounded down, the rest neutral (+438 −151: 3 green, 1 red, 1 neutral).
// The hub counts a file's + and − on all its lines, past the size cap too (switchboard diff.rs
// parse/cap), so the sum is the PR's whole. Pure: PrTab draws it. Owner: pr-artifacts.

/** How many squares the bar has (GitHub's five). */
export const SQUARES = 5;

export type Square = 'add' | 'del' | 'none';

export interface DiffStat {
  readonly add: number;
  readonly del: number;
  /** how many files it changes (its tip's '· 12 files', designer m_62857) */
  readonly files: number;
  readonly squares: readonly Square[];
}

/** The totals of a diff's files and their squares; no line changed: five neutral squares. */
export function diffStat(files: readonly { readonly add: number; readonly del: number }[]): DiffStat {
  const add = files.reduce((n, f) => n + f.add, 0);
  const del = files.reduce((n, f) => n + f.del, 0);
  const total = add + del;
  const green = total === 0 ? 0 : Math.floor((SQUARES * add) / total);
  const red = total === 0 ? 0 : Math.floor((SQUARES * del) / total);
  const squares = Array.from({ length: SQUARES }, (_, i): Square => (i < green ? 'add' : i < green + red ? 'del' : 'none'));
  return { add, del, files: files.length, squares };
}
