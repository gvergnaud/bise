import { describe, expect, it } from 'vitest';
import { frameAt, pointInWindow, isViewOrigin, isViewProxy, originOf, stampGesture, viewKey, viewNavigation, viewPermission, type ViewFrame } from './viewrules';

const A = 'http://v-0123456789abcdef.localhost:47123';
const B = 'http://v-fedcba9876543210.localhost:47123';
const frame = (origin: string, perms: string[], rect = { x: 0, y: 0, w: 100, h: 100 }, call = '1-2'): ViewFrame => ({ project: 'p', origin, agent: 't1', call, rect, perms });

describe('view origins', () => {
  it('exactly v-<16 hex>.localhost:<port>', () => {
    expect(isViewOrigin(A)).toBe(true);
    for (const bad of [
      'http://v-0123456789ABCDEF.localhost:47123',
      'http://v-0123456789abcde.localhost:47123',
      'http://v-0123456789abcdef.localhost',
      'https://v-0123456789abcdef.localhost:47123',
      'http://x.v-0123456789abcdef.localhost:47123',
      'http://v-0123456789abcdef.localhost.evil.com:47123',
      'http://v-0123456789abcdef.localhost:99999',
      'http://127.0.0.1:47123',
      'null',
      '',
    ]) {
      expect(isViewOrigin(bad), bad).toBe(false);
    }
    expect(isViewOrigin(undefined)).toBe(false);
  });

  it("a blob's origin is its creator's; the proxy is /mcp-app on a view origin", () => {
    expect(originOf(`blob:${A}/5c219979-9279`)).toBe(A);
    expect(originOf(`${A}/`)).toBe(A);
    expect(originOf('about:srcdoc')).toBe(null);
    expect(originOf('data:text/html,x')).toBe(null);
    expect(isViewProxy(`${A}/mcp-app`)).toBe(true);
    expect(isViewProxy(`${A}/api/p/x/notes`)).toBe(false);
    expect(isViewProxy('http://127.0.0.1:47123/mcp-app')).toBe(false);
  });
});

describe('a view asks for a permission', () => {
  const frames = [frame(A, ['microphone', 'midi']), frame(B, ['camera', 'clipboardWrite'])];
  const fromA = { isMainFrame: false, requestingUrl: `blob:${A}/x` };

  it('gets what its frame was granted, nothing else', () => {
    expect(viewPermission('media', { ...fromA, mediaTypes: ['audio'], securityOrigin: `${A}/` }, frames)).toBe(true);
    expect(viewPermission('media', { ...fromA, mediaTypes: ['video'], securityOrigin: `${A}/` }, frames)).toBe(false);
    expect(viewPermission('media', { ...fromA, mediaTypes: ['audio', 'video'], securityOrigin: `${A}/` }, frames)).toBe(false);
    expect(viewPermission('media', { ...fromA, mediaTypes: [] }, frames)).toBe(false);
    expect(viewPermission('midi', fromA, frames)).toBe(true);
    expect(viewPermission('midiSysex', fromA, frames), 'Chromium asks midiSysex for any MIDI').toBe(true);
    expect(viewPermission('clipboard-sanitized-write', fromA, frames)).toBe(false);
    for (const p of ['notifications', 'geolocation', 'display-capture', 'clipboard-read', 'openExternal', 'fullscreen', 'pointerLock', 'unknown']) {
      expect(viewPermission(p, fromA, frames), p).toBe(false);
    }
    // B's grant is B's
    expect(viewPermission('clipboard-sanitized-write', { isMainFrame: false, requestingUrl: `blob:${B}/y` }, frames)).toBe(true);
    expect(viewPermission('midi', { isMainFrame: false, requestingUrl: `blob:${B}/y` }, frames)).toBe(false);
  });

  it('a view not in the live list, or one granted nothing: denied', () => {
    expect(viewPermission('midi', fromA, [])).toBe(false);
    expect(viewPermission('midi', fromA, [frame(A, [])])).toBe(false);
    expect(viewPermission('midi', fromA, [frame(A, ['midi; camera *', 'MIDI'])])).toBe(false);
  });

  it("a check (navigator.permissions, the clipboard's) follows the same rule", () => {
    expect(viewPermission('media', { isMainFrame: false, requestingOrigin: A, mediaType: 'audio' }, frames)).toBe(true);
    expect(viewPermission('media', { isMainFrame: false, requestingOrigin: A, mediaType: 'video' }, frames)).toBe(false);
  });

  it("a subframe whose origin can't be told is denied; the app's own pages keep their rule", () => {
    expect(viewPermission('media', { isMainFrame: false, requestingUrl: 'about:srcdoc', mediaTypes: ['audio'] }, frames)).toBe(false);
    expect(viewPermission('midi', { isMainFrame: false, requestingUrl: 'about:blank' }, frames)).toBe(false);
    expect(viewPermission('media', { isMainFrame: true, requestingUrl: 'file:///Applications/bise.app/x/index.html', mediaTypes: ['audio'] }, frames)).toBe(null);
    expect(viewPermission('clipboard-sanitized-write', { isMainFrame: false, requestingUrl: 'http://127.0.0.1:47123/a/x/1/x.html' }, frames)).toBe(null);
  });
});

