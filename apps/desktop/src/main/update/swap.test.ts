import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { readSwap, startup, type Startup } from './plan';
import { removeTree, swapIn, type SwapStep } from './swap';

// One law per crash point of the swap (architect m_18352): before the
// record, after the record, after rename 1, after rename 2, after ok. Each
// gives the right startup outcome, and none loses both bundles.

const dirs: string[] = [];
afterEach(() => {
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});

function world() {
  const d = mkdtempSync(join(tmpdir(), 'swap-'));
  dirs.push(d);
  const target = join(d, 'Apps', 'bise.app');
  const staged = join(d, 'updates', 'unpack-bise', 'bise.app');
  const recFile = join(d, 'updates', 'swap.json');
  for (const [app, mark] of [[target, 'v1'], [staged, 'v2']] as const) {
    mkdirSync(join(app, 'Contents'), { recursive: true });
    writeFileSync(join(app, 'Contents', 'mark'), mark);
  }
  const mark = (app: string) => (existsSync(join(app, 'Contents', 'mark')) ? readFileSync(join(app, 'Contents', 'mark'), 'utf8') : null);
  const next = (): Startup => {
    const old = `${target}.old`;
    return startup(old, existsSync(old), existsSync(recFile) ? readSwap(readFileSync(recFile, 'utf8')) : null);
  };
  return { target, staged, recFile, updates: join(d, 'updates'), old: `${target}.old`, mark, next };
}

class Crash extends Error {}

async function crashAfter(w: ReturnType<typeof world>, at: SwapStep): Promise<void> {
  await expect(
    swapIn(w.target, w.staged, w.recFile, { from: 'v1', to: 'v2' }, (s) => {
      if (s === at) throw new Crash(at);
    }),
  ).rejects.toBeInstanceOf(Crash);
}

describe('the swap, write-ahead: each crash point', () => {
  it('before the record: nothing moved, nothing to do', () => {
    const w = world();
    expect(w.mark(w.target)).toBe('v1');
    expect(w.next()).toBe('none');
  });

  it('after the record: the old app is still the target; the record is dropped, nothing shown', async () => {
    const w = world();
    await crashAfter(w, 'record');
    expect(w.mark(w.target)).toBe('v1');
    expect(w.mark(w.staged)).toBe('v2');
    expect(readSwap(readFileSync(w.recFile, 'utf8'))).toMatchObject({ phase: 'staged', old_path: w.old, to_version: 'v2' });
    expect(w.next()).toBe('forget');
  });

  it('after rename 1: no target, but both bundles are on disk and the record names .old', async () => {
    const w = world();
    await crashAfter(w, 'rename_old');
    expect(existsSync(w.target)).toBe(false);
    expect(w.mark(w.old)).toBe('v1');
    expect(w.mark(w.staged)).toBe('v2');
    expect(readSwap(readFileSync(w.recFile, 'utf8'))?.old_path).toBe(w.old);
  });

  it("after rename 2 (record still 'staged'): the new app's first start", async () => {
    const w = world();
    await crashAfter(w, 'rename_new');
    expect(w.mark(w.target)).toBe('v2');
    expect(w.mark(w.old)).toBe('v1');
    expect(readSwap(readFileSync(w.recFile, 'utf8'))?.phase).toBe('staged');
    expect(w.next()).toBe('first');
  });

  it("swapped: the first start; started once and never ok: went_bad, .old kept", async () => {
    const w = world();
    await swapIn(w.target, w.staged, w.recFile, { from: 'v1', to: 'v2' });
    const rec = readSwap(readFileSync(w.recFile, 'utf8'));
    expect(rec).toEqual({ phase: 'swapped', old_path: w.old, from_version: 'v1', to_version: 'v2', attempts: 0, ok: false });
    expect(existsSync(`${w.recFile}.tmp`)).toBe(false);
    expect(w.next()).toBe('first');
    writeFileSync(w.recFile, JSON.stringify({ ...rec, attempts: 1 }));
    expect(w.next()).toBe('went_bad');
    expect(w.mark(w.old)).toBe('v1');
  });

  it('after ok: confirmed (.old and the record go)', async () => {
    const w = world();
    await swapIn(w.target, w.staged, w.recFile, { from: 'v1', to: 'v2' });
    const rec = readSwap(readFileSync(w.recFile, 'utf8'));
    writeFileSync(w.recFile, JSON.stringify({ ...rec, attempts: 1, ok: true }));
    expect(w.next()).toBe('confirmed');
  });

  it('a failed second rename rolls back: the old app in place, the record removed last', async () => {
    const w = world();
    rmSync(w.staged, { recursive: true });
    await expect(swapIn(w.target, w.staged, w.recFile, { from: 'v1', to: 'v2' })).rejects.toThrow();
    expect(w.mark(w.target)).toBe('v1');
    expect(existsSync(w.old)).toBe(false);
    expect(existsSync(w.recFile)).toBe(false);
    expect(w.next()).toBe('none');
  });

  it('removeTree takes a bundle whole, app.asar too; none there is nothing', async () => {
    const w = world();
    mkdirSync(join(w.old, 'Contents', 'Resources'), { recursive: true });
    writeFileSync(join(w.old, 'Contents', 'Resources', 'app.asar'), 'asar');
    await removeTree(w.old, w.updates);
    expect(existsSync(w.old)).toBe(false);
    await removeTree(w.old, w.updates);
  });

  it('removeTree refuses, by throwing, what the updater never makes: the app itself stays', async () => {
    const w = world();
    await expect(removeTree(w.target, w.updates)).rejects.toThrow(/refused/);
    expect(w.mark(w.target)).toBe('v1');
  });

  it(".old left half-deleted by an earlier update (only its app.asar): the next swap still goes (go-back-banner)", async () => {
    const w = world();
    mkdirSync(join(w.old, 'Contents', 'Resources'), { recursive: true });
    writeFileSync(join(w.old, 'Contents', 'Resources', 'app.asar'), 'asar');
    await swapIn(w.target, w.staged, w.recFile, { from: 'v1', to: 'v2' });
    expect(w.mark(w.target)).toBe('v2');
    expect(w.mark(w.old)).toBe('v1');
  });

  it("a launcher's .old next to the bundle, no record: stray, never went_bad", () => {
    const w = world();
    mkdirSync(join(w.old, 'Contents'), { recursive: true });
    expect(w.next()).toBe('stray');
  });
});
