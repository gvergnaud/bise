// The right-click menus (page artifacts-gallery v7): a card's (open ⏎, quick look space, versions
// v, put in a message @, copy link ⌘C, show in Finder r, move to… m, archive ⌫; its keys on the
// right, the same the grid takes) and a folder's (rename F2, new folder inside, merge into…, delete
// folder…), plus the merge-into picker ('merge "page heroes" into…', its foot says what happens).
// Each is the ds PickerPop (the menu law), from the point he clicked or the ··· he pressed.
// Owner: art-folders.
import { useRef, type RefObject } from 'react';
import { AppIcon, PickerPop, type PickerRow } from '@/ds';
import { library } from '@/proto/words_library.gen';
import type { Library } from './folders';

/** Where a menu opens: a point on the screen (a right-click), or a box (a ··· pressed, a card: its rect). */
export interface MenuAt {
  readonly x: number;
  readonly y: number;
  readonly w?: number;
  readonly h?: number;
}

/** The box of an element, for a menu that opens from it. */
export function atOf(el: Element): MenuAt {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, h: r.height };
}

/** A fixed box at the point, for the PickerPop to open from (its ref set by React, never in render). */
export function MenuAnchor({ at, anchor }: { at: MenuAt; anchor: RefObject<HTMLSpanElement | null> }) {
  return <span className="art-menu-at" style={{ left: at.x, top: at.y, width: at.w ?? 0, height: at.h ?? 0 }} ref={anchor} />;
}

/** A card's menu's actions. */
export type CardAct = 'open' | 'look' | 'versions' | 'put' | 'copy' | 'finder' | 'move' | 'archive' | KeepAct;

/** His Keep (◆, it never goes to Done) or Let go (sections.ts; page artifacts-signal v3). */
export type KeepAct = 'keep' | 'let_go';

/** The menu's keep group, its own (plugin-apps adds an Open with group beside it): Let go when every one picked is kept, else Keep. Pure. */
export function keepRows(allKept: boolean): { id: KeepAct; name: string }[] {
  return [allKept ? { id: 'let_go', name: library.let_go } : { id: 'keep', name: library.keep }];
}

/** The card menu's rows: what applies to it (a link has no Finder), its key at the right. Pure. */
export function cardRows(o: { finder: boolean; several: number }): { id: CardAct; name: string; key: string }[] {
  if (o.several > 1) {
    return [
      { id: 'move', name: library.menu_move, key: 'm' },
      { id: 'put', name: library.put_in_message, key: '@' },
      { id: 'archive', name: library.archive, key: '⌫' },
    ];
  }
  const rows: { id: CardAct; name: string; key: string }[] = [
    { id: 'open', name: library.menu_open, key: '⏎' },
    { id: 'look', name: library.menu_look, key: 'space' },
    { id: 'versions', name: library.menu_versions, key: 'v' },
    { id: 'put', name: library.put_in_message, key: '@' },
    { id: 'copy', name: library.menu_copy, key: '⌘C' },
  ];
  if (o.finder) rows.push({ id: 'finder', name: library.menu_finder, key: 'r' });
  rows.push({ id: 'move', name: library.menu_move, key: 'm' }, { id: 'archive', name: library.archive, key: '⌫' });
  return rows;
}

export function CardMenu({ at, finder, several, kept, onAct, onClose, testId }: {
  at: MenuAt;
  finder: boolean;
  several: number;
  /** every one it acts on is kept: its keep row says Let go */
  kept: boolean;
  onAct: (a: CardAct) => void;
  onClose: () => void;
  testId: string;
}) {
  const anchor = useRef<HTMLSpanElement>(null);
  const rows: PickerRow[] = cardRows({ finder, several }).map((r) => ({ id: r.id, name: r.name, trailing: r.key, testId: `${testId}.${r.id}` }));
  const keep: PickerRow[] = keepRows(kept).map((r) => ({ id: r.id, name: r.name, testId: `${testId}.${r.id}` }));
  return (
    <>
      <MenuAnchor at={at} anchor={anchor} />
      <PickerPop anchor={anchor} align="start" groups={[{ id: 'card', rows }, { id: 'keep', rows: keep }]} onClose={onClose}
        onPick={(id) => { onClose(); onAct(id as CardAct); }} testId={testId} />
    </>
  );
}

/** A folder's menu's actions. */
export type FolderAct = 'rename' | 'inside' | 'merge' | 'delete';

/** The folder menu's rows: 'new folder inside' only on a theme (two levels at most). Pure. */
export function folderRows(isTheme: boolean): { id: FolderAct; name: string; key?: string }[] {
  return [
    { id: 'rename', name: library.menu_rename, key: 'F2' },
    ...(isTheme ? [{ id: 'inside' as const, name: library.menu_new_inside }] : []),
    { id: 'merge', name: library.menu_merge },
    { id: 'delete', name: library.menu_delete },
  ];
}

export function FolderMenu({ at, isTheme, onAct, onClose, testId }: { at: MenuAt; isTheme: boolean; onAct: (a: FolderAct) => void; onClose: () => void; testId: string }) {
  const anchor = useRef<HTMLSpanElement>(null);
  const rows: PickerRow[] = folderRows(isTheme).map((r) => ({ id: r.id, name: r.name, ...(r.key === undefined ? {} : { trailing: r.key }), testId: `${testId}.${r.id}` }));
  return (
    <>
      <MenuAnchor at={at} anchor={anchor} />
      <PickerPop anchor={anchor} align="start" groups={[{ id: 'folder', rows }]} onClose={onClose}
        onPick={(id) => { onClose(); onAct(id as FolderAct); }} testId={testId} />
    </>
  );
}

/** The merge picker: every other folder of its level (a theme into a theme, a task into any folder), its foot the consequence. */
export function MergePicker({ at, lib, id, n, onMerge, onClose, testId }: { at: MenuAt; lib: Library; id: string; n: number; onMerge: (into: string) => void; onClose: () => void; testId: string }) {
  const anchor = useRef<HTMLSpanElement>(null);
  const f = lib.byId.get(id);
  const name = f?.name ?? '';
  const isTheme = (f?.parent ?? null) === null && (lib.tasks.get(id)?.length ?? 0) > 0;
  const targets = [...lib.byId.values()].filter((x) => x.id !== id && x.parent !== id && (!isTheme || (x.parent ?? null) === null));
  const rows: PickerRow[] = targets.map((x, i) => {
    const p = x.parent ?? null;
    return {
      id: x.id,
      name: x.name,
      glyph: <AppIcon name="folder" size={16} />,
      ...(p === null ? {} : { secondary: lib.byId.get(p)?.name ?? '' }),
      ...(i < 9 ? { trailing: String(i + 1) } : {}),
      testId: `${testId}.row.${x.id}`,
    };
  });
  return (
    <>
      <MenuAnchor at={at} anchor={anchor} />
      <PickerPop anchor={anchor} align="start" inline title={library.merge_into({ name })} groups={[{ id: 'into', rows }]} onClose={onClose}
        onPick={(into) => { onClose(); onMerge(into); }}
        footer={<span className="art-merge-say">{library.merge_say({ n, name })}</span>} testId={testId} />
    </>
  );
}
