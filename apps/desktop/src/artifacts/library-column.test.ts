// His q-library pick (artifacts-signal v13, designer m_62289): the column's views, its done tasks'
// fold, its grey places and slots (Column.tsx, folders.ts, slots.ts), and a folder's For you ·
// Everything switch (FolderSwitch.tsx). The hub's section is read as sent, never re-decided.
import { describe, expect, it } from 'vitest';
import type { Artifact } from '@/proto';
import { columnPlaces, tasksOf } from './Column';
import { atOfPath, atPath, countsOf, doneFolders, isHis, itemsAt, libraryOf, samePlace, type At, type Folder } from './folders';
import { folderView } from './FolderSwitch';
import type { Item } from './model';
import { slotOf, type ColumnSlot } from './slots';

const NOW = Date.UTC(2026, 10, 12, 18, 0);
const art = (id: string, over: Partial<Artifact> = {}): Artifact =>
  ({ id, kind: 'file', title: id, agent: 'perf', url: '', at_ms: NOW, version: 1, new: false, ...over });
const item = (a: Artifact): Item => ({ project: 'harness', projectName: 'harness', a });

const desk: Folder = { id: 'desk', name: 'desktop', by: 'main', named_by_you: false };
const live: Folder = { id: 'live', name: 'artifacts-keep', parent: 'desk', by: 'main', named_by_you: false };
const ended: Folder = { id: 'ended', name: 'perf', parent: 'desk', by: 'main', named_by_you: false };
const FOLDERS = [desk, live, ended];
const ITEMS: Item[] = [
  item(art('plan', { folder: 'live', section: 'open', by: 'artifacts-keep' })),
  item(art('shot', { folder: 'live', section: 'agents', at_ms: NOW - 1 })),
  item(art('kept', { folder: 'ended', section: 'kept', done: true, at_ms: NOW - 2 })),
  item(art('log', { folder: 'ended', section: 'agents', done: true, at_ms: NOW - 3 })),
  item(art('loose', { section: 'agents', at_ms: NOW - 4 })),
];

describe("the column (his q-library pick)", () => {
  const lib = libraryOf(FOLDERS, ITEMS);

  it('keeps Kept and a slot place in the route, and back', () => {
    for (const at of [{ at: 'kept' }, { at: 'ext', key: 'bise:plugins' }, { at: 'folder', id: 'live' }] as At[]) {
      expect(atOfPath(atPath(at))).toEqual(at);
    }
    expect(atPath({ at: 'ext', key: 'bise' })).toBe('x/bise');
    expect(samePlace({ at: 'ext', key: 'bise' }, { at: 'ext', key: 'bise:plugins' })).toBe(false);
  });

  it("Kept holds the hub's Kept section, any folder", () => {
    expect(itemsAt(lib, ITEMS, { at: 'kept' }).map((it) => it.a.id)).toEqual(['kept']);
  });

  it("a folder's count is his files only, never the agents'", () => {
    const his = ITEMS.filter(isHis);
    const counts = countsOf(lib, his, his);
    expect(counts.get('live')?.total).toBe(1);
    expect(counts.get('desk')?.total).toBe(2);
  });

  it("a task folder whose work is all done folds into its theme's 'N done'", () => {
    const done = doneFolders(lib, ITEMS);
    expect([...done]).toEqual(['ended']);
    const p = { lib, open: new Set(['desk']), full: new Set<string>(), done, doneOpen: new Set<string>(), unfiledAny: true };
    expect(tasksOf(p, 'desk')).toMatchObject({ shown: [live], more: 0, done: [ended], doneShown: false });
    // the views on top, the tree (the done one only once its fold is open), then Unfiled and Archived
    expect(columnPlaces(p)).toEqual([
      { at: 'all' }, { at: 'recent' }, { at: 'kept' }, { at: 'folder', id: 'desk' }, { at: 'folder', id: 'live' }, { at: 'unfiled' }, { at: 'archived' },
    ]);
    expect(columnPlaces({ ...p, doneOpen: new Set(['desk']), unfiledAny: false })).toEqual([
      { at: 'all' }, { at: 'recent' }, { at: 'kept' }, { at: 'folder', id: 'desk' }, { at: 'folder', id: 'live' }, { at: 'folder', id: 'ended' }, { at: 'archived' },
    ]);
  });

  it('a slot owns its places by key; none owns an unknown one', () => {
    const bise: ColumnSlot = { id: 'bise', rows: () => null, owns: (k) => k === 'bise' || k.startsWith('bise:'), crumb: () => null, body: () => null };
    expect(slotOf('bise:plugins', [bise])?.id).toBe('bise');
    expect(slotOf('shared:acme', [bise])).toBeUndefined();
  });
});

describe("a folder's For you · Everything switch (his condition)", () => {
  const lib = libraryOf(FOLDERS, ITEMS);
  const here = itemsAt(lib, ITEMS, { at: 'folder', id: 'desk' });

  it('For you shows his files and counts the agents\' it hides; Everything shows every file', () => {
    expect(folderView(here, 'you')).toEqual({ shown: here.filter((it) => it.a.section !== 'agents'), hidden: 2 });
    expect(folderView(here, 'all')).toEqual({ shown: here, hidden: 0 });
  });
});
