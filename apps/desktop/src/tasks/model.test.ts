// The right bar's rows and the since line, on plain rows (model.ts, since.ts). Owner: tasks.
import { describe, expect, it } from 'vitest';
import type { WorkRow, WorkStatus } from '@/proto';
import { barRows, cutAtWord, goalsOf, type ProjWork } from './model';
import { sinceLooked } from './since';
import { tasksOf } from './chips';
import { artsOf, resolveMerged } from './model';
import { originIn } from './TaskTab';
import { edgesOf } from './Board';
import { agentLineOf, agentStateOf, cardCrew, cardSaid, firstSentence, nowOfTask, originOf, reportsOf, soloOn } from './model';

const NOW = 1_700_000_000_000;
const H = 3_600_000;

function w(id: string, status: WorkStatus, last: number, extra: Partial<WorkRow> = {}): ProjWork {
  return { project: 'p', projectName: 'p', w: { id, name: id, by: 'main', created_ms: NOW - 48 * H, last_ms: last, status, ...extra } };
}

describe('the bar rows', () => {
  it('lists the open ones in the Board order, newest first inside a status, and folds ongoing and done this week', () => {
    const all = [
      w('a', 'working', NOW - 2 * H),
      w('b', 'needs_you', NOW - 5 * H),
      w('c', 'working', NOW - H),
      w('d', 'paused', NOW),
      w('e', 'to_review', NOW),
      w('f', 'ongoing', NOW),
      w('g', 'done', NOW, { done: { at_ms: NOW - 24 * H, by: 'user' } }),
      w('h', 'done', NOW, { done: { at_ms: NOW - 8 * 24 * H, by: 'user' } }),
    ];
    const r = barRows(all, NOW);
    expect(r.open.map((x) => x.w.id)).toEqual(['b', 'c', 'a', 'e', 'd']);
    expect(r.ongoing.map((x) => x.w.id)).toEqual(['f']);
    expect(r.done.map((x) => x.w.id)).toEqual(['g']);
  });

  it('counts the done-when items ticked of all, none without items', () => {
    expect(goalsOf({ goals: [{ text: 'x', done: true }, { text: 'y' }] })).toEqual({ done: 1, all: 2 });
    expect(goalsOf({})).toBeNull();
  });

  it('cuts a long name at a word', () => {
    expect(cutAtWord('Plugin apps in the gallery and the store', 20)).toBe('Plugin apps in the…');
    expect(cutAtWord('Short', 20)).toBe('Short');
  });
});

describe('the chips of a thing', () => {
  const all = [
    w('a', 'done', NOW - H, { agents: ['perf'], cards: [3], origin: 12 }),
    w('b', 'working', NOW - 2 * H, { agents: ['perf', 'docs'] }),
    w('c', 'paused', NOW, { agents: ['docs'], cards: [3] }),
  ].map((x) => x.w);
  it('reads an agent from the tasks it served, open ones first, then the newest', () => {
    expect(tasksOf(all, { agent: 'perf' }).map((x) => x.id)).toEqual(['b', 'a']);
    expect(tasksOf(all, { agent: 'docs' }).map((x) => x.id)).toEqual(['c', 'b']);
  });
  it('reads a card, his message and an artifact by their own fields', () => {
    expect(tasksOf(all, { card: 3 }).map((x) => x.id)).toEqual(['c', 'a']);
    expect(tasksOf(all, { origin: 12 }).map((x) => x.id)).toEqual(['a']);
    expect(tasksOf(all, { ids: ['b', 'gone'] }).map((x) => x.id)).toEqual(['b']);
  });
});

describe('since you looked', () => {
  it('counts what was done after his look, the tasks working now (one count), and every task that needs him now', () => {
    const since = NOW - 3 * H;
    const all = [
      w('a', 'done', NOW, { done: { at_ms: NOW - H, by: 'user' } }),
      w('b', 'done', NOW - 5 * H, { done: { at_ms: NOW - 5 * H, by: 'user' } }),
      w('c', 'working', NOW - H),
      w('d', 'waiting', NOW - 4 * H),
      w('e', 'needs_you', NOW - 10 * H),
    ];
    expect(sinceLooked(all, since)).toEqual({ done: 1, moving: 1, needs: 1 });
    // a working task counts whatever his last look: one count for moving (tasks-ux m_64959)
    expect(sinceLooked([w('c', 'working', NOW - 9 * H)], since).moving).toBe(1);
  });
});

describe('a merged task read as the one it went into', () => {
  it('follows the merges to the end, and stops on a loop', () => {
    const tombs = [{ id: 'a', merged_into: 'b' }, { id: 'b', merged_into: 'c' }, { id: 'x', merged_into: null }];
    expect(resolveMerged(tombs, 'a')).toBe('c');
    expect(resolveMerged(tombs, 'x')).toBe('x');
    expect(resolveMerged(tombs, 'live')).toBe('live');
    expect(['p', 'q']).toContain(resolveMerged([{ id: 'p', merged_into: 'q' }, { id: 'q', merged_into: 'p' }], 'p'));
  });
});

