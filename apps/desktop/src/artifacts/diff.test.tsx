// @vitest-environment happy-dom
// A .diff artifact in the panel (try-diff-open, his m_52956: « je m'attendais à avoir un diff
// panel »): it asks the hub for that artifact's diff at the version shown, then draws its files and
// hunks with the review's own DiffFiles, + and − lines marked; never a blank frame.
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen, act } from '@testing-library/react';
import type { Artifact, HubCmd } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { RouterContext, createRouter } from '@/app/router';
import { ArtifactOpen } from './index';
import { isDiff } from './model';

afterEach(() => {
  cleanup();
});

const P = 'harness-1a2b3c4d';
const cuDiff = (version: number): Artifact => ({
  id: 'cu-upload-diff', kind: 'code', title: 'cu-upload at dd86c8a', agent: 'main', version, at_ms: 0, new: false,
  url: '/s/features/cu-upload.diff', path: '/s/features/cu-upload.diff',
  versions: Array.from({ length: version }, (_, i) => ({ v: i + 1, at_ms: 0, copy: `v${String(i + 1)}/cu-upload.diff` })),
});

function mount(a: Artifact) {
  const sent: HubCmd[] = [];
  const store = new Store(null);
  const command = store.command.bind(store);
  store.command = (c: HubCmd) => {
    sent.push(c);
    command(c);
  };
  store.apply({ ev: 'projects', projects: [{ project: P, name: 'harness', path: '/x/harness' }] });
  store.apply({ ev: 'artifacts', project: P, items: [a] });
  render(
    <StoreProvider store={store}>
      <RouterContext.Provider value={createRouter({ screen: 'artifacts', project: P, id: a.id })}>
        <ArtifactOpen project={P} id={a.id} composer={false} bare />
      </RouterContext.Provider>
    </StoreProvider>,
  );
  return { store, sent };
}

describe('a .diff artifact', () => {
  it("asks the hub for its diff at the version shown, then draws its files with + and − lines", () => {
    const { store, sent } = mount(cuDiff(2));
    expect(sent).toContainEqual({ cmd: 'diff', project: P, artifact: 'cu-upload-diff', version: 2 });
    expect(screen.queryByTestId('artifact.page')).toBeNull();
    expect(screen.getByTestId('artifact.diff.loading').textContent).toBe('loading cu-upload at dd86c8a…');
    act(() => {
      store.apply({
        ev: 'diff', project: P, agent: '', base: '', artifact: 'cu-upload-diff', version: 2, title: 'cu-upload at dd86c8a',
        files: [{
          path: 'rust/upload.rs', status: 'modified', add: 1, del: 1,
          hunks: [{ header: '@@ -1 +1 @@', lines: [{ kind: 'del', old: 1, text: 'old line' }, { kind: 'add', new: 1, text: 'new line' }, { kind: 'ctx', old: 2, new: 2, text: 'same' }] }],
        }],
      });
    });
    const file = screen.getByTestId('artifact.diff.file.0');
    expect(file.textContent).toContain('rust/upload.rs');
    // the hunk's head, then − old (rm), + new (ad), the context plain: the review's own colours
    const rows = [...file.querySelectorAll('.b-dl')].map((r) => [r.classList.contains('ad') ? '+' : r.classList.contains('rm') ? '-' : ' ', r.textContent.includes('old line') ? 'old' : r.textContent.includes('new line') ? 'new' : r.textContent.includes('same') ? 'same' : 'head']);
    expect(rows).toEqual([[' ', 'head'], ['-', 'old'], ['+', 'new'], [' ', 'same']]);
    // another version's answer never lands on this one
    expect(store.artifactDiff(P, 'cu-upload-diff', 1)).toBeNull();
  });

  it("colors a .ts file's code by its language, its + line still green (diff-syntax, his m_53153)", () => {
    const { store } = mount(cuDiff(1));
    act(() => {
      store.apply({
        ev: 'diff', project: P, agent: '', base: '', artifact: 'cu-upload-diff', version: 1, title: 'cu-upload at dd86c8a',
        files: [{
          path: 'computer-use/extension/lib/text.ts', status: 'modified', add: 1, del: 0,
          hunks: [{ header: '@@ -1 +1,2 @@', lines: [{ kind: 'ctx', old: 1, new: 1, text: '// a note' }, { kind: 'add', new: 2, text: 'import { uploadSummary } from "./upload.js";' }] }],
        }],
      });
    });
    const line = screen.getByTestId('artifact.diff.file.0.2');
    expect(line.classList.contains('ad')).toBe(true);
    expect(line.textContent).toContain('import { uploadSummary } from "./upload.js";');
    const colorOf = (word: string) => [...line.querySelectorAll<HTMLElement>('span[style]')].find((s) => s.textContent === word)?.style.color;
    expect(colorOf('import')).toBe('var(--md-syn-keyword)');
    expect(colorOf('"./upload.js"')).toBe('var(--md-syn-string)');
    expect([...screen.getByTestId('artifact.diff.file.0.1').querySelectorAll<HTMLElement>('span[style]')].map((s) => s.style.color)).toContain('var(--md-syn-comment)');
  });

  it('only a .diff or .patch file is drawn as a diff', () => {
    expect(isDiff(cuDiff(1))).toBe(true);
    expect(isDiff({ ...cuDiff(1), path: '/w/fix.PATCH' })).toBe(true);
    expect(isDiff({ ...cuDiff(1), path: '/w/notes.md', url: '/w/notes.md' })).toBe(false);
  });
});
