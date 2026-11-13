// The settings pages (desktop-app v9 "les réglages", the old window's
// winsetup.js settings, ported): general, voice and fn, context and
// privacy, quiet moments, models, projects, keys. Each one draws its part
// of SettingsData with the DS's Group rows and says what he changes.
// Owner: amb-home.
import { useState, type ReactNode } from 'react';
import type { JsonValue } from '@/proto';
import { Act, Button, Chip, Field, Group, Kbd, Seg, Toggle, type GroupItem } from '@/ds';
import { MeGroup } from './me';
import { TODAY, lookOf, prefOf } from '@/bims/render/look';
import { Approvals } from './approvals';
import { Projects } from './projects';
import { UpdateGroup, updateRow } from './update';
import { DistUpdateGroup, distRow } from './appupdate';
import { Computer } from './computer';
import { capsOf, chromeCase, homeOf } from './chrome';
import { SetupPage } from './setupPage';
import { TelemetryGroup } from './telemetry';
import { KEY_SECTIONS } from './keys';
import { appName, excludedApps, flag, getPref, knownApps, num, str, type Prefs } from './prefs';
import type { SettingsData, SettingsPorts } from './Settings';
import type { NotifyAuth } from '@/shared/api';
import { settings } from '@/proto/words.gen';
import { clearWebHistory, keepWebHistory, useWebHistory } from '@/app/panel/webpages';

export type Page = 'setup' | 'general' | 'voice' | 'privacy' | 'quiet' | 'approvals' | 'projects' | 'computer' | 'keys';

export const PAGES: readonly { id: Page; title: string; sub: string }[] = [
  { id: 'general', title: settings.page_general, sub: settings.page_general_sub },
  { id: 'setup', title: settings.page_setup, sub: settings.page_setup_sub },
  { id: 'voice', title: settings.page_voice, sub: settings.page_voice_sub },
  { id: 'privacy', title: settings.page_privacy, sub: settings.page_privacy_sub },
  { id: 'quiet', title: settings.page_quiet, sub: settings.page_quiet_sub },
  { id: 'approvals', title: settings.page_approvals, sub: settings.page_approvals_sub },
  { id: 'projects', title: settings.page_projects, sub: settings.page_projects_sub },
  { id: 'computer', title: settings.page_computer, sub: settings.page_computer_sub },
  { id: 'keys', title: settings.page_keys, sub: settings.page_keys_sub },
];

/**
 * The general page's macOS notifications row (amb-mac m_14410, designer m_14211): what it says and the
 * one thing it does for each state. 'never' asks; 'asked' and 'off' only System Settings can change
 * (macOS asks once), so they open it; 'on' does nothing. null (no notify api, not known yet): no row. Pure.
 */
/** Whether the general page shows an update group (UpdateGroup or DistUpdateGroup, each titled
 * 'Updates'): when it doesn't, the auto-update card takes that head. Pure. */
export function updatesShown(data: Pick<SettingsData, 'appUpdate' | 'dist'>): boolean {
  if (updateRow(data.appUpdate ?? null, null) !== null) return true;
  if (!data.dist) return false;
  return distRow(data.dist.state, data.dist.info.version) !== null;
}

export function notifyRow(auth: NotifyAuth | null): { sub: string; act: 'ask' | 'open' | null; button: string } | null {
  switch (auth) {
    case null:
      return null;
    case 'never':
      return { sub: settings.notify_never, act: 'ask', button: settings.notify_turn_on };
    case 'asked':
      return { sub: settings.notify_asked, act: 'open', button: settings.open_system_settings };
    case 'on':
      return { sub: settings.notify_on, act: null, button: '' };
    case 'off':
      return { sub: settings.notify_off, act: 'open', button: settings.open_system_settings };
  }
}

/** The pages a route names that open another (page models-setup): /models opens Setup with the
 * roles unfolded, /voice Setup at Dictation and fn (setupPage.tsx's focusOf). */
const ALIAS: Readonly<Record<string, Page>> = { models: 'setup', dictation: 'setup' };

/** A route's page (none, or one this version doesn't know: general). */
export function pageOf(id: string | undefined): (typeof PAGES)[number] {
  const to = id !== undefined ? (ALIAS[id] ?? id) : id;
  return PAGES.find((p) => p.id === to) ?? (PAGES[0] as (typeof PAGES)[number]);
}

interface PageProps {
  page: Page;
  /** the route's own page word ('models', 'dictation' open Setup at their part) */
  route?: string | undefined;
  prefs: Prefs | null;
  setPref: (key: string, value: JsonValue) => void;
  data: SettingsData;
  ports: SettingsPorts;
}

