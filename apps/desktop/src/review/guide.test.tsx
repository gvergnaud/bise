// @vitest-environment happy-dom
// The guided review in DiffFiles (page review-guided, pick B): the guide asked once per diff, swapped
// in place before he scrolls, behind 'Show it' after; the bar's slot for each state; Viewed (v).
// A fake core and fake guides: never a model call.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CoreCmd, CorePort } from '@/shared/api';
import type { DiffFile, Guide } from '@/proto';
import { Store, StoreProvider, type Diff } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { pages } from '@/proto/words_pages.gen';
import { DiffFiles } from './DiffFiles';

const P = 'acme-1a2b3c4d';
const H = '9f2c41d07a3be815';
const hunk = (id: string, text: string) => ({ id, header: `@@ ${id} @@`, lines: [{ kind: 'add' as const, new: 1, text }] });
const file = (path: string, hunks: ReturnType<typeof hunk>[]): DiffFile => ({ path, status: 'modified', add: hunks.length, del: 0, hunks });
const diff: Diff = {
  ev: 'diff',
  project: P,
  agent: 'pr-access',
  base: 'main',
  head: 'sb/pr-access',
  hash: H,
  files: [
    file('apps/desktop/src/agents/SidebarRow.tsx', [hunk('a1', 'row'), hunk('a2', 'row2')]),
    file('apps/desktop/src/agents/rowEnd.ts', [hunk('b1', 'end')]),
    file('apps/desktop/src/agents/sidebar.css', [hunk('c1', 'css')]),
    file('docs/desktop-bar.md', [hunk('d1', 'doc')]),
  ],
};
const guide: Guide = {
  overview: 'the row end gets one owner',
  parts: [
    { area: 'Sidebar', title: "Give the row's end one owner", summary: 'rowEndOf lists it', hunks: ['b1', 'a1', 'a2'], weight: 'critical' },
    { area: 'Sidebar', title: 'Room for the pin', summary: 'flex', hunks: ['c1'], weight: 'context' },
  ],
  rest: ['d1'],
};

function view() {
  const cmds: CoreCmd[] = [];
  const port: CorePort = { send: (c) => cmds.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'bise' })}>
        <DiffFiles project={P} diff={diff} loading="" none="" to="pr-access" from="pr-access vs main" onEscape={() => undefined} guide={{ kind: 'agent', name: 'pr-access' }} />
      </RouterContext>
    </StoreProvider>,
  );
  const send = (ev: Parameters<Store['apply']>[0]) => {
    act(() => {
      store.apply(ev);
    });
  };
  return { v, cmds, send };
}

const ready = { ev: 'guide' as const, project: P, hash: H, state: 'ready' as const, guide };

afterEach(cleanup);

