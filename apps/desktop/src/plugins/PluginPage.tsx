// A plugin's page (page plugins §2-§3): its head (its object, its name, what it does, what it gives,
// its switch: 'on for every project'), up to 3 lines to try (a click puts one in main's composer),
// then its parts, each with its switch: skills, MCP servers (their status words, a login or a
// failure's line, their tools folded under '▸ n tools', each tool a switch), a connector's tools;
// then its settings (an API key: SettingsGroup), then 'about': where it comes from, for which projects, when his agents see a change. A plugin of
// his own folder has remove (one confirm, plug-tidy's `plugin_remove`). A row to add (browse) has
// connect or + add in place of the switch. Every change is the core's. Owner: plug-desktop.
import { useMemo, useState, type KeyboardEvent, type MouseEvent } from 'react';
import { AppIcon, BannerWait, Button, GroupLabel, PageBanner, Press, sentenceCase, Toggle, tipAttrs } from '@/ds';
import { InBar } from '@/app/barslot';
import { BarButton } from '@/app/barbutton';
import type { BiseApi, PopupItem } from '@/shared/api';
import { useBannerGutter } from '@/artifacts/hero/vignettes';
import { openTab } from '@/app/panel/tabs';
import { usePanelState } from '@/app/panel/state';
import { entityKey } from '@/shared/entity';
import type { AppCmd, FileType, Plugin, PluginPart } from '@/proto';
import { settings } from '@/proto/words.gen';
import { badgeOf, controlOf, gives, installed, isComputer, leadOf, needsLogin, needsSetting, removable, tagsOf, titleOf, toolsOf } from './model';
import { ds } from '@/proto/words_ds.gen';
import { PluginIcon } from './PluginIcon';
import { ViewsGroup } from './ViewsGroup';
import { SettingsGroup } from './SettingsGroup';
import { opensWords } from './typerows';
import { AccountsGroup } from './AccountsGroup';

export interface PagePorts {
  send: (cmd: AppCmd) => void;
  back: () => void;
  /** a line to try: into main's composer, never sent */
  tryIt: (words: string) => void;
  setUpComputer: () => void;
  /** remove a plugin of his own folder (none: the core has no `plugin_remove` yet, no button) */
  remove?: (name: string) => void;
}

/** When his agents see a change to this kind (a skill: their next turn; a tool, a connector: their next call). */
function whenWords(p: Plugin): string {
  return p.kind === 'skills' || (p.parts ?? []).some((x) => x.kind === 'skill') ? settings.plugins_next_turn : settings.plugins_next_call;
}

const bise = (): Partial<BiseApi> | undefined => (window as { bise?: Partial<BiseApi> }).bise;

