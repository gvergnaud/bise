// bise's web history, pure (main/webhistory.ts; architect m_64583): what is never recorded, a
// visit noted, the prune, a removal, his switch, a broken file. Fake URLs and times only.
import { describe, expect, it } from 'vitest';
import { cleared, EMPTY_HISTORY, siteOf, titled, TITLE_MAX, withOut, historyText, KEEP_DAYS, MAX_PAGES, noteVisit, parseHistory, pruned, recordable, URL_MAX, VISITS_KEPT, webHistoryFile, withKeep, withoutUrl } from './webhistory';
import { standInPaths } from './userdata';

const DAY = 86_400_000;
const NOW = 1_800_000_000_000;

describe('what is never recorded (one rule, architect m_64583)', () => {
  it.each([
    ['https://github.com/gvergnaud/bise', 'https://github.com/gvergnaud/bise'],
    ['http://example.com', 'http://example.com/'],
    ['https://example.com/s?q=a+b#top', 'https://example.com/s?q=a+b#top'],
  ])('keeps %s as he opened it, its query kept', (raw, kept) => {
    expect(recordable(raw)).toBe(kept);
  });
  it.each([
    'https://user:pass@example.com/',
    'https://user@example.com/',
    'http://127.0.0.1:47438/p/page?token=x',
    'http://127.9.9.9/',
    'http://localhost:5173/',
    'http://v-abc.localhost:4000/mcp-app',
    'http://[::1]:80/',
    'http://0.0.0.0/',
    'file:///Users/me/notes.html',
    'mailto:me@example.com',
    'javascript:alert(1)',
    'not a url',
    `https://example.com/${'a'.repeat(URL_MAX)}`,
  ])('never %s', (raw) => {
    expect(recordable(raw)).toBeNull();
  });
  it('never anything but a string', () => {
    for (const x of [null, undefined, 3, {}, ['https://a.com']]) expect(recordable(x)).toBeNull();
  });
});

describe('a visit noted', () => {
  it('its page first, its visit first, counted; typed kept', () => {
    let h = noteVisit(EMPTY_HISTORY, 'https://a.com/', NOW - 2, false);
    h = noteVisit(h, 'https://b.com/', NOW - 1, false);
    h = noteVisit(h, 'https://a.com/', NOW, true);
    expect(h.pages.map((p) => p.url)).toEqual(['https://a.com/', 'https://b.com/']);
    expect(h.pages[0]).toEqual({ url: 'https://a.com/', n: 2, visits: [{ at: NOW, typed: true }, { at: NOW - 2, typed: false }] });
  });
  it(`keeps a page's last ${String(VISITS_KEPT)} visits and counts them all`, () => {
    let h = EMPTY_HISTORY;
    for (let i = 0; i < VISITS_KEPT + 5; i++) h = noteVisit(h, 'https://a.com/', NOW + i, false);
    expect(h.pages[0]?.visits).toHaveLength(VISITS_KEPT);
    expect(h.pages[0]?.n).toBe(VISITS_KEPT + 5);
    expect(h.pages[0]?.visits[0]?.at).toBe(NOW + VISITS_KEPT + 4);
  });
  it(`${String(MAX_PAGES)} pages at most, the least recent go`, () => {
    let h = EMPTY_HISTORY;
    for (let i = 0; i <= MAX_PAGES; i++) h = noteVisit(h, `https://p${String(i)}.com/`, NOW + i, false);
    expect(h.pages).toHaveLength(MAX_PAGES);
    expect(h.pages.some((p) => p.url === 'https://p0.com/')).toBe(false);
  });
  it('nothing while his switch is off; off clears it; on again starts empty', () => {
    const h = noteVisit(EMPTY_HISTORY, 'https://a.com/', NOW, false);
    const off = withKeep(h, false);
    expect(off).toEqual({ keep: false, pages: [], out: [] });
    expect(noteVisit(off, 'https://b.com/', NOW, true)).toBe(off);
    expect(withKeep(off, true)).toEqual({ keep: true, pages: [], out: [] });
  });
});

