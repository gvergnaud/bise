// src/app/panel · a document in its tab, drawn by its app (docs/plugin-apps.md A5, A6b; designer's
// mocks, page plugin-apps-mocks §3): a head (its name, ⋯) with one facts line under it (the app,
// the version, the save state, or the one thing that changes what he can do: docline.ts), then
// the view, kept live by the views layer (the frame is placed over this tab's box, never moved:
// src/views). A built-in page's document (view/open's {frame}) draws today's page in the same
// place. One tab per document (its key: views/port.ts docKey): Open with swaps the app in this tab
// (store.reopen), an older version's 'Open v5' and a conflict's Reload open the newest in it.
// Mounted by Thing.tsx for an artifact whose row has an opener and for a ~/bise file. Owner:
// apps-host.
import { useEffect, useMemo, useRef, type ReactNode } from 'react';
import { AppIcon, IconButton, Press } from '@/ds';
import { ds } from '@/proto/words_ds.gen';
import { views } from '@/proto/words_views.gen';
import { ArtifactOpen } from '@/artifacts';
import { timeOf } from '@/artifacts/model';
import { useEntityMenu } from '@/app/menus/use';
import { useViewState } from '@/views/hooks';
import { docKey, type DocRef } from '@/views/port';
import { StateLine } from '@/views/StateLine';
import type { ViewState, ViewsStore } from '@/views/store';
import type { SavedBy } from '@/proto/rows';
import { docEntity } from '@/views/window';
import { closeTabId } from './state';
import { docLine, type DocLine, type RowFacts } from './docline';

/** A document tab's words for its line's facts: 'Sheets · v3 · saved 09:41'. Pure. */
export function factsWords(l: Extract<DocLine, { kind: 'facts' }>): string {
  const parts = [l.app];
  if (l.version !== undefined) parts.push(views.doc_version({ n: l.version }));
  const s = l.save;
  if (l.readOnly) parts.push(views.doc_read_only);
  else if (s.kind === 'saving') parts.push(views.doc_saving);
  else if (s.kind === 'saved') parts.push(views.doc_saved({ time: timeOf(s.at) }));
  else if (s.kind === 'too-large') parts.push(views.doc_too_large({ max: `${String(Math.round(s.maxBytes / (1024 * 1024)))} MB` }));
  else if (s.kind === 'failed') parts.push(views.doc_save_failed);
  return parts.filter((p) => p !== '').join(' · ');
}

/** The lines that say why the document isn't there: the body under them stays empty. */
const SAID: ReadonlySet<DocLine['kind']> = new Set(['gone', 'off', 'failed']);

/** Who saved a version, in words: 'You', the agent's name, else 'someone'. Pure. */
export function whoWord(by: SavedBy): string {
  if (by.kind === 'you') return views.doc_you;
  return by.kind === 'agent' ? by.name : views.doc_someone;
}

/**
 * The newer version a tab goes to by itself, or null: a tab that follows the newest, live on an
 * older one it can't edit (nothing of his to lose). Pure.
 */
export function newerAct(state: ViewState, row: RowFacts | undefined, follows: boolean): number | null {
  if (!follows || row === undefined || state.kind !== 'live' || state.doc === undefined) return null;
  const d = state.doc.document;
  return d.version !== undefined && d.version < row.newest && !d.writable ? row.newest : null;
}

export interface DocumentProps {
  readonly docRef: DocRef;
  readonly tabId: string;
  readonly store: ViewsStore;
  /** its name before the view says it (the artifact's title) */
  readonly title: string;
  /** the app the row names, for the lines said before the view runs ('Sheets is off') */
  readonly rowApp: string;
  /** its artifact's row facts (none: a ~/bise file) */
  readonly row?: RowFacts;
  /** turns its app's plugin on (the app-off line's act) */
  readonly turnOn?: () => void;
}

