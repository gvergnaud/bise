import { describe, expect, it } from 'vitest';
import type { Entry } from '@/proto';
import { hub_ev } from '@/proto/fixtures.gen';
import { GROUP_GAP_MS, RUN_MS, TOOL_LINES, briefOf, deliveryMark, editWords, lastOfRun, sideOf, placeSteered, chipOf, dropAuthor, sendInThread, handoffOf, stepTime, toolEnd, toolLines, toolsWords, toolSource, jsonStrField, toolTime, clock, dayLabel, entryKey, findText, groupHead, namedRows, pendingKey, repeatedReport, repeatsReport, posOfKey, reportLine, reportStatus, schedHead, toolWords, sameDay, stripContext, whenOf, whoOf } from './entry';
import { hitsIn } from './find';
import FIND_TEXT from '../../../../rust/proto/fixtures/find_text.json';

const e = (over: Partial<Entry>): Entry => ({ pos: 1, at_ms: 0, kind: 'agent', text: '', ...over });

describe('thread rows', () => {
  it('his sends in the tail: a queued one only once failed or routed, a steer always', () => {
    expect(sendInThread({ mode: 'now', state: 'sending' }, false)).toBe(true);
    expect(sendInThread({ mode: 'queued', state: 'sending' }, false)).toBe(false);
    expect(sendInThread({ mode: 'queued', state: 'failed' }, false)).toBe(true);
    expect(sendInThread({ mode: 'queued', state: 'sending' }, true)).toBe(true);
  });

  it('keys: entries by pos, sends by cid, never the same', () => {
    expect(entryKey(12)).toBe('e:12');
    expect(posOfKey(entryKey(12))).toBe(12);
    expect(posOfKey(pendingKey('12'))).toBeUndefined();
  });

  it("who speaks: his words, bise's main with the pearl, an agent, a message from another agent", () => {
    expect(whoOf(e({ kind: 'you' }), { agent: 'perf', bise: false }).from).toBe('you');
    expect(whoOf(e({}), { agent: 'main', bise: true })).toEqual({ from: 'bise', name: 'bise', indent: false });
    expect(whoOf(e({}), { agent: 'main', bise: false })).toEqual({ from: 'bise', name: 'bise', indent: false });
    expect(whoOf(e({ kind: 'from_agent', from: 'docs' }), { agent: 'main', bise: false })).toEqual({ from: 'agent', name: 'docs', indent: true });
  });

  it('a clock in his zone', () => {
    expect(clock(Date.UTC(2026, 0, 2, 14, 5), 'UTC')).toBe('14:05');
    expect(clock(0)).toBe('');
  });

  it("⌘F finds a tools entry's steps in the fixture's thread", () => {
    const entries = hub_ev.flatMap((ev) => (ev.ev === 'thread' ? ev.entries : ev.ev === 'entry' ? [ev.entry] : []));
    const found = hitsIn(
      entries.map((x) => ({ pos: x.pos, text: findText(x) })),
      'slow',
    );
    expect(found.length).toBeGreaterThan(0);
  });

  it("findText is the hub's find_text (rust/proto/fixtures/find_text.json, bise_proto::thread::find)", () => {
    for (const c of FIND_TEXT) expect(findText(c.entry as unknown as Entry), c.name).toBe(c.text);
  });
});

describe('M14: a turn time on hover', () => {
  const NOW = Date.UTC(2026, 9, 14, 15, 0);
  it("today's turn shows its clock, an older day's its date too, no time shows nothing", () => {
    expect(whenOf(Date.UTC(2026, 9, 14, 9, 46), NOW, 'UTC')).toBe('09:46');
    expect(whenOf(Date.UTC(2026, 9, 12, 7, 0), NOW, 'UTC')).toBe('12 Oct 07:00');
    expect(whenOf(0, NOW, 'UTC')).toBe('');
  });
});

