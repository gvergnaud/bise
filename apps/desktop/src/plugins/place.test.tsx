// @vitest-environment happy-dom
// The plugins place from props (Place.tsx, PluginPage.tsx, Add.tsx): the head's counts and the
// band from the core's summary, a row's one control and what it sends, the kind filter, the
// add sheet's secrets (sent once with the confirm, then dropped), a plugin's page and remove.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppCmd, Plugin, PluginsSummary } from '@/proto';
import { core_ev } from '@/proto/fixtures.gen';
import { sentenceCase } from '@/ds';
import { settings } from '@/proto/words.gen';
import { PlaceView } from './Place';
import { PluginPage } from './PluginPage';
import { BarSlot } from '@/app/barslot';
import { AddSheet } from './Add';

afterEach(cleanup);

const full = (): PluginsSummary => {
  const e = core_ev.find((x) => x.ev === 'plugins_summary' && x.project == null && x.summary.on > 0);
  if (e?.ev !== 'plugins_summary') throw new Error('fixture');
  return e.summary;
};
const preview = () => {
  const e = core_ev.find((x) => x.ev === 'plugin_preview');
  if (e?.ev !== 'plugin_preview') throw new Error('fixture');
  return { token: e.token, preview: e.preview };
};

const row = (id: string, more: Partial<Plugin> = {}): Plugin => ({ id, name: id, kind: 'plugin', scope: 'user', title: id, state: 'loaded', on: true, ...more });
const ROWS: Plugin[] = [
  row('skills:bise', { kind: 'skills', scope: 'built-in', title: 'bise', locked: true }),
  row('computer', { scope: 'built-in', on: false, state: 'disabled' }),
  row('blender', { what: 'drive your running Blender', parts: [{ id: 'blender#skill:blender', kind: 'skill', name: 'blender', on: true }, { id: 'blender#mcp:blender', kind: 'mcp', name: 'blender', on: true, tool_count: 23 }] }),
  row('mistral:outlook', { kind: 'mistral', scope: 'mistral', title: 'Outlook Calendar', login: 'needs' }),
  row('linear', { kind: 'mcp', parts: [{ id: 'linear#mcp:linear', kind: 'mcp', name: 'linear', on: true, login: 'needs' }] }),
  row('pg', { kind: 'mcp', parts: [{ id: 'pg#mcp:pg', kind: 'mcp', name: 'pg', on: true, error: 'exited at start: DATABASE_URL is not set' }] }),
];
const BROWSE = [{ source: 'mistral', items: [row('mistral:linear', { kind: 'mistral', scope: 'mistral', title: 'Linear', state: 'available', on: false, action: 'connect', what: 'issues, projects and cycles' })] }];

function place(over: Partial<Parameters<typeof PlaceView>[0]> = {}) {
  const sent: AppCmd[] = [];
  const ports = { send: (c: AppCmd) => { sent.push(c); }, open: vi.fn(), setUpComputer: vi.fn(), add: vi.fn() };
  render(<PlaceView rows={ROWS} summary={full()} browse={BROWSE} ports={ports} {...over} />);
  return { sent, ports };
}

