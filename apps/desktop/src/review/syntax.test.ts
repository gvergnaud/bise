// A diff's code colored by its language (diff-syntax, his m_53153): each side of a hunk tokenized
// as one text, so a − line takes the old side's colors and a + line the new side's.
import { describe, expect, it } from 'vitest';
import type { DiffLine } from '@/ds';
import { diffRuns, langOfPath } from './syntax';

const colorOf = (runs: ReturnType<typeof diffRuns>[number] | undefined, word: string): unknown => runs?.find((r) => r.text.includes(word))?.style?.color;

describe('a diff colored by its language', () => {
  it("knows a file's language by its extension, plain when prism doesn't", () => {
    expect(langOfPath('computer-use/extension/lib/text.ts')).toBe('typescript');
    expect(langOfPath('rust/tui/src/lib.rs')).toBe('rust');
    expect(langOfPath('apps/desktop/src/App.TSX')).toBe('tsx');
    expect(langOfPath('Makefile')).toBeUndefined();
    expect(langOfPath('.gitignore')).toBeUndefined();
    expect(langOfPath('bise.toml')).toBeUndefined();
  });

  it("colors a .ts hunk's lines, its head plain, a comment over lines one comment", () => {
    const lines: DiffLine[] = [
      { text: '@@ -1,3 +1,4 @@', sign: '', head: true },
      { text: '/** a note', sign: '', old: 1, new: 1 },
      { text: '  over two lines */', sign: '', old: 2, new: 2 },
      { text: 'const a = 1;', sign: '-', old: 3 },
      { text: 'const a = "two";', sign: '+', new: 3 },
      { text: 'export function f() {}', sign: '+', new: 4 },
    ];
    const runs = diffRuns(lines, langOfPath('a.ts'));
    expect(runs[0]).toBeNull();
    expect(colorOf(runs[2], 'over two lines')).toBe('var(--md-syn-comment)');
    expect(colorOf(runs[3], 'const')).toBe('var(--md-syn-keyword)');
    expect(colorOf(runs[3], '1')).toBe('var(--md-syn-number)');
    expect(colorOf(runs[4], '"two"')).toBe('var(--md-syn-string)');
    expect(colorOf(runs[5], 'export')).toBe('var(--md-syn-keyword)');
    // each line's runs are its own words
    runs.forEach((r, i) => {
      if (r !== null) expect(r.map((x) => x.text).join('')).toBe(lines[i]?.text);
    });
  });

  it('draws a language prism lacks plain', () => {
    expect(diffRuns([{ text: 'all: build', sign: '+' }], langOfPath('Makefile'))).toEqual([null]);
  });

  it('colors a 2 000-line diff fast enough to stay fluid', () => {
    const lines: DiffLine[] = [{ text: '@@ -1,1000 +1,1000 @@', sign: '', head: true }];
    for (let i = 0; i < 2000; i++) lines.push({ text: `  const x${String(i)} = await f(\`a \${String(i)}\`, { k: [1, 2] }); // n`, sign: i % 3 === 0 ? '-' : i % 3 === 1 ? '+' : '' });
    const t = Date.now();
    diffRuns(lines, 'typescript');
    expect(Date.now() - t).toBeLessThan(400);
  });
});
