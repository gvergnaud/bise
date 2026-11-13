// Which app opens what, as File types shows it (contract docs/plugin-apps.md A3, A6b; designer's
// page plugin-apps-mocks block 5): the Plugins place's third group (Installed · Browse · File
// types), one row per type the hub knows (the plugins event's file_types, A2's picks): the type
// by its ending ('.xlsx') or its title for a folder rule ('Website'), its app as a button that
// opens the candidates like the model picker ('Sheets ▾'; none: 'your Mac'), a faint status
// ('default', 'your pick' with Reset, 'no app'). A pick sends opener_set {type, plugin, tool};
// Reset sends it without an opener. Pure: the group draws it. Owner: apps-gallery.
import { common } from '@/proto/words_common.gen';
import { settings } from '@/proto/words.gen';

/** An app that opens a type (bise_proto `Opener`, A3): what File types reads of it. */
export interface TypeOpener {
  readonly plugin: string;
  readonly tool: string;
  readonly app: string;
}

/** One type the hub knows (bise_proto `FileType`, A3). */
export interface TypeRow {
  // the canonical key: '*.xlsx', a folder rule's '*/index.html'
  readonly type: string;
  /** the entry's title, 'Excel workbook' */
  readonly title: string;
  /** the app A2 picks; none: his Mac */
  readonly app?: TypeOpener | null;
  /** the other candidates, in A2's order */
  readonly others: readonly TypeOpener[];
  /** the app is his pick */
  readonly his?: boolean | null;
}

/** One row of File types as it reads. */
export interface TypeLine {
  readonly type: string;
  /** '.xlsx'; a folder rule or a scoped type: its title */
  readonly name: string;
  /** the name is an ending (mono); a title is a word (the UI font: designer m_60513) */
  readonly ending: boolean;
  /** the button's word: the app, else 'your Mac' */
  readonly app: string;
  /** 'default', 'your pick', 'no app' */
  readonly status: string;
  /** Reset shows (his pick) */
  readonly reset: boolean;
  /** the button's menu: every candidate, the current one marked; none: no menu (one app, or none) */
  readonly picks: readonly TypePick[];
}

/** One candidate of a type's menu. */
export interface TypePick {
  /** '<plugin>#<tool>' */
  readonly id: string;
  readonly app: string;
  readonly current: boolean;
}

/** An opener's id in a menu: its plugin and tool. Pure. */
export const pickId = (o: TypeOpener): string => `${o.plugin}#${o.tool}`;

/** A type's name: its ending for a plain ending key ('*.board.json' → '.board.json'), else its title. Pure. */
export function typeName(type: string, title: string): string {
  const m = /^\*(\.[^*?/]+)$/u.exec(type);
  return m === null ? title : (m[1] ?? title);
}

/** A row of File types. Pure. */
export function typeLine(r: TypeRow): TypeLine {
  const app = r.app ?? null;
  const his = r.his === true && app !== null;
  const all = app === null ? r.others : [app, ...r.others];
  const name = typeName(r.type, r.title);
  return {
    type: r.type,
    name,
    ending: name !== r.title,
    app: app === null ? common.your_mac : app.app,
    status: app === null ? settings.file_type_none : his ? settings.file_type_his : settings.file_type_default,
    reset: his,
    picks: all.length < 2 ? [] : all.map((o) => ({ id: pickId(o), app: o.app, current: app !== null && pickId(o) === pickId(app) })),
  };
}

/** The rows, by name (endings first, then titles). Pure. */
export function typeLines(rows: readonly TypeRow[]): TypeLine[] {
  return rows.map(typeLine).sort((x, y) => Number(!x.name.startsWith('.')) - Number(!y.name.startsWith('.')) || x.name.localeCompare(y.name));
}

/**
 * The types the Plugins list's search finds (any case): by the name he reads ('.xlsx',
 * 'Website'), the title ('Excel workbook') or the app that opens it now ('Sheets'; none: 'your
 * Mac'). No words: every type. Pure.
 */
