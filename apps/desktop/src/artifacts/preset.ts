// The Library's filter text given before it opens (the Task tab's 'All n in Library →' gives
// `task:<id>`): taken once by the screen that opens next, then gone. Owner: tasks.

let pending: string | null = null;

/** The next Library screen opens with this filter text. */
export function presetLibraryQuery(q: string): void {
  pending = q;
}

/** The text given, once (null: none). */
export function takeLibraryQuery(): string | null {
  const q = pending;
  pending = null;
  return q;
}
