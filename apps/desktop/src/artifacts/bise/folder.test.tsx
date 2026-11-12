// @vitest-environment happy-dom
// A folder of bise (docs/plugin-apps.md A6): folder/list's answer read, the crumb, each row's line
// and open, and the folder tab as he uses it, against a fake window door (window.bise.views.call).
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { FolderList, FolderRow } from '@/proto/files';
import { common } from '@/proto/words_common.gen';
import { library } from '@/proto/words_library.gen';
import { panel } from '@/proto/words_panel.gen';
import { crumbRow, folderListOf, openOf, rowLine } from './folder';
import { FolderTab } from './FolderTab';

const sheets = { plugin: 'sheets', tool: 'open_sheet', app: 'Sheets', type: '*.xlsx', type_title: 'Excel workbook' };
const NOW = 1_794_405_600_000;
const H = 3_600_000;

const row = (over: Partial<FolderRow> & Pick<FolderRow, 'id' | 'name'>): FolderRow => ({ kind: 'file', size: 12_400, mtime: NOW - 2 * H, ...over });

const BOARDS: FolderList = {
  folder: { id: 'e:3', name: 'Boards' },
  crumb: [{ id: 'e:1', name: '' }, { id: 'e:2', name: 'Plugins' }],
  rows: [
    row({ id: 'e:4', name: 'old', kind: 'folder', size: 0 }),
    row({ id: 'e:5', name: 'q3.xlsx', opener: sheets, openers: [sheets], artifact: { project: 'acme', id: 'q3' } }),
    row({ id: 'e:6', name: 'logs.zip' }),
  ],
};

const age = (ms: number) => `${String(Math.round(ms / H))} h`;

describe("folder/list's answer, read", () => {
  it('the crumb root first, the root said bise, the folder listed last and current', () => {
    expect(crumbRow(BOARDS)).toEqual([
      { id: 'e:1', name: panel.bise, current: false },
      { id: 'e:2', name: 'Plugins', current: false },
      { id: 'e:3', name: 'Boards', current: true },
    ]);
    expect(crumbRow({ folder: { id: 'e:1', name: '' }, crumb: [], rows: [] })).toEqual([{ id: 'e:1', name: panel.bise, current: true }]);
  });

  it("refuses what isn't one; a row of an unknown kind is left out", () => {
    expect(folderListOf(null)).toBeNull();
    expect(folderListOf({ folder: { id: 'e:1', name: '' }, rows: [] })).toBeNull();
    const l = folderListOf({ ...BOARDS, rows: [...BOARDS.rows, { id: 'e:9', name: 'x', kind: 'socket' }] });
    expect(l?.rows.map((r) => r.id)).toEqual(['e:4', 'e:5', 'e:6']);
  });

  it("a row's line is the list's: its app and its age; no app: your Mac; a folder: its age", () => {
    expect(BOARDS.rows.map((r) => rowLine(r, age, NOW))).toEqual(['2 h', 'Sheets · 2 h', `${common.your_mac} · 2 h`]);
  });

  it('a folder opens in place, a file its artifact, else the file by its id (opaque, as given)', () => {
    expect(BOARDS.rows.map((r) => openOf(r, 'home'))).toEqual([
      { kind: 'folder', entry: 'e:4', name: 'old' },
      { kind: 'artifact', project: 'acme', id: 'q3' },
      { kind: 'file', project: 'home', entry: 'e:6', name: 'logs.zip' },
    ]);
  });
});

// the tab, with the panel's open functions and the home project faked
const opened: unknown[] = [];
const tabbed: unknown[] = [];
vi.mock('@/app/panel/state', () => ({ openWhat: (what: unknown, instead: unknown) => { opened.push([what, instead]); } }));
vi.mock('@/app/panel/tabs', () => ({ openTab: (e: unknown) => { tabbed.push(e); } }));
vi.mock('@/store/hooks', () => ({ useProjects: () => [{ project: 'home', name: 'bise', home: true }] }));

