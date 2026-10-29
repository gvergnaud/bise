// A tool row's views (C4's ToolItem.views): one card each, in order, keyed by its call; none
// without; a call twice is one card. The hub's app_open names the view whose tab opens.
import { describe, expect, it } from 'vitest';
import type { ToolItem } from '@/proto';
import { viewsOf, waitsOnHim, type ToolView } from './Card';
import { viewOpenOf } from './hooks';

const item = { pos: 1, id: 4, at_ms: 0, text: 'run_typescript', kind: 'other', state: 'ok' } as unknown as ToolItem;
const v = (call: string): ToolView => ({ plugin: 'fake-app', server: 'fake-app', uri: 'ui://fake-app/counter', call, state: 'live' });

describe('viewsOf', () => {
  it('every view of the call, in order, each call once', () => {
    expect(viewsOf({ ...item, views: [v('1-4'), v('1-5'), v('1-4')] })).toEqual([v('1-4'), v('1-5')]);
  });

  it('none: no views, an empty list, a view with no call', () => {
    expect(viewsOf(item)).toEqual([]);
    expect(viewsOf({ ...item, views: [] })).toEqual([]);
    expect(viewsOf({ ...item, views: [{ ...v(''), call: '' }] })).toEqual([]);
  });
});

describe("the hub's app_open", () => {
  it('a view with its agent and call opens; anything else is not a view to open', () => {
    expect(viewOpenOf({ ev: 'app_open', project: 'p', what: 'view', agent: 'a', id: '1-1' })).toEqual({ project: 'p', agent: 'a', call: '1-1' });
    expect(viewOpenOf({ ev: 'app_open', project: 'p', what: 'view', agent: 'a' })).toBeNull();
    expect(viewOpenOf({ ev: 'app_open', project: 'p', what: 'thread', agent: 'a', id: '1-1' })).toBeNull();
    expect(viewOpenOf({ ev: 'card_open', project: 'p', id: 3 })).toBeNull();
  });
});

describe('waitsOnHim', () => {
  it('an open notification placed on this view call waits on him (C5)', () => {
    const cards = [{ place: 'wt:dark' }, { place: 'view:main/1-4' }, { place: null }, {}];
    expect(waitsOnHim(cards, 'main', '1-4')).toBe(true);
  });

  it('another call, another agent, or no place: nothing waits', () => {
    expect(waitsOnHim([{ place: 'view:main/1-5' }], 'main', '1-4')).toBe(false);
    expect(waitsOnHim([{ place: 'view:perf/1-4' }], 'main', '1-4')).toBe(false);
    expect(waitsOnHim([{ place: null }, {}], 'main', '1-4')).toBe(false);
  });
});