describe('batch 3a: a message between two other agents', () => {
  it("names both ('perf → docs'), and only the sender when it is to the agent in view", () => {
    const m = e({ kind: 'from_agent', from: 'perf', to: 'docs' });
    expect(whoOf(m, { agent: 'main', bise: false }).name).toBe('perf → docs');
    expect(whoOf(m, { agent: 'docs', bise: false }).name).toBe('perf');
    expect(whoOf(e({ kind: 'from_agent', from: 'perf' }), { agent: 'main', bise: false }).name).toBe('perf');
  });
});

describe("the thread's reading (ambient m_11777)", () => {
  const ctx = { agent: 'main', bise: true };
  const T = Date.UTC(2026, 9, 6, 9, 46);

  it('a group opens on a new author, after 5 min of silence, on a new day; notices open nothing', () => {
    const a = e({ at_ms: T });
    expect(groupHead(undefined, a, ctx, 'UTC')).toBe(true);
    expect(groupHead(a, e({ at_ms: T + 60_000 }), ctx, 'UTC')).toBe(false);
    expect(groupHead(a, e({ at_ms: T + GROUP_GAP_MS + 1 }), ctx, 'UTC')).toBe(true);
    expect(groupHead(a, e({ kind: 'you', at_ms: T + 1 }), ctx, 'UTC')).toBe(true);
    expect(groupHead(e({ kind: 'you', at_ms: T }), e({ kind: 'you', at_ms: T + 1 }), ctx, 'UTC')).toBe(false);
    expect(groupHead(a, e({ kind: 'notice', at_ms: T + 1 }), ctx, 'UTC')).toBe(true);
    expect(groupHead(e({ kind: 'from_agent', from: 'docs' }), e({ kind: 'from_agent', from: 'perf' }), ctx, 'UTC')).toBe(true);
    const late = Date.UTC(2026, 9, 6, 23, 58);
    expect(groupHead(e({ at_ms: late }), e({ at_ms: late + 3 * 60_000 }), ctx, 'UTC')).toBe(true);
  });

  it("days: 'today', 'yesterday', then 'mon 6 oct'", () => {
    expect(sameDay(T, T + 3_600_000, 'UTC')).toBe(true);
    expect(sameDay(0, T, 'UTC')).toBe(true);
    expect(dayLabel(T, T + 3_600_000, 'UTC')).toBe('today');
    expect(dayLabel(T, T + 86_400_000, 'UTC')).toBe('yesterday');
    expect(dayLabel(T, T + 3 * 86_400_000, 'UTC')).toBe('tue 6 oct');
  });

  it('his words never show the screen_context block', () => {
    const t = 'why is this slow?\n\n<screen_context app="Ghostty">\nThis is what was in front of the user\ntitle: x\n</screen_context>\n';
    expect(stripContext(t)).toBe('why is this slow?');
    expect(stripContext('<screen_context app="X">a</screen_context>\nhi')).toBe('hi');
    expect(stripContext('a < b, no block')).toBe('a < b, no block');
  });

  it('the chip cuts the path, never the app or the host; a url opens, a shot alone is not kept', () => {
    expect(chipOf({ app: 'Chrome', url: 'http://localhost:4801/settings' })).toEqual({
      label: '◫ with Chrome · localhost:4801/sett…',
      open: { url: 'http://localhost:4801/settings' },
      tip: undefined,
    });
    expect(chipOf({ app: 'Zed', file: '/repo/src/main.rs', url: 'https://a.dev/' })?.label).toBe('◫ with Zed · a.dev/ +1');
    expect(chipOf({ app: 'Zed', file: '/repo/src/main.rs' })?.open).toEqual({ file: '/repo/src/main.rs' });
    expect(chipOf({ app: 'Ghostty', shot: 'x.png' })).toEqual({ label: '◫ with Ghostty', open: null, tip: "the screenshot isn't kept" });
    expect(chipOf({ app: 'Ghostty', title: 'zsh — harness' })?.label).toBe('◫ with Ghostty · zsh — harness');
    expect(chipOf({})).toBeNull();
  });

  it("a row that starts with its author's name drops it: the name line says it (ambient m_11934)", () => {
    expect(dropAuthor('perf: the parsed copy cuts it', 'perf')).toBe('the parsed copy cuts it');
    expect(dropAuthor('performance: fine', 'perf')).toBe('performance: fine');
    expect(dropAuthor('the perf: fine', 'perf')).toBe('the perf: fine');
    expect(dropAuthor('hi', '')).toBe('hi');
  });
});

