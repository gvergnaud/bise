import { describe, expect, it } from 'vitest';
import { afterStart, bundleOf, cantSwapAt, designated, developerId, leftovers, localFeed, readAfter, readSwap, readVersion, refuseRemove, signerChange, startup, wholeAppParts, zipName } from './plan';

describe('readVersion (the app bundle VERSION)', () => {
  it('reads id and built; a dev build has no built; no id is no build', () => {
    expect(readVersion('id=d25\nbuilt=2026-10-07T10:00:00Z\ntarget=darwin-arm64\n')).toEqual({ id: 'd25', built: '2026-10-07T10:00:00Z' });
    expect(readVersion('id=dev\nrepo=/x\n')).toEqual({ id: 'dev' });
    expect(readVersion('built=2026\n')).toBeNull();
    expect(readVersion('')).toBeNull();
  });
  it("a release's (desktop_feed.py version_text): its commit, the About panel's build", () => {
    const text = ['id=desktop-2026.10.2', 'built=2026-10-24T10:00:00Z', 'commit=5f9a429951ab', 'runtime_id=r1', ''].join('\u000a');
    expect(readVersion(text)).toEqual({ id: 'desktop-2026.10.2', built: '2026-10-24T10:00:00Z', commit: '5f9a429951ab' });
  });
});

describe('bundleOf', () => {
  it('the .app around its executable, else null (an unpacked run)', () => {
    expect(bundleOf('/Applications/bise.app/Contents/MacOS/bise')).toBe('/Applications/bise.app');
    expect(bundleOf('/Users/x/Apps/bise check.app/Contents/MacOS/bise check')).toBe('/Users/x/Apps/bise check.app');
    expect(bundleOf('/x/node_modules/electron/dist/Electron.app/Contents/Resources/x')).toBeNull();
  });
});

describe('designated (codesign -d -r-)', () => {
  it("reads the requirement; none means no update", () => {
    const out = 'Executable=/A/bise.app/Contents/MacOS/bise\ndesignated => identifier "dev.bise.desktop" and certificate leaf = H"ab12"\n';
    expect(designated(out)).toBe('identifier "dev.bise.desktop" and certificate leaf = H"ab12"');
    expect(designated('Executable=/x\n')).toBeNull();
    expect(designated('designated => \n')).toBeNull();
  });
});

describe('startup after a swap (.old kept until started ok, write-ahead record: architect m_18352)', () => {
  const OLD = '/A/bise.app.old';
  const rec = (attempts: number, ok: boolean, phase: 'staged' | 'swapped' = 'swapped', old_path = OLD) => ({
    phase, old_path, from_version: '2026.10.1-25', to_version: '2026.10.2-26', attempts, ok,
  });
  it('no swap, a forgotten one, the first start, a confirmed one', () => {
    expect(startup(OLD, false, null)).toBe('none');
    expect(startup(OLD, false, rec(1, false))).toBe('forget');
    expect(startup(OLD, false, rec(0, false, 'staged'))).toBe('forget');
    expect(startup(OLD, true, rec(0, false))).toBe('first');
    expect(startup(OLD, true, rec(0, false, 'staged'))).toBe('first');
    expect(startup(OLD, true, rec(1, true))).toBe('confirmed');
  });
  it('the record names this .old, started before and never said ok: offer to go back', () => {
    expect(startup(OLD, true, rec(1, false))).toBe('went_bad');
  });
  it('a start after one that never said ok is on probation again: its core answer confirms it, the line goes (go-back-banner)', () => {
    expect(afterStart('went_bad', true)).toEqual({ probation: true, offerBack: true });
    expect(afterStart('first', true)).toEqual({ probation: true, offerBack: false });
    for (const s of ['none', 'forget', 'stray', 'confirmed'] as const) expect(afterStart(s, true)).toEqual({ probation: false, offerBack: false });
  });
  it("'go back' only to a whole app: a half-deleted .old (Gauthier's: only app.asar left) offers nothing", () => {
    expect(afterStart('went_bad', false)).toEqual({ probation: true, offerBack: false });
    expect(wholeAppParts('/A/bise.app.old')).toEqual(['/A/bise.app.old/Contents/Info.plist', '/A/bise.app.old/Contents/MacOS']);
  });
  it("the updater's rm -rf takes only its own: <x>.app.old, <x>.app.bad, unpack-* in its folder (architect m_58148)", () => {
    const U = '/U/bise/updates';
    expect(refuseRemove('/A/bise.app.old', U)).toBeNull();
    expect(refuseRemove('/A/bise.app.bad', U)).toBeNull();
    expect(refuseRemove(`${U}/unpack-bise-1`, U)).toBeNull();
    expect(refuseRemove(`${U}/unpack-bise-1`, `${U}/`)).toBeNull();
    for (const p of ['/A/bise.app', '/', '', 'bise.app.old', '/x.app.old', '/A/../bise.app.old', `${U}/swap.json`, '/elsewhere/unpack-x', `${U}/unpack-x/bise.app`, '/A/bise.app.old/Contents']) {
      expect(refuseRemove(p, U), p).not.toBeNull();
    }
  });
  it("a .old the updater didn't make (no record, or a record naming another path) is stray: never went_bad", () => {
    expect(startup(OLD, true, null)).toBe('stray');
    expect(startup(OLD, true, rec(1, false, 'swapped', '/B/bise.app.old'))).toBe('stray');
    expect(startup(OLD, true, rec(1, true, 'swapped', '/B/bise.app.old'))).toBe('stray');
  });
  it('a record reads back; junk, or one without what it owns, is none', () => {
    expect(readSwap(JSON.stringify(rec(0, false, 'staged')))).toEqual(rec(0, false, 'staged'));
    expect(readSwap(JSON.stringify({ version: '2026.10.2-26', attempts: 1, ok: false }))).toBeNull();
    expect(readSwap(JSON.stringify({ ...rec(0, false), phase: 'done' }))).toBeNull();
    expect(readSwap(JSON.stringify({ ...rec(0, false), old_path: '' }))).toBeNull();
    expect(readSwap('{"version":1}')).toBeNull();
    expect(readSwap('{')).toBeNull();
  });
});

