// Where a link he clicks goes, and a web tab's title (page newtab-browser, designer m_64680).
import { describe, expect, it } from 'vitest';
import { closesOnVerdict, linkGoes, linkPlan, siteOfUrl, webKey } from './web';
import { webTitle } from './look';

describe('a link he clicks', () => {
  it('a web tab; his browser with ⌘, for a site he always opens there, or anything not http(s)', () => {
    expect(linkGoes('https://github.com/x', false, [])).toBe('tab');
    expect(linkGoes('https://github.com/x', true, [])).toBe('browser');
    expect(linkGoes('https://www.slack.com/client', false, ['slack.com'])).toBe('browser');
    expect(linkGoes('mailto:a@b.c', false, [])).toBe('browser');
    expect(siteOfUrl('https://WWW.Notion.so/x')).toBe('notion.so');
    expect(siteOfUrl('file:///x')).toBeNull();
  });
  it("a page of one of his hubs opens in the panel, never a web tab (panel-page-blank, his m_65005)", () => {
    const projects = [
      { project: 'dashboard', pages_url: 'http://127.0.0.1:47520/p' },
      { project: 'harness', pages_url: 'http://127.0.0.1:7841/p' },
    ];
    expect(linkPlan('http://127.0.0.1:47520/p/vibebot-sprint-critique', false, [], projects)).toEqual({ go: 'artifact', project: 'dashboard', id: 'vibebot-sprint-critique' });
    expect(linkPlan('http://127.0.0.1:7841/p/plan/v/2', false, [], projects)).toEqual({ go: 'artifact', project: 'harness', id: 'plan', v: 2 });
    // ⌘: his browser, as for any link
    expect(linkPlan('http://127.0.0.1:47520/p/vibebot-sprint-critique', true, [], projects)).toEqual({ go: 'browser' });
    // a port no hub of his serves: a web tab (a dev server); the web's rule otherwise
    expect(linkPlan('http://127.0.0.1:5173/p/x', false, [], projects)).toEqual({ go: 'tab' });
    expect(linkPlan('https://github.com/x', false, [], projects)).toEqual({ go: 'tab' });
  });
  it('his go out or refused closes a tab that never showed a page, never one with a page', () => {
    const page = { url: 'https://github.com/', title: 'GitHub', favicon: null, back: false, forward: false, loading: false, failed: false, zoom: 1, entries: [{ url: 'https://github.com/', title: 'GitHub' }], index: 0 };
    expect(closesOnVerdict('external', null)).toBe(true);
    expect(closesOnVerdict('refused', { ...page, url: '', entries: [] })).toBe(true);
    expect(closesOnVerdict('external', page)).toBe(false);
    expect(closesOnVerdict('shown', null)).toBe(false);
  });
  it("a tab's key names its project's panel", () => {
    expect(webKey('acme-1', 'web:2')).toBe('acme-1|web:2');
  });
});

describe("a web tab's title", () => {
  it('whole when short, else cut at a word with …', () => {
    expect(webTitle('GitHub')).toBe('GitHub');
    expect(webTitle('The quick brown fox jumps over the lazy dog again and again')).toBe('The quick brown fox jumps over the lazy…');
    expect(webTitle('x'.repeat(50))).toBe(`${'x'.repeat(40)}…`);
  });
});
