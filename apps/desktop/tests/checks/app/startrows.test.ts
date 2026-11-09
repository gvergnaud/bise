// The fake core's start rows (tests/harness/fakes/startrows.mts): a row never leaves out a card a
// later start row lists for its project. The window prunes the kept answer of a card its project's
// row no longer lists (src/inbox/drafts.ts), so a world whose rows drop #9 then bring it back loses
// his half-typed answer at every launch (app/keep-all's ⌘I red on 11.10: a shape line appended to
// hub_ev.jsonl, d67b4e432, became acme's whole card list).
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { startRows, type WorldEv } from '../../harness/fakes/startrows.mts';

const FIXTURES = path.resolve(__dirname, '..', '..', '..', '..', '..', 'rust', 'proto', 'fixtures');
const jsonl = (file: string): string[] => readFileSync(path.join(FIXTURES, file), 'utf8').split('\n').map((l) => l.trim()).filter(Boolean);

/** Each start `projects` row's card ids, by project, in order. */
function listed(lines: readonly string[]): Map<unknown, number[][]> {
  const out = new Map<unknown, number[][]>();
  for (const l of lines) {
    const ev = JSON.parse(l) as WorldEv;
    if (ev.ev !== 'projects') continue;
    for (const r of ev.projects ?? []) if (r.cards !== undefined) out.set(r.project, [...(out.get(r.project) ?? []), r.cards.map((c) => c.id)]);
  }
  return out;
}

/** The cards a row leaves out that a later row of its project lists again. */
function comeBack(lines: readonly string[]): string[] {
  const bad: string[] = [];
  for (const [project, rows] of listed(lines)) {
    rows.forEach((ids, i) => {
      for (const later of rows.slice(i + 1)) for (const id of later) if (!ids.includes(id)) bad.push(`${String(project)} #${String(id)}`);
    });
  }
  return bad;
}

const world = (lines: readonly string[]): string[] => startRows(lines, lines.map((l) => JSON.parse(l) as WorldEv));

describe("the fake core's start rows", () => {
  it('the fixture world: no card leaves its row and comes back (acme #9, the card keep-all answers)', () => {
    const lines = [...jsonl('core_ev.jsonl'), ...jsonl('hub_ev.jsonl')];
    const rows = world(lines);
    expect(comeBack(rows)).toEqual([]);
    expect(listed(rows).get('acme-1a2b3c4d')?.every((ids) => ids.includes(9))).toBe(true);
  });

  it("a project's later shape line for one card never becomes its whole list", () => {
    const lines = [
      JSON.stringify({ ev: 'projects', projects: [{ project: 'p' }] }),
      JSON.stringify({ ev: 'projects', projects: [{ project: 'p', cards: [{ id: 9 }, { id: 10 }] }] }),
      JSON.stringify({ ev: 'cards', project: 'p', cards: [{ id: 9 }, { id: 10 }] }),
      JSON.stringify({ ev: 'cards', project: 'p', cards: [{ id: 924 }] }),
    ];
    const rows = listed(world(lines)).get('p');
    expect(rows?.[0]).toEqual([9, 10]);
    expect(comeBack(world(lines))).toEqual([]);
  });
});
