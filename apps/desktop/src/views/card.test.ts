// A tool row's views (C4's ToolItem.views): one card each, in order, keyed by its call; none
// without; a call twice is one card.
import { describe, expect, it } from 'vitest';
import type { ToolItem } from '@/proto';
import { viewsOf, type ToolView } from './Card';

const item = { pos: 1, id: 4, at_ms: 0, text: 'run_typescript', kind: 'other', state: 'ok' } as unknown as ToolItem;
const v = (call: string): ToolView => ({ plugin: 'fake-app', server: 'fake-app', uri: 'ui://fake-app/counter', call, state: 'live' });

describe('viewsOf', () => {
  it('every view of the call, in order, each call once', () => {
    expect(viewsOf({ ...item, views: [v('1-4'), v('1-5'), v('1-4')] } as ToolItem)).toEqual([v('1-4'), v('1-5')]);
  });

  it('none: no views, an empty list, a view with no call', () => {
    expect(viewsOf(item)).toEqual([]);
    expect(viewsOf({ ...item, views: [] } as ToolItem)).toEqual([]);
    expect(viewsOf({ ...item, views: [{ ...v(''), call: '' }] } as ToolItem)).toEqual([]);
  });
});