describe('the plugins place', () => {
  it("heads with the core's counts and the band; connect sends plugin_connect", () => {
    const { sent } = place();
    expect(screen.getByTestId('plugins.counts').textContent).toBe('16 on · 2 off · 3 need you');
    expect(screen.getByTestId('plugins.band.count').textContent).toBe('3 of 10 connected');
    expect(screen.getByTestId('plugins.band.import').textContent).toContain('3 MCP servers found on this Mac: Claude Code, Cursor');
    fireEvent.click(screen.getByTestId('plugins.band.mistral:linear.connect'));
    expect(sent).toEqual([{ cmd: 'plugin_connect', id: 'mistral:linear' }]);
  });

  it("says why each card: the core's why under the title, the description and where it was seen in its tip", () => {
    place();
    const sentry = screen.getByTestId('plugins.band.mistral:sentry.why');
    expect(sentry.textContent).toBe('SentrySENTRY_DSN in 3 of your projects');
    expect(sentry.getAttribute('data-tip')).toBe('seen in harness, ai-docs, bend-lab');
    const linear = screen.getByTestId('plugins.band.mistral:linear.why');
    expect(linear.textContent).toBe('Lineartrack and update your issues and projects');
    // the description left the card for its tip; a generic why says no 'seen in'
    expect(linear.getAttribute('data-tip')).toBe('issues, projects and cycles');
    // his one marketplace is named
    expect(screen.getByTestId('plugins.band.market').textContent).toContain('2 new plugins in openai/plugins');
  });

  it('draws a connector with no row in the same tile, its capital initial; the core names it; the grid ends on browse N more (ui-review #4, #5, #9)', () => {
    place();
    const icon = screen.getByTestId('plugins.band.mistral:google_calendar.icon');
    expect(icon.className).toBe('plg-icon row');
    expect(icon.textContent).toBe('G');
    expect(screen.getByTestId('plugins.band.mistral:google_calendar.title').textContent).toBe('Google Calendar');
    // the core's n is 6 for 5 ids: 5 cards and the 6th cell
    expect(screen.getByTestId('plugins.band.more').textContent).toBe(sentenceCase(settings.plugins_band_more({ n: 1 })));
  });

  it("gives each row its one control: always on, set up, connect, log in, details + switch", () => {
    const { sent, ports } = place();
    expect(screen.getByTestId('plugins.row.skills:bise').textContent).toContain('always on');
    expect(screen.queryByTestId('plugins.row.skills:bise.toggle')).toBeNull();
    fireEvent.click(screen.getByTestId('plugins.row.computer.set_up'));
    expect(ports.setUpComputer).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByTestId('plugins.row.mistral:outlook.connect'));
    fireEvent.click(screen.getByTestId('plugins.row.linear.login'));
    fireEvent.click(screen.getByTestId('plugins.row.blender.toggle'));
    expect(sent).toEqual([
      { cmd: 'plugin_connect', id: 'mistral:outlook' },
      { cmd: 'plugin_login', name: 'linear' },
      { cmd: 'plugin_set', name: 'blender', on: false },
    ]);
    expect(screen.getByTestId('plugins.row.blender').textContent).toContain('1 skill · 23 tools');
    expect(screen.getByTestId('plugins.row.pg').textContent).toContain('✗ exited at start: DATABASE_URL is not set');
    expect(screen.getByTestId('plugins.row.pg.details')).toBeTruthy();
    fireEvent.click(screen.getByTestId('plugins.row.blender'));
    expect(ports.open).toHaveBeenCalledWith('blender');
  });

  // his m_62905: no installed | browse tabs; one list, installed first, then 'Not installed'
  const order = () => Array.from(screen.getByTestId('plugins.list').querySelectorAll('[data-testid^="plugins.group."], [data-testid="plugins.avail.head"], [data-testid^="plugins.avail.mistral"]')).map((e) => e.getAttribute('data-testid'));

  it("one list: the installed rows first, then 'Not installed' with what he can add; the kind chips filter both (/skills opens on skills)", () => {
    place();
    expect(screen.queryByTestId('plugins.tabs')).toBeNull();
    expect(order()).toEqual(['plugins.group.plugin', 'plugins.group.mistral', 'plugins.group.mcp', 'plugins.group.skills', 'plugins.avail.head', 'plugins.avail.mistral']);
    expect(screen.getByTestId('plugins.avail.head').textContent).toBe(sentenceCase(settings.plugins_not_installed));
    fireEvent.click(screen.getByTestId('plugins.avail.row.mistral:linear.connect'));
    cleanup();
    place({ kind: 'skills' });
    expect(order()).toEqual(['plugins.group.skills']);
    fireEvent.click(screen.getByTestId('plugins.kind.mistral'));
    expect(order()).toEqual(['plugins.group.mistral', 'plugins.avail.head', 'plugins.avail.mistral']);
  });

  it('one search: an installed and a not-installed match both show', () => {
    place();
    fireEvent.change(screen.getByTestId('plugins.search.input'), { target: { value: 'linear' } });
    expect(screen.getByTestId('plugins.row.linear')).toBeTruthy();
    expect(screen.getByTestId('plugins.avail.row.mistral:linear')).toBeTruthy();
    expect(screen.queryByTestId('plugins.row.blender')).toBeNull();
    fireEvent.change(screen.getByTestId('plugins.search.input'), { target: { value: 'blender' } });
    expect(screen.queryByTestId('plugins.avail')).toBeNull();
  });

  it("nothing installed: the list starts at 'Not installed'; nothing to add either: one plain line", () => {
    place({ rows: [] });
    expect(order()).toEqual(['plugins.avail.head', 'plugins.avail.mistral']);
    // the band says only what the list can't: no connector cards (the rows below), its import and marketplace lines (designer m_63123)
    expect(screen.queryByTestId('plugins.band.mistral:linear')).toBeNull();
    expect(screen.queryByTestId('plugins.band.sub')).toBeNull();
    expect(screen.getByTestId('plugins.band.import')).toBeTruthy();
    expect(screen.getByTestId('plugins.band.market')).toBeTruthy();
    cleanup();
    place({ rows: [], browse: [] });
    expect(screen.getByTestId('plugins.none').textContent).toBe(settings.plugins_nothing_installed);
  });

  it("add's browse lands on the 'Not installed' head with every row back, tinted for a moment", () => {
    const seen: Element[] = [];
    const before = Object.getOwnPropertyDescriptor(Element.prototype, 'scrollIntoView');
    Element.prototype.scrollIntoView = function (this: Element) { seen.push(this); };
    vi.useFakeTimers();
    try {
      place({ kind: 'skills' });
      fireEvent.click(screen.getByTestId('plugins.add'));
      fireEvent.click(screen.getByTestId('plugins.add.browse'));
      const head = screen.getByTestId('plugins.avail.head');
      expect(seen).toEqual([head]);
      expect(head.className).toContain('lit');
      expect(screen.getByTestId('plugins.group.plugin')).toBeTruthy();
      act(() => { vi.advanceTimersByTime(1000); });
      expect(head.className).not.toContain('lit');
    } finally {
      vi.useRealTimers();
      if (before !== undefined) Object.defineProperty(Element.prototype, 'scrollIntoView', before);
    }
  });

  it("the search is at the right of the kind chips, before add, never in the window's bar (his m_52658); '/' from anywhere, esc clears then leaves", () => {
    render(<BarSlot />);
    place();
    expect(screen.getByTestId('app.bar.slot').childElementCount).toBe(0);
    const bar = screen.getByTestId('plugins.list.row');
    expect(bar.querySelector('.b-listbar-start [data-testid="plugins.kinds"]')).not.toBeNull();
    const end = bar.querySelector('.b-listbar-end');
    expect(end?.firstElementChild?.getAttribute('data-testid')).toBe('plugins.search');
    const field = screen.getByTestId<HTMLInputElement>('plugins.search.input');
    fireEvent.keyDown(document.body, { key: '/' });
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: 'zzqx' } });
    expect(screen.getByTestId('plugins.none')).toBeTruthy();
    fireEvent.keyDown(field, { key: 'Escape' });
    expect([field.value, document.activeElement]).toEqual(['', field]);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(document.activeElement).not.toBe(field);
  });

  it('says it reads before the rows came; no band before the summary', () => {
    place({ rows: null, summary: null });
    expect(screen.getByTestId('plugins.loading')).toBeTruthy();
    expect(screen.queryByTestId('plugins.band')).toBeNull();
  });
});

