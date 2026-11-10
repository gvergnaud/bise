// src/app/panel · New tab, the launcher (page native-mac mockup 3, §6 'lanceur', round 2 r2-launcher):
// padding 16, a search 40 r 8 with its 1 px line (artifacts by name, or an address), 'Open'
// (Artifacts ⌘P, Terminal ⌃`, Changes ⌥⌘D only when the agent in view has some, Browser ⌘L,
// Office: the bims head) in one group box (r 10, a hairline, rows 40 with hairlines between, 12
// inside, the kbd at the right), 'Recent' in rows
// 48 with a 32 tile (what he opened in a tab last, newest first, the project in front's only: his
// m_46444). Artifacts turns this tab into the picker of the 10 newest artifacts (kind icon, name,
// kind · who, its age at the end, '22 d'; the field filters). The field keeps the keys: ↑/↓ walk
// Open then Recent from none (↑ on the first gives the field back, no wrap), or as he types his
// address then the matches from the first; ⏎ opens the highlighted row (launch.ts, designer
// m_46449); a pointer that moves takes the highlight back. Esc clears the field, then
// leaves the picker, then closes the tab. A pick opens in this tab's place, as a browser's new tab
// becomes the page. The rows act as their keys do (keys.ts doPanelKey). Owner: nm-panel.
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { AppIcon, Input, KindIcon, Kbd, Press, iconKindOf, type AppIconName } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { useAllArtifacts } from '@/artifacts';
import { ago } from '@/agents/model';
import type { Item } from '@/artifacts/model';
import { useAgents, useProjects, useStore } from '@/store';
import { useRoute } from '@/app/router';
import { projectOf } from '@/app/route';
import { ThingPic } from '@/app/side3d/Bubbles';
import { bubbleThing } from '@/app/side3d/objects';
import { entityKey, type Entity } from '@/shared/entity';
import { agentInView, changesInView, doPanelKey, PANEL_KEYS, type PanelCtx } from './keys';
import { closeTabId, launcherDraft, launcherHome, openWhat, panelAt, setLauncherDraft, takeOpened, useOpenedN, usePanelState } from './state';
import { scopeOf } from './scope';
import { useTabLooks } from './look';
import { tabOf } from './model';
import { enterRow, firstAt, isListing, latestArtifacts, launchMatches, launchRows, NO_ROW, openRows, recentIn, walkAt, type LaunchRow, type OpenRow } from './launch';

/** Each Open row: its label, its icon, its key when it has one. */
const ROW: Readonly<Record<OpenRow, { readonly label: string; readonly icon: AppIconName | 'head'; readonly keys?: string }>> = {
  artifacts: { label: panel.launch_artifacts, icon: 'artifacts', keys: PANEL_KEYS.files },
  terminal: { label: panel.launch_terminal, icon: 'terminal', keys: PANEL_KEYS.terminal },
  changes: { label: panel.launch_changes, icon: 'changes', keys: PANEL_KEYS.changes },
  browser: { label: panel.launch_browser, icon: 'browser', keys: PANEL_KEYS.browser },
  // the bims head, the side's agents bubble's object (designer m_44218: 16, no key)
  office: { label: panel.launch_office, icon: 'head' },
};

