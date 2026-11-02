// Which object each of his plugins brings to the office (plan bims-plugins p3, page bims-plugins
// v16): the default one by the plugin's name and kind (bims-plugins m_40041; anything else: a
// plant whose pot is its kind's), or his own pick, kept per project in the prefs
// `bims.objects.<project>` = { <PluginId string>: { obj } } (architect m_40029: the prefs' one
// writer, next to bims.map and bims.grow; an object so p4's spot joins it without a shape change).
// Its look comes from the plugins event's typed fields only, never a status text: loaded = the
// object, disabled = glass, invalid or a failed server = lit with the '✗' pill, a login he
// owes = the '?' pill; a shadowed row brings nothing (the plugin that wins it does). In the office
// each object stands once (uniqueObjects, Gabriel m_42685, bims-plugins m_42693): a plugin whose
// object is taken gets its kin, or none; the plugins page's room keeps one per plugin.
// Pure, no React, no three: the office driver and settings › plugins read it. Owner: bims-link.
import type { JsonValue, Plugin } from '@/proto';
import { isObjId, type ObjId } from './render/objects/ids';

/**
 * The fields of a plugins row this module reads. Main's rows have no `id`, `kind`, `login` or
 * `parts` yet (the plugins branch adds them): each is optional, so both rows fit.
 */
export type PluginRow = Pick<Plugin, 'name' | 'state'> & {
  readonly id?: string | null;
  readonly kind?: string | null;
  readonly login?: string | null;
  /** its name as he reads it ('Gmail'); empty or absent: its name */
  readonly title?: string | null;
  readonly parts?: readonly { readonly kind: string; readonly on: boolean; readonly login?: string | null; readonly error?: string | null }[] | null;
};

/** What a plugin is, for its default object and its plant's pot; never its scope. */
export type PluginKind = 'plugin' | 'connector' | 'mcp' | 'skills' | 'bise';

/** One plugin's object in the office. */
export interface PluginObject {
  /** the plugin's id string (pluginKey): the prefs' key and the placement's seed */
  readonly key: string;
  /** the plugin's name as he reads it (its title, else its name): the object's hover tip */
  readonly name: string;
  readonly obj: ObjId;
  /** the object's count (makeObject's n): books on a shelf, lights on a rack, the plant's pot */
  readonly n?: number;
  /** true: drawn in glass (off, or broken) */
  readonly glass: boolean;
  /** '✗' it is broken, '?' it waits on his login; null: none */
  readonly pill: '✗' | '?' | null;
  /** true: his pick, not the default */
  readonly picked: boolean;
  /** the plugin its pill is for when not its own (uniqueObjects: a plugin not drawn whose object this is, its state worse); its click opens that one */
  readonly pillOf?: string;
  /** the name of the plugin its pill is for, as he reads it (the pill's tip: 'slides failed to start'); absent: no pill */
  readonly pillName?: string;
}

/** His picks for one project: plugin key → object. */
export type Picks = ReadonlyMap<string, ObjId>;

/** The prefs key of a project's picks. */
export const objectsPrefKey = (project: string): string => `bims.objects.${project}`;

/** A row's PluginId string: its `id` when the row has one, else its name (a real plugin's id is its bare name). */
export function pluginKey(row: Pick<PluginRow, 'id' | 'name'>): string {
  return row.id ?? row.name;
}

/** What the row is: its `kind` when the row says, else read from its id's prefix. */
export function kindOf(row: Pick<PluginRow, 'id' | 'name' | 'kind'>): PluginKind {
  const key = pluginKey(row);
  if (key === 'bise') return 'bise';
  switch (row.kind) {
    case 'mistral':
      return 'connector';
    case 'mcp':
      return 'mcp';
    case 'skills':
      return 'skills';
    case 'plugin':
      return 'plugin';
    default:
      return key.startsWith('mistral:') ? 'connector' : key.startsWith('skills:') ? 'skills' : 'plugin';
  }
}

/** The plant's pot by kind (makeObject's n for a plant, signed with designer): 1 amber, 2 sky, 3 violet, else teal. */
const POT: Readonly<Record<PluginKind, number>> = { plugin: 0, connector: 1, mcp: 2, skills: 3, bise: 0 };

/** The name the table matches: a connector's own name, lowercased, '_' and '-' as spaces. */
function nameOf(key: string): string {
  const bare = key.startsWith('mistral:') ? key.slice('mistral:'.length) : key;
  let name = bare;
  try {
    name = decodeURIComponent(bare);
  } catch {
    // a lone '%': the name as written
  }
  return name.toLowerCase().replace(/[_-]+/g, ' ').trim();
}

