// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render as rtlRender, screen } from '@testing-library/react';
import type { ReactElement, ReactNode } from 'react';
import { BarSlot } from '@/app/barslot';
import { formatRoute, parseRoute } from '@/app/route';
import { createRouter, RouterContext } from '@/app/router';
import { sentenceCase } from '@/ds/case';

// the rows' menus (useEntityMenu, nm-menus) read the router: every render is inside one
const Routed = ({ children }: { children: ReactNode }) => <RouterContext.Provider value={createRouter({ screen: 'scheduled' })}>{children}</RouterContext.Provider>;
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: Routed });
import { ahead, endsWords, howOften, nextRunWords, ordinal, runsWords, ScheduledView, type ScheduledTask, type Task } from './index';
import { scheduled } from '@/proto/words_scheduled.gen';

// the window's bar: the page's search is never there (his m_52658)
beforeEach(() => { render(<BarSlot />); });
afterEach(cleanup);

const NOW = new Date(2026, 9, 3, 14, 0).getTime();
const at = (days: number, h: number, m: number) => new Date(2026, 9, 3 + days, h, m).getTime();
const task = (id: number, extra: Partial<ScheduledTask> = {}): ScheduledTask => ({
  id, agent: 'deploy-watch', by: 'main', words: 'check the deploy of storefront\nsay so only when it fails', every: '5m', done: 6, times: 24, next_ms: at(0, 14, 22), ...extra,
});
const T = (t: ScheduledTask, project = 'shop-1', projectName = 'storefront'): Task => ({ project, projectName, t });

describe('scheduled words', () => {
  it("says times, runs and ends as the TUI's screen", () => {
    expect([1, 2, 3, 4, 11, 22].map(ordinal)).toEqual(['1st', '2nd', '3rd', '4th', '11th', '22nd']);
    expect(ahead(at(0, 14, 22), NOW)).toBe('14:22');
    expect(ahead(at(1, 7, 30), NOW)).toBe('tomorrow 07:30');
    expect(ahead(at(6, 7, 30), NOW)).toBe('the 9th, 07:30');
    expect(runsWords(task(4))).toBe(scheduled.n_of({ count: 6, total: 24 }));
    expect(runsWords(task(4, { times: null, done: 1 }))).toBe(scheduled.runs_so_far({ n: 1 }));
    expect(nextRunWords(task(4), NOW)).toBe(`${scheduled.nth_of({ nth: '7th', total: 24 })} · 14:22`);
    expect(endsWords(task(4), NOW)).toBe(scheduled.ends_after_nth({ nth: '24th' }));
    expect(endsWords(task(4, { times: null, until_ms: at(0, 18, 0) }), NOW)).toBe(scheduled.ends_at({ at: '18:00' }));
    expect(endsWords(task(4, { times: null }), NOW)).toBe(scheduled.ends_when_stopped);
  });

  it("the route is 'scheduled'", () => {
    expect(parseRoute('scheduled')).toEqual({ screen: 'scheduled' });
    expect(formatRoute({ screen: 'scheduled' })).toBe('scheduled');
  });
});

