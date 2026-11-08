// The folder tiles (page artifacts-gallery v7 'la page'): on 'all artifacts', a big tile per theme
// shaped as a folder (a tab over its back; page folder-tiles, pick A) with its newest three artifacts
// as a cover, the folder glyph, its name, and '412 artifacts · today'; inside a theme, a small
// tile per task folder, its name and its count. Each takes a drop, a click opens it, a right-click
// its menu. The cover's pictures are the grid's thumbnails (art-thumbs' Thumb, through `thumb`).
// The pink dot when something new is inside. A pure render of its props. Owner: art-folders.
import type { MouseEvent, ReactNode } from 'react';
import { AppIcon, Press, tipAttrs } from '@/ds';
import { library } from '@/proto/words_library.gen';
import { countWords, tileCountWords, type Count, type Folder } from './folders';
import type { DropTarget } from './Column';
import type { Item } from './model';

export interface TileProps {
  readonly folder: Folder;
  readonly count: Count | undefined;
  /** its newest time's words: 'today', 'oct 22' */
  readonly when: string;
  readonly cover?: readonly Item[];
  readonly thumb?: (it: Item) => ReactNode;
  readonly drop?: DropTarget;
  readonly flash: boolean;
  readonly onOpen: () => void;
  readonly onMenu: (e: MouseEvent) => void;
  readonly testId: string;
}

function dropProps(drop: DropTarget | undefined, id: string) {
  if (drop === undefined) return {};
  return {
    onDragOver: (e: React.DragEvent) => { drop.onDragOver(id, e); },
    onDragLeave: () => { drop.onDragLeave(id); },
    onDrop: (e: React.DragEvent) => { drop.onDrop(id, e); },
  };
}

/**
 * A theme's big tile, shaped as a folder so it never reads as an artifact card (page folder-tiles,
 * pick A): a tab on top of a back one step darker than a card's thumb, the 3-thumbnail cover in it;
 * then the folder glyph, its name and '32 artifacts · today'.
 */
export function BigTile(p: TileProps) {
  const cover = p.cover ?? [];
  const fresh = p.count?.fresh ?? 0;
  return (
    <Press look="art-tile big" onClick={p.onOpen} onContextMenu={(e) => { e.preventDefault(); p.onMenu(e); }}
      data-over={p.drop?.over === p.folder.id || undefined} data-testid={p.testId} {...dropProps(p.drop, p.folder.id)}>
      <span className="tab" aria-hidden="true" />
      <span className="cover" data-n={cover.length}>
        {cover.map((it) => (
          <span key={`${it.project}/${it.a.id}`} className="pic">{p.thumb?.(it)}</span>
        ))}
        {cover.length === 0 && <span className="pic none"><AppIcon name="folder" size={16} /></span>}
      </span>
      <span className="nm">
        <AppIcon name="folder" size={14} />
        <span className="t" {...tipAttrs(p.folder.name)}>{p.folder.name}</span>
        {fresh > 0 && <span className="dot" {...tipAttrs(library.tip_new_dot({ n: fresh }))} />}
      </span>
      <span className="ln" data-flash={p.flash || undefined}>{`${tileCountWords(p.count)} · ${p.when}`}</span>
    </Press>
  );
}

/**
 * A folder in the ☰ list: a row at the list's height, on top of the artifacts' rows: its glyph, its
 * name (the dot when something in it is new), its count, its newest time (designer m_40850).
 */
export function FolderListRow(p: TileProps) {
  const fresh = p.count?.fresh ?? 0;
  return (
    <Press look="art-frow" onClick={p.onOpen} onContextMenu={(e) => { e.preventDefault(); p.onMenu(e); }}
      data-over={p.drop?.over === p.folder.id || undefined} data-testid={p.testId} {...dropProps(p.drop, p.folder.id)}>
      <AppIcon name="folder" size={16} />
      <span className="t" {...tipAttrs(p.folder.name)}>{p.folder.name}</span>
      {fresh > 0 ? <span className="dot" {...tipAttrs(library.tip_new_dot({ n: fresh }))} /> : <span />}
      <span className="ct" data-flash={p.flash || undefined}>{(p.count?.total ?? 0) === 0 ? library.tile_empty : countWords(p.count)}</span>
      <span className="when">{p.when}</span>
    </Press>
  );
}

/** A task folder's small tile, inside its theme: its icon, its name, its count on hover. */
export function SmallTile(p: TileProps) {
  const fresh = p.count?.fresh ?? 0;
  return (
    <Press look="art-tile small" onClick={p.onOpen} onContextMenu={(e) => { e.preventDefault(); p.onMenu(e); }}
      data-over={p.drop?.over === p.folder.id || undefined} data-testid={p.testId} {...dropProps(p.drop, p.folder.id)}>
      <AppIcon name="folder" size={16} />
      <span className="t" {...tipAttrs(p.folder.name)}>{p.folder.name}</span>
      <span className="end" data-flash={p.flash || undefined}>
        {fresh > 0 && <span className="dot" {...tipAttrs(library.tip_new_dot({ n: fresh }))} />}
        <span className="ct">{countWords(p.count)}</span>
      </span>
    </Press>
  );
}