export function DocumentTab({ docRef, tabId, store, title, rowApp, row, turnOn }: DocumentProps) {
  // a fresh object each render from its parts: its key is its identity
  const key = docKey(docRef);
  const ref = useMemo(() => docRef, [key]); // eslint-disable-line react-hooks/exhaustive-deps
  const state = useViewState(store, ref);
  const box = useRef<HTMLDivElement>(null);
  const menu = useEntityMenu(docEntity(ref));
  // a removed file asks the hub nothing: its line says so
  const gone = row?.gone === true;
  useEffect(() => {
    if (!gone) store.ensure(ref);
  }, [store, ref, gone]);
  useEffect(() => {
    store.anchor(ref, 'tab', box.current);
    return () => {
      store.anchor(ref, 'tab', null);
    };
  }, [store, ref]);
  const follows = store.follows(ref);
  const line = docLine(state, row, rowApp, follows);
  // a tab on the newest he can't edit goes to a newer version by itself, its view's kept state
  // with it (apps-office m_60432: the view gets its place back from ai.mistral.vibe/viewState)
  const auto = newerAct(state, row, follows);
  useEffect(() => {
    if (auto !== null) void store.reopen(ref, { version: null });
  }, [store, ref, auto]);
  const name = state.kind === 'live' && state.doc !== undefined ? state.doc.document.name : title;
  const t = ref.target;
  return (
    <div className="pn-doc" data-testid="app.panel.doc" data-doc={key}>
      <div className="pn-doc-head">
        <span className="pn-doc-name" data-testid="app.panel.doc.name">
          {name === '' ? views.doc_tab : name}
        </span>
        <IconButton label={ds.more_actions} size="bar" onClick={menu.open} testId="app.panel.doc.more">
          <AppIcon name="more" size={16} />
        </IconButton>
      </div>
      <Line
        line={line}
        store={store}
        docRef={ref}
        {...(turnOn === undefined
          ? {}
          : {
              turnOn: () => {
                turnOn();
                void store.reopen(ref);
              },
            })}
      />
      {state.kind === 'live' && state.doc !== undefined && state.doc.detached.length > 0 && (
        <div className="pn-doc-detached" data-testid="app.panel.doc.detached">
          <span className="pn-doc-line-t">{views.doc_detached({ n: state.doc.detached.length })}</span>
          <ul>
            {state.doc.detached.map((n) => (
              <li key={n.id}>{n.quote === undefined ? n.text : `“${n.quote}” · ${n.text}`}</li>
            ))}
          </ul>
        </div>
      )}
      {line.kind === 'page' ? (
        <div className="pn-page">
          {t.kind === 'artifact' ? (
            <ArtifactOpen
              project={ref.project}
              id={t.id}
              bare
              {...(t.version === undefined ? {} : { version: t.version })}
              composer={false}
              onClose={() => {
                closeTabId(tabId);
              }}
            />
          ) : (
            state.kind === 'frame' && <iframe className="art-paper" src={state.url} title={name} />
          )}
        </div>
      ) : (
        <div className="pn-doc-view">
          <div ref={box} className="vw-tab-box" data-testid="views.tab.anchor" data-state={state.kind} data-phase={state.kind === 'live' ? state.phase : undefined} />
          {/* the head's line says why it isn't there (gone, off, failed): the body adds nothing */}
          {!SAID.has(line.kind) && (state.kind !== 'live' || state.phase !== 'live') && <StateLine store={store} r={ref} state={state} />}
        </div>
      )}
    </div>
  );
}

/** The head's one line: its facts, or the one thing to do. */
function Line({ line, store, docRef, turnOn }: { line: DocLine; store: ViewsStore; docRef: DocRef; turnOn?: () => void }) {
  const act = (label: string, testId: string, on: () => void) => (
    <Press look="pn-doc-act" onClick={on} data-testid={testId}>
      {label}
    </Press>
  );
  const row = (testId: string, children: ReactNode) => (
    <div className="pn-doc-line" data-testid="app.panel.doc.line" data-line={testId}>
      {children}
    </div>
  );
  switch (line.kind) {
    case 'loading':
    case 'page':
      return null;
    case 'gone':
      return row('gone', <span className="pn-doc-line-t">{views.doc_gone}</span>);
    case 'off':
      return row(
        'off',
        <>
          <span className="pn-doc-line-t">{views.doc_app_off({ app: line.app })}</span>
          {turnOn !== undefined && act(views.doc_turn_on, 'app.panel.doc.turn-on', turnOn)}
        </>,
      );
    case 'failed':
      return row(
        'failed',
        <>
          <span className="pn-doc-line-t">
            <span className="pn-doc-err" aria-hidden="true">
              ✗
            </span>{' '}
            {views.doc_failed}
          </span>
          {act(views.doc_try_again, 'app.panel.doc.retry', () => {
            store.retry(docRef);
          })}
        </>,
      );
    case 'conflict':
      return row(
        'conflict',
        <>
          <span className="pn-doc-line-t">{[line.app, ...(line.version === undefined ? [] : [views.doc_version({ n: line.version })]), views.doc_newer].join(' · ')}</span>
          {act(views.doc_reload, 'app.panel.doc.reload', () => {
            void store.reopen(docRef, { version: null });
          })}
          {act(views.doc_keep_copy, 'app.panel.doc.keep-copy', () => {
            void store.keepCopy(docRef);
          })}
        </>,
      );
    case 'newer':
      return row(
        'newer',
        <>
          <span className="pn-doc-line-t">{[line.app, views.doc_version({ n: line.version }), views.doc_newer_by({ who: whoWord(line.by), n: line.newest })].join(' · ')}</span>
          {act(views.doc_reload, 'app.panel.doc.reload', () => {
            void store.reopen(docRef, { version: null });
          })}
        </>,
      );
    case 'older':
      return row(
        'older',
        <>
          <span className="pn-doc-line-t">{[line.app, views.doc_version({ n: line.version }), views.doc_older].join(' · ')}</span>
          {act(views.doc_open_newest({ n: line.newest }), 'app.panel.doc.open-newest', () => {
            void store.reopen(docRef, { version: null });
          })}
        </>,
      );
    case 'facts':
      return row('facts', <span className="pn-doc-line-t">{factsWords(line)}</span>);
  }
}