describe('the scheduled screen', () => {
  it('lists the tasks soonest first; ⏎ opens one in the pane; x asks in place, then stops it', () => {
    const stopped: string[] = [];
    const tasks = [T(task(9, { words: 'the morning digest', agent: 'main', every: 'day 07:30', next_ms: at(1, 7, 30), times: null, done: 2 }), 'home', 'bise'), T(task(4))];
    render(<ScheduledView tasks={tasks} ports={{ stop: (p, id) => stopped.push(`${p}#${String(id)}`) }} now={NOW} />);
    const rows = screen.getByTestId('scheduled.list').querySelectorAll('.b-lrow');
    expect([...rows].map((r) => r.getAttribute('data-testid'))).toEqual(['scheduled.row.4', 'scheduled.row.9']);
    // by project (ds GroupLabel, sentence case), the soonest's project first
    expect([...screen.getByTestId('scheduled.list').querySelectorAll('.b-glabel')].map((g) => g.textContent)).toEqual(['storefront', 'bise']);
    // two lines: its name and how often, then who it wakes and its next run
    expect(rows[0]?.querySelector('.l1')?.textContent).toBe('check the deploy of storefrontevery 5 min');
    expect(rows[0]?.querySelector('.l2')?.textContent).toBe(`deploy-watch · ${scheduled.next_at({ at: '14:22' })}`);
    // 'how often' holds only the rhythm; the count is the pane's 'its runs' (designer m_12105)
    expect(rows[0]?.textContent).not.toContain(scheduled.n_of({ count: 6, total: 24 }));
    // its '…' (the menu: stop and the rest, nm-menus) at its end; no text action link
    expect(screen.getByTestId('scheduled.row.4.more')).toBeTruthy();
    // the lead (no hero card): how many run, then the soonest run
    expect(screen.queryByTestId('scheduled.hero')).toBeNull();
    expect(screen.getByTestId('scheduled.lead').querySelector('.facts')?.textContent).toMatch(/^2 running · next at /);
    expect(screen.queryByTestId('scheduled.keys')).toBeNull();
    const root = screen.getByTestId('scheduled.screen');
    fireEvent.keyDown(root, { key: 'Enter' });
    // its name (else its words' first line), never its id; who it wakes beside it (designer m_15929)
    expect(screen.getByTestId('scheduled.pane.title').textContent).toBe('check the deploy of storefrontdeploy-watch · storefront');
    expect(screen.getByTestId('scheduled.pane').textContent).not.toContain('#4');
    expect(screen.getByTestId('scheduled.stop').textContent).toBe(sentenceCase(scheduled.key_stop));
    expect(screen.getByTestId('scheduled.pane').textContent).toContain(`${scheduled.nth_of({ nth: '7th', total: 24 })} · 14:22`);
    expect(screen.getByTestId('scheduled.pane.words').textContent).toContain('say so only when it fails');
    expect(screen.queryByTestId('scheduled.run')).toBeNull();
    fireEvent.keyDown(root, { key: 'x' });
    expect(screen.getByTestId('scheduled.ask').textContent).toContain(scheduled.stop_ask_desktop({ agent: 'deploy-watch' }));
    fireEvent.keyDown(root, { key: 'Escape' });
    expect(stopped).toEqual([]);
    fireEvent.click(screen.getByTestId('scheduled.stop'));
    fireEvent.click(screen.getByTestId('scheduled.ask.stop'));
    expect(stopped).toEqual(['shop-1#4']);
  });

  it("walks by the TUI's list keys: ↓↓⏎ opens the 3rd, opened the moves stay put, esc closes, ↑ at the top stays, / searches", () => {
    const tasks = [T(task(1, { next_ms: at(0, 15, 0) })), T(task(2, { agent: 'perf', words: 'rerun the bench', next_ms: at(0, 16, 0) })), T(task(3, { next_ms: at(0, 17, 0) }))];
    render(<ScheduledView tasks={tasks} ports={{ stop: () => undefined }} now={NOW} />);
    const root = screen.getByTestId('scheduled.screen');
    const on = () => screen.getByTestId('scheduled.list').querySelector('.b-lrow.on')?.getAttribute('data-testid');
    const shown = () => [...screen.getByTestId('scheduled.list').querySelectorAll('.b-lrow')].map((r) => r.getAttribute('data-testid'));
    fireEvent.keyDown(root, { key: 'Home' });
    expect(on()).toBe('scheduled.row.1');
    fireEvent.keyDown(root, { key: 'ArrowUp' });
    expect(on(), '↑ at the top stays').toBe('scheduled.row.1');
    fireEvent.keyDown(root, { key: 'ArrowDown' });
    fireEvent.keyDown(root, { key: 'j' });
    fireEvent.keyDown(root, { key: 'Enter' });
    expect(on()).toBe('scheduled.row.3');
    expect(screen.getByTestId('scheduled.pane')).toBeTruthy();
    fireEvent.keyDown(root, { key: 'ArrowUp' });
    expect(on(), 'a task is open: the moves stay put').toBe('scheduled.row.3');
    fireEvent.keyDown(root, { key: 'Escape' });
    expect(screen.queryByTestId('scheduled.pane')).toBeNull();
    fireEvent.keyDown(root, { key: 'k' });
    expect(on()).toBe('scheduled.row.2');
    fireEvent.keyDown(root, { key: '/' });
    const field = screen.getByTestId('scheduled.search.input');
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: 'bench' } });
    expect(shown()).toEqual(['scheduled.row.2']);
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(document.activeElement).toBe(root);
    fireEvent.keyDown(root, { key: 'Escape' });
    expect(shown()).toHaveLength(3);
  });

  it("the search is at the right of the list's own row, never in the window's bar (his m_52658); '/' from anywhere, esc clears then gives the keys back", () => {
    render(<ScheduledView tasks={[T(task(1)), T(task(2, { words: 'rerun the bench' }))]} ports={{ stop: () => undefined }} now={NOW} />);
    expect(screen.getByTestId('app.bar.slot').childElementCount).toBe(0);
    const bar = screen.getByTestId('scheduled.bar');
    expect(bar.querySelector('.b-listbar-end [data-testid="scheduled.search"]')).not.toBeNull();
    expect(bar.compareDocumentPosition(screen.getByTestId('scheduled.list')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const field = screen.getByTestId<HTMLInputElement>('scheduled.search.input');
    fireEvent.keyDown(document.body, { key: '/' });
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: 'zzqx' } });
    expect(screen.getByTestId('scheduled.none').textContent).toBe(scheduled.none_match_desktop);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect([field.value, document.activeElement]).toEqual(['', field]);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(document.activeElement).toBe(screen.getByTestId('scheduled.screen'));
  });

  it('nothing scheduled says how one comes', () => {
    render(<ScheduledView tasks={[]} ports={{ stop: () => undefined }} now={NOW} />);
    expect(screen.getByTestId('scheduled.empty').textContent).toBe(scheduled.empty_desktop);
  });
});

describe('how often', () => {
  it("says 'every' once, whether or not the hub's rhythm already has it (designer D11)", () => {
    expect(howOften(task(1, { every: '5m' }))).toBe('every 5 min');
    expect(howOften(task(1, { every: 'every day 07:30' }))).toBe('every day 07:30');
    expect(howOften(task(1, { every: 'Every 10m' }))).toBe('Every 10 min');
  });

  it("a one-shot says 'once', never 'every once' (the hub's every_words rule)", () => {
    expect(howOften(task(1, { every: scheduled.once, times: 1 }))).toBe(scheduled.once);
    expect(howOften(task(1, { every: 'every 20m', times: 1 }))).toBe(scheduled.once);
    expect(howOften(task(1, { every: scheduled.once, times: null }))).toBe(scheduled.once);
    expect(howOften(task(1, { every: '20m', times: 3 }))).toBe(scheduled.every({ when: '20 min' }));
  });
});
