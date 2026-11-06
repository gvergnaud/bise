import { describe, expect, it } from 'vitest';
import { helper_cmd, helper_ev } from '../../proto/fixtures.gen';
import { decodeEv, encodeCmd, evForWindow, HelperState, LineSplitter, restartDelay, talkOpen, windowMaySend } from './lines';

describe('evForWindow: his selection goes to the capsule alone (architect m_53938)', () => {
  const read = helper_ev.find((e) => e.ev === 'context' && e.context.selection !== undefined);
  it('the capsule gets the context with his selection', () => {
    expect(read).toBeDefined();
    if (read === undefined) return;
    expect(evForWindow(read, true)).toBe(read);
  });
  it('every other window gets it without the selection, the rest unchanged', () => {
    if (read === undefined || read.ev !== 'context') throw new Error('no context fixture with a selection');
    const w = evForWindow(read, false);
    expect(w.ev === 'context' && 'selection' in w.context).toBe(false);
    expect(w).toEqual({ ...read, context: Object.fromEntries(Object.entries(read.context).filter(([k]) => k !== 'selection')) });
  });
  it('other events pass as they are', () => {
    for (const ev of helper_ev.filter((e) => e.ev !== 'context')) expect(evForWindow(ev, false)).toBe(ev);
  });
});

describe("the helper's lines (bise_proto::helper fixtures)", () => {
  it('decodes every fixture event line as itself', () => {
    for (const ev of helper_ev) {
      const d = decodeEv(JSON.stringify(ev));
      expect(d).toEqual({ ok: true, ev });
    }
  });

  it('every fixture command encodes to one JSON line that parses back', () => {
    for (const c of helper_cmd) {
      const l = encodeCmd(c);
      expect(l.endsWith('\n')).toBe(true);
      expect(l.slice(0, -1)).not.toContain('\n');
      expect(JSON.parse(l)).toEqual(c);
    }
  });

  it('refuses junk and drops a newer tag', () => {
    expect(decodeEv('hello').ok).toBe(false);
    expect(decodeEv('[1]').ok).toBe(false);
    expect(decodeEv('{"act":"tap"}').ok).toBe(false);
    expect(decodeEv('{"ev":"pulse","n":1}')).toEqual({ ok: false, why: 'unknown ev pulse' });
  });

  it('a window may send anything but quit', () => {
    expect(helper_cmd.filter((c) => !windowMaySend(c)).map((c) => c.cmd)).toEqual(['quit']);
  });
});

describe('LineSplitter', () => {
  it('joins a line split across chunks and drops blanks', () => {
    const s = new LineSplitter();
    expect(s.push('{"ev":"fn","act":')).toEqual([]);
    expect(s.push('"tap"}\n\n{"ev":"fn",')).toEqual(['{"ev":"fn","act":"tap"}']);
    expect(s.push('"act":"double"}\n')).toEqual(['{"ev":"fn","act":"double"}']);
  });
});

describe('restartDelay', () => {
  it('backs off 1, 2, 4, 8, 16 s, then gives up within the minute', () => {
    const now = 100_000;
    expect(restartDelay([], now)).toBe(1000);
    expect(restartDelay([now - 1000], now)).toBe(2000);
    expect(restartDelay([1, 2, 3, 4].map((i) => now - i * 1000), now)).toBe(16_000);
    expect(restartDelay([1, 2, 3, 4, 5].map((i) => now - i * 1000), now)).toBeNull();
  });

  it('old crashes no longer count', () => {
    expect(restartDelay([0, 1, 2, 3, 4], 200_000)).toBe(1000);
  });
});

describe('HelperState', () => {
  it('keeps the last state event of each kind, never fn or context', () => {
    const s = new HelperState();
    for (const ev of helper_ev) s.take(ev);
    const tags = s.snapshot().map((e) => e.ev);
    expect(new Set(tags)).toEqual(new Set(['hello', 'front', 'perms', 'quiet']));
    expect(s.snapshot().find((e) => e.ev === 'quiet')).toEqual({ ev: 'quiet', on: false, why: '' });
    s.clear();
    expect(s.snapshot()).toEqual([]);
  });
});

describe('talkOpen', () => {
  it('a hold or fn fn opens a talk, its end or a cancel closes it; other fn acts and other events leave it (amb-core m_22285)', () => {
    const fn = (act: 'talk_start' | 'talk_end' | 'cancel' | 'tap' | 'double' | 'key') => ({ ev: 'fn', act }) as const;
    expect(talkOpen(false, fn('talk_start'))).toBe(true);
    expect(talkOpen(false, fn('double'))).toBe(true);
    expect(talkOpen(true, fn('talk_end'))).toBe(false);
    expect(talkOpen(true, fn('cancel'))).toBe(false);
    expect(talkOpen(true, fn('key'))).toBe(true);
    expect(talkOpen(false, fn('tap'))).toBe(false);
    expect(talkOpen(true, { ev: 'quiet', on: false } as never)).toBe(true);
  });
});