export function PageBody({ page, route, prefs, setPref, data, ports }: PageProps) {
  const tg = (key: string, label: string, sub: string, dflt = true): GroupItem => {
    const on = flag(prefs, key, dflt);
    return {
      id: key,
      label,
      sub,
      control: (
        <Toggle
          on={on}
          label={label}
          onChange={(v) => {
            setPref(key, v);
          }}
          testId={`settings.pref.${key}`}
        />
      ),
    };
  };
  switch (page) {
    case 'general': {
      const themes = ['system', 'light', 'dark'] as const;
      const themeWords = [settings.theme_system, settings.theme_light, settings.theme_dark];
      const theme = str(prefs, 'theme', 'system');
      const on = Math.max(0, themes.indexOf(theme === 'auto' ? 'system' : (theme as (typeof themes)[number])));
      return (
        <>
        {/* his bim's look first (page bim-pick, his pick A): the coin and the office follow at once */}
        <MeGroup
          look={lookOf(getPref(prefs, 'bim')) ?? TODAY}
          set={(l) => {
            setPref('bim', prefOf(l));
          }}
        />
        <Group
          title={settings.theme_label}
          testId="settings.general"
          items={[
            {
              id: 'theme',
              label: settings.theme_row,
              sub: settings.theme_sub,
              control: (
                <Seg
                  items={themeWords}
                  on={on}
                  onPick={(i) => {
                    setPref('theme', themes[i] ?? 'system');
                  }}
                  testId="settings.pref.theme"
                />
              ),
            },
          ]}
        />
        {/* mockup 10: notifications in a group of their own (none when macOS's state isn't known) */}
        {notifyItems(data.notify ?? null, ports.notify).length > 0 && (
          <Group title={settings.notify_title} testId="settings.general.notify" items={notifyItems(data.notify ?? null, ports.notify)} />
        )}
        {updateRow(data.appUpdate ?? null, null) !== null || !data.dist ? (
          <UpdateGroup update={data.appUpdate ?? null} ports={ports.update} />
        ) : (
          <DistUpdateGroup
            state={data.dist.state}
            info={data.dist.info}
            now={data.dist.readAt}
            beta={str(prefs, 'desktop_channel', 'stable') === 'beta'}
            setBeta={(on) => {
              setPref('desktop_channel', on ? 'beta' : 'stable');
            }}
            ports={ports.dist}
          />
        )}
        {/* update-flow (Gabriel's pick, designer m_45087): bise's own updates, off by default; it
            takes the 'Updates' head when no update group above has it (designer m_48227) */}
        <Group
          title={updatesShown(data) ? undefined : settings.update_title}
          testId="settings.general.bise"
          items={[tg('auto_update', settings.auto_update, settings.auto_update_sub, false)]}
        />
        </>
      );
    }
    case 'voice': {
      const lang = str(prefs, 'voice.language', 'auto');
      const langs: readonly (readonly [string, string])[] = [
        ['auto', settings.lang_auto],
        ['en', settings.lang_en],
        ['fr', settings.lang_fr],
      ];
      return (
        <Group
          testId="settings.voice"
          items={[
            tg('voice.fn_hold', settings.voice_fn_hold, settings.voice_fn_hold_sub),
            tg('voice.fn_twice', settings.voice_fn_twice, settings.voice_fn_twice_sub),
            {
              id: 'voice.language',
              label: settings.voice_language,
              sub: settings.voice_language_sub,
              control: (
                <Seg
                  items={langs.map(([, n]) => n)}
                  on={Math.max(0, langs.findIndex(([v]) => v === lang))}
                  onPick={(i) => {
                    setPref('voice.language', langs[i]?.[0] ?? 'auto');
                  }}
                  testId="settings.pref.voice.language"
                />
              ),
            },
          ]}
        />
      );
    }
    case 'privacy':
      return (
        <>
          <TelemetryGroup prefs={prefs} view={data.telemetry ?? null} send={ports.send} />
          <Privacy prefs={prefs} setPref={setPref} tg={tg} />
        </>
      );
    case 'quiet': {
      const every = num(prefs, 'quiet.batch_min', 30);
      return (
        <>
          <Group
            title={settings.quiet_wait}
            testId="settings.quiet"
            items={[
              tg('quiet.call', settings.quiet_call, settings.quiet_call_sub),
              tg('quiet.share', settings.quiet_share, settings.quiet_share_sub),
              tg('quiet.fullscreen', settings.quiet_fullscreen, settings.quiet_fullscreen_sub),
              tg('quiet.focus', settings.quiet_focus, settings.quiet_focus_sub),
            ]}
          />
          <Group
            title={settings.quiet_then}
            testId="settings.quiet.then"
            items={[
              tg('quiet.batch', settings.quiet_batch, settings.quiet_batch_sub),
              {
                id: 'quiet.batch_min',
                label: settings.quiet_batch_min,
                sub: settings.quiet_batch_min_sub,
                control: (
                  <Seg
                    items={[10, 30, 60].map((n) => settings.minutes({ n }))}
                    on={Math.max(0, [10, 30, 60].indexOf(every))}
                    onPick={(i) => {
                      setPref('quiet.batch_min', [10, 30, 60][i] ?? 30);
                    }}
                    testId="settings.pref.quiet.batch_min"
                  />
                ),
              },
              tg('quiet.away_card', settings.quiet_away, settings.quiet_away_sub),
            ]}
          />
        </>
      );
    }
    case 'approvals':
      return <Approvals approvals={data.approvals ?? null} send={ports.approvals ?? null} error={data.approvalsError ?? null} home={data.home ?? ''} />;
    case 'projects':
      return (
        <Projects
          projects={data.projects}
          found={data.found}
          ports={ports}
          // ~/acme, never /Users/… (designer m_43851): the shell's home, or the folder that holds bise's home
          home={homeOf(data.home, data.projects.map((p) => p.path), data.projects.find((p) => p.home)?.path) ?? ''}
        />
      );
    case 'setup':
      // a new route (/models, /voice) opens the page again at its part
      return <SetupPage key={route ?? 'setup'} data={data} ports={ports} page={route} setPref={setPref} />;
    case 'computer':
      return <Computer cu={data.computerUse} send={ports.send} />;
    case 'keys':
      // Shortcuts: the keyboard's keys only (designer m_43851); his API keys and sign-ins are Models'
      return <KeyList />;
  }
}

