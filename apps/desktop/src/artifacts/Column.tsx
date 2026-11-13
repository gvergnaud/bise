// The artifacts library (bise fait mac mockup 7, decision d2; page artifacts-gallery v7): the
// 'library' label, all artifacts, recent, unfiled (each with its line icon), then the 'folders'
// label with its + and the folders (the same for every project). Rows 28, margin 0 8, r 6, the
// chosen one --c-active; labels 12/500 text-3, 28 high, 12 above (nm-frame's sidebar rows). It
// draws in the window's sidebar on the artifacts route (ArtLibrary, library.tsx, `side`), else in
// the page's own column. A theme opens under itself onto its task folders (4, then 'N more'). At
// rest a row is its name and, in pink, a dot when something new is inside; its count shows on hover
// or focus only, in the dot's place, and for 2 s after a drop on it (density rule 2). The folders'
// + puts a field in the list at its place; F2 or a double-click renames one in place; a right-click
// opens the folder's menu. A row takes a drop (drag.ts: holding 600 ms on a closed theme opens it).
// Under 900 px the same rows sit in the crumb's popover (Fold.tsx). A pure render of its props:
// Screen.tsx holds the state. Owner: nm-pages-a (art-folders' before).
//
// His q-library pick (artifacts-signal v13, designer m_62289): the views on top (For you, Recent,
// Kept), the folder tree (a done task's folder folds into its theme's quiet 'N done', its name
// text-2), the slots (slots.ts: Shared · <org>, bise), then grey From agents · Unfiled (only while
// something is in it) · Archived. A folder's count is his files only, never the agents'.
import type { DragEvent, KeyboardEvent, MouseEvent, ReactNode } from 'react';
import { AppIcon, Input, Press, sentenceCase, tipAttrs, type AppIconName } from '@/ds';
import { library } from '@/proto/words_library.gen';
import { countWords, samePlace, type At, type Count, type Folder, type Library } from './folders';
import type { SectionView } from './sections';

/** How many task folders an open theme shows before its 'N more'. */
export const TASKS_SHOWN = 4;

/** A field in the column: a new folder (in a theme or at the top) or a rename. */
export type Editing = { readonly kind: 'new'; readonly parent: string | null } | { readonly kind: 'rename'; readonly id: string } | null;

/** What a row needs to take a drop (drag.ts's useDrop builds it). */
export interface DropTarget {
  /** the row a drag is over now */
  readonly over: string | null;
  readonly onDragOver: (id: string, e: DragEvent) => void;
  readonly onDragLeave: (id: string) => void;
  readonly onDrop: (id: string, e: DragEvent) => void;
}

export interface ColumnProps {
  readonly lib: Library;
  readonly counts: ReadonlyMap<string, Count>;
  readonly at: At;
  /** the themes open under themselves */
  readonly open: ReadonlySet<string>;
  /** the themes showing all their tasks (past 'N more') */
  readonly full: ReadonlySet<string>;
  readonly editing: Editing;
  /** a row whose count shows for 2 s (a drop just landed on it) */
  readonly flash: string | null;
  readonly drop?: DropTarget;
  readonly onGo: (at: At) => void;
  readonly onToggle: (theme: string) => void;
  readonly onMore: (theme: string) => void;
  readonly onNew: () => void;
  readonly onRename: (id: string) => void;
  /** the field's ⏎ (its words) or esc (null) */
  readonly onEdited: (name: string | null) => void;
  readonly onMenu: (id: string, e: MouseEvent) => void;
  /** drawn in the window's sidebar (ArtLibrary, library.tsx): the sidebar's rows, not sticky */
  readonly side?: boolean;
  /** ⌘ held (app/held.ts): each of the first 9 rows shows its ⌘1-9 in its end (rail-keys, designer m_45492) */
  readonly digits?: boolean;
  /** the section view he is on (For you, Done, From agents): For you and From agents are column rows */
  readonly sect?: SectionView;
  /** the column's For you and From agents rows: that view at the library's top (route '' or 'agents') */
  readonly onSect?: (v: SectionView) => void;
  /** the task folders whose work is all done (folders.ts doneFolders): in their theme's 'N done' */
  readonly done?: ReadonlySet<string>;
  /** the themes whose 'N done' is open */
  readonly doneOpen?: ReadonlySet<string>;
  readonly onDone?: (theme: string) => void;
  /** something is unfiled (any section): the Unfiled row shows */
  readonly unfiledAny?: boolean;
  /** the slots' rows after the tree (slots.ts: Shared · <org>, then bise) */
  readonly slots?: ReactNode;
  readonly testId: string;
}

