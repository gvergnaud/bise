import { describe, expect, it } from 'vitest';
import type { Agent } from '@/proto';
import { agentRows, agentsMore, archivedRows, pageRows, procRoots, procRows, roleWords, timeOf, watchRows, type ProjAgent } from './barrows';
import type { Proc } from '@/proto';

const NOW = 1_791_100_000_000;
const ag = (name: string, over: Partial<Agent> = {}): ProjAgent => ({
  project: 'shop',
  projectName: 'storefront',
  a: { name, main: false, status: 'idle', archived: false, title: '', purpose: `${name}'s job`, since_ms: NOW - 60_000, waits: 0, watching: [], ...over } as Agent,
});

describe('the right bar rows (designer desktop-right-bar v5)', () => {
  it('AGENTS: main first as bise, then the live ones in start order, the waiting one says waits in the accent', () => {
    const rows = agentRows([ag('perf', { status: 'working' }), ag('main', { main: true }), ag('search', { waits: 1 }), ag('old', { archived: true })], NOW, false);
    expect(rows.map((r) => r.name)).toEqual(['bise', 'perf', 'search']);
    expect(rows[0]).toMatchObject({ kind: 'agent', main: true, agent: 'main', glyph: '○', sub: 'team lead', testId: 'app.right.agent.main' });
    expect(rows[1]?.main).toBeUndefined();
    expect(rows[2]).toMatchObject({ glyph: '?', tone: 'acc', end: '', waits: true, sub: "search's job" });
  });
  it('AGENTS: waiting for disk (wt-disk) reads so, dim, with no turn time (ui-review m_67968)', () => {
    const disk = ag('a', { status: 'working', phase: 'waiting_disk' });
    expect(timeOf(disk.a, NOW)).toBe('');
    // the right bar's waiting look: '…' dim (barSign), never ∿; its time column stays empty (never a state word)
    const r = agentRows([disk], NOW, false)[0];
    expect(r).toMatchObject({ glyph: '…', tone: 'dim', end: '' });
    // its hover tip says why (OuterCard shows r.why over its job's line, ui-review m_67988)
    expect(r?.why).toBe("waiting for disk · starts on its own at 40 GB free · a's job");
    expect(agentRows([ag('b', { status: 'working' })], NOW, false)[0]?.why).toBeUndefined();
  });
  it("main stays first past the ⌥ numbers and the worktrees, ⌥0 when ⌥ is held, its title as its line, the breeze while it works", () => {
    const nums = new Map([['main', 0], ['docs', 1], ['perf', 2]]);
    const list = [ag('perf'), ag('docs'), ag('main', { main: true, status: 'working', title: 'routing his asks' })];
    const rows = agentRows(list, NOW, false, false, { numbers: nums });
    expect(rows.map((r) => [r.name, r.sub, r.working ?? false])).toEqual([
      ['bise', 'routing his asks', true],
      ['docs', "docs's job", false],
      ['perf', "perf's job", false],
    ]);
    expect(agentRows(list, NOW, false, true, { numbers: nums }).map((r) => r.glyph)).toEqual(['⌥0', '⌥1', '⌥2']);
  });
  it("bise's thread: bise's own main first (the home project's), its doing line; no other project's main; an archived main none", () => {
    const home = (n: string, over: Partial<Agent> = {}): ProjAgent => ({ ...ag(n, over), project: 'home', projectName: 'bise' });
    const list = [ag('main', { main: true }), ag('perf'), home('main', { main: true, title: 'routing his asks' })];
    expect(agentRows(list, NOW, true, false, { home: 'home' }).map((r) => [r.name, r.sub, r.testId])).toEqual([
      ['bise', 'routing his asks', 'app.right.agent.home.main'],
      ['perf', 'storefront', 'app.right.agent.shop.perf'],
    ]);
    expect(agentRows([ag('main', { main: true, archived: true }), ag('perf')], NOW, false).map((r) => r.name)).toEqual(['perf']);
  });
  it("bise's thread: the project's name in place of the role; ⌥ held: ⌥1-⌥9", () => {
    const rows = agentRows([ag('perf'), ag('docs')], NOW, true, true);
    expect(rows.map((r) => [r.glyph, r.sub, r.testId])).toEqual([
      ['⌥1', 'storefront', 'app.right.agent.shop.perf'],
      ['⌥2', 'storefront', 'app.right.agent.shop.docs'],
    ]);
  });
  it("SCHEDULED & WATCHING: an alarm, '◌', its agent, 'if …', until a clock ahead; a pause is no row here", () => {
    const off = () => 7200;
    const now = 1791843420000; // tue 13 oct 00:17 (+02:00), the fixture's capture
    const gate = { id: 4, kind: 'file' as const, what: 'the nightly gate goes red', noted: true, since_ms: 1791785220000, until_ms: 1791871620000 };
    const bench = { id: 7, kind: 'bg' as const, what: 'cargo bench', since_ms: now - 180_000, resumes: true };
    const rows = watchRows([ag('main', { watching: [gate] }), ag('perf', { watching: [bench] }), ag('old', { archived: true, watching: [gate] })], now, false, off);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      glyph: '◌',
      tone: 'dim',
      name: 'main',
      sub: 'if the nightly gate goes red',
      fit: true,
      end: 'until 08:07',
      title: 'bise wakes main if the nightly gate goes red · set yesterday 08:07 · stops watching at 08:07',
      testId: 'app.right.watching.main.4',
      watch: 4,
    });
  });
  it("ARCHIVED: sign, name, age, no 'done ·': ✓ done, ✗ red failed, – stopped", () => {
    const rows = archivedRows(
      [
        ag('release-32', { archived: true, status: 'done', since_ms: NOW - 2 * 3_600_000 }),
        ag('import', { archived: true, status: 'failed', since_ms: NOW - 86_400_000 }),
        ag('b', { archived: true, since_ms: NOW - 2 * 86_400_000 }),
      ],
      NOW,
      false,
    );
    expect(rows.map((r) => `${r.glyph} ${r.name} ${r.end}`)).toEqual(['✓ release-32 2 h', '✗ import 1 d', '– b 2 d']);
    expect(rows.map((r) => r.tone)).toEqual(['dim', 'err', 'dim']);
    expect(rows[0]?.title).toBe("done · release-32's job");
  });
  it('the cap never hides what waits on him: 12 agents, the 10th waits', () => {
    const list = Array.from({ length: 12 }, (_, i) => ag(`a${String(i + 1)}`, i === 9 ? { waits: 1 } : {}));
    const { shown, hidden } = pageRows(agentRows(list, NOW, false), 8);
    expect(shown.map((r) => r.name)).toEqual(['a1', 'a2', 'a3', 'a4', 'a5', 'a6', 'a7', 'a8', 'a10']);
    expect(hidden).toBe(3);
  });
  it("AGENTS' 'show N more' says how many folded agents work and rest (designer m_39151)", () => {
    const busy = (i: number) => (i % 5 === 0 ? { status: 'working' as const } : i % 7 === 0 ? { waiting_on: { who: 'agent' as const, name: 'main' } } : {});
    const rows = agentRows(Array.from({ length: 22 }, (_, i) => ag(`a${String(i + 1)}`, i >= 8 ? busy(i) : {})), NOW, false);
    const p = pageRows(rows, 8, 0, agentsMore);
    expect(p.more).toBe('show 14 more · 5 working · 9 idle');
    expect(p.words?.parts.map((x) => x.tone)).toEqual(['mid', 'faint', 'mid', 'faint', 'dim']);
    expect(p.words?.tip).toBe('14 agents not shown: 5 working (a11, a15, a16, + 2), 9 idle');
    // the folded ones all at rest, all at work
    const idle = agentRows(Array.from({ length: 11 }, (_, i) => ag(`b${String(i)}`)), NOW, false);
    expect(pageRows(idle, 8, 0, agentsMore).more).toBe('show 3 more · all idle');
    const work = agentRows(Array.from({ length: 11 }, (_, i) => ag(`w${String(i)}`, { status: 'working' })), NOW, false);
    expect(pageRows(work, 8, 0, agentsMore)).toMatchObject({ more: 'show 3 more · all working' });
    expect(pageRows(work, 8, 0, agentsMore).words?.tip).toBe('3 agents not shown: 3 working (w8, w9, w10)');
    // past a page: 'show 50 more of 365 · 3 working', then 'of 315'
    const many = agentRows(Array.from({ length: 372 }, (_, i) => ag(`m${String(i)}`, i === 100 || i === 200 || i === 300 ? { status: 'working' } : {})), NOW, false);
    expect(pageRows(many, 7, 0, agentsMore).more).toBe('show 50 more of 365 · 3 working');
    expect(pageRows(many, 7, 1, agentsMore).more).toBe('show 50 more of 315 · 3 working');
    // what waits on him is never folded: never counted, never the accent
    const asks = agentRows(Array.from({ length: 10 }, (_, i) => ag(`q${String(i)}`, i === 9 ? { waits: 1 } : {})), NOW, false);
    expect(pageRows(asks, 8, 0, agentsMore)).toMatchObject({ hidden: 1, more: 'show 1 more · all idle' });
  });
  it("'show 50 more' opens 50 at a time, 'show less' goes back to the cap (his m_26894): 370 rows, cap 5", () => {
    const rows = agentRows(Array.from({ length: 370 }, (_, i) => ag(`a${String(i + 1)}`, i === 300 ? { waits: 1 } : {})), NOW, false);
    const at = (p: number) => pageRows(rows, 5, p);
    expect([at(0).shown.length, at(0).hidden, at(0).more, at(0).less]).toEqual([6, 364, 'show 50 more · 364 left', false]);
    expect([at(1).shown.length, at(1).hidden, at(1).more, at(1).less]).toEqual([56, 314, 'show 50 more · 314 left', true]);
    expect(at(1).shown.at(-1)?.name).toBe('a301');
    expect([at(7).shown.length, at(7).hidden, at(7).more]).toEqual([355, 15, 'show 15 more']);
    expect([at(8).shown.length, at(8).hidden, at(8).more, at(8).less]).toEqual([370, 0, '', true]);
    // more pages than the list has (it shrank): clamped, 'show less' still there
    expect([at(40).pages, at(40).hidden]).toEqual([8, 0]);
    // under the cap: no row at all, whatever was kept
    expect(pageRows(rows.slice(0, 4), 5, 3)).toMatchObject({ pages: 0, hidden: 0, more: '', less: false });
  });
  it("never draws a raw tag as an agent's role: '<think>' reads 'thinking' (main m_22156)", () => {
    expect(roleWords('<think>')).toBe('thinking');
    expect(roleWords('</think> fixing the login')).toBe('thinking');
    expect(roleWords('fixing a < b in the parser')).toBe('fixing a < b in the parser');
    const rows = agentRows([ag('perf', { status: 'working', title: '<think>' })], NOW, false);
    expect(rows[0]?.sub).toBe('thinking');
  });
  it("PROCESSES (Gabriel's test, procs-bar): the program and its arguments, where it runs past the project or its worktree, a link only on a web page", () => {
    const p = (id: string, over: Partial<Proc>): Proc => ({ id, agent: 'perf', kind: { is: 'bg', n: 3 }, cmd: 'pnpm dev', pid: 1, started_ms: NOW, ports: [], state: { is: 'running' }, cpu: 0, rss_mb: 0, procs: 1, out: 'bg', ...over });
    const it = (x: Proc) => ({ project: 'shop', projectName: 'storefront', p: x });
    const wt = '/Users/g/.bise/worktrees/shop-1/perf/shop';
    const roots = procRoots([{ project: 'shop', path: '/Users/g/code/shop' }], [ag('perf', { worktree: wt }), ag('main', { main: true })]);
    const items = [
      it(p('bg3', { cmd: '/opt/homebrew/bin/python3 -m http.server 8000', cwd: `${wt}/site`, ports: [8000], url: 'http://127.0.0.1:8000' })),
      it(p('p9.1', { kind: { is: 'detached' }, cmd: 'node --inspect=9229 /Users/g/code/shop/x.js', cwd: '/Users/g/code/shop/apps/api', ports: [9229] })),
      it(p('bg4', { cmd: 'cargo watch', cwd: '/Users/g/code/shop' })),
    ];
    const rows = procRows(items, 'project', roots);
    expect(rows.map((r) => [r.name, r.sub, r.end])).toEqual([
      ['python3 -m http.server 8000', 'perf/site', 'localhost:8000 ↗'],
      ['node --inspect=9229 x.js', 'apps/api', ':9229'],
      ['cargo watch', '', ''],
    ]);
    expect(rows[0]?.title).toContain('by perf');
    expect(procRows(items, 'all', roots).map((r) => r.sub)).toEqual(['storefront › perf/site', 'storefront › apps/api', 'storefront']);
  });
});