export function PluginPage({ p, project, ports, types = [] }: { p: Plugin; project?: string; ports: PagePorts; types?: readonly FileType[] }) {
  const id = p.id === '' ? p.name : p.id;
  const at = project === undefined ? {} : { project };
  const set = (name: string, on: boolean) => {
    ports.send({ cmd: 'plugin_set', ...at, name, on });
  };
  const [asking, setAsking] = useState(false);
  const c = controlOf(p);
  const skills = (p.parts ?? []).filter((x) => x.kind === 'skill');
  const servers = (p.parts ?? []).filter((x) => x.kind === 'mcp');
  const tools = (p.parts ?? []).filter((x) => x.kind === 'tool');
  const off = (p.parts ?? []).filter((x) => !x.on).length;
  const tid = `plugins.page.${id}`;
  const gutter = useBannerGutter();
  const badge = badgeOf(p);
  const more = () => {
    const items: PopupItem[] = [{ id: 'remove', label: settings.plugins_remove_menu }];
    void bise()?.popupMenu?.(items).then((x) => { if (x === 'remove') setAsking(true); });
  };
  const kept = usePanelState();
  const openTabs = useMemo(() => new Set(kept.tabs.list.map((t) => t.id)), [kept]);
  return (
    <section className="plg-place plg-page" data-testid={tid}>
      {/* the page's banner (ds PageBanner): its name and tags, what it does, what it gives, its own
          object in the box; its actions in the row under it (no button in a banner) */}
      <PageBanner edge="column" gutter={gutter} icon="plugins" testId={`${tid}.lead`}
        title={<>{titleOf(p)}{tagsOf(p).map((t) => <span key={t} className="plg-tag">{t}</span>)}</>}
        sentence={leadOf(p.what ?? '')}
        // a built-in plugin says 'always on' in its facts, never alone in the actions' row (designer m_43933 #11)
        // what waits on him joins them, its sign alone in the accent ('1 server · ? needs a login'),
        // never a lone badge in the box's corner (designer m_44023 #4)
        facts={<span data-testid={`${tid}.facts`}>
          {[gives(p), c.kind === 'locked' ? settings.plugins_always_on : '', off > 0 ? settings.plugins_n_off({ n: off }) : ''].filter((x) => x !== '').join(' · ')}
          {/* a login it needs, else a setting it needs (ui-review m_51732: never 'needs a login' for a setting) */}
          {badge === '?' && <>{' · '}<BannerWait><AppIcon name="ask" size={14} /></BannerWait>{` ${needsSetting(p) && !needsLogin(p) ? settings.plugins_setting_waits : settings.plugins_part_needs_login}`}</>}
          {badge === '✗' && <>{' · '}<span className="plg-err">{`✗ ${settings.plugins_badge_failed}`}</span></>}
        </span>}
        visual={<PluginIcon row={p} size="head" {...(project === undefined ? {} : { project })} {...(isComputer(p) ? { lit: true } : {})} />} />
      {/* its other actions in the window's bar: the page's '⋯' (Remove…, asked once below) */}
      {ports.remove !== undefined && removable(p) && (
        <InBar>
          <BarButton icon="more" label={ds.more_actions} onClick={more} testId={`${tid}.more`} />
        </InBar>
      )}
      <div className="plg-page-col">
        <div className="plg-page-acts" data-testid={`${tid}.acts`}>
          {!installed(p) ? (
            p.action === 'connect' || p.action === 'set_up' ? (
              <Button kind="pri" onClick={() => { ports.send({ cmd: 'plugin_connect', ...at, id }); }} testId={`${tid}.connect`}>
                {sentenceCase(p.action === 'set_up' ? settings.plugins_set_up : settings.plugins_connect)}
              </Button>
            ) : null
          ) : c.kind === 'locked' ? null : c.kind === 'set_up_cu' || (isComputer(p) && !p.on) ? (
            <Button kind="pri" onClick={ports.setUpComputer} testId={`${tid}.set_up`}>{sentenceCase(settings.plugins_set_up)}</Button>
          ) : (
            // its switch, a row in a box like its parts' (designer m_44023 #4)
            <div className="plg-list plg-page-switch">
              <div className="plg-row">
                <span className="plg-row-words">
                  <span className="plg-title">{sentenceCase(p.on ? settings.plugins_on_everywhere : settings.plugins_off_everywhere)}</span>
                  {/* its server runs on Node and there is none: said beside the switch, which still turns it on */}
                  {p.needs_node === true && <span className="plg-row-what err" data-testid={`${tid}.needs_node`}>{settings.plugins_needs_node}</span>}
                </span>
                <span className="plg-row-end"><Toggle on={p.on} label={titleOf(p)} onChange={(on) => { set(id, on); }} testId={`${tid}.toggle`} /></span>
              </div>
            </div>
          )}
        </div>
      {asking && ports.remove !== undefined && (
        <div className="plg-ask" role="group" data-testid={`${tid}.ask`}>
          <span>{settings.plugins_remove_confirm({ name: titleOf(p) })}</span>
          <span className="plg-acts">
            <Button kind="danger" size="sm" onClick={() => { setAsking(false); ports.remove?.(id); }} testId={`${tid}.ask.yes`}>{settings.plugins_remove_yes}</Button>
            <Button kind="ghost" size="sm" onClick={() => { setAsking(false); }} testId={`${tid}.ask.no`}>{settings.plugins_remove_no}</Button>
          </span>
        </div>
      )}
      {/* a ready connector's accounts, the first group (designer m_57870) */}
      <AccountsGroup p={p} {...at} send={ports.send} />
      {(p.prompts ?? []).length > 0 && (
        <div className="plg-try" data-testid={`${tid}.try`}>
          {(p.prompts ?? []).map((t, i) => (
            <Press key={i} look="plg-try-line" onClick={() => { ports.tryIt(t); }} {...tipAttrs(settings.plugins_try_tip)} data-testid={`${tid}.try.${String(i)}`}>
              <span>{t}</span>
              <span className="plg-dim">→</span>
            </Press>
          ))}
        </div>
      )}
      {/* its settings (dev.bise.settings: an API key), after the try lines, before its parts (designer m_51514) */}
      <SettingsGroup plugin={p.name} list={p.settings ?? []} {...at} send={ports.send} />
      {skills.length > 0 && (
        <Parts head={settings.plugins_kind_skills} n={skills.length}>
          {skills.map((x) => (
            <PartRow key={x.id} x={x} lead="$" locked={p.locked === true} owner={p.on} onSet={set}
              // a skill opens in its own tab (page skill-panel, pick A); its row stays tinted while it is open
              open={{ on: openTabs.has(entityKey({ kind: 'skill', plugin: id, name: x.name })), go: () => { openTab({ kind: 'skill', plugin: id, name: x.name }); } }} />
          ))}
        </Parts>
      )}
      {servers.length > 0 && (
        <Parts head={settings.plugins_group_mcp} n={servers.length}>
          {servers.map((x) => (
            <ServerRow key={x.id} x={x} owner={p.on} onSet={set} onLogin={(name) => { ports.send({ cmd: 'plugin_login', ...at, name }); }} />
          ))}
        </Parts>
      )}
      {/* MCP Apps: its views and what each may use (C8, mcp-apps-host) */}
      <ViewsGroup plugin={p.name} parts={p.parts ?? []} {...at} send={ports.send} />
      {tools.length > 0 && (
        <Parts head={settings.plugins_tools_head} n={tools.length}>
          {tools.map((x) => (
            <PartRow key={x.id} x={x} lead="·" locked={false} owner={p.on} onSet={set} />
          ))}
        </Parts>
      )}
      <div className="plg-about" data-testid={`${tid}.about`}>
        <GroupLabel>{sentenceCase(settings.plugins_about)}</GroupLabel>
        <dl className="plg-kv">
          <AppLines p={p} types={types} />
          {p.path != null && (
            <>
              <dt>{settings.plugins_comes_from}</dt>
              <dd className="plg-mono">{p.path}</dd>
            </>
          )}
          {p.version != null && (
            <>
              <dt>{settings.plugins_version}</dt>
              <dd>{p.version}</dd>
            </>
          )}
          <dt>{settings.plugins_for}</dt>
          <dd>{p.scope === 'workspace' ? settings.plugins_for_project : settings.plugins_for_every}</dd>
          <dt>{settings.plugins_when}</dt>
          <dd>{settings.plugins_change_when({ when: whenWords(p) })}</dd>
        </dl>
      </div>
      </div>
    </section>
  );
}

