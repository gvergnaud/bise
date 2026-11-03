// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render as rtlRender, screen } from '@testing-library/react';
import type { ReactElement, ReactNode } from 'react';
import { BarSlot } from '@/app/barslot';
import { formatRoute, parseRoute } from '@/app/route';
import { createRouter, RouterContext } from '@/app/router';

// the rows' menus (useEntityMenu, nm-menus) read the router: every render is inside one
const Routed = ({ children }: { children: ReactNode }) => <RouterContext.Provider value={createRouter({ screen: 'running' })}>{children}</RouterContext.Provider>;
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: Routed });
import type { Proc } from '@/proto';
import type { ProcItem, RunningPorts } from './index';

// the terminal is xterm's (a canvas): here its props only
vi.mock('./Output', () => ({
  Output: ({ id, none, live, quiet }: { id: string; none: boolean; live: boolean; quiet: string }) => <div data-testid="running.out" data-id={id} data-none={none} data-live={live} data-quiet={quiet} />,
}));
const { RunningView } = await import('./Screen');

// the window's bar: the page's search is never there (his m_52658)
beforeEach(() => { render(<BarSlot />); });
afterEach(cleanup);

const NOW = new Date(2026, 9, 23, 14, 6).getTime();
const MIN = 60_000;
const proc = (over: Partial<Proc> & Pick<Proc, 'id' | 'agent'>): Proc => ({
  kind: { is: 'bg', n: Number(/^bg(\d+)$/u.exec(over.id)?.[1] ?? 0) },
  cmd: 'pnpm dev',
  pid: 48211,
  started_ms: NOW - 14 * MIN,
  state: { is: 'running' },
  out: 'bg',
  cpu: 12,
  rss_mb: 340,
  procs: 1,
  ...over,
});
const at = (project: string, p: Proc): ProcItem => ({ project, projectName: project === 'shop' ? 'storefront' : project, p });

const dev = at('shop', proc({ id: 'bg3', agent: 'perf', ports: [5173], url: 'http://127.0.0.1:5173' }));
const stripe = at('shop', proc({ id: 'bg1', agent: 'checkout', cmd: 'stripe listen', started_ms: NOW - 22 * MIN }));
const failed = at('shop', proc({ id: 'bg2', agent: 'checkout', cmd: 'pnpm test checkout', state: { is: 'exited', rc: 1 }, ended_ms: NOW - 3 * MIN }));
const nohup = at('tele', proc({ id: 'p9.1', agent: 'p99', kind: { is: 'detached' }, cmd: 'grafana-server', out: 'none' }));

function ports(): RunningPorts & { log: string[] } {
  const log: string[] = [];
  return {
    log,
    watch: (on) => log.push(`watch ${String(on)}`),
    stop: (x) => log.push(`stop ${x.project}/${x.p.agent}/${x.p.id}`),
    open: (u) => log.push(`open ${u}`),
  };
}
const rowIds = () => [...screen.getByTestId('running.list').querySelectorAll('[data-row]')].map((r) => r.getAttribute('data-row'));