/** A New tab: search, Open, Recent; or its Artifacts picker. `tabId`: its own tab, which a pick replaces. */
export function Launcher({ tabId, shown, artifacts = false }: { tabId: string; shown: boolean; artifacts?: boolean }) {
  const route = useRoute();
  const store = useStore();
  const projects = useProjects();
  const home = projects.find((p) => p.home)?.project;
  const allRecent = usePanelState().recent;
  const look = useTabLooks();
  const arts = useAllArtifacts();
  // the agent in view's rows: its Changes row shows and hides with its diff
  useAgents(agentInView(route)?.project ?? '');
  // his words come back with the tab when it was behind another (state.ts drafts)
  const [q, setWords] = useState(() => launcherDraft(tabId));
  const setQ = (v: string) => {
    setWords(v);
    setLauncherDraft(tabId, v);
  };
  const [at, setAt] = useState(() => firstAt(q, artifacts));
  // ↑/↓ moved the highlight: the row under a resting pointer drops its hover tint until it moves (macOS lists)
  const [keyed, setKeyed] = useState(false);
  const [now] = useState(() => Date.now());
  const field = useRef<HTMLInputElement>(null);
  const bise = (window as Partial<Window>).bise;
  const ctx: PanelCtx = { route, home, agents: (p) => store.agents(p), places: (p) => store.places(p) };
  const opens = openRows({ changes: changesInView(ctx) !== undefined, office: (projectOf(route) ?? home) !== undefined });
  // Recent: the project in front's own (his m_46444), the panel's scope (scope.ts scopeOf)
  const recent = recentIn(allRecent, scopeOf(route, home, panelAt()));
  // a New tab he just opened, or the picker it turned into: the field takes the keys; back in
  // front by a project switch, his focus stays where it was (his m_46543, state.ts takeOpened);
  // ⌘T on it again (openedN): the keys back in its field, his words selected (designer m_46965)
  const openedN = useOpenedN();
  useEffect(() => {
    if (!shown || !takeOpened(tabId)) return;
    field.current?.focus();
    field.current?.select();
  }, [shown, artifacts, tabId, openedN]);
  // the tab turned into the picker or back: its highlight starts over, in the same render
  const [wasPicker, setWasPicker] = useState(artifacts);
  if (wasPicker !== artifacts) {
    setWasPicker(artifacts);
    setAt(firstAt('', artifacts));
  }
  const found = useMemo(() => (artifacts ? latestArtifacts(arts, q) : launchMatches(arts, q)), [arts, q, artifacts]);
  const listing = isListing(q, artifacts);
  const rows = launchRows({ q, picker: artifacts, found, open: opens, recent });
  const pick = (e: Entity) => {
    openWhat({ kind: 'thing', e }, tabId);
  };
  const open = (x: Item) => {
    pick({ kind: 'artifact', project: x.project, id: x.a.id });
  };
  const go = (r: LaunchRow) => {
    if (r.kind === 'open') doPanelKey(r.row, ctx, tabId);
    else if (r.kind === 'recent') pick(r.e);
    else if (r.kind === 'found') open(r.item);
    else void bise?.openExternal(r.url);
  };
  const enter = () => {
    const r = enterRow(rows, at, listing);
    if (r !== undefined) go(r);
  };
  const esc = () => {
    if (q !== '') {
      setQ('');
      setAt(firstAt('', artifacts));
    } else if (artifacts) launcherHome(tabId);
    else closeTabId(tabId);
  };
  // one highlight at a time: a pointer that moves after a key takes it back (designer m_46449 a)
  const pointed = () => {
    if (!keyed) return;
    setKeyed(false);
    if (!listing) setAt(NO_ROW);
  };
  const optId = (i: number) => `${tabId}.opt.${String(i)}`;
  const label = artifacts ? panel.launch_pick_search : panel.launch_search;
  return (
    <div className="pn-launch" data-testid="app.panel.launcher" data-picker={artifacts || undefined}>
      <label className="pn-search">
        <AppIcon name="search" size={16} />
        <Input
          ref={field}
          look="pn-search-in"
          value={q}
          placeholder={label}
          aria-label={label}
          role="combobox"
          aria-expanded={listing}
          aria-controls={`${tabId}.list`}
          aria-activedescendant={rows[at] !== undefined ? optId(at) : undefined}
          onChange={(e) => {
            setQ(e.target.value);
            setAt(firstAt(e.target.value, artifacts));
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              enter();
            } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
              e.preventDefault();
              setKeyed(true);
              setAt((i) => walkAt(i, rows.length, e.key === 'ArrowDown' ? 1 : -1));
            } else if (e.key === 'Escape') {
              e.preventDefault();
              e.stopPropagation();
              esc();
            }
          }}
          data-testid="app.panel.launcher.search"
        />
      </label>
      {/* every row is an option of one list the field walks (aria-activedescendant); display: contents */}
      <div role="listbox" id={`${tabId}.list`} aria-label={label} className="pn-rows" data-keyed={keyed || undefined} onPointerMove={pointed}>
        {listing ? (
          <>
            {artifacts && <h3 className="pn-lh">{panel.launch_latest}</h3>}
            {rows.length === 0 ? (
              <p className="pn-none" data-testid="app.panel.launcher.nomatch">
                {artifacts && q.trim() === '' ? panel.launch_no_artifact : panel.launch_no_match}
              </p>
            ) : (
              <div className="pn-found">
                {rows.map((r, i) =>
                  r.kind === 'url' ? (
                    <PickRow
                      key="url"
                      id={optId(i)}
                      on={i === at}
                      icon={<AppIcon name="browser" size={16} />}
                      title={r.url}
                      sub={panel.addr_out}
                      onPick={() => {
                        go(r);
                      }}
                      testId="app.panel.launcher.url"
                    />
                  ) : r.kind === 'found' ? (
                    <PickRow
                      key={`${r.item.project}/${r.item.a.id}`}
                      id={optId(i)}
                      on={i === at}
                      icon={<KindIcon kind={iconKindOf(r.item.a)} size={16} />}
                      title={r.item.a.title}
                      sub={panel.launch_kind_by({ kind: r.item.a.kind, who: r.item.a.agent })}
                      {...(artifacts ? { when: ago(r.item.a.at_ms, now) } : {})}
                      onPick={() => {
                        go(r);
                      }}
                      testId={`app.panel.launcher.found.${r.item.project}/${r.item.a.id}`}
                    />
                  ) : null,
                )}
              </div>
            )}
          </>
        ) : (
          <>
            <h3 className="pn-lh">{panel.launch_open}</h3>
            {/* one group box, as settings' (designer m_44486 #1): rows 40 with hairlines between */}
            <div className="pn-open" role="group" aria-label={panel.launch_open} data-testid="app.panel.launcher.open">
              {opens.map((k, i) => {
                const r = ROW[k];
                return (
                  <Press
                    key={k}
                    look="pn-li"
                    id={optId(i)}
                    role="option"
                    aria-selected={i === at}
                    tabIndex={-1}
                    data-on={i === at || undefined}
                    onClick={() => {
                      go({ kind: 'open', row: k });
                    }}
                    data-testid={`app.panel.launcher.${k}`}
                  >
                    {r.icon === 'head' ? <ThingPic thing={bubbleThing('agents')} px={16} tile="thread" className="pn-li-pic" /> : <AppIcon name={r.icon} size={16} />}
                    <span className="pn-li-t">{r.label}</span>
                    {r.keys !== undefined && <Kbd>{r.keys}</Kbd>}
                  </Press>
                );
              })}
            </div>
            <h3 className="pn-lh">{panel.launch_recent}</h3>
            {recent.length === 0 ? (
              <p className="pn-none" data-testid="app.panel.launcher.none">
                {panel.launch_none}
              </p>
            ) : (
              recent.map((e, j) => {
                const i = opens.length + j;
                const l = look(tabOf({ kind: 'thing', e }));
                const sub = e.kind === 'artifact' ? arts.find((x) => x.project === e.project && x.a.id === e.id) : undefined;
                return (
                  <PickRow
                    key={entityKey(e)}
                    id={optId(i)}
                    on={i === at}
                    icon={l.icon}
                    title={l.title}
                    sub={sub === undefined ? '' : panel.launch_kind_by({ kind: sub.a.kind, who: sub.a.agent })}
                    onPick={() => {
                      pick(e);
                    }}
                    testId={`app.panel.launcher.recent.${entityKey(e)}`}
                  />
                );
              })
            )}
          </>
        )}
      </div>
    </div>
  );
}

/** A row with its 32 tile: Recent's, a found artifact's (an option: `on` the highlighted one). */
export function PickRow({ icon, title, sub, when, onPick, onHover, testId, id, on }: {
  icon: ReactNode;
  title: string;
  sub: string;
  /** its age at the row's end (the Artifacts picker's) */
  when?: string;
  onPick: () => void;
  onHover?: () => void;
  testId: string;
  /** a list's option: its id (the field's aria-activedescendant) */
  id?: string;
  on?: boolean;
}) {
  return (
    <Press
      look="pn-rec"
      id={id}
      role={id === undefined ? undefined : 'option'}
      aria-selected={id === undefined ? undefined : on === true}
      tabIndex={id === undefined ? undefined : -1}
      data-on={on === true || undefined}
      onClick={onPick}
      onPointerMove={onHover}
      data-testid={testId}
    >
      <span className="pn-tile">{icon}</span>
      <span className="pn-rec-w">
        <span className="pn-rec-t">{title}</span>
        {sub !== '' && <span className="pn-rec-s">{sub}</span>}
      </span>
      {when !== undefined && <span className="pn-rec-when">{when}</span>}
    </Press>
  );
}