/** A part group: its sentence-case label and count ('Skills 2', never 'SKILLS · 2': x1b), its rows. */
function Parts({ head, n, children }: { head: string; n: number; children: React.ReactNode }) {
  return (
    <div className="plg-group">
      <GroupLabel count={n}>{sentenceCase(head)}</GroupLabel>
      <div className="plg-list">{children}</div>
    </div>
  );
}

/** A skill or a tool: its name, what it does, its switch (off with its plugin: no switch). A skill's
 *  row opens its tab (a click, ⏎), tinted while that tab is open. */
function PartRow({ x, lead, locked, owner, onSet, open }: { x: PluginPart; lead: string; locked: boolean; owner: boolean; onSet: (id: string, on: boolean) => void; open?: { on: boolean; go: () => void } }) {
  const go = open?.go;
  return (
    <div className={`plg-row part${x.on ? '' : ' off'}${open?.on === true ? ' on' : ''}`} data-testid={`plugins.part.${x.id}`}
      {...(go === undefined ? {} : {
        role: 'button',
        tabIndex: 0,
        'aria-current': open?.on === true || undefined,
        onClick: (e: MouseEvent) => { if (!(e.target instanceof Element && e.target.closest('button, [role="switch"]') !== null)) go(); },
        onKeyDown: (e: KeyboardEvent) => { if (e.key === 'Enter' && e.target === e.currentTarget) go(); },
      })}>
      <span className="plg-lead">{lead}</span>
      <span className="plg-row-words">
        <span className="plg-row-name"><span className={lead === '$' ? 'plg-title plg-mono' : 'plg-title'}>{x.name}</span></span>
        {x.what != null && <span className="plg-row-what">{x.what}</span>}
        {/* a skill linked into Documents, not read while his switch is off (skills-scan-tcc, designer m_64245) */}
        {x.guarded != null && !x.guarded.read && (
          <span className="plg-row-what plg-dim" data-testid={`plugins.part.${x.id}.guarded`}>{settings.plugins_skill_guarded({ folder: x.guarded.folder })}</span>
        )}
      </span>
      <span className="plg-row-end">
        {!locked && owner && <Toggle on={x.on} label={x.name} onChange={(on) => { onSet(x.id, on); }} testId={`plugins.part.${x.id}.toggle`} />}
      </span>
    </div>
  );
}