describe('the prune, a removal, a clear', () => {
  it(`visits older than ${String(KEEP_DAYS)} days go, then a page with none left`, () => {
    let h = noteVisit(EMPTY_HISTORY, 'https://old.com/', NOW - (KEEP_DAYS + 1) * DAY, false);
    h = noteVisit(h, 'https://mixed.com/', NOW - (KEEP_DAYS + 2) * DAY, false);
    h = noteVisit(h, 'https://mixed.com/', NOW - DAY, false);
    const p = pruned(h, NOW);
    expect(p.pages.map((x) => x.url)).toEqual(['https://mixed.com/']);
    expect(p.pages[0]?.visits).toEqual([{ at: NOW - DAY, typed: false }]);
  });
  it('one page out (⇧⌫, ×); all out, his switch kept', () => {
    let h = noteVisit(EMPTY_HISTORY, 'https://a.com/', NOW, false);
    h = noteVisit(h, 'https://b.com/', NOW, false);
    expect(withoutUrl(h, 'https://a.com/').pages.map((x) => x.url)).toEqual(['https://b.com/']);
    expect(cleared(h)).toEqual({ keep: true, pages: [], out: [] });
  });
});

describe('the file', () => {
  it('reads back what it wrote', () => {
    const h = noteVisit(noteVisit(EMPTY_HISTORY, 'https://a.com/', NOW, true), 'https://b.com/x', NOW + 1, false);
    expect(parseHistory(historyText(h))).toEqual({ h, broken: false });
    expect(parseHistory(historyText(withKeep(h, false)))).toEqual({ h: { keep: false, pages: [], out: [] }, broken: false });
  });
  it('none yet: empty, his switch on; broken: empty and said, never a throw', () => {
    expect(parseHistory(null)).toEqual({ h: EMPTY_HISTORY, broken: false });
    for (const t of ['{', '[]', '{"v":2}', 'null']) expect(parseHistory(t), t).toEqual({ h: EMPTY_HISTORY, broken: true });
  });
  it('a row it would never record, or with no visit, is dropped on read', () => {
    const t = JSON.stringify({ v: 1, keep: true, pages: [{ url: 'http://localhost:1/', n: 1, visits: [{ at: NOW }] }, { url: 'https://a.com/', n: 1, visits: [] }, { url: 'https://b.com/', n: 1, visits: [{ at: NOW, typed: 'yes' }] }] });
    expect(parseHistory(t).h.pages).toEqual([{ url: 'https://b.com/', n: 1, visits: [{ at: NOW, typed: false }] }]);
  });
  it("a test copy's file is in its own userData, never his (architect m_64583)", () => {
    const REAL = '/Users/gabriel';
    const his = webHistoryFile(`${REAL}/Library/Application Support/bise`);
    for (const env of [{ HOME: `${REAL}/.bise/gate/w/h` }, { HOME: REAL, BISE_HOME: `${REAL}/.bise/try/desktop` }]) {
      const p = standInPaths(env, REAL, 'bise');
      if (p === null) throw new Error('a stand-in');
      expect(webHistoryFile(p.userData)).not.toBe(his);
      expect(webHistoryFile(p.userData).startsWith(`${REAL}/Library`)).toBe(false);
    }
  });
});

describe('titles and his sites', () => {
  it('a title a web tab read is kept on its page, one line, capped; a later visit keeps it', () => {
    let h = noteVisit(EMPTY_HISTORY, 'https://a.com/', NOW, false);
    h = titled(h, 'https://a.com/', '  A \n page ');
    expect(h.pages[0]?.title).toBe('A page');
    h = noteVisit(h, 'https://a.com/', NOW + 1, false);
    expect(h.pages[0]?.title).toBe('A page');
    expect(titled(h, 'https://a.com/', 'x'.repeat(TITLE_MAX + 9)).pages[0]?.title).toHaveLength(TITLE_MAX);
    expect(titled(h, 'https://none.com/', 'X')).toBe(h);
    expect(parseHistory(historyText(h)).h.pages[0]?.title).toBe('A page');
  });
  it("'Always open <site> in your browser': by host, www. aside, kept across a Clear and his switch", () => {
    expect(siteOf('https://www.Slack.com/client')).toBe('slack.com');
    expect(siteOf('http://localhost:3000/')).toBeNull();
    let h = withOut(EMPTY_HISTORY, 'slack.com', true);
    h = withOut(h, 'notion.so', true);
    expect(h.out).toEqual(['notion.so', 'slack.com']);
    expect(cleared(h).out).toEqual(['notion.so', 'slack.com']);
    expect(withKeep(h, false).out).toEqual(['notion.so', 'slack.com']);
    expect(withOut(h, 'slack.com', false).out).toEqual(['notion.so']);
    expect(parseHistory(historyText(h)).h.out).toEqual(['notion.so', 'slack.com']);
  });
});
