// The picker's rows for a trigger, and what picking one does. Pure: the plugin (Picker.tsx) asks
// for the rows when the trigger changes and applies the pick to the editor.
import { filterEmoji } from './emoji';
import { fileName, type Chip } from './message';
import { isImagePath, type ComposerSources, type PickAgent, type PickArtifact, type PickCommand, type PickFile, type PickPlugin, type PickSkill } from './sources';
import { agentPick, filePick, prefixFirst, skillPick, type Trigger } from './tokens';
import { composer } from '../proto/words.gen';

export interface Row {
  /** Stable per row, for React keys and the test id `composer.pick.<id>`. */
  readonly id: string;
  readonly label: string;
  readonly desc: string;
  /** A tag in front: `agent`, `dir`, `file`, `skill`, `cmd`, `emoji`. */
  readonly tag: 'agent' | 'plugin' | 'artifact' | 'dir' | 'file' | 'skill' | 'cmd' | 'emoji' | 'choice';
  /** Greyed, `not yet`: shown, picking it does nothing (bar 2.1). */
  readonly off: boolean;
  readonly pick: Pick;
  /** a value it has now: its ✓ (an argument's Choice.current) */
  readonly current?: boolean;
  /** its tip words instead of 'label · desc' */
  readonly tip?: string;
}

/** What a pick does: replace the trigger's word by `text` (and keep the list open on `browse`),
 *  or run the command now. */
export type Pick =
  | { readonly kind: 'insert'; readonly text: string; readonly browse: boolean; readonly path?: string }
  | { readonly kind: 'run'; readonly name: string; readonly args?: string }
  | { readonly kind: 'artifact'; readonly id: string; readonly title: string; readonly url: string }
  | { readonly kind: 'plugin'; readonly name: string };

/** At most this many rows in a list (the TUI's popup shows fewer; the rest is a query away). */
export const MAX_ROWS = 50;

export function agentRows(t: Trigger, agents: readonly PickAgent[]): Row[] {
  if (t.kind !== 'at' || t.quoted || t.query.includes('/')) return [];
  return prefixFirst(agents, t.query, (a) => a.name).map((a) => ({
    id: `agent.${a.name}`,
    label: `@${a.name}`,
    desc: a.desc ?? '',
    tag: 'agent',
    off: false,
    pick: { kind: 'insert', text: agentPick(a.name), browse: false },
  }));
}

/** The `@` list's plugins (at-plugins, the TUI's commands.rs plugin_items), after the agents: their
 *  own mark and the word 'plugin' first, so one never reads as an agent; never in a path query. A
 *  pick puts its `◆ name` chip where the `@word` was, sent as Part::Plugin. */
export function pluginRows(t: Trigger, plugins: readonly PickPlugin[]): Row[] {
  if (t.kind !== 'at' || t.quoted || t.query.includes('/')) return [];
  return prefixFirst(plugins, t.query, (p) => p.name).map((p) => ({
    id: `plugin.${p.name}`,
    label: `@${p.name}`,
    desc: p.desc === '' ? composer.at_plugin : `${composer.at_plugin} · ${p.desc}`,
    tag: 'plugin',
    off: false,
    pick: { kind: 'plugin', name: p.name },
  }));
}

/** At most this many artifacts in the `@` list (commands.rs ARTIFACT_ROWS). */
export const ARTIFACT_ROWS = 8;

/** `a few seconds ago`-style words for a time (artifacts.rs ago_words, short). */
export function agoWords(ms: number, now: number): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 60) return 'just now';
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  const d = Math.round(h / 24);
  return d === 1 ? 'yesterday' : `${d} days ago`;
}

/** The `@` list's artifacts (commands.rs): by title, agent, kind or id, never in a path query; a
 *  pick puts a `↗ title` chip in the message, never `@name`. */
export function artifactRows(t: Trigger, artifacts: readonly PickArtifact[], now: number): Row[] {
  if (t.kind !== 'at' || t.quoted || t.query.includes('/')) return [];
  const q = t.query.toLowerCase();
  return artifacts
    .filter((a) => q === '' || [a.title, a.agent, a.kind, a.id].some((f) => f.toLowerCase().includes(q)))
    .slice(0, ARTIFACT_ROWS)
    .map((a) => ({
      id: `artifact.${a.id}`,
      label: `↗ ${a.title}`,
      desc: `${a.kind} · ${a.agent} · ${agoWords(a.atMs, now)}`,
      tag: 'artifact',
      off: false,
      pick: { kind: 'artifact', id: a.id, title: a.title, url: a.url },
    }));
}

export function fileRows(files: readonly PickFile[]): Row[] {
  return files.map((f) => {
    const p = filePick(f.path, f.dir);
    return { id: `file.${f.path}`, label: f.dir && !f.path.endsWith('/') ? `${f.path}/` : f.path, desc: '', tag: f.dir ? 'dir' : 'file', off: false, pick: { kind: 'insert', text: p.text, browse: p.browse, path: f.path } };
  });
}

/** at-chips: the chip a file picked in the `@` list becomes, by its absolute path (`rel` from the
 *  workspace `root`, or `../` out of it; an absolute `rel` as it is): an image chip by its
 *  extension, else a file chip, code too (bise_proto::parts chip_kind, the TUI's pick). Its
 *  thumbnail, icon and size come later (pickedFacts). Pure. */
