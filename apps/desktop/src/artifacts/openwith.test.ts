import { describe, expect, it } from 'vitest';
import type { Artifact } from '@/proto';
import { openWithSub } from '@/app/menus/registry';
import { menus } from '@/proto/words_menus.gen';
import { common } from '@/proto/words_common.gen';
import { cardRows, openWithGroups } from './Menus';

const sheets = { plugin: 'sheets', tool: 'open_sheet', app: 'Sheets', type: '*.xlsx', type_title: 'Excel workbook' };
const grid = { ...sheets, plugin: 'grid', tool: 'open', app: 'Grid' };

function file(o: Partial<Artifact>): Artifact {
  return { id: 'a1', project: 'acme', kind: 'file', title: 'q3.xlsx', created_ms: 0, ...o } as Artifact;
}

describe("the card's ⋯ Open with (docs/plugin-apps.md A6b)", () => {
  it("has its row right after Open, only when something opens it", () => {
    expect(cardRows({ finder: true, several: 1, openWith: true }).map((r) => r.id).slice(0, 3)).toEqual(['open', 'with', 'look']);
    expect(cardRows({ finder: true, several: 1, openWith: true })[1]?.name).toBe(`${menus.open_with}…`);
    expect(cardRows({ finder: true, several: 1 }).map((r) => r.id)).not.toContain('with');
    // several picked: no Open with
    expect(cardRows({ finder: true, several: 2, openWith: true }).map((r) => r.id)).not.toContain('with');
  });

  it("draws the registry's own items: the apps, the hub's pick ticked, 'your Mac', then Always open as a group", () => {
    const g = openWithGroups(openWithSub(file({ path: '/w/acme/q3.xlsx', opener: sheets, openers: [grid] })));
    // three groups: the picker draws a divider between each
    expect(g.map((x) => x.id)).toEqual(['apps', 'mac', 'always']);
    expect(g[0]?.rows.map((r) => [r.id, r.name, r.current === true])).toEqual([
      ['open-with:0', 'Sheets', true],
      ['open-with:1', 'Grid', false],
    ]);
    expect(g[1]?.rows.map((r) => [r.id, r.name])).toEqual([['open-mac', common.your_mac]]);
    expect(g[2]?.rows.map((r) => r.id)).toEqual(['always-open:0', 'always-open:1']);
  });

  it("a file no app opens: 'your Mac' alone; nothing on disk: no Open with", () => {
    const g = openWithGroups(openWithSub(file({ title: 'logs.zip', path: '/w/acme/logs.zip' })));
    expect(g.flatMap((x) => x.rows.map((r) => r.id))).toEqual(['open-mac']);
    expect(openWithGroups(openWithSub(file({ path: '/w/acme/logs.zip', gone: true })))).toEqual([]);
    expect(openWithGroups(openWithSub(file({ kind: 'link', url: 'https://x.dev' })))).toEqual([]);
  });
});
