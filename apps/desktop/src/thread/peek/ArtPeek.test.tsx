// @vitest-environment happy-dom
// An artifact pill's hover preview (Gabriel m_58167, designer m_58173): it shows after the rest,
// never on a pass-through, goes on leave, a click still opens the artifact, and a cached picture is
// drawn with no new ask.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Artifact } from '@/proto';
import type { CorePort, ThumbAnswer, ThumbReq } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { planOf } from '@/artifacts/thumbs/plan';
import { resetThumbQueue, thumbQueue } from '@/artifacts/thumbs/run';
import { Markdown } from '../Markdown';
import { ArtPeekContext, resetPeek } from './ArtPeek';
import { PEEK_DELAY_MS, PEEK_GONE, PEEK_GRACE_MS, PEEK_WARM_MS, PEEK_WIDTH, agoWords, peekMeta, peekWait, placePeek } from './peek';

const P = 'acme-1a2b3c4d';
const NOW = 1_760_000_000_000;
const PAGE: Artifact = { id: 'resume', kind: 'page', title: 'Le résumé · Vibe deployment', agent: 'designer', version: 3, at_ms: NOW - 2 * 3_600_000, url: 'http://127.0.0.1:47123/p/resume', new: false };
const PR: Artifact = { id: 'pr-482', kind: 'pr', title: 'fix the login wait', agent: 'perf', version: 1, at_ms: NOW, url: 'https://github.com/acme/bise/pull/482', new: false };
const IMG = 'data:image/png;base64,AAAA';

describe('when and what (pure)', () => {
  it('waits the rest, at once on focus or while warm', () => {
    expect(peekWait(NOW, 0, false)).toBe(PEEK_DELAY_MS);
    expect(PEEK_DELAY_MS).toBeLessThanOrEqual(200);
    expect(peekWait(NOW, 0, true)).toBe(0);
    expect(peekWait(NOW, NOW + 1, false)).toBe(0);
  });
  it('goes above its pill, below with no room above, clamped 8 px off the right edge', () => {
    const view = { width: 1200, height: 800 };
    const card = { width: PEEK_WIDTH, height: 260 };
    // room above: above, its left edge on the pill's
    expect(placePeek({ left: 250, top: 600, width: 90, height: 22 }, card, view)).toEqual({ left: 250, top: 600 - 8 - 260, side: 'above' });
    // a pill near the window's top: below it
    expect(placePeek({ left: 250, top: 60, width: 90, height: 22 }, card, view)).toEqual({ left: 250, top: 60 + 22 + 8, side: 'below' });
    // a pill near the right edge: the card's right edge 8 px off the window's
    const r = placePeek({ left: 1100, top: 600, width: 80, height: 22 }, card, view);
    expect(r.left + PEEK_WIDTH).toBe(view.width - 8);
  });
  it("says its kind, its agent, its age and its version from v2", () => {
    expect(peekMeta(PAGE, NOW)).toBe('page · designer · 2 h ago · v3');
    expect(peekMeta(PR, NOW)).toBe('PR · perf · just now');
    expect(agoWords(NOW - 5 * 60_000, NOW)).toBe('5 min ago');
    expect(agoWords(NOW - 26 * 3_600_000, NOW)).toBe('yesterday');
    expect(agoWords(NOW - 3 * 86_400_000, NOW)).toBe('3 d ago');
  });
});

