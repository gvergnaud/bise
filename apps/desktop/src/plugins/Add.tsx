// The add menu and its sheets (page plugins §4, plug-add's wire, architect m_39121): browse, an MCP
// server (a command or a URL, its secrets), an import (Claude Code, Codex, Cursor), a plugin (a
// folder or a git URL), a marketplace (a git URL), make one with an agent. Each sends
// `plugin_add`; what will run comes back as `plugin_preview` and shows before his click; `add`
// sends `plugin_add_confirm {token, secrets}` (the values only then, never kept by the window:
// the sheet's state drops them at its close), `cancel` sends `plugin_add_cancel`. Nothing runs
// before his click. Owner: plug-desktop.
import { useRef, useState } from 'react';
import { Button, Caret, Field, PickerPop, sentenceCase, Sheet, TextArea } from '@/ds';
import type { AppCmd, ImportFrom, PluginPreview, PluginsSummary, SecretValue } from '@/proto';
import { settings } from '@/proto/words.gen';
import { itemsOf } from './model';
import { grantsOf, PreviewViews } from './ViewsGroup';

/** The menu's items, in its order. */
export type AddItem = 'browse' | 'mcp' | 'import' | 'plugin' | 'market' | 'agent';
export const ADD_ITEMS: readonly AddItem[] = ['browse', 'mcp', 'import', 'plugin', 'market', 'agent'];

const GLYPH: Record<AddItem, string> = { browse: '⌕', mcp: '+', import: '↓', plugin: '⊞', market: '⌂', agent: ':*' };

/** An item's words and its line (the import's line names what was found, from the summary). */
export function itemWords(item: AddItem, summary: PluginsSummary | null): { readonly name: string; readonly what: string } {
  switch (item) {
    case 'browse':
      return { name: settings.plugins_menu_browse, what: settings.plugins_menu_browse_what };
    case 'mcp':
      return { name: settings.plugins_add_mcp, what: settings.plugins_add_mcp_what };
    case 'import': {
      const found = itemsOf(summary, 'import')[0];
      return { name: settings.plugins_add_import, what: found === undefined ? settings.plugins_add_import_what : settings.plugins_add_import_found({ found: (found.titles ?? []).join(', ') }) };
    }
    case 'plugin':
      return { name: settings.plugins_add_plugin, what: settings.plugins_add_plugin_what };
    case 'market':
      return { name: settings.plugins_add_market, what: settings.plugins_add_market_what };
    case 'agent':
      return { name: settings.plugins_add_agent, what: settings.plugins_add_agent_what };
  }
}

/** 'add' and its menu (the ds PickerPop from the button: its keys, its 50vh cap). */
export function AddMenu({ summary, onPick }: { summary: PluginsSummary | null; onPick: (item: AddItem) => void }) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLButtonElement>(null);
  return (
    <>
      <Button ref={anchor} kind="pri" pressed={open} onClick={() => { setOpen(!open); }} testId="plugins.add">
        <span className="plg-add-words">
          {sentenceCase(settings.plugins_add)}
          <Caret />
        </span>
      </Button>
      {open && (
        <PickerPop anchor={anchor} align="end" onClose={() => { setOpen(false); }} testId="plugins.add.menu"
          groups={[{
            id: 'add',
            rows: ADD_ITEMS.map((it) => {
              const w = itemWords(it, summary);
              return { id: it, name: w.name, secondary: w.what, glyph: GLYPH[it], testId: `plugins.add.${it}` };
            }),
          }]}
          onPick={(id) => {
            setOpen(false);
            const it = ADD_ITEMS.find((x) => x === id);
            if (it !== undefined) onPick(it);
          }} />
      )}
    </>
  );
}

/** 'KEY=value' lines (or '; ' between them) as names and values; a line without '=' is skipped. Pure. */
export function envOf(text: string): { name: string; value: string }[] {
  return text
    .split(/\n|;/u)
    .map((l) => l.trim())
    .flatMap((l) => {
      const i = l.indexOf('=');
      if (i <= 0) return [];
      return [{ name: l.slice(0, i).trim(), value: l.slice(i + 1).trim() }];
    });
}

