// The plugins place's pure part (model.ts): a row's control from typed fields only, its words,
// the list's groups and filters, the sidebar row's words and the band's from the core's summary,
// and the 'new to connect' dot's key (seen.ts).
import { describe, expect, it } from 'vitest';
import type { Plugin, PluginPart, PluginsSummary } from '@/proto';
import { core_ev } from '@/proto/fixtures.gen';
import { settings } from '@/proto/words.gen';
import { availableOf, badgeOf, bandCells, bandWords, controlOf, gives, groupsOf, hitOf, leadOf, lineOf, newKey, removable, sideOf, waitsWords } from './model';
import { isNew, markSeen, seenKey, type SeenStore } from './seen';

const row = (id: string, more: Partial<Plugin> = {}): Plugin => ({ id, name: id, kind: 'plugin', scope: 'user', title: id, state: 'loaded', on: true, ...more });
const part = (id: string, kind: PluginPart['kind'], more: Partial<PluginPart> = {}): PluginPart => ({ id, kind, name: id.split(':').at(-1) ?? id, on: true, ...more });

const full = (): PluginsSummary => {
  const e = core_ev.find((x) => x.ev === 'plugins_summary' && x.project == null && x.summary.on > 0);
  if (e?.ev !== 'plugins_summary') throw new Error('fixture');
  return e.summary;
};
const empty = (): PluginsSummary => {
  const e = core_ev.find((x) => x.ev === 'plugins_summary' && x.project != null);
  if (e?.ev !== 'plugins_summary') throw new Error('fixture');
  return e.summary;
};

describe("a row's control", () => {
  it('reads typed fields, never the status words', () => {
    expect(controlOf(row('docs'))).toEqual({ kind: 'toggle' });
    expect(controlOf(row('skills:bise', { kind: 'skills', locked: true }))).toEqual({ kind: 'locked' });
    expect(controlOf(row('computer', { scope: 'built-in', on: false, state: 'disabled' }))).toEqual({ kind: 'set_up_cu' });
    expect(controlOf(row('mistral:outlook', { kind: 'mistral', login: 'needs' }))).toEqual({ kind: 'connect', id: 'mistral:outlook' });
    const linear = row('linear', { parts: [part('linear#mcp:linear', 'mcp', { login: 'needs' })] });
    expect(controlOf(linear)).toEqual({ kind: 'log_in', name: 'linear', pending: false });
    expect(badgeOf(linear)).toBe('?');
    const pg = row('pg', { parts: [part('pg#mcp:pg', 'mcp', { error: 'exited at start: DATABASE_URL is not set' })] });
    expect(controlOf(pg)).toEqual({ kind: 'failed', why: 'exited at start: DATABASE_URL is not set' });
    expect(badgeOf(pg)).toBe('✗');
    expect(lineOf({ p: pg })).toContain('DATABASE_URL');
    // the words alone: a switch, no badge
    const said = row('said', { parts: [part('said#mcp:s', 'mcp', { status: 'needs a login' })] });
    expect(controlOf(said)).toEqual({ kind: 'toggle' });
    expect(badgeOf(said)).toBeNull();
    // off: nothing waits
    expect(badgeOf({ ...pg, on: false, state: 'disabled' })).toBeNull();
  });

  it('says what it gives; remove only on a plugin of his own folder', () => {
    const blender = row('blender', { parts: [part('blender#skill:blender', 'skill'), part('blender#mcp:blender', 'mcp', { tool_count: 23 })] });
    expect(gives(blender)).toBe('1 skill · 23 tools');
    expect(gives(row('cu', { parts: [part('cu#skill:cu', 'skill'), part('cu#mcp:cu', 'mcp')] }))).toBe('1 skill · 1 server');
    expect(removable(row('pay'))).toBe(true);
    expect(removable(row('docs', { scope: 'built-in' }))).toBe(false);
    expect(removable(row('mistral:gmail', { kind: 'mistral', scope: 'mistral' }))).toBe(false);
    expect(removable(row('skills:personal', { kind: 'skills' }))).toBe(false);
  });
});

