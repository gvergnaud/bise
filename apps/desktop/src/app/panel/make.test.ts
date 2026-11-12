import { describe, expect, it } from 'vitest';
import { newRows, usesOf, type MakeOpt } from './make';

const sheet: MakeOpt = { plugin: 'sheets', app: 'Sheets', ending: '.xlsx', title: 'Sheet' };
const board: MakeOpt = { plugin: 'boards', app: 'Boards', ending: '.board.json', title: 'Board' };
const doc: MakeOpt = { plugin: 'docs', app: 'Writer', ending: '.docx', title: 'Document' };
const deck: MakeOpt = { plugin: 'slides', app: 'Slides', ending: '.pptx', title: 'Slides' };
const titles = (rows: readonly MakeOpt[]) => rows.map((m) => `${m.title} ${m.app}`);

describe("New ›: one row per type his apps make, the most-used app first", () => {
  it("the mocks' order: Sheets, Boards, Writer, Slides by use; ties by word", () => {
    const uses = new Map([['sheets', 4], ['boards', 2], ['docs', 2]]);
    expect(titles(newRows([deck, doc, board, sheet], '', uses))).toEqual(['Sheet Sheets', 'Board Boards', 'Document Writer', 'Slides Slides']);
  });

  it('his words filter by the row word, the app or the ending; one row per plugin and ending', () => {
    const none = new Map<string, number>();
    expect(titles(newRows([sheet, board, doc], 'writer', none))).toEqual(['Document Writer']);
    expect(titles(newRows([sheet, board, doc], 'XLSX', none))).toEqual(['Sheet Sheets']);
    expect(titles(newRows([sheet, sheet, board], 'b', none))).toEqual(['Board Boards']);
    expect(newRows([sheet, board], 'nothing', none)).toEqual([]);
  });

  it("an app's use: how many rows its opener picked, plugins only", () => {
    expect([...usesOf([{ opener: { plugin: 'sheets' } }, { opener: null }, {}, { opener: { plugin: 'sheets' } }])]).toEqual([['sheets', 2]]);
  });
});
