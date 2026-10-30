import { describe, expect, it } from 'vitest';
import type { Agent } from '@/proto';
import { agentPicks, BRANCH_MIN, answerFit, answerHead, contextOf, folderName, pinkState, ringTip, stripFit, stripState, tabPlace, wordCut, worktreeName } from './tab';

describe("the composer's one pink state (composer-target A, art-target, agent-target)", () => {
  const base = { answer: null, aboutPink: undefined, about: false, agent: 'perf', voiceTyping: false, otherTip: (a: string) => `to ${a}, not bise` };
  it('is pink, with its place and context, when his words go to an agent that is not main: the border alone (quiet)', () => {
    expect(pinkState(base)).toEqual({ pink: true, tip: 'to perf, not bise', bare: false, whole: false, quiet: true, asks: false });
  });
  it("main stays plain, and voice typing (as art-target)", () => {
    expect(pinkState({ ...base, agent: 'main' }).pink).toBe(false);
    expect(pinkState({ ...base, voiceTyping: true }).pink).toBe(false);
  });
  it("an open artifact decides by its maker alone: its pink bare and whole, else plain (one not listed yet, main's)", () => {
    expect(pinkState({ ...base, about: true, aboutPink: 'tip' })).toEqual({ pink: true, tip: 'tip', bare: true, whole: true, quiet: false, asks: false });
    expect(pinkState({ ...base, about: true })).toEqual({ pink: false, tip: undefined, bare: false, whole: false, quiet: false, asks: false });
  });
  it("an open item wins: a question pink with its tip, a closed one or an approval plain, the tab bare", () => {
    expect(pinkState({ ...base, answer: { tip: 'q' }, aboutPink: 'tip' })).toEqual({ pink: true, tip: 'q', bare: true, whole: false, quiet: false, asks: false });
    expect(pinkState({ ...base, answer: { free: 'deny' } })).toMatchObject({ pink: false, bare: true });
    expect(pinkState({ ...base, answer: { closed: true }, agent: 'main' })).toMatchObject({ pink: false, bare: true });
  });
  // his m_47126: the border pink whenever his words go to an agent that is not the project's
  // main (an inbox reply, an artifact open behind it), never for main (designer m_47129).
  // Mutation: pinkState's main question back to the full pink turns the first red; a closed
  // item's border back to plain turns the second red.
  it("an inbox reply decides by its asker: main's question plain with its label in the accent, another agent's item the border", () => {
    expect(pinkState({ ...base, agent: 'main', answer: { asker: 'main', tip: 'q' } })).toEqual({ pink: false, tip: undefined, bare: true, whole: false, quiet: false, asks: true });
    expect(pinkState({ ...base, agent: 'main', answer: { asker: 'perf', tip: 'q' } })).toMatchObject({ pink: true, quiet: false, asks: false });
    for (const item of [{ asker: 'perf', closed: true }, { asker: 'perf', free: 'deny' as const }, { asker: 'perf', free: 'steer' as const }])
      expect(pinkState({ ...base, agent: 'main', answer: item, about: true }), JSON.stringify(item)).toEqual({ pink: true, tip: 'to perf, not bise', bare: true, whole: false, quiet: true, asks: false });
    expect(pinkState({ ...base, answer: { asker: 'main', free: 'steer' } })).toMatchObject({ pink: false, quiet: false });
  });
  it("his words opening with another agent's @name (at-start): its pink, bare, from main's thread too", () => {
    expect(pinkState({ ...base, agent: 'main', leadTip: 'lead' })).toEqual({ pink: true, tip: 'lead', bare: true, whole: false, quiet: false, asks: false });
    expect(pinkState({ ...base, leadTip: 'lead' })).toEqual({ pink: true, tip: 'lead', bare: true, whole: false, quiet: false, asks: false });
  });
});

