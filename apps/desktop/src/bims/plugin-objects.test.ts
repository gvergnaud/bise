import { describe, expect, it } from 'vitest';
import type { Plugin, PluginState } from '@/proto';
import { defaultObject, kindOf, objectOf, objectsPrefKey, officeObjects, picksOf, pluginKey, uniqueObjects, unitOf, withPick, type PluginObject, type PluginRow } from './plugin-objects';
import type { ObjId } from './render/objects/ids';

const NONE = new Map();
// main's row: no id, no kind (a real plugin's id is its bare name)
const mainRow = (name: string, state: PluginState = 'loaded', more: Partial<Plugin> = {}): PluginRow => ({ name, state, ...more });
// the plugins branch's row: an id and a kind
const row = (id: string, kind: string, state: PluginState = 'loaded', more: Partial<PluginRow> = {}): PluginRow => ({ id, name: id, kind, state, ...more });

describe('pluginKey', () => {
  it('is the bare name on a row without id, the id on a row with one', () => {
    expect(pluginKey(mainRow('docs'))).toBe('docs');
    expect(pluginKey(row('mistral:gmail', 'mistral'))).toBe('mistral:gmail');
    expect(pluginKey({ id: null, name: 'docs' })).toBe('docs');
  });
});

describe('kindOf', () => {
  it('reads the row kind, else the id prefix, never the scope', () => {
    expect(kindOf(mainRow('docs'))).toBe('plugin');
    expect(kindOf(row('mistral:gmail', 'mistral'))).toBe('connector');
    expect(kindOf({ id: 'mistral:gmail', name: 'mistral:gmail' })).toBe('connector');
    expect(kindOf({ id: 'skills:personal', name: 'skills:personal' })).toBe('skills');
    expect(kindOf(row('linear', 'mcp'))).toBe('mcp');
    expect(kindOf(mainRow('bise'))).toBe('bise');
  });
});

describe('defaultObject', () => {
  it('follows the table by name, a connector by its own name', () => {
    const cases: [PluginRow, string, number?][] = [
      [row('mistral:notion', 'mistral'), 'library'],
      [row('mistral:google_drive', 'mistral'), 'files'],
      [mainRow('google_drive_mcp'), 'files'],
      [row('mistral:granola', 'mistral'), 'visio'],
      [row('mistral:slack', 'mistral'), 'booth'],
      [mainRow('hn'), 'arcade'],
      [mainRow('reddit'), 'pingpong'],
      [mainRow('kit'), 'scooter'],
      [mainRow('bise'), 'cat'],
      [row('mistral:gmail', 'mistral'), 'mailbox'],
      [mainRow('blender'), 'easel'],
      [row('mistral:image_generation', 'mistral'), 'easel'],
      [row('mistral:figma', 'mistral'), 'easel'],
      [row('mistral:linear', 'mistral'), 'blackboard'],
      [row('mistral:atlassian', 'mistral'), 'charts'],
      [row('mistral:datadog_eu', 'mistral'), 'dashboard'],
      [mainRow('computer'), 'macdesk'],
      [mainRow('docs'), 'printer', 1],
      [mainRow('sheets'), 'printer', 2],
      [mainRow('slides'), 'printer', 3],
      [row('mistral:outlook_calendar', 'mistral'), 'calendar'],
      [row('mistral:web_search', 'mistral'), 'telescope'],
      [mainRow('tavily'), 'telescope'],
      [mainRow('bise-stats'), 'trophies'],
      [row('mistral:github_app', 'mistral'), 'packing'],
      [mainRow('sentry'), 'extinguisher'],
      [mainRow('metabase'), 'charts'],
      [mainRow('bigquery'), 'charts'],
      [mainRow('wandb'), 'charts'],
    ];
    for (const [r, obj, n] of cases) expect(defaultObject(r), pluginKey(r)).toEqual(n === undefined ? { obj } : { obj, n });
  });

  it('gives a skills folder a shelf with a book a skill', () => {
    const parts = ['a', 'b', 'c'].map((s) => ({ kind: 'skill', on: s !== 'c' }));
    expect(defaultObject(row('skills:personal', 'skills', 'loaded', { parts }))).toEqual({ obj: 'shelf', n: 3 });
    expect(defaultObject(row('skills:vibe', 'skills'))).toEqual({ obj: 'shelf', n: 1 });
  });

  it('gives an MCP server with no entry a rack, a light per server on', () => {
    const parts = [{ kind: 'mcp', on: true }, { kind: 'mcp', on: true }, { kind: 'mcp', on: false }, { kind: 'skill', on: true }];
    expect(defaultObject(row('postgres', 'mcp', 'loaded', { parts }))).toEqual({ obj: 'rack', n: 2 });
  });

  it('gives anything else a plant whose pot is its kind', () => {
    expect(defaultObject(mainRow('weather'))).toEqual({ obj: 'plant', n: 0 });
    expect(defaultObject(row('mistral:hubspot', 'mistral'))).toEqual({ obj: 'plant', n: 1 });
    expect(defaultObject(mainRow('x'))).toEqual({ obj: 'plant', n: 0 });
  });

  it('never gives a pick-only object by default', () => {
    const only = new Set(['chess', 'piano', 'jukebox', 'aquarium', 'babyfoot', 'dog', 'bike', 'campfire', 'pinball']);
    for (const name of ['chess', 'piano', 'jukebox', 'aquarium', 'babyfoot', 'dog', 'bike', 'campfire', 'pinball', 'x']) {
      expect(only.has(defaultObject(mainRow(name)).obj)).toBe(false);
    }
  });
});

