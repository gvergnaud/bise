// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import type { Artifact } from '@/proto';
import type { ThumbAnswer, ThumbReq } from '@/shared/api';
import type { Item } from '../model';
import { Thumb } from './Thumb';
import { resetThumbQueue } from './run';

const art = (id: string, kind: string, extra: Partial<Artifact> = {}): Artifact => ({ id, kind, title: id, agent: 'perf', version: 1, at_ms: 1_000, url: '', new: false, ...extra });
const item = (a: Artifact): Item => ({ project: 'shop-1', projectName: 'storefront', a });

let asked: { req: ThumbReq; want: boolean }[] = [];
let answers: Record<string, ThumbAnswer | null> = {};

beforeEach(() => {
  asked = [];
  answers = {};
  (globalThis as { bise?: unknown }).bise = {
    thumb: (req: ThumbReq, want: boolean): Promise<ThumbAnswer | null> => {
      asked.push({ req, want });
      return Promise.resolve(answers[req.id] ?? null);
    },
  };
  resetThumbQueue();
});
afterEach(() => {
  cleanup();
  delete (globalThis as { bise?: unknown }).bise;
  resetThumbQueue();
});

const flush = () => act(async () => new Promise((r) => setTimeout(r, 0)));
const kindOf = (c: HTMLElement): string | null => c.querySelector('[data-thumb]')?.getAttribute('data-thumb') ?? null;

describe('<Thumb />', () => {
  it('shows the faint icon, then fades the picture in once it loads', async () => {
    answers['shot'] = { img: 'data:image/png;base64,AAAA' };
    const { container } = render(<Thumb item={item(art('shot', 'image', { path: '/w/shot.png' }))} visible />);
    expect(kindOf(container)).toBe('icon');
    expect(container.querySelector('.th-faint svg')).not.toBeNull();
    await flush();
    expect(kindOf(container)).toBe('img');
    const img = container.querySelector('img');
    // the fade (thumbs.css: .in's 160 ms opacity) starts once it is decoded; the faint icon goes then
    if (img) fireEvent.load(img);
    expect(img?.className).toBe('in');
    expect(container.querySelector('.th-faint')).toBeNull();
    expect(asked).toEqual([{ req: expect.objectContaining({ how: 'file', src: '/w/shot.png', id: 'shot', v: 1 }) as unknown, want: true }]);
  });

  it('shows a picture it already has at once, with no fade, on a second mount', async () => {
    answers['shot'] = { img: 'data:image/png;base64,AAAA' };
    const it1 = item(art('shot', 'image', { path: '/w/shot.png' }));
    const first = render(<Thumb item={it1} visible />);
    await flush();
    first.unmount();
    const { container } = render(<Thumb item={it1} visible />);
    expect(container.querySelector('img')?.className).toBe('in');
    expect(asked).toHaveLength(1);
  });

  it("draws a PR's and a link's tile with no ask at all", () => {
    const pr = render(<Thumb item={item(art('p', 'pr', { url: 'https://github.com/gvergnaud/bise/pull/482' }))} visible />);
    expect(kindOf(pr.container)).toBe('tile');
    expect(pr.container.textContent).toBe('bise #482');
    const link = render(<Thumb item={item(art('l', 'link', { url: 'https://drive.google.com/x' }))} visible />);
    expect(link.container.textContent).toBe('drive.google.com');
    expect(asked).toEqual([]);
  });

  it("draws a file's picture (Quick Look's, a md's too), a file with no picture as its extension and size, a video's length", async () => {
    answers['doc'] = { img: 'data:image/png;base64,AAAA' };
    answers['glb'] = { none: { ext: 'glb', bytes: 12_400_000 } };
    answers['clip'] = { img: 'data:image/png;base64,AAAA', secs: 4.2 };
    const doc = render(<Thumb item={item(art('doc', 'doc', { path: '/w/review.md' }))} visible />);
    const glb = render(<Thumb item={item(art('glb', 'file', { path: '/w/flat.glb' }))} visible />);
    const clip = render(<Thumb item={item(art('clip', 'video', { path: '/w/clip.mp4' }))} visible />);
    await flush();
    expect(kindOf(doc.container)).toBe('img');
    expect(kindOf(glb.container)).toBe('none');
    expect(glb.container.textContent).toBe('glb12.4 MB');
    expect(clip.container.querySelector('.th-play')?.textContent).toBe('0:04');
  });

  it('keeps the faint icon when nothing comes back, and asks again for a new version', async () => {
    const a = art('pg', 'page', { url: 'http://127.0.0.1:4747/p/pg' });
    const { container, rerender } = render(<Thumb item={item(a)} visible />);
    await flush();
    expect(kindOf(container)).toBe('icon');
    answers['pg'] = { img: 'data:image/png;base64,BBBB' };
    rerender(<Thumb item={item({ ...a, version: 2 })} visible />);
    await flush();
    expect(kindOf(container)).toBe('img');
    expect(asked.map((x) => [x.req.how, x.req.v])).toEqual([
      ['page', 1],
      ['page', 2],
    ]);
  });
});
