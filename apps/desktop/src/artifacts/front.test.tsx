// @vitest-environment happy-dom
// ⌘⏎ (artifact-shortcut, side-ux m_45436): the artifact in front's head's first button, nothing
// else; the inbox's ⌘⏎ and a field's keep theirs.
import { afterEach, describe, expect, it } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Composer } from '@/composer/Composer';
import type { ComposerActions } from '@/composer/actions';
import type { Artifact } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { RouterContext, createRouter } from '@/app/router';
import { emitAction } from '@/app/menu';
import { ArtifactOpen } from './index';
import { frontOf, primaryKeyFires, PRIMARY_KEY, usePrimaryKey } from './front';
import { frame } from '@/proto/words_frame.gen';
import { panel } from '@/proto/words_panel.gen';
import { rowKey } from './List';

afterEach(() => {
  cleanup();
  delete (globalThis as { bise?: unknown }).bise;
});

const art = (id: string, kind: string, extra: Partial<Artifact> = {}): Artifact => ({ id, kind, title: id, agent: 'perf', version: 1, at_ms: 0, url: `http://127.0.0.1:47123/p/${id}`, new: false, ...extra });

const key = { key: 'Enter', metaKey: true, altKey: false, ctrlKey: false, shiftKey: false, repeat: false, defaultPrevented: false };

describe('which artifact is in front (frontOf)', () => {
  const f = (place: 'view' | 'panel' | 'row', focus = false, hover = false) => ({ place, focus, hover });
  it('the focused one, else the hovered one, else the view before the panel; a row only under the pointer', () => {
    const view = f('view');
    const panel = f('panel');
    expect(frontOf([panel, view])).toBe(view);
    const focusedPanel = f('panel', true);
    expect(frontOf([view, focusedPanel])).toBe(focusedPanel);
    const hoveredPanel = f('panel', false, true);
    expect(frontOf([view, hoveredPanel])).toBe(hoveredPanel);
    const row = f('row');
    expect(frontOf([row])).toBeUndefined();
    const hoveredRow = f('row', false, true);
    expect(frontOf([view, hoveredRow])).toBe(hoveredRow);
    expect(frontOf<ReturnType<typeof f>>([])).toBeUndefined();
  });
});

describe('when ⌘⏎ is the artifact\'s (primaryKeyFires)', () => {
  const none = { field: false, elsewhere: false, inbox: false };
  it('⌘⏎ alone, untaken, out of any field, the inbox not in front', () => {
    expect(primaryKeyFires(key, none)).toBe(true);
    expect(primaryKeyFires({ ...key, shiftKey: true }, none)).toBe(false);
    expect(primaryKeyFires({ ...key, metaKey: false }, none)).toBe(false);
    expect(primaryKeyFires({ ...key, repeat: true }, none)).toBe(false);
    expect(primaryKeyFires({ ...key, defaultPrevented: true }, none)).toBe(false);
    expect(primaryKeyFires(key, { ...none, field: true })).toBe(false);
    expect(primaryKeyFires(key, { ...none, elsewhere: true })).toBe(false);
    expect(primaryKeyFires(key, { ...none, inbox: true })).toBe(false);
  });
});

function Shell({ inbox }: { inbox: () => boolean }) {
  usePrimaryKey(inbox);
  return null;
}

function mount(id: string, items: Artifact[], inbox = () => false) {
  const store = new Store(null);
  store.apply({ ev: 'projects', projects: [{ project: 'shop-1', name: 'storefront', path: '/x/shop' }] });
  store.apply({ ev: 'artifacts', project: 'shop-1', items });
  render(
    <StoreProvider store={store}>
      <RouterContext.Provider value={createRouter({ screen: 'artifacts', project: 'shop-1', id })}>
        <Shell inbox={inbox} />
        <ArtifactOpen project="shop-1" id={id} composer={false} />
        <input data-testid="field" />
        <div data-testid="composer" contentEditable suppressContentEditableWarning />
        <button type="button" data-testid="elsewhere">a list's row</button>
      </RouterContext.Provider>
    </StoreProvider>,
  );
}

/** The bridge, recording what the window was asked. */
function fakeBise() {
  const calls: string[] = [];
  (globalThis as { bise?: unknown }).bise = {
    openExternal: (u: string) => { calls.push(`open ${u}`); return Promise.resolve(true); },
    saveCopy: (p: string, n: string) => { calls.push(`save ${p} ${n}`); return Promise.resolve({ to: `/h/Downloads/${n}` }); },
  };
  return calls;
}

// keyCode 13 as Chromium sends it: ProseMirror prevents every ⏎ by its keyCode (captureKeyDown)
const cmdEnter = (target: Element | Window = window) => { fireEvent.keyDown(target, { key: 'Enter', keyCode: 13, metaKey: true }); };

