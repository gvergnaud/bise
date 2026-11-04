import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { BADGE_ENV, badgePath, loadBadge, TrayBadge } from './traybadge';
import { TRAY_DOT } from './traylook';

describe('the tray badge addon', () => {
  it('its path: darwin only, the env first, else the packed app only', () => {
    expect(badgePath({ platform: 'linux', env: {}, packaged: true, resources: '/r' })).toBeNull();
    expect(badgePath({ platform: 'darwin', env: { [BADGE_ENV]: '/x/b.node' }, packaged: false, resources: '/r' })).toBe('/x/b.node');
    expect(badgePath({ platform: 'darwin', env: {}, packaged: true, resources: '/r' })).toBe('/r/tray-badge.node');
    expect(badgePath({ platform: 'darwin', env: {}, packaged: false, resources: '/r' })).toBeNull();
  });

  it('missing or broken, it fails soft: no addon, one log line, set says false (the title dot)', () => {
    const logs: string[] = [];
    const log = (w: string): void => { logs.push(w); };
    const never = (): unknown => { throw new Error('never loaded'); };
    expect(loadBadge(null, never, log)).toBeNull();
    expect(logs).toEqual([]);
    expect(loadBadge('/nowhere/tray-badge.node', never, log)).toBeNull();
    expect(logs[0]).toContain('no addon');
    const d = mkdtempSync(join(tmpdir(), 'badge-'));
    const f = join(d, 'tray-badge.node');
    writeFileSync(f, 'not a Mach-O');
    expect(loadBadge(f, () => { throw new Error('bad'); }, log)).toBeNull();
    expect(logs[1]).toContain('did not load (bad)');
    expect(loadBadge(f, () => ({}), log)).toBeNull();
    const b = new TrayBadge(null, log);
    expect(b.loaded).toBe(false);
    expect(b.set(TRAY_DOT)).toBe(false);
  });

  it('loaded, it passes the dot as given and says whether it found the item, logging once', () => {
    const logs: string[] = [];
    const seen: unknown[] = [];
    let found = true;
    const b = new TrayBadge({ setBadge: (dot) => { seen.push(dot); return found; } }, (w) => { logs.push(w); });
    expect(b.set(TRAY_DOT)).toBe(true);
    expect(b.set(null)).toBe(true);
    expect(seen).toEqual([TRAY_DOT, null]);
    found = false;
    expect(b.set(TRAY_DOT)).toBe(false);
    expect(b.set(TRAY_DOT)).toBe(false);
    expect(logs).toHaveLength(1);
    const boom = new TrayBadge({ setBadge: () => { throw new Error('x'); } }, (w) => { logs.push(w); });
    expect(boom.set(TRAY_DOT)).toBe(false);
  });
});
