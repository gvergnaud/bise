// The plugins place (page plugins v48, plugins-ui's screens; bise fait mac §9, nm-pages-b). Top to
// bottom: its banner (ds PageBanner: 'Plugins', what it is, the core's counts, the room in its box:
// bims' diorama of his plugins as office objects, Room.tsx, their icons on a floor without WebGL),
// 'connect your tools' (Mistral's connectors not connected yet, most useful first, the MCP servers
// found in other apps, a repo's own skills), then installed | browse (ds Filters, add at their
// right) with the kind chips, and the list in sentence-case ds GroupLabels: one row per plugin, its
// object as its icon, its tags, what it gives and its one control (a switch; 'always on'; connect /
// log in where it waits on his account; a failed server's red line and details); a skill the search
// found opens in its own tab (SkillTab.tsx). The search is in the list's row (ds ListBar: installed |
// browse left, the search then add right; '/' and ⌘F focus it, app/searchkeys.ts). Adding goes
// through add, the band and browse only. Every number is the core's (catalog::rows,
// catalog::summary); every change is the core's (plugin_set, plugin_connect, plugin_login,
// plugin_add…), answered by `plugins` again. Owner: nm-pages-b (plug-desktop's before).
import { useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { BannerWait, Button, Chip, Filters, GroupLabel, ListBar, ListSearch, PageBanner, sentenceCase, Toggle, tipAttrs } from '@/ds';
import { usePageSearchKeys } from '@/app/searchkeys';
import { hero } from '@/proto/words_hero.gen';
import { openTab } from '@/app/panel/tabs';
import { useBannerGutter } from '@/artifacts/hero/vignettes';
import type { AppCmd, FileType, Plugin, PluginsSummary, ConnectorsProblem } from '@/proto';
import { settings } from '@/proto/words.gen';
import {
  bandCells,
  bandShows,
  bandWords,
  controlOf,
  foundWords,
  gives,
  groupWords,
  groupsOf,
  itemsOf,
  kindCounts,
  kindWords,
  KINDS,
  lineOf,
  marketWords,
  tagsOf,
  titleOf,
  type Hit,
  type KindFilter,
} from './model';
import { MonoTile, PluginIcon } from './PluginIcon';
import { Room } from './Room';
import { AddMenu, type AddItem } from './Add';
import { FileTypes } from './FileTypes';
import './plugins.css';

/** What the place does (the shell's live ones, or a test's). */
export interface PlacePorts {
  send: (cmd: AppCmd) => void;
  /** a plugin's page (its id) */
  open: (id: string) => void;
  /** computer's setup (settings › computer use) */
  setUpComputer: () => void;
  /** an add menu item: its sheet */
  add: (item: AddItem) => void;
}

export interface PlaceProps {
  /** the installed rows (`plugins`): null until the core sent them */
  rows: readonly Plugin[] | null;
  summary: PluginsSummary | null;
  /** what to add: Mistral's connectors not connected yet, then each marketplace's */
  browse: readonly { readonly source: string; readonly items: readonly Plugin[] }[];
  /** why Mistral's connectors may be missing or old */
  problem?: ConnectorsProblem | null;
  /** the project the list is for (none: every project, bise's home) */
  project?: string;
  /** the project's name, for the band's workspace line */
  projectName?: string;
  /** /skills: the list on skills */
  kind?: KindFilter;
  ports: PlacePorts;
  /** the room's own drawing (bims' diorama); none: the plain stand-in */
  room?: ReactNode;
  /** which app opens what (the plugins event's file_types): the File types group */
  fileTypes?: readonly FileType[];
}

type Tab = 'installed' | 'browse' | 'types';
const TABS: readonly Tab[] = ['installed', 'browse', 'types'];

export function PlaceView({ rows, summary, browse, problem, project, projectName, kind: kind0, ports, room, fileTypes = [] }: PlaceProps) {
  const [tab, setTab] = useState<Tab>('installed');
  // /skills and /plugins mount it anew (the screen keys it by its kind)
  const [kind, setKind] = useState<KindFilter>(kind0 ?? 'all');
  const [q, setQ] = useState('');
  const searchRef = useRef<HTMLInputElement>(null);
  const gutter = useBannerGutter();
  const at = project === undefined ? {} : { project };
  const all = useMemo(() => rows ?? [], [rows]);
  const groups = useMemo(() => groupsOf(all, kind, q), [all, kind, q]);
  const counts = useMemo(() => kindCounts(all), [all]);
  const toBrowse = browse.reduce((n, b) => n + b.items.length, 0);
  // three sections at --gap-page (the banner, connect your tools, the list); inside the list, its
  // filters (installed | browse, add at their right), the chips and the groups at --gap-block
  // (ui-review #1, #2, #10); the search is in the list's row and filters the list
  usePageSearchKeys(searchRef);
  // its esc clears the words, a second esc leaves it (Artifacts' search, designer m_52674)
  const searchKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    if (q !== '') setQ('');
    else e.currentTarget.blur();
  };
  return (
    <section className="plg-place" data-testid="plugins.place">
      {/* the page's banner (ds PageBanner, designer m_42762): its name, what it is, its counts (the
          accent only on what needs him), the live room (Room.tsx) in its 360 x 176 box; full width: 32 in */}
      <PageBanner title={hero.plugins_title} sentence={settings.plugins_place_sub} edge="column" gutter={gutter} icon="plugins" testId="plugins.head"
        {...(summary !== null ? { facts: <span data-testid="plugins.counts">{settings.plugins_place_counts({ on: summary.on, off: summary.off })}{summary.waiting > 0 && <>{' · '}<BannerWait>{settings.plugins_place_waits({ n: summary.waiting })}</BannerWait></>}</span> } : {})}
        visual={<div className="plg-room" data-bleed data-testid="plugins.room">{room ?? <Room rows={all} project={project} fallback={<PlainRoom rows={all} project={project} />} onOpen={ports.open} />}</div>} />
      {(bandShows(summary) && summary !== null) || problem != null ? (
        <div className="plg-sec">
          {bandShows(summary) && summary !== null && (
            <Band summary={summary} browse={browse.find((b) => b.source === 'mistral')?.items ?? []} projectName={projectName}
              onConnect={(id) => { ports.send({ cmd: 'plugin_connect', ...at, id }); }}
              onImport={() => { ports.add('import'); }}
              onWorkspace={() => { setTab('installed'); setKind('all'); setQ(''); }}
              onBrowse={() => { setTab('browse'); }} />
          )}
          {problem != null && <Problem problem={problem} onRefresh={() => { ports.send({ cmd: 'plugins_refresh', ...at }); }} />}
        </div>
      ) : null}
      <div className="plg-sec" data-testid="plugins.list">
        <ListBar inset testId="plugins.tabs.row"
          end={
            <>
              <ListSearch ref={searchRef} value={q} onChange={(e) => { setQ(e.currentTarget.value); }} onKeyDown={searchKey} onClear={() => { setQ(''); searchRef.current?.focus(); }} testId="plugins.search" />
              <AddMenu summary={summary} onPick={(item) => { if (item === 'browse') setTab('browse'); else ports.add(item); }} />
            </>
          }>
          <Filters items={[
            { id: 'installed', label: <TabWords words={sentenceCase(settings.plugins_tab_installed)} n={counts.all} /> },
            { id: 'browse', label: <TabWords words={sentenceCase(settings.plugins_tab_browse)} n={toBrowse} /> },
            { id: 'types', label: <TabWords words={settings.plugins_tab_file_types} n={fileTypes.length} /> },
          ]}
            on={tab} onPick={(id) => { setTab(TABS.find((t) => t === id) ?? 'installed'); }} testId="plugins.tabs" />
        </ListBar>
        {tab === 'installed' ? (
          <>
            <div className="plg-kinds" role="group" data-testid="plugins.kinds">
              {KINDS.map((k) => (
                <Chip key={k} onClick={() => { setKind(k); }} testId={`plugins.kind.${k}`} expanded={k === kind}>
                  {sentenceCase(kindWords(k))}
                  {k !== 'all' && <span className="plg-n">{counts[k]}</span>}
                </Chip>
              ))}
            </div>
            {rows === null ? (
              <div className="plg-empty" data-testid="plugins.loading">{settings.plugins_place_loading}</div>
            ) : groups.length === 0 ? (
              <div className="plg-empty" data-testid="plugins.none">{settings.plugins_none_match}</div>
            ) : (
              groups.map((g) => (
                <div key={g.kind} className="plg-group" data-testid={`plugins.group.${g.kind}`}>
                  <GroupLabel count={g.hits.length}>{sentenceCase(groupWords(g.kind))}</GroupLabel>
                  <div className="plg-list">
                    {g.hits.map((h) => (
                      <PlaceRow key={h.p.id === '' ? h.p.name : h.p.id} h={h} project={project}
                        onOpen={() => {
                          // a skill the search found opens in its own tab (page skill-panel, pick A)
                          if (h.part?.kind === 'skill') openTab({ kind: 'skill', plugin: h.p.id === '' ? h.p.name : h.p.id, name: h.part.name });
                          else ports.open(h.p.id === '' ? h.p.name : h.p.id);
                        }}
                        onSet={(on) => { ports.send({ cmd: 'plugin_set', ...at, name: h.p.id === '' ? h.p.name : h.p.id, on }); }}
                        onConnect={(id) => { ports.send({ cmd: 'plugin_connect', ...at, id }); }}
                        onLogin={(name) => { ports.send({ cmd: 'plugin_login', ...at, name }); }}
                        onSetUp={ports.setUpComputer} />
                    ))}
                  </div>
                </div>
              ))
            )}
          </>
        ) : tab === 'types' ? (
          <FileTypes rows={fileTypes} send={ports.send} {...at} />
        ) : (
          <BrowseList browse={browse} q={q} project={project}
            onConnect={(id) => { ports.send({ cmd: 'plugin_connect', ...at, id }); }}
            onOpen={ports.open} />
        )}
      </div>
    </section>
  );
}