describe("the agent view's tool rows (designer's desktop-screens v6 (2))", () => {
  it("a step's length: '38 s', '1 m 12 s', '2 m', none under 1 s", () => {
    expect(stepTime(38_400)).toBe('38 s');
    expect(stepTime(72_000)).toBe('1 m 12 s');
    expect(stepTime(120_000)).toBe('2 m');
    expect(stepTime(400)).toBe('');
  });

  it('commands and edits are lines with their length; reads and searches fold into counted words', () => {
    const T = 1_791_100_000_000;
    const t = toolLines([
      { pos: 1, at_ms: T, text: 'src/a.rs', kind: 'read' },
      { pos: 2, at_ms: T + 1000, text: 'lighthouse --preset=mobile /p/tee-classic', kind: 'run' },
      { pos: 3, at_ms: T + 39_000, text: 'web/product/Hero.tsx', kind: 'edit' },
      { pos: 4, at_ms: T + 40_000, text: 'reviews', kind: 'search' },
    ]);
    expect(t.lines.map(({ pos, mark, text, time, end }) => ({ pos, mark, text, time, end }))).toEqual([
      { pos: 2, mark: '$', text: 'lighthouse --preset=mobile /p/tee-classic', time: '38 s', end: '38 s' },
      { pos: 3, mark: '±', text: 'web/product/Hero.tsx', time: '1 s', end: '1 s' },
    ]);
    expect(t.rest.map((r) => r.pos)).toEqual([1, 4]);
    expect(t.restWords).toBe('read 1 file, searched once');
    const many = toolLines(Array.from({ length: 8 }, (_, i) => ({ pos: i, at_ms: 0, text: `cmd ${String(i)}`, kind: 'run' })));
    expect(many.lines).toHaveLength(TOOL_LINES);
    expect(many.restWords).toBe('ran 2 commands');
  });

  it("a row's words: the last command's description there is (his m_49672), a group's count after it (designer m_49693), else the hub's counted words", () => {
    const run = (pos: number, intent?: string) => ({ pos, at_ms: 0, text: intent ?? `cmd ${String(pos)}`, kind: 'run', intent });
    expect(toolsWords([run(1, 'start the dev server')], 'ran 1 command')).toEqual({ words: 'start the dev server', count: '' });
    // a group keeps its steps under it; its line says the last description, past a plain command after it, and how many
    expect(toolsWords([run(1, 'build it'), run(2, 'run the tests'), run(3)], 'ran 3 commands')).toEqual({ words: 'run the tests', count: '3 commands' });
    // only commands count: a read beside one command leaves it alone
    expect(toolsWords([{ pos: 1, at_ms: 0, text: 'src/a.rs', kind: 'read' }, run(2, 'run the tests')], 'read 1 file, ran 1 command')).toEqual({ words: 'run the tests', count: '' });
    expect(toolsWords([run(1), run(2, '  ')], 'ran 2 commands')).toEqual({ words: 'ran 2 commands', count: '' });
    expect(toolsWords([{ pos: 1, at_ms: 0, text: 'src/a.rs', kind: 'read' }], 'read 1 file')).toEqual({ words: 'read 1 file', count: '' });
    // the call at work the step fold draws (its intent at the thread's end) is not said twice
    expect(toolsWords([run(1, 'build it'), run(2, 'lecture du PDF')], 'ran 2 commands', 2)).toEqual({ words: 'build it', count: '2 commands' });
    expect(toolsWords([run(1, 'lecture du PDF')], 'ran 1 command', 1)).toEqual({ words: 'ran 1 command', count: '' });
  });

  it("R11/S9: each call's state on its line as the TUI's: ✓ time, ✗ exit · time with its error line, code and output to open", () => {
    expect(toolTime(300)).toBe('0.3 s');
    // rounded as the TUI's {:.1}, never floored (qa-flows' tools-real: '✓ 0.1s' there)
    expect(toolTime(60)).toBe('0.1 s');
    expect(toolTime(2_160)).toBe('2.2 s');
    expect(toolTime(4_120)).toBe('4.1 s');
    expect(toolTime(12_400)).toBe('12 s');
    expect(toolTime(64_000)).toBe('1 m 4 s');
    const t = toolLines([
      { pos: 1, at_ms: 0, text: 'cargo test', kind: 'run', state: 'ok', ms: 300, code: 'cargo test', out: 'ok: 12 passed' },
      { pos: 2, at_ms: 0, text: 'npm run lint', kind: 'run', state: 'err', ms: 2_100, exit: 1, err: 'src/a.ts:3 unused x', code: 'npm run lint', out: 'exit 1: src/a.ts:3 unused x' },
      { pos: 3, at_ms: 0, text: 'read_file: {"path":"gone.rs"}', kind: 'read', state: 'err', err: 'no such file' },
      { pos: 4, at_ms: 0, text: 'src/b.rs', kind: 'read', state: 'ok', ms: 10 },
      { pos: 5, at_ms: 0, text: 'cargo build', kind: 'run', state: 'run' },
    ]);
    expect(t.lines.map((l) => [l.pos, l.mark, l.state, l.end, l.err])).toEqual([
      [1, '$', 'ok', '✓ 0.3 s', ''],
      [2, '$', 'err', '✗ exit 1 · 2.1 s', 'src/a.ts:3 unused x'],
      [3, '', 'err', '✗', 'no such file'],
      [5, '$', 'run', '', ''],
    ]);
    expect(t.lines[1]).toMatchObject({ code: 'npm run lint', out: 'exit 1: src/a.ts:3 unused x' });
    // the failed read is a line: it isn't counted again in the words
    expect(t.restWords).toBe('read 1 file');
    expect(toolEnd({ pos: 1, at_ms: 0, text: '', kind: 'run', state: 'err', exit: 0 }, '')).toBe('✗');
    expect(toolEnd({ pos: 1, at_ms: 0, text: '', kind: 'run', state: 'ok' }, '3 s')).toBe('✓');
  });
});

