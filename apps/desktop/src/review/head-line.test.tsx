// @vitest-environment happy-dom
// A folded section's head is one line in the reading column (his m_58129: 'The rest · 11 files ·
// electron.vite.config.ts, …' ran past the column, under the side panel, and 'The rest' wrapped on
// two lines). Its word never wraps; its facts (a folded one's file list) give way and end in '…',
// the whole list in their tip; a part's area line too. happy-dom has no layout: the real heads are
// rendered (Guide: the rest and a context part; By kind: Generated), review.css's and the ds
// thread.css's top-level rules cascaded onto them (the last matching rule wins), and the row's
// floor (the narrowest it can be: what can't shrink or wrap) laid out here against the reading
// column at the main column's floor.
// Mutation: .rv-facts without its min-width: 0 turns every row red (the list's whole width is its
// floor); .rv-kind without white-space: nowrap turns its word red; .rv-area without its ellipsis
// turns the context part red.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import type { DiffFile, Guide } from '@/proto';
import { Store, StoreProvider, type Diff } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { MAIN_MIN } from '@/app/resize';
import { DiffFiles } from './DiffFiles';

afterEach(cleanup);

type Rule = { sel: string; decls: ReadonlyMap<string, string> };

/** A CSS file's top-level rules (an @media or @container block skipped), each with its declarations. */
function rulesOf(file: string): readonly Rule[] {
  const css = readFileSync(join(__dirname, file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const out: Rule[] = [];
  for (const m of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const sel = (m[1] ?? '').trim();
    if (sel.startsWith('@') || sel.startsWith(':root') || sel === '') continue;
    const decls = new Map((m[2] ?? '').split(';').map((d) => d.split(':')).filter((d) => d.length >= 2).map((d) => [(d[0] ?? '').trim(), d.slice(1).join(':').trim()] as const));
    out.push({ sel, decls });
  }
  return out;
}
// the ds first, the screen's own after (its import order)
const RULES = [...rulesOf('../ds/css/thread.css'), ...rulesOf('review.css')];
const TOKENS = readFileSync(join(__dirname, '../ds/tokens.css'), 'utf8');

/** An element's declared value of a property: its last matching rule's. */
function decl(el: Element, prop: string): string | undefined {
  return RULES.filter((r) => {
    if (!r.decls.has(prop)) return false;
    try {
      return el.matches(r.sel);
    } catch {
      return false;
    }
  }).at(-1)?.decls.get(prop);
}

/** A length in px: its px numbers and its tokens (var(--x), resolved in tokens.css) summed. */
function px(expr: string): number {
  let sum = 0;
  for (const m of expr.matchAll(/var\((--[\w-]+)\)|(-?\d+(?:\.\d+)?)px/g)) {
    if (m[2] !== undefined) sum += Number(m[2]);
    else {
      const v = new RegExp(`${m[1] ?? ''}:\\s*([^;]+);`).exec(TOKENS)?.[1];
      if (v === undefined) throw new Error(`no token ${m[1] ?? ''}`);
      sum += px(v);
    }
  }
  return sum;
}

/** A run of words at 13 px in the UI font: 7 px a character, wide enough to never flatter a row. */
const textWidth = (s: string): number => s.length * 7;

/** The reading column at the main column's floor: the column's pads and the head's own sides off. */
const COLUMN = MAIN_MIN - 2 * px('var(--pad-column)') - 2 * px('var(--pad-row-x)');

/**
 * A head's floor: its chevron, its gaps and each item's narrowest. An item that may shrink to
 * nothing (min-width 0, its overflow hidden) adds nothing; one that keeps its line (nowrap, pre)
 * adds its whole text; one that wraps, its longest word.
 */
function floorOf(head: Element): number {
  const items = [...head.children];
  const gap = px(decl(head, 'gap') ?? '0px');
  let sum = gap * (items.length - 1);
  for (const el of items) {
    if (el.classList.contains('b-fold-chev')) sum += 14;
    else if (decl(el, 'min-width') === '0' && decl(el, 'overflow') === 'hidden') sum += 0;
    else if (['nowrap', 'pre'].includes(decl(el, 'white-space') ?? 'normal')) sum += textWidth(el.textContent);
    else sum += Math.max(...el.textContent.split(/\s+/).map(textWidth));
  }
  return sum;
}

/** A head's item stays one line: it keeps its line, and when it may shrink it is cut with '…'. */
function oneLine(el: Element): void {
  const ws = decl(el, 'white-space') ?? 'normal';
  expect(['nowrap', 'pre'], el.className).toContain(ws);
  if (decl(el, 'min-width') === '0') {
    expect(decl(el, 'overflow'), el.className).toBe('hidden');
    expect(decl(el, 'text-overflow'), el.className).toBe('ellipsis');
  }
}

const P = 'acme-1a2b3c4d';
const H = '9f2c41d07a3be815';
const hunk = (id: string) => ({ id, header: `@@ ${id} @@`, lines: [{ kind: 'add' as const, new: 1, text: id }] });
const file = (path: string, id: string): DiffFile => ({ path, status: 'modified', add: 1, del: 0, hunks: [hunk(id)] });
// his vibebot-review's rest: 11 files whose names alone are wider than any column
const REST = [
  'electron.vite.config.ts', 'next.config.tsx', 'app-router.tsx', 'surface-gates.test.ts', 'surface-gates.ts', 'conversation.ts',
  'vibebot-client.ts', 'bots-router.tsx', 'work-sidebar-content.tsx', 'work-sidebar-header.tsx', 'experimental-flags.ts',
];
const GEN = ['words_pages.gen.ts', 'routes.gen.ts', 'schema.gen.ts', 'icons.gen.ts', 'flags.gen.ts', 'tokens.gen.ts', 'proto.gen.ts', 'words_inbox.gen.ts'];
const diff: Diff = {
  ev: 'diff',
  project: P,
  agent: 'vibebot-review',
  base: 'main',
  head: 'sb/vibebot-poc',
  hash: H,
  files: [
    file('vibe_bot/cli.py', 'a1'),
    file('apps/web/src/bots/BotsPage.tsx', 'b1'),
    ...REST.map((n, i) => file(`apps/web/src/${n}`, `r${String(i)}`)),
    ...GEN.map((n, i) => file(`apps/web/src/gen/${n}`, `g${String(i)}`)),
  ],
};
const guide: Guide = {
  overview: 'a new Python POC for the personal assistant',
  parts: [
    { area: 'Runtime', title: 'New Python POC for the personal assistant', summary: 'the vibe_bot package', hunks: ['a1'], weight: 'critical' },
    { area: 'Web UI and the experimental Bots surface behind its feature flag', title: 'Bots page', summary: 'its components', hunks: ['b1'], weight: 'context' },
  ],
  // the generated files' hunks in the rest go to Generated (a path rule)
  rest: [...REST.map((_, i) => `r${String(i)}`), ...GEN.map((_, i) => `g${String(i)}`)],
};

function view() {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'bise' })}>
        <DiffFiles project={P} diff={diff} loading="" none="" to="vibebot-review" from="vibebot-review vs main" onEscape={() => undefined} guide={{ kind: 'agent', name: 'vibebot-review' }} />
      </RouterContext>
    </StoreProvider>,
  );
  act(() => {
    store.apply({ ev: 'guide', project: P, hash: H, state: 'ready', guide });
  });
  return v;
}

