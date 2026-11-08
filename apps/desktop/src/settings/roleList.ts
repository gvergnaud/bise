// Which model does what (page models-settings, Gabriel's pick A): the words of settings · Setup's
// one list, from the core's setup_gauge (each feature, its models and their last test) and roles
// (each role's effort and where its model comes from). One row per feature, its model by name as a
// button when the row owns that role, one status; Voice mode's models as hears / thinks / speaks.
// The core decides what works, what failed and what fixes it (bise_catalog::features); this file
// only words it. Pure. Owner: setup-page.
import type { FeatureId, FeaturePart, FeatureRow, Model, RoleId, RoleRow, SetupFix, SetupGauge } from '@/proto';
import type { PickerGroup } from '@/ds';
import { settings } from '@/proto/words.gen';
import { FEATURE_WORDS, PERM_NAME, keyFailed, providerName, secsOf, tookMs, whyWords, type Tone } from './gauge';
import { modelRow } from './hold';

/** A row's name: bise's own row is bise (designer m_43851), the others the feature's. */
export const ROW_NAME: Record<FeatureId, string> = { ...FEATURE_WORDS, talk: settings.role_main_name };

/** A row's plain line: what it does. */
export const ABOUT: Record<FeatureId, string> = {
  talk: settings.about_talk,
  agents: settings.about_agents,
  small: settings.about_small,
  vision: settings.about_vision,
  dictation: settings.about_dictation,
  voice_mode: settings.about_voice_mode,
  approvals: settings.about_approvals,
  unknown: '',
};

/** A row in the fix line's sentence ("Titles, Dictation and Voice mode stopped"). */
const SHORT: Record<FeatureId, string> = {
  talk: settings.short_talk,
  agents: settings.short_agents,
  small: settings.short_small,
  vision: settings.short_vision,
  dictation: settings.short_dictation,
  voice_mode: settings.short_voice_mode,
  approvals: settings.short_approvals,
  unknown: '',
};

/** Voice mode's sub-rows, by the part's role. */
const PART_NAME: Partial<Record<RoleId, string>> = { voice: settings.part_hears, main: settings.part_thinks, tts: settings.part_speaks };

/** A sub-row's name ('hears'), none for a role it doesn't name. */
export const partName = (p: FeaturePart): string => PART_NAME[p.role] ?? p.role;

/** Voice mode's parts in the order he says them: hears, thinks, speaks. Pure. */
export function partsInOrder(r: FeatureRow): FeaturePart[] {
  const at = (p: FeaturePart) => ['voice', 'main', 'tts'].indexOf(p.role);
  return [...r.parts].sort((a, b) => at(a) - at(b));
}

/** The roles role_set changes from this list (the voices have their own settings). */
const PICKED: ReadonlySet<RoleId> = new Set<RoleId>(['main', 'agents', 'small', 'classify']);

/** "a, b and c". Pure. */
export function joinAnd(items: readonly string[]): string {
  if (items.length < 2) return items.join('');
  return `${items.slice(0, -1).join(', ')} ${settings.list_and} ${items.at(-1) ?? ''}`;
}

/** A row's model as its cell shows it. */
export interface ModelView {
  /** its name for people ('opus 5.5'), else its id */
  name: string;
  /** after the name, faint: its provider, or why it isn't a button ('same as bise', 'set by X') */
  faint: string;
  /** its effort, when not its model's default */
  effort: string | null;
  /** the role a pick sets (role_set); null: not a button */
  role: RoleId | null;
  tip: string;
}

/** A part's model in row `r`: a button on the row that owns its role; bise's model elsewhere
 * (images, voice mode's thinks) says so and points to bise's row; an env var's names it with the
 * step that gives it back (models-ux m_57191). Pure. */
export function modelView(r: FeatureRow, p: FeaturePart, roles: readonly RoleRow[], models: readonly Model[]): ModelView {
  // the core names every model; none: no model (the checker off, no voice)
  const name = p.name !== undefined && p.name !== '' ? p.name : settings.part_none;
  const role = roles.find((x) => x.role === p.role);
  const row = modelRow(p.model, models);
  const effort = role?.effort != null && role.effort !== '' && role.effort !== row?.default_effort ? role.effort : null;
  const provider = p.provider ?? '';
  if (p.source === 'env' && p.var != null) return { name, faint: settings.part_set_by({ var: p.var }), effort, role: null, tip: settings.tip_set_by({ var: p.var }) };
  if (p.role === 'main' && r.id !== 'talk') return { name, faint: settings.part_same_as_bise, effort: null, role: null, tip: settings.tip_same_as_bise };
  const pick = PICKED.has(p.role) && role?.kind === 'chat' ? p.role : null;
  return { name, faint: provider, effort, role: pick, tip: p.model };
}

/** A status: its words, its tone, its tip. */
export interface Status {
  text: string;
  tone: Tone;
  tip: string;
}

/** Why a part stopped, short, after ✗. Pure. */
export function stopWords(p: FeaturePart): string {
  switch (p.why) {
    case 'no_key':
      return settings.stop_no_key;
    case 'wrong_key':
      return settings.stop_wrong_key;
    case 'no_credit':
      return settings.stop_no_credit;
    case 'model':
      return p.model === '' ? settings.stop_no_model : settings.stop_model;
    case 'no_access':
      return settings.stop_no_access;
    case 'config':
      return settings.stop_config;
    case 'unreachable':
      return settings.stop_unreachable;
    default:
      return settings.mark_failed;
  }
}

const testing = (g: SetupGauge, p: FeaturePart): boolean => p.model !== '' && g.testing?.includes(p.model) === true;

