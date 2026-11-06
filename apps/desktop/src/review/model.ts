// The review's words (bar A.6: features and review, the merged/diff work
// of the old window's S7, winreview.js). Owner: amb-win. Pure.
import type { Agent, Card, DiffFile, Merged, Place, Pr, Worktree } from '@/proto';
import type { DiffLine as DsLine } from '@/ds';
import type { Diff } from '@/store';
import { livePr, placeOf } from '@/app/barplaces';

/** The open or draft PR of `agent`'s worktree (the hub's PR poller puts it on its place), with its link; else null. Pure. */
export function openPrOf(agent: string | undefined, agents: readonly Agent[], places: readonly Place[]): Pr | null {
  const a = agent === undefined ? undefined : agents.find((x) => x.name === agent);
  const p = a === undefined ? undefined : placeOf(a, places);
  const pr = p === undefined ? null : livePr(p);
  return pr !== null && pr.url !== '' ? pr : null;
}

/** A file's lines for the ds Diff: each hunk's header as a context line, then its lines. */
export function linesOf(f: DiffFile): DsLine[] {
  const out: DsLine[] = [];
  for (const h of f.hunks) {
    out.push({ text: h.header, sign: '', head: true });
    for (const l of h.lines) out.push({ old: l.old ?? null, new: l.new ?? null, sign: l.kind === 'add' ? '+' : l.kind === 'del' ? '-' : '', text: l.text });
  }
  return out;
}

/** A file's head: its path, a rename's old path, and why its lines aren't all there. */
export function fileHead(f: DiffFile): string {
  const parts = [f.from != null && f.from !== '' ? `${f.from} → ${f.path}` : f.path];
  if (f.binary === true) parts.push(f.size == null ? 'a binary file' : `a binary file, ${bytes(f.size)}`);
  else if (f.generated === true) parts.push('generated, lines left out');
  else if (f.truncated === true) parts.push('cut: too long to show whole');
  if (f.status === 'deleted') parts.push('deleted');
  if (f.status === 'added') parts.push('new');
  return parts.join(' · ');
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/** "3 files · +12 −4" */
export function totals(d: Diff): string {
  const add = d.files.reduce((n, f) => n + f.add, 0);
  const del = d.files.reduce((n, f) => n + f.del, 0);
  const n = d.files.length;
  return `${n} ${n === 1 ? 'file' : 'files'} · +${add} −${del}`;
}

/** The sha a merged change is shown by: the diff's merge, else the commit asked (a commit already on main). */
export function mergedSha(d: Diff | null, commitRoute: string | undefined): string | undefined {
  return d?.merged ?? commitRoute ?? undefined;
}

/** The review's head line: 'merged · e0f3df5 · 14:05' once on main, else '<head> → <base>'. */
export function headLine(d: Diff | null, opt: { commit?: string; merged: readonly Merged[]; clock: (ms: number) => string }): string {
  const sha = mergedSha(d, opt.commit);
  if (sha !== undefined) {
    const m = opt.merged.find((x) => x.sha === sha || x.sha.startsWith(sha) || sha.startsWith(x.sha));
    return ['merged', sha.slice(0, 7), m ? opt.clock(m.at_ms) : ''].filter(Boolean).join(' · ');
  }
  if (d === null) return '';
  return d.head != null && d.head !== '' ? `${d.head} → ${d.base}` : `the shared folder → ${d.base}`;
}

/** The open merge card of this agent, if any: the merge happens only by answering it (his click). */
export function mergeCardOf(cards: readonly Card[], agent: string): Card | undefined {
  return cards.find((c) => c.agent === agent && (c.kind === 'merge' || c.kind === 'feature_try' || c.kind === 'land'));
}

/** The changes tab's branch rows: the agents' worktrees first (ahead first), then his others. */
export function branchesOf(items: readonly Worktree[]): Worktree[] {
  return [...items].sort((a, b) => Number(b.agent != null) - Number(a.agent != null) || b.ahead - a.ahead || a.branch.localeCompare(b.branch));
}

/** A branch row's end: "2 ahead · 1 behind · changed". */
export function branchEnd(w: Worktree): string {
  return [w.ahead > 0 ? `${w.ahead} ahead` : '', w.behind > 0 ? `${w.behind} behind` : '', w.dirty ? 'changed' : ''].filter(Boolean).join(' · ') || 'even';
}