function TabWords({ words, n }: { words: string; n: number }) {
  return (
    <>
      {words} <span className="plg-n">{n}</span>
    </>
  );
}

/** The room without WebGL: his on plugins' icons on a floor, the off ones in glass. */
function PlainRoom({ rows, project }: { rows: readonly Plugin[]; project: string | undefined }) {
  const shown = rows.filter((p) => p.state === 'loaded' || p.state === 'disabled').slice(0, 24);
  return (
    <div className="plg-floor" aria-hidden="true">
      {shown.map((p) => (
        <PluginIcon key={p.id === '' ? p.name : p.id} row={p} {...(project === undefined ? {} : { project })} />
      ))}
    </div>
  );
}

/** A click on one of a row's own controls (a button, a switch) is that control's, not the row's. */
function onControl(t: EventTarget): boolean {
  return t instanceof Element && t.closest('button, [role="switch"]') !== null;
}

/** One row of the installed list. */
export function PlaceRow({ h, project, onOpen, onSet, onConnect, onLogin, onSetUp }: {
  h: Hit;
  project: string | undefined;
  onOpen: () => void;
  onSet: (on: boolean) => void;
  onConnect: (id: string) => void;
  onLogin: (name: string) => void;
  onSetUp: () => void;
}) {
  const p = h.p;
  const id = p.id === '' ? p.name : p.id;
  const c = controlOf(p);
  const tid = `plugins.row.${id}`;
  const failed = c.kind === 'failed';
  const what = gives(p);
  return (
    <div className={`plg-row${p.on || c.kind === 'set_up_cu' ? '' : ' off'}`} role="button" tabIndex={0} onClick={(e) => { if (!onControl(e.target)) onOpen(); }}
      onKeyDown={(e) => { if (e.key === 'Enter' && e.target === e.currentTarget) onOpen(); }} data-testid={tid}>
      <PluginIcon row={p} {...(project === undefined ? {} : { project })} {...(c.kind === 'set_up_cu' ? { lit: true } : {})} testId={`${tid}.icon`} />
      <span className="plg-row-words">
        <span className="plg-row-name">
          <span className="plg-title">{titleOf(p)}</span>
          {tagsOf(p).map((t) => (
            <span key={t} className="plg-tag">{t}</span>
          ))}
        </span>
        <span className={`plg-row-what${failed && h.part === undefined ? ' err' : ''}`}>{lineOf(h)}</span>
      </span>
      <span className="plg-row-end">
        {c.kind === 'locked' && <span className="plg-dim">{settings.plugins_always_on}</span>}
        {c.kind === 'set_up_cu' && (
          <>
            <span className="plg-dim">{settings.plugins_not_set_up}</span>
            <Button size="sm" onClick={onSetUp} testId={`${tid}.set_up`}>{sentenceCase(settings.plugins_set_up)}</Button>
          </>
        )}
        {c.kind === 'connect' && (
          <>
            <span className="plg-acc">{settings.plugins_expired}</span>
            <Button size="sm" onClick={() => { onConnect(c.id); }} testId={`${tid}.connect`}>{sentenceCase(settings.plugins_sign_in_again)}</Button>
          </>
        )}
        {c.kind === 'log_in' && (
          <>
            <span className="plg-acc">{c.pending ? settings.plugins_login_waiting_short : settings.plugins_part_needs_login}</span>
            <Button size="sm" state={c.pending ? 'loading' : undefined} onClick={() => { onLogin(c.name); }} testId={`${tid}.login`}>{sentenceCase(settings.plugins_key_login)}</Button>
          </>
        )}
        {c.kind === 'failed' && (
          <Button size="sm" onClick={onOpen} testId={`${tid}.details`}>{sentenceCase(settings.plugins_details)}</Button>
        )}
        {(c.kind === 'toggle' || c.kind === 'failed') && (
          <>
            {what !== '' && <span className="plg-dim">{what}</span>}
            <span className="plg-tg" {...tipAttrs(settings.plugins_toggle_tip({ name: titleOf(p) }))}>
              <Toggle on={p.on} label={titleOf(p)} onChange={onSet} testId={`${tid}.toggle`} />
            </span>
          </>
        )}
      </span>
    </div>
  );
}