/** One part's status (Voice mode's sub-rows): ✓ and its check's time, ✗ and why, ○ off. Pure. */
export function partStatus(p: FeaturePart, g: SetupGauge): Status {
  if (testing(g, p)) return { text: settings.gauge_testing, tone: 'faint', tip: settings.gauge_test_tip };
  if (p.state === 'failed') return { text: stopWords(p), tone: 'err', tip: p.said ?? stopWords(p) };
  if (p.state !== 'on') return { text: settings.mark_off, tone: 'faint', tip: settings.tip_off };
  if (p.ms == null) return { text: settings.mark_on, tone: 'ink', tip: settings.tip_untested };
  return { text: settings.mark_took({ secs: secsOf(p.ms) }), tone: 'ink', tip: settings.tip_took({ secs: secsOf(p.ms) }) };
}

/** A row's status: its model's (one part), else only what its permissions miss (Voice mode's head:
 * its parts say the rest). A permission shows only when it's missing. Pure. */
export function rowStatus(r: FeatureRow, g: SetupGauge): Status | null {
  const denied = r.perms?.find((x) => x.grant === 'denied');
  const ask = r.perms?.find((x) => x.grant === 'ask');
  const partsOk = r.parts.every((p) => p.state === 'on');
  if (r.home_gone != null) return { text: settings.stop_home_gone, tone: 'err', tip: whyWords(r, g) };
  if (partsOk && denied !== undefined) return { text: settings.stop_perm({ perm: PERM_NAME[denied.perm] }), tone: 'err', tip: whyWords(r, g) };
  if (partsOk && ask !== undefined && r.state === 'off') return { text: settings.off_perm({ perm: PERM_NAME[ask.perm] }), tone: 'faint', tip: settings.tip_off };
  if (r.parts.length !== 1) return null;
  const p = r.parts[0] as FeaturePart;
  const s = partStatus(p, g);
  // off because bise's model doesn't read images: the tip says so
  if (r.id === 'vision' && r.state === 'off' && p.state === 'on') return { text: settings.mark_off, tone: 'faint', tip: settings.tip_no_images({ model: p.name ?? p.model }) };
  if (s.tone !== 'ink') return s;
  const better = r.better != null && r.better.t === 'add_key' ? settings.tip_better({ provider: providerName(g, r.better.provider) }) : null;
  const ms = tookMs(r);
  return { ...s, tip: better ?? (ms === null ? settings.tip_untested : settings.tip_took({ secs: secsOf(ms) })) };
}

/** Two fixes are the same act. Pure. */
export const sameFix = (a: SetupFix, b: SetupFix): boolean => JSON.stringify(a) === JSON.stringify(b);

/** The fix line on top: only when something stopped, the core's Next, and the rows it brings back
 * ('your Mistral key expired: Titles, Dictation and Voice mode stopped.'). Pure. */
export function fixLine(g: SetupGauge): { text: string; fix: SetupFix } | null {
  const n = g.next;
  if (n == null) return null;
  const stopped = n.unlocks.filter((id) => g.rows.find((r) => r.id === id)?.state === 'failed');
  if (stopped.length === 0) return null;
  const what = joinAnd(stopped.map((id) => SHORT[id]));
  const f = n.fix;
  const text = f.t === 'add_key' && keyFailed(g, f.provider) ? settings.fixline_key({ provider: providerName(g, f.provider), what }) : settings.fixline_stopped({ what });
  return { text, fix: f };
}

/** A row's own fix button: its fix, unless the fix line on top is the same act, or the fix is
 * another model (the row's model button is that act). Pure. */
export function ownFix(r: FeatureRow, line: { fix: SetupFix } | null): SetupFix | null {
  if (r.fix == null || r.fix.t === 'pick_model') return null;
  return line !== null && sameFix(r.fix, line.fix) ? null : r.fix;
}

/** the picker's id for "back to its fallback" (role_set with no model): a word no model id can be */
export const FALLBACK = '(fallback)';

/** The picker of a role: its fallback first (the model it resolves to), then the models under
 * their provider; a model whose provider isn't set up yet says so (picking it opens that setup:
 * hold.ts). Pure. */
export function pickGroups(role: RoleRow, roles: readonly RoleRow[], models: readonly Model[]): PickerGroup[] {
  const follows = role.follows != null && role.follows !== '' ? roles.find((x) => x.role === role.follows) : undefined;
  const fallbackModel = follows !== undefined ? follows.model : role.source === 'default' ? role.model : '';
  const fallbackName = modelRow(fallbackModel, models)?.label ?? fallbackModel;
  const fallback = {
    id: FALLBACK,
    name: follows !== undefined ? settings.role_as({ role: follows.role === 'main' ? settings.role_main_name : follows.name }) : settings.role_default,
    ...(fallbackName !== '' ? { secondary: fallbackName } : {}),
    current: role.source === 'default',
  };
  const byProvider = new Map<string, Model[]>();
  for (const m of models) {
    if (m.alias_of != null && m.alias_of !== '') continue;
    byProvider.set(m.provider, [...(byProvider.get(m.provider) ?? []), m]);
  }
  return [
    { id: 'fallback', rows: [fallback] },
    ...[...byProvider].map(([provider, ms]) => ({
      id: provider,
      head: provider,
      rows: ms.map((m) => ({
        id: m.id,
        name: m.label,
        current: role.source !== 'default' && role.model === m.id,
        tip: m.id,
        ...(m.key_ready === false ? { trailing: settings.pick_setup_first } : {}),
      })),
    })),
  ];
}
