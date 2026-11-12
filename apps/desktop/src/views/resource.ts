// src/views · view/resource's answer read (docs/mcp-apps.md C5, C6), and view/open's view part
// (docs/plugin-apps.md A4) which answers the same fields: the view's origin, its proxy url (checked
// to be that origin's /mcp-app), its html, CSP and permissions, the tool's input and result. Pure.
// Owner: apps-host.
import { permsOf, type ViewPerm } from '@/shared/viewperms';
import type { Json, JsonObject } from './protocol';
import type { ViewResource } from './session';

/** Is `url` the view's proxy on `origin` (its /mcp-app, with the hub's session query)? Pure. */
function isProxyOf(url: unknown, origin: string): url is string {
  if (typeof url !== 'string') return false;
  try {
    const u = new URL(url);
    return u.origin === origin && u.pathname === '/mcp-app' && u.username === '' && u.password === '' && u.hash === '';
  } catch {
    return false;
  }
}

/** view/resource's result as a ViewResource, or null when it isn't one; no `proxy` (an older hub): <origin>/mcp-app. Pure. */
export function resourceOf(r: Json): ViewResource | null {
  if (r === null || typeof r !== 'object' || Array.isArray(r)) return null;
  const origin = r['origin'];
  const html = r['html'];
  if (typeof origin !== 'string' || typeof html !== 'string') return null;
  const proxy = r['proxy'] ?? `${origin}/mcp-app`;
  if (!isProxyOf(proxy, origin)) return null;
  const obj = (k: string): JsonObject | null => {
    const v = r[k];
    return v !== null && typeof v === 'object' && !Array.isArray(v) ? v : null;
  };
  return {
    origin,
    proxy,
    html,
    csp: r['csp'] ?? null,
    permissions: permsOf(r['permissions']),
    prefersBorder: r['prefersBorder'] !== false,
    toolInput: obj('toolInput'),
    toolResult: obj('toolResult'),
    tool: obj('tool'),
  };
}

/** A view's permissions it declared but was not granted: view/resource's `declared` minus `permissions`. Pure. */
export function ungranted(r: Json): readonly ViewPerm[] {
  if (r === null || typeof r !== 'object' || Array.isArray(r)) return [];
  const granted = permsOf(r['permissions']);
  return permsOf(r['declared']).filter((p) => !granted.includes(p));
}
