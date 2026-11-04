// A diff's code colored by its language (diff-syntax, his m_53153): each side of a hunk tokenized
// as one text, so a − line takes the old side's colors and a + line the new side's.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { DiffLine } from '@/ds';
import { diffRuns, langOfPath } from './syntax';

// the ds's own values: the palettes (tokens.css, md-tokens.css) and the diff's tints (thread.css)
type Rgb = readonly [number, number, number];
const read = (f: string): string => readFileSync(join(__dirname, '../ds', f), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
/** A theme's --p-* values: dark from the `:root, [data-theme="dark"]` blocks, light from `[data-theme="light"]`'s. */
function palette(theme: 'dark' | 'light'): Record<string, string> {
  const head = theme === 'dark' ? /^:root, \[data-theme="dark"\] \{/u : /^\[data-theme="light"\] \{/u;
  const out: Record<string, string> = {};
  for (const css of [read('tokens.css'), read('md-tokens.css')]) {
    for (const m of css.matchAll(/^([^{}\n]+)\{([^{}]*)\}/gmu)) {
      if (!head.test(`${(m[1] ?? '').trim()} {`)) continue;
      for (const v of (m[2] ?? '').matchAll(/(--p-[\w-]+):\s*(#[0-9a-f]{6})/giu)) out[v[1] ?? ''] ??= v[2] ?? '';
    }
  }
  return out;
}
const tintOf = (cls: string): number => Number(new RegExp(`\\.b-dl\\.${cls}\\{background:color-mix\\(in srgb,var\\(--c-\\w+\\) (\\d+)%`, 'u').exec(read('css/thread.css'))?.[1]) / 100;
const TINT = { ad: tintOf('ad'), rm: tintOf('rm') };
const rgbOf = (h: string | undefined): Rgb => [1, 3, 5].map((i) => parseInt((h ?? '').slice(i, i + 2), 16)) as unknown as Rgb;
const mix = (a: Rgb, p: number, b: Rgb): Rgb => a.map((c, i) => c * p + (b[i] ?? 0) * (1 - p)) as unknown as Rgb;
const lin = (c: number): number => (c / 255 <= 0.04045 ? c / 255 / 12.92 : ((c / 255 + 0.055) / 1.055) ** 2.4);
const lum = ([r, g, b]: Rgb): number => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
const contrast = (a: Rgb, b: Rgb): number => (Math.max(lum(a), lum(b)) + 0.05) / (Math.min(lum(a), lum(b)) + 0.05);

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

  it('keeps code ≥ 4.5:1 on the ground and on both +/- tints, light and dark (designer m_53174)', () => {
    const lows: string[] = [];
    for (const theme of ['dark', 'light'] as const) {
      const p = palette(theme);
      const bg = rgbOf(p['--p-bg']);
      const grounds: Record<string, Rgb> = { bg, '+': mix(rgbOf(p['--p-ok']), TINT.ad, bg), '−': mix(rgbOf(p['--p-err']), TINT.rm, bg) };
      for (const syn of ['keyword', 'string', 'comment', 'number', 'call', 'type']) {
        for (const [on, g] of Object.entries(grounds)) {
          const c = contrast(rgbOf(p[`--p-syn-${syn}`]), g);
          if (c < 4.5) lows.push(`${theme} ${syn} on ${on}: ${c.toFixed(2)}`);
        }
      }
    }
    expect(lows).toEqual([]);
  });

  it('colors a 2 000-line diff fast enough to stay fluid', () => {
    const lines: DiffLine[] = [{ text: '@@ -1,1000 +1,1000 @@', sign: '', head: true }];
    for (let i = 0; i < 2000; i++) lines.push({ text: `  const x${String(i)} = await f(\`a \${String(i)}\`, { k: [1, 2] }); // n`, sign: i % 3 === 0 ? '-' : i % 3 === 1 ? '+' : '' });
    const t = Date.now();
    diffRuns(lines, 'typescript');
    expect(Date.now() - t).toBeLessThan(400);
  });
});