/** The default object by name (bims-plugins m_40041); null: none by name. */
const BY_NAME: readonly (readonly [RegExp, ObjId, number?])[] = [
  [/^notion$/, 'library'],
  [/^(google )?drive( mcp)?$/, 'files'],
  [/^granola$/, 'visio'],
  [/^slack$/, 'booth'],
  [/^(hn|hacker news)$/, 'arcade'],
  [/^reddit$/, 'pingpong'],
  [/^kit$/, 'scooter'],
  [/^gmail$/, 'mailbox'],
  // figma: where you draw (designer m_40518)
  [/^(blender|image generation|figma)$/, 'easel'],
  [/^(computer|computer use)$/, 'macdesk'],
  [/^docs$/, 'printer', 1],
  [/^sheets$/, 'printer', 2],
  [/^slides$/, 'printer', 3],
  [/calendar/, 'calendar'],
  [/^(web search|tavily)$/, 'telescope'],
  [/^bise stats$/, 'trophies'],
  [/^github( app)?$/, 'packing'],
  [/^sentry/, 'extinguisher'],
  // the board of issues; Jira's and Confluence's charts; Datadog's dials (designer m_40880)
  [/^linear$/, 'blackboard'],
  [/^(atlassian|jira|confluence)/, 'charts'],
  [/^datadog/, 'dashboard'],
  [/^(metabase|bigquery|wandb)$/, 'charts'],
];

/** The plugin's own object when he never picked one, with its count. */
export function defaultObject(row: PluginRow): { readonly obj: ObjId; readonly n?: number } {
  const kind = kindOf(row);
  if (kind === 'bise') return { obj: 'cat' };
  if (kind === 'skills') return { obj: 'shelf', n: count(row, 'skill', false) };
  const name = nameOf(pluginKey(row));
  for (const [re, obj, n] of BY_NAME) if (re.test(name)) return n === undefined ? { obj } : { obj, n };
  if (kind === 'mcp') return { obj: 'rack', n: count(row, 'mcp', true) };
  return { obj: 'plant', n: POT[kind] };
}

/** How many parts of that kind the row has (only the ones on), at least 1; absent parts: 1. */
function count(row: PluginRow, kind: string, onOnly: boolean): number {
  const n = (row.parts ?? []).filter((p) => p.kind === kind && (!onOnly || p.on)).length;
  return Math.max(1, n);
}

/** A login he owes: the row's own (a connector) or a part's (a remote server's). */
function owesLogin(row: PluginRow): boolean {
  return row.login === 'needs' || (row.parts ?? []).some((p) => p.on && p.login === 'needs');
}

/** A server of it that failed (a part with an error: its presence marks it, never its text). */
function failed(row: PluginRow): boolean {
  return (row.parts ?? []).some((p) => p.on && p.error != null);
}

/** His picks from the prefs value at `bims.objects.<project>`: a value that is no object or no known ObjId is skipped. */
export function picksOf(v: JsonValue | undefined): Picks {
  const out = new Map<string, ObjId>();
  if (v === null || v === undefined || typeof v !== 'object' || Array.isArray(v)) return out;
  for (const [key, e] of Object.entries(v)) {
    if (e === null || typeof e !== 'object' || Array.isArray(e)) continue;
    const obj = e.obj;
    if (typeof obj === 'string' && isObjId(obj)) out.set(key, obj);
  }
  return out;
}

/**
 * The prefs value with `key`'s pick set (obj null: back to its default). Every other entry stays
 * as it was, a stale key (a plugin gone for now) and an entry's other fields (p4's spot) too.
 */
export function withPick(v: JsonValue | undefined, key: string, obj: ObjId | null): JsonValue {
  const was = v !== null && v !== undefined && typeof v === 'object' && !Array.isArray(v) ? v : {};
  const old = was[key];
  const rest = old !== null && old !== undefined && typeof old === 'object' && !Array.isArray(old) ? old : {};
  const keep = Object.fromEntries(Object.entries(rest).filter(([k]) => k !== 'obj'));
  const others = Object.fromEntries(Object.entries(was).filter(([k]) => k !== key));
  if (obj !== null) return { ...others, [key]: { ...keep, obj } };
  return Object.keys(keep).length > 0 ? { ...others, [key]: keep } : others;
}