/** A theme's task folders as the column draws them: the live ones (4, then 'N more'), and its done ones
 *  behind 'N done' (drawn when open). Pure. */
export function tasksOf(p: Pick<ColumnProps, 'lib' | 'full' | 'done' | 'doneOpen'>, theme: string): { shown: Folder[]; more: number; done: Folder[]; doneShown: boolean } {
  const tasks = p.lib.tasks.get(theme) ?? [];
  const isDone = (f: Folder) => p.done?.has(f.id) === true;
  const live = tasks.filter((f) => !isDone(f));
  const shown = p.full.has(theme) ? live : live.slice(0, TASKS_SHOWN);
  return { shown, more: live.length - shown.length, done: tasks.filter(isDone), doneShown: p.doneOpen?.has(theme) === true };
}

/** The places the column draws, top to bottom: For you, Recent, Kept, then each theme and, open, the
 *  tasks it shows, then Unfiled (when something is in it) and Archived. ⌘1-9 go to the first nine
 *  (app/keys.ts); the column numbers them the same way. Pure. */
export function columnPlaces(p: Pick<ColumnProps, 'lib' | 'open' | 'full' | 'done' | 'doneOpen' | 'unfiledAny'>): At[] {
  const out: At[] = [{ at: 'all' }, { at: 'recent' }, { at: 'kept' }];
  for (const t of p.lib.themes) {
    out.push({ at: 'folder', id: t.id });
    if (!p.open.has(t.id)) continue;
    const ts = tasksOf(p, t.id);
    for (const f of [...ts.shown, ...(ts.doneShown ? ts.done : [])]) out.push({ at: 'folder', id: f.id });
  }
  if (p.unfiledAny !== false) out.push({ at: 'unfiled' });
  out.push({ at: 'archived' });
  return out;
}

/** A place's ⌘ digit in the column (1-9), none past the ninth. Pure. */
function digitOf(places: readonly At[], at: At): number | undefined {
  const i = places.findIndex((x) => same(x, at));
  return i >= 0 && i < 9 ? i + 1 : undefined;
}

function Digit({ n, testId }: { n: number | undefined; testId: string }) {
  return n === undefined ? null : (
    <span className="end">
      <kbd data-testid={`${testId}.digit`}>{`⌘${String(n)}`}</kbd>
    </span>
  );
}

const same = samePlace;

