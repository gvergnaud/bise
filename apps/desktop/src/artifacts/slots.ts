// The library column's slots (his q-library pick, designer m_62289): the groups after the folder
// tree that other modules fill, in a fixed order: 'Shared · <org>' (kb-desktop-ui, only while a
// shared base exists), then 'bise' (apps-gallery, ~/bise's top folders). One seam so neither side
// imports the other's module (apps-gallery m_62903): a slot draws its rows (with the column's
// PlaceRow/FolderRow and the 'art-col-row' look), owns its places (At `ext`, its key opaque, kept in
// the route as 'x/<key>' so ⌘←/⌘→ and the selection stay one model), and on its place draws the
// crumb and the list in the page's (no For you · Everything switch, no sections or filter bar).
// Its owner adds one entry to SLOTS. Owner: artifacts-keep.
import type { ReactNode } from 'react';
import type { At } from './folders';

export interface SlotRows {
  readonly at: At;
  readonly onGo: (at: At) => void;
  /** ⌘ held (app/held.ts) */
  readonly digits: boolean;
  /** the library's project scope ('' all projects) */
  readonly project: string;
}

export interface ColumnSlot {
  readonly id: 'shared' | 'bise';
  /** its rows after the tree (and after the slots before it) */
  readonly rows: (p: SlotRows) => ReactNode;
  /** the `ext` place keys it owns */
  readonly owns: (key: string) => boolean;
  /** its place's crumb, in the page's head */
  readonly crumb: (key: string) => ReactNode;
  /** its place's list, in place of the gallery */
  readonly body: (key: string, project: string) => ReactNode;
}

/** The slots, in the column's order: Shared, then bise. Each owner adds its entry here. */
export const SLOTS: readonly ColumnSlot[] = [];

/** The slot that owns an `ext` place, if any. Pure. */
export function slotOf(key: string, slots: readonly ColumnSlot[] = SLOTS): ColumnSlot | undefined {
  return slots.find((s) => s.owns(key));
}