describe("a project's bise hands off (designer's desktop-screens v6 (3))", () => {
  it("reads the hub's spawn notice: the agent and its brief's first line", () => {
    expect(handoffOf('✚ main → new agent @perf : product page speed\nmore words')).toEqual({ agent: 'perf', brief: 'product page speed' });
    expect(handoffOf('✚ main → new task @t : x')).toEqual({ agent: 't', brief: 'x' });
    expect(handoffOf('✚ main → new agent @solo')).toEqual({ agent: 'solo', brief: '' });
    expect(handoffOf('session restored · 6 messages')).toBeNull();
  });
});

describe('D14 / D15: a report is a status line, a command reads once', () => {
  it("'[report: blocked] waiting for your answer: …' -> 'blocked · waiting for your answer' (the card says the question)", () => {
    expect(reportLine('[report: blocked] waiting for your answer: cart page or checkout only?')).toBe('blocked · waiting for your answer');
    expect(reportLine('[report: done] p95 at 180 ms, nothing to fix.\n- ran 3 times')).toBe('done · p95 at 180 ms, nothing to fix.');
    expect(reportStatus('[report: done] p95 at 180 ms, nothing to fix.\n- ran 3 times')).toBe('✓ done · p95 at 180 ms, nothing to fix.');
    expect(reportStatus('[report: failed] the staging token expired.')).toBe('✗ failed · the staging token expired.');
    expect(reportStatus('[report: blocked] waiting for your answer: x?')).toBe('blocked · waiting for your answer');
    expect(reportStatus('a plain message')).toBeNull();
    expect(reportLine('a plain answer')).toBeNull();
    expect(reportLine('waiting for your answer: x', 'blocked')).toBe('blocked · waiting for your answer');
  });
  it("words that open on the kind say it once: 'blocked: signing key …', never 'blocked · blocked: …' (designer m_52857)", () => {
    expect(reportLine('[report: blocked] blocked: signing key not found')).toBe('blocked: signing key not found');
    expect(reportLine('Blocked · waiting on signing', 'blocked')).toBe('Blocked · waiting on signing');
    expect(reportStatus('[report: done] done: p95 at 180 ms')).toBe('✓ done: p95 at 180 ms');
    expect(reportStatus('[report: failed] failed: token expired')).toBe('✗ failed: token expired');
    // a word that only starts like the kind keeps it
    expect(reportLine('[report: done] doneness checked')).toBe('done · doneness checked');
  });
  it("'bash: bash /Users/…' reads '$ bash /Users/…'; a tool's args object never shows", () => {
    expect(toolWords('bash: bash /Users/me/x.sh')).toBe('bash /Users/me/x.sh');
    expect(toolWords('cargo test')).toBe('cargo test');
    expect(toolWords('edit: {"file_path":"panels.md"}')).toBe('edit panels.md');
    expect(toolWords('read_file: {"path":"a.rs"}')).toBe('read_file a.rs');
    expect(toolWords('bash: {"command":"ls -la\\nmore"}')).toBe('ls -la');
    expect(toolWords('grep: {"pattern":"slow","path":"src"}')).toBe('grep src');
    expect(toolWords('web_fetch: {"url":"https://x.test"}')).toBe('web_fetch https://x.test');
    expect(toolWords('todo: {"items":[1,2]}')).toBe('todo');
    expect(toolWords('looking for the slow query')).toBe('looking for the slow query');
    const t = toolLines([{ pos: 1, at_ms: 0, text: 'edit: {"file_path":"panels.md"}', kind: 'edit' }, { pos: 2, at_ms: 0, text: 'read_file: {"path":"a.rs"}', kind: 'read' }]);
    expect(t.lines[0]?.text).toBe('edit panels.md');
    // a folded read opens to a row with the TUI's verb (designer m_14269)
    expect(t.rest[0]?.text).toBe('read a.rs');
  });

  it('a connector call keeps the account it ran as, none when the hub wrote none (multi-accounts piece 5)', () => {
    const t = toolLines([
      { pos: 1, at_ms: 0, text: 'gmail.search: {"q":"x"}', kind: 'run', account: 'Gabriel pro' },
      { pos: 2, at_ms: 0, text: 'gmail.search: {"q":"y"}', kind: 'run' },
    ]);
    expect(t.lines.map((l) => l.account)).toEqual(['Gabriel pro', '']);
  });
});

