// @vitest-environment happy-dom
// ⌘F in an artifact (artifact-find, his m_52682): the artifact with the focus (or the pointer, the
// focus on nothing) takes ⌘F and Edit › Find; anywhere else the screen's own search keeps it.
import { afterEach, describe, expect, it } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { Artifact } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { RouterContext, createRouter } from '@/app/router';
import { emitAction, useMenuAction } from '@/app/menu';
import { ArtifactOpen } from './index';
import '@/app/escape';
import { counterOf, findable, findTarget, stepAt, useArtifactFindKey } from './find';

afterEach(() => {
  cleanup();
  delete (globalThis as { bise?: unknown }).bise;
});

describe('who takes ⌘F (findTarget)', () => {
  const f = (focus = false, hover = false) => ({ focus, hover });
  it('the focused artifact; else, the focus on nothing, the hovered one; else none (the screen)', () => {
    const focused = f(true);
    const hovered = f(false, true);
    expect(findTarget([hovered, focused], false)).toBe(focused);
    expect(findTarget([hovered, focused], true)).toBe(focused);
    expect(findTarget([f(), hovered], false)).toBe(hovered);
    // the caret in a field, the composer, a list: the screen's, even with the pointer on the page
    expect(findTarget([hovered], true)).toBeUndefined();
    expect(findTarget([f()], false)).toBeUndefined();
  });
});

describe('the bar', () => {
  it('⏎ and ⇧⏎ go around the ends; no match: none', () => {
    expect(stepAt(3, 0, false)).toBe(1);
    expect(stepAt(3, 2, false)).toBe(0);
    expect(stepAt(3, 0, true)).toBe(2);
    expect(stepAt(3, -1, false)).toBe(0);
    expect(stepAt(3, -1, true)).toBe(2);
    expect(stepAt(0, 0, false)).toBe(-1);
  });
  it("the count: 'n/m', '0/0' with no match, nothing before a needle", () => {
    expect(counterOf('', 0, -1)).toBe('');
    expect(counterOf('kiwi', 0, -1)).toBe('0/0');
    expect(counterOf('kiwi', 12, 2)).toBe('3/12');
  });
  it('pages, docs and text have ⌘F; an image, a pdf, a video do not', () => {
    expect(findable({ kind: 'page', url: 'http://127.0.0.1:1/p/x' })).toBe(true);
    expect(findable({ kind: 'file', path: '/w/notes.md' })).toBe(true);
    expect(findable({ kind: 'file', path: '/w/main.rs' })).toBe(true);
    expect(findable({ kind: 'file', path: '/w/q3.docx' })).toBe(true);
    expect(findable({ kind: 'file', path: '/w/deck.pdf' })).toBe(false);
    expect(findable({ kind: 'file', path: '/w/a.png' })).toBe(false);
    expect(findable({ kind: 'file', path: '/w/clip.mp4' })).toBe(false);
    expect(findable({ kind: 'shot', path: '/w/s' })).toBe(false);
  });
});

const art = (id: string, kind: string, extra: Partial<Artifact> = {}): Artifact => ({ id, kind, title: id, agent: 'perf', version: 1, at_ms: 0, url: `http://127.0.0.1:47123/p/${id}`, new: false, ...extra });

/** The window's shell (its ⌘F key) and a screen with its own search, as the thread's (useMenuAction('find')). */
function Shell({ onScreenFind }: { onScreenFind: () => void }) {
  useArtifactFindKey();
  useMenuAction('find', onScreenFind);
  return null;
}

function mount(items: Artifact[], id: string) {
  const heard: string[] = [];
  const store = new Store(null);
  store.apply({ ev: 'projects', projects: [{ project: 'shop-1', name: 'storefront', path: '/x/shop' }] });
  store.apply({ ev: 'artifacts', project: 'shop-1', items });
  render(
    <StoreProvider store={store}>
      <RouterContext.Provider value={createRouter({ screen: 'artifacts', project: 'shop-1', id })}>
        <Shell onScreenFind={() => heard.push('screen')} />
        <ArtifactOpen project="shop-1" id={id} composer={false} />
        <input data-testid="composer" />
      </RouterContext.Provider>
    </StoreProvider>,
  );
  return heard;
}

describe('the law: ⌘F with the focus in an artifact finds in it; elsewhere the screen keeps it', () => {
  it('Edit › Find (which the menu sends even with the focus inside the frame) goes to the page', () => {
    const heard = mount([art('notes', 'page')], 'notes');
    const page = screen.getByTestId('artifact.page');
    page.focus();
    act(() => {
      emitAction('find');
    });
    expect(screen.getByTestId('artifact.find')).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByTestId('artifact.find.field'));
    expect(heard).toEqual([]);
  });

  it('the window key ⌘F with the focus in the artifact: its bar, the screen never hears it', () => {
    const heard = mount([art('notes', 'page')], 'notes');
    const close = screen.getByTestId('artifact.close');
    close.focus();
    fireEvent.keyDown(close, { key: 'f', metaKey: true });
    expect(screen.getByTestId('artifact.find')).toBeTruthy();
    expect(heard).toEqual([]);
  });

  it('the caret in the composer: the screen\'s search, no bar', () => {
    const heard = mount([art('notes', 'page')], 'notes');
    screen.getByTestId('composer').focus();
    act(() => {
      emitAction('find');
    });
    expect(screen.queryByTestId('artifact.find')).toBeNull();
    expect(heard).toEqual(['screen']);
  });

  it('an image: no find of its own, the screen keeps ⌘F', () => {
    const heard = mount([art('shot', 'file', { path: '/x/a.png', frame_url: 'http://127.0.0.1:47123/a/shot/view' })], 'shot');
    screen.getByTestId('artifact.close').focus();
    act(() => {
      emitAction('find');
    });
    expect(screen.queryByTestId('artifact.find')).toBeNull();
    expect(heard).toEqual(['screen']);
  });

  it('esc in the field closes the bar, the artifact stays', () => {
    mount([art('notes', 'page')], 'notes');
    screen.getByTestId('artifact.page').focus();
    act(() => {
      emitAction('find');
    });
    fireEvent.keyDown(screen.getByTestId('artifact.find.field'), { key: 'Escape' });
    expect(screen.queryByTestId('artifact.find')).toBeNull();
    expect(screen.getByTestId('artifact.open')).toBeTruthy();
  });

  it('esc with the focus in the page (the window gets it, the frame its active element): the bar closes, the artifact stays (ui-review m_52779)', () => {
    mount([art('notes', 'page')], 'notes');
    const page = screen.getByTestId('artifact.page');
    page.focus();
    act(() => {
      emitAction('find');
    });
    page.focus();
    expect(document.activeElement).toBe(page);
    fireEvent.keyDown(page, { key: 'Escape' });
    expect(screen.queryByTestId('artifact.find')).toBeNull();
    expect(screen.getByTestId('artifact.open')).toBeTruthy();
  });
});