export function FolderColumn(p: ColumnProps) {
  const { lib, counts, at, testId } = p;
  // ⌘ held: the first nine places' ⌘1-9 in their ends, in place of the dot or the count
  const places = p.digits === true ? columnPlaces(p) : [];
  const digit = (a: At) => (p.digits === true ? digitOf(places, a) : undefined);
  const field = (parent: string | null) =>
    p.editing?.kind === 'new' && p.editing.parent === parent ? <EditRow key={`new:${parent ?? ''}`} indent={parent !== null} initial="" onEdited={p.onEdited} testId={`${testId}.field`} /> : null;
  const sect = p.sect ?? 'you';
  const toSect = (v: SectionView) => { if (p.onSect) p.onSect(v); else p.onGo({ at: 'all' }); };
  const folderRow = (f: Folder, indent: boolean, done = false) => {
    if (p.editing?.kind === 'rename' && p.editing.id === f.id) {
      return <EditRow key={f.id} indent={indent} initial={f.name} onEdited={p.onEdited} testId={`${testId}.field`} />;
    }
    const tasks = lib.tasks.get(f.id) ?? [];
    const isOpen = p.open.has(f.id);
    return (
      <FolderRow key={f.id} id={f.id} name={f.name} indent={indent} theme={!indent} done={done} hasTasks={tasks.length > 0} isOpen={isOpen}
        on={same(at, { at: 'folder', id: f.id })} digit={digit({ at: 'folder', id: f.id })} count={counts.get(f.id)} flash={p.flash === f.id} over={p.drop?.over === f.id}
        onGo={() => { p.onGo({ at: 'folder', id: f.id }); }} onToggle={() => { p.onToggle(f.id); }} onRename={() => { p.onRename(f.id); }}
        onMenu={(e) => { p.onMenu(f.id, e); }} drop={p.drop} testId={`${testId}.f.${f.id}`} />
    );
  };
  return (
    <nav className={`art-col${p.side === true ? ' side' : ''}`} aria-label={library.library_head} data-testid={testId}>
      <div className="art-col-head">{library.library_head}</div>
      {/* the views (his q-library pick): Home (the root; For you | All is the bar's, designer m_63213), Recent, Kept */}
      <PlaceRow icon="artifacts" name={library.home} on={at.at === 'all' && sect === 'you'} digit={digit({ at: 'all' })}
        onGo={() => { toSect('you'); }} testId={`${testId}.all`} />
      <PlaceRow icon="scheduled" name={library.recent} on={at.at === 'recent'} digit={digit({ at: 'recent' })} onGo={() => { p.onGo({ at: 'recent' }); }} testId={`${testId}.recent`} />
      <PlaceRow icon="pin" name={library.sect_kept} on={at.at === 'kept'} digit={digit({ at: 'kept' })} onGo={() => { p.onGo({ at: 'kept' }); }} testId={`${testId}.kept`} />
      {/* the folders' label, its + at its end (mockup 7): a field for a new folder's name */}
      <div className="art-col-head">
        <span className="nm">{library.folders}</span>
        <Press look="art-col-add" onClick={p.onNew} aria-label={library.new_folder_tip} data-testid={`${testId}.new`} {...tipAttrs(library.new_folder_tip)}>
          <AppIcon name="plus" size={14} />
        </Press>
      </div>
      {lib.themes.map((t) => {
        const { shown, more, done, doneShown } = tasksOf(p, t.id);
        return [
          folderRow(t, false),
          ...(p.open.has(t.id)
            ? [
                ...shown.map((f) => folderRow(f, true)),
                more > 0 ? (
                  <Press key={`${t.id}:more`} look="art-col-row art-col-more" onClick={() => { p.onMore(t.id); }} data-testid={`${testId}.more.${t.id}`}>
                    <span className="nm">{library.n_more({ n: more })}</span>
                  </Press>
                ) : null,
                field(t.id),
                // the done tasks' folders at the theme's end, quiet (designer m_62289)
                done.length > 0 ? (
                  <Press key={`${t.id}:done`} look="art-col-row art-col-more in" aria-expanded={doneShown} onClick={() => { p.onDone?.(t.id); }} data-testid={`${testId}.done.${t.id}`}>
                    <span className="nm">{`${library.n_done({ n: done.length })} ${doneShown ? '▾' : '▸'}`}</span>
                  </Press>
                ) : null,
                ...(doneShown ? done.map((f) => folderRow(f, true, true)) : []),
              ]
            : []),
        ];
      })}
      {field(null)}
      {p.slots}
      {/* grey, one click each: Unfiled (while something is in it), Archived; the agents' files are the
          bar's All (designer m_63213: no From agents row) */}
      <div className="art-col-grey" data-testid={`${testId}.grey`}>
        {p.unfiledAny !== false && (
          <PlaceRow icon="inbox" name={sentenceCase(library.unfiled)} on={at.at === 'unfiled'} digit={digit({ at: 'unfiled' })} count={counts.get('unfiled')} countAtRest
            over={p.drop?.over === 'unfiled'} drop={p.drop} dropId="unfiled" flash={p.flash === 'unfiled'}
            onGo={() => { p.onGo({ at: 'unfiled' }); }} testId={`${testId}.unfiled`} />
        )}
        <PlaceRow icon="archive" name={library.archived_place} on={at.at === 'archived'} digit={digit({ at: 'archived' })} onGo={() => { p.onGo({ at: 'archived' }); }} testId={`${testId}.archived`} />
      </div>
    </nav>
  );
}

/** A row's right end: the pink dot at rest when new is inside, the count on hover or focus (or always). */
function End({ count, atRest = false, flash = false }: { count: Count | undefined; atRest?: boolean; flash?: boolean }) {
  const fresh = count?.fresh ?? 0;
  return (
    <span className="end" data-rest={atRest || undefined} data-flash={flash || undefined}>
      {fresh > 0 && <span className="dot" {...tipAttrs(library.tip_new_dot({ n: fresh }))} />}
      {count !== undefined && (count.total > 0 || atRest) && <span className="ct" {...tipAttrs(library.tip_count({ n: count.n }))}>{countWords(count)}</span>}
    </span>
  );
}

