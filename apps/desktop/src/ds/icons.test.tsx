// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { extOf, iconKindOf, KindIcon, PlayIcon, type IconKind } from './icons';
import { AppIcon } from './icons-app';

describe('iconKindOf', () => {
  const sheets = { icon: 'sheet', builtin: true };
  const cases: [Parameters<typeof iconKindOf>[0], IconKind][] = [
    // a file: its app's icon, as the hub picked it (bise's types: a ds name; never its ending)
    [{ kind: 'file', path: '/a/budget.xlsx', opener: sheets }, 'sheet'],
    [{ kind: 'shots', path: '/a/b.png', opener: { icon: 'image', builtin: true } }, 'image'],
    [{ kind: 'page', url: 'http://127.0.0.1:47438/p/x', opener: { icon: 'page', builtin: true } }, 'page'],
    // a plugin's own app (its /icon/<plugin> route), an unknown ds name: a file
    [{ kind: 'file', path: '/a/q.fake', opener: { icon: '/icon/fake-opener' } }, 'file'],
    [{ kind: 'file', path: '/a/q.xlsx', opener: { icon: 'spreadsheet', builtin: true } }, 'file'],
    // no app: the ending says nothing, the kind word does
    [{ kind: 'file', path: '/a/budget.xlsx' }, 'file'],
    [{ kind: 'file', path: '/a/appart.glb' }, 'file'],
    [{ kind: 'shots', path: '/a/b' }, 'image'],
    [{ kind: 'slides' }, 'deck'],
    [{ kind: 'link', url: 'https://drive.google.com/x' }, 'link'],
    [{ kind: 'link', url: 'https://example.com/report.pdf' }, 'link'],
    [{ kind: 'pr', url: 'https://github.com/a/b/pull/3' }, 'pr'],
    [{ kind: 'release', url: 'https://github.com/a/b/releases/tag/v1' }, 'release'],
    [{ kind: 'site', target: '/w/site/index.html' }, 'page'],
    [{ kind: 'whatever', url: 'https://x.dev' }, 'link'],
    [{ kind: 'whatever' }, 'file'],
  ];
  it.each(cases)('%j → %s', (a, want) => {
    expect(iconKindOf(a)).toBe(want);
  });
});

describe('extOf', () => {
  it('reads the path, then the target, then a url with a file', () => {
    expect(extOf({ kind: 'file', path: '/x/a.tar.GZ' })).toBe('gz');
    expect(extOf({ kind: 'file', target: '/x/a.md' })).toBe('md');
    expect(extOf({ kind: 'link', url: 'https://x.dev/a/b.pdf?dl=1' })).toBe('pdf');
    expect(extOf({ kind: 'link', url: 'https://drive.google.com/drive/folders/1' })).toBe('');
    expect(extOf({ kind: 'link', url: 'https://x.dev' })).toBe('');
  });
});

describe('KindIcon', () => {
  it('draws a 16 px line icon in currentColor, stroke 1.5, for every kind', () => {
    const kinds: IconKind[] = ['page', 'doc', 'sheet', 'deck', 'image', 'video', 'audio', '3d', 'file', 'link', 'pr', 'release'];
    for (const k of kinds) {
      const html = renderToStaticMarkup(<KindIcon kind={k} />);
      expect(html).toContain('width="16"');
      expect(html).toContain('stroke="currentColor"');
      expect(html).toContain('stroke-width="1.5"');
      expect(html).toContain(`data-kind="${k}"`);
    }
    expect(renderToStaticMarkup(<PlayIcon />)).toContain('fill="currentColor"');
  });
});

describe('AppIcon', () => {
  it('draws a circle part as a circle and a filled part as a filled path (pin-on, artifact-pin)', () => {
    // an array passes `'fill' in p` (Array.prototype.fill): the ask's ring once drew as an empty path
    expect(renderToStaticMarkup(<AppIcon name="ask" />)).toContain('<circle cx="12" cy="12" r="9"');
    const on = renderToStaticMarkup(<AppIcon name="pin-on" />);
    expect(on).toContain('<path d="M9 4h6l-1 6 3 3H7l3-3-1-6z" fill="currentColor"');
    expect(renderToStaticMarkup(<AppIcon name="pin" />)).not.toContain('fill="currentColor"');
  });
});
