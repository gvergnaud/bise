// The composer's pure parts: triggers, the send's text, drafts, the picker's rows.
import { describe, expect, it } from 'vitest';
import { argAt, browseText, filePick, parentQuery, prefixFirst, triggerAt } from './tokens';
import { emojiToken, filterEmoji, get, typedEmoji, all as allEmoji, EMOJI_MAX } from './emoji';
import { agoWords, artifactRows, emojiRows, fileRows, pickedChip } from './pick';
import TUI_EMOJI from '../../../../rust/tui/src/emoji.tsv?raw';
import DESKTOP_EMOJI from './emoji.tsv?raw';
import COMPOSER_PARTS from '../../../../rust/proto/fixtures/composer_parts.json';
import { noVisionLine } from './message';
import { isImagePath } from './sources';
import { colorLines, cssText, proseRuns } from './mdcolor';
import { codeLines } from '@/thread/Code';
import { buildMessage, chipStrip, fileChipName, isLongPaste, lineCount, nextChipNumber, type Chip, type Segment } from './message';
import { load, remember, save, HISTORY_MAX, type Store } from './drafts';
import { argRows, commandLine, commandRows, completion, enterSends, move, type Row } from './pick';
import { enterMakesNewline, indent, item, newline, outdentAtStart, subwordLeft, subwordRight, type Op } from './mdlive';

const memStore = (): Store & { m: Map<string, string> } => {
  const m = new Map<string, string>();
  return { m, getItem: (k) => m.get(k) ?? null, setItem: (k, v) => void m.set(k, v), removeItem: (k) => void m.delete(k) };
};

describe('triggers (the TUI rules)', () => {
  it('@ and $ open a word at a line start or after a space, not inside a word', () => {
    expect(triggerAt('look at @src/ma', false)).toMatchObject({ kind: 'at', query: 'src/ma', start: 8 });
    expect(triggerAt('$rev', false)).toMatchObject({ kind: 'skill', query: 'rev', start: 0 });
    expect(triggerAt('mail me@home', false)).toBeNull();
    expect(triggerAt('cost 5$', false)).toBeNull();
    expect(triggerAt('done @x ', false)).toBeNull();
  });
  it('/ only at the start of the message, one word', () => {
    expect(triggerAt('/mod', true)).toMatchObject({ kind: 'slash', query: 'mod' });
    expect(triggerAt('/mod', false)).toBeNull();
    expect(triggerAt('see /usr/bin', true)).toBeNull();
    expect(triggerAt('/usr/bin', true)).toBeNull();
  });
  it('@" opens a quoted path that may hold spaces', () => {
    expect(triggerAt('read @"docs/my no', false)).toMatchObject({ kind: 'at', query: 'docs/my no', quoted: true, start: 5 });
  });
  it('a pick inserts the bare path, quoted with a space; a folder keeps browsing', () => {
    expect(filePick('src/app.ts', false)).toEqual({ text: 'src/app.ts ', browse: false });
    expect(filePick('my notes/a.md', false)).toEqual({ text: '"my notes/a.md" ', browse: false });
    expect(filePick('src', true)).toEqual({ text: '@src/', browse: true });
  });
  it('prefix matches come before substring matches, each in order', () => {
    expect(prefixFirst(['xreview', 'review', 'rev2', 'other'], 'rev', (s) => s)).toEqual(['review', 'rev2', 'xreview']);
  });
});

