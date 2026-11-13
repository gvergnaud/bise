// @vitest-environment happy-dom
// The folders macOS guards under the projects list (Guarded.tsx, setup-scan-tcc): a button per folder
// the scan skipped, his click is the only scan of it; then its answer (designer m_64068).
import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import type { AppCmd, Looked } from '@/proto';
import { settings } from '@/proto/words.gen';
import { Guarded, guardedRows, type Folders } from './Guarded';

const H = '/Users/camille';
const docs = `${H}/Documents`;
const desk = `${H}/Desktop`;

describe('guardedRows', () => {
  it('a skipped folder is a button, waits once clicked, then says its answer; one that found repos goes', () => {
    const f: Folders = { guarded: [docs, desk], looked: [] };
    expect(guardedRows(f, new Map()).map((r) => [r.kind, r.name])).toEqual([['look', 'Documents'], ['look', 'Desktop']]);
    expect(guardedRows(f, new Map([[docs, null]])).map((r) => r.kind)).toEqual(['looking', 'look']);
    const found: Looked = { path: docs, n: 2, refused: false };
    const no: Looked = { path: desk, n: 0, refused: true };
    const after: Folders = { guarded: [`${H}/Downloads`], looked: [found, no] };
    expect(guardedRows(after, new Map([[docs, null], [desk, null]])).map((r) => [r.kind, r.name])).toEqual([['look', 'Downloads'], ['refused', 'Desktop']]);
    expect(guardedRows({ guarded: [], looked: [{ path: docs, n: 0, refused: false }] }, new Map()).map((r) => r.kind)).toEqual(['none']);
    // looked again after a refusal: waits until a newer answer than the one he saw
    expect(guardedRows({ guarded: [], looked: [no] }, new Map([[desk, no]])).map((r) => r.kind)).toEqual(['looking']);
  });
});

describe('Guarded', () => {
  it("his click on 'Look in Documents' is the only scan of it; macOS's no offers System Settings", () => {
    const sent: AppCmd[] = [];
    let opened = 0;
    const ui = (folders: Folders) => (
      <Guarded folders={folders} send={(c) => sent.push(c)} openSettings={() => { opened += 1; }} testId="g" />
    );
    const { rerender, container } = render(ui({ guarded: [docs, desk], looked: [] }));
    expect(screen.getByTestId('g').textContent).toContain(settings.onb_guarded);
    expect(sent).toEqual([]);
    fireEvent.click(screen.getByTestId('g.Documents'));
    expect(sent).toEqual([{ cmd: 'found_scan', dir: docs }]);
    expect(screen.getByTestId('g.Documents').textContent).toBe(settings.onb_looking_in({ folder: 'Documents' }));
    // Documents had repos (they joined the list): its button goes; Desktop: macOS said no
    rerender(ui({ guarded: [desk], looked: [{ path: docs, n: 1, refused: false }] }));
    expect(screen.queryByTestId('g.Documents')).toBeNull();
    fireEvent.click(screen.getByTestId('g.Desktop'));
    rerender(ui({ guarded: [], looked: [{ path: docs, n: 1, refused: false }, { path: desk, n: 0, refused: true }] }));
    expect(screen.getByTestId('g.Desktop').textContent).toContain(settings.onb_refused_in({ folder: 'Desktop' }));
    fireEvent.click(screen.getByTestId('g.Desktop.settings'));
    expect(opened).toBe(1);
    // nothing skipped, nothing to say: no line
    rerender(ui({ guarded: [], looked: [{ path: docs, n: 3, refused: false }] }));
    expect(container.textContent).toBe('');
  });
});