describe("the composer's context strip (bise fait mac, mockup 1)", () => {
  const w = { gap: 16, state: 80, branch: 120, end: 60 };
  it('shows every item when they fit', () => {
    expect(stripFit({ ...w, avail: 80 + 120 + 60 + 2 * 16 })).toEqual({ state: true, end: true, lands: true, branch: true });
  });
  // Mutation: the old order (state before end) turns this red.
  it('drops whole items: the right end first, then the state; the branch stays', () => {
    expect(stripFit({ ...w, avail: 80 + 120 + 60 + 2 * 16 - 1 })).toEqual({ state: true, end: false, lands: false, branch: true });
    expect(stripFit({ ...w, avail: 80 + 120 + 16 - 1 })).toEqual({ state: false, end: false, lands: false, branch: true });
    expect(stripFit({ ...w, avail: 10 })).toEqual({ state: false, end: false, lands: false, branch: false });
  });
  // ui-review m_47192: 'lands on main' was left out of the fit, so the css squeezed it to a 3 px
  // sliver while the ring and the state stayed. Mutation: 'lands' out of stripFit's sum turns it red.
  it("'lands on main' goes whole first, before the ring and the state", () => {
    const l = { ...w, lands: 100 };
    expect(stripFit({ ...l, avail: 80 + 120 + 100 + 60 + 3 * 16 })).toEqual({ state: true, end: true, lands: true, branch: true });
    expect(stripFit({ ...l, avail: 80 + 120 + 100 + 60 + 3 * 16 - 1 })).toEqual({ state: true, end: true, lands: false, branch: true });
    expect(stripFit({ ...l, avail: 80 + 120 + 16 })).toEqual({ state: true, end: false, lands: false, branch: true });
  });
  // ui-review m_47192: at 680 with a panel open the branch kept its icon and one clipped letter.
  // Mutation: the branch always kept turns it red.
  it('the branch, last, cuts with its ellipsis down to BRANCH_MIN, then goes whole', () => {
    expect(stripFit({ ...w, avail: BRANCH_MIN })).toEqual({ state: false, end: false, lands: false, branch: true });
    expect(stripFit({ ...w, avail: BRANCH_MIN - 1 })).toEqual({ state: false, end: false, lands: false, branch: false });
    expect(stripFit({ ...w, branch: 40, avail: 40 })).toMatchObject({ branch: true });
  });
  it('an item that is not there takes no room and no gap', () => {
    expect(stripFit({ gap: 16, state: 0, branch: 120, end: 60, avail: 120 + 60 + 16 })).toEqual({ state: true, end: true, lands: true, branch: true });
  });
  it("the addressee's state (his m_44719): waits on him first, then its turn's length; idle says nothing", () => {
    const a = { status: 'working', waits: 0, since_ms: 1_000, turn_ms: 180_000 };
    expect(stripState(a, 9_000)).toEqual({ kind: 'working', ms: 180_000 });
    expect(stripState({ ...a, turn_ms: null }, 9_000)).toEqual({ kind: 'working', ms: 8_000 });
    expect(stripState({ ...a, waits: 1 }, 9_000)).toEqual({ kind: 'waits' });
    expect(stripState({ ...a, status: 'idle', waiting_on: { who: 'you' } }, 9_000)).toEqual({ kind: 'waits' });
    expect(stripState({ ...a, status: 'idle' }, 9_000)).toBeNull();
    expect(stripState({ ...a, status: 'stopped' }, 9_000)).toBeNull();
    expect(stripState(undefined, 9_000)).toBeNull();
  });
  it("says where it works by its folder's name: a worktree after the ψ, his checkout bare, nothing for bise's own main (his m_24782)", () => {
    const t = (p: string) => p.replace('/Users/g', '~');
    const wt = { path: '/Users/g/.bise/worktrees/harness-3abb2bd8/composer-strip/harness', tip: 'ψ worktree: its own copy of the repo, on sb/composer-strip · ~/.bise/worktrees/harness-3abb2bd8/composer-strip/harness' };
    expect(tabPlace({ branch: 'sb/composer-strip' }, false, wt, '/Users/g/code/harness', t)).toEqual({ name: 'composer-strip', worktree: true, tip: wt.tip, branch: 'sb/composer-strip' });
    expect(tabPlace({ branch: null }, false, undefined, '/Users/g/lab/bend-lab/harness', t)).toEqual({ name: 'harness', worktree: false, tip: 'your own folder: ~/lab/bend-lab/harness', branch: undefined });
    expect(tabPlace({ branch: null, checkout_branch: 'main' }, false, undefined, '/Users/g/lab/bend-lab/harness', t)?.branch).toBe('main');
    expect(tabPlace({ branch: 'sb/ui', worktree: '/Users/g/.bise/worktrees/h-1/ui/storefront' }, false, undefined, '/p', t)).toEqual({ name: 'ui', worktree: true, tip: 'ψ worktree: ~/.bise/worktrees/h-1/ui/storefront', branch: 'sb/ui' });
    expect(tabPlace({ branch: 'x' }, true, wt, '/p', t)).toBeUndefined();
    expect(tabPlace(undefined, false, wt, '/p', t)).toBeUndefined();
    expect(worktreeName('/w/x/repo')).toBe('repo');
    expect(folderName('/a/b/harness/')).toBe('harness');
  });

});