describe('the gesture', () => {
  const frames = [frame(A, [], { x: 10, y: 10, w: 100, h: 50 }, '1-1'), frame(B, [], { x: 50, y: 40, w: 100, h: 100 }, '1-2')];

  it("a pointer-down falls in the topmost frame's visible rect", () => {
    expect(frameAt(frames, 20, 20)?.call).toBe('1-1');
    expect(frameAt(frames, 60, 45)?.call, 'overlap: the later one is on top').toBe('1-2');
    expect(frameAt(frames, 5, 5)).toBe(null);
    expect(frameAt(frames, 110, 30), 'the right edge is out').toBe(null);
  });

  it("a click in a view's own frame is placed by its screen point, never its widget's x/y", () => {
    // a window whose content starts at (100, 80) on screen; a card at (600, 400) in it, and
    // another one at the window's corner, where the OOPIF's own x/y (2, 3) would land
    const cards = [frame(A, [], { x: 0, y: 0, w: 50, h: 50 }, 'corner'), frame(B, [], { x: 600, y: 400, w: 300, h: 200 }, 'card')];
    const content = { x: 100, y: 80 };
    const click = { x: 2, y: 3, globalX: 100 + 602, globalY: 80 + 403 };
    const p = pointInWindow(click, content, 1);
    expect(p).toEqual({ x: 602, y: 403 });
    expect(frameAt(cards, p?.x ?? -1, p?.y ?? -1)?.call, "the card's corner is the card").toBe('card');
    // zoomed 125 %: DIP over the zoom is CSS px
    expect(pointInWindow({ globalX: 100 + 750, globalY: 80 + 500 }, content, 1.25)).toEqual({ x: 600, y: 400 });
    expect(pointInWindow({}, content, 1), 'no screen point: no gesture').toBe(null);
    expect(pointInWindow({ globalX: 110, globalY: 90 }, content, 0), 'a zoom of 0 reads as 1').toEqual({ x: 10, y: 10 });
  });

  it("view_message carries main's time, never the window's", () => {
    const downs = new Map([[viewKey('p', 't1', '1-1'), 1000]]);
    const cmd = { cmd: 'view_message', project: 'p', agent: 't1', call: '1-1', content: [{ type: 'text', text: 'hi' }], gesture_ms: 99_999 };
    expect(stampGesture(cmd, downs)).toEqual({ ...cmd, gesture_ms: 1000 });
    expect(stampGesture({ ...cmd, call: '1-2' }, downs), 'no pointer-down in that view: none').not.toHaveProperty('gesture_ms');
    expect(stampGesture({ ...cmd, project: 'q' }, downs), 'another project').not.toHaveProperty('gesture_ms');
    const other = { cmd: 'send', text: 'x', gesture_ms: 5 };
    expect(stampGesture(other, downs)).toEqual(other);
  });
});

describe("a view's navigation", () => {
  const app = { url: 'file:///Applications/bise.app/x/index.html', origin: 'file://' };
  const proxy = { url: `${A}/mcp-app`, origin: A };
  const view = { url: `blob:${A}/v1`, origin: A };

  it('the app sets the proxy; nobody else does', () => {
    expect(viewNavigation(`${A}/mcp-app`, { url: '', origin: 'file://', parent: app }, true)).toBe('stay');
    expect(viewNavigation(`${A}/mcp-app`, { url: '', origin: 'file://', parent: app }, false)).toBe('drop');
    expect(viewNavigation(`${B}/mcp-app`, { ...proxy, parent: app }, true), 'a view moving its proxy elsewhere').toBe('drop');
  });

  it('the proxy makes its blob view, of its own origin only', () => {
    expect(viewNavigation(`blob:${A}/v1`, { url: '', origin: A, parent: proxy }, false)).toBe('stay');
    expect(viewNavigation(`blob:${B}/v1`, { url: '', origin: A, parent: proxy }, false)).toBe('drop');
    expect(viewNavigation('https://evil.example/', { ...view, parent: proxy }, false), 'the view leaving').toBe('drop');
  });

  it('a frame nested in the view goes where its CSP lets it', () => {
    expect(viewNavigation('https://www.youtube.com/embed/x', { url: '', origin: A, parent: view }, false)).toBe('stay');
  });

  it("the proxy's frame leaving is dropped, never sent to his browser", () => {
    expect(viewNavigation('https://evil.example/', { ...proxy, parent: app }, false)).toBe('drop');
    expect(viewNavigation('http://127.0.0.1:47123/p/x', { ...proxy, parent: app }, false)).toBe('drop');
  });

  it("anything else is not a view's: framelink.ts decides", () => {
    expect(viewNavigation('http://127.0.0.1:47123/a/x/1/x.html', { url: '', origin: 'file://', parent: app }, true)).toBe(null);
    expect(viewNavigation('https://github.com', null, false)).toBe(null);
  });
});
