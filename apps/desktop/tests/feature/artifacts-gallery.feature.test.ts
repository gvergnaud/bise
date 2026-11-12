// The artifacts gallery, in his words (brief part 3, page artifacts-gallery v7): the hubs'
// artifacts events in, the cards he reads out (their names, their one line, a stack's count, the
// new chip). Nothing else is read.
import { describe, expect, it } from 'vitest';
import { artifact, artifacts, projects, row } from './inputs';
import { feature } from './run';

const HARNESS = 'harness-3abb2bd8';
const DOCS = 'ai-docs-1a2b3c4d';
const NOW = new Date(2026, 9, 22, 19, 0).getTime();
const at = (daysAgo: number, h: number, m: number) => new Date(2026, 9, 22 - daysAgo, h, m).getTime();
const TURN = at(0, 14, 0);
// the hub's openers (bise's own types until their step lands: docs/plugin-apps.md A3)
const PREVIEW = { plugin: 'media', tool: 'open_media', app: 'Preview', type: '*.png', type_title: 'PNG image', builtin: true };
const WRITER = { plugin: 'docs', tool: 'open_doc', app: 'Writer', type: '*.docx', type_title: 'Word document', builtin: true };
const both = projects(row({ project: HARNESS, name: 'harness' }), row({ project: DOCS, name: 'ai-docs' }));
const shots = [1, 2, 3].map((n) =>
  artifact({ id: `shot-${String(n)}`, kind: 'shot', agent: 'amb-home', title: `agent view step 2 · ${['dark', 'light', 'dark'][n - 1] ?? ''} · ${String(n)}`, turn: TURN, at_ms: at(0, 14, 2) - n, opener: PREVIEW }),
);

describe('the artifacts gallery, in his words', () => {
  it("one turn's shots are one card with their count; across projects, newest first; the line leads with the app, then who made it", () => {
    const w = feature(NOW).given(
      both,
      artifacts(HARNESS, ...shots, artifact({ id: 'target', agent: 'agent-target', at_ms: at(0, 9, 12), opener: PREVIEW })),
      artifacts(DOCS, artifact({ id: 'offre', agent: 'epargne-offre', new: true, at_ms: at(0, 16, 0), opener: WRITER })),
    );
    const folderOf = (it: { project: string }) => (it.project === DOCS ? 'épargne' : 'desktop app / agent view');
    expect(w.view.gallery({ kind: 'recent', folderOf })).toEqual([
      { key: `${DOCS}/offre`, name: 'offre', line: 'Writer · epargne-offre · 16:00', new: true },
      { key: `stack:${HARNESS}/shot-1`, name: 'agent view step 2', line: 'Preview · amb-home · 14:01', count: 3, new: false },
      { key: `${HARNESS}/target`, name: 'target', line: 'Preview · agent-target · 09:12', new: false },
    ]);
  });

  it("in a task's folder too the line says the app and who made it (designer m_61626); filtered to one project only its own", () => {
    const w = feature(NOW).given(
      both,
      artifacts(HARNESS, ...shots, artifact({ id: 'target', agent: 'agent-target', at_ms: at(0, 9, 12), opener: PREVIEW })),
      artifacts(DOCS, artifact({ id: 'offre', agent: 'epargne-offre', at_ms: at(0, 16, 0), opener: WRITER })),
    );
    expect(w.view.gallery({ kind: 'folder', owner: 'amb-home' }, HARNESS).map((c) => [c.name, c.line, c.count])).toEqual([
      ['agent view step 2', 'Preview · amb-home · 14:01', 3],
      ['target', 'Preview · agent-target · 09:12', undefined],
    ]);
  });

  it('a new version of one shot in a later turn leaves the stack: the rest stay stacked, it gets its own card', () => {
    const w = feature(NOW).given(both, artifacts(HARNESS, ...shots));
    expect(w.view.gallery({ kind: 'folder', owner: 'amb-home' }).map((c) => c.count ?? 1)).toEqual([3]);
    const later = { ...shots[0], id: 'shot-1', turn: TURN + 60_000, version: 2, at_ms: at(0, 15, 0) } as typeof shots[number];
    w.given(artifacts(HARNESS, later, ...shots.slice(1)));
    expect(w.view.gallery({ kind: 'folder', owner: 'amb-home' }).map((c) => [c.name, c.count ?? 1])).toEqual([
      ['agent view step 2 · dark · 1', 1],
      ['agent view step 2', 2],
    ]);
  });

  it("his own adds never stack, and their line says 'you'", () => {
    const mine = [1, 2].map((n) => artifact({ id: `cart-${String(n)}`, by: 'you', agent: 'main', title: `cart · ${String(n)}`, at_ms: at(5, 10, n), opener: WRITER }));
    const w = feature(NOW).given(both, artifacts(DOCS, ...mine));
    expect(w.view.gallery({ kind: 'unfiled' }).map((c) => [c.name, c.line])).toEqual([
      ['cart · 2', 'Writer · you · oct 17'],
      ['cart · 1', 'Writer · you · oct 17'],
    ]);
  });
});