describe('objectOf', () => {
  it('on: the object; disabled: glass; invalid: lit and ✗; shadowed: nothing', () => {
    expect(objectOf(mainRow('docs'), NONE)).toEqual({ key: 'docs', name: 'docs', obj: 'printer', n: 1, glass: false, pill: null, picked: false });
    expect(objectOf(mainRow('docs', 'disabled'), NONE)).toMatchObject({ glass: true, pill: null });
    expect(objectOf(mainRow('docs', 'invalid'), NONE)).toMatchObject({ glass: false, pill: '✗' });
    expect(objectOf(mainRow('docs', 'shadowed'), NONE)).toBeNull();
    expect(objectOf(row('mistral:gmail', 'mistral', 'available'), NONE)).toBeNull();
  });

  it('a login he owes is the ? pill: a connector own login, a part login', () => {
    expect(objectOf(row('linear', 'plugin', 'loaded', { parts: [{ kind: 'mcp', on: true, login: 'in' }] }), NONE)?.pill).toBeNull();
    expect(objectOf(row('mistral:gmail', 'mistral', 'loaded', { login: 'needs' }), NONE)?.pill).toBe('?');
    expect(objectOf(row('linear', 'plugin', 'loaded', { parts: [{ kind: 'mcp', on: true, login: 'needs' }] }), NONE)?.pill).toBe('?');
  });

  it('a failed server marks it broken by its error field, ✗ over ?', () => {
    const parts = [{ kind: 'mcp', on: true, login: 'needs', error: 'exited at start' }];
    expect(objectOf(row('linear', 'plugin', 'loaded', { parts }), NONE)).toMatchObject({ glass: false, pill: '✗' });
    // a server he turned off does not break it
    expect(objectOf(row('linear', 'plugin', 'loaded', { parts: [{ kind: 'mcp', on: false, error: 'x' }] }), NONE)).toMatchObject({ glass: false, pill: null });
  });

  it('his pick wins, the default count only stays on the same object', () => {
    expect(objectOf(mainRow('docs'), new Map([['docs', 'piano']]))).toEqual({ key: 'docs', name: 'docs', obj: 'piano', glass: false, pill: null, picked: true });
    expect(objectOf(mainRow('sheets'), new Map([['sheets', 'printer']]))).toMatchObject({ obj: 'printer', n: 2, picked: true });
  });
});

describe('picks in the prefs', () => {
  it('keys a project', () => {
    expect(objectsPrefKey('shop')).toBe('bims.objects.shop');
  });

  it('reads { key: { obj } }, skips what is not a known object', () => {
    const picks = picksOf({ docs: { obj: 'piano' }, 'mistral:gmail': { obj: 'nope' }, slack: 'booth', x: null, y: { spot: 'a' } });
    expect([...picks]).toEqual([['docs', 'piano']]);
    expect(picksOf(undefined).size).toBe(0);
    expect(picksOf([1, 2]).size).toBe(0);
  });

  it('a stale key is skipped on read and kept on write', () => {
    const v = { gone: { obj: 'piano' } };
    expect(officeObjects([mainRow('docs')], picksOf(v)).map((o) => o.obj)).toEqual(['printer']);
    expect(withPick(v, 'docs', 'chess')).toEqual({ gone: { obj: 'piano' }, docs: { obj: 'chess' } });
  });

  it('a pick keeps the entry other fields; back to the default drops only obj', () => {
    expect(withPick({ docs: { obj: 'piano', spot: 'a:1,2' } }, 'docs', 'chess')).toEqual({ docs: { obj: 'chess', spot: 'a:1,2' } });
    expect(withPick({ docs: { obj: 'piano', spot: 'a:1,2' } }, 'docs', null)).toEqual({ docs: { spot: 'a:1,2' } });
    expect(withPick({ docs: { obj: 'piano' }, b: { obj: 'cat' } }, 'docs', null)).toEqual({ b: { obj: 'cat' } });
    expect(withPick(undefined, 'docs', 'cat')).toEqual({ docs: { obj: 'cat' } });
  });

  it('round-trips through picksOf', () => {
    expect(picksOf(withPick(undefined, 'mistral:gmail', 'dog')).get('mistral:gmail')).toBe('dog');
  });
});