function dropAttrs(drop: DropTarget | undefined, id: string) {
  if (drop === undefined) return {};
  return {
    onDragOver: (e: DragEvent) => { drop.onDragOver(id, e); },
    onDragLeave: () => { drop.onDragLeave(id); },
    onDrop: (e: DragEvent) => { drop.onDrop(id, e); },
  };
}

export function PlaceRow(p: { icon: AppIconName; name: string; on: boolean; digit?: number | undefined; count?: Count | undefined; countAtRest?: boolean; over?: boolean; flash?: boolean; drop?: DropTarget | undefined; dropId?: string; onGo: () => void; testId: string }) {
  return (
    <Press look="art-col-row" aria-current={p.on || undefined} data-over={p.over || undefined} onClick={p.onGo} data-testid={p.testId}
      {...(p.dropId === undefined ? {} : dropAttrs(p.drop, p.dropId))}>
      <AppIcon name={p.icon} size={16} />
      <span className="nm">{p.name}</span>
      {p.digit !== undefined ? <Digit n={p.digit} testId={p.testId} /> : p.count !== undefined && p.count.total > 0 && <End count={p.count} atRest={p.countAtRest} flash={p.flash} />}
    </Press>
  );
}

export interface FolderRowProps {
  id: string;
  name: string;
  indent: boolean;
  theme: boolean;
  /** a done task's folder, behind its theme's 'N done': its name text-2 */
  done?: boolean;
  hasTasks: boolean;
  isOpen: boolean;
  on: boolean;
  digit: number | undefined;
  count: Count | undefined;
  flash: boolean;
  over: boolean;
  drop: DropTarget | undefined;
  onGo: () => void;
  onToggle: () => void;
  onRename: () => void;
  onMenu: (e: MouseEvent) => void;
  testId: string;
}

export function FolderRow(p: FolderRowProps) {
  const key = (e: KeyboardEvent) => {
    if (e.key === 'F2') {
      e.preventDefault();
      p.onRename();
    }
  };
  return (
    <Press look={`art-col-row${p.indent ? ' in' : ''}${p.done === true ? ' done' : ''}`} aria-current={p.on || undefined} aria-expanded={p.hasTasks ? p.isOpen : undefined}
      data-over={p.over || undefined} data-folder={p.id}
      onClick={() => {
        if (p.hasTasks && (!p.isOpen || p.on)) p.onToggle();
        p.onGo();
      }}
      onDoubleClick={(e) => { e.preventDefault(); p.onRename(); }}
      onContextMenu={(e) => { e.preventDefault(); p.onMenu(e); }}
      onKeyDown={key} data-testid={p.testId} {...dropAttrs(p.drop, p.id)}>
      {p.theme && <AppIcon name="folder" size={16} />}
      <span className="nm" {...tipAttrs(p.name)}>{p.name}</span>
      {p.digit !== undefined ? <Digit n={p.digit} testId={p.testId} /> : <End count={p.count} flash={p.flash} />}
    </Press>
  );
}

/** The column's field: a new folder's name or a rename, ⏎ keeps it, esc (or leaving it empty) drops it. */
function EditRow({ indent, initial, onEdited, testId }: { indent: boolean; initial: string; onEdited: (name: string | null) => void; testId: string }) {
  return (
    <span className={`art-col-edit${indent ? ' in' : ''}`}>
      <Input look="art-col-input" autoFocus defaultValue={initial} placeholder={library.folder_name} aria-label={library.folder_name} data-testid={testId}
        onFocus={(e) => { e.currentTarget.select(); }}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === 'Enter') {
            e.preventDefault();
            const v = e.currentTarget.value.trim();
            onEdited(v === '' ? null : v);
          } else if (e.key === 'Escape') {
            e.preventDefault();
            onEdited(null);
          }
        }}
        onBlur={(e) => {
          const v = e.currentTarget.value.trim();
          onEdited(v === '' || v === initial ? null : v);
        }} />
    </span>
  );
}
