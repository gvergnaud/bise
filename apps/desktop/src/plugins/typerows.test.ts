import { describe, expect, it } from 'vitest';
import { common } from '@/proto/words_common.gen';
import { settings } from '@/proto/words.gen';
import { claimPicks, claimsView, openerSet, opensWords, typeLine, typeLines, typeName, type TypeOpener } from './typerows';

const sheets: TypeOpener = { plugin: 'sheets', tool: 'grid', app: 'Sheets' };
const grid: TypeOpener = { plugin: 'grid', tool: 'open', app: 'Grid' };
const boards: TypeOpener = { plugin: 'boards', tool: 'board', app: 'Boards' };

describe("File types: one row per type, its app as a button, a faint status", () => {
  it("the mocks' rows: '.xlsx Sheets default', '.csv Grid your pick Reset', '.zip your Mac no app'", () => {
    const xlsx = typeLine({ type: '*.xlsx', title: 'Excel workbook', app: sheets, others: [grid] });
    expect([xlsx.name, xlsx.ending, xlsx.app, xlsx.status, xlsx.reset]).toEqual(['.xlsx', true, 'Sheets', settings.file_type_default, false]);
    // a folder rule reads as its title, a word (UI font), never an ending (mono)
    const site = typeLine({ type: '*/index.html', title: 'Website', others: [] });
    expect([site.name, site.ending]).toEqual(['Website', false]);
    const csv = typeLine({ type: '*.csv', title: 'CSV table', app: grid, others: [sheets], his: true });
    expect([csv.name, csv.app, csv.status, csv.reset]).toEqual(['.csv', 'Grid', settings.file_type_his, true]);
    const zip = typeLine({ type: '*.zip', title: 'ZIP archive', others: [] });
    expect([zip.app, zip.status, zip.reset]).toEqual([common.your_mac, settings.file_type_none, false]);
  });

  it('the menu lists every candidate, the current one marked; one app alone: no menu', () => {
    expect(typeLine({ type: '*.csv', title: 'CSV', app: grid, others: [sheets], his: true }).picks).toEqual([
      { id: 'grid#open', app: 'Grid', current: true },
      { id: 'sheets#grid', app: 'Sheets', current: false },
    ]);
    expect(typeLine({ type: '*.board.json', title: 'Board', app: boards, others: [] }).picks).toEqual([]);
  });

  it("'his' with no app left (his pick's plugin is off) reads as no app, no Reset", () => {
    const l = typeLine({ type: '*.csv', title: 'CSV', others: [], his: true });
    expect([l.status, l.reset]).toEqual([settings.file_type_none, false]);
  });

  it("a type's name: its ending for a plain ending key, else its title (a folder rule, a scoped type)", () => {
    expect([typeName('*.board.json', 'Board'), typeName('*/index.html', 'Website'), typeName('@documents/boards/*.board.json', 'Board')]).toEqual(['.board.json', 'Website', 'Board']);
  });

  it('endings first, then titles, each by name', () => {
    const rows = typeLines([
      { type: '*/index.html', title: 'Website', others: [] },
      { type: '*.xlsx', title: 'Excel workbook', others: [] },
      { type: '*.csv', title: 'CSV', others: [] },
    ]);
    expect(rows.map((r) => r.name)).toEqual(['.csv', '.xlsx', 'Website']);
  });

  it('a pick sends opener_set with its plugin and tool; Reset without', () => {
    expect(openerSet('*.csv', 'sheets#grid')).toEqual({ cmd: 'opener_set', type: '*.csv', plugin: 'sheets', tool: 'grid' });
    expect(openerSet('*.csv')).toEqual({ cmd: 'opener_set', type: '*.csv' });
  });
});

describe("a plugin page's 'opens': its types, and those another app opens now", () => {
  it("'Excel workbook (.xlsx) · CSV table (.csv)', then '.csv: Grid opens them, your pick'", () => {
    const w = opensWords('sheets', [{ type: '*.xlsx', title: 'Excel workbook' }, { type: '*.csv', title: 'CSV table' }, { type: '*/index.html', title: 'Website' }], [
      { type: '*.xlsx', title: 'Excel workbook', app: sheets, others: [] },
      { type: '*.csv', title: 'CSV table', app: grid, others: [sheets], his: true },
    ]);
    expect(w.list).toBe([settings.plugins_opens_type({ title: 'Excel workbook', ending: '.xlsx' }), settings.plugins_opens_type({ title: 'CSV table', ending: '.csv' }), 'Website'].join(' · '));
    expect(w.elsewhere).toEqual([`${settings.plugins_opens_elsewhere({ name: '.csv', app: 'Grid' })}, ${settings.file_type_his}`]);
  });
});

describe("the add sheet's claims: what it would open, and Keep / Use for a type another app has", () => {
  const claims = [
    { type: '*.xlsx', title: 'Excel workbook', app: 'Tables', tool: 'table', now: 'Sheets' },
    { type: '*.csv', title: 'CSV table', app: 'Tables', tool: 'table' },
    { type: '*.json', title: 'JSON', app: 'Tables', tool: 'table', wide: true },
  ];

  it("its opens: '.xlsx · .csv · every .json file'; one line for .xlsx, Keep Sheets / Use Tables", () => {
    const v = claimsView(claims);
    expect(v.opens).toBe(['.xlsx', '.csv', settings.plugins_claim_wide({ ending: '.json' })].join(' · '));
    expect(v.taken).toEqual([{ type: '*.xlsx', line: settings.plugins_claim_now({ ending: '.xlsx', app: 'Sheets' }), keep: settings.plugins_claim_keep({ app: 'Sheets' }), use: settings.plugins_claim_use({ app: 'Tables' }) }]);
  });

  it('Keep by default sends nothing; Use sends opener_set for that type alone', () => {
    expect(claimPicks('tables', claims, new Set())).toEqual([]);
    expect(claimPicks('tables', claims, new Set(['*.xlsx', '*.csv']))).toEqual([{ cmd: 'opener_set', type: '*.xlsx', plugin: 'tables', tool: 'table' }]);
  });
});
