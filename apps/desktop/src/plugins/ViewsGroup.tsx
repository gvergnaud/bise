// A plugin's views (MCP Apps, docs/mcp-apps.md C8): on its page, the VIEWS group (each view, the
// origins it reaches, a tick per permission it declares, 'not granted' when he was never asked);
// on the add screen, the views it shows before it runs, ticked there. A tick sends `plugin_grant`
// with the view's whole grant; what he leaves unticked is not granted. Owner: mcp-apps-host.
import { useEffect, useState } from 'react';
import { Button, Check, GroupLabel, sentenceCase, Sheet } from '@/ds';
import type { AppCmd, PluginPart } from '@/proto';
import { views } from '@/proto/words_views.gen';
import { permsOf } from '@/shared/viewperms';
import type { PluginViewsAsk } from '@/views/port';

/** A permission's words. */
export function permWords(p: string): string {
  switch (p) {
    case 'camera':
      return views.perm_camera;
    case 'microphone':
      return views.perm_microphone;
    case 'midi':
      return views.perm_midi;
    case 'clipboardWrite':
      return views.perm_clipboard;
    default:
      return p;
  }
}

/** A view's grant with one permission ticked or not, in its declared order. Pure. */
export function ticked(declared: readonly string[], granted: readonly string[], perm: string, on: boolean): string[] {
  return declared.filter((p) => (p === perm ? on : granted.includes(p)));
}

