// src/shared · a view's permissions (MCP Apps, docs/mcp-apps.md C6, C8), read from the one list
// (bise_proto::view_rules, generated in proto/view_rules.gen.ts: the names, their `allow=` features
// and their words). What stays here is the TS reader of a raw JSON value (a plugin/views row, a
// view/resource answer) and the iframe's `allow=`. permsOf is a second reader of the rule
// bise_proto::view_rules::declared owns, kept only because tests/fixtures/mcp-apps/declared.json
// pins it to the hub's (viewperms.test.ts runs every row): change both or neither (architect,
// mcp-apps review d1). Pure: the renderer's views layer
// (src/views) and main's viewframes.ts (mcp-apps-sandbox) import it, so the frame's allow and main's
// grant never drift. Owner: mcp-apps-host.

import { VIEW_PERMS, type ViewPerm, type ViewPermRow } from '../proto/view_rules.gen';

export type { ViewPerm };

const NAMES: readonly ViewPerm[] = VIEW_PERMS.map((r) => r.perm);

/** A permission's row of the generated table; null for any other name. */
export function permRow(x: string): ViewPermRow | null {
  return VIEW_PERMS.find((r) => r.perm === x) ?? null;
}

/** Is it a permission's name? */
export function isViewPerm(x: unknown): x is ViewPerm {
  return typeof x === 'string' && (NAMES as readonly string[]).includes(x);
}

/**
 * The permissions some JSON values name (bise_proto::view_rules::perms_of, then declared's merge):
 * each a list of names (`["camera"]`, ours) or the spec's object (`{camera: {}}`, a key set when
 * its value is not false or null); unknown names dropped, each once, in VIEW_PERMS' order. Pure.
 */
export function permsOf(...vs: readonly unknown[]): ViewPerm[] {
  const names: unknown[] = [];
  for (const v of vs) {
    if (Array.isArray(v)) names.push(...(v as readonly unknown[]));
    else if (v !== null && typeof v === 'object') names.push(...Object.entries(v).filter(([, x]) => x !== false && x !== null).map(([k]) => k));
  }
  return NAMES.filter((p) => names.includes(p));
}

/** The `allow=` of a view's frame for these permissions: '' for none. A permission with no frame feature (`write`, which the hub checks on each save) never reaches it. Pure. */
export function allowOf(perms: readonly ViewPerm[]): string {
  return VIEW_PERMS.filter((r) => perms.includes(r.perm))
    .flatMap((r) => (r.feature === null ? [] : [r.feature]))
    .join('; ');
}