describe("the tab's label cut (designer m_24325)", () => {
  const w = (s: string) => s.length * 10;
  const label = 'answering search · which size system should the filter use?';
  it('whole when it fits', () => {
    expect(wordCut(label, 10_000, w)).toBe(label);
  });
  it("after a whole word, with one '…', never mid-word", () => {
    expect(wordCut(label, 300, w)).toBe('answering search · which size…');
    expect(wordCut(label, 290, w)).toBe('answering search · which…');
  });
  it("a cut right after the '·' leaves the '·' out", () => {
    expect(wordCut(label, 200, w)).toBe('answering search…');
  });
  it('not even the first word: the first word and the …', () => {
    expect(wordCut(label, 20, w)).toBe('answering…');
  });
});

describe("the answering tab at a narrow width (composer-target A, designer m_26531)", () => {
  const w = (s: string) => s.length * 10;
  const label = '↳ answering search · which size system…';
  const end = 200; // 'esc to bise instead' and its gap
  it("its head is the label's first part, never cut", () => {
    expect(answerHead(label)).toBe('↳ answering search');
    expect(answerHead('↳ answering perf · telemetry · which bench?')).toBe('↳ answering perf');
  });
  it('both whole when they fit', () => {
    expect(answerFit(label, 10_000, end, w)).toEqual({ label, end: true });
  });
  it('the question goes first, after a whole word, the end stays', () => {
    expect(answerFit(label, 330 + end, end, w)).toEqual({ label: '↳ answering search · which size…', end: true });
    expect(answerFit(label, 190 + end, end, w)).toEqual({ label: '↳ answering search…', end: true });
  });
  it("then the end; '↳ answering search' never goes", () => {
    expect(answerFit(label, 270, end, w)).toEqual({ label: '↳ answering search · which…', end: false });
    expect(answerFit(label, 190, end, w)).toEqual({ label: '↳ answering search…', end: false });
    expect(answerFit(label, 60, end, w)).toEqual({ label: '↳ answering search', end: false });
  });
  it("an open artifact's '· about “…”' shows whole or goes whole, never 'about…'; then the end (designer m_27021)", () => {
    const about = '↳ to perf · about “product page”'; // 32 chars, head 9
    expect(answerFit(about, 320 + end, end, w, true)).toEqual({ label: about, end: true });
    expect(answerFit(about, 310 + end, end, w, true)).toEqual({ label: '↳ to perf', end: true });
    expect(answerFit(about, 90 + end, end, w, true)).toEqual({ label: '↳ to perf', end: true });
    expect(answerFit(about, 89 + end, end, w, true)).toEqual({ label: '↳ to perf', end: false });
  });
});

describe("the tab's context ring (designer m_24404 touch 4)", () => {
  it("reads the hub's words, named or not", () => {
    expect(contextOf('42k · 17%')).toEqual({ tokens: '42k', pct: 17 });
    expect(contextOf('context 263k · 26%')).toEqual({ tokens: '263k', pct: 26 });
    expect(contextOf('')).toBeNull();
    expect(contextOf(undefined)).toBeNull();
    expect(contextOf('lots')).toBeNull();
  });
  it('its tip says what the ring means', () => {
    expect(ringTip('perf', { tokens: '42k', pct: 17 })).toBe("17% of perf's context used · 42k tokens · compacts at 90%");
  });
});

// his m_47126: the strip's addressee is a picker of the project's agents (designer m_47129: bise
// first, the sidebar's order, each with its sign, archived never)
describe("the strip's addressee picker", () => {
  const ag = (name: string, over: Partial<Agent> = {}): Agent => ({ name, dir: name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: '', since_ms: 1, waits: 0, watching: [], ...over });
  it("lists main first as bise, then the live agents by the hub's rank, each with its state's sign", () => {
    const rows = agentPicks([ag('perf', { rank: 2, status: 'working' }), ag('old', { archived: true }), ag('docs', { rank: 1, waits: 1 }), ag('main'), ag('qa', { status: 'failed' }), ag('ship', { status: 'done' })], 10_000);
    expect(rows.map((r) => `${r.shown}${r.sign === '' ? '' : ` ${r.sign}`}`)).toEqual(['bise', 'docs ?', 'perf ∿', 'qa ✗', 'ship ✓']);
    expect(rows[0]?.name).toBe('main');
  });
});