/** 'connect your tools' (page plugins v48): what to connect, import or look at, from the summary. */
export function Band({ summary, browse, projectName, onConnect, onImport, onWorkspace, onBrowse }: {
  summary: PluginsSummary;
  /** Mistral's connectors not connected yet (their words) */
  browse: readonly Plugin[];
  projectName: string | undefined;
  onConnect: (id: string) => void;
  onImport: () => void;
  onWorkspace: () => void;
  onBrowse: () => void;
}) {
  const mistral = itemsOf(summary, 'mistral')[0];
  const found = foundWords(itemsOf(summary, 'import')[0]);
  const imp = itemsOf(summary, 'import')[0];
  const ws = itemsOf(summary, 'workspace')[0];
  const market = itemsOf(summary, 'marketplace');
  const byId = new Map(browse.map((p) => [p.id, p]));
  const words = mistral === undefined ? null : bandWords(mistral, byId);
  const ids = mistral?.ids ?? [];
  const cells = bandCells(ids.length, mistral?.n ?? 0);
  return (
    <div className="plg-band" data-testid="plugins.band">
      <div className="plg-band-top">
        <div className="plg-band-head">
          <span className="plg-band-h">{sentenceCase(settings.plugins_band_head)}</span>
          <span className="plg-dim">{settings.plugins_band_through}</span>
          <span className="plg-band-count" data-testid="plugins.band.count">{settings.plugins_band_count({ connected: summary.connected, total: summary.connectors })}</span>
        </div>
        <div className="plg-band-sub" data-testid="plugins.band.sub">{words?.sub ?? settings.plugins_band_sub}</div>
      </div>
      {mistral !== undefined && words !== null && (
        <div className="plg-cards">
          {ids.slice(0, cells.shown).map((id, i) => {
            const row = byId.get(id);
            const title = (mistral.titles ?? [])[i] ?? id;
            const card = words.cards[i] ?? { line: '', tip: '' };
            return (
              <div key={id} className="plg-card" data-testid={`plugins.band.${id}`}>
                {row !== undefined ? <PluginIcon row={row} testId={`plugins.band.${id}.icon`} /> : <MonoTile title={title} testId={`plugins.band.${id}.icon`} />}
                <span className="plg-card-words" {...(card.tip !== '' ? tipAttrs(card.tip) : {})} data-testid={`plugins.band.${id}.why`}>
                  <span className="plg-title" data-testid={`plugins.band.${id}.title`}>{title}</span>
                  {card.line !== '' && <span className="plg-dim">{card.line}</span>}
                </span>
                <Button size="sm" onClick={() => { onConnect(id); }} testId={`plugins.band.${id}.connect`}>{sentenceCase(settings.plugins_connect)}</Button>
              </div>
            );
          })}
          {cells.more > 0 && (
            <div className="plg-card more">
              <Button kind="ghost" onClick={onBrowse} testId="plugins.band.more">{sentenceCase(settings.plugins_band_more({ n: cells.more }))}</Button>
            </div>
          )}
        </div>
      )}
      {found !== null && imp !== undefined && (
        <div className="plg-band-line" data-testid="plugins.band.import">
          <span>{found}</span>
          <Button size="sm" kind="ghost" onClick={onImport} testId="plugins.band.import.go">{sentenceCase(settings.plugins_band_import({ n: imp.n }))}</Button>
        </div>
      )}
      {ws !== undefined && (
        <div className="plg-band-line" data-testid="plugins.band.workspace">
          <span>{settings.plugins_band_workspace({ project: projectName ?? '', n: ws.n })}</span>
          <Button size="sm" kind="ghost" onClick={onWorkspace} testId="plugins.band.workspace.see">{sentenceCase(settings.plugins_band_see)}</Button>
        </div>
      )}
      {market.map((m, i) => (
        <div key={i} className="plg-band-line" data-testid="plugins.band.market">
          <span>{marketWords(market, m)}</span>
          <Button size="sm" kind="ghost" onClick={onBrowse} testId="plugins.band.market.browse">{sentenceCase(settings.plugins_band_browse)}</Button>
        </div>
      ))}
    </div>
  );
}