describe('the send', () => {
  it('one typed message, the same parts from both composers (architect m_22426: the TUI runs the same fixture)', () => {
    const cases = COMPOSER_PARTS as unknown as { name: string; window: Segment[]; parts: unknown[] }[];
    for (const c of cases) expect(buildMessage(c.window).parts, c.name).toEqual(c.parts);
  });

  it("his m_22231: 'regarding [Image #1] and [Image #2] and ❝ 3' keeps its order in the parts", () => {
    const out = buildMessage([
      { kind: 'text', text: 'regarding ' },
      { kind: 'chip', chip: { kind: 'image', n: 1, path: '/i/a.png', name: 'a.png' } },
      { kind: 'text', text: ' and ' },
      { kind: 'chip', chip: { kind: 'image', n: 2, path: '/i/b.png', name: 'b.png' } },
      { kind: 'text', text: ' and ' },
      { kind: 'chip', chip: { kind: 'quote', n: 3, text: 'the cache is warm', from: 'main' } },
      { kind: 'text', text: ', which is right?' },
    ]);
    expect(out.text).toBe('regarding [Image #1] and [Image #2] and ❝ 3, which is right?');
    expect(out.parts?.map((p) => (p.kind === 'text' ? p.text : p.kind === 'image' ? p.name : p.kind === 'quote' ? `q${String(p.n)}` : p.kind))).toEqual([
      'regarding ',
      '[Image #1]',
      ' and ',
      '[Image #2]',
      ' and ',
      'q3',
      ', which is right?',
    ]);
  });

  it("one counter (architect m_22248 (3)): a message's chip numbers are unique across quotes, pastes, images and files", () => {
    let segs: Segment[] = [];
    const kinds = ['quote', 'paste', 'image', 'file', 'quote', 'image', 'paste', 'file'] as const;
    for (const k of kinds) {
      const n = nextChipNumber(segs);
      const chip =
        k === 'quote'
          ? ({ kind: 'quote', n, text: `q${String(n)}`, from: 'main' } as const)
          : k === 'paste'
            ? ({ kind: 'paste', n, text: `p${String(n)}` } as const)
            : k === 'image'
              ? ({ kind: 'image', n, path: `/i/${String(n)}.png`, name: `${String(n)}.png` } as const)
              : ({ kind: 'file', n, path: `/f/${String(n)}.md`, name: `${String(n)}.md` } as const);
      segs = [...segs, { kind: 'text', text: ' x ' }, { kind: 'chip', chip }];
    }
    const ns = (buildMessage(segs).parts ?? []).flatMap((p) => {
      if (p.kind === 'quote' || p.kind === 'paste') return p.n === undefined ? [] : [String(p.n)];
      if (p.kind === 'image' || p.kind === 'file') return [/#(\d+)/u.exec(p.name ?? '')?.[1] ?? ''];
      return [];
    });
    expect(ns).toHaveLength(kinds.length);
    expect(new Set(ns).size).toBe(ns.length);
  });

  it('a quote, a paste or an image: typed parts, each at its place (a quote with its number, his m_22231; an image by its source), the text his words with labels, never a template or files', () => {
    const segs: Segment[] = [
      { kind: 'text', text: '  what about ' },
      { kind: 'chip', chip: { kind: 'paste', n: 2, text: 'a\nb' } },
      { kind: 'text', text: ' and ' },
      { kind: 'chip', chip: { kind: 'image', n: 3, path: '/tmp/x.png', name: 'x.png' } },
      { kind: 'chip', chip: { kind: 'quote', n: 1, text: 'fn main()', from: 'main' } },
      { kind: 'text', text: '?\n' },
    ];
    expect(buildMessage(segs)).toEqual({
      // the text: his words with the chips' labels, never the model's template (delta review D2)
      text: 'what about ▤ 2 and [Image #3] ❝ 1?',
      parts: [
        { kind: 'text', text: 'what about ' },
        { kind: 'paste', n: 2, text: 'a\nb' },
        { kind: 'text', text: ' and ' },
        { kind: 'image', name: '[Image #3]', source: '/tmp/x.png' },
        { kind: 'text', text: ' ' },
        { kind: 'quote', from: 'main', text: 'fn main()', n: 1 },
        { kind: 'text', text: '?' },
      ],
    });
    // an image alone with words: typed too (main 18a32ce3: the hub stores it from its source)
    expect(buildMessage([{ kind: 'chip', chip: { kind: 'image', n: 1, path: '/i/a.png', name: 'a.png' } }, { kind: 'text', text: 'what is wrong' }])).toEqual({
      text: '[Image #1] what is wrong',
      parts: [
        { kind: 'image', name: '[Image #1]', source: '/i/a.png' },
        { kind: 'text', text: ' what is wrong' },
      ],
    });
    expect(nextChipNumber(segs)).toBe(4);
  });
  it('a paste of 12 lines or 1200 characters is long', () => {
    expect(isLongPaste('x\n'.repeat(11))).toBe(false);
    expect(isLongPaste('x\n'.repeat(12))).toBe(true);
    expect(isLongPaste('é'.repeat(1200))).toBe(true);
    expect(lineCount('a\nb\n')).toBe(2);
  });
  it('file-chips: a file chip is a file part where it was, next to an image, by its absolute path', () => {
    const seg: Segment[] = [
      { kind: 'text', text: "ajouter l'image" },
      { kind: 'chip', chip: { kind: 'image', n: 1, path: '/h/images/logo.png', name: 'logo.png' } },
      { kind: 'text', text: 'dans la presentation powerpoint' },
      { kind: 'chip', chip: { kind: 'file', n: 2, path: '/Users/me/Desktop/deck.pptx', bytes: 2_100_000 } },
    ];
    const out = buildMessage(seg);
    expect(out.parts).toEqual([
      { kind: 'text', text: "ajouter l'image " },
      { kind: 'image', name: '[Image #1]', source: '/h/images/logo.png' },
      { kind: 'text', text: ' dans la presentation powerpoint ' },
      { kind: 'file', name: '[File #2]', path: '/Users/me/Desktop/deck.pptx' },
    ]);
    expect(out.text).toBe("ajouter l'image [Image #1] dans la presentation powerpoint [File #2]");
    expect(chipStrip({ kind: 'file', n: 2, path: '/Users/me/Desktop/deck.pptx', bytes: 2_100_000 })).toBe('▯ 2  deck.pptx  2.1 MB');
  });
  it("at-chips: an @-picked file is its chip by its absolute path, an image's or a file's, code too; the send carries it where it was", () => {
    const root = '/Users/me/acme/';
    const img = pickedChip(root, 'shots/logo.png', 1);
    const code = pickedChip(root, 'computer-use/extension/test/e2e.mjs', 2);
    expect(img).toEqual({ kind: 'image', n: 1, path: '/Users/me/acme/shots/logo.png', name: 'logo.png' });
    expect(code).toEqual({ kind: 'file', n: 2, path: '/Users/me/acme/computer-use/extension/test/e2e.mjs' });
    expect(pickedChip(root, '../other/a.rs', 3)).toMatchObject({ kind: 'file', path: '/Users/me/other/a.rs' });
    expect(pickedChip(root, '/abs/x.pdf', 4)).toMatchObject({ kind: 'file', path: '/abs/x.pdf' });
    const out = buildMessage([{ kind: 'text', text: 'voir ' }, { kind: 'chip', chip: img }, { kind: 'text', text: ' et ' }, { kind: 'chip', chip: code }, { kind: 'text', text: ' stp' }]);
    expect(out.parts).toEqual([
      { kind: 'text', text: 'voir ' },
      { kind: 'image', name: '[Image #1]', source: '/Users/me/acme/shots/logo.png' },
      { kind: 'text', text: ' et ' },
      { kind: 'file', name: '[File #2]', path: '/Users/me/acme/computer-use/extension/test/e2e.mjs' },
      { kind: 'text', text: ' stp' },
    ]);
  });
  it("a file chip's words: its name, cut at 24 with a glued '…' (bise_proto::parts file_chip_name)", () => {
    expect(fileChipName('/w/computer-use/extension/test/e2e.mjs')).toBe('e2e.mjs');
    expect(fileChipName('/w/a-very-long-file-name-for-the-chip.rs')).toBe('a-very-long-file-name-f…');
  });
  it('the one chip table (bise_proto::parts): images by extension, any other file a file chip', () => {
    expect(['a.png', 'b.JPG', 'c.webp'].every(isImagePath)).toBe(true);
    expect(['src/main.rs', 'deck.pptx', '.png', 'Makefile'].some(isImagePath)).toBe(false);
  });
  it('the strip says what a chip holds', () => {
    expect(chipStrip({ kind: 'quote', n: 1, text: 'one\ntwo\nthree', from: 'main' })).toBe('❝ 1 “one two three”  bise · 3 lines');
    expect(chipStrip({ kind: 'paste', n: 2, text: 'x\n'.repeat(240) })).toMatch(/^▤ 2 “x x x .*” {3}240 lines · 0\.5 kB$/u);
  });
});

describe('drafts', () => {
  it('one per target, kept across a reload, unreadable reads as empty', () => {
    const s = memStore();
    save(s, 'p/main', { draft: '{"a":1}', history: [] });
    expect(load(s, 'p/main').draft).toBe('{"a":1}');
    expect(load(s, 'p/other').draft).toBeNull();
    s.setItem('bise.composer.v1.p/bad', '{nope');
    expect(load(s, 'p/bad')).toEqual({ draft: null, history: [] });
    save(s, 'p/main', { draft: null, history: [] });
    expect(s.m.size).toBe(1);
  });
  it('history is newest first, deduplicated, 50 kept', () => {
    let st = load(memStore(), 't');
    for (let i = 0; i < 60; i += 1) st = remember(st, `m${i}`);
    st = remember(st, 'm55');
    expect(st.history.length).toBe(HISTORY_MAX);
    expect(st.history.slice(0, 2)).toEqual(['m55', 'm59']);
  });
});

describe('the / list', () => {
  const cmds = [
    { name: '/model', desc: 'set the model', args: true, ready: true },
    { name: '/compact', desc: 'compact', args: false, ready: true },
    { name: '/inbox', desc: 'the inbox', args: false, ready: false },
  ];
  it('filters by prefix; a not-yet command shows greyed', () => {
    const t = { kind: 'slash', query: '', start: 0, quoted: false } as const;
    const rows = commandRows(t, cmds);
    expect(rows.map((r) => r.label)).toEqual(['/model', '/compact', '/inbox']);
    expect(rows[2]).toMatchObject({ off: true, desc: 'the inbox · not yet' });
    expect(commandRows({ ...t, query: 'co' }, cmds).map((r) => r.label)).toEqual(['/compact']);
  });
  it('⏎ runs a command without arguments, tab only completes', () => {
    const [, compact] = commandRows({ kind: 'slash', query: '', start: 0, quoted: false }, cmds);
    expect(compact?.pick).toEqual({ kind: 'run', name: '/compact' });
    expect(compact && completion(compact)).toEqual({ kind: 'insert', text: '/compact ', browse: false });
  });
  it('after a command, its argument popup: fixed words here, listed kinds from the screen', () => {
    const withArgs = [
      { name: '/model', desc: 'm', args: true, ready: true, argKinds: [{ words: [{ value: 'opus', desc: 'big' }, { value: 'haiku', desc: 'small' }] }] },
      { name: '/archive', desc: 'a', args: true, ready: true, argKinds: ['agent' as const] },
      { name: '/new', desc: 'n', args: true, ready: true, argKinds: ['text' as const, 'agent' as const] },
    ];
    const is = (n: string): boolean => withArgs.some((c) => c.name === n);
    const text = (n: string, i: number): boolean => withArgs.find((c) => c.name === n)?.argKinds[i] === 'text';
    const t = argAt('/model op', true, is, text);
    expect(t).toMatchObject({ kind: 'arg', command: '/model', index: 0, query: 'op', start: 7 });
    expect(t && argRows(t, withArgs, {}).map((r) => r.pick)).toEqual([{ kind: 'insert', text: 'opus ', browse: false }]);
    const a = argAt('/archive p', true, is, text);
    expect(a && argRows(a, withArgs, { choices: (k, q) => (k === 'agent' ? [{ value: `perf-${q}`, desc: '' }] : []) }).map((r) => r.label)).toEqual(['perf-p']);
    expect(argAt('/new fix the build', true, is, text)).toBeNull();
    expect(argAt('/model op', false, is, text)).toBeNull();
    expect(argAt('/nope x', true, is, text)).toBeNull();
  });
  it('⏎ sends a command whose argument is already typed whole; otherwise it picks (qa-flows 28a5419b)', () => {
    const withArgs = [
      { name: '/close', desc: 'c', args: true, ready: true, argKinds: ['card' as const, 'text' as const] },
      { name: '/archive', desc: 'a', args: true, ready: true, argKinds: ['agent' as const] },
      { name: '/flow', desc: 'f', args: true, ready: true, argKinds: [{ words: [{ value: 'pr', desc: '' }, { value: 'trunk', desc: '' }] }] },
    ];
    const is = (n: string): boolean => withArgs.some((c) => c.name === n);
    const text = (): boolean => false;
    const choices = (k: string, q: string): { value: string; desc: string }[] =>
      (k === 'card' ? ['1', '12'] : k === 'agent' ? ['perf', 'perf2'] : []).filter((v) => v.startsWith(q)).map((value) => ({ value, desc: '' }));
    const top = (line: string): [ReturnType<typeof argAt>, Row | undefined] => {
      const t = argAt(line, true, is, text);
      return [t, t === null ? undefined : argRows(t, withArgs, { choices })[0]];
    };
    for (const line of ['/close 1', '/archive perf', '/flow pr']) expect(enterSends(...top(line)), line).toBe(true);
    for (const line of ['/close ', '/archive pe', '/flow t']) expect(enterSends(...top(line)), line).toBe(false);
    expect(enterSends({ kind: 'slash', query: 'close', start: 0, quoted: false }, commandRows({ kind: 'slash', query: 'close', start: 0, quoted: false }, cmds)[0])).toBe(false);
    expect(enterSends(null, undefined)).toBe(false);
  });
  it('a typed command line runs; an unknown /word is text', () => {
    expect(commandLine('/model opus ', cmds)).toEqual({ name: '/model', args: 'opus', ready: true });
    expect(commandLine('/usr/bin is odd', cmds)).toBeNull();
    expect(move(0, -1, 3)).toBe(2);
  });
});

const applied = (text: string, op: Op | null): [string, number] | null =>
  op === null ? null : [text.slice(0, op.from) + op.insert + text.slice(op.to), op.caret];

describe('live markdown keys (mdlive.rs)', () => {
  it('⏎ in a ``` block is a newline that keeps the indentation; outside it sends', () => {
    const t = '```\n  let x = 1';
    expect(enterMakesNewline(t, t.length)).toBe(true);
    expect(applied(t, newline(t, t.length))).toEqual(['```\n  let x = 1\n  ', t.length + 3]);
    expect(enterMakesNewline('```\nx\n```', 9)).toBe(false);
    expect(enterMakesNewline('plain', 5)).toBe(false);
    // the opening fence's line is code too (mdlive.rs enter_makes_newline('```ts', 5)); a tagged
    // fence inside a block is code, a bare one closes it
    expect(enterMakesNewline('```ts', 5)).toBe(true);
    expect(applied('  ```rust', newline('  ```rust', 9))?.[0]).toBe('  ```rust\n  ');
    expect(enterMakesNewline('```\n```py\nx', 12)).toBe(true);
    expect(enterMakesNewline('```\nx\n```\nafter', 15)).toBe(false);
  });
  it('⇧⏎ continues a list, numbers count up, a checkbox comes back empty', () => {
    expect(applied('- one', newline('- one', 5))).toEqual(['- one\n- ', 8]);
    expect(applied('3. three', newline('3. three', 8))?.[0]).toBe('3. three\n4. ');
    expect(applied('- [x] done', newline('- [x] done', 10))?.[0]).toBe('- [x] done\n- [ ] ');
    expect(newline('just words', 10)).toBeNull();
  });
  it('an empty item steps out a level, or ends the list at the top', () => {
    expect(applied('- a\n  - ', newline('- a\n  - ', 8))?.[0]).toBe('- a\n- ');
    expect(applied('- a\n- ', newline('- a\n- ', 6))).toEqual(['- a\n', 4]);
  });
  it('tab indents an item by its marker width; backspace at its text start outdents, then drops the bullet', () => {
    expect(applied('- a\n- b', indent('- a\n- b', 7))).toEqual(['- a\n  - b', 9]);
    expect(applied('1. a\n1. b', indent('1. a\n1. b', 9))?.[0]).toBe('1. a\n   1. b');
    expect(indent('no list', 3)).toBeNull();
    expect(applied('- a\n  - b', outdentAtStart('- a\n  - b', 8))).toEqual(['- a\n- b', 6]);
    expect(applied('- b', outdentAtStart('- b', 2))).toEqual(['b', 0]);
    expect(outdentAtStart('- b', 3)).toBeNull();
    expect(item('* [ ] x')).toMatchObject({ indent: 0, bullet: '*', check: false, content: 6 });
  });
  it('subword moves stop at camelCase, snake_case, digits', () => {
    const t = 'parseHTTPHeader_v2';
    expect(subwordLeft(t, t.length)).toBe(17);
    expect(subwordLeft(t, 17)).toBe(16);
    expect(subwordLeft(t, 16)).toBe(9);
    expect(subwordLeft(t, 9)).toBe(5);
    expect(subwordLeft(t, 5)).toBe(0);
    expect(subwordRight(t, 0)).toBe(5);
    expect(subwordRight(t, 5)).toBe(9);
    expect(subwordRight(t, 9)).toBe(15);
  });
});

describe('@ folders (files.rs)', () => {
  it("thread/at-recent-real: the list is the core's, in its order (newest first, folders and files mixed), never sorted again here", () => {
    const core = [
      { path: 'notes/today.md', dir: false },
      { path: 'src', dir: true },
      { path: 'a-old.txt', dir: false },
      { path: 'docs/', dir: true },
    ];
    expect(fileRows(core).map((r) => [r.label, r.tag])).toEqual([
      ['notes/today.md', 'file'],
      ['src/', 'dir'],
      ['a-old.txt', 'file'],
      ['docs/', 'dir'],
    ]);
  });
  it('one folder up, and the word that browses it', () => {
    expect(parentQuery('rust/tui/')).toBe('rust');
    expect(parentQuery('rust/')).toBe('');
    expect(parentQuery('/usr/')).toBe('/');
    expect(parentQuery('rust/tu')).toBeNull();
    expect(parentQuery('')).toBeNull();
    expect(browseText('rust')).toBe('@rust/');
    expect(browseText('docs/my notes')).toBe('@"docs/my notes/');
    expect(browseText('')).toBe('@');
    expect(browseText('/')).toBe('@/');
  });
});

describe('emoji (emoji.rs)', () => {
  it('the table is the TUI\'s, with its standard names', () => {
    expect(DESKTOP_EMOJI).toBe(TUI_EMOJI);
    expect(allEmoji().length).toBeGreaterThan(1500);
    expect(get('clap')).toBe('👏');
    expect(get('CLAP')).toBe('👏');
    expect(get('+1')).toBe('👍');
    expect(get('tada')).toBe('🎉');
    expect(get('aplause')).toBeNull();
  });
  it('the list opens at a word start with two letters, never in urls, times or paths', () => {
    expect(emojiToken(':cl')).toEqual({ start: 0, query: 'cl' });
    expect(emojiToken('bravo :cla')).toEqual({ start: 6, query: 'cla' });
    expect(emojiToken('(:cl')).toEqual({ start: 1, query: 'cl' });
    expect(emojiToken('👏:ta')).toEqual({ start: 2, query: 'ta' });
    for (const no of [':c', 'at :30', ':-D', ':)', 'http://ex', 'see https:', '12:30', '12:ab', 'note:todo', 'a: b', 'std::vec']) expect(emojiToken(no)).toBeNull();
  });
  it('a typed closing colon replaces a known name only', () => {
    expect(typedEmoji(':clap:')).toEqual({ start: 0, glyph: '👏' });
    expect(typedEmoji('bravo :clap:')).toEqual({ start: 6, glyph: '👏' });
    expect(typedEmoji(':+1:')).toEqual({ start: 0, glyph: '👍' });
    expect(typedEmoji('👏:tada:')).toEqual({ start: 2, glyph: '🎉' });
    for (const no of [':aplause:', ':nope:', 'a:clap:', 'http://', '12:30:', '::', ':clap']) expect(typedEmoji(no)).toBeNull();
  });
  it('ranks exact, prefix, substring, tags, then misspellings', () => {
    const cl = filterEmoji('cl');
    expect(cl[0]?.name).toBe('cl');
    expect(cl.findIndex((h) => h.emoji.glyph === '👏')).toBeLessThan(8);
    const g = (q: string) => filterEmoji(q).map((h) => h.emoji.glyph);
    expect(g('clap')[0]).toBe('👏');
    expect(g('applause')).toContain('👏');
    expect(g('aplause')).toContain('👏');
    expect(g('aplau')).toContain('👏');
    expect(g('tdaa')).toContain('🎉');
    expect(filterEmoji('zzzzqq')).toEqual([]);
    expect(filterEmoji('')).toEqual([]);
    expect(filterEmoji('e').length).toBeLessThanOrEqual(EMOJI_MAX);
    expect(emojiRows({ kind: 'emoji', query: 'tada', start: 0, quoted: false })[0]).toMatchObject({ label: '🎉  :tada:', pick: { kind: 'insert', text: '🎉' } });
  });
});

describe('artifacts in the @ list (commands.rs, W11)', () => {
  const now = 1_791_100_100_000;
  const made = [
    { id: 'perf-notes', title: 'Perf notes', kind: 'page', agent: 'perf', url: 'http://127.0.0.1:47123/p/perf-notes', atMs: now - 5 * 60_000 },
    { id: 'pricing', title: 'Pricing [draft]', kind: 'doc', agent: 'pm', url: 'http://127.0.0.1:47123/a/pricing', atMs: now - 3 * 3_600_000 },
  ];
  const at = (query: string, quoted = false) => ({ kind: 'at' as const, query, start: 0, quoted });
  it('by title, agent, kind or id, never in a path query, 8 at most', () => {
    expect(artifactRows(at(''), made, now).map((r) => r.id)).toEqual(['artifact.perf-notes', 'artifact.pricing']);
    expect(artifactRows(at('PERF'), made, now)[0]).toMatchObject({ label: '↗ Perf notes', desc: 'page · perf · 5 min ago', tag: 'artifact' });
    expect(artifactRows(at('doc'), made, now).map((r) => r.id)).toEqual(['artifact.pricing']);
    expect(artifactRows(at('src/'), made, now)).toEqual([]);
    // a path query lists no artifact, even one whose title holds it (min-tests: the clause on its own)
    expect(artifactRows(at('notes/'), made.map((a) => ({ ...a, title: `${a.title} notes/` })), now)).toEqual([]);
    expect(artifactRows(at('perf', true), made, now)).toEqual([]);
    const many = Array.from({ length: 12 }, (_, i) => ({ id: `a${i}`, title: 'Perf notes', kind: 'page', agent: 'perf', url: 'http://h', atMs: now }));
    expect(artifactRows(at(''), many, now)).toHaveLength(8);
    expect(agoWords(now - 3 * 3_600_000, now)).toBe('3 h ago');
  });
  it('its chip sends as a markdown link where it was, brackets out of the title', () => {
    const segs: Segment[] = [
      { kind: 'text', text: 'see ' },
      { kind: 'chip', chip: { kind: 'artifact', n: 1, id: 'pricing', title: 'Pricing [draft]', url: 'http://h/a/pricing' } },
      { kind: 'text', text: ' and fix it' },
    ];
    expect(buildMessage(segs)).toEqual({ text: 'see [Pricing draft](http://h/a/pricing) and fix it' });
    expect(chipStrip({ kind: 'artifact', n: 1, id: 'p', title: 'Perf notes', url: 'http://h/p' })).toBe('↗ Perf notes  http://h/p');
  });
  it('a label gets one space from the words, never two (qa-native R41: "[Image #1]what is…")', () => {
    const a: Chip = { kind: 'image', n: 1, path: '/i/a.png', name: 'a.png' };
    const b: Chip = { kind: 'image', n: 2, path: '/i/b.png', name: 'b.png' };
    const art: Chip = { kind: 'artifact', n: 3, id: 'p', title: 'Plan', url: 'http://h/p' };
    const send = (...parts: (string | Chip)[]): string =>
      buildMessage(parts.map((p): Segment => (typeof p === 'string' ? { kind: 'text', text: p } : { kind: 'chip', chip: p }))).text;
    expect(send(a, 'what is wrong')).toBe('[Image #1] what is wrong');
    expect(send(a, b, 'what is wrong in these')).toBe('[Image #1] [Image #2] what is wrong in these');
    expect(send('see', a, 'here')).toBe('see [Image #1] here');
    expect(send('see ', a, ' here')).toBe('see [Image #1] here');
    expect(send('read', art, ', then fix')).toBe('read [Plan](http://h/p), then fix');
    expect(send('line one\n', a)).toBe('line one\n[Image #1]');
    expect(send(a, '', 'go')).toBe('[Image #1] go');
  });
});

describe('sending: no-vision line and prefixes (attach.rs, sb.rs)', () => {
  it('the no-vision line names the model and says image', () => {
    expect(noVisionLine('mistral-small')).toBe("mistral-small does not support image input (bise's model catalog): not sent");
  });
  it('say and steer go out without their word', () => {
    expect(buildMessage([{ kind: 'text', text: 'say hello' }]).text).toBe('hello');
    expect(buildMessage([{ kind: 'text', text: 'steer go on' }]).text).toBe('go on');
    expect(buildMessage([{ kind: 'text', text: 'sayonara' }]).text).toBe('sayonara');
    expect(buildMessage([{ kind: 'text', text: 'they say hello' }]).text).toBe('they say hello');
  });
});

describe('live markdown colors (B9/W8)', () => {
  const cls = (runs: readonly { text: string; cls: string | null }[]) => runs.map((r) => `${r.cls ?? '-'}:${r.text}`);
  it("code: the bubble's own rule (codeLines), run for run and style for style (code-fences)", () => {
    const text = '```rust\nfn main() { let x = 1; } // note\n```\n```md\n# Plan\n- **do** `it`\n```\n```\nplain words\n```';
    const lines = colorLines(text);
    const bubble = [...codeLines('fn main() { let x = 1; } // note', 'rust'), ...codeLines('# Plan\n- **do** `it`', 'md'), ...codeLines('plain words', '')];
    const composer = [lines[1], lines[4], lines[5], lines[8]].map((runs) => (runs ?? []).map((r) => [r.text, r.css ?? '']));
    expect(composer).toEqual(bubble.map((runs) => runs.filter((r) => r.text !== '').map((r) => [r.text, cssText(r.style)])));
    // a bare fence's words: one gray run
    expect(lines[8]).toEqual([{ text: 'plain words', cls: null, css: 'color: var(--md-syn-bare)' }]);
  });
  it('prose: list markers, headings, inline code; plain words stay one run', () => {
    expect(cls(proseRuns('- [ ] do `this` now'))).toEqual(['mark:- [ ] ', '-:do ', 'code:`this`', '-: now']);
    expect(cls(proseRuns('## Plan'))).toEqual(['head:## Plan']);
    expect(cls(proseRuns("salut, je regarde ça ce soir : «on garde la file d'attente» ou pas ?"))).toEqual(["-:salut, je regarde ça ce soir : «on garde la file d'attente» ou pas ?"]);
  });
  it('fences dim, the block between them is code, after it prose again', () => {
    const lines = colorLines('```rust\nfn x\n```\n- a');
    expect(lines.map(cls)).toEqual([['fence:```rust'], ['-:fn', '-: ', '-:x'], ['fence:```'], ['mark:- ', '-:a']]);
    expect(lines[1]?.[0]?.css).toBe('color: var(--md-syn-keyword); font-weight: 700');
  });
});

