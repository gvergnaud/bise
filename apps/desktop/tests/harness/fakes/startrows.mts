// The fake core's start rows (core-stub.mts): the cards its `projects` rows list at the start. Pure,
// so a vitest reads the same rows the stub sends (tests/checks/app/startrows.test.ts).
//
// The real core lists a held hub's cards in its `projects` rows in the same beat it forwards that
// hub's `cards` (core/hubs.rs project_ev: emit_projects after 'cards'; his cards and bise's own by
// id, inbox_cards); the window reads the rows only (inbox-sync). So does the stub: a start row that
// says nothing of its cards gets its project's cards from the start world (a row that lists them
// keeps what the test wrote).
//
// A project's cards in the start world: its FIRST `cards` line. The default world is the fixtures'
// catalog (rust/proto/fixtures), one line per shape: a shape line appended later for one card (#924,
// a feature try, d67b4e432) is not the hub's whole list. Taken as acme's cards, it made acme's first
// row list #924 alone, then the next row #9 and #10: the window's draft prune (inbox/drafts.ts) saw
// acme live without #9 and dropped his kept answer to it (app/keep-all's ⌘I red on 11.10).

export type Card = { id: number };
export type Row = { project?: unknown; cards?: Card[]; waits?: number; [k: string]: unknown };
export type WorldEv = { ev?: string; project?: unknown; cards?: Card[]; others?: Card[]; projects?: Row[]; [k: string]: unknown };

/** A `cards` line's cards, his and bise's own, one per id (his first: a hub's ids are its own; the fixtures' hub_ev reuses #11 in `others`), by id. */
export const cardsOf = (ev: WorldEv): Card[] => {
  const all = [...(ev.cards ?? []), ...(ev.others ?? [])];
  return all.filter((c, i) => all.findIndex((x) => x.id === c.id) === i).sort((a, b) => a.id - b.id);
};

/** The rows with `byProject`'s cards; `onlyUnsaid`: only the rows that list none. */
export const withCards = (list: Row[], byProject: ReadonlyMap<unknown, Card[]>, onlyUnsaid = false): Row[] =>
  list.map((r) => {
    const cards = byProject.get(r.project);
    return cards && !(onlyUnsaid && r.cards !== undefined) ? { ...r, cards, waits: cards.length } : r;
  });

/** Each project's cards in the start world: its first `cards` line's. */
export function heldCards(evs: readonly WorldEv[]): Map<unknown, Card[]> {
  const held = new Map<unknown, Card[]>();
  for (const ev of evs) {
    if (ev.ev === 'cards' && typeof ev.project === 'string' && Array.isArray(ev.cards) && !held.has(ev.project)) held.set(ev.project, cardsOf(ev));
  }
  return held;
}

/** The start lines as the stub sends them: a `projects` row that lists no cards gets its project's held ones. */
export function startRows(lines: readonly string[], evs: readonly WorldEv[]): string[] {
  const held = heldCards(evs);
  return lines.map((l) => {
    const ev = JSON.parse(l) as WorldEv;
    return ev.ev === 'projects' && Array.isArray(ev.projects) && held.size > 0 ? JSON.stringify({ ...ev, projects: withCards(ev.projects, held, true) }) : l;
  });
}