export function typesMatching<R extends TypeRow>(rows: readonly R[], q: string): R[] {
  const s = q.trim().toLowerCase();
  if (s === '') return [...rows];
  return rows.filter((r) => [typeName(r.type, r.title), r.title, r.app?.app ?? common.your_mac].some((w) => w.toLowerCase().includes(s)));
}

/** The command a pick sends (AppCmd opener_set); `id` none: Reset. Pure. */
export function openerSet(type: string, id?: string): { readonly cmd: 'opener_set'; readonly type: string; readonly plugin?: string; readonly tool?: string } {
  if (id === undefined) return { cmd: 'opener_set', type };
  const at = id.indexOf('#');
  return { cmd: 'opener_set', type, plugin: id.slice(0, at), tool: id.slice(at + 1) };
}

/** One type a plugin's app opens (app_rows::Plugin `opens`, A3). */
export interface PluginType {
  readonly type: string;
  readonly title: string;
}

/**
 * A plugin page's 'opens': each type by its name and ending ('Excel workbook (.xlsx)'), and the
 * ones another app opens now said after ('.csv: Grid opens them', ', your pick' when he chose
 * it): designer's mocks block 5. `plugin`: this page's; `types`: File types' rows. Pure.
 */
export function opensWords(plugin: string, opens: readonly PluginType[], types: readonly TypeRow[]): { readonly list: string; readonly elsewhere: readonly string[] } {
  const byType = new Map(types.map((t) => [t.type, t]));
  const list = opens.map((o) => {
    const name = typeName(o.type, o.title);
    return name === o.title ? o.title : settings.plugins_opens_type({ title: o.title, ending: name });
  });
  const elsewhere = opens.flatMap((o) => {
    const t = byType.get(o.type);
    const app = t?.app ?? null;
    if (app === null || app.plugin === plugin) return [];
    const said = settings.plugins_opens_elsewhere({ name: typeName(o.type, o.title), app: app.app });
    return [t?.his === true ? `${said}, ${settings.file_type_his}` : said];
  });
  return { list: list.join(' · '), elsewhere };
}

/** One type a plugin being added would open (bise_proto `OpenClaim`, A3). */
export interface Claim {
  readonly type: string;
  readonly title: string;
  /** the new plugin's app, 'Tables', and its opener tool */
  readonly app: string;
  readonly tool: string;
  /** the app that opens it today, 'Sheets'; none: no one does */
  readonly now?: string | null;
  /** a bare ending many apps' files share ('*.json'): said in plain words */
  readonly wide?: boolean | null;
}

/**
 * The add sheet's opens (designer's mocks block 6): every type it would open, a wide claim said
 * in plain words ('every .json file', no warning colour), then one line per type another app opens
 * today ('.xlsx opens in Sheets now · Keep Sheets / Use Tables', Keep preselected). Pure.
 */
export function claimsView(claims: readonly Claim[]): { readonly opens: string; readonly taken: readonly { readonly type: string; readonly line: string; readonly keep: string; readonly use: string }[] } {
  const opens = claims.map((c) => {
    const name = typeName(c.type, c.title);
    return c.wide === true && name.startsWith('.') ? settings.plugins_claim_wide({ ending: name }) : name;
  });
  const taken = claims.flatMap((c) => {
    const now = c.now ?? null;
    if (now === null) return [];
    return [{ type: c.type, line: settings.plugins_claim_now({ ending: typeName(c.type, c.title), app: now }), keep: settings.plugins_claim_keep({ app: now }), use: settings.plugins_claim_use({ app: c.app }) }];
  });
  return { opens: opens.join(' · '), taken };
}

/** What the add sends for the types he gave to the new app ('Use Tables'), after its confirm. Pure. */
export function claimPicks(plugin: string, claims: readonly Claim[], used: ReadonlySet<string>): ReturnType<typeof openerSet>[] {
  return claims.filter((c) => (c.now ?? null) !== null && used.has(c.type)).map((c) => openerSet(c.type, `${plugin}#${c.tool}`));
}
