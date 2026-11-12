// @vitest-environment happy-dom
// md-artifact (architect m_38679): a .md file in Writer (plugins/docs, its markdown.js on office-md.js,
// micromark + GFM) reads as an answer does. The thread's Markdown draws office/fixtures/gfm.md with the
// block structure Writer draws it with (office/fixtures/gfm.blocks.json; Writer's side:
// plugins/docs/test/markdown.test.mjs).
import { readFileSync } from 'node:fs';
import { cleanup, render } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { Markdown } from './Markdown';

afterEach(cleanup);

const FIX = '../../office/fixtures';
const BLOCKS = 'h1, h2, h3, h4, h5, h6, p, blockquote, ul, ol, li, table, tr, th, td, pre, hr';

it('the thread draws gfm.md with the same blocks as the artifact viewer', () => {
  const text = readFileSync(`${FIX}/gfm.md`, 'utf8');
  const want = (JSON.parse(readFileSync(`${FIX}/gfm.blocks.json`, 'utf8')) as { tags: string[] }).tags;
  const v = render(<Markdown text={text} />);
  const tags = [...v.container.querySelectorAll(BLOCKS)].map((e) => e.tagName.toLowerCase());
  expect(tags).toEqual(want);
});
