// The web view's guard, pure (webview/guard.ts; architect m_64682 rules 2 and 4): navigations and
// the requests to bise itself, with a pages server's port as main would learn it.
import { describe, expect, it } from 'vitest';
import { bisePortsOf, blockedRequest, goVerdict, isLoopback, webNav } from './guard';

const PAGES = 47438;
const PORTS = new Set([PAGES]);

describe('webNav: what a navigation in a web tab does', () => {
  it.each([
    ['https://github.com/', 'main', 'allow'],
    ['http://localhost:5173/', 'main', 'allow'],
    ['http://127.0.0.1:3000/x', 'main', 'allow'],
    ['about:blank', 'main', 'allow'],
    ['https://github.com/', 'window', 'new-tab'],
    ['about:blank', 'window', 'refuse'],
    ['mailto:a@b.c', 'main', 'external'],
    ['mailto:a@b.c', 'window', 'external'],
    ['file:///etc/passwd', 'main', 'refuse'],
    ['javascript:alert(1)', 'main', 'refuse'],
    ['data:text/html,<b>x</b>', 'main', 'refuse'],
    ['blob:https://a.com/x', 'main', 'refuse'],
    ['slack://open', 'main', 'refuse'],
    ['https://u:p@github.com/', 'main', 'refuse'],
    [`http://127.0.0.1:${String(PAGES)}/p/x`, 'main', 'refuse'],
    [`http://localhost:${String(PAGES)}/`, 'main', 'refuse'],
    ['http://v-0123456789abcdef.localhost:9999/mcp-app', 'main', 'refuse'],
    ['https://youtube.com/embed/x', 'sub', 'allow'],
    ['data:text/html,x', 'sub', 'allow'],
    ['about:srcdoc', 'sub', 'allow'],
    [`http://127.0.0.1:${String(PAGES)}/p/x`, 'sub', 'refuse'],
    ['file:///x', 'sub', 'refuse'],
    ['not a url', 'main', 'refuse'],
  ] as const)('%s in a %s frame: %s', (url, frame, want) => {
    expect(webNav(url, frame, PORTS)).toBe(want);
  });
});

describe('blockedRequest: nothing reaches bise from a web page (rule 4)', () => {
  it.each([
    [`http://127.0.0.1:${String(PAGES)}/api/p/x/send`, true],
    [`ws://127.0.0.1:${String(PAGES)}/p/x/events`, true],
    [`http://localhost:${String(PAGES)}/`, true],
    [`http://[::1]:${String(PAGES)}/`, true],
    [`http://127.1.2.3:${String(PAGES)}/`, true],
    ['http://v-0123456789abcdef.localhost:1234/', true],
    ['http://anything.localhost/', true],
    ['http://127.0.0.1:5173/', false],
    ['http://localhost/', false],
    [`https://example.com:${String(PAGES)}/`, false],
    ['https://github.com/', false],
  ] as const)('%s: %s', (url, want) => {
    expect(blockedRequest(url, PORTS)).toBe(want);
  });
  it('no port known yet: loopback dev servers pass, view hosts never', () => {
    expect(blockedRequest(`http://127.0.0.1:${String(PAGES)}/`, new Set())).toBe(false);
    expect(blockedRequest('http://v-1.localhost:80/', new Set())).toBe(true);
  });
});

describe('goVerdict: his own go never leaves a blank tab (panel-page-blank, architect m_65009)', () => {
  it.each([
    ['https://github.com/', 'shown'],
    ['http://127.0.0.1:3000/x', 'shown'],
    // his bug: a bise page loads in his browser, never refused into about:blank
    [`http://127.0.0.1:${String(PAGES)}/p/vibebot-sprint-critique`, 'external'],
    [`http://localhost:${String(PAGES)}/`, 'external'],
    ['mailto:a@b.c', 'external'],
    [`http://u:p@127.0.0.1:${String(PAGES)}/p/x`, 'refused'],
    ['file:///etc/passwd', 'refused'],
    ['javascript:alert(1)', 'refused'],
    ['not a url', 'refused'],
  ] as const)('%s: %s', (url, want) => {
    expect(goVerdict(url, PORTS)).toBe(want);
  });
  it("a page's own navigation to bise stays refused (webNav), whatever his go does", () => {
    expect(webNav(`http://127.0.0.1:${String(PAGES)}/p/x`, 'main', PORTS)).toBe('refuse');
  });
});

describe("bise's ports, from main's live facts", () => {
  it('a state, a page, a hub hello name their pages server', () => {
    expect(bisePortsOf({ ev: 'state', pages_url: 'http://127.0.0.1:7841/p' })).toEqual([7841]);
    expect(bisePortsOf({ ev: 'page', url: 'http://127.0.0.1:7842/p/weekly' })).toEqual([7842]);
    expect(bisePortsOf({ ev: 'app_update', url: 'https://bise.dev/x.dmg' })).toEqual([]);
    expect(bisePortsOf({ ev: 'state' })).toEqual([]);
  });
  it('loopback hosts', () => {
    for (const h of ['localhost', 'LOCALHOST', 'v-1.localhost', '127.0.0.1', '127.9.0.1', '[::1]']) expect(isLoopback(h), h).toBe(true);
    for (const h of ['localhost.com', '128.0.0.1', '10.0.0.1', 'example.com']) expect(isLoopback(h), h).toBe(false);
  });
});
