// The window's view error codes are the hub's (rust/proto view_rpc.rs ViewError, its contract
// test's table): a code the hub dropped (-32036 server-restarted) or added (-32037 no-gesture)
// shows here.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { VIEW_ERR } from './port';

const RPC = join(__dirname, '..', '..', '..', '..', 'rust', 'proto', 'src', 'view_rpc.rs');

describe('VIEW_ERR', () => {
  it("has exactly the hub's ViewError codes", () => {
    const src = readFileSync(RPC, 'utf8');
    const table = src.slice(src.indexOf('fn codes_and_reasons_are_the_contracts'));
    const hub = [...table.matchAll(/\((-320\d\d), "([a-z-]+)"\)/gu)].map((m) => Number(m[1]));
    expect(hub.length).toBeGreaterThan(5);
    expect(Object.values(VIEW_ERR).sort()).toEqual([...hub].sort());
  });
});
