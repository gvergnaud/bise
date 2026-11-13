// Where a link he clicks goes, and a web tab's title (page newtab-browser, designer m_64680).
import { describe, expect, it } from 'vitest';
import { linkGoes, siteOfUrl, webKey } from './web';
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
