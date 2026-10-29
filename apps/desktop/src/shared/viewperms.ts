// src/shared · a view's permissions (MCP Apps, docs/mcp-apps.md C6, C8): the names the hub's
// view/resource, the plugin's grant and Electron main's permission rule share ('camera',
// 'microphone', 'clipboardWrite' are the spec's _meta.ui.permissions keys; 'midi' is bise's,
// _meta["dev.bise/ui"].permissions.midi), and the iframe's `allow=` built from them. Pure: the
// renderer's views layer (src/views) and main's viewframes.ts (mcp-apps-sandbox) import it, so the
// frame's allow and main's grant never drift. Owner: mcp-apps-host.

/** One permission a view may hold. */
export type ViewPerm = 'camera' | 'microphone' | 'midi' | 'clipboardWrite';

/** Every permission, in the order an `allow=` lists them. */
export const VIEW_PERMS: readonly ViewPerm[] = ['camera', 'microphone', 'midi', 'clipboardWrite'];

/** The permission policy feature each one delegates to the frame. */
const FEATURE: Readonly<Record<ViewPerm, string>> = {
  camera: 'camera',
  microphone: 'microphone',
  midi: 'midi',
  clipboardWrite: 'clipboard-write',
};

/** Is it a permission's name? */
export function isViewPerm(x: unknown): x is ViewPerm {
  return typeof x === 'string' && (VIEW_PERMS as readonly string[]).includes(x);
}

/**
 * The permissions in a JSON value: a list of names (`["camera"]`) or the spec's object
 * (`{camera: {}, microphone: {}}`, a key set when its value is not false or null); unknown names
 * dropped, each once, in VIEW_PERMS' order. Pure.
 */
export function permsOf(v: unknown): ViewPerm[] {
  let names: unknown[] = [];
  if (Array.isArray(v)) names = v;
  else if (v !== null && typeof v === 'object') names = Object.entries(v).filter(([, x]) => x !== false && x !== null).map(([k]) => k);
  return VIEW_PERMS.filter((p) => names.includes(p));
}

/** The `allow=` of a view's frame for these permissions: '' for none. Pure. */
export function allowOf(perms: readonly ViewPerm[]): string {
  return VIEW_PERMS.filter((p) => perms.includes(p))
    .map((p) => FEATURE[p])
    .join('; ');
}
