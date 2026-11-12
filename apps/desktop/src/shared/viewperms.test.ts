import { describe, expect, it } from 'vitest';
import FX from '../../../../tests/fixtures/mcp-apps/declared.json';
import { VIEW_PERMS } from '../proto/view_rules.gen';
import { allowOf, isViewPerm, permRow, permsOf } from './viewperms';

describe('view permissions', () => {
  it("reads a resource's _meta as the hub does (declared.json, bise_proto::view_rules::declared)", () => {
    for (const row of FX.rows) {
      const meta = (row.meta ?? {}) as { ui?: { permissions?: unknown }; 'dev.bise/ui'?: { permissions?: unknown } };
      expect(permsOf(meta.ui?.permissions, meta['dev.bise/ui']?.permissions), JSON.stringify(row.meta)).toEqual(row.declared);
    }
  });

  it("makes the frame's allow from the one table", () => {
    expect(allowOf(['clipboardWrite', 'camera'])).toBe('camera; clipboard-write');
    expect(allowOf([])).toBe('');
    expect(VIEW_PERMS.map((r) => r.perm)).toEqual(['camera', 'microphone', 'midi', 'clipboardWrite', 'write']);
    // write is the hub's grant on each save, never a frame feature
    expect(allowOf(['write'])).toBe('');
    expect(allowOf(['write', 'camera'])).toBe('camera');
    expect(isViewPerm('midi') && !isViewPerm('geolocation')).toBe(true);
    expect(permRow('geolocation')).toBeNull();
  });
});
