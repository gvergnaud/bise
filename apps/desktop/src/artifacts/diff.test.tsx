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
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { NO_LAYOUT_ROWS } from '@/review/lineWindow';

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

  // code-viewer-width (his m_57763: security.diff, 29k lines, one column of characters, the window
  // lagged). Mutations: artifacts.css without its `.art-open-body > .rv-screen` rule turns the
  // width red; FileDiff drawing all its lines (no lineWindow) turns the window red.
  it("takes the panel's width: the body's flex item grows and may shrink below its content", () => {
    const style = document.createElement('style');
    style.textContent = ['artifacts.css', '../review/review.css'].map((f) => readFileSync(join(__dirname, f), 'utf8')).join('\n');
    document.head.append(style);
    try {
      mount(cuDiff(1));
      const col = screen.getByTestId('artifact.diff');
      expect(col.parentElement?.classList.contains('art-open-body')).toBe(true);
      // .rv-body is an inline-size container (no width of its own): without a flex grow the
      // centered item shrinks to nothing
      const css = getComputedStyle(col);
      expect(css.flexGrow).toBe('1');
      expect(Number.parseFloat(css.minWidth)).toBe(0);
    } finally {
      style.remove();
    }
  });

  it('draws only a window of a 5,000-line file, the rest a gap of its height, its lines keeping their ids', () => {
    const { store } = mount(cuDiff(1));
    const n = 5_000;
    act(() => {
      store.apply({
        ev: 'diff', project: P, agent: '', base: '', artifact: 'cu-upload-diff', version: 1, title: 'cu-upload at dd86c8a',
        files: [{
          path: 'security.ts', status: 'added', add: n, del: 0,
          hunks: [{ header: `@@ -0,0 +1,${String(n)} @@`, lines: Array.from({ length: n }, (_, i) => ({ kind: 'add' as const, new: i + 1, text: `const line${String(i)} = ${String(i)};` })) }],
        }],
      });
    });
    const file = screen.getByTestId('artifact.diff.file.0');
    const rows = file.querySelectorAll('.b-dl');
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThanOrEqual(NO_LAYOUT_ROWS);
    expect(screen.getByTestId('artifact.diff.file.0.1').textContent).toContain('const line0 = 0;');
    // the rows not drawn: one gap after them, as many line-heights (the scroller keeps its length)
    const gap = file.querySelector<HTMLElement>('.b-dgap');
    expect(gap?.style.getPropertyValue('--rows')).toBe(String(n + 1 - rows.length));
  });

  // designer m_57777: a long line is never cut; it wraps at any character, its next rows at its own
  // indent. Mutation: thread.css without its .b-dl>.cd rule turns it red.
  it('wraps a 300-character line, its next rows at its indent, never cut', () => {
    const style = document.createElement('style');
    style.textContent = readFileSync(join(__dirname, '../ds/css/thread.css'), 'utf8');
    document.head.append(style);
    try {
      const { store } = mount(cuDiff(1));
      act(() => {
        store.apply({
          ev: 'diff', project: P, agent: '', base: '', artifact: 'cu-upload-diff', version: 1, title: 'cu-upload at dd86c8a',
          files: [{ path: 'notes.txt', status: 'modified', add: 1, del: 0, hunks: [{ header: '@@ -1 +1 @@', lines: [{ kind: 'add', new: 1, text: `    ${'x'.repeat(300)}` }] }] }],
        });
      });
      const cell = screen.getByTestId('artifact.diff.file.0.1').querySelector<HTMLElement>('.cd');
      expect(cell?.textContent).toBe(`    ${'x'.repeat(300)}`);
      expect(cell?.style.getPropertyValue('--ind')).toBe('4');
      const css = getComputedStyle(cell as HTMLElement);
      expect([css.whiteSpace, css.wordBreak]).toEqual(['pre-wrap', 'break-all']);
    } finally {
      style.remove();
    }
  });

  it('only a .diff or .patch file is drawn as a diff', () => {
    expect(isDiff(cuDiff(1))).toBe(true);
    expect(isDiff({ ...cuDiff(1), path: '/w/fix.PATCH' })).toBe(true);
    expect(isDiff({ ...cuDiff(1), path: '/w/notes.md', url: '/w/notes.md' })).toBe(false);
  });
});