/** The commands his sheets send, and the preview's (its secrets only on the confirm). */
export interface AddPorts {
  send: (cmd: AppCmd) => void;
}

/**
 * The sheet of one add item, open while `item` is set. `project`: the list's (none: bise's home).
 * The MCP sheet's env values are secrets: they wait in the sheet's memory until the preview's add,
 * go out once in `plugin_add_confirm`, and are dropped with the sheet.
 */
export function AddSheet({ item, project, summary, preview, ports, onClose }: {
  item: Exclude<AddItem, 'browse'> | null;
  project: string | undefined;
  summary: PluginsSummary | null;
  preview: { readonly token: string; readonly preview: PluginPreview } | null;
  ports: AddPorts;
  onClose: () => void;
}) {
  const [name, setName] = useState('');
  const [target, setTarget] = useState('');
  const [env, setEnv] = useState('');
  const [words, setWords] = useState('');
  const [asked, setAsked] = useState(false);
  // the values of the secrets he typed, by name: sent only with the confirm
  const [secrets, setSecrets] = useState<Record<string, string>>({});
  // the views' ticks, by uri: granted once he adds it (C8), nothing ticked is nothing granted
  const [ticks, setTicks] = useState<Record<string, readonly string[]>>({});
  const at = project === undefined ? {} : { project };
  const reset = () => {
    setName('');
    setTarget('');
    setEnv('');
    setWords('');
    setAsked(false);
    setSecrets({});
    setTicks({});
  };
  const close = () => {
    if (preview !== null) ports.send({ cmd: 'plugin_add_cancel', token: preview.token });
    reset();
    onClose();
  };
  if (item === null) return null;
  const go = (cmd: AppCmd) => {
    ports.send(cmd);
    setAsked(true);
  };
  // the preview came back: it shows what runs, then add or cancel
  if (asked && preview !== null) {
    return (
      <PreviewSheet preview={preview.preview} values={secrets} onValue={(n, v) => { setSecrets({ ...secrets, [n]: v }); }}
        ticks={ticks} onTicks={(uri, perms) => { setTicks({ ...ticks, [uri]: perms }); }}
        onAdd={() => {
          const out: SecretValue[] = preview.preview.secrets?.flatMap((n) => (secrets[n] !== undefined && secrets[n] !== '' ? [{ name: n, value: secrets[n] }] : [])) ?? [];
          ports.send(out.length === 0 ? { cmd: 'plugin_add_confirm', token: preview.token } : { cmd: 'plugin_add_confirm', token: preview.token, secrets: out });
          for (const g of grantsOf(preview.preview.name, preview.preview.views ?? [], ticks, project)) ports.send(g);
          reset();
          onClose();
        }}
        onCancel={close} />
    );
  }
  const foot = (primary: string, ok: boolean, run: () => void) => (
    <>
      <Button kind="ghost" onClick={close} testId="plugins.sheet.cancel">{settings.plugins_cancel}</Button>
      <Button kind="pri" state={asked ? 'loading' : ok ? undefined : 'disabled'} onClick={run} testId="plugins.sheet.go">{primary}</Button>
    </>
  );
  switch (item) {
    case 'mcp': {
      const kv = envOf(env);
      const ok = name.trim() !== '' && target.trim() !== '';
      const run = () => {
        if (!ok) return;
        const t = target.trim();
        const isUrl = /^https?:\/\//u.test(t);
        setSecrets(Object.fromEntries(kv.map((x) => [x.name, x.value])));
        go({ cmd: 'plugin_add', ...at, add: { kind: 'mcp', name: name.trim(), ...(isUrl ? { url: t } : { command: t }), ...(kv.length > 0 ? { secrets: kv.map((x) => x.name) } : {}) } });
      };
      return (
        <Sheet open onClose={close} title={settings.plugins_add_mcp} sub={settings.plugins_mcp_sub} onPrimary={run} foot={foot(settings.plugins_add_go, ok, run)} testId="plugins.sheet.mcp">
          <div className="plg-form">
            <label className="plg-label">{settings.plugins_field_name}</label>
            <Field autoFocus value={name} onChange={(e) => { setName(e.currentTarget.value); }} testId="plugins.sheet.mcp.name" />
            <label className="plg-label">{settings.plugins_field_target}</label>
            <Field value={target} onChange={(e) => { setTarget(e.currentTarget.value); }} testId="plugins.sheet.mcp.target" />
            <label className="plg-label">{settings.plugins_field_env}</label>
            <TextArea value={env} placeholder={settings.plugins_field_env_hint} onChange={(e) => { setEnv(e.currentTarget.value); }} testId="plugins.sheet.mcp.env" />
            <div className="plg-dim">{settings.plugins_secrets_where}</div>
          </div>
        </Sheet>
      );
    }
    case 'import': {
      const found = itemsOf(summary, 'import')[0];
      const froms: { from: ImportFrom; title: string }[] = [
        { from: 'claude', title: 'Claude Code' },
        { from: 'codex', title: 'Codex' },
        { from: 'cursor', title: 'Cursor' },
      ];
      return (
        <Sheet open onClose={close} title={settings.plugins_import_head} sub={settings.plugins_import_sub} testId="plugins.sheet.import"
          foot={<Button kind="ghost" onClick={close} testId="plugins.sheet.cancel">{settings.plugins_cancel}</Button>}>
          <div className="plg-form">
            {found === undefined && <div className="plg-dim" data-testid="plugins.sheet.import.none">{settings.plugins_import_none}</div>}
            {froms.map((f) => {
              const has = (found?.titles ?? []).includes(f.title);
              return (
                <div key={f.from} className="plg-band-line">
                  <span className={has ? undefined : 'plg-dim'}>{f.title}</span>
                  <Button size="sm" state={asked ? 'loading' : has ? undefined : 'disabled'}
                    onClick={() => { go({ cmd: 'plugin_add', ...at, add: { kind: 'import', from: f.from } }); }} testId={`plugins.sheet.import.${f.from}`}>
                    {settings.plugins_band_import({ n: 1 }).replace(/ 1$/u, '')}
                  </Button>
                </div>
              );
            })}
          </div>
        </Sheet>
      );
    }
    case 'plugin': {
      const ok = target.trim() !== '';
      const run = () => {
        if (!ok) return;
        const t = target.trim();
        go({ cmd: 'plugin_add', ...at, add: t.startsWith('/') || t.startsWith('~') ? { kind: 'folder', path: t } : { kind: 'git', url: t } });
      };
      return (
        <Sheet open onClose={close} title={settings.plugins_add_plugin} sub={settings.plugins_add_plugin_what} onPrimary={run} foot={foot(settings.plugins_add_go, ok, run)} testId="plugins.sheet.plugin">
          <div className="plg-form">
            <label className="plg-label">{settings.plugins_field_source}</label>
            <Field autoFocus value={target} onChange={(e) => { setTarget(e.currentTarget.value); }} testId="plugins.sheet.plugin.source" />
          </div>
        </Sheet>
      );
    }
    case 'market': {
      const ok = target.trim() !== '';
      const run = () => {
        if (!ok) return;
        ports.send({ cmd: 'plugin_add', ...at, add: { kind: 'marketplace', url: target.trim() } });
        reset();
        onClose();
      };
      return (
        <Sheet open onClose={close} title={settings.plugins_add_market} sub={settings.plugins_add_market_what} onPrimary={run} foot={foot(settings.plugins_add_go, ok, run)} testId="plugins.sheet.market">
          <div className="plg-form">
            <label className="plg-label">{settings.plugins_field_market}</label>
            <Field autoFocus value={target} placeholder="openai/plugins" onChange={(e) => { setTarget(e.currentTarget.value); }} testId="plugins.sheet.market.url" />
          </div>
        </Sheet>
      );
    }
    case 'agent': {
      const ok = words.trim() !== '';
      const run = () => {
        if (!ok) return;
        ports.send({ cmd: 'plugin_add', ...at, add: { kind: 'agent', words: words.trim() } });
        reset();
        onClose();
      };
      return (
        <Sheet open onClose={close} title={settings.plugins_add_agent} sub={settings.plugins_agent_sub} foot={foot(settings.plugins_add_go, ok, run)} testId="plugins.sheet.agent">
          <div className="plg-form">
            <label className="plg-label">{settings.plugins_agent_field}</label>
            <TextArea autoFocus value={words} onChange={(e) => { setWords(e.currentTarget.value); }} testId="plugins.sheet.agent.words" />
          </div>
        </Sheet>
      );
    }
  }
}

