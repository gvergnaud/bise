// @vitest-environment happy-dom
// An artifact whose file is gone (ui-review m_52744, designer m_52746): the pages server answers
// 404, the window draws bise's own 'file gone' state, never the server's raw line in a frame.
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { Artifact } from '@/proto';
import { panel } from '@/proto/words_panel.gen';
import { Store, StoreProvider } from '@/store';
import { RouterContext, createRouter } from '@/app/router';
import { ArtifactOpen } from './index';

afterEach(() => {
  cleanup();
  delete (globalThis as { bise?: unknown }).bise;
});

const SRV = 'http://127.0.0.1:47123';
const notes = (version: number): Artifact => ({
  id: 'notes', kind: 'file', title: 'notes.md', agent: 'perf', version, at_ms: 0, new: false,
  url: '/w/notes.md', path: '/w/notes.md', frame_url: `${SRV}/a/notes/view`,
  versions: Array.from({ length: version }, (_, i) => ({ v: i + 1, at_ms: 0, frame_url: `${SRV}/a/notes/${String(i + 1)}/view` })),
});

/** The window's bridge: the pages server answers 404 for `gone` urls, else up. */
function mount(a: Artifact, gone: (url: string) => boolean) {
  const asked: string[] = [];
  (globalThis as { bise?: unknown }).bise = {
    probePage: (u: string) => {
      asked.push(u);
      return Promise.resolve(gone(u) ? 'gone' : 'up');
    },
  };
  const store = new Store(null);
  store.apply({ ev: 'projects', projects: [{ project: 'shop-1', name: 'storefront', path: '/x/shop' }] });
  store.apply({ ev: 'artifacts', project: 'shop-1', items: [a] });
  render(
    <StoreProvider store={store}>
      <RouterContext.Provider value={createRouter({ screen: 'artifacts', project: 'shop-1', id: a.id })}>
        <ArtifactOpen project="shop-1" id={a.id} composer={false} />
      </RouterContext.Provider>
    </StoreProvider>,
  );
  return asked;
}

describe("the 'file gone' state", () => {
  it('the only file gone: its words, never a frame, and ask its agent about it', async () => {
    mount(notes(1), () => true);
    await waitFor(() => screen.getByTestId('artifact.filegone'));
    expect(screen.queryByTestId('artifact.page')).toBeNull();
    expect(screen.getByTestId('artifact.filegone').textContent).toContain(panel.art_file_gone);
    expect(screen.getByTestId('artifact.filegone').textContent).toContain(panel.art_file_gone_why);
    expect(screen.getByTestId('artifact.filegone.ask').textContent).toBe(panel.art_file_gone_ask({ agent: 'perf' }));
    // the head stays as it is
    expect(screen.getByTestId('artifact.title').textContent).toBe('notes.md');
  });

  it("an older version's file gone, a newer one there: 'open v3' shows the newest", async () => {
    mount(notes(3), (u) => u.includes('/a/notes/1/'));
    fireEvent.click(screen.getByTestId('artifact.version'));
    fireEvent.click(screen.getByTestId('artifact.versions.v1'));
    await waitFor(() => screen.getByTestId('artifact.filegone'));
    expect(screen.getByTestId('artifact.filegone').textContent).toContain(panel.art_file_gone_version);
    expect(screen.getByTestId('artifact.filegone.newest').textContent).toBe(panel.art_file_gone_open({ v: 3 }));
    fireEvent.click(screen.getByTestId('artifact.filegone.newest'));
    await waitFor(() => screen.getByTestId('artifact.page'));
    expect(screen.queryByTestId('artifact.filegone')).toBeNull();
    expect(screen.getByTestId('artifact.version').textContent).toContain('v3');
  });

  it('a file still there: the page, no gone state', async () => {
    mount(notes(1), () => false);
    await waitFor(() => screen.getByTestId('artifact.page'));
    expect(screen.queryByTestId('artifact.filegone')).toBeNull();
  });
});