describe('the list', () => {
  const rows: Plugin[] = [
    row('docs', { scope: 'built-in', what: 'Word documents' }),
    row('mistral:notion', { kind: 'mistral', scope: 'mistral', title: 'Notion', parts: [part('mistral:notion#tool:notion_sync_github_issue', 'tool')] }),
    row('mistral:linear', { kind: 'mistral', scope: 'mistral', title: 'Linear', state: 'available', on: false }),
    row('pg', { kind: 'mcp' }),
    row('skills:personal', { kind: 'skills', title: 'personal skills' }),
    row('old', { state: 'shadowed' }),
  ];

  it('groups by kind in order, Browse and shadowed rows out', () => {
    expect(groupsOf(rows, 'all', '').map((g) => [g.kind, g.hits.map((h) => h.p.id)])).toEqual([
      ['plugin', ['docs']],
      ['mistral', ['mistral:notion']],
      ['mcp', ['pg']],
      ['skills', ['skills:personal']],
    ]);
    expect(groupsOf(rows, 'skills', '').map((g) => g.kind)).toEqual(['skills']);
  });

  // his m_62905: one list, installed first, then what he can add, under one search and one chip
  const browse = [
    { source: 'mistral', items: [row('mistral:linear', { kind: 'mistral', scope: 'mistral', title: 'Linear', state: 'available', on: false, what: 'issues and cycles' }), row('mistral:notion', { kind: 'mistral', scope: 'mistral', title: 'Notion', state: 'available', on: false }), row('mistral:slack', { kind: 'mistral', scope: 'mistral', title: 'Slack', state: 'available', on: false, what: 'channels and threads' })] },
    { source: 'openai/plugins', items: [row('figma', { state: 'available', on: false, what: 'read your Figma issues' })] },
    { source: 'empty/market', items: [] },
  ];
  const ids = (g: readonly { readonly hits: readonly { readonly p: Plugin }[] }[]) => g.map((x) => x.hits.map((h) => h.p.id));

  it('the not-installed part: each source in the core order, a row he has already out, empty sources out', () => {
    expect(availableOf(browse, rows, 'all', '').map((b) => [b.source, b.hits.map((h) => h.p.id)])).toEqual([
      ['mistral', ['mistral:linear', 'mistral:slack']],
      ['openai/plugins', ['figma']],
    ]);
    // a chip filters it like the installed part
    expect(ids(availableOf(browse, rows, 'mistral', ''))).toEqual([['mistral:linear', 'mistral:slack']]);
    expect(availableOf(browse, rows, 'skills', '')).toEqual([]);
  });

  it('one search: an installed and a not-installed match both come back', () => {
    expect(ids(groupsOf(rows, 'all', 'issue'))).toEqual([['mistral:notion']]);
    expect(ids(availableOf(browse, rows, 'all', 'issue'))).toEqual([['mistral:linear'], ['figma']]);
  });

  it('a search finds a tool inside a row and says which', () => {
    const g = groupsOf(rows, 'all', 'github');
    expect(g.map((x) => x.hits.map((h) => h.p.id))).toEqual([['mistral:notion']]);
    const h = g[0]?.hits[0];
    expect(h?.part?.name).toBe('notion_sync_github_issue');
    expect(h === undefined ? '' : lineOf(h)).toBe('tool notion_sync_github_issue');
    expect(hitOf(rows[0] as Plugin, 'word')?.part).toBeUndefined();
  });
});

