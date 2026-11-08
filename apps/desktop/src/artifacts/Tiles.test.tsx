// @vitest-environment happy-dom
// A theme's big tile reads as a folder, never as an artifact card (page folder-tiles, pick A): a tab
// on top of its cover, the folder glyph before its name, its count in words ('32 artifacts').
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { library } from '@/proto/words_library.gen';
import { BigTile, type TileProps } from './Tiles';
import type { Count } from './folders';

afterEach(cleanup);

const tile = (count: Count | undefined, extra: Partial<TileProps> = {}) => {
  render(<BigTile folder={{ id: 'f-loom', name: 'loom-like', by: 'you', named_by_you: true }} count={count} when="nov 7"
    flash={false} onOpen={() => {}} onMenu={() => {}} testId="tile" {...extra} />);
  return screen.getByTestId('tile');
};

describe('BigTile', () => {
  it('draws a folder: a tab above the cover, the folder glyph before the name', () => {
    const el = tile({ n: 32, total: 32, fresh: 0 });
    const tab = el.querySelector(':scope > .tab');
    expect(tab?.nextElementSibling?.classList.contains('cover')).toBe(true);
    expect(tab?.getAttribute('aria-hidden')).toBe('true');
    const nm = el.querySelector('.nm');
    expect(nm?.firstElementChild?.getAttribute('data-icon')).toBe('folder');
    expect(nm?.textContent).toBe('loom-like');
  });

  it('says its count in artifacts, one, many, filtered or none', () => {
    const line = (c: Count | undefined) => { const t = tile(c).querySelector('.ln')?.textContent; cleanup(); return t; };
    expect(line({ n: 32, total: 32, fresh: 0 })).toBe(`${library.n_artifacts({ n: 32 })} · nov 7`);
    expect(line({ n: 1, total: 1, fresh: 0 })).toBe(`${library.n_artifacts({ n: 1 })} · nov 7`);
    expect(line({ n: 3, total: 12, fresh: 0 })).toBe(`${library.of_total({ n: 3, total: 12 })} · nov 7`);
    expect(line(undefined)).toBe(`${library.tile_empty} · nov 7`);
  });
});
