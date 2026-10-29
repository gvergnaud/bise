// A plugin's views (MCP Apps, docs/mcp-apps.md C8): on its page, the VIEWS group (each view, the
// origins it reaches, a tick per permission it declares, 'not granted' when he was never asked);
// on the add screen, the views it shows before it runs, ticked there. A tick sends `plugin_grant`
// with the view's whole grant; what he leaves unticked is not granted. Owner: mcp-apps-host.
import { Check, GroupLabel, sentenceCase } from '@/ds';
import type { AppCmd, PluginPart } from '@/proto';
import { views } from '@/proto/words_views.gen';

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
