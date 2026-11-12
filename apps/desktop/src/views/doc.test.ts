// A document's view, the pure parts (docs/plugin-apps.md A4, A5): view/open's answer read, the
// selection, the notes a view gets, the state it may keep, a write's answer as the save line.
import { describe, expect, it } from 'vitest';
import { bytesOf, copyBody, detachedOf, docOpenOf, keptState, MAX_ANCHOR, MAX_VIEW_STATE, notesOf, saveOf, selectionOf } from './doc';
import type { Json } from './protocol';
import { DOC_ERR } from './port';

const ORIGIN = 'http://v-0123456789abcdef.localhost:47123';
const DOC = 'a'.repeat(32);
const HANDLE = `view-file://${'b'.repeat(32)}`;

export function opened(over: Record<string, Json> = {}): Json {
  return {
    origin: ORIGIN,
    proxy: `${ORIGIN}/mcp-app?s=${DOC}`,
    html: '<p>fake</p>',
    csp: {},
    permissions: [],
    declared: [],
    prefersBorder: false,
    toolInput: { file: { name: 'q3.fake', resourceUri: HANDLE } },
    toolResult: { content: [] },
    tool: { name: 'open_fake' },
    kind: 'view',
    doc: DOC,
    handle: HANDLE,
    document: { name: 'q3.fake', type: 'Fake document', version: 3, size: 12, writable: true },
    opener: { plugin: 'fake-opener', tool: 'open_fake', app: 'Fake' },
    ...over,
  };
}

describe('view/open', () => {
  it('reads a view with its session, handle, document and app', () => {
    const o = docOpenOf(opened());
    expect(o?.kind).toBe('view');
    if (o?.kind !== 'view') return;
    expect(o.doc).toBe(DOC);
    expect(o.document).toEqual({ name: 'q3.fake', type: 'Fake document', version: 3, size: 12, writable: true });
    expect(o.opener.app).toBe('Fake');
    expect(o.res.proxy).toBe(`${ORIGIN}/mcp-app?s=${DOC}`);
  });

  it('a built-in page is its frame, on the pages server only', () => {
    expect(docOpenOf({ kind: 'frame', frame: 'http://127.0.0.1:47123/a/x/view' })).toEqual({ kind: 'frame', url: 'http://127.0.0.1:47123/a/x/view' });
    expect(docOpenOf({ kind: 'frame', frame: 'https://evil.example/a' })).toBe(null);
    expect(docOpenOf({ kind: 'frame', frame: 'file:///etc/passwd' })).toBe(null);
    expect(docOpenOf(opened({ kind: 'other' })), 'neither a view nor a frame').toBe(null);
  });

  it('refuses an answer with a field off: no doc, a made-up handle, a path, no document', () => {
    expect(docOpenOf(opened({ doc: 'short' }))).toBe(null);
    expect(docOpenOf(opened({ handle: '/Users/x/q3.fake' }))).toBe(null);
    expect(docOpenOf(opened({ handle: 'view-file://nothex' }))).toBe(null);
    expect(docOpenOf(opened({ document: null }))).toBe(null);
    expect(docOpenOf(opened({ opener: { plugin: 'p' } }))).toBe(null);
  });
});

describe('selection', () => {
  it('a typed key or his click on the pill is his ask; a move is not', () => {
    expect(selectionOf({ anchor: 'r:1', text: 'B2', key: 'w' })).toEqual({ anchor: 'r:1', text: 'B2', key: 'w' });
    expect(selectionOf({ anchor: 'r:1', take: true })).toEqual({ anchor: 'r:1', take: true });
    expect(selectionOf({ anchor: 'r:1' })).toEqual({ anchor: 'r:1' });
    expect(selectionOf({ anchor: 'r:1', key: 'w', take: true }), 'never both').toEqual({ anchor: 'r:1', key: 'w' });
    expect(selectionOf({ anchor: 'b1', take: true, label: ' page 2 · a box ' }), 'its label, trimmed').toEqual({ anchor: 'b1', take: true, label: 'page 2 · a box' });
  });

  it('drops a key that is not one printable character, and an anchor past 512 bytes', () => {
    expect(selectionOf({ anchor: 'r', key: 'ab' })).toEqual({ anchor: 'r' });
    expect(selectionOf({ anchor: 'r', key: '\n' })).toEqual({ anchor: 'r' });
    expect(selectionOf({ anchor: 'é'.repeat(MAX_ANCHOR / 2 + 1) })).toBe(null);
    expect(selectionOf({ anchor: null, key: 'w' }), 'nothing selected: no ask').toEqual({ anchor: null });
  });
});

