// @vitest-environment happy-dom
// page-unreach (Benjamin's report): a page whose server doesn't answer left the panel blank, with
// no word. Now the panel probes first and says why, and frames the page once it answers.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Artifact } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { RouterContext, createRouter } from '@/app/router';
import { ArtifactOpen } from './index';
import { panel } from '@/proto/words_panel.gen';
import { downWords, LOAD_GRACE_MS, probed, REPROBE_MS, type DownWords } from './reach';

const URL_ = 'http://127.0.0.1:47415/p/blokhus-own-deployment';
const page: Artifact = { id: 'deploy', kind: 'page', title: 'own deployment', agent: 'perf', version: 1, at_ms: 0, url: URL_, new: false };

type Bise = { probePage?: (u: string) => Promise<'up' | 'down'>; openExternal?: (u: string) => Promise<boolean> };
const g = globalThis as { bise?: Bise };

afterEach(() => {
  cleanup();
  delete g.bise;
  vi.useRealTimers();
});

function mount(running: boolean, down?: string) {
  const store = new Store(null);
  store.apply({ ev: 'projects', projects: [{ project: 'blokhus-1', name: 'blokhus', path: '/x/blokhus', running, ...(down === undefined ? {} : { down }) }] });
  store.apply({ ev: 'artifacts', project: 'blokhus-1', items: [page] });
  const router = createRouter({ screen: 'artifacts', project: 'blokhus-1', id: 'deploy' });
  render(<StoreProvider store={store}><RouterContext.Provider value={router}><ArtifactOpen project="blokhus-1" id="deploy" composer={false} /></RouterContext.Provider></StoreProvider>);
}

/** Let the probe's promise land. */
const settle = () => act(async () => { await Promise.resolve(); await Promise.resolve(); });

describe('the words when a page does not answer', () => {
  it('its hub stopped: bise starts it; its hub failed: why; it runs: the address, plainly (designer m_38170)', () => {
    const text = (w: DownWords) => w.parts.map((p) => ('t' in p ? p.t : p.mono)).join('');
    const stopped = downWords({ name: 'blokhus', running: false }, URL_);
    expect([stopped.title, text(stopped), stopped.starting]).toEqual(["∿ starting blokhus's hub…", "blokhus's hub had stopped, and its pages with it. this one shows here as soon as the hub is up.", true]);
    const failed = downWords({ name: 'blokhus', running: false, down: 'the hub did not start: no runtime (its log: /h/.bise/hubs/blokhus-1a2b/hub.log)' }, URL_, '/h');
    expect([failed.title, text(failed)]).toEqual(["blokhus's hub didn't start", 'no runtime. its log: ~/.bise/hubs/blokhus-1a2b/hub.log. its page shows here once the hub runs.']);
    expect(failed.parts[1]).toEqual({ mono: '~/.bise/hubs/blokhus-1a2b/hub.log', reveal: '/h/.bise/hubs/blokhus-1a2b/hub.log' });
    expect(text(downWords({ name: 'b', running: false, down: 'the hub did not start in 15 s (its log: /x/hub.err)' }, URL_))).toBe('no answer in 15 s. its log: /x/hub.err. its page shows here once the hub runs.');
    const runs = downWords({ name: 'blokhus', running: true }, URL_);
    expect([runs.title, runs.starting]).toEqual(["this page doesn't answer", false]);
    expect(runs.parts).toContainEqual({ mono: '127.0.0.1:47415' });
  });
  it('only the local pages server is probed', () => {
    expect([probed(URL_), probed('https://example.com/'), probed(null)]).toEqual([true, false, false]);
  });
});

describe('the panel (page-unreach)', () => {
  it('nothing answers: it says why instead of a blank frame, then frames the page once it answers', async () => {
    vi.useFakeTimers();
    let answer: 'up' | 'down' = 'down';
    const probePage = vi.fn(() => Promise.resolve(answer));
    g.bise = { probePage };
    mount(false);
    await settle();
    expect(screen.queryByTestId('artifact.page')).toBeNull();
    expect(screen.getByTestId('artifact.down.title').textContent).toBe("∿ starting blokhus's hub…");
    // the hub comes up: the next ask frames the page by itself
    answer = 'up';
    await act(async () => { vi.advanceTimersByTime(REPROBE_MS); await Promise.resolve(); await Promise.resolve(); });
    expect(screen.queryByTestId('artifact.down')).toBeNull();
    expect(screen.getByTestId('artifact.page').getAttribute('src')).toContain(URL_);
  });

  it('artifact-loading: a page server still starting shows a loading state, never the error, then the page', async () => {
    // his report (m_60703): 'nothing answers at 127.0.0.1:<port>' flashed for a few seconds on
    // open, until the page loaded. A first refused probe is a server not ready yet, not a failure.
    vi.useFakeTimers();
    let answer: 'up' | 'down' = 'down';
    g.bise = { probePage: vi.fn(() => Promise.resolve(answer)) };
    mount(true);
    await settle();
    expect(screen.queryByTestId('artifact.down')).toBeNull();
    expect(screen.getByTestId('artifact.checking').textContent).toBe(panel.art_loading);
    await act(async () => { vi.advanceTimersByTime(LOAD_GRACE_MS / 2); await Promise.resolve(); await Promise.resolve(); });
    expect(screen.queryByTestId('artifact.down')).toBeNull();
    answer = 'up';
    await act(async () => { vi.advanceTimersByTime(LOAD_GRACE_MS / 2 - 1); await Promise.resolve(); await Promise.resolve(); });
    expect(screen.queryByTestId('artifact.down')).toBeNull();
    expect(screen.getByTestId('artifact.page').getAttribute('src')).toContain(URL_);
  });

  it("'try again' asks at once and loads again; '↗ browser' opens no dead tab", async () => {
    vi.useFakeTimers();
    const opened: string[] = [];
    const probePage = vi.fn(() => Promise.resolve('down' as const));
    g.bise = { probePage, openExternal: (u) => { opened.push(u); return Promise.resolve(true); } };
    mount(true);
    await settle();
    // the error only once loading really failed: still nothing after the grace
    await act(async () => { vi.advanceTimersByTime(LOAD_GRACE_MS); await Promise.resolve(); await Promise.resolve(); });
    expect(screen.getByTestId('artifact.down.title').textContent).toBe("this page doesn't answer");
    const asked = probePage.mock.calls.length;
    fireEvent.click(screen.getByTestId('artifact.down.again'));
    await settle();
    expect(probePage.mock.calls.length).toBeGreaterThan(asked);
    expect(screen.getByTestId('artifact.checking')).toBeTruthy();
    fireEvent.click(screen.getByTestId('artifact.browser'));
    await settle();
    expect(opened).toEqual([]);
  });

  it('a page that answers opens in the browser as before', async () => {
    const opened: string[] = [];
    g.bise = { probePage: () => Promise.resolve('up'), openExternal: (u) => { opened.push(u); return Promise.resolve(true); } };
    mount(true);
    await settle();
    expect(screen.getByTestId('artifact.page')).toBeTruthy();
    fireEvent.click(screen.getByTestId('artifact.browser'));
    await settle();
    expect(opened).toEqual([URL_]);
  });
});
