import { describe, expect, it } from 'vitest';
import { optTip } from './optAct';
import { optPlan } from './optPlan';
import { anchoredSrc } from '@/artifacts/Open';

describe('an option the window does itself (notif-open)', () => {
  const review = { act: 'open' as const, url: 'http://127.0.0.1:1/p/issue-15#m-pr16', artifact: 'issue-15', at: 'm-pr16' };
  it("the asker's artifact opens in the window at its block; gone, its url; no url, still its view (it says it's gone)", () => {
    expect(optPlan(review, () => true)).toEqual({ t: 'artifact', id: 'issue-15', at: 'm-pr16' });
    expect(optPlan(review, () => false)).toEqual({ t: 'url', url: review.url });
    expect(optPlan({ act: 'open', artifact: 'plan' }, () => false)).toEqual({ t: 'artifact', id: 'plan', at: null });
  });
  it("a link in his browser, the sign-in the app's own; an Open with nothing opens nothing", () => {
    expect(optPlan({ act: 'open', url: 'https://x.test/pull/1' }, () => true)).toEqual({ t: 'url', url: 'https://x.test/pull/1' });
    expect(optPlan({ act: 'sign_in' }, () => true)).toEqual({ t: 'sign_in' });
    expect(optPlan({ act: 'open' }, () => true)).toEqual({ t: 'none' });
  });
  // code-viewer-width (architect m_57792): a feature try card's '2 show the diff' carries its .diff
  // and its branch's review; this window opens the review, an older one the .diff. Mutation:
  // optPlan without its review line turns the first case red.
  it("an option with a branch review opens that branch's guided review, else its artifact; a newer hub's act opens nothing", () => {
    const tryDiff = { act: 'open' as const, artifact: 'security-diff', review: { kind: 'branch' as const, name: 'security' } };
    expect(optPlan(tryDiff, () => true)).toEqual({ t: 'branch', branch: 'security' });
    expect(optPlan({ act: 'open', artifact: 'security-diff' }, () => true)).toEqual({ t: 'artifact', id: 'security-diff', at: null });
    // a review of another kind is not this act's: its artifact
    expect(optPlan({ ...tryDiff, review: { kind: 'agent', name: 'perf' } }, () => true)).toEqual({ t: 'artifact', id: 'security-diff', at: null });
    expect(optPlan({ act: 'unknown' }, () => true)).toEqual({ t: 'none' });
  });
  it('the frame opens at the block: its hash, the theme query kept', () => {
    expect(anchoredSrc('http://127.0.0.1:1/p/issue-15?theme=dark', 'm-pr16')).toBe('http://127.0.0.1:1/p/issue-15?theme=dark#m-pr16');
    expect(anchoredSrc('http://h/p/a?theme=light#old', 'b')).toBe('http://h/p/a?theme=light#b');
    expect(anchoredSrc('http://h/p/a', null)).toBe('http://h/p/a');
    expect(anchoredSrc(null, 'b')).toBeNull();
  });
  it("an option that opens an artifact says where it goes: 'opens <title> beside the thread'", () => {
    expect(optTip(review, (id) => (id === 'issue-15' ? 'issue #15: chatgpt plan fix' : undefined))).toBe('opens issue #15: chatgpt plan fix beside the thread');
    expect(optTip(review, () => undefined)).toBe('opens issue-15 beside the thread');
    expect(optTip({ act: 'open', url: 'https://x.test' }, () => 'x')).toBeNull();
    expect(optTip(undefined, () => 'x')).toBeNull();
  });
});
