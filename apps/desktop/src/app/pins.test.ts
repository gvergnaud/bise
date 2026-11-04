// Pins (d5): his pinned agents read from the prefs, a pin and an unpin as the list prefs_set
// writes, a renamed agent's old key dropped.
import { describe, expect, it } from 'vitest';
import { isPinned, namesOf, pinFirst, pinKeyOf, pinned, pinnedIn, pinSetOf, pinsKey, pinsOf, procPinId, watchPinId } from './pins';

describe('pinsOf', () => {
  it('reads every project’s pins in pin order, strings only, each once', () => {
    const prefs = { pins: { acme: ['perf', 'docs', 'perf', 3], shop: ['gift-cards'] }, theme: 'dark' };
    expect(pinsOf(prefs)).toEqual([
      { project: 'acme', agent: 'perf' },
      { project: 'acme', agent: 'docs' },
      { project: 'shop', agent: 'gift-cards' },
    ]);
  });

  it('reads nothing from no prefs or a broken value', () => {
    expect(pinsOf(null)).toEqual([]);
    expect(pinsOf({ pins: ['perf'] })).toEqual([]);
    expect(namesOf('perf')).toEqual([]);
  });
});

describe('pinned', () => {
  it('adds a pin at the end, once', () => {
    expect(pinned(['docs'], 'perf', true)).toEqual(['docs', 'perf']);
    expect(pinned(['perf', 'docs'], 'perf', true)).toEqual(['docs', 'perf']);
  });

  it('removes an unpinned agent', () => {
    expect(pinned(['docs', 'perf'], 'perf', false)).toEqual(['docs']);
  });

  it('drops the names no longer in the rows (a rename’s old key)', () => {
    expect(pinned(['old-name', 'docs'], 'perf', true, ['docs', 'perf'])).toEqual(['docs', 'perf']);
  });
});

describe('isPinned', () => {
  it('is true for a pinned agent of that project only', () => {
    const pins = [{ project: 'acme', agent: 'perf' }];
    expect(isPinned(pins, 'acme', 'perf')).toBe(true);
    expect(isPinned(pins, 'acme', 'docs')).toBe(false);
    expect(isPinned(pins, 'shop', 'perf')).toBe(false);
  });
});

describe('every kind (his m_49100)', () => {
  it('keeps one key per kind and project, the agents’ unchanged', () => {
    expect(pinsKey('acme')).toBe('pins.acme');
    expect(pinKeyOf('artifact', 'acme')).toBe('pins_art.acme');
    expect(pinKeyOf('schedule', 'acme')).toBe('pins_sched.acme');
    expect(pinKeyOf('process', 'acme')).toBe('pins_proc.acme');
    expect(procPinId('perf', 'bg3.1791100100')).toBe('perf/bg3.1791100100');
  });

  it('reads one kind’s pins per project, never another kind’s', () => {
    const prefs = { pins: { acme: ['perf'] }, pins_art: { acme: ['a1', 'a2'], shop: [] }, pins_proc: { acme: ['perf/bg3.1'] } };
    const art = pinSetOf(prefs, 'artifact');
    expect([...art]).toEqual([['acme', ['a1', 'a2']]]);
    expect(pinnedIn(art, 'acme', 'a2')).toBe(true);
    expect(pinnedIn(art, 'shop', 'a2')).toBe(false);
    // the same id in another project is another pin
    expect(pinnedIn(pinSetOf(prefs, 'process'), 'shop', 'perf/bg3.1')).toBe(false);
    expect(pinsOf(prefs)).toEqual([{ project: 'acme', agent: 'perf' }]);
    expect([...pinSetOf(null, 'schedule')]).toEqual([]);
  });
});

describe('pinFirst: a stable partition, never a sort (architect m_49104)', () => {
  const rows = ['e', 'b', 'd', 'a', 'c'];
  it('is the rows themselves with no pin', () => {
    expect(pinFirst(rows, () => false)).toBe(rows);
  });
  it('puts the pinned first, each group in the order the rows came', () => {
    const on = new Set(['a', 'd']);
    const out = pinFirst(rows, (r) => on.has(r));
    expect(out).toEqual(['d', 'a', 'e', 'b', 'c']);
    // the same rows, none lost or doubled
    expect([...out].sort()).toEqual([...rows].sort());
  });
  it('keeps a row the Nth it shows: index N of the output is the Nth row drawn', () => {
    const out = pinFirst(rows, (r) => r === 'c');
    expect(out.indexOf('c')).toBe(0);
    expect(out.slice(1)).toEqual(['e', 'b', 'd', 'a']);
  });
});

describe('a watch’s pin (ui-review m_49214)', () => {
  it('has its own id beside the timers’, so a watch and a timer of the same number never share a pin', () => {
    expect(watchPinId(4)).toBe('watch:4');
    const set = pinSetOf({ pins_sched: { acme: ['watch:4'] } }, 'schedule');
    expect(pinnedIn(set, 'acme', watchPinId(4))).toBe(true);
    expect(pinnedIn(set, 'acme', '4')).toBe(false);
  });
});
