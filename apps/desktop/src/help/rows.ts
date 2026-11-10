// The key list overlay's rows (tui-parity V3; the TUI's rust/tui/src/help.rs):
// two pages built from settings/keys.ts (amb-home's table, read only here):
// 'help' the keys he needs first, 'shortcuts' every key. Typing filters over
// the section, the keys and the action, case-insensitive. Pure. Owner: amb-web.
import { KEY_SECTIONS, type KeyRow, type KeySection } from '@/settings/keys';

export type HelpPage = 'help' | 'shortcuts';

export const PAGES: readonly HelpPage[] = ['help', 'shortcuts'];

/** The keys the help page shows (the TUI's `.top()` rows), by their drawn keys. */
export const TOP: ReadonlySet<string> = new Set([
  'hold fn',
  'fn space',
  '⌘0',
  '⌘1 … ⌘9',
  '⌥0 / ⌥1 … ⌥9',
  '⌘K',
  '⌘I',
  '⌘/',
  '⏎',
  '⇧⏎',
  'esc',
  '/',
  '@',
  '1 … 9',
]);

export interface HelpSection {
  readonly id: string;
  readonly title: string;
  readonly rows: readonly KeyRow[];
}

/** The sections of a page with their rows that match `filter`; a section with none is left out. */
export function helpRows(page: HelpPage, filter: string, sections: readonly KeySection[] = KEY_SECTIONS): HelpSection[] {
  const f = filter.trim().toLowerCase();
  const hit = (s: KeySection, r: KeyRow) => f === '' || [s.title, r.keys, r.also ?? '', r.action, r.tui ?? ''].some((t) => t.toLowerCase().includes(f));
  return sections
    .map((s) => ({ id: s.id, title: s.title, rows: s.rows.filter((r) => (page === 'shortcuts' || TOP.has(r.keys)) && hit(s, r)) }))
    .filter((s) => s.rows.length > 0);
}

/** The other page (tab and shift+tab both switch, as the TUI). */
export function otherPage(p: HelpPage): HelpPage {
  return p === 'help' ? 'shortcuts' : 'help';
}

/** What esc does: with a filter, clear it; else close. */
export function escOf(filter: string): 'clear' | 'close' {
  return filter === '' ? 'close' : 'clear';
}