describe('a pill in a thread', () => {
  let thumb: ReturnType<typeof vi.fn<(req: ThumbReq, want: boolean) => Promise<ThumbAnswer | null>>>;
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'Date'] });
    vi.setSystemTime(NOW);
    resetPeek();
    thumb = vi.fn(() => Promise.resolve<ThumbAnswer | null>({ img: IMG }));
    (globalThis as { bise?: unknown }).bise = { thumb };
    resetThumbQueue();
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    delete (globalThis as { bise?: unknown }).bise;
    resetThumbQueue();
  });

  function draw(text: string, items: Artifact[] = [PAGE, PR]) {
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    store.setCore('up');
    store.apply({ ev: 'artifacts', project: P, items });
    const open = vi.fn();
    const onArtifact = vi.fn();
    const v = render(
      <StoreProvider store={store}>
        <ArtPeekContext.Provider value={{ project: P, open }}>
          <Markdown text={text} onArtifact={onArtifact} />
        </ArtPeekContext.Provider>
      </StoreProvider>,
    );
    return { ...v, open, onArtifact };
  }
  const card = () => document.body.querySelector<HTMLElement>('[data-testid="thread.peek"]');
  const rest = (el: Element, x = 20, y = 20) => {
    fireEvent.pointerMove(el, { clientX: x, clientY: y });
  };
  const wait = async (ms: number) => {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ms);
    });
  };

  it('shows after the rest, not on a pass-through, and goes on leave', async () => {
    const v = draw('[Le résumé](artifact:resume) reste provisoire.');
    const pill = v.getByTestId('thread.peek.pill');
    // a pass-through: in and out before the rest
    rest(pill);
    await wait(PEEK_DELAY_MS - 50);
    fireEvent.pointerLeave(pill);
    await wait(PEEK_DELAY_MS);
    expect(card()).toBeNull();
    // past the warm second, a rest shows it
    await wait(PEEK_WARM_MS);
    rest(pill, 21, 21);
    await wait(PEEK_DELAY_MS - 1);
    expect(card()).toBeNull();
    await wait(1);
    expect(card()).not.toBeNull();
    expect(card()?.querySelector('[data-testid="thread.peek.title"]')?.textContent).toBe(PAGE.title);
    expect(card()?.querySelector('[data-testid="thread.peek.meta"]')?.textContent).toBe('page · designer · 2 h ago · v3');
    expect(card()?.className).toContain('cold');
    // it goes on leave, after the grace
    fireEvent.pointerLeave(pill);
    await wait(PEEK_GRACE_MS);
    expect(card()).toBeNull();
  });

  it('a thread scrolled under a still pointer shows nothing until it moves', async () => {
    const v = draw('[Le résumé](artifact:resume)');
    const pill = v.getByTestId('thread.peek.pill');
    rest(pill, 30, 30);
    fireEvent.pointerLeave(pill);
    await wait(PEEK_WARM_MS + PEEK_DELAY_MS);
    // the same place again: the content moved, not the pointer
    rest(pill, 30, 30);
    await wait(PEEK_DELAY_MS * 2);
    expect(card()).toBeNull();
  });

  it('a click on the pill still opens the artifact; a click on the card opens it too', async () => {
    const v = draw('[Le résumé](artifact:resume)');
    const pill = v.getByTestId('thread.peek.pill');
    fireEvent.click(v.getByTestId('thread.artname'));
    expect(v.onArtifact).toHaveBeenCalledWith('resume');
    rest(pill);
    await wait(PEEK_DELAY_MS);
    const c = card();
    expect(c).not.toBeNull();
    if (c !== null) fireEvent.click(c);
    expect(v.open).toHaveBeenCalledWith('resume');
    expect(card()).toBeNull();
  });

  it('asks its picture as the pointer comes in, and a cached one is drawn with no new ask', async () => {
    const plan = planOf({ project: P, projectName: '', a: PAGE });
    if (plan.how !== 'page') throw new Error(`a local page is captured, got ${plan.how}`);
    // the gallery made it already: the queue has it
    await thumbQueue()?.get({ how: plan.how, src: plan.src, project: P, id: PAGE.id, v: plan.v, sig: plan.sig }, 'view');
    expect(thumb).toHaveBeenCalledTimes(1);
    const v = draw('[Le résumé](artifact:resume)');
    rest(v.getByTestId('thread.peek.pill'));
    await wait(PEEK_DELAY_MS);
    expect(card()?.querySelector('img')?.getAttribute('src')).toBe(IMG);
    expect(thumb).toHaveBeenCalledTimes(1);
  });

  it('one not cached: asked on the way in, before the rest ends', async () => {
    const v = draw('[Le résumé](artifact:resume)');
    rest(v.getByTestId('thread.peek.pill'));
    expect(thumb).toHaveBeenCalledTimes(1);
    await wait(PEEK_DELAY_MS);
    expect(card()?.querySelector('img')?.getAttribute('src')).toBe(IMG);
  });

  it('a PR shows its words, no picture; one gone from the store says so', async () => {
    const v = draw('[the PR](artifact:pr-482) and [old](artifact:gone-one)');
    const [pr, gone] = v.getAllByTestId('thread.peek.pill');
    if (pr === undefined || gone === undefined) throw new Error('two pills');
    rest(pr);
    await wait(PEEK_DELAY_MS);
    expect(card()?.querySelector('[data-testid="thread.peek.tile"]')?.textContent).toContain('#482');
    expect(card()?.querySelector('[data-testid="thread.peek.frame"]')).toBeNull();
    expect(thumb).not.toHaveBeenCalled();
    // warm: the next pill's shows at once
    fireEvent.pointerLeave(pr);
    await wait(PEEK_GRACE_MS);
    rest(gone, 60, 20);
    await wait(0);
    expect(card()?.querySelector('[data-testid="thread.peek.title"]')?.textContent).toBe('old');
    expect(card()?.querySelector('[data-testid="thread.peek.meta"]')?.textContent).toBe(PEEK_GONE);
    expect(card()?.className).not.toContain('cold');
  });

  it('esc hides it', async () => {
    const v = draw('[Le résumé](artifact:resume)');
    rest(v.getByTestId('thread.peek.pill'));
    await wait(PEEK_DELAY_MS);
    expect(card()).not.toBeNull();
    fireEvent.keyDown(document, { key: 'Escape' });
    await wait(0);
    expect(card()).toBeNull();
  });
});