describe('the add sheet', () => {
  it("sends the secrets' names first, their values only with the confirm, and forgets them", () => {
    const sent: AppCmd[] = [];
    const onClose = vi.fn();
    const props = { item: 'mcp' as const, project: undefined, summary: full(), ports: { send: (c: AppCmd) => { sent.push(c); } }, onClose };
    const r = render(<AddSheet {...props} preview={null} />);
    fireEvent.change(screen.getByTestId('plugins.sheet.mcp.name'), { target: { value: 'linear' } });
    fireEvent.change(screen.getByTestId('plugins.sheet.mcp.target'), { target: { value: 'npx -y linear-mcp' } });
    fireEvent.change(screen.getByTestId('plugins.sheet.mcp.env'), { target: { value: 'LINEAR_TOKEN=sk-PLANTED' } });
    fireEvent.click(screen.getByTestId('plugins.sheet.go'));
    expect(sent).toEqual([{ cmd: 'plugin_add', add: { kind: 'mcp', name: 'linear', command: 'npx -y linear-mcp', secrets: ['LINEAR_TOKEN'] } }]);
    expect(JSON.stringify(sent)).not.toContain('sk-PLANTED');
    // the preview came: what runs, then add
    const p = preview();
    const withSecret = { ...p, preview: { ...p.preview, secrets: ['LINEAR_TOKEN'] } };
    r.rerender(<AddSheet {...props} preview={withSecret} />);
    expect(screen.getByTestId('plugins.preview.runs').textContent).toContain('npx -y latex-mcp');
    fireEvent.click(screen.getByTestId('plugins.preview.add'));
    expect(sent[1]).toEqual({ cmd: 'plugin_add_confirm', token: p.token, secrets: [{ name: 'LINEAR_TOKEN', value: 'sk-PLANTED' }] });
    expect(onClose).toHaveBeenCalledOnce();
    // opened again: nothing of the last one is left
    r.rerender(<AddSheet {...props} preview={null} />);
    expect(screen.getByTestId<HTMLTextAreaElement>('plugins.sheet.mcp.env').value).toBe('');
  });

  it('cancel on the preview sends plugin_add_cancel', () => {
    const sent: AppCmd[] = [];
    render(<AddSheet item="plugin" project={undefined} summary={null} preview={null} ports={{ send: (c) => { sent.push(c); } }} onClose={() => {}} />);
    fireEvent.change(screen.getByTestId('plugins.sheet.plugin.source'), { target: { value: 'openai/plugins' } });
    fireEvent.click(screen.getByTestId('plugins.sheet.go'));
    expect(sent).toEqual([{ cmd: 'plugin_add', add: { kind: 'git', url: 'openai/plugins' } }]);
    cleanup();
    render(<AddSheet item="plugin" project={undefined} summary={null} preview={preview()} ports={{ send: (c) => { sent.push(c); } }} onClose={() => {}} />);
    fireEvent.click(screen.getByTestId('plugins.sheet.cancel'));
    expect(sent.at(-1)).toEqual({ cmd: 'plugin_add_cancel', token: preview().token });
  });
});