describe("N2: a scheduled task's line names the agent, not the task's number", () => {
  it("'main scheduled #1 for dashboards · every 30m · next 16:56' -> 'bise scheduled dashboards · …'", () => {
    expect(schedHead('main scheduled #1 for dashboards · every 30m · next 16:56')).toBe('bise scheduled dashboards · every 30 min · next 16:56');
    expect(schedHead('main scheduled #48 for perf · every 2m · 6 times · next 14:22')).toBe('bise scheduled perf · every 2 min · 6 times · next 14:22');
    expect(schedHead('a plain line')).toBe('a plain line');
    // never the internal number (designer m_14246)
    expect(schedHead('scheduled #48 ended · stopped by you', 'perf')).toBe('scheduled perf ended · stopped by you');
    expect(schedHead('scheduled #48 ended · stopped by you')).toBe('a scheduled task ended · stopped by you');
  });
  it("names main 'bise' as every window row does (designer m_40683); the TUI keeps 'main'", () => {
    // the hub's set_own and set_for heads (bise_proto::words::scheduled)
    expect(schedHead('main scheduled the ingest lag check · every 2m · next 10:44')).toBe('bise scheduled the ingest lag check · every 2 min · next 10:44');
    expect(schedHead('perf scheduled build check for main · every 2m · next 14:22')).toBe('perf scheduled build check for bise · every 2 min · next 14:22');
    expect(schedHead('main scheduled build check for main')).toBe('bise scheduled build check for bise');
    // another agent keeps its name; 'main' inside the title stays the title's
    expect(schedHead('perf scheduled the main branch check · every 5m')).toBe('perf scheduled the main branch check · every 5 min');
  });
});