describe('officeObjects', () => {
  it('one per key, by key, the first row winning', () => {
    const rows = [mainRow('slides'), mainRow('docs'), mainRow('docs', 'disabled'), mainRow('x', 'shadowed')];
    expect(officeObjects(rows, NONE).map((o) => [o.key, o.glass])).toEqual([['docs', false], ['slides', false]]);
    expect(officeObjects(null, NONE)).toEqual([]);
  });
});

describe('uniqueObjects: each object once in an office (bims-plugins m_42693)', () => {
  const o = (key: string, obj: ObjId, more: Partial<PluginObject> = {}): PluginObject => ({ key, name: key, obj, glass: false, pill: null, picked: false, ...more });
  const pairs = (objs: readonly PluginObject[]) => objs.map((x) => [x.key, x.obj]);

  it('three printers: one keeps it, one takes the press, the third is not drawn', () => {
    const out = uniqueObjects([o('slides', 'printer', { n: 3 }), o('docs', 'printer', { n: 1 }), o('sheets', 'printer', { n: 2 })]);
    expect(pairs(out)).toEqual([['docs', 'printer'], ['sheets', 'press']]);
    // a replacement has its own look: no count
    expect(out[1]?.n).toBeUndefined();
  });

  it('his pick, then a pill, then lit, then glass keeps a contested object', () => {
    expect(pairs(uniqueObjects([o('a', 'chess', { glass: true }), o('b', 'chess')]))).toEqual([['b', 'chess']]);
    expect(pairs(uniqueObjects([o('a', 'chess'), o('b', 'chess', { pill: '?' })]))).toEqual([['b', 'chess']]);
    expect(pairs(uniqueObjects([o('a', 'chess', { pill: '✗' }), o('b', 'chess', { picked: true })]))).toEqual([['b', 'chess']]);
    // the winner keeps its pill
    expect(uniqueObjects([o('a', 'printer', { pill: '✗' }), o('b', 'printer')]).map((x) => [x.key, x.obj, x.pill])).toEqual([['a', 'printer', '✗'], ['b', 'press', null]]);
  });

  it("an object carries the worst state of the plugins it stands for, '✗' over '?' (designer m_43133)", () => {
    // the failed one keeps the printer, the one waiting on a login takes the press, the fine one is not drawn
    const out = uniqueObjects([o('docs', 'printer'), o('sheets', 'printer', { pill: '?' }), o('slides', 'printer', { pill: '✗' })]);
    expect(out.map((x) => [x.key, x.obj, x.pill])).toEqual([['sheets', 'press', '?'], ['slides', 'printer', '✗']]);
    // his pick keeps the printer, the failed one takes the press; the one not drawn puts its '?' on the printer, and the pill is that plugin's
    const picked = uniqueObjects([o('docs', 'printer', { picked: true }), o('sheets', 'printer', { pill: '?' }), o('slides', 'printer', { pill: '✗' })]);
    expect(picked.map((x) => [x.key, x.obj, x.pill, x.pillOf])).toEqual([['docs', 'printer', '?', 'sheets'], ['slides', 'press', '✗', undefined]]);
    // a better state never hides a worse one
    const kept = uniqueObjects([o('a', 'chess', { picked: true, pill: '✗' }), o('b', 'chess', { pill: '?' })]);
    expect(kept.map((x) => [x.key, x.pill, x.pillOf])).toEqual([['a', '✗', undefined]]);
  });

  it('a plugin keeps its own object before another takes it as a kin', () => {
    // a dashboard plugin (datadog) keeps the dashboard a second charts one would take
    expect(pairs(uniqueObjects([o('a', 'charts'), o('b', 'charts'), o('z', 'dashboard')]))).toEqual([['a', 'charts'], ['z', 'dashboard']]);
  });

  it('a plant once per pot', () => {
    const out = uniqueObjects([o('a', 'plant', { n: 0 }), o('b', 'plant', { n: 0 }), o('c', 'plant', { n: 1 }), o('d', 'plant')]);
    expect(pairs(out)).toEqual([['a', 'plant'], ['c', 'plant']]);
    expect(unitOf({ obj: 'plant' })).toBe('plant:0');
    expect(unitOf({ obj: 'printer', n: 2 })).toBe('printer');
  });

  it('nothing shared: the same objects', () => {
    const objs = [o('a', 'chess'), o('b', 'piano')];
    expect(uniqueObjects(objs)).toEqual(objs);
  });
});