/** One plugin's object, its look and its pill; null: it brings none (shadowed, or a state this app does not know). */
export function objectOf(row: PluginRow, picks: Picks): PluginObject | null {
  const broken = row.state === 'invalid' || failed(row);
  const on = row.state === 'loaded';
  // shadowed (another row wins its name), available (a connector he never added), unknown: nothing
  if (!on && !broken && row.state !== 'disabled') return null;
  const key = pluginKey(row);
  const pick = picks.get(key);
  const dflt = defaultObject(row);
  // his pick keeps the default's count only when it is the same object (the plant's pot, the shelf's books)
  const n = pick === undefined || pick === dflt.obj ? dflt.n : undefined;
  const pill = broken ? '✗' : owesLogin(row) ? '?' : null;
  const name = row.title !== undefined && row.title !== null && row.title !== '' ? row.title : row.name;
  return {
    key,
    name,
    obj: pick ?? dflt.obj,
    ...(n === undefined ? {} : { n }),
    // broken is not off (designer m_41417): a broken plugin's object stays lit, its '✗' says it failed; glass is for off
    glass: !on && !broken,
    pill,
    picked: pick !== undefined,
    ...(pill === null ? {} : { pillName: name }),
  };
}

/**
 * What may stand at most once in an office (bims-plugins m_42693): the object, and for a plant its
 * pot (a plant per kind: makeObject's n, 1 to 3, anything else the teal one).
 */
export function unitOf(o: { readonly obj: ObjId; readonly n?: number }): string {
  if (o.obj !== 'plant') return o.obj;
  const pot = o.n === 1 || o.n === 2 || o.n === 3 ? o.n : 0;
  return `plant:${String(pot)}`;
}

/**
 * A taken object's kin, in order (bims-plugins m_42693): the same job at a glance, in the same
 * corner of corners.ts (so it stands where its own would). An object with none: the plugin that
 * loses it is not drawn.
 */
const KIN: Partial<Record<ObjId, readonly ObjId[]>> = {
  printer: ['press'],
  charts: ['dashboard'],
  dashboard: ['charts'],
  library: ['shelf'],
  shelf: ['library'],
  rack: ['crt'],
  booth: ['rotary'],
  visio: ['crt'],
};

/** who keeps a contested object (bims-plugins m_42693): his pick, then the worst state (designer m_43133: '✗' over '?'), then lit, then glass */
const rankOf = (o: PluginObject): number => (o.picked ? 0 : o.pill === '✗' ? 1 : o.pill === '?' ? 2 : !o.glass ? 3 : 4);
const PILL_RANK = { '✗': 2, '?': 1 } as const;
/** the worse of two pills: '✗' over '?' over none */
const worse = (a: PluginObject['pill'], b: PluginObject['pill']): boolean => (b === null ? 0 : PILL_RANK[b]) > (a === null ? 0 : PILL_RANK[a]);
const byKey = (a: PluginObject, b: PluginObject): number => (a.key < b.key ? -1 : a.key > b.key ? 1 : 0);

/**
 * The office's objects, each at most once (Gabriel m_42685: three printers clutter it; not every
 * plugin needs one). Each plugin keeps its own object while no one before it (rankOf, then the
 * key) has it; the ones that lost it then take their object's first free kin (its own look, no
 * count), else they are not drawn. A plant is one per pot (unitOf). By key, like officeObjects. Pure.
 */
export function uniqueObjects(objs: readonly PluginObject[]): readonly PluginObject[] {
  const taken = new Set<string>();
  const out: PluginObject[] = [];
  const lost: PluginObject[] = [];
  for (const o of [...objs].sort((a, b) => rankOf(a) - rankOf(b) || byKey(a, b))) {
    const u = unitOf(o);
    if (taken.has(u)) lost.push(o);
    else {
      taken.add(u);
      out.push(o);
    }
  }
  // a kin only once every plugin had its own: a dashboard plugin keeps the dashboard a third charts one would take
  for (const o of lost) {
    const kin = (KIN[o.obj] ?? []).find((k) => !taken.has(k));
    if (kin === undefined) {
      // not drawn: its object carries the worst state of the plugins it stands for (designer m_43133)
      const at = out.findIndex((h) => unitOf(h) === unitOf(o));
      const h = out[at];
      if (h !== undefined && worse(h.pill, o.pill)) out[at] = { ...h, pill: o.pill, pillOf: o.key, ...(o.pillName === undefined ? {} : { pillName: o.pillName }) };
      continue;
    }
    taken.add(kin);
    out.push({ key: o.key, name: o.name, obj: kin, glass: o.glass, pill: o.pill, picked: o.picked, ...(o.pillName === undefined ? {} : { pillName: o.pillName }) });
  }
  return out.sort(byKey);
}

/** Every plugin's object, in a stable order (by key), one per key (the first row wins). */
export function officeObjects(rows: readonly PluginRow[] | null, picks: Picks): readonly PluginObject[] {
  const seen = new Set<string>();
  const out: PluginObject[] = [];
  for (const row of rows ?? []) {
    const o = objectOf(row, picks);
    if (o === null || seen.has(o.key)) continue;
    seen.add(o.key);
    out.push(o);
  }
  return out.sort((a, b) => (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
}