/** What a plugin will run, before his click (page plugins §4: 'add LaTeX: what it runs'). */
export function PreviewSheet({ preview, values, onValue, ticks = {}, onTicks = () => undefined, onAdd, onCancel }: {
  preview: PluginPreview;
  values: Record<string, string>;
  onValue: (name: string, value: string) => void;
  /** the views' ticks by uri (C8) */
  ticks?: Readonly<Record<string, readonly string[]>>;
  onTicks?: (uri: string, perms: string[]) => void;
  onAdd: () => void;
  onCancel: () => void;
}) {
  const runs = preview.runs ?? [];
  const skills = preview.skills ?? [];
  const secrets = preview.secrets ?? [];
  return (
    <Sheet open onClose={onCancel} title={settings.plugins_preview_head({ name: preview.name })} sub={settings.plugins_preview_from({ from: preview.from })} testId="plugins.preview"
      foot={
        <>
          <Button kind="ghost" onClick={onCancel} testId="plugins.preview.cancel">{settings.plugins_cancel}</Button>
          <Button kind="pri" onClick={onAdd} testId="plugins.preview.add">{runs.length > 0 ? settings.plugins_key_add_run : settings.plugins_add_go}</Button>
        </>
      }>
      <div className="plg-form">
        {runs.length === 0 ? (
          <div className="plg-dim">{settings.plugins_preview_nothing}</div>
        ) : (
          <div className="plg-runs" data-testid="plugins.preview.runs">
            {runs.map((r) => (
              <div key={r.server} className="plg-run">
                <span className="plg-run-name">{r.server}</span>
                <code className="plg-run-cmd">{r.transport === 'stdio' ? r.argv?.join(' ') : r.host}</code>
              </div>
            ))}
          </div>
        )}
        {skills.length > 0 && (
          <>
            <label className="plg-label">{settings.plugins_preview_label_skills}</label>
            <div className="plg-dim">{skills.join(', ')}</div>
          </>
        )}
        {secrets.length > 0 && (
          <>
            <label className="plg-label">{settings.plugins_preview_label_secrets}</label>
            {secrets.map((n) => (
              <div key={n} className="plg-secret">
                <span className="plg-run-name">{n}</span>
                <Field type="password" value={values[n] ?? ''} placeholder={settings.plugins_secret_empty({ name: n })} onChange={(e) => { onValue(n, e.currentTarget.value); }}
                  testId={`plugins.preview.secret.${n}`} />
              </div>
            ))}
            <div className="plg-dim">{settings.plugins_secrets_where}</div>
          </>
        )}
        {/* MCP Apps: its views and what each may use, ticked here (C8, mcp-apps-host) */}
        <PreviewViews parts={preview.views ?? []} ticks={ticks} onTicks={onTicks} />
        {(preview.dropped ?? []).length > 0 && (
          <>
            <label className="plg-label">{settings.plugins_preview_label_dropped}</label>
            <div className="plg-dim">{(preview.dropped ?? []).join(', ')}</div>
          </>
        )}
        {preview.replaces != null && <div className="plg-acc">{settings.plugins_preview_replaces({ name: preview.replaces })}</div>}
        {(preview.notes ?? []).map((n) => (
          <div key={n} className="plg-dim">{n}</div>
        ))}
        <div className="plg-dim">{settings.plugins_preview_after}</div>
      </div>
    </Sheet>
  );
}
