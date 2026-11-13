import { describe, expect, it } from 'vitest';
import type { WorkRow } from '@/proto';
import { whatOf } from './what';
import FX from '../../../../rust/proto/fixtures/work_what.jsonl?raw';

type Case = { now: number; want: string; row: WorkRow };
const cases: Case[] = FX.split('\n')
  .filter((l) => l.trim() !== '')
  .map((l) => JSON.parse(l) as Case);

describe("a task's line is the hub's (rust/proto/fixtures/work_what.jsonl, rows::WorkRow::what)", () => {
  it('each fixture row reads its line; times are each side`s own format', () => {
    expect(cases.length).toBeGreaterThanOrEqual(20);
    for (const c of cases) {
      expect(whatOf(c.row, c.now, () => '<past>', () => '<next>'), JSON.stringify(c.row)).toBe(c.want);
    }
  });
});
