// src/app/panel · the launcher's New › (contract docs/plugin-apps.md A4b, A6b; designer's page
// plugin-apps-mocks block 4): one row per type his on apps make ('Sheet  Sheets', from each
// plugin's `makes`), the most-used app first (how many of his documents here follow it), ties by
// word; his words filter New and Recent together (a row's word, its app, its ending). Pure: the
// Launcher draws it, a pick asks the hub's file/new. Owner: apps-gallery.

/** One New › row: what an on app makes (app_rows::Plugin `makes`, A1). */
export interface MakeOpt {
  readonly plugin: string;
  /** the app's title, 'Sheets' */
  readonly app: string;
  /** the ending it makes, '.xlsx' */
  readonly ending: string;
  /** the row's word, 'Sheet' */
  readonly title: string;
}

/** A row's key: its plugin and ending (one per type an app makes). */
export const makeKey = (m: MakeOpt): string => `${m.plugin}${m.ending}`;

/**
 * New › rows: his words filter them (every word in its title, its app or its ending), the app
 * he uses most first (`uses`: how many of his documents follow each plugin), then by word; one
 * row per plugin and ending. Pure.
 */
export function newRows(makes: readonly MakeOpt[], q: string, uses: ReadonlyMap<string, number>): MakeOpt[] {
  const words = q.toLowerCase().split(/\s+/u).filter((w) => w !== '');
  const seen = new Set<string>();
  return makes
    .filter((m) => {
      const k = makeKey(m);
      if (seen.has(k)) return false;
      seen.add(k);
      const hay = `${m.title} ${m.app} ${m.ending}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    })
    .toSorted((x, y) => (uses.get(y.plugin) ?? 0) - (uses.get(x.plugin) ?? 0) || x.title.localeCompare(y.title) || x.app.localeCompare(y.app));
}

/** How many of these documents follow each plugin's app (the hub's opener on each row). Pure. */
export function usesOf(rows: readonly { readonly opener?: { readonly plugin: string } | null }[]): Map<string, number> {
  const n = new Map<string, number>();
  for (const r of rows) {
    const p = r.opener?.plugin;
    if (p !== undefined) n.set(p, (n.get(p) ?? 0) + 1);
  }
  return n;
}
