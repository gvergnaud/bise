// A provider's live check (provider_use, answered by provider_check; the core's firstkey.rs, the
// TUI's key step): where a key field is, and the catalog's words for a check that failed or works
// (bise_proto words::settings first_*, the TUI's own). Pure; settings · models and the first
// launch's model step both read it. Owner: amb-home.
import type { CoreEv, KeyFrom } from '@/proto';
import { settings } from '@/proto/words.gen';

export type ProviderCheck = Extract<CoreEv, { ev: 'provider_check' }>;

/** The article before a provider's name, by its sound: "an OpenRouter", "an xAI", "a Mistral",
 * "a Unify". Pure. */
export function article(name: string): 'a' | 'an' {
  if (/^x[A-Z]/.test(name)) return 'an'; // xAI: "ex-A-I"
  if (/^(uni|use|one)/i.test(name)) return 'a';
  return /^[aeiou]/i.test(name) ? 'an' : 'a';
}

/** What he sent last (the provider, and the check the store held at that time: older ones don't count). */
export interface Sent {
  id: string;
  base: ProviderCheck | null;
}

/** A check that failed for want of an answer: retrying the same thing makes sense ('try again'). */
export function retries(c: ProviderCheck): boolean {
  return c.why === 'unreachable' || c.why === 'config';
}

/** The key field after ⏎: checking until this send's answer, then refused or added. */
export function keyField(sent: Sent | null, check: ProviderCheck | null): 'typing' | 'checking' | 'refused' | 'added' {
  if (sent === null) return 'typing';
  if (check === null || check === sent.base || check.id !== sent.id) return 'checking';
  if (check.state === 'works') return 'added';
  if (check.state === 'failed') return 'refused';
  return 'checking';
}

/** A model's id without its provider ("anthropic/claude-opus-5-5" -> "claude-opus-5-5"). */
export function shortModel(m: string): string {
  const i = m.indexOf('/');
  return i < 0 ? m : m.slice(i + 1);
}

/** Where a found key is, as the TUI says it: "ANTHROPIC_API_KEY", "auth.json", "~/.vibe/.env". */
export function fromWords(f: KeyFrom): string {
  switch (f.t) {
    case 'env':
      return f.var;
    case 'auth_file':
      return 'auth.json';
    case 'env_file':
      return f.path;
    default:
      return '';
  }
}

/** A failed check, in words: its line, the provider's own words, and the fix with its page. */
export interface Failed {
  line: string;
  said: string | null;
  fix: { text: string; url: string | null } | null;
}

/** The TUI's failed screen's words (onboarding.rs Sub::Failed), for a pasted key, a found one (`from`) or a plan. */
export function failedWords(c: ProviderCheck, from: string | null, plan: boolean): Failed {
  const model = c.label != null && c.label !== '' ? c.label : shortModel(c.model ?? '');
  const theKey = from !== null && from !== '' ? settings.first_the_key_in({ var: from }) : settings.first_the_key;
  const hasSaid = c.said != null && c.said !== '' && c.why !== 'unreachable' && c.why !== 'config';
  const said = hasSaid ? settings.onb_said({ provider: c.name, said: c.said ?? '' }) : null;
  const url = c.url ?? null;
  const reason = (c.said ?? '').replace(/\.$/, '');
  switch (c.why) {
    case 'wrong_key':
      if (plan) return { line: settings.signin_expired, said, fix: null };
      return from !== null && from !== ''
        ? { line: settings.first_found_wrong({ key: theKey, provider: c.name }), said, fix: { text: settings.first_paste_another_get, url } }
        : hasSaid
          ? { line: settings.onb_key_wrong({ provider: c.name, said: c.said ?? '' }), said: null, fix: url !== null ? { text: settings.first_copy_again, url } : null }
          : { line: settings.first_key_wrong({ provider: c.name }), said: null, fix: url !== null ? { text: settings.first_copy_again, url } : null };
    case 'no_credit':
      return { line: settings.first_no_credit({ key: theKey, provider: c.name }), said, fix: { text: settings.first_add_here, url } };
    case 'model':
      return { line: plan ? settings.first_plan_model({ model }) : settings.first_key_model({ provider: c.name, model }), said, fix: null };
    case 'no_access':
      return { line: settings.first_no_access({ model }), said, fix: { text: settings.first_no_access_note, url: null } };
    case 'unreachable':
      return { line: settings.first_unreachable({ provider: c.name, why: reason }), said: null, fix: { text: settings.first_check_network, url: null } };
    case 'no_key':
      return { line: settings.first_paste_key({ provider: c.name }), said: null, fix: url !== null ? { text: settings.first_get_one, url } : null };
    default:
      return { line: reason !== '' ? reason : settings.key_refused, said: null, fix: null };
  }
}