/** Why Mistral's connectors may be missing or old, said from the kind (never from its words). */
function Problem({ problem, onRefresh }: { problem: ConnectorsProblem; onRefresh: () => void }) {
  const words = problem.kind === 'no_key' ? settings.plugins_conn_no_key : problem.kind === 'locked' ? settings.plugins_conn_locked : settings.plugins_conn_fetch;
  return (
    <div className="plg-problem" data-testid="plugins.problem">
      <span>{words}</span>
      {problem.kind === 'fetch' && <Button size="sm" kind="ghost" onClick={onRefresh} testId="plugins.refresh">{sentenceCase(settings.plugins_refresh)}</Button>}
    </div>
  );
}

/** Browse: Mistral's connectors first, then each marketplace (page plugins §4). */
function BrowseList({ browse, q, project, onConnect, onOpen }: {
  browse: PlaceProps['browse'];
  q: string;
  project: string | undefined;
  onConnect: (id: string) => void;
  onOpen: (id: string) => void;
}) {
  const s = q.trim().toLowerCase();
  const shown = browse
    .map((b) => ({ ...b, items: b.items.filter((p) => s === '' || titleOf(p).toLowerCase().includes(s) || (p.what ?? '').toLowerCase().includes(s)) }))
    .filter((b) => b.items.length > 0);
  if (shown.length === 0) return <div className="plg-empty" data-testid="plugins.browse.empty">{s === '' ? settings.plugins_browse_empty : settings.plugins_none_match}</div>;
  return (
    <>
      {shown.map((b) => (
        <div key={b.source} className="plg-group" data-testid={`plugins.browse.${b.source}`}>
          <GroupLabel count={b.items.length}>{b.source === 'mistral' ? settings.plugins_browse_mistral : b.source}</GroupLabel>
          <div className="plg-list">
            {b.items.map((p) => {
              const id = p.id === '' ? p.name : p.id;
              return (
                <div key={id} className="plg-row" role="button" tabIndex={0} onClick={(e) => { if (!onControl(e.target)) onOpen(id); }} data-testid={`plugins.browse.row.${id}`}>
                  <PluginIcon row={p} {...(project === undefined ? {} : { project })} />
                  <span className="plg-row-words">
                    <span className="plg-row-name">
                      <span className="plg-title">{titleOf(p)}</span>
                      {tagsOf(p).map((t) => (
                        <span key={t} className="plg-tag">{t}</span>
                      ))}
                    </span>
                    <span className="plg-row-what">{p.what ?? ''}</span>
                  </span>
                  <span className="plg-row-end">
                    {p.action === 'connect' || p.action === 'set_up' ? (
                      <Button size="sm" state={p.login === 'pending' ? 'loading' : undefined} onClick={() => { onConnect(id); }} testId={`plugins.browse.row.${id}.connect`}>
                        {sentenceCase(p.action === 'set_up' ? settings.plugins_set_up : settings.plugins_connect)}
                      </Button>
                    ) : (
                      <Button size="sm" kind="ghost" onClick={() => { onOpen(id); }} testId={`plugins.browse.row.${id}.open`}>{sentenceCase(settings.plugins_browse_add_one)}</Button>
                    )}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </>
  );
}