/** An MCP server: its status words, a login or its failure, its tools folded. */
function ServerRow({ x, owner, onSet, onLogin }: { x: PluginPart; owner: boolean; onSet: (id: string, on: boolean) => void; onLogin: (name: string) => void }) {
  const [open, setOpen] = useState(false);
  const tools = x.tools ?? [];
  const n = toolsOf(x);
  const off = tools.filter((t) => !t.on).length;
  return (
    <>
      <div className={`plg-row part${x.on ? '' : ' off'}`} data-testid={`plugins.part.${x.id}`}>
        {/* no text glyph as its mark (ui-review m_44085): its lead column stays, empty */}
        <span className="plg-lead" aria-hidden="true" />
        <span className="plg-row-words">
          <span className="plg-row-name"><span className="plg-title">{x.name}</span>{x.what != null && <span className="plg-dim">{x.what}</span>}</span>
          {x.error != null ? (
            <span className="plg-row-what err">{settings.plugins_failed({ why: x.error })}</span>
          ) : (
            x.status != null && <span className="plg-row-what">{x.status}</span>
          )}
        </span>
        <span className="plg-row-end">
          {x.on && (x.login === 'needs' || x.login === 'pending') && (
            <Button size="sm" state={x.login === 'pending' ? 'loading' : undefined} onClick={() => { onLogin(x.name); }} testId={`plugins.part.${x.id}.login`}>{sentenceCase(settings.plugins_key_login)}</Button>
          )}
          {x.on && x.login === 'in' && (
            <Button size="sm" kind="ghost" onClick={() => { /* log out: the TUI's /plugins logout */ }} testId={`plugins.part.${x.id}.logout`}>{settings.log_out}</Button>
          )}
          {owner && <Toggle on={x.on} label={x.name} onChange={(on) => { onSet(x.id, on); }} testId={`plugins.part.${x.id}.toggle`} />}
        </span>
      </div>
      {tools.length > 0 && (
        <Press look="plg-fold" onClick={() => { setOpen(!open); }} aria-expanded={open} data-testid={`plugins.part.${x.id}.fold`}>
          {open ? settings.plugins_tools_unfold({ n: n ?? tools.length }) : settings.plugins_tools_fold({ n: n ?? tools.length })}
          {off > 0 && <span className="plg-dim">{` · ${settings.plugins_n_off({ n: off })}`}</span>}
        </Press>
      )}
      {open && (
        <div className="plg-tools" data-testid={`plugins.part.${x.id}.tools`}>
          {tools.map((t) => (
            <span key={t.id} className={`plg-tool${t.on ? '' : ' off'}`} data-testid={`plugins.part.${t.id}`}>
              {/* a long name gives way (an ellipsis, the whole name in its tip, mono), never its switch */}
              <span className="plg-tool-name plg-mono" {...tipAttrs(`\`${t.name}\``)}>{t.name}</span>
              {x.on && owner && <Toggle on={t.on} label={t.name} onChange={(on) => { onSet(t.id, on); }} testId={`plugins.part.${t.id}.toggle`} />}
            </span>
          ))}
        </div>
      )}
    </>
  );
}

/** Its app's files in its about (docs/plugin-apps.md A3; designer's mocks block 5): what it opens,
 *  the types another app opens now said after; what it makes. Nothing when it opens and makes none. */
function AppLines({ p, types }: { p: Plugin; types: readonly FileType[] }) {
  const opens = p.opens ?? [];
  const makes = p.makes ?? [];
  const w = opensWords(p.name, opens, types.map((t) => ({ ...t, others: t.others ?? [] })));
  return (
    <>
      {opens.length > 0 && (
        <>
          <dt>{settings.plugins_opens}</dt>
          <dd data-testid={`plugins.page.${p.name}.opens`}>
            {w.list}
            {w.elsewhere.map((x) => <span key={x} className="plg-dim plg-kv-sub">{x}</span>)}
          </dd>
        </>
      )}
      {makes.length > 0 && (
        <>
          <dt>{settings.plugins_makes}</dt>
          <dd data-testid={`plugins.page.${p.name}.makes`}>{makes.map((m) => m.title).join(' · ')}</dd>
        </>
      )}
    </>
  );
}