/** A section's head (its fold row). */
function headOf(v: ReturnType<typeof view>, key: string): Element {
  const head = v.getByTestId(`review.part.${key}`).querySelector(':scope > .b-fold');
  if (head === null) throw new Error(key);
  return head;
}

/** Fold a section by its head, as he does. */
function fold(v: ReturnType<typeof view>, key: string): Element {
  const head = headOf(v, key);
  if (head.getAttribute('aria-expanded') === 'true') fireEvent.click(head);
  return headOf(v, key);
}

describe("a folded section's head", () => {
  it.each([
    ['Guide: the rest, folded', 'guide', 'rest', REST],
    ['Guide: Generated', 'guide', 'generated', GEN],
    ['By kind: Generated', 'kind', 'generated', GEN],
    // its test file goes to Tests
    ['By kind: Code, folded', 'kind', 'code', ['cli.py', 'BotsPage.tsx', ...REST.filter((n) => !n.includes('.test.'))]],
  ])(`%s: one line in a ${String(COLUMN)} px column, its word whole, its list cut with its whole list in the tip`, (_, mode, key, names) => {
    const v = view();
    if (mode === 'kind') fireEvent.click(v.getByTestId('review.bar.mode.kind'));
    const head = fold(v, key);
    const word = head.querySelector('.rv-kind');
    const facts = head.querySelector('.rv-facts');
    if (word === null || facts === null) throw new Error(key);
    // the list is wider than the column: without its give the row runs past it
    expect(textWidth(facts.textContent)).toBeGreaterThan(COLUMN);
    expect(floorOf(head), key).toBeLessThanOrEqual(COLUMN);
    oneLine(word);
    expect(decl(word, 'flex'), 'the word never gives way').toBe('none');
    oneLine(facts);
    expect(decl(facts, 'min-width')).toBe('0');
    expect(facts.getAttribute('data-tip')?.split(', ').sort()).toEqual([...names].sort());
  });

  it('Guide: a context part folded, its long area cut on one line', () => {
    const v = view();
    const head = fold(v, 'part-2');
    const area = head.querySelector('.rv-area');
    if (area === null) throw new Error('area');
    expect(textWidth(area.textContent)).toBeGreaterThan(COLUMN);
    expect(floorOf(head)).toBeLessThanOrEqual(COLUMN);
    oneLine(area);
    expect(decl(area, 'min-width')).toBe('0');
  });
});
