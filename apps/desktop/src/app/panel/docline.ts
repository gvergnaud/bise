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
import { RPC } from '@/views/protocol';

/** What the tab knows of its artifact's row (none: a ~/bise file). */
export interface RowFacts {
  /** its newest version */
  readonly newest: number;
  /** its file is gone from disk (the row's `gone`) */
  readonly gone: boolean;
  /**
   * bise kept a copy of its newest version (the row's `copy`): a gone file opens it, read only
   * (his m_66157); `was` is where its file was
   */
  readonly kept?: { readonly was: string };
  /** who saved its newest version (ArtifactVersion.by); none: not said */
  readonly by?: SavedBy;
  /**
   * TODO(page-open-old-hub): a bise page's row (kind 'page') on a hub without initialize's
   * page_frame (ProjectRow.page_frame), which refuses its view/open as a bad handle
   */
  readonly oldHubPage?: boolean;
}

/** The head's line. `app` is the app's name ('Sheets'); `version` the one shown. */
export type DocLine =
  | { readonly kind: 'loading' }
  | { readonly kind: 'gone' }
  /** its file is gone, the tab shows bise's copy of its newest version, read only; `was` its tip */
  | { readonly kind: 'copy'; readonly app: string; readonly version?: number; readonly was: string }
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
 * Whether the tab asks the hub for its document: not for a file gone with no copy kept (its line
 * says it was removed); a gone file with bise's copy opens the copy (the hub serves it). Pure.
 */
export function asksHub(row: RowFacts | undefined): boolean {
  return row?.gone !== true || row.kept !== undefined;
}

/**
 * The line, in this order: gone (no copy kept), off, failed, loading, bise's copy of a file gone,
 * a conflict, an older version, else the facts with the save state ('read only' when it can't
 * save). An older version: one he opened so (`follows` false: a chip's version, Open with's) says
 * Open vN; under a tab that follows the newest, a newer one saved meanwhile says who saved it when
 * he can edit (else the tab reopens on it by itself: newerAct). `app` when the view never said
 * one: the opener the row named. Pure.
 */
export function docLine(state: ViewState, row: RowFacts | undefined, rowApp: string, follows = true): DocLine {
  if (!asksHub(row)) return { kind: 'gone' };
  switch (state.kind) {
    case 'failed':
      // a hub that doesn't know view/open yet (method not found): today's page draws it (F0, apps-lead m_62087)
      if (state.code === RPC.notFound) return { kind: 'page' };
      // TODO(page-open-old-hub): a hub before views-broken's fix refuses a page's view/open as a
      // bad handle (its source is a URL, no file): today's page draws it, never 'didn't load'.
      // A hub with the fix (page_frame) is believed: a page it refuses stays said
      if (row?.oldHubPage === true && state.code === DOC_ERR.badHandle) return { kind: 'page' };
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
  // the file's kind, when its app names one in its manifest, after the app: 'Code · TypeScript'
  // (Opener.kind, code-view: architect m_67161, designer m_67265); Sheets names none
  const app = d.opener.kind == null ? d.opener.app : `${d.opener.app} · ${d.opener.kind}`;
  // his own save made the row's newest (by 'you'): the tab shows it, no 'newer' line over his own work
  // (the write's answer has no version, A4)
  const mine = d.savedHere && row?.by?.kind === 'you' && d.document.version !== undefined && d.document.version < row.newest;
  const version = mine ? row.newest : d.document.version;
  // the file gone, bise's copy shown: read only, no save state (designer m_66162)
  if (row?.gone === true && row.kept !== undefined) return { kind: 'copy', app, ...(version === undefined ? {} : { version }), was: row.kept.was };
  if (d.save.kind === 'conflict') return { kind: 'conflict', app, ...(version === undefined ? {} : { version }) };
  const newest = row?.newest;
  if (version !== undefined && newest !== undefined && version < newest) {
    // a tab on the newest that he can edit: who saved the newer one, and Reload (a tab he can't
    // edit reopens on it by itself, newerAct); a version he picked stays, with Open vN
    if (!follows) return { kind: 'older', app, version, newest };
    if (d.document.writable) return { kind: 'newer', app, version, newest, by: row?.by ?? { kind: 'unknown' } };
  }
  // refused for any reason but an older version (its own line) reads as read only
  const readOnly = !d.document.writable || (d.save.kind === 'refused' && d.save.why !== 'older_version');
  return { kind: 'facts', app, ...(version === undefined ? {} : { version }), save: d.save, readOnly };
}
