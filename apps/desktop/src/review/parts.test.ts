import { describe, expect, it } from 'vitest';
import type { DiffFile } from '@/proto';
import { counts, folderOf, framed, kindOf, partsOf, shownOrder } from './parts';

const file = (path: string, add: number, del = 0, extra: Partial<DiffFile> = {}): DiffFile => ({ path, status: 'modified', add, del, hunks: [], ...extra });

describe('the simple mode: parts by kind, then folder (page review-guided)', () => {
  it('kinds: Generated before Tests before Words and docs before Code', () => {
    expect(kindOf(file('apps/desktop/src/proto/words_pages.gen.ts', 1))).toBe('generated');
    expect(kindOf(file('pnpm-lock.yaml', 1))).toBe('generated');
    expect(kindOf(file('rust/Cargo.lock', 1))).toBe('generated');
    expect(kindOf(file('src/__snapshots__/a.tsx.snap', 1))).toBe('generated');
    expect(kindOf(file('out/bundle.js', 1, 0, { generated: true }))).toBe('generated');
    expect(kindOf(file('apps/desktop/src/agents/SidebarRow.test.tsx', 1))).toBe('tests');
    expect(kindOf(file('rust/switchboard/src/diff_test.rs', 1))).toBe('tests');
    expect(kindOf(file('tests/review_e2e.py', 1))).toBe('tests');
    expect(kindOf(file('scripts/x_tmux.py', 1))).toBe('tests');
    // a test folder's markdown is a test (the segment wins over .md)
    expect(kindOf(file('apps/desktop/tests/checks/review.md', 1))).toBe('tests');
    expect(kindOf(file('docs/desktop-bar.md', 1))).toBe('docs');
    expect(kindOf(file('README.md', 1))).toBe('docs');
    expect(kindOf(file('docs/brand/logo.svg', 1))).toBe('docs');
    expect(kindOf(file('rust/switchboard/prompts/main.txt', 1))).toBe('docs');
    expect(kindOf(file('rust/proto/src/words/pages.rs', 1))).toBe('code');
    expect(kindOf(file('bise.toml', 1))).toBe('code');
    // a name that only holds 'test' is code
    expect(kindOf(file('src/latest.ts', 1))).toBe('code');
  });

  it('a folder is cut to its first 4 segments, the root is empty', () => {
    expect(folderOf('apps/desktop/src/agents/rowEnd.ts')).toBe('apps/desktop/src/agents');
    expect(folderOf('apps/desktop/src/agents/deep/er/x.ts')).toBe('apps/desktop/src/agents');
    expect(folderOf('docs/x.md')).toBe('docs');
    expect(folderOf('bise.toml')).toBe('');
  });

  it("pr-access's 9 files: Code, Tests, Words and docs, Generated; folders and files the most changed first", () => {
    const files = [
      file('apps/desktop/src/agents/SidebarRow.test.tsx', 100, 3),
      file('apps/desktop/src/agents/SidebarRow.tsx', 38, 12),
      file('apps/desktop/src/agents/rowEnd.test.ts', 29),
      file('apps/desktop/src/agents/rowEnd.ts', 52),
      file('apps/desktop/src/agents/sidebar.css', 14, 6),
      file('apps/desktop/src/proto/words_pages.gen.ts', 2),
      file('docs/desktop-bar.md', 12, 2),
      file('rust/proto/src/words/side.rs', 4),
      file('README.md', 4),
    ];
    const parts = partsOf(files);
    expect(parts.map((p) => [p.kind, p.files, counts(p)])).toEqual([
      ['code', 4, '+108 −18'],
      ['tests', 2, '+129 −3'],
      ['docs', 2, '+16 −2'],
      ['generated', 1, '+2'],
    ]);
    expect(parts[0]?.groups.map((g) => [g.folder, g.files.map((i) => files[i]?.path.split('/').pop())])).toEqual([
      ['apps/desktop/src/agents', ['rowEnd.ts', 'SidebarRow.tsx', 'sidebar.css']],
      ['rust/proto/src/words', ['side.rs']],
    ]);
    // ties by path: '' (the root) before 'docs' only when sizes tie; here docs is bigger
    expect(parts[2]?.groups.map((g) => g.folder)).toEqual(['docs', '']);
    expect(framed(parts)).toBe(true);
    expect(shownOrder(parts, []).map((i) => files[i]?.path.split('/').pop())).toEqual([
      'rowEnd.ts', 'SidebarRow.tsx', 'sidebar.css', 'side.rs', 'SidebarRow.test.tsx', 'rowEnd.test.ts', 'desktop-bar.md', 'README.md', 'words_pages.gen.ts',
    ]);
    // the generated part folded: its files aren't shown
    expect(shownOrder(parts, ['generated'])).toHaveLength(8);
  });

  it('one kind in one folder needs no frame; two folders do', () => {
    expect(framed(partsOf([file('src/a.ts', 1), file('src/b.ts', 2)]))).toBe(false);
    expect(framed(partsOf([file('pnpm-lock.yaml', 1)]))).toBe(false);
    expect(framed(partsOf([file('src/a.ts', 1), file('lib/b.ts', 2)]))).toBe(true);
    expect(framed(partsOf([]))).toBe(false);
  });

  it('counts leave out a zero', () => {
    expect(counts({ add: 3, del: 0 })).toBe('+3');
    expect(counts({ add: 0, del: 4 })).toBe('−4');
    expect(counts({ add: 0, del: 0 })).toBe('');
  });
});