describe('briefOf (the hub brief, the TUI is_brief)', () => {
  it('reads the name, the objective and the body without its heading', () => {
    const text = '# Task `gift-ui`\n\nObjective: make the gift page fast\n\nNo end criterion: this is a long-running task. Stay available.\n';
    expect(briefOf(text)).toEqual({ name: 'gift-ui', objective: 'make the gift page fast', body: 'Objective: make the gift page fast\n\nNo end criterion: this is a long-running task. Stay available.' });
  });
  it('is none for any other text, a heading elsewhere included', () => {
    expect(briefOf('make the gift page fast')).toBeNull();
    expect(briefOf('see # Task `x`')).toBeNull();
    expect(briefOf('# Tasks')).toBeNull();
  });
});

describe("his answer to main's own card, the hub's raw line (designer m_15195)", () => {
  it('reads the card, the question without its options, and his words', async () => {
    const { answeredCardOf } = await import('./entry');
    expect(answeredCardOf('the user answered card #1 ("merge checkout into main? (2 files, tests pass) 1. merge 2. not yet"): "[Image #1] this one"')).toEqual({
      card: 1,
      question: 'merge checkout into main? (2 files, tests pass)',
      words: '[Image #1] this one',
    });
    expect(answeredCardOf('the user answered notification #12 ("which bench?"): "the cold one"')).toEqual({ card: 12, question: 'which bench?', words: 'the cold one' });
    // a transcript written before notif-rename keeps the old word, read for good
    expect(answeredCardOf('the user answered card #12 ("which bench?"): "the cold one"')).toEqual({ card: 12, question: 'which bench?', words: 'the cold one' });
    expect(answeredCardOf('the user answered a question')).toBeNull();
    expect(answeredCardOf('done: the build is green')).toBeNull();
  });
});

describe("a call that rounds to 0.0 s (designer m_15569)", () => {
  it("says '✓' alone, never '✓ 0.0 s'; 0.1 s and up keep their time", () => {
    expect(toolEnd({ pos: 1, at_ms: 0, text: '', kind: 'run', state: 'ok', ms: 30 }, '')).toBe('✓');
    expect(toolEnd({ pos: 1, at_ms: 0, text: '', kind: 'run', state: 'err', exit: 1, ms: 20 }, '')).toBe('✗ exit 1');
    expect(toolEnd({ pos: 1, at_ms: 0, text: '', kind: 'run', state: 'ok', ms: 60 }, '')).toBe('✓ 0.1 s');
  });
});

describe('messages-right v1: whose side a row is on', () => {
  const at = 1_000_000;
  const e = (kind: string, text: string, ms = at): Entry => ({ pos: 1, at_ms: ms, kind, text }) as unknown as Entry;
  it('everything he said goes right; what bise or an agent said or did stays left', () => {
    expect(sideOf(e('you', 'cap them at 400 KB'))).toBe('you');
    expect(sideOf(e('approval', 'you answered perf: 1 · merge'))).toBe('you');
    expect(sideOf(e('from_agent', 'the user answered card #1 ("merge? 1. yes 2. no"): "1"'))).toBe('you');
    expect(sideOf(e('agent', 'on it.'))).toBe('them');
    expect(sideOf(e('answered', 'answered gift-ui for you'))).toBe('them');
    expect(sideOf(e('you', '# Task `gift-ui`\n\nObjective: x'))).toBe('them');
    expect(sideOf(undefined)).toBe('them');
  });
  it('his time shows under the last bubble of a 2 min run', () => {
    expect(lastOfRun(e('you', 'a'), e('you', 'b', at + 60_000))).toBe(false);
    expect(lastOfRun(e('you', 'a'), e('you', 'b', at + RUN_MS + 1))).toBe(true);
    expect(lastOfRun(e('you', 'a'), e('agent', 'b', at + 1000))).toBe(true);
    expect(lastOfRun(e('you', 'a'), undefined)).toBe(true);
  });
});