describe('notes', () => {
  it("a view gets its own anchor strings, every version's notes, a bad one dropped", () => {
    const got = notesOf({
      notes: [
        { id: 'n1', anchor: { kind: 'view', key: 'r:1' }, text: 'why?', status: 'sent', version: 2 },
        { id: 'n2', anchor: 'r:2', text: 'ok', quote: 'B2', status: 'done', version: 1, done_in: 3 },
        { id: 'n3', anchor: { kind: 'cell', sheet: 's', range: 'A1' }, text: 'x', status: 'sent', version: 1 },
        { id: 'n4', anchor: 'r', text: 'x', status: 'lost', version: 1 },
      ],
    });
    expect(got).toEqual([
      { id: 'n1', anchor: 'r:1', text: 'why?', status: 'sent', version: 2 },
      { id: 'n2', anchor: 'r:2', text: 'ok', quote: 'B2', status: 'done', version: 1, done_in: 3 },
    ]);
  });

  it('detached: only ids of notes the view was given', () => {
    const given = notesOf({ notes: [{ id: 'n1', anchor: 'a', text: 't', status: 'sent', version: 1 }] });
    expect(detachedOf({ ids: ['n1', 'n9', 'n1', 3] }, given)).toEqual(['n1']);
    expect(detachedOf({}, given)).toEqual([]);
  });
});

describe('view state', () => {
  it('keeps 64 KiB of JSON at most', () => {
    const at = (n: number) => 'x'.repeat(n - 2);
    expect(bytesOf(JSON.stringify(at(MAX_VIEW_STATE)))).toBe(MAX_VIEW_STATE);
    expect(keptState(at(MAX_VIEW_STATE))).toBe(at(MAX_VIEW_STATE));
    expect(keptState(at(MAX_VIEW_STATE + 1))).toBe(null);
    expect(keptState(undefined)).toBe(null);
  });
});

describe('save', () => {
  it("a write's answer is the head's save state", () => {
    expect(saveOf({ result: { outcome: 'saved', etag: 'e2' } }, 5, DOC_ERR.noWrite)).toEqual({ kind: 'saved', at: 5 });
    expect(saveOf({ result: { outcome: 'conflict', etag: 'e3' } }, 5, DOC_ERR.noWrite)).toEqual({ kind: 'conflict' });
    expect(saveOf({ result: { outcome: 'too-large', maxBytes: 100 } }, 5, DOC_ERR.noWrite)).toEqual({ kind: 'too-large', maxBytes: 100 });
    expect(saveOf({ error: { code: DOC_ERR.noWrite, message: 'no', data: { reason: 'older_version' } } }, 5, DOC_ERR.noWrite)).toEqual({ kind: 'refused', why: 'older_version' });
    expect(saveOf({ error: { code: DOC_ERR.noWrite, message: 'no', data: { reason: 'no_grant' } } }, 5, DOC_ERR.noWrite)).toEqual({ kind: 'refused', why: 'no_grant' });
    expect(saveOf({ error: { code: DOC_ERR.badHandle, message: 'bad' } }, 5, DOC_ERR.noWrite)).toEqual({ kind: 'failed', message: 'bad' });
  });

  it("a copy takes the refused write's body only (never its uri: the session names the file)", () => {
    expect(copyBody({ uri: HANDLE, ifMatch: 'e1', seq: 4, text: 'mine' })).toEqual({ text: 'mine' });
    expect(copyBody({ uri: HANDLE, blob: 'AAA=' })).toEqual({ blob: 'AAA=' });
    expect(copyBody({ uri: HANDLE })).toBe(null);
  });
});
