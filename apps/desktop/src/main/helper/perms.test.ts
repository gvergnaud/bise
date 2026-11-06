import { describe, expect, it } from 'vitest';
import type { AppCmd, HelperEv } from '../../proto';
import { PermsRelay, permsCmd } from './perms';

const PERMS: HelperEv = { ev: 'perms', accessibility: 'allowed', mic: 'denied', screen: 'ask' };

describe('the helper perms relay', () => {
  it('turns a perms event into helper_perms and ignores the others', () => {
    expect(permsCmd(PERMS)).toEqual({ cmd: 'helper_perms', mic: 'denied', accessibility: 'allowed', screen: 'ask' });
    expect(permsCmd({ ev: 'quiet', on: true, why: 'call' })).toBeNull();
  });

  it('sends each new perms, and the last one again when a core comes up', () => {
    const sent: AppCmd[] = [];
    const r = new PermsRelay((c) => {
      sent.push(c);
      return true;
    });
    r.coreUp();
    expect(sent).toEqual([]);
    r.helperEv(PERMS);
    r.coreUp();
    expect(sent).toHaveLength(2);
    expect(sent[1]).toEqual(sent[0]);
  });
});
