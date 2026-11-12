// A folder of bise as the folder tab draws it (docs/plugin-apps.md A6; apps-hub-files' folder/list,
// m_63095): the hub's answer read (FolderList: the folder, its crumb root first, its rows folders
// first), the crumb's words (the root says 'bise', never a path), each row's line as the ☰ list's
// ('Sheets · 2 h', a file no app opens 'your Mac · 2 h'), and what a click on a row opens: a folder
// in place, a file the artifact it is the target of, else the file itself (apps-host's 'file'
// entity, view/open {kind:'entry'}). Ids are the hub's, opaque: never built, split or read here.
// Pure: FolderTab.tsx draws it. Owner: apps-gallery.
import type { Crumb, FolderList, FolderRow } from '@/proto/files';
import { panel } from '@/proto/words_panel.gen';
import { appWord } from '../apps';

/** The hub's folder/list answer, or null when it isn't one (an older hub's, a wrong shape). Pure. */
export function folderListOf(x: unknown): FolderList | null {
  if (x === null || typeof x !== 'object') return null;
  const o = x as Record<string, unknown>;
  const folder = crumbOf(o['folder']);
  const crumb = o['crumb'];
  const rows = o['rows'];
  if (folder === null || !Array.isArray(crumb) || !Array.isArray(rows)) return null;
  const cs = crumb.map(crumbOf);
  if (cs.some((c) => c === null)) return null;
  const rs = rows.filter(isRow);
  return { folder, crumb: cs as Crumb[], rows: rs };
}

function crumbOf(x: unknown): Crumb | null {
  if (x === null || typeof x !== 'object') return null;
  const o = x as Record<string, unknown>;
  return typeof o['id'] === 'string' && o['id'] !== '' && typeof o['name'] === 'string' ? { id: o['id'], name: o['name'] } : null;
}

function isRow(x: unknown): x is FolderRow {
  if (x === null || typeof x !== 'object') return false;
  const o = x as Record<string, unknown>;
  return typeof o['id'] === 'string' && o['id'] !== '' && typeof o['name'] === 'string' && (o['kind'] === 'file' || o['kind'] === 'folder');
}

/** A folder's name as the window says it: the root (its name empty) is 'bise'. Pure. */
export const nameOfFolder = (c: Crumb): string => (c.name === '' ? panel.bise : c.name);

/** The crumb as drawn: 'bise › Plugins › Boards', the root first, the folder listed last (current). Pure. */
export function crumbRow(l: FolderList): { id: string; name: string; current: boolean }[] {
  return [...l.crumb, l.folder].map((c, i, all) => ({ id: c.id, name: nameOfFolder(c), current: i === all.length - 1 }));
}

/** A row's line, as the ☰ list's: its app and its age ('Sheets · 2 h'); a folder: its age alone. Pure. */
export function rowLine(r: FolderRow, age: (ms: number) => string, now: number): string {
  const when = age(Math.max(0, now - r.mtime));
  if (r.kind === 'folder') return when;
  const app = appWord({ opener: r.opener ?? null, path: r.name });
  return app === null ? when : `${app} · ${when}`;
}

/** What a click on a row opens: a folder in this tab, an artifact, or the file itself by its id. */
export type RowOpen =
  | { readonly kind: 'folder'; readonly entry: string; readonly name: string }
  | { readonly kind: 'artifact'; readonly project: string; readonly id: string }
  | { readonly kind: 'file'; readonly project: string; readonly entry: string; readonly name: string };

/** A row's open (`project`: the hub that listed it, for a file with no artifact). Pure. */
export function openOf(r: FolderRow, project: string): RowOpen {
  if (r.kind === 'folder') return { kind: 'folder', entry: r.id, name: r.name };
  if (r.artifact !== undefined) return { kind: 'artifact', project: r.artifact.project, id: r.artifact.id };
  return { kind: 'file', project, entry: r.id, name: r.name };
}