describe('messages-right v1: a steer', () => {
  it("a steer shows where the agent read it: right before the first row past its receipt, bundled steers in their order", () => {
    // 1 his ask, 2 tools, 3 his steer (read at the receipt 5), 4 more tools, 6 the answer
    const at = new Map<number, number>([[3, 5]]);
    expect(placeSteered([1, 2, 3, 4, 6], (p) => at.get(p))).toEqual([1, 2, 4, 3, 6]);
    // two messages read at one receipt keep their order
    const two = new Map<number, number>([[3, 5], [4, 5]]);
    expect(placeSteered([1, 2, 3, 4, 5, 6, 7], (p) => two.get(p))).toEqual([1, 2, 5, 3, 4, 6, 7]);
    // a receipt past every row: at the end; none, or one at or before its own pos: nothing moves
    expect(placeSteered([1, 2, 3], (p) => (p === 1 ? 9 : undefined))).toEqual([2, 3, 1]);
    const order = [1, 2, 3];
    expect(placeSteered(order, () => undefined)).toBe(order);
    expect(placeSteered(order, (p) => (p === 2 ? 2 : null))).toBe(order);
  });

  it("his message's delivery mark is the TUI's: ✓ got it, ✓✓ read it, ▲ not sent; sent, unknown or none show the time alone", () => {
    expect(deliveryMark('received', 'perf')).toEqual({ glyph: '✓', tip: 'perf got it', tone: 'got' });
    expect(deliveryMark('read', 'perf')).toEqual({ glyph: '✓✓', tip: 'perf read it', tone: 'read' });
    expect(deliveryMark('failed', 'perf')).toEqual({ glyph: '▲ not sent', tip: "perf didn't get it", tone: 'failed' });
    for (const d of ['sent', 'unknown', null, undefined] as const) expect(deliveryMark(d, 'perf'), String(d)).toBeNull();
  });
});

describe("an edit's line counts on its tool line (designer m_12065, hub-rows ToolItem.files)", () => {
  it("names the file, a move's two names, several as 'and N more', and sums the lines", () => {
    expect(editWords([{ path: 'web/product/Hero.tsx', add: 18, del: 6 }])).toEqual({ text: 'Hero.tsx', counts: '+18 −6' });
    expect(editWords([{ path: 'src/old.rs → src/new.rs', add: 0, del: 0 }])).toEqual({ text: 'old.rs → new.rs', counts: '' });
    expect(editWords([{ path: 'a/x.rs', add: 3, del: 1 }, { path: 'b/y.rs', add: 2, del: 0 }, { path: 'z.md', add: 1, del: 4 }])).toEqual({ text: 'x.rs and 2 more', counts: '+6 −5' });
    expect(editWords([])).toBeNull();
    expect(editWords(undefined)).toBeNull();
  });

  it("an edit line reads '± <file>' with its counts; one with no files keeps its words", () => {
    const { lines } = toolLines([
      { pos: 1, at_ms: 0, text: 'patching the query', kind: 'edit', files: [{ path: 'src/query.rs', add: 1, del: 1 }] },
      { pos: 2, at_ms: 0, text: 'apply_patch: src/lib.rs', kind: 'edit' },
      { pos: 3, at_ms: 0, text: 'cargo test', kind: 'run' },
    ]);
    expect(lines.map((l) => [l.mark, l.text, l.counts])).toEqual([
      ['±', 'query.rs', '+1 −1'],
      ['±', lines[1]?.text ?? '', ''],
      ['$', 'cargo test', ''],
    ]);
  });
});

describe('namedRows (tui-parity H2, designer m_19832)', () => {
  const ctx = { agent: 'main', bise: false };
  const mk = (pos: number, kind: string, min: number) => ({ pos, kind, text: 'x', at_ms: 1_000_000_000_000 + min * 60_000 }) as unknown as Entry;
  it('one head per author run, on its first row, a tool row too (designer m_21404 (3))', () => {
    const list = [mk(1, 'you', 0), mk(2, 'tools', 1), mk(3, 'agent', 1), mk(4, 'agent', 2), mk(5, 'you', 3), mk(6, 'tools', 3), mk(7, 'landed', 4), mk(8, 'agent', 4), mk(9, 'agent', 12)];
    const m = new Map(list.map((e) => [e.pos, e]));
    const { named, carry } = namedRows([...m.keys()], (p) => m.get(p), ctx);
    // 1 his, 2 the run's first row (tools), 5 his, 6 a tools-only turn's own head; 8 after the land; 9 after 8 min
    expect([...named]).toEqual([1, 2, 5, 6, 8, 9]);
    expect(carry).toBe(false);
  });
  it('a turn of work still open has its head already: the live row owes none', () => {
    const list = [mk(1, 'you', 0), mk(2, 'tools', 1)];
    const m = new Map(list.map((e) => [e.pos, e]));
    const r = namedRows([1, 2], (p) => m.get(p), ctx);
    expect([...r.named]).toEqual([1, 2]);
    expect(r.carry).toBe(false);
  });
});