// the agents' order, kept (agent-rank cf77a069a, his m_45828): main first, then the hub's rank
// (rows::Agent.rank, rankedOf's numbers); a worktree two agents share sits at its first member's
// number with its agents under it, never pushed after the others; open PRs with no agent last; on
// bise's own, project by project (the home's first), each in its rank
describe('the agents order (law: the hub rank leads)', () => {
  const place = (id: string, agents: string[]) => ({ id, branch: `sb/${id}`, agents } as unknown as import('@/proto').Place);
  it('rows follow the rank; a shared worktree takes its first member\'s place', () => {
    const list = [ag('main', { main: true }), ag('a', { rank: 4 }), ag('b', { rank: 1 }), ag('c', { rank: 3 }), ag('d', { rank: 2 })];
    const numbers = new Map([['main', 0], ['b', 1], ['d', 2], ['c', 3], ['a', 4]]);
    // no worktree: exactly the rank
    expect(agentRows(list, NOW, false, false, { numbers }).map((r) => r.agent)).toEqual(['main', 'b', 'd', 'c', 'a']);
    // d and a share a worktree: its head at d's number (2), d then a under it; c stays after
    const rows = agentRows(list, NOW, false, false, { numbers, places: [place('wt', ['d', 'a'])] });
    expect(rows.map((r) => (r.kind === 'group' ? 'ψ' : r.agent))).toEqual(['main', 'b', 'ψ', 'd', 'a', 'c']);
  });
  it('every permutation of the input gives the same order (the rank, not the arrival)', () => {
    const base = [ag('x', { rank: 2 }), ag('y', { rank: 1 }), ag('z', { rank: 3 })];
    const numbers = new Map([['y', 1], ['x', 2], ['z', 3]]);
    for (const list of [base, [...base].reverse(), [base[1], base[2], base[0]]] as ProjAgent[][]) {
      expect(agentRows(list, NOW, false, false, { numbers }).map((r) => r.agent)).toEqual(['y', 'x', 'z']);
    }
  });
  it("bise's own: the home's agents first, then each project's, each in its rank", () => {
    const at = (project: string, name: string, rank: number): ProjAgent => ({ ...ag(name, { rank }), project, projectName: project });
    const list = [at('shop', 's2', 2), at('home', 'h2', 2), at('shop', 's1', 1), at('home', 'h1', 1)];
    expect(agentRows(list, NOW, true, false, { home: 'home' }).map((r) => `${r.project}/${r.agent ?? ''}`)).toEqual(['home/h1', 'home/h2', 'shop/s1', 'shop/s2']);
  });
});
