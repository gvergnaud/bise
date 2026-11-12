import { describe, expect, it } from 'vitest';
import { common } from '@/proto/words_common.gen';
import { library } from '@/proto/words_library.gen';
import { appCompletions, appOf, appTabs, appWord, endingOf, inApps, leadOf, opensTip, pickApp, PRS, type AppRow, type AppTag } from './apps';

const sheets = { plugin: 'sheets', app: 'Sheets', type_title: 'Excel workbook' };
const boards = { plugin: 'boards', app: 'Boards', type_title: 'Board' };
const row = (extra: Partial<AppRow> = {}): AppRow => ({ url: '', ...extra });
const tag = (id: string, word = id): AppTag => ({ id, word });
const many = (t: AppTag, n: number): AppTag[] => Array.from({ length: n }, () => t);
const words = (tabs: readonly { word: string; n: number }[]) => tabs.map((t) => `${t.word} ${String(t.n)}`);

describe('which app a row follows (the hub picked it, A2: the window never matches)', () => {
  it("its opener's app, keyed by plugin and app", () => {
    expect(appOf(row({ opener: sheets, path: '/p/pricing.xlsx' }))).toEqual({ id: 'app:sheets/Sheets', word: 'Sheets' });
  });

  it("a PR is bise's review whatever its opener; a link his browser; nothing else his Mac", () => {
    expect(appOf(row({ pr: { state: 'open' }, url: 'https://github.com/x/y/pull/1' })).id).toBe(PRS.id);
    expect(appOf(row({ url: 'https://example.com' })).id).toBe('links');
    expect(appOf(row({ path: '/p/logs.zip' }))).toEqual({ id: 'mac', word: common.your_mac });
  });
});

describe('the filter: one tab per app here, the most first, five at most, the rest under More', () => {
  const writer = tag('app:docs/Writer', 'Writer');
  const sh = tag('app:sheets/Sheets', 'Sheets');
  const bo = tag('app:boards/Boards', 'Boards');
  const pv = tag('app:media/Preview', 'Preview');
  const sl = tag('app:slides/Slides', 'Slides');
  const si = tag('app:sites/Sites', 'Sites');

  it("the mocks' row: Writer 5 · Sheets 4 · Pull requests 3 · Boards 2, no zero counts", () => {
    const { tabs, more } = appTabs([...many(writer, 5), ...many(sh, 4), ...many(PRS, 3), ...many(bo, 2)]);
    expect(words(tabs)).toEqual(['Writer 5', 'Sheets 4', 'Pull requests 3', 'Boards 2']);
    expect(more).toEqual([]);
  });

  it('ties by word; past five the rest goes under More, the most first', () => {
    const { tabs, more } = appTabs([...many(writer, 3), ...many(sh, 3), ...many(bo, 2), ...many(pv, 2), sl, si, tag('mac', 'your Mac')]);
    expect(words(tabs)).toEqual(['Sheets 3', 'Writer 3', 'Boards 2', 'Preview 2', 'Sites 1']);
    expect(words(more)).toEqual(['Slides 1', 'your Mac 1']);
  });

  it('Pull requests never fold into More: the last shown app goes there first', () => {
    const { tabs, more } = appTabs([...many(writer, 6), ...many(sh, 5), ...many(bo, 4), ...many(pv, 3), ...many(sl, 2), PRS], [], 5);
    expect(words(tabs)).toEqual(['Writer 6', 'Sheets 5', 'Boards 4', 'Preview 3', 'Pull requests 1']);
    expect(words(more)).toEqual(['Slides 2']);
  });

  it('a kept tag shows at 0 (the archived PRs behind the Pull requests tab), never twice', () => {
    expect(words(appTabs([writer], [PRS]).tabs)).toEqual(['Writer 1', 'Pull requests 0']);
    expect(words(appTabs([PRS, PRS], [PRS]).tabs)).toEqual(['Pull requests 2']);
  });

  it('the picked apps keep their rows, OR inside the dimension; none picked keeps every row', () => {
    const rows = [row({ opener: sheets }), row({ opener: boards }), row({ opener: sheets }), row({ path: '/p/x.zip' })];
    expect(inApps(rows, appOf, new Set(['app:sheets/Sheets']))).toHaveLength(2);
    expect(inApps(rows, appOf, new Set(['app:sheets/Sheets', 'app:boards/Boards']))).toHaveLength(3);
    expect(inApps(rows, appOf, new Set())).toHaveLength(4);
  });

  it('a click shows one app alone (again: all), ⌘-click adds or takes one off', () => {
    const one = pickApp(new Set(), 'app:sheets/Sheets', false);
    expect([...one]).toEqual(['app:sheets/Sheets']);
    expect([...pickApp(one, 'app:sheets/Sheets', false)]).toEqual([]);
    const two = pickApp(one, 'app:boards/Boards', true);
    expect([...two]).toEqual(['app:sheets/Sheets', 'app:boards/Boards']);
    expect([...pickApp(two, 'app:sheets/Sheets', true)]).toEqual(['app:boards/Boards']);
    expect([...pickApp(two, 'all', false)]).toEqual([]);
  });

  it("a picked app with nothing left keeps its tab at 0, so he can take it off (rule 3)", () => {
    expect(words(appTabs([writer], [sh]).tabs)).toEqual(['Writer 1', 'Sheets 0']);
  });

  it("the app: token completes from the apps in scope: starts-with first, then the most", () => {
    const tabs = [{ id: 'a', word: 'Slides', n: 2 }, { id: 'b', word: 'Sheets', n: 6 }, { id: 'c', word: 'Sites', n: 9 }, { id: 'd', word: 'Writer', n: 9 }];
    expect(appCompletions(tabs, 's').map((t) => t.word)).toEqual(['Sites', 'Sheets', 'Slides']);
    expect(appCompletions(tabs, 'it').map((t) => t.word)).toEqual(['Sites', 'Writer']);
  });
});