describe('the running screen', () => {
  it("the route is 'running', or 'running/<project>/<agent>/<id>' for one's output", () => {
    expect(parseRoute('running')).toEqual({ screen: 'running' });
    expect(parseRoute('running/shop/perf/bg3')).toEqual({ screen: 'running', open: { project: 'shop', agent: 'perf', id: 'bg3' } });
    expect(formatRoute({ screen: 'running', open: { project: 'shop', agent: 'perf', id: 'bg3' } })).toBe('running/shop/perf/bg3');
    expect(parseRoute('running/shop').screen).toBe('notfound');
  });

  it('watches the hubs while shown, and stops when it goes', () => {
    const p = ports();
    const v = render(<RunningView items={[dev]} ports={p} now={NOW} />);
    expect(p.log).toEqual(['watch true']);
    v.unmount();
    expect(p.log).toEqual(['watch true', 'watch false']);
  });

  it("groups by project, running ones under 'running N', ended ones under 'ended today N'", () => {
    render(<RunningView items={[stripe, dev, failed, nohup]} ports={ports()} now={NOW} />);
    // the segmented filter (ds Filters), its count apart in text-3
    expect(screen.getByTestId('running.filter.running').textContent).toBe('Running3');
    expect(screen.getByTestId('running.filter.running').querySelector('.run-n')?.textContent).toBe('3');
    expect(screen.getByTestId('running.filter.ended').textContent).toBe('Ended today1');
    expect(screen.getByTestId('running.filter.running').getAttribute('aria-selected')).toBe('true');
    // the lead (no hero card) and one group label per project (ds GroupLabel)
    expect(screen.queryByTestId('running.hero')).toBeNull();
    expect(screen.getAllByTestId(/^running\.group\./u).map((g) => g.querySelector('.b-glabel')?.textContent)).toEqual(['storefront', 'tele']);
    expect(screen.getByTestId('running.sub').textContent).toBe('3 running in 2 projects · 1 listening');
    expect(rowIds()).toEqual(['shop/checkout/bg1', 'shop/perf/bg3', 'tele/p99/p9.1']);
    // its address on the second line; the chosen row's 'open' opens it (its tip says where)
    expect(screen.getByTestId('running.row.shop/perf/bg3.url').textContent).toBe(' · :5173');
    expect(screen.getByTestId('running.row.shop/perf/bg3.url').querySelector('.b-app-icon')).not.toBeNull();
    fireEvent.click(screen.getByTestId('running.row.shop/perf/bg3.open'));
    expect(screen.getByTestId('running.row.shop/perf/bg3.go').getAttribute('data-tip')).toBe('open localhost:5173');
    // every row has its '…' (its menu: stop, open in a tab: nm-menus); no glyph as a mark
    expect(screen.getByTestId('running.row.shop/perf/bg3.more')).toBeTruthy();
    // running: '∿' in text-3, as the terminal's /processes (green is only a diff's +: designer m_43401)
    expect(screen.getByTestId('running.row.shop/perf/bg3.mark').querySelector('.run-wave')?.textContent).toBe('∿');
    expect(screen.getByTestId('running.row.shop/perf/bg3.mark').getAttribute('data-tip')).toBe('running');
    expect(screen.getByTestId('running.row.shop/perf/bg3.end').textContent).toBe('14 min');
    fireEvent.click(screen.getByTestId('running.filter.ended'));
    expect(rowIds()).toEqual(['shop/checkout/bg2']);
    expect(screen.getByTestId('running.row.shop/checkout/bg2.end').textContent).toBe('ended 3 min ago · rc 1');
    expect(screen.getByTestId('running.row.shop/checkout/bg2.mark').getAttribute('data-tip')).toBe('ended, exit 1');
  });

  it("one project: no head, the hero says it; a row's program by name, its whole command in its tip (designer m_41694 #1 #2)", () => {
    const py = at('shop', proc({ id: 'bg5', agent: 'perf', cmd: '/Library/Developer/CommandLineTools/usr/bin/python3 -m http.server 8765' }));
    render(<RunningView items={[dev, py]} ports={ports()} now={NOW} />);
    expect(document.querySelector('.b-glabel')).toBeNull();
    const cmd = screen.getByTestId('running.row.shop/perf/bg5.cmd');
    expect(cmd.textContent).toBe('python3 -m http.server 8765');
    expect(screen.getByTestId('running.row.shop/perf/bg5.open').getAttribute('data-tip')?.split('\n')[0]).toBe(py.p.cmd);
  });

  it("in the bar's scope (a project's, an agent's 'all ›') its head names no projects (designer m_38700)", () => {
    render(<RunningView items={[dev]} ports={ports()} now={NOW} scoped />);
    expect(screen.getByTestId('running.sub').textContent).toBe('1 running · 1 listening');
  });

  it("s stops the selected one at once: 'stopping…', then its hub's row says how; it stays in place", () => {
    const p = ports();
    const v = render(<RunningView items={[dev, stripe]} ports={p} now={NOW} />);
    fireEvent.click(screen.getByTestId('running.row.shop/perf/bg3.open'));
    fireEvent.keyDown(screen.getByTestId('running.screen'), { key: 's' });
    expect(p.log).toContain('stop shop/perf/bg3');
    expect(screen.getByTestId('running.row.shop/perf/bg3.end').textContent).toBe('stopping…');
    // a second s sends nothing more
    fireEvent.keyDown(screen.getByTestId('running.screen'), { key: 's' });
    expect(p.log.filter((l) => l.startsWith('stop'))).toHaveLength(1);
    const ended = at('shop', { ...dev.p, state: { is: 'stopped_by_you' }, ended_ms: new Date(2026, 9, 23, 14, 5, 30).getTime() });
    expect(rowIds()).toEqual(['shop/checkout/bg1', 'shop/perf/bg3']);
    v.rerender(<RunningView items={[ended, stripe]} ports={p} now={NOW} />);
    expect(rowIds()).toEqual(['shop/checkout/bg1', 'shop/perf/bg3']);
    expect(screen.getByTestId('running.row.shop/perf/bg3.end').textContent).toBe('stopped by you · 14:05');
  });

  it('opens the output beside the list: head, meta, keys; o opens its address; no output kept: the dim line', () => {
    const p = ports();
    render(<RunningView items={[dev, nohup]} ports={p} now={NOW} opened="shop/perf/bg3" />);
    expect(screen.getByTestId('running.pane.cmd').textContent.trim()).toBe('pnpm dev');
    expect(screen.getByTestId('running.pane.meta').textContent).toContain('running for 14 min');
    expect(screen.getByTestId('running.pane.open').getAttribute('data-tip')).toBe('open localhost:5173');
    expect(screen.getByTestId('running.out').getAttribute('data-live')).toBe('true');
    fireEvent.keyDown(screen.getByTestId('running.screen'), { key: 'o' });
    expect(p.log).toContain('open http://127.0.0.1:5173');
    fireEvent.click(screen.getByTestId('running.row.tele/p99/p9.1.open'));
    expect(screen.getByTestId('running.out').getAttribute('data-none')).toBe('true');
    expect(screen.queryByTestId('running.pane.open')).toBeNull();
    fireEvent.keyDown(screen.getByTestId('running.screen'), { key: 'Escape' });
    expect(screen.queryByTestId('running.pane')).toBeNull();
    // none picked: the pane says what it is for (designer m_41694 #12)
    expect(screen.getByTestId('running.pane.none').textContent).toBe('pick a process to read its output.');
  });

  it("the pane shows the whole command, wrapped, highlighted as bash by the thread's highlighter (Gabriel's ask)", () => {
    const cmd = 'export BEND_MEM_LIMIT_MB=16384; R=$PWD; (scripts/heavy-run --low -- cargo test -p sb-core 2>&1 | tail -n 40) && echo "rc=$?"';
    const long = at('shop', proc({ id: 'bg6', agent: 'perf', cmd }));
    render(<RunningView items={[long]} ports={ports()} now={NOW} opened="shop/perf/bg6" />);
    const box = screen.getByTestId('running.pane.cmd');
    // whole, never cut with an ellipsis
    expect(box.textContent.trim()).toBe(cmd);
    expect(box.querySelector('code.language-bash')).not.toBeNull();
    // its tokens colored: a keyword, a variable, a string, a flag each in its own run
    const runs = [...box.querySelectorAll('code > span > span')].map((r) => [r.textContent, (r as HTMLElement).style.color]);
    expect(runs).toContainEqual(['export', 'var(--md-syn-keyword)']);
    expect(runs.some(([t, c]) => t === '"rc=$?"' && c === 'var(--md-syn-string)')).toBe(true);
    // the empty state's words go to the output (quietWords)
    expect(screen.getByTestId('running.out').getAttribute('data-quiet')).toBe('running for 14 min · nothing printed yet');
  });

  it("the search is at the right of the filter's row, never in the window's bar (his m_52658); '/' from anywhere, esc clears then gives the keys back", () => {
    render(<RunningView items={[dev, nohup]} ports={ports()} now={NOW} />);
    expect(screen.getByTestId('app.bar.slot').childElementCount).toBe(0);
    const bar = screen.getByTestId('running.filters');
    expect(bar.querySelector('.b-listbar-start [data-testid="running.filter"]')).not.toBeNull();
    expect(bar.querySelector('.b-listbar-end [data-testid="running.search"]')).not.toBeNull();
    const field = screen.getByTestId<HTMLInputElement>('running.search.input');
    fireEvent.keyDown(document.body, { key: '/' });
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: 'zzqx' } });
    expect(screen.getByTestId('running.empty')).toBeTruthy();
    fireEvent.keyDown(field, { key: 'Escape' });
    expect([field.value, document.activeElement]).toEqual(['', field]);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(document.activeElement).toBe(screen.getByTestId('running.screen'));
  });

  it('stop asks once before it stops: the first click asks, the second stops', () => {
    const p = ports();
    render(<RunningView items={[dev]} ports={p} now={NOW} opened="shop/perf/bg3" />);
    fireEvent.click(screen.getByTestId('running.pane.stop'));
    expect(screen.getByTestId('running.pane.stop').textContent).toBe('Stop it?');
    expect(p.log.filter((l) => l.startsWith('stop'))).toHaveLength(0);
    fireEvent.click(screen.getByTestId('running.pane.stop'));
    expect(p.log).toContain('stop shop/perf/bg3');
  });

  it("a route to an ended one opens it under 'ended today'", () => {
    render(<RunningView items={[dev, failed]} ports={ports()} now={NOW} opened="shop/checkout/bg2" />);
    expect(screen.getByTestId('running.filter.ended').getAttribute('aria-selected')).toBe('true');
    expect(screen.getByTestId('running.out').getAttribute('data-live')).toBe('false');
  });
});
