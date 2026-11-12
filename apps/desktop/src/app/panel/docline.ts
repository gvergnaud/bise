// src/app/panel · a document tab's head line (docs/plugin-apps.md A6b; designer's mocks, page
// plugin-apps-mocks §3): one line, never a banner. Its facts (the app, the version, the save
// state) or the one thing that changes what he can do: the file is gone, the app is off, the view
// didn't open, a newer version was saved while he edited, he opened an older version, the app can't
// save it. Pure: the view's state from the views store, the artifact's row facts in, a line out;
// Document.tsx draws it with the views words. Owner: apps-host.
import type { SavedBy } from '@/proto/rows';
import type { Save } from '@/views/doc';
import type { ViewState } from '@/views/store';
import { DOC_ERR, VIEW_ERR } from '@/views/port';

/** What the tab knows of its artifact's row (none: a ~/bise file). */
export interface RowFacts {
  /** its newest version */
  readonly newest: number;
  /** its file is gone from disk (the row's `gone`) */
  readonly gone: boolean;
  /** who saved its newest version (ArtifactVersion.by); none: not said */
  readonly by?: SavedBy;
}

/** The head's line. `app` is the app's name ('Sheets'); `version` the one shown. */
export type DocLine =
  | { readonly kind: 'loading' }
  | { readonly kind: 'gone' }
  | { readonly kind: 'off'; readonly app: string }
  | { readonly kind: 'failed' }
  /** a built-in page draws it: its own head says the rest */
  | { readonly kind: 'page' }
  | { readonly kind: 'conflict'; readonly app: string; readonly version?: number }
  | { readonly kind: 'older'; readonly app: string; readonly version: number; readonly newest: number }
  /** a newer version was saved under a tab on the newest he can edit: who saved it, and Reload */
  | { readonly kind: 'newer'; readonly app: string; readonly version: number; readonly newest: number; readonly by: SavedBy }
  | { readonly kind: 'facts'; readonly app: string; readonly version?: number; readonly save: Save; readonly readOnly: boolean };

/**
 * The line, in this order: gone, off, failed, loading, a conflict, an older version, else the
 * facts with the save state ('read only' when it can't save). An older version: one he opened so
 * (`follows` false: a chip's version, Open with's) says Open vN; under a tab that follows the
 * newest, a newer one saved meanwhile says who saved it when he can edit (else the tab reopens on
 * it by itself: newerAct). `app` when the view never said one: the opener the row named. Pure.
 */
export function docLine(state: ViewState, row: RowFacts | undefined, rowApp: string, follows = true): DocLine {
  if (row?.gone === true) return { kind: 'gone' };
  switch (state.kind) {
    case 'failed':
      if (state.code === DOC_ERR.gone) return { kind: 'gone' };
      if (state.code === VIEW_ERR.pluginOff) return { kind: 'off', app: rowApp };
      return { kind: 'failed' };
    case 'expired':
      return { kind: 'failed' };
    case 'frame':
      return { kind: 'page' };
    case 'none':
    case 'loading':
      return { kind: 'loading' };
    case 'live':
      break;
  }
  const d = state.doc;
  if (d === undefined) return { kind: 'loading' };
  const app = d.opener.app;
  const version = d.document.version;
  if (d.save.kind === 'conflict') return { kind: 'conflict', app, ...(version === undefined ? {} : { version }) };
  const newest = row?.newest;
  if (version !== undefined && newest !== undefined && version < newest) {
    // a tab on the newest that he can edit: who saved the newer one, and Reload (a tab he can't
    // edit reopens on it by itself, newerAct); a version he picked stays, with Open vN
    if (!follows) return { kind: 'older', app, version, newest };
    if (d.document.writable) return { kind: 'newer', app, version, newest, by: row?.by ?? { kind: 'unknown' } };
  }
  const readOnly = !d.document.writable || (d.save.kind === 'refused' && d.save.why === 'no_grant');
  return { kind: 'facts', app, ...(version === undefined ? {} : { version }), save: d.save, readOnly };
}
