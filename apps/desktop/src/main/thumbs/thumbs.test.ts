import { describe, expect, it } from 'vitest';
import { cachedOf, fileFor, fitWidth, readReq, safePart, stemOf, thumbsDirOf } from './cache';
import { toPrune } from './prune';

describe('the thumbnails cache', () => {
  const r = { how: 'file' as const, src: '/w/a.png', project: 'shop-1a2b', id: 'plan', v: 3, sig: '0123abcd' };

  it('keeps one file per version under the caches folder, its sig in the name', () => {
    // the folder is the core's (bise_home's thumbs_cache), never joined here
    expect(thumbsDirOf({ proto: 1, thumbs_dir: '/h/.bise/cache/thumbs' })).toBe('/h/.bise/cache/thumbs');
    expect(thumbsDirOf({ proto: 1 })).toBe('');
    expect(thumbsDirOf({ thumbs_dir: 'rel/x' })).toBe('');
    expect(thumbsDirOf(null)).toBe('');
    expect(fileFor('/c', r)).toBe('/c/shop-1a2b/plan-v3.0123abcd.png');
    expect(stemOf(r)).toBe('plan-v3.');
    // a video's or a sound's length (the helper's) rides the name, so a cached card still says it
    expect(fileFor('/c', r, 61500)).toBe('/c/shop-1a2b/plan-v3.0123abcd.61500ms.png');
  });

  it("finds this version's picture among a folder's names, with the length it keeps", () => {
    expect(cachedOf(['plan-v3.0123abcd.png'], r)).toEqual({ name: 'plan-v3.0123abcd.png' });
    expect(cachedOf(['x.png', 'plan-v3.0123abcd.61500ms.png'], r)).toEqual({ name: 'plan-v3.0123abcd.61500ms.png', ms: 61500 });
    // another sig, another version, the helper's temp file, a stranger: none
    expect(cachedOf(['plan-v3.ffffffff.png', 'plan-v2.0123abcd.png', 'plan-v3.0123abcd.png.tmp', 'plan-v3.0123abcd.xms.png'], r)).toBeNull();
  });

  it("refuses a request whose names could leave the cache, or that isn't one", () => {
    expect(readReq(r)).toEqual(r);
    expect(readReq({ ...r, project: '..' })).toBeNull();
    expect(readReq({ ...r, id: 'a/b' })).toBeNull();
    expect(readReq({ ...r, sig: 'zz' })).toBeNull();
    expect(readReq({ ...r, how: 'exec' })).toBeNull();
    expect(readReq({ ...r, v: 1.5 })).toBeNull();
    expect(readReq('x')).toBeNull();
    expect(safePart('a..b')).toBeNull();
    expect(safePart('.hidden')).toBeNull();
  });

  it('cuts a picture to 480 wide, its ratio kept, never up', () => {
    expect(fitWidth(1120, 700)).toEqual({ width: 480, height: 300 });
    expect(fitWidth(200, 100)).toEqual({ width: 200, height: 100 });
  });
});

describe('the thumbnails cache cap', () => {
  it('deletes the least recently read first, until the rest fit under the cap', () => {
    const f = (path: string, bytes: number, atime: number) => ({ path, bytes, atime });
    const files = [f('/c/a', 40, 3), f('/c/b', 40, 1), f('/c/c', 40, 2), f('/c/d', 40, 4)];
    expect(toPrune(files, 160)).toEqual([]);
    expect(toPrune(files, 100)).toEqual(['/c/b', '/c/c']);
    expect(toPrune(files, 0)).toEqual(['/c/b', '/c/c', '/c/a', '/c/d']);
  });
});
