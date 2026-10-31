// A plugin's views (MCP Apps, docs/mcp-apps.md C8): on its page, the VIEWS group (each view, what
// it uses and reaches, a switch per permission it declares, 'not granted' while off); on the add
// screen, the views it shows before it runs, switched there. A switch sends `plugin_grant` with
// the view's whole grant; what he leaves off is not granted. Owner: mcp-apps-host.
import { useEffect, useRef, useState } from 'react';
import { Button, GroupLabel, sentenceCase, Sheet, Toggle } from '@/ds';
import type { AppCmd, PluginPart } from '@/proto';
import { views } from '@/proto/words_views.gen';
import { permRow, permsOf } from '@/shared/viewperms';
import { humanName, permsIn } from '@/views/names';
import type { PluginViewsAsk } from '@/views/port';

/** A permission's words (its tick's label, the generated table's); an unknown name as it is. */
export function permWords(p: string): string {
  return permRow(p)?.word ?? p;
}

/** A view's grant with one permission ticked or not, in its declared order. Pure. */
export function ticked(declared: readonly string[], granted: readonly string[], perm: string, on: boolean): string[] {
  return declared.filter((p) => (p === perm ? on : granted.includes(p)));
}

/** A view's sub line: what it uses, the origins it reaches, or that it asks for nothing. Pure. */
export function viewWhat(declared: readonly string[], domains: readonly string[]): string {
  const parts = [
    ...(declared.length > 0 ? [views.uses({ perms: permsIn(declared) })] : []),
    ...(domains.length > 0 ? [views.reaches({ domains: domains.map((d) => d.replace(/^https?:\/\//, '')).join(', ') })] : []),
  ];
  return parts.length === 0 ? views.asks_nothing : parts.join(' · ');
}

/**
 * A view's rows, like a server's (designer m_50385): its name and what it uses; then one row per
 * permission it declares, its switch at the end ('not granted' before it when off). A view that
 * asks nothing has no switch.
 */
function ViewRow({ x, granted, onTick, testId }: { x: PluginPart; granted: readonly string[]; onTick: (perm: string, on: boolean) => void; testId: string }) {
  const v = x.view;
  if (v == null) return null;
  const declared = v.declared ?? [];
  return (
    <>
      <div className="plg-row part" data-testid={testId}>
        <span className="plg-lead" aria-hidden="true" />
        <span className="plg-row-words">
          <span className="plg-row-name">
            <span className="plg-title">{humanName(x.name)}</span>
          </span>
          <span className="plg-row-what plg-dim">{viewWhat(declared, v.domains ?? [])}</span>
        </span>
      </div>
      {declared.map((p) => {
        const on = granted.includes(p);
        return (
          <div key={p} className="plg-row part plg-perm" data-testid={`${testId}.perm.${p}`}>
            <span className="plg-row-words">{permWords(p)}</span>
            <span className="plg-row-end">
              {!on && <span className="plg-dim" data-testid={`${testId}.perm.${p}.off`}>{views.not_granted}</span>}
              <Toggle on={on} label={permWords(p)} onChange={(o) => { onTick(p, o); }} testId={`${testId}.perm.${p}.toggle`} />
            </span>
          </div>
        );
      })}
    </>
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
    const declared = permsOf(v['permissions'], v['bise_permissions']);
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

/** How step 2 waits for a server's views: asked again `every` ms while none came, for `upTo` ms (the hub's views bridge rereads plugins every 2 s, its READY_WAIT is 12 s). */
export const STEP_WAIT = { every: 1000, upTo: 15_000 } as const;

/**
 * The add's step 2 (C8): once the plugin runs, its server's views with their ticks; 'done' grants
 * what he ticked. Nothing found yet (the views bridge may not have loaded the plugin: a 404, an
 * empty list; examples m_50524) is asked again each second for 15 s, still 'starting…'; then it closes.
 */
export function ViewsStep({ plugin, project, skip, ask, send, onClose, wait = STEP_WAIT }: { plugin: string; project: string; skip: readonly string[]; ask: PluginViewsAsk; send: (cmd: AppCmd) => void; onClose: () => void; wait?: { readonly every: number; readonly upTo: number } }) {
  const [parts, setParts] = useState<PluginPart[] | null>(null);
  const [ticks, setTicks] = useState<Record<string, readonly string[]>>({});
  // the parent's onClose is a new function at each of its renders: read through a ref, never an
  // effect's dependency (each render restarted the asks, faster than once a second, and pushed the
  // bound back without end); the bound counts from the step's opening, once
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);
  useEffect(() => {
    let live = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let asks = 0;
    const once = () => {
      asks += 1;
      void ask({ cmd: 'plugin_views', project, plugin }).then((a) => {
        if (!live) return;
        const found = 'error' in a ? [] : partsOfList(plugin, a.result, skip);
        if (found.length > 0) setParts(found);
        else if (asks * wait.every > wait.upTo) close.current();
        else timer = setTimeout(once, wait.every);
      });
    };
    once();
    return () => {
      live = false;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [ask, project, plugin, skip, wait.every, wait.upTo]);
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
