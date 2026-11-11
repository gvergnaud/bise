// The gap row (inbox-open): when the thread holds a window around a far entry (his answer weeks
// back, store/window.ts), the row between that window and the latest messages. While it shows, the
// window pages newer (store.newerPage, one page at a time) until it meets the latest, then the row
// goes: the 'older messages…' line's twin, scrolled into, it loads; the thread's own '↓ back to the
// bottom' takes him to the latest. Its key never equals an entry's (thread-perf's anchor skips it).
// Owner: inbox-open.
import { useEffect } from 'react';
import { Marker } from '@/ds';
import { thread } from '@/proto/words.gen';
import { threadKey, useStore, useThreadOrder } from '@/store';
import './gap.css';

const GAP = 'gap:';

/** The gap row's key: 'gap:<the tail's first pos>'. */
export const gapKey = (hi: number): string => `${GAP}${String(hi)}`;
export const isGapKey = (key: string): boolean => key.startsWith(GAP);

/** The pos a list key starts at: an entry's, a folded run's first ('r:<start>'); none for the rest. */
function startOf(key: string): number | undefined {
  const m = /^[er]:(\d+)$/u.exec(key);
  return m?.[1] === undefined ? undefined : Number(m[1]);
}

/** The keys with the gap row before the first one at or after `hi` (the tail's first). Pure. */
export function withGap(keys: readonly string[], hi: number | undefined): readonly string[] {
  if (hi === undefined) return keys;
  const i = keys.findIndex((k) => (startOf(k) ?? -1) >= hi);
  if (i <= 0) return keys;
  return [...keys.slice(0, i), gapKey(hi), ...keys.slice(i)];
}

export function GapRow({ project, agent }: { project: string; agent: string }) {
  const store = useStore();
  const view = useThreadOrder(threadKey(project, agent));
  const from = view.gap?.from;
  // shown: the next newer page; its answer moves `from`, and the next one goes while it still shows
  useEffect(() => {
    if (from !== undefined) store.newerPage(project, agent);
  }, [store, project, agent, from]);
  return (
    <div className="th-gap" data-testid="thread.gap">
      <Marker>{thread.newer_messages}</Marker>
    </div>
  );
}

/**
 * The 'older messages…' line over the oldest row while older ones exist (the list's head): the gap
 * row's look, on the column as every row (thread.css's column rule, .th-older), never at the list's
 * left edge (his screenshot, thread-older).
 */
export function OlderLine() {
  return (
    <div className="th-gap th-older" data-testid="thread.older">
      <Marker>{thread.older_messages}</Marker>
    </div>
  );
}