describe('files and feeds', () => {
  it('a start cleans partial downloads and unpacked copies, keeps the rest', () => {
    expect(leftovers(['bise-1.zip.part', 'unpack-1', 'swap.json', 'bise-1.zip'])).toEqual(['bise-1.zip.part', 'unpack-1']);
  });
  it('a test run reads only a local feed', () => {
    expect(localFeed('file:///tmp/t/feed/app.zip')).toBe(true);
    expect(localFeed('http://127.0.0.1:4100/app.zip')).toBe(true);
    expect(localFeed('https://github.com/gvergnaud/bise/releases/latest/download/app.zip')).toBe(false);
    expect(localFeed('http://127.0.0.1.evil.com/app.zip')).toBe(false);
  });
  it('a version is a safe file name', () => {
    expect(zipName('2026.10.2-26')).toBe('bise-2026.10.2-26.zip');
    expect(zipName('../x y')).toBe('bise-.._x_y.zip');
  });
});

describe('the signature rule (architect m_25198 Q2: the running signature decides)', () => {
  const SELF = 'identifier "dev.bise.desktop" and certificate leaf = H"0123456789abcdef0123456789abcdef01234567"';
  const DEVID = 'identifier "dev.bise.desktop" and anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] /* exists */ and certificate leaf[field.1.2.840.113635.100.6.1.13] /* exists */ and certificate leaf[subject.OU] = "ABCDE12345"';
  const ADHOC = 'cdhash H"89abcdef0123456789abcdef0123456789abcdef"';
  it('a Developer ID requirement asks Gatekeeper too; a self-signed or ad-hoc one only codesign', () => {
    expect(developerId(DEVID)).toBe(true);
    expect(developerId(SELF)).toBe(false);
    expect(developerId(ADHOC)).toBe(false);
  });
  it('a self-signed app meeting its first Developer ID build is refused with its own reason; any other pair goes to codesign', () => {
    expect(signerChange(SELF, DEVID)).toBe('new_signer');
    expect(signerChange(ADHOC, DEVID)).toBe('new_signer');
    expect(signerChange(DEVID, DEVID)).toBeNull();
    expect(signerChange(SELF, SELF)).toBeNull();
    expect(signerChange(SELF, null)).toBeNull();
    expect(signerChange(DEVID, SELF)).toBeNull();
  });
});

describe('cantSwapAt (Q3: said, then move.ts offers the move)', () => {
  it('a disk image or a translocated copy cannot replace itself; Applications or a home folder can', () => {
    expect(cantSwapAt('/Volumes/bise/bise.app')).toBe('disk_image');
    expect(cantSwapAt('/h/Volumes/bise/bise.app', '/h/Volumes/')).toBe('disk_image');
    expect(cantSwapAt('/h/Volumes/bise/bise.app')).toBeNull();
    expect(cantSwapAt('/private/var/folders/xy/T/AppTranslocation/1234-ABCD/d/bise.app')).toBe('translocated');
    expect(cantSwapAt('/Applications/bise.app')).toBeNull();
    expect(cantSwapAt('/Users/g/Applications/bise.app')).toBeNull();
  });
});

describe('readAfter (route.json)', () => {
  it("a click's: the route, in front; a quiet moment's: the route and whether its window was shown", () => {
    expect(readAfter('{"route":"settings"}')).toEqual({ route: 'settings', quiet: false, shown: true });
    expect(readAfter('{"route":"inbox","quiet":true,"shown":false}')).toEqual({ route: 'inbox', quiet: true, shown: false });
    expect(readAfter('{"quiet":true}')).toEqual({ quiet: true, shown: true });
    expect(readAfter('nope')).toBeNull();
  });
});