describe('the guided review', () => {
  it('asks the guide once for its diff; until it comes, By kind and the slot says it is being written', () => {
    const { v, cmds, send } = view();
    expect(cmds.filter((c) => c.cmd === 'guide')).toEqual([{ cmd: 'guide', project: P, target: { kind: 'agent', name: 'pr-access' } }]);
    send({ ev: 'guide', project: P, hash: H, state: 'writing' });
    expect(v.getByTestId('review.bar.guide').textContent).toBe(pages.review_guide_writing);
    expect(v.getByTestId('review.part.code')).toBeTruthy();
  });

  it('ready before he scrolls: the guide in place, its overview, its parts, the rest last; every hunk drawn', () => {
    const { v, send } = view();
    send(ready);
    expect(v.getByTestId('review.overview').textContent).toBe(guide.overview);
    expect(v.queryByTestId('review.part.code')).toBeNull();
    const p1 = v.getByTestId('review.part.part-1');
    expect(p1.textContent).toContain("Give the row's end one owner");
    expect(p1.querySelector('.rv-dot')).not.toBeNull();
    // context: a dim word, its code open
    const p2 = v.getByTestId('review.part.part-2');
    expect(p2.textContent).toContain(pages.review_part_context);
    expect(p2.querySelector('.b-dl')).not.toBeNull();
    expect(v.getByTestId('review.part.rest').textContent).toContain(pages.review_part_rest);
    // the guide's reading order, then the rest
    expect([0, 1, 2, 3].map((i) => v.getByTestId(`review.file.${String(i)}`).querySelector('.fh')?.textContent.split(' ')[0])).toEqual([
      'apps/desktop/src/agents/rowEnd.ts',
      'apps/desktop/src/agents/SidebarRow.tsx',
      'apps/desktop/src/agents/sidebar.css',
      'docs/desktop-bar.md',
    ]);
    expect(v.getByTestId('review.bar.place').querySelector('.rv-wide')?.textContent).toBe("01 / 02 · Give the row's end one owner");
    expect(v.getByTestId('review.bar.place').querySelector('.rv-narrow')?.textContent).toBe('Sidebar');
    // By kind is one click away, and back
    fireEvent.click(v.getByTestId('review.bar.mode.kind'));
    expect(v.getByTestId('review.part.code')).toBeTruthy();
    fireEvent.click(v.getByTestId('review.bar.mode.guide'));
    expect(v.getByTestId('review.part.part-1')).toBeTruthy();
  });

  it('ready after he scrolled: nothing moves, the slot offers it; Show it swaps', () => {
    const { v, send } = view();
    const body = v.getByTestId('review.files');
    act(() => {
      body.scrollTop = 400;
      fireEvent.scroll(body);
    });
    send(ready);
    expect(v.getByTestId('review.part.code')).toBeTruthy();
    expect(v.getByTestId('review.bar.guide').textContent).toContain(pages.review_guide_ready);
    fireEvent.click(v.getByTestId('review.bar.guide.show'));
    expect(v.getByTestId('review.part.part-1')).toBeTruthy();
    expect(v.queryByTestId('review.bar.guide')).toBeNull();
  });

  it("his main model: nothing runs, the button names the model and the cost; his click runs it; a failure offers Try again", () => {
    const { v, cmds, send } = view();
    send({ ev: 'guide', project: P, hash: H, state: 'needs_click', tokens: 5200, model: 'anthropic/claude-opus-5-5' });
    const b = v.getByTestId('review.bar.guide.write');
    // the model's name without its provider; its whole id in the tip; a narrow column's short words
    expect(b.querySelector('.rv-wide')?.textContent).toBe(pages.review_guide_write({ tokens: '5k', model: 'claude-opus-5-5' }));
    expect(b.querySelector('.rv-narrow')?.textContent).toBe(pages.review_guide_write_short({ tokens: '5k' }));
    expect(b.querySelector('.rv-bare')?.textContent).toBe(pages.review_guide_write_bare);
    expect(b.getAttribute('title')).toBe(pages.review_guide_write_tip({ model: 'anthropic/claude-opus-5-5' }));
    // Guide stays in the control, off until a guide is here
    expect(v.getByTestId('review.bar.mode.guide').getAttribute('aria-disabled')).toBe('true');
    fireEvent.click(b);
    expect(cmds.at(-1)).toEqual({ cmd: 'guide', project: P, target: { kind: 'agent', name: 'pr-access' }, run: true });
    send({ ev: 'guide', project: P, hash: H, state: 'failed' });
    fireEvent.click(v.getByTestId('review.bar.guide.retry'));
    expect(cmds.filter((c) => c.cmd === 'guide' && c.run === true)).toHaveLength(2);
    send({ ev: 'guide', project: P, hash: H, state: 'small' });
    expect(v.getByTestId('review.bar.guide').textContent).toBe(pages.review_guide_small);
    // a small change never gets one: the control drops Guide
    expect(v.queryByTestId('review.bar.mode.guide')).toBeNull();
  });

  it('Viewed: the box or v marks a file, its card folds, the bar counts it', () => {
    const { v } = view();
    expect(v.getByTestId('review.bar.viewed').querySelector('.rv-wide')?.textContent).toBe(pages.review_viewed_count({ n: 0, total: 4 }));
    fireEvent.click(v.getByTestId('review.file.0.viewed'));
    expect(v.getByTestId('review.bar.viewed').querySelector('.rv-wide')?.textContent).toBe(pages.review_viewed_count({ n: 1, total: 4 }));
    expect(v.getByTestId('review.file.0').querySelector('.b-dl')).toBeNull();
    fireEvent.click(v.getByTestId('review.file.0.viewed'));
    expect(v.getByTestId('review.bar.viewed').querySelector('.rv-wide')?.textContent).toBe(pages.review_viewed_count({ n: 0, total: 4 }));
  });

  it('n and p walk the sections; ⌘\\ opens the outline, a pick goes there', () => {
    const { v } = view();
    const seen: string[] = [];
    Element.prototype.scrollIntoView = function (this: Element) {
      seen.push(this.getAttribute('data-section') ?? '');
    };
    const body = v.getByTestId('review.files');
    fireEvent.keyDown(body, { key: 'n' });
    fireEvent.keyDown(body, { key: 'n' });
    fireEvent.keyDown(body, { key: 'p' });
    expect(seen).toEqual(['docs', 'docs', 'code']);
    fireEvent.keyDown(body, { key: String.fromCharCode(92), metaKey: true });
    expect(v.getByTestId('review.outline')).toBeTruthy();
  });
});