function notifyItems(auth: NotifyAuth | null, act: ((a: 'ask' | 'open') => void) | undefined): GroupItem[] {
  const row = notifyRow(auth);
  if (row === null) return [];
  const go = row.act;
  return [
    {
      id: 'notify',
      label: settings.notify_label,
      sub: <span data-testid="settings.notify.sub">{row.sub}</span>,
      control:
        go === null ? (
          <Chip tone="ok" testId="settings.notify.on">
            {settings.notify_on_chip}
          </Chip>
        ) : (
          // designer m_18736: nothing ghost in rows; 'turn on' (ask) is the row's own action, the
          // way to the system's settings an Act
          go === 'ask' ? (
            <Button kind="pri" onClick={() => { act?.(go); }} testId={`settings.notify.${go}`}>
              {row.button}
            </Button>
          ) : (
            <Act onClick={() => { act?.(go); }} testId={`settings.notify.${go}`}>
              {row.button}
            </Act>
          )
        ),
    },
  ];
}

function Privacy({ prefs, setPref, tg }: { prefs: Prefs | null; setPref: (key: string, value: JsonValue) => void; tg: (key: string, label: string, sub: string, dflt?: boolean) => GroupItem }) {
  const days = num(prefs, 'shots_keep_days', 7);
  const apps = excludedApps(prefs);
  const [adding, setAdding] = useState(false);
  const flip = (id: string) => {
    setPref('excluded_apps', apps.includes(id) ? apps.filter((a) => a !== id) : [...apps, id]);
  };
  return (
    <>
      <Group
        title={settings.privacy_hold}
        testId="settings.privacy"
        items={[
          tg('context.shot', settings.privacy_shot, settings.privacy_shot_sub),
          tg('context.url', settings.privacy_url, settings.privacy_url_sub),
          tg('context.file', settings.privacy_file, settings.privacy_file_sub),
          tg('context.quote', settings.privacy_quote, settings.privacy_quote_sub),
          tg('context.window_text', settings.privacy_window_text, settings.privacy_window_text_sub),
        ]}
      />
      <Group
        title={settings.privacy_kept}
        testId="settings.privacy.kept"
        items={[
          {
            id: 'shots_keep_days',
            label: settings.privacy_shots,
            sub: settings.privacy_shots_sub,
            control: (
              <Seg
                items={[1, 7, 30].map((n) => settings.days({ n }))}
                on={Math.max(0, [1, 7, 30].indexOf(days))}
                onPick={(i) => {
                  setPref('shots_keep_days', [1, 7, 30][i] ?? 7);
                }}
                testId="settings.pref.shots_keep_days"
              />
            ),
          },
        ]}
      />
      <WebHistoryGroup />
      <Group
        title={settings.privacy_apps}
        testId="settings.privacy.apps"
        help={settings.privacy_apps_help}
        items={[
          ...knownApps(prefs).map((id) => ({
            id,
            label: appName(prefs, id),
            sub: <span className="settings-app-id">{id}</span>,
            control: (
              <Toggle
                on={apps.includes(id)}
                label={appName(prefs, id)}
                onChange={() => {
                  flip(id);
                }}
                testId={`settings.app.${id}`}
              />
            ),
          })),
          {
            id: 'add',
            label: adding ? <AddApp onAdd={(id) => { flip(id); setAdding(false); }} onCancel={() => { setAdding(false); }} /> : settings.privacy_another_app,
            control: adding ? undefined : (
              <Button onClick={() => { setAdding(true); }} testId="settings.app.add">
                {settings.privacy_add}
              </Button>
            ),
          },
        ]}
      />
    </>
  );
}

