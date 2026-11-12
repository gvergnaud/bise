import { describe, expect, it } from 'vitest';
import type { Artifact } from '@/proto';
import type { Item } from '../model';
import { cacheKeyOf, extOf, fileOf, hostOf, isLocal, planOf, sigOf, thumbExt } from './plan';
import { textThumb } from './text';
import { bytesWords, secsWords } from './words';

const art = (id: string, kind: string, extra: Partial<Artifact> = {}): Artifact => ({ id, kind, title: id, agent: 'perf', version: 1, at_ms: 1_000, url: '', new: false, ...extra });
const item = (a: Artifact): Item => ({ project: 'shop-1', projectName: 'storefront', a });

describe('thumbs plan', () => {
  it('draws a PR, a release and a link as tiles of words, with no fetch', () => {
    expect(planOf(item(art('p', 'pr', { url: 'https://github.com/gvergnaud/bise/pull/482', pr: { repo: 'gvergnaud/bise', number: 482 } })))).toEqual({ how: 'tile', icon: 'pr', words: 'bise #482' });
    expect(planOf(item(art('p', 'pr', { url: 'https://github.com/gvergnaud/ts-pattern/pull/384' })))).toMatchObject({ words: 'ts-pattern #384' });
    expect(planOf(item(art('r', 'release', { url: 'https://github.com/gvergnaud/bise/releases/tag/v2026.10.2-27' })))).toEqual({ how: 'tile', icon: 'release', words: 'v2026.10.2-27' });
    expect(planOf(item(art('l', 'link', { url: 'https://www.drive.google.com/x/y' })))).toEqual({ how: 'tile', icon: 'link', words: 'drive.google.com' });
    // a Google sheet is a link: its site
    expect(planOf(item(art('s', 'sheet', { url: 'https://docs.google.com/spreadsheets/d/1' })))).toMatchObject({ how: 'tile', words: 'docs.google.com' });
  });

  it('captures a bise page or a local site from the page server, never a remote one', () => {
    const page = planOf(item(art('plan', 'page', { url: 'http://127.0.0.1:47438/p/plan', version: 3 })));
    expect(page).toMatchObject({ how: 'page', src: 'http://127.0.0.1:47438/p/plan', project: 'shop-1', id: 'plan', v: 3 });
    expect(planOf(item(art('h', 'page', { url: '/w/x.html', frame_url: 'http://127.0.0.1:4747/a/h/1/x.html', path: '/w/x.html' })))).toMatchObject({ how: 'page', src: 'http://127.0.0.1:4747/a/h/1/x.html' });
    expect(planOf(item(art('v', 'site', { url: 'https://shop.vercel.app' })))).toEqual({ how: 'tile', icon: 'link', words: 'shop.vercel.app' });
  });

  it("makes a file's picture from bise's copy first, then the file, none when it is gone with no copy", () => {
    const a = art('f', 'doc', { path: '/w/plan.pdf', versions: [{ v: 1, at_ms: 1, copy: '/s/a/f/v1/plan.pdf' }] });
    expect(fileOf(a)).toBe('/s/a/f/v1/plan.pdf');
    expect(fileOf(art('f', 'doc', { path: '/w/plan.pdf' }))).toBe('/w/plan.pdf');
    expect(fileOf(art('f', 'doc', { path: '/w/plan.pdf', gone: true }))).toBeNull();
    expect(planOf(item(art('f', 'doc', { path: '/w/plan.pdf', gone: true })))).toEqual({ how: 'none' });
    expect(planOf(item(a))).toMatchObject({ how: 'file', src: '/s/a/f/v1/plan.pdf' });
  });

  it('makes no picture of an expired file: its copies are deleted, its file may be gone (artifacts_retention)', () => {
    const a = art('f', 'doc', { path: '/w/plan.pdf', expired: true, versions: [{ v: 1, at_ms: 1, copy: '/s/a/f/v1/plan.pdf' }] });
    expect(fileOf(a)).toBeNull();
  });

  it('keys the cache per version, and a new target under the same number changes the sig', () => {
    const a = art('f', 'image', { path: '/w/a.png', version: 2, target: '/w/a.png' });
    const p = planOf(item(a));
    if (p.how !== 'file') throw new Error(p.how);
    expect(cacheKeyOf(p)).toBe('shop-1/f-v2');
    expect(p.sig).toMatch(/^[0-9a-f]{8}$/);
    expect(sigOf(a)).toBe(sigOf({ ...a }));
    expect(sigOf({ ...a, target: '/w/b.png' })).not.toBe(sigOf(a));
    expect(sigOf({ ...a, version: 3 })).not.toBe(sigOf(a));
  });

  it("gives the hover's extension, none for what opens in the browser", () => {
    expect(thumbExt(item(art('a', 'image', { path: '/w/shot.PNG' })))).toBe('png');
    expect(thumbExt(item(art('a', 'page', { url: 'http://127.0.0.1:1/p/a' })))).toBe('page');
    expect(thumbExt(item(art('a', 'folder', { path: '/w/out' })))).toBe('dir');
    expect(thumbExt(item(art('a', 'pr', { url: 'https://github.com/a/b/pull/1' })))).toBeNull();
    expect(thumbExt(item(art('a', 'link', { url: 'https://x.dev' })))).toBeNull();
  });

  it('reads paths and hosts', () => {
    expect(extOf('/a/b.tar.GZ')).toBe('gz');
    expect(extOf('/a/.env')).toBe('');
    expect(hostOf('https://www.figma.com/file/1')).toBe('figma.com');
    expect(isLocal('http://localhost:5173/')).toBe(true);
    expect(isLocal('https://bise.dev/m/x')).toBe(false);
  });
});

describe('thumbs words', () => {
  it('says sizes and lengths as the tiles do', () => {
    expect(bytesWords(812)).toBe('812 B');
    expect(bytesWords(64_000)).toBe('64 KB');
    expect(bytesWords(12_400_000)).toBe('12.4 MB');
    expect(bytesWords(88_000_000)).toBe('88 MB');
    expect(secsWords(4.2)).toBe('0:04');
    expect(secsWords(42)).toBe('0:42');
    expect(secsWords(0.2)).toBe('0:01');
    expect(secsWords(3723)).toBe('1:02:03');
  });
});

describe('a text drawn from its words', () => {
  it("takes a md's first heading as title and its first lines without marks", () => {
    const md = '# Delta review\n\n38 **non-merge** commits not on [main](http://x).\n\n```sh\nls\n```\n- a `b`\n---\n## Next\n| a | b |\n|---|---|\n| 1 | 2 |\n';
    expect(textThumb(md, true)).toEqual({ title: 'Delta review', lines: ['38 non-merge commits not on main.', 'a b', 'Next', 'a b', '1 2'] });
  });

  it("reads a front matter's title, skips html comments, caps the lines", () => {
    const md = '---\ntitle: "Offre"\ndate: x\n---\n<!-- hidden\nstill -->\nline 1\nline 2\nline 3';
    expect(textThumb(md, true, 2)).toEqual({ title: 'Offre', lines: ['line 1', 'line 2'] });
  });

  it("takes a txt's first line as title and keeps its marks", () => {
    expect(textThumb('\n  notes  \n# not a heading\n*x*', false)).toEqual({ title: 'notes', lines: ['# not a heading', '*x*'] });
  });
});
