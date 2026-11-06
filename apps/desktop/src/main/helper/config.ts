// The helper's config from his prefs (pure): the core's {ev:'prefs'} object
// -> {cmd:'config'}. excluded_apps is bise_home's (the seed already resolved
// by the core when unset); right option as the fn fallback has no pref yet:
// its default (on) until it does. context.quote off: the helper never reads
// his selection (fn space's ❝, architect m_53938).
import type { HelperCmd } from '../../proto';
import { quoteOn } from '../../shared/quote';

export type HelperConfig = Extract<HelperCmd, { cmd: 'config' }>;

export const DEFAULT_CONFIG: HelperConfig = { cmd: 'config', exclude: [], right_option: true, quote: true };

function obj(v: unknown): Record<string, unknown> | null {
  return typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
}

export function configOfPrefs(prefs: unknown): HelperConfig {
  const p = obj(prefs);
  if (p === null) return DEFAULT_CONFIG;
  const apps = p['excluded_apps'];
  const exclude = Array.isArray(apps) ? apps.filter((a): a is string => typeof a === 'string') : [];
  const ro = p['right_option'];
  return {
    cmd: 'config',
    exclude,
    right_option: typeof ro === 'boolean' ? ro : DEFAULT_CONFIG.right_option,
    quote: quoteOn(p),
  };
}