/**
 * The web pages he opened from bise, for New tab's search (page newtab-omnibox-flow; architect
 * m_64583): his switch 'Keep the links I open' (off clears it and records nothing) and how many
 * pages, with Clear. Kept by the app on this Mac only (main/webhistoryipc.ts).
 */
function WebHistoryGroup() {
  const h = useWebHistory();
  return (
    <Group
      title={settings.privacy_web}
      testId="settings.privacy.web"
      items={[
        {
          id: 'web.keep',
          label: settings.privacy_web_keep,
          sub: settings.privacy_web_keep_sub,
          control: (
            <Toggle
              on={h.keep}
              label={settings.privacy_web_keep}
              onChange={() => {
                keepWebHistory(!h.keep);
              }}
              testId="settings.privacy.web.keep"
            />
          ),
        },
        {
          id: 'web.pages',
          label: settings.privacy_web_pages({ n: h.pages.length }),
          sub: settings.privacy_web_pages_sub,
          control: (
            <Button disabled={h.pages.length === 0} onClick={clearWebHistory} testId="settings.privacy.web.clear">
              {settings.privacy_web_clear}
            </Button>
          ),
        },
      ]}
    />
  );
}

/** a bundle id typed by hand (com.example.app), ⏎ adds it */
function AddApp({ onAdd, onCancel }: { onAdd: (id: string) => void; onCancel: () => void }) {
  return (
    <Field
      autoFocus
      placeholder={settings.privacy_app_id}
      testId="settings.app.id"
      onKeyDown={(e) => {
        const v = e.currentTarget.value.trim();
        if (e.key === 'Enter' && /^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$/.test(v)) {
          e.preventDefault();
          onAdd(v);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          onCancel();
        }
      }}
    />
  );
}

/** bar 3.5: every key of the app */
export function KeyList(): ReactNode {
  return (
    <>
      {KEY_SECTIONS.map((s) => (
        <Group
          key={s.id}
          title={s.title}
          testId={`settings.keys.${s.id}`}
          items={s.rows.map((r) => ({
            id: r.keys,
            // a row title: sentence case like every page's rows (designer m_43882), the sub lowercase
            label: chromeCase(r.action),
            sub: r.tui === undefined ? undefined : settings.key_terminal({ keys: r.tui }),
            // one keycap per key, a text-3 ' / ' or ' … ' between two (designer m_43941)
            control: (
              <span className="settings-caps">
                {capsOf(r.keys).map((c, i) =>
                  c.sep ? (
                    <span key={i} className="settings-caps-sep">
                      {c.text}
                    </span>
                  ) : (
                    <Kbd key={i}>{c.text}</Kbd>
                  ),
                )}
                {/* older keys that still work, dim after a ' · ' (history-keys, designer m_58552) */}
                {r.also !== undefined && (
                  <span className="settings-caps-also" data-testid="settings.keys.also">
                    <span className="settings-caps-sep">·</span>
                    {capsOf(r.also).map((c, i) =>
                      c.sep ? (
                        <span key={i} className="settings-caps-sep">
                          {c.text}
                        </span>
                      ) : (
                        <Kbd key={i}>{c.text}</Kbd>
                      ),
                    )}
                  </span>
                )}
              </span>
            ),
          }))}
        />
      ))}
      <Chip>{settings.keys_same}</Chip>
    </>
  );
}

/**
 * settings · models: his accounts, then which model does what (bar V17). A role set to a model
 * whose provider has no key or sign-in yet waits for its setup here (bar L5): the accounts show
 * that provider's sign-in or open its key field, and the screen sends the held role_set once the
 * provider is signed in (hold.ts).
 */