function door(answer: (cmd: Record<string, unknown>) => unknown) {
  const calls: Record<string, unknown>[] = [];
  const views = { call: (cmd: Record<string, unknown>) => { calls.push(cmd); return Promise.resolve(answer(cmd)); }, frames: () => undefined };
  (window as unknown as { bise: unknown }).bise = { views };
  return calls;
}

afterEach(() => {
  cleanup();
  opened.length = 0;
  tabbed.length = 0;
  delete (window as unknown as { bise?: unknown }).bise;
});

describe('the folder tab, as he uses it', () => {
  it("asks the home hub for its folder, draws the crumb and the rows; a click opens each its way", async () => {
    const calls = door(() => ({ result: BOARDS }));
    render(<FolderTab entry="e:3" tabId="dir:e:3" />);
    await screen.findByTestId('bise.folder.crumb');
    expect(calls[0]).toEqual({ cmd: 'folder_list', project: 'home', folder: 'e:3' });
    expect(screen.getByTestId('bise.folder.crumb').textContent).toBe(`${panel.bise}›Plugins›Boards`);
    expect(screen.getByTestId('bise.folder.crumb.2').getAttribute('aria-current')).toBe('page');
    expect(screen.getByTestId('bise.folder.row.1').textContent).toContain('q3.xlsx');
    expect(screen.getByTestId('bise.folder.row.1').textContent).toContain('Sheets · ');
    expect(screen.getByTestId('bise.folder.row.1').textContent).toContain('12.4 KB');
    fireEvent.click(screen.getByTestId('bise.folder.row.0.open'));
    fireEvent.click(screen.getByTestId('bise.folder.row.1.open'));
    fireEvent.click(screen.getByTestId('bise.folder.row.2.open'));
    // the crumb's root goes back to the root (null: one tab for it), a parent by its id
    fireEvent.click(screen.getByTestId('bise.folder.crumb.0'));
    fireEvent.click(screen.getByTestId('bise.folder.crumb.1'));
    expect(opened).toEqual([
      [{ kind: 'folder', entry: 'e:4', name: 'old' }, 'dir:e:3'],
      [{ kind: 'folder', entry: null, name: panel.bise }, 'dir:e:3'],
      [{ kind: 'folder', entry: 'e:2', name: 'Plugins' }, 'dir:e:3'],
    ]);
    expect(tabbed).toEqual([{ kind: 'artifact', project: 'acme', id: 'q3' }, { kind: 'file', project: 'home', entry: 'e:6', name: 'logs.zip' }]);
  });

  it("the root asks with no folder; an empty folder says 'empty'; back to the front, it lists again", async () => {
    const calls = door(() => ({ result: { folder: { id: 'e:1', name: '' }, crumb: [], rows: [] } }));
    render(<FolderTab entry={null} tabId="dir:" />);
    await screen.findByTestId('bise.folder.empty');
    expect(screen.getByTestId('bise.folder.empty').textContent).toBe(library.folder_empty);
    expect(calls[0]).toEqual({ cmd: 'folder_list', project: 'home' });
    act(() => { window.dispatchEvent(new Event('focus')); });
    await waitFor(() => { expect(calls).toHaveLength(2); });
  });

  it("a hub that can't list yet says so; another error says its reason", async () => {
    door(() => ({ error: { code: -32034, message: 'not supported' } }));
    render(<FolderTab entry={null} tabId="dir:" />);
    expect((await screen.findByTestId('bise.folder.said')).textContent).toBe(library.folder_not_yet);
    cleanup();
    door(() => ({ error: { code: -32603, message: 'disk gone' } }));
    render(<FolderTab entry={null} tabId="dir:" />);
    expect((await screen.findByTestId('bise.folder.said')).textContent).toBe(library.folder_failed({ why: 'disk gone' }));
  });
});