export function pickedChip(root: string, rel: string, n: number): Chip {
  const path = rel.startsWith('/') ? rel : normalize(`${root.replace(/\/+$/u, '')}/${rel}`);
  return isImagePath(path) ? { kind: 'image', n, path, name: fileName(path) } : { kind: 'file', n, path };
}

/** `/a/b/../c/./d` -> `/a/c/d`: a path the attachment list and the hub read plainly. */
function normalize(path: string): string {
  const out: string[] = [];
  for (const s of path.split('/')) {
    if (s === '..') out.pop();
    else if (s !== '.' && s !== '') out.push(s);
  }
  return `/${out.join('/')}`;
}

/** The emoji list for `:cl` (emoji.rs filter): `👏 :clap:`, picking puts the emoji in place. */
export function emojiRows(t: Trigger): Row[] {
  if (t.kind !== 'emoji') return [];
  return filterEmoji(t.query).map(({ emoji, name }) => ({
    id: `emoji.${name}`,
    label: `${emoji.glyph}  :${name}:`,
    desc: emoji.desc,
    tag: 'emoji',
    off: false,
    pick: { kind: 'insert', text: emoji.glyph, browse: false },
  }));
}

export function skillRows(t: Trigger, skills: readonly PickSkill[]): Row[] {
  if (t.kind !== 'skill') return [];
  return prefixFirst(skills, t.query, (s) => s.name).map((s) => ({
    id: `skill.${s.name}`,
    label: `$${s.name}`,
    desc: s.desc,
    tag: 'skill',
    off: false,
    pick: { kind: 'insert', text: skillPick(s.name), browse: false },
  }));
}

/** The `/` list: the commands whose name starts with the query (commands.rs `popup_matches`). */
export function commandRows(t: Trigger, commands: readonly PickCommand[]): Row[] {
  if (t.kind !== 'slash') return [];
  const q = t.query.toLowerCase();
  return commands
    .filter((c) => c.name.replace(/^\//u, '').toLowerCase().startsWith(q))
    .map((c) => {
      const name = c.name.startsWith('/') ? c.name : `/${c.name}`;
      return {
        id: `cmd.${name.slice(1)}`,
        label: name,
        desc: c.ready ? c.desc : composer.cmd_not_yet({ desc: c.desc }),
        tag: 'cmd',
        off: !c.ready,
        pick: c.args ? { kind: 'insert', text: `${name} `, browse: false } : { kind: 'run', name },
      };
    });
}

/** The values of the argument being typed (`/model op` -> the models starting with or holding
 *  `op`): fixed words filtered here, listed kinds from the screen's `choices`. */
export function argRows(t: Trigger, commands: readonly PickCommand[], sources: { readonly choices?: ComposerSources['choices'] }): Row[] {
  if (t.kind !== 'arg' || t.command === undefined || t.index === undefined) return [];
  const cmd = commands.find((c) => `/${c.name.replace(/^\//u, '')}` === t.command);
  const kind = cmd?.argKinds?.[t.index];
  if (kind === undefined || kind === 'text') return [];
  const list = typeof kind === 'object' ? prefixFirst(kind.words, t.query, (w) => w.value) : (sources.choices?.(kind, t.query) ?? []);
  return list.map((c) => ({
    id: `arg.${c.value}`,
    label: c.label ?? c.value,
    desc: c.desc,
    tag: c.label === undefined ? 'cmd' : 'choice',
    off: false,
    pick: c.runs === true && t.command !== undefined ? { kind: 'run', name: t.command, args: c.value } : { kind: 'insert', text: `${c.value} `, browse: false },
    ...(c.current === true ? { current: true } : {}),
    ...(c.tip === undefined ? {} : { tip: c.tip }),
  }));
}

/**
 * ⏎ with the list open on an argument whose word is already typed whole (`/close 1` and the row
 * `1`, `/archive perf` and the row `perf`): the line is a complete command, so ⏎ sends it instead
 * of inserting the same word again (qa-flows 28a5419b: '/close 1' never left the field). Every
 * argument kind; Tab still completes. Pure.
 */
export function enterSends(t: Trigger | null, row: Row | undefined): boolean {
  return t?.kind === 'arg' && row !== undefined && t.query !== '' && row.label === t.query;
}

/** Tab on a row: always completes (a command's name, then its arguments), never runs. */
export function completion(row: Row): Pick {
  if (row.pick.kind !== 'run') return row.pick;
  // an argument that runs at once (/bims kyoto): Tab writes its value, ⏎ or a click runs it
  return { kind: 'insert', text: `${row.pick.args ?? row.pick.name} `, browse: false };
}

/** The keyboard's row after a move: wraps at both ends, skips nothing (greyed rows are visible). */
export function move(index: number, by: number, length: number): number {
  if (length === 0) return 0;
  return (((index + by) % length) + length) % length;
}

/** A whole line `/name args` that names a known command: what ⏎ runs instead of sending. */
export function commandLine(text: string, commands: readonly PickCommand[]): { readonly name: string; readonly args: string; readonly ready: boolean } | null {
  const m = /^\/(\S+)(?:\s+([\s\S]*))?$/u.exec(text.trim());
  if (m === null) return null;
  const word = m[1] ?? '';
  const c = commands.find((x) => x.name.replace(/^\//u, '') === word);
  if (c === undefined) return null;
  return { name: `/${word}`, args: (m[2] ?? '').trim(), ready: c.ready };
}