describe("a plugin's page", () => {
  it('switches its parts, tries a line, removes his own after one confirm (the bar\'s ⋯: Remove…)', async () => {
    const sent: AppCmd[] = [];
    const ports = { send: (c: AppCmd) => { sent.push(c); }, back: vi.fn(), tryIt: vi.fn(), setUpComputer: vi.fn(), remove: vi.fn() };
    const blender = { ...(ROWS[2] as Plugin), prompts: ['make a low-poly fox'], path: '/Users/x/.agents/plugins/blender' };
    const popupMenu = vi.fn(() => Promise.resolve('remove'));
    (window as unknown as { bise?: unknown }).bise = { popupMenu };
    render(<><BarSlot /><PluginPage p={blender} ports={ports} /></>);
    fireEvent.click(screen.getByTestId('plugins.page.blender.try.0'));
    expect(ports.tryIt).toHaveBeenCalledWith('make a low-poly fox');
    fireEvent.click(screen.getByTestId('plugins.part.blender#skill:blender.toggle'));
    expect(sent).toEqual([{ cmd: 'plugin_set', name: 'blender#skill:blender', on: false }]);
    fireEvent.click(screen.getByTestId('plugins.page.blender.more'));
    expect(popupMenu).toHaveBeenCalledWith([{ id: 'remove', label: settings.plugins_remove_menu }]);
    expect(ports.remove).not.toHaveBeenCalled();
    expect((await screen.findByTestId('plugins.page.blender.ask')).textContent).toContain('remove blender? its saved secrets and logins go too.');
    fireEvent.click(screen.getByTestId('plugins.page.blender.ask.yes'));
    expect(ports.remove).toHaveBeenCalledWith('blender');
    delete (window as unknown as { bise?: unknown }).bise;
  });

  it('no remove on a built-in, nor before the core has plugin_remove', () => {
    render(<PluginPage p={row('docs', { scope: 'built-in' })} ports={{ send: () => {}, back: () => {}, tryIt: () => {}, setUpComputer: () => {}, remove: () => {} }} />);
    expect(screen.queryByTestId('plugins.page.docs.more')).toBeNull();
    cleanup();
    render(<PluginPage p={row('pay')} ports={{ send: () => {}, back: () => {}, tryIt: () => {}, setUpComputer: () => {} }} />);
    expect(screen.queryByTestId('plugins.page.pay.remove')).toBeNull();
  });
});

