// A document tab's head line (docs/plugin-apps.md A6b): its facts, or the one thing that changes
// what he can do, in its order (gone, off, failed, loading, a conflict, an older or newer version, read only).
import { describe, expect, it } from 'vitest';
import type { Save } from '@/views/doc';
import { DOC_ERR, VIEW_ERR } from '@/views/port';
import type { ViewState } from '@/views/store';
import { views } from '@/proto/words_views.gen';
import { factsWords, newerAct, whoWord } from './Document';
import { docLine } from './docline';

function live(version: number | undefined, save: Save = { kind: 'idle' }, writable = true): ViewState {
  return {
    kind: 'live',
    phase: 'live',
    note: 'none',
    height: 96,
    mode: 'fullscreen',
    prefersBorder: false,
    ungranted: [],
    tool: 'open_fake',
    plugin: '',
    messageAt: null,
    doc: { document: { name: 'q3.fake', type: 'Fake', size: 1, writable, ...(version === undefined ? {} : { version }) }, opener: { plugin: 'fake-opener', tool: 'open_fake', app: 'Fake' }, save, detached: [] },
  };
}

const ROW = { newest: 3, gone: false };

describe('docLine', () => {
  it('the facts: the app, the version, the save state', () => {
    expect(docLine(live(3), ROW, 'Fake')).toEqual({ kind: 'facts', app: 'Fake', version: 3, save: { kind: 'idle' }, readOnly: false });
    expect(docLine(live(3, { kind: 'saving' }), ROW, 'Fake')).toMatchObject({ kind: 'facts', save: { kind: 'saving' } });
    expect(docLine(live(undefined), undefined, ''), 'a ~/bise file: no version').toEqual({ kind: 'facts', app: 'Fake', save: { kind: 'idle' }, readOnly: false });
  });

  it('a file gone wins over everything, from the row or the hub', () => {
    expect(docLine(live(3, { kind: 'conflict' }), { newest: 3, gone: true }, 'Fake')).toEqual({ kind: 'gone' });
    expect(docLine({ kind: 'failed', message: 'x', code: DOC_ERR.gone }, ROW, 'Fake')).toEqual({ kind: 'gone' });
  });

  it("the app off says the row's app; another failure is plain", () => {
    expect(docLine({ kind: 'failed', message: 'off', code: VIEW_ERR.pluginOff }, ROW, 'Sheets')).toEqual({ kind: 'off', app: 'Sheets' });
    expect(docLine({ kind: 'failed', message: 'x' }, ROW, 'Sheets')).toEqual({ kind: 'failed' });
  });

  it('a conflict before an older version, an older version before read only', () => {
    expect(docLine(live(2, { kind: 'conflict' }, false), ROW, 'Fake')).toEqual({ kind: 'conflict', app: 'Fake', version: 2 });
    expect(docLine(live(2, { kind: 'idle' }, false), ROW, 'Fake', false)).toEqual({ kind: 'older', app: 'Fake', version: 2, newest: 3 });
    expect(docLine(live(3, { kind: 'idle' }, false), ROW, 'Fake')).toMatchObject({ kind: 'facts', readOnly: true });
    expect(docLine(live(3, { kind: 'refused', why: 'no_grant' }), ROW, 'Fake')).toMatchObject({ kind: 'facts', readOnly: true });
  });

  it('a newer version under a tab on the newest: who saved it when he can edit; a version he picked says Open vN (apps-office m_60432)', () => {
    const row = { newest: 4, gone: false, by: { kind: 'agent', name: 'perf' } } as const;
    expect(docLine(live(3), row, 'Fake', true)).toEqual({ kind: 'newer', app: 'Fake', version: 3, newest: 4, by: { kind: 'agent', name: 'perf' } });
    expect(docLine(live(3), { newest: 4, gone: false }, 'Fake', true), 'none said: unknown').toMatchObject({ kind: 'newer', by: { kind: 'unknown' } });
    expect(docLine(live(3), row, 'Fake', false)).toEqual({ kind: 'older', app: 'Fake', version: 3, newest: 4 });
    expect(docLine(live(3, { kind: 'idle' }, false), row, 'Fake', true), 'read only: the tab reopens by itself, its facts meanwhile').toMatchObject({ kind: 'facts', readOnly: true });
    expect(newerAct(live(3, { kind: 'idle' }, false), row, true)).toBe(4);
    expect(newerAct(live(3), row, true), 'he can edit: never by itself').toBeNull();
    expect(newerAct(live(3, { kind: 'idle' }, false), row, false), 'a version he picked stays').toBeNull();
    expect(newerAct(live(4, { kind: 'idle' }, false), row, true)).toBeNull();
  });

  it("who saved it: 'You', the agent, else someone", () => {
    expect(whoWord({ kind: 'you' })).toBe(views.doc_you);
    expect(whoWord({ kind: 'agent', name: 'perf' })).toBe('perf');
    expect(whoWord({ kind: 'unknown' })).toBe(views.doc_someone);
  });

  it('loading until the view says its document; a built-in page draws its own', () => {
    expect(docLine({ kind: 'loading', height: 96 }, ROW, 'Fake')).toEqual({ kind: 'loading' });
    expect(docLine({ kind: 'frame', url: 'http://127.0.0.1:1/a/x/view' }, ROW, 'Fake')).toEqual({ kind: 'page' });
  });
});

describe('factsWords', () => {
  it("joins the app, the version and the save state with ' · '", () => {
    const at = new Date(2026, 10, 11, 9, 41).getTime();
    const w = factsWords({ kind: 'facts', app: 'Sheets', version: 3, save: { kind: 'saved', at }, readOnly: false });
    expect(w.split(' · ')).toHaveLength(3);
    expect(w.startsWith('Sheets · ')).toBe(true);
    expect(w).toContain('09:41');
    expect(factsWords({ kind: 'facts', app: 'Sheets', save: { kind: 'idle' }, readOnly: false })).toBe('Sheets');
  });
});