/** A view's row: its name, the origins it reaches, its ticks (or that it asks for nothing). */
function ViewRow({ x, granted, onTick, testId }: { x: PluginPart; granted: readonly string[]; onTick: (perm: string, on: boolean) => void; testId: string }) {
  const v = x.view;
  if (v == null) return null;
  const declared = v.declared ?? [];
  const domains = v.domains ?? [];
  return (
    <div className="plg-row part" data-testid={testId}>
      <span className="plg-lead" aria-hidden="true" />
      <span className="plg-row-words">
        <span className="plg-row-name">
          <span className="plg-title plg-mono">{x.name}</span>
          {!v.asked && declared.length > 0 && <span className="plg-dim" data-testid={`${testId}.not_granted`}>{views.not_granted}</span>}
        </span>
        {domains.length > 0 && <span className="plg-row-what">{views.reaches({ domains: domains.map((d) => d.replace(/^https?:\/\//, '')).join(', ') })}</span>}
        {declared.length === 0 ? (
          <span className="plg-row-what">{views.asks_nothing}</span>
        ) : (
          <span className="plg-perms">
            {declared.map((p) => (
              <Check key={p} on={granted.includes(p)} label={permWords(p)} onChange={(on) => { onTick(p, on); }} testId={`${testId}.perm.${p}`} />
            ))}
          </span>
        )}
      </span>
    </div>
  );
}

/** The VIEWS group of a plugin's page: its view parts, each tick a `plugin_grant`. */
export function ViewsGroup({ plugin, parts, project, send }: { plugin: string; parts: readonly PluginPart[]; project?: string; send: (cmd: AppCmd) => void }) {
  const vs = parts.filter((x) => x.kind === 'view' && x.view != null);
  if (vs.length === 0) return null;
  return (
    <div className="plg-group" data-testid="plugins.views">
      <GroupLabel count={vs.length}>{sentenceCase(views.group)}</GroupLabel>
      <div className="plg-list">
        {vs.map((x) => {
          const v = x.view;
          if (v == null) return null;
          const granted = v.granted ?? [];
          return (
            <ViewRow key={x.id} x={x} granted={granted} testId={`plugins.view.${x.id}`}
              onTick={(perm, on) => {
                send({ cmd: 'plugin_grant', ...(project === undefined ? {} : { project }), plugin, uri: v.uri, perms: ticked(v.declared ?? [], granted, perm, on) });
              }} />
          );
        })}
      </div>
    </div>
  );
}

/** The add screen's views (its preview's), ticked in the sheet: `ticks` by uri. */
export function PreviewViews({ parts, ticks, onTicks }: { parts: readonly PluginPart[]; ticks: Readonly<Record<string, readonly string[]>>; onTicks: (uri: string, perms: string[]) => void }) {
  const vs = parts.filter((x) => x.view != null);
  if (vs.length === 0) return null;
  return (
    <>
      <label className="plg-label">{views.preview_label}</label>
      <div className="plg-list" data-testid="plugins.preview.views">
        {vs.map((x) => {
          const v = x.view;
          if (v == null) return null;
          const granted = ticks[v.uri] ?? [];
          return <ViewRow key={x.id} x={{ ...x, view: { ...v, asked: true } }} granted={granted} testId={`plugins.preview.view.${x.id}`} onTick={(perm, on) => { onTicks(v.uri, ticked(v.declared ?? [], granted, perm, on)); }} />;
        })}
      </div>
      <div className="plg-dim">{views.preview_ticks}</div>
    </>
  );
}

/** The grants an add sends after its confirm: one per previewed view, what he ticked (maybe nothing). Pure. */
export function grantsOf(plugin: string, parts: readonly PluginPart[], ticks: Readonly<Record<string, readonly string[]>>, project?: string): AppCmd[] {
  return parts.flatMap((x) => (x.view == null ? [] : [{ cmd: 'plugin_grant' as const, ...(project === undefined ? {} : { project }), plugin, uri: x.view.uri, perms: [...(ticks[x.view.uri] ?? [])] }]));
}

/** The CSP fields whose origins a view reaches (catalog::views' order). */
const DOMAIN_FIELDS = ['connectDomains', 'resourceDomains', 'frameDomains', 'baseUriDomains'] as const;

function obj(v: unknown): Record<string, unknown> | null {
  return v !== null && typeof v === 'object' && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
}

/**
 * hub/plugin_views' answer as view parts for the add's step 2 (the bridge's GET /<plugin>/~ui:
 * {views: [{server, tool, uri, csp, permissions, bise_permissions, …}]}), each once, never one of
 * `skip` (the static views ticked at step 1). Pure.
 */
export function partsOfList(plugin: string, result: unknown, skip: readonly string[] = []): PluginPart[] {
  const list = obj(result)?.['views'];
  if (!Array.isArray(list)) return [];
  const out: PluginPart[] = [];
  for (const raw of list) {
    const v = obj(raw);
    const uri = v?.['uri'];
    if (v === null || typeof uri !== 'string' || uri === '' || skip.includes(uri) || out.some((x) => x.view?.uri === uri)) continue;
    const csp = obj(v['csp']);
    const domains: string[] = [];
    for (const f of DOMAIN_FIELDS) {
      const ds = csp?.[f];
      if (Array.isArray(ds)) for (const d of ds) if (typeof d === 'string' && !domains.includes(d)) domains.push(d);
    }
    const declared = permsOf([...permsOf(v['permissions']), ...permsOf(v['bise_permissions'])]);
    const tool = typeof v['tool'] === 'string' ? v['tool'] : (uri.split('/').at(-1) ?? uri);
    out.push({
      id: `${plugin}#view:${encodeURIComponent(uri)}`,
      kind: 'view',
      name: tool,
      on: true,
      view: { uri, declared, domains, granted: [], asked: true, ...(typeof v['server'] === 'string' ? { server: v['server'] } : {}) },
    });
  }
  return out;
}

/**
 * The add's step 2 (C8): once the plugin runs, its server's views with their ticks; 'done' grants
 * what he ticked; nothing found (or the hub can't tell) closes it at once.
 */
export function ViewsStep({ plugin, project, skip, ask, send, onClose }: { plugin: string; project: string; skip: readonly string[]; ask: PluginViewsAsk; send: (cmd: AppCmd) => void; onClose: () => void }) {
  const [parts, setParts] = useState<PluginPart[] | null>(null);
  const [ticks, setTicks] = useState<Record<string, readonly string[]>>({});
  useEffect(() => {
    let live = true;
    void ask({ cmd: 'plugin_views', project, plugin }).then((a) => {
      if (!live) return;
      const found = 'error' in a ? [] : partsOfList(plugin, a.result, skip);
      if (found.length === 0) onClose();
      else setParts(found);
    });
    return () => {
      live = false;
    };
  }, [ask, project, plugin, skip, onClose]);
  return (
    <Sheet open onClose={onClose} title={views.step_head({ name: plugin })} testId="plugins.views_step"
      foot={
        <Button kind="pri" disabled={parts === null} onClick={() => {
          for (const g of grantsOf(plugin, parts ?? [], ticks, project)) send(g);
          onClose();
        }} testId="plugins.views_step.done">{sentenceCase(views.step_done)}</Button>
      }>
      <div className="plg-form">
        {parts === null ? (
          <div className="plg-dim" data-testid="plugins.views_step.starting">{views.step_starting}</div>
        ) : (
          <PreviewViews parts={parts} ticks={ticks} onTicks={(uri, perms) => { setTicks({ ...ticks, [uri]: perms }); }} />
        )}
      </div>
    </Sheet>
  );
}
