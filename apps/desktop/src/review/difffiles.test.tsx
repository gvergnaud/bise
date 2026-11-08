// @vitest-environment happy-dom
// DiffFiles' simple mode (page review-guided): the parts by kind and folder, the generated part
// folded, and the quote's rows and test ids in the shown order (review-guided m_57256).
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import type { DiffFile } from '@/proto';
import type { Diff } from '@/store';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { pages } from '@/proto/words_pages.gen';
import { DiffFiles } from './DiffFiles';
import { takeQuotes } from './quote';

const P = 'acme-1a2b3c4d';

const file = (path: string, add: number, text = 'x'): DiffFile => ({
  path,
  status: 'modified',
  add,
  del: 0,
  hunks: [{ header: '@@ -1 +1 @@', lines: Array.from({ length: add }, (_, i) => ({ kind: 'add' as const, new: i + 1, text: `${text}${String(i)}` })) }],
});

const diffOf = (files: DiffFile[]): Diff => ({ ev: 'diff', project: P, agent: 'pr-access', base: 'main', head: 'sb/pr-access', files });

// git's order: alphabetical; the test first, the generated words between two real files
const nine = diffOf([
  file('apps/desktop/src/agents/SidebarRow.test.tsx', 6, 't'),
  file('apps/desktop/src/agents/SidebarRow.tsx', 4, 'row'),
  file('apps/desktop/src/agents/rowEnd.ts', 5, 'end'),
  file('apps/desktop/src/proto/words_pages.gen.ts', 1, 'gen'),
  file('docs/desktop-bar.md', 2, 'doc'),
]);

function view(diff: Diff) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  const router = createRouter({ screen: 'bise' });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <DiffFiles project={P} diff={diff} loading="" none="" to="pr-access" from="pr-access vs main" onEscape={() => undefined} />
      </RouterContext>
    </StoreProvider>,
  );
  return { v, router };
}

afterEach(cleanup);

describe('the review in parts (simple mode)', () => {
  it('shows Code, Tests, Words and docs, then Generated folded; the files in that order', () => {
    const { v } = view(nine);
    const body = v.getByTestId('review.files');
    const heads = [...body.querySelectorAll('.b-fold')].map((h) => h.textContent);
    expect(heads).toEqual([
      `${pages.review_kind_code} · ${pages.review_part_files({ n: 2 })} · +9`,
      `${pages.review_kind_tests} · ${pages.review_part_files({ n: 1 })} · +6`,
      `${pages.review_kind_docs} · ${pages.review_part_files({ n: 1 })} · +2`,
      `${pages.review_kind_generated} · ${pages.review_part_files({ n: 1 })} · words_pages.gen.ts · +1`,
    ]);
    expect([...body.querySelectorAll('.rv-folder')].map((f) => f.textContent)).toEqual(['apps/desktop/src/agents', 'apps/desktop/src/agents', 'docs']);
    // the shown order names the test ids; the folded file isn't drawn
    expect(v.getByTestId('review.file.0').textContent).toContain('rowEnd.ts');
    expect(v.getByTestId('review.file.1').textContent).toContain('SidebarRow.tsx');
    expect(v.getByTestId('review.file.2').textContent).toContain('SidebarRow.test.tsx');
    expect(v.getByTestId('review.file.3').textContent).toContain('desktop-bar.md');
    expect(v.queryByTestId('review.file.4')).toBeNull();
    fireEvent.click(v.getByTestId('review.part.generated.fold'));
    expect(v.getByTestId('review.file.4').textContent).toContain('words_pages.gen.ts');
  });

  it('tab walks the shown files and skips the folded ones', () => {
    const { v } = view(nine);
    const seen: string[] = [];
    Element.prototype.scrollIntoView = function (this: Element) {
      seen.push(this.querySelector('.fh')?.textContent ?? '');
    };
    const body = v.getByTestId('review.files');
    for (let i = 0; i < 5; i++) fireEvent.keyDown(body, { key: 'Tab' });
    expect(seen.map((h) => h.split(' ')[0])).toEqual(['rowEnd.ts', 'SidebarRow.tsx', 'SidebarRow.test.tsx', 'desktop-bar.md', 'rowEnd.ts']);
  });

  it('under a folder line a head names the file short, its full path in the tip; he folds any kind', () => {
    const { v } = view(nine);
    const head = v.getByTestId('review.file.0').querySelector('.fh');
    expect(head?.textContent).toBe('rowEnd.ts +5');
    expect(head?.getAttribute('data-tip')).toBe('apps/desktop/src/agents/rowEnd.ts');
    fireEvent.click(v.getByTestId('review.part.code.fold'));
    expect(v.getByTestId('review.part.code.fold').textContent).toBe(`${pages.review_kind_code} · ${pages.review_part_files({ n: 2 })} · rowEnd.ts, SidebarRow.tsx · +9`);
    // the code's files leave the shown order: the test is first now
    expect(v.getByTestId('review.file.0').textContent).toContain('SidebarRow.test.tsx');
  });

  it('a quote dragged across two parts quotes the lines shown, in the shown order', () => {
    const { v, router } = view(nine);
    // from SidebarRow.tsx's last line (Code) to the test's first (Tests)
    fireEvent.mouseDown(v.getByTestId('review.file.1.4'), { button: 0 });
    fireEvent.mouseOver(v.getByTestId('review.file.2.1'));
    act(() => {
      window.dispatchEvent(new MouseEvent('mouseup'));
    });
    fireEvent.keyDown(v.getByTestId('review.files'), { key: 'Tab' });
    expect(router.get()).toEqual({ screen: 'thread', project: P, agent: 'pr-access' });
    const q = takeQuotes(P, 'pr-access');
    expect(q?.quotes.map((x) => [x.at.file, x.at.new])).toEqual([
      ['apps/desktop/src/agents/SidebarRow.tsx', '4'],
      ['apps/desktop/src/agents/SidebarRow.test.tsx', '1'],
    ]);
  });

  it('one kind in one folder: no part heads, as before', () => {
    const { v } = view(diffOf([file('src/a.ts', 2), file('src/b.ts', 3)]));
    expect(v.getByTestId('review.files').querySelector('.b-fold, .rv-folder')).toBeNull();
    expect(v.getByTestId('review.file.0').textContent).toContain('src/b.ts');
  });
});