describe('the law: ⌘⏎ runs exactly the head\'s first button', () => {
  const page = art('notes', 'page', { version: 2 });
  const file = art('bench', 'file', { url: '/x/shop/bench.csv', path: '/x/shop/bench.csv' });

  it("a page: '↗ browser', the same call as its click; its tip says the key", () => {
    const calls = fakeBise();
    mount('notes', [page]);
    const first = screen.getByTestId('artifact.head').querySelector('.art-open-end button:nth-of-type(2)');
    expect(first?.getAttribute('data-testid')).toBe('artifact.browser');
    expect(screen.getByTestId('artifact.browser').getAttribute('title')).toBe(frame.tip_key({ what: panel.addr_out, key: PRIMARY_KEY }));
    fireEvent.click(screen.getByTestId('artifact.browser'));
    const byClick = [...calls];
    calls.length = 0;
    cmdEnter();
    expect(byClick).toEqual(['open http://127.0.0.1:47123/p/notes']);
    expect(calls).toEqual(byClick);
  });

  it("a file with no page: '⤓ save a copy…', the same call as its click; ⌘S stays", () => {
    const calls = fakeBise();
    mount('bench', [file]);
    expect(screen.queryByTestId('artifact.browser')).toBeNull();
    expect(screen.getByTestId('artifact.save').getAttribute('data-tip-keys')).toBe(PRIMARY_KEY);
    fireEvent.click(screen.getByTestId('artifact.save'));
    const byClick = [...calls];
    calls.length = 0;
    cmdEnter();
    expect(byClick).toEqual(['save /x/shop/bench.csv bench.csv']);
    expect(calls).toEqual(byClick);
    calls.length = 0;
    emitAction('save');
    expect(calls).toEqual(byClick);
  });

  it('the inbox in front keeps its ⌘⏎; a field with the caret, or a screen that took the key, too', () => {
    const calls = fakeBise();
    let inbox = true;
    mount('notes', [page], () => inbox);
    cmdEnter();
    expect(calls).toEqual([]);
    inbox = false;
    const field = screen.getByTestId('field');
    field.focus();
    cmdEnter(field);
    expect(calls).toEqual([]);
    field.blur();
    // the composer (an editable), empty, with the caret: its ⌘⏎, never the artifact's
    const composer = screen.getByTestId('composer');
    composer.focus();
    expect(document.activeElement).toBe(composer);
    cmdEnter(composer);
    expect(calls).toEqual([]);
    composer.blur();
    // the focus on something that is no artifact (a list, a card): its ⌘⏎
    const other = screen.getByTestId('elsewhere');
    other.focus();
    cmdEnter(other);
    expect(calls).toEqual([]);
    other.blur();
    // a screen's own ⌘⏎ (the palette, a library list) took it first
    const taken = (e: KeyboardEvent) => { e.preventDefault(); };
    window.addEventListener('keydown', taken, true);
    cmdEnter();
    window.removeEventListener('keydown', taken, true);
    expect(calls).toEqual([]);
    cmdEnter();
    expect(calls).toEqual(['open http://127.0.0.1:47123/p/notes']);
    // the focus inside the artifact (its head's ⋯): still its
    screen.getByTestId('artifact.more').focus();
    cmdEnter(screen.getByTestId('artifact.more'));
    expect(calls).toHaveLength(2);
  });

  it("the library's selected row: its list's ⌘⏎ is the row's head's first button (rowKey)", () => {
    const calls = fakeBise();
    const ev = (k: string) => ({ key: k, metaKey: true, altKey: false, ctrlKey: false, shiftKey: false }) as never;
    const ports = { onOpen: () => undefined };
    expect(rowKey(ev('Enter'), { project: 'shop-1', projectName: 'storefront', a: page }, ports)).toBe(true);
    expect(rowKey(ev('Enter'), { project: 'shop-1', projectName: 'storefront', a: file }, ports)).toBe(true);
    expect(calls).toEqual(['open http://127.0.0.1:47123/p/notes', 'save /x/shop/bench.csv bench.csv']);
  });

  it("the composer beside it, empty, the caret in it: ⌘⏎ is the artifact's; with words it sends them (his m_54852)", async () => {
    const calls = fakeBise();
    const sent: unknown[] = [];
    const got: { a: ComposerActions | null } = { a: null };
    const store = new Store(null);
    store.apply({ ev: 'projects', projects: [{ project: 'shop-1', name: 'storefront', path: '/x/shop' }] });
    store.apply({ ev: 'artifacts', project: 'shop-1', items: [page] });
    const mem = new Map<string, string>();
    render(
      <StoreProvider store={store}>
        <RouterContext.Provider value={createRouter({ screen: 'thread', project: 'shop-1', agent: 'perf' })}>
          <Shell inbox={() => false} />
          <ArtifactOpen project="shop-1" id="notes" bare composer={false} />
          <Composer target="shop-1/perf" store={{ getItem: (k) => mem.get(k) ?? null, setItem: (k, v) => void mem.set(k, v), removeItem: (k) => void mem.delete(k) }} onSend={(o) => sent.push(o)} onReady={(a) => (got.a = a)} />
        </RouterContext.Provider>
      </StoreProvider>,
    );
    await act(async () => {
      await Promise.resolve();
    });
    const field = screen.getByTestId('composer.field');
    field.focus();
    expect(field.contains(document.activeElement)).toBe(true);
    cmdEnter(field);
    expect(calls).toEqual(['open http://127.0.0.1:47123/p/notes']);
    expect(sent).toEqual([]);
    await act(async () => {
      got.a?.insertText('ship it');
      await Promise.resolve();
    });
    cmdEnter(field);
    expect(calls).toHaveLength(1);
    expect(sent).toHaveLength(1);
  });

  it("File › Open in Browser runs it; the key's own menu echo is dropped", () => {
    const calls = fakeBise();
    mount('notes', [page]);
    emitAction('browser');
    expect(calls).toEqual(['open http://127.0.0.1:47123/p/notes']);
    cmdEnter();
    emitAction('browser');
    expect(calls).toHaveLength(2);
  });
});