describe('the summary', () => {
  it("the sidebar: the plugins on, or what waits with its tip; nothing before the core's summary", () => {
    expect(sideOf(null)).toEqual({ end: '', waits: 0, tip: '' });
    const s = full();
    const side = sideOf(s);
    expect(side.waits).toBe(3);
    expect(side.end).toBe('');
    expect(side.tip).toBe('16 plugins on · 3 need you: Outlook Calendar needs a login, linear needs a login, postgres-dev failed to start');
    expect(waitsWords(s)).toHaveLength(3);
    const calm = sideOf({ ...s, waiting: 0 });
    expect(calm).toEqual({ end: '16', waits: 0, tip: '16 plugins on' });
    expect(sideOf(empty()).end).toBe('');
  });

  it('the dot: something new to connect he has not seen; seen once the band showed', () => {
    const mem = new Map<string, string>();
    const st: SeenStore = { getItem: (k) => mem.get(k) ?? null, setItem: (k, v) => { mem.set(k, v); } };
    const s = full();
    expect(newKey(empty())).toBe('');
    expect(isNew(empty(), '')).toBe(false);
    expect(isNew(s, seenKey(st))).toBe(true);
    markSeen(s, st);
    expect(isNew(s, seenKey(st))).toBe(false);
    // a login waiting is not 'new to connect': same key
    expect(isNew({ ...s, waiting: 0, suggest: (s.suggest ?? []).filter((x) => x.kind !== 'login') }, seenKey(st))).toBe(false);
    // a new connector to suggest: the dot again
    const more = { ...s, suggest: (s.suggest ?? []).map((x) => (x.kind === 'mistral' ? { ...x, ids: ['mistral:hex', ...(x.ids ?? []).slice(0, 4)] } : x)) };
    expect(isNew(more, seenKey(st))).toBe(true);
  });

  it("the band: a why every card says goes up to the sub once, each card then says what its tool does (ui-review #6)", () => {
    const m = (full().suggest ?? []).find((x) => x.kind === 'mistral');
    if (m === undefined) throw new Error('fixture');
    const rows = new Map<string, Plugin>([['mistral:linear', { id: 'mistral:linear', name: 'mistral:linear', kind: 'mistral', scope: 'mistral', title: 'Linear', state: 'available', on: false, what: 'issues, projects and cycles' }]]);
    const mixed = bandWords(m, rows);
    expect(mixed.sub).toBe(settings.plugins_band_sub);
    expect(mixed.cards[1]).toEqual({ line: 'track and update your issues and projects', tip: 'issues, projects and cycles' });
    const same = bandWords({ ...m, whys: (m.whys ?? []).map(() => ({ text: settings.plugins_why_mistral, source: 'generic' as const })) }, rows);
    expect(same.sub).toBe(settings.plugins_band_sub_why({ why: settings.plugins_why_mistral }));
    expect(same.cards.map((c) => c.line)).toEqual(['', 'issues, projects and cycles', '', '', '']);
  });

  it('the band grid: 2, 4 or 6 cells, the last one browse N more when more wait (ui-review #9)', () => {
    expect(bandCells(5, 6)).toEqual({ shown: 5, more: 1 });
    expect(bandCells(5, 122)).toEqual({ shown: 5, more: 117 });
    expect(bandCells(5, 5)).toEqual({ shown: 3, more: 2 });
    expect(bandCells(4, 4)).toEqual({ shown: 4, more: 0 });
    expect(bandCells(3, 3)).toEqual({ shown: 1, more: 2 });
    expect(bandCells(2, 9)).toEqual({ shown: 1, more: 8 });
    expect(bandCells(2, 2)).toEqual({ shown: 2, more: 0 });
    expect(bandCells(1, 1)).toEqual({ shown: 1, more: 0 });
    expect(bandCells(0, 0)).toEqual({ shown: 0, more: 0 });
  });
});

describe('a lead', () => {
  it('ends with a period, but not after a path or an address (designer m_44086)', () => {
    expect(leadOf("bise's own skills")).toBe("bise's own skills.");
    expect(leadOf('done already.')).toBe('done already.');
    expect(leadOf('')).toBe('');
    expect(leadOf('linear from ~/.agents/plugins/linear')).toBe('linear from ~/.agents/plugins/linear');
    expect(leadOf('served by mcp.linear.app')).toBe('served by mcp.linear.app');
  });
});