describe("a card's lead word and its type tip", () => {
  it("leads with its app: 'Sheets', its tip 'Excel workbook · opens in Sheets'", () => {
    const a = row({ opener: sheets, path: '/p/pricing.xlsx' });
    expect(leadOf(a)).toBe('Sheets');
    expect(opensTip(a)).toBe(library.tip_opens({ kind: 'Excel workbook', app: 'Sheets' }));
  });

  it("no app: 'opens on your Mac', its type by its ending ('ZIP file'), a folder 'Folder'", () => {
    const zip = row({ path: '/p/logs.zip' });
    expect(leadOf(zip)).toBe(library.opens_mac);
    expect(opensTip(zip)).toBe(library.tip_opens_mac({ kind: library.file_of({ ending: 'ZIP' }) }));
    expect(opensTip(row(), 'site/')).toBe(library.tip_opens_mac({ kind: library.folder_kind }));
  });

  it("the list's app column: the app, 'your Mac' when none (the card's line keeps 'opens on your Mac'), a PR's or a link's kind", () => {
    expect(appWord(row({ opener: sheets, path: '/p/q3.xlsx' }))).toBe('Sheets');
    expect(appWord(row({ path: '/p/logs.zip' }))).toBe(common.your_mac);
    expect(appWord(row({ url: 'https://example.com' }))).toBeNull();
  });

  it('a PR and a link keep their own marks: no lead, no type tip', () => {
    const pr = row({ pr: { state: 'open' }, url: 'https://github.com/x/y/pull/1' });
    const link = row({ url: 'https://example.com' });
    expect([leadOf(pr), opensTip(pr), leadOf(link), opensTip(link)]).toEqual([null, null, null, null]);
  });

  it('the ending of a name: upper case, none for a folder, a dot file or no dot', () => {
    expect([endingOf('logs.zip'), endingOf('/a/b/Q3.pricing.XLSX'), endingOf('site/'), endingOf('.env'), endingOf('Makefile'), endingOf('x.')]).toEqual(['ZIP', 'XLSX', null, null, null, null]);
  });
});