describe("his message quoted on the Task tab", () => {
  it('reads his message by its id, none when this window does not hold it', () => {
    const es = [undefined, { kind: 'agent', msg_id: 7, text: 'not his' }, { kind: 'you', msg_id: 7, text: ' make the app feel native ' }] as const;
    expect(originIn(es as never, 7)?.words).toBe('make the app feel native');
    expect(originIn(es as never, 8)).toBeNull();
  });

  it('quotes his words from the parts, never the model\'s tags or paths; his image apart (his m_65766)', () => {
    const o = originOf({
      text: '<image name="[Image #1]" path="/Users/g/.bise/images/f8.png" mime="image/png"> the chat takes too much space',
      parts: [
        { kind: 'image', name: '[Image #1]', source: '/Users/g/.bise/images/f8.png', mime: 'image/png' },
        { kind: 'text', text: ' the chat takes too much space\nsecond line' },
      ],
    });
    expect(o?.words).toBe('the chat takes too much space');
    expect(o?.images.map((i) => i.path)).toEqual(['/Users/g/.bise/images/f8.png']);
  });

  it('an image alone: no quote line, never an empty one (architect m_65793)', () => {
    expect(originOf({ text: '<image …>', parts: [{ kind: 'image', name: '[Image #1]', source: '/x.png' }] })).toBeNull();
  });
});

describe("what its agents say on the Task tab is about this task (tasks-ux m_65775)", () => {
  const lines = [
    { kind: 'report' as const, who: 'chat-collapse', text: '[report: progress] landed 59ea752f7. waiting on ui-review for shots', at_ms: 3 },
    { kind: 'report' as const, who: 'chat-collapse', text: 'older', at_ms: 1 },
    { kind: 'land' as const, who: 'chat-collapse', at_ms: 2 },
    { kind: 'report' as const, who: 'ui-review', text: 'shots green? not yet', at_ms: 2 },
  ];

  it('Now is the newest report on it, its first sentence', () => {
    const r = reportsOf(lines);
    expect(r.get('chat-collapse')?.text).toContain('landed 59ea752f7');
    expect(nowOfTask(r, [])).toBe('landed 59ea752f7.');
    expect(firstSentence('one line, no stop')).toBe('one line, no stop');
  });

  it('a note counts only from an agent on this task alone', () => {
    const works = [
      { id: 'a', status: 'working' as const, agents: ['designer', 'solo'] },
      { id: 'b', status: 'working' as const, agents: ['designer'] },
    ];
    const solo = soloOn(works, 'a', ['designer', 'solo']);
    expect([...solo]).toEqual(['solo']);
    const designer = { note: 'inbox-scroll pick', title: '', status: 'idle' as const };
    expect(agentLineOf('designer', new Map(), designer, solo.has('designer'))).toBe('');
    expect(agentLineOf('solo', new Map(), { ...designer, note: 'on it' }, true)).toBe('on it');
    expect(nowOfTask(new Map(), [{ note: 'on it' }])).toBe('on it');
  });

  it('waits for him only on a card of this task', () => {
    expect(agentStateOf({ status: 'idle' }, false)).toBe('idle');
    expect(agentStateOf({ status: 'idle' }, true)).toBe('waits');
    expect(agentStateOf({ status: 'working' }, true)).toBe('working');
    expect(agentStateOf(undefined, true)).toBe('archived');
  });
});
describe('artsOf: a task\'s artifacts in its tab', () => {
  it('the newest first, archived out, a merged task\'s read as its target, his before the agents\'', () => {
    const tombs = [{ id: 'old', merged_into: 'kb' }];
    const arts = [
      { id: 'a', works: ['kb'], at_ms: 1, audience: 'you' as const },
      { id: 'b', works: ['old'], at_ms: 3, audience: 'agents' as const },
      { id: 'c', works: ['kb'], at_ms: 2, archived: true },
      { id: 'd', works: ['other'], at_ms: 4 },
      { id: 'e', works: ['kb'], at_ms: 5 },
    ];
    const r = artsOf(arts, 'kb', tombs);
    expect(r.all.map((a) => a.id)).toEqual(['e', 'b', 'a']);
    expect(r.mine.map((a) => a.id)).toEqual(['e', 'a']);
    expect(artsOf(arts.slice(1, 2), 'kb', tombs).mine.map((a) => a.id)).toEqual(['b']);
  });
});

describe('a Board card (designer m_65805, tasks-ui-qa m_65799)', () => {
  it('names its first agent and how many more, never main', () => {
    expect(cardCrew(['main', 'auth-fix', 'kb', 'ui'])).toBe('auth-fix +2');
    expect(cardCrew(['main'])).toBe('');
    expect(cardCrew(['auto-approve-lead'])).toBe('auto-approve-lead');
  });

  it("says only an agent's report when it is on this task alone", () => {
    const designer = { name: 'designer', report: { kind: 'progress' as const, text: 'chat-collapse: folded float line agreed. more', at_ms: 5 }, note: '' };
    const solo = { name: 'kb', report: { kind: 'progress' as const, text: 'found the cause. the proxy', at_ms: 2 }, note: '' };
    expect(cardSaid([designer], new Set())).toBeNull();
    expect(cardSaid([designer, solo], new Set(['kb']))).toEqual({ text: 'found the cause.', at: 2 });
  });
});

describe('the Board edge fades', () => {
  it('show on the side that has more columns past the edge', () => {
    expect(edgesOf({ scrollLeft: 0, clientWidth: 600, scrollWidth: 600 })).toEqual({ l: false, r: false });
    expect(edgesOf({ scrollLeft: 0, clientWidth: 600, scrollWidth: 1400 })).toEqual({ l: false, r: true });
    expect(edgesOf({ scrollLeft: 800, clientWidth: 600, scrollWidth: 1400 })).toEqual({ l: true, r: false });
  });
});