describe("File types: the installed part's last group, where every pick is seen and reset (docs/plugin-apps.md A3)", () => {
  const sheets = { plugin: 'sheets', tool: 'grid', app: 'Sheets', type: '*.xlsx', type_title: 'Excel workbook' };
  const grid = { plugin: 'grid', tool: 'open', app: 'Grid', type: '*.csv', type_title: 'CSV table' };
  const TYPES = [
    { type: '*.xlsx', title: 'Excel workbook', app: sheets, others: [] },
    { type: '*.csv', title: 'CSV table', app: grid, others: [{ ...sheets, type: '*.csv' }], his: true },
  ];

  it("one row per type: '.csv  Grid  your pick  Reset'; a pick and Reset send opener_set", () => {
    const { sent } = place({ fileTypes: TYPES });
    expect(screen.getByTestId('plugins.types.row.*.xlsx.app').textContent).toBe('Sheets');
    expect(screen.getByTestId('plugins.types.row.*.xlsx.status').textContent).toBe(settings.file_type_default);
    expect(screen.queryByTestId('plugins.types.row.*.xlsx.reset')).toBeNull();
    expect(screen.getByTestId('plugins.types.row.*.csv.status').textContent).toBe(settings.file_type_his);
    fireEvent.click(screen.getByTestId('plugins.types.row.*.csv.app'));
    fireEvent.click(screen.getByTestId('plugins.types.row.*.csv.pick.sheets#grid'));
    fireEvent.click(screen.getByTestId('plugins.types.row.*.csv.reset'));
    expect(sent).toEqual([{ cmd: 'opener_set', type: '*.csv', plugin: 'sheets', tool: 'grid' }, { cmd: 'opener_set', type: '*.csv' }]);
  });

  it("sits after the installed groups, before 'Not installed', under All only (designer m_62919); none: no group", () => {
    place({ fileTypes: TYPES });
    const list = screen.getByTestId('plugins.list');
    const at = (id: string) => Array.from(list.querySelectorAll('[data-testid]')).findIndex((e) => e.getAttribute('data-testid') === id);
    expect(at('plugins.group.skills')).toBeLessThan(at('plugins.types'));
    expect(at('plugins.types')).toBeLessThan(at('plugins.avail.head'));
    fireEvent.click(screen.getByTestId('plugins.kind.plugin'));
    expect(screen.queryByTestId('plugins.types')).toBeNull();
    cleanup();
    place();
    expect(screen.queryByTestId('plugins.types')).toBeNull();
    expect(screen.queryByTestId('plugins.types.empty')).toBeNull();
  });
});
