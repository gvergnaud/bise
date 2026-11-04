// The sidebar's rows (round 2, his r2-img11): one row per project, no agent under it, its facts
// (round 3, H1: projectfacts.test.ts); ⌘ held, its digit 1-9.
import { describe, expect, it } from 'vitest';
import type { Agent, ProjectRow } from '@/proto';
import { projects } from '@/proto/words_projects.gen';
import { projectRowOf, projectsOf, workingOf } from './siderows';

const agent = (name: string, o: Partial<Agent> = {}): Agent =>
  ({ name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: '', since_ms: 0, waits: 0, ...o }) as Agent;
const project = (id: string, o: Partial<ProjectRow> = {}): ProjectRow => ({ project: id, name: id, order: 0, home: false, ...o }) as ProjectRow;

const ACME = project('acme');

describe('projectRowOf', () => {
  it('counts its live agents at work, an archived one out; its facts say them with the pill', () => {
    const agents = [agent('main', { status: 'working' }), agent('perf', { status: 'working' }), agent('old', { status: 'working', archived: true }), agent('docs')];
    expect(workingOf(agents)).toBe(2);
    const r = projectRowOf({ p: ACME, agents, waits: 1, index: 0, on: true, cmd: false, now: 0 });
    expect(r).toMatchObject({ p: ACME, on: true, facts: { pill: 1, quiet: false, sentence: { text: projects.working({ n: 2 }), err: false } } });
  });

  it('⌘ held: its digit, index + 1, the first 9 only (⌘1-9 in keys.ts)', () => {
    const row = (index: number, cmd: boolean) => projectRowOf({ p: ACME, agents: [], waits: 0, index, on: false, cmd, now: 0 });
    expect(row(0, true)).toMatchObject({ digit: 1 });
    expect(row(8, true)).toMatchObject({ digit: 9 });
    expect(row(9, true)).not.toHaveProperty('digit');
    expect(row(0, false)).not.toHaveProperty('digit');
  });
});

describe('projectsOf', () => {
  it("his projects in his order, bise's own out", () => {
    const rows = [project('b', { order: 2 }), project('home', { home: true }), project('a', { order: 1 })];
    expect(projectsOf(rows).map((p) => p.project)).toEqual(['a', 'b']);
  });
});
