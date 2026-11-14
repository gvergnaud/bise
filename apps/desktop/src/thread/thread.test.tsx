// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/react';
import { artifactOf, Markdown, openable } from './Markdown';
import { anchorRow, nextFirstIndex, placeOf } from './ThreadList';

afterEach(cleanup);

describe('the list keeps what he reads when an older page comes in front', () => {
  it('firstItemIndex goes down by the rows added in front, nothing else moves it', () => {
    const a = { first: 'e:10', index: 1000 };
    expect(nextFirstIndex(a, ['e:10', 'e:12', 'p:1'])).toBe(a);
    expect(nextFirstIndex(a, ['e:4', 'e:7', 'e:10', 'e:12'])).toEqual({ first: 'e:4', index: 998 });
    // a first page into an empty list: the index stays
    expect(nextFirstIndex({ first: undefined, index: 1000 }, ['e:1'])).toEqual({ first: 'e:1', index: 1000 });
  });
  it('the row held still is the first one under the top edge (the line he reads)', () => {
    const rows = [
      { index: 998, top: -90, bottom: -50 },
      { index: 999, top: -50, bottom: 10 },
      { index: 1000, top: 10, bottom: 60 },
    ];
    expect(anchorRow(rows, 0)).toEqual({ index: 999, top: -50 });
    expect(anchorRow(rows, 10)).toEqual({ index: 1000, top: 10 });
    expect(anchorRow([], 0)).toBeUndefined();
  });
  it('his place in keep: the row and its px from the top; an older build kept the row alone', () => {
    expect(placeOf({ key: 'e:14', off: -17 })).toEqual({ key: 'e:14', off: -17 });
    expect(placeOf({ key: 'e:14', off: Number.NaN })).toEqual({ key: 'e:14', off: 0 });
    expect(placeOf({ off: 3 })).toBeUndefined();
    expect(placeOf(undefined)).toBeUndefined();
    expect(placeOf(400)).toBeUndefined();
  });
});

const answer = [
  'here is the plan:',
  '',
  '- one',
  '- two',
  '',
  '```rust',
  'fn main() {',
  '    println!("hi");',
  '}',
  '```',
  '',
  '| a | b |',
  '|---|---|',
  '| 1 | 2 |',
  '',
  'see [the docs](https://bise.dev/docs), [a file](file:///etc/passwd) and <b>raw</b>.',
].join('\n');

describe('answers in markdown', () => {
  it('lists, a code block that keeps its shape, a table, links that never navigate', () => {
    const onLink = vi.fn();
    const { container } = render(<Markdown text={answer} onLink={onLink} />);
    expect(container.querySelectorAll('li')).toHaveLength(2);
    expect(container.querySelector('pre.th-pre code')?.textContent).toBe('fn main() {\n    println!("hi");\n}\n');
    expect(container.querySelector('.th-table td')?.textContent).toBe('1');
    const a = container.querySelector('a');
    expect(a?.getAttribute('href')).toBe('https://bise.dev/docs');
    if (a === null) throw new Error('no link');
    const click = fireEvent.click(a);
    expect(click).toBe(false); // default prevented: the window stays
    expect(onLink).toHaveBeenCalledWith('https://bise.dev/docs', false);
    // file: is not a link; raw HTML is not rendered as HTML
    expect(container.querySelectorAll('a')).toHaveLength(1);
    expect(container.querySelector('b')).toBeNull();
  });

  it("an artifact: link is the artifact's pink name inline, its view on a click (desktop-agents-bar §5)", () => {
    const onLink = vi.fn();
    const onArtifact = vi.fn();
    const { getByTestId, container } = render(<Markdown text="the numbers are in [product page, before / after](artifact:product-page@v2)." onLink={onLink} onArtifact={onArtifact} />);
    const name = getByTestId('thread.artname');
    expect(name.className).toContain('b-artname');
    expect(name.textContent).toBe('product page, before / after');
    expect(name.getAttribute('role')).toBe('link');
    fireEvent.click(name);
    expect(onArtifact).toHaveBeenCalledWith('product-page');
    expect(onLink).not.toHaveBeenCalled();
    expect(container.querySelectorAll('a')).toHaveLength(0);
    expect(artifactOf('artifact:q3')).toBe('q3');
    expect(artifactOf('artifact:q3@v1')).toBe('q3');
    expect(artifactOf('https://x')).toBeUndefined();
  });

  it('only http(s) and mailto open', () => {
    expect(openable('https://x')).toBe(true);
    expect(openable('mailto:a@b.c')).toBe(true);
    expect(openable('javascript:alert(1)')).toBe(false);
    expect(openable('file:///x')).toBe(false);
    expect(openable(undefined)).toBe(false);
  });
});

describe('his own messages keep every line he typed (qa-flows 68b4f1ea)', () => {
  it('a newline in his words is a line break; an answer stays standard markdown; code keeps its shape', async () => {
    const { hardBreaks } = await import('./Markdown');
    const tree = { type: 'root', children: [{ type: 'paragraph', children: [{ type: 'text', value: 'one\ntwo\nthree' }] }, { type: 'code', value: 'a\nb' }] };
    hardBreaks(tree);
    expect(tree.children[0]?.children?.map((c) => c.type)).toEqual(['text', 'break', 'text', 'break', 'text']);
    expect(tree.children[1]).toEqual({ type: 'code', value: 'a\nb' });
    const mine = render(<Markdown text={'line 1\nline 2\nline 3'} breaks />);
    expect(mine.container.querySelectorAll('br')).toHaveLength(2);
    cleanup();
    const answer = render(<Markdown text={'line 1\nline 2'} />);
    expect(answer.container.querySelectorAll('br')).toHaveLength(0);
  });
});