describe('repeatsReport (tui-parity H8)', () => {
  it('the message under a report says its first line: folded; other words: kept', () => {
    expect(repeatsReport('p99 rose because it averaged the slowest 10%; it is the real percentile now', 'P99 rose because it averaged the slowest 10%; it is the real percentile now (src/latency.py). ready to merge.')).toBe(true);
    expect(repeatsReport('p99 rose because it averaged the slowest 10%', 'Asked main: today or after the batch test.')).toBe(false);
    expect(repeatsReport('ok', 'ok')).toBe(false);
  });
});

describe('repeatedReport (H8: drawn as a quiet row)', () => {
  it('a report whose next message says it again; not a report, not without a message under it', () => {
    const rep = { kind: 'to_agent', text: '[report: done] p99 rose because it averaged the slowest 10%' } as unknown as Entry;
    const msg = { kind: 'agent', text: 'P99 rose because it averaged the slowest 10%; ready to merge.' } as unknown as Entry;
    expect(repeatedReport(rep, msg)).toBe(true);
    expect(repeatedReport(rep, undefined)).toBe(false);
    expect(repeatedReport(msg, msg)).toBe(false);
  });
});

describe("a code tool's box shows the source it ran, as the TUI's (main m_22134, code.rs tool_source)", () => {
  const ts = JSON.stringify({ code: "async function main() {\n  const q = ['\"a\" b'];\n  return q;\n}", description: 'search the error' });
  it("run_typescript: its 'code' field, decoded (real newlines, real quotes), never the description", () => {
    const r = toolSource('run_typescript', ts);
    expect(r.lang).toBe('typescript');
    expect(r.code).toBe("async function main() {\n  const q = ['\"a\" b'];\n  return q;\n}");
    expect(r.code).not.toContain('description');
  });
  it('a capped payload keeps what came', () => {
    expect(jsonStrField('{"code":"const a = 1;\\nconst b', 'code')).toBe('const a = 1;\nconst b');
    expect(jsonStrField('{"x":1}', 'code')).toBeNull();
  });
  it("bash: its command from the args, or the raw command as it came", () => {
    expect(toolSource('bash', JSON.stringify({ arg: 'cargo test\necho ok', description: 'tests' }))).toEqual({ code: 'cargo test\necho ok', lang: 'bash' });
    expect(toolSource('bash', 'cargo test -p perf')).toEqual({ code: 'cargo test -p perf', lang: 'bash' });
  });
  it('another tool keeps its code as it came, plain', () => {
    expect(toolSource('read_file', '{"path":"a.rs"}')).toEqual({ code: '{"path":"a.rs"}', lang: '' });
    expect(toolSource(undefined, 'npm run lint')).toEqual({ code: 'npm run lint', lang: '' });
  });
  it('toolLines carries it', () => {
    const l = toolLines([{ pos: 1, at_ms: 1, name: 'run_typescript', text: 'search the error', kind: 'run', state: 'ok', ms: 400, code: ts }]).lines[0];
    expect(l?.lang).toBe('typescript');
    expect(l?.mark, "the TUI's sign for a TypeScript run").toBe('ƒ');
    expect(l?.said, 'no intent: its words are machine text').toBe(false);
    const said = toolLines([{ pos: 2, at_ms: 1, name: 'bash', intent: 'the tests', text: 'the tests', kind: 'run', state: 'ok', code: 'cargo test' }]).lines[0];
    expect(said?.mark).toBe('$');
    expect(said?.said, 'its description: words').toBe(true);
    expect(l?.code.startsWith('async function main() {\n')).toBe(true);
  });
});
