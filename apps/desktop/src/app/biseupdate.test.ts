// update-flow (designer m_45087), update-dot (m_46058): a newer bise is an inbox card; the pill says it runs.
import { describe, expect, it } from 'vitest';
import type { BiseUpdate } from '@/proto';
import { biseUpdateLine, causeParts, updateReport } from './UpdateLine';

const u = (over: Partial<BiseUpdate>): BiseUpdate => ({ id: 'r3', version: 'v2026.10.3', running: 'v2026.10.2', state: 'ready', ...over });

describe('the bise update line', () => {
  it('ready: nothing in the pill, the inbox card asks (update-dot, designer m_46058)', () => {
    expect(biseUpdateLine(u({ notes: ['a'] }), null)).toBeNull();
  });
  it('installing and updating run, with no action; the projects left are named', () => {
    for (const state of ['installing', 'updating'] as const) {
      const l = biseUpdateLine(u({ state, waiting: ['shop', 'web'] }), null);
      expect(l?.tone).toBe('running');
      expect(l?.act).toBeNull();
      expect(l?.words).toBe('updating to v2026.10.3… your agents keep working');
      expect(l?.lines).toEqual(['shop, web', '2 to go']);
    }
  });
  it('failed: ▲ in the error tone, try again, and nothing changed', () => {
    const l = biseUpdateLine(u({ state: 'failed', why: 'the download failed' }), null);
    expect(l?.tone).toBe('err');
    expect(l?.words).toBe("▲ couldn't update to v2026.10.3");
    expect(l?.act).toBe('try again');
    expect(l?.lines).toEqual(['the download failed', "you're still on v2026.10.2. nothing changed."]);
    expect(l?.copy).toBe('Copy report');
  });
  it("a crash's cause: the head as it reads, the new version's raw words apart (designer m_59048)", () => {
    expect(causeParts("the hub stopped (crash): thread 'main' panicked at daemon.rs:12")).toEqual({ head: 'the hub stopped (crash):', tail: "thread 'main' panicked at daemon.rs:12" });
    expect(causeParts('the hub stopped (crash) after: boot: journal read')).toEqual({ head: 'the hub stopped (crash) after:', tail: 'boot: journal read' });
    expect(causeParts('the hub stopped (crash)')).toEqual({ head: 'the hub stopped (crash)', tail: '' });
    expect(causeParts('the download failed')).toEqual({ head: 'the download failed', tail: '' });
  });
  it('Copy report: from → to, the cause, the last log lines, the macOS version, plain text', () => {
    const f = u({ state: 'failed', why: 'the hub stopped (crash): boom', log: ['1 hub start', '2 boot done'] });
    expect(updateReport(f, '14.5.0')).toBe(["bise v2026.10.2 → v2026.10.3 couldn't start", 'the hub stopped (crash): boom', '', '1 hub start', '2 boot done', '', 'macOS 14.5.0'].join('\n'));
    expect(updateReport(u({ state: 'failed' }), '15.0')).toBe(["bise v2026.10.2 → v2026.10.3 couldn't start", '', 'macOS 15.0'].join('\n'));
  });
  it('gone after updating: ✓ updated, then nothing', () => {
    expect(biseUpdateLine(null, 'v2026.10.3')?.words).toBe('✓ updated to v2026.10.3');
    expect(biseUpdateLine(null, null)).toBeNull();
    expect(biseUpdateLine(u({ state: 'unknown' }), null)).toBeNull();
  });
});
