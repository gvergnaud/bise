// The first launch's model step (the TUI's key step, onboarding.rs: "how do you want to pay for
// the models?"): the keys found first, then a plan he has (ChatGPT), OpenRouter (sign in or a
// key), an API key (which provider), a coding plan key; then the key pasted or the sign-in, its
// live check (provider_use -> provider_check, the core's firstkey.rs) and "it works" with the
// model's own first words. Pure: the rows, where the step is, what a pick sends. Owner: amb-home.
import type { Account, AppCmd } from '@/proto';
import type { ProviderCheck, Sent } from '@/settings';

export const CHATGPT = 'chatgpt';
export const OPENROUTER = 'openrouter';

/** Where the step is: the list, a provider list, OpenRouter's two ways, a key field, a check. */
export type View =
  | { t: 'list' }
  | { t: 'which'; coding: boolean }
  | { t: 'openrouter' }
  | { t: 'key'; id: string }
  | { t: 'check'; id: string };

export interface PayState {
  view: View;
  /** the last provider_use he sent (its answer is the one newer than `base`) */
  sent: Sent | null;
  /** a sign-in started here (its browser is open): its provider_use goes once accounts say signed in */
  signing: string | null;
}

export const PAY_START: PayState = { view: { t: 'list' }, sent: null, signing: null };

export type Row =
  | { t: 'found'; acc: Account }
  | { t: 'chatgpt'; acc: Account | null }
  | { t: 'openrouter' }
  | { t: 'api' }
  | { t: 'coding' };

/** The list's rows: the keys found (env, auth.json, an old .env) first, then the ways to pay. */
export function rows(accounts: readonly Account[]): Row[] {
  const found: Row[] = accounts.filter((a) => a.kind === 'key' && a.from != null).map((acc) => ({ t: 'found', acc }));
  const chatgpt = accounts.find((a) => a.id === CHATGPT) ?? null;
  const v: Row[] = [...found];
  if (chatgpt !== null) v.push({ t: 'chatgpt', acc: chatgpt });
  if (accounts.some((a) => a.id === OPENROUTER)) v.push({ t: 'openrouter' });
  v.push({ t: 'api' });
  if (accounts.some((a) => a.coding === true)) v.push({ t: 'coding' });
  return v;
}

/** `which provider?`: the providers a key is pasted for (the coding plans after `a coding plan key`). */
export function keyProviders(accounts: readonly Account[], coding: boolean): Account[] {
  return accounts.filter((a) => a.kind === 'key' && (a.coding === true) === coding);
}

/** What his pick of a row does: a command to send (a check, a sign-in), and where the step goes. */
export function pickRow(s: PayState, r: Row, check: ProviderCheck | null): { s: PayState; cmd: AppCmd | null } {
  switch (r.t) {
    case 'found':
      // a key with no base URL (the foundry proxy): its field asks for the URL first (Isidor, 28 oct)
      if (r.acc.needs_url === true) return { s: { ...s, view: { t: 'key', id: r.acc.id } }, cmd: null };
      return startCheck(s, r.acc.id, undefined, check);
    case 'chatgpt':
      // signed in already: check the plan; else its browser sign-in, then the check
      if (r.acc?.state === 'signed_in') return startCheck(s, CHATGPT, undefined, check);
      return { s: { ...s, signing: CHATGPT, view: { t: 'check', id: CHATGPT } }, cmd: { cmd: 'sign_in', id: CHATGPT } };
    case 'openrouter':
      return { s: { ...s, view: { t: 'openrouter' } }, cmd: null };
    case 'api':
      return { s: { ...s, view: { t: 'which', coding: false } }, cmd: null };
    case 'coding':
      return { s: { ...s, view: { t: 'which', coding: true } }, cmd: null };
  }
}

/** OpenRouter's browser login (it mints a key in auth.json), then its check. */
export function openRouterSignIn(s: PayState): { s: PayState; cmd: AppCmd } {
  return { s: { ...s, signing: OPENROUTER, view: { t: 'check', id: OPENROUTER } }, cmd: { cmd: 'sign_in', id: OPENROUTER } };
}

/** provider_use of `id` (a pasted key, or none: the key found, the sign-in); `url`: the base URL of a provider with none. */
export function startCheck(s: PayState, id: string, key: string | undefined, check: ProviderCheck | null, url?: string): { s: PayState; cmd: AppCmd } {
  const cmd: AppCmd = { cmd: 'provider_use', id, ...(key !== undefined ? { key } : {}), ...(url !== undefined && url !== '' ? { base_url: url } : {}) };
  // a check sent from the key field (a pasted key, a URL) shows on its step (the failure under its fields); else the check's
  const field = key !== undefined || (url !== undefined && url !== '');
  return { s: { ...s, signing: null, sent: { id, base: check }, view: field ? { t: 'key', id } : { t: 'check', id } }, cmd };
}

/** What the key field sends: its key (blank: the key found, when there is one) and its URL (a provider with none). Null: nothing to check yet. Pure. */
export function keyFieldSend(acc: Account | undefined, key: string, url: string): { key: string | undefined; url: string | undefined } | null {
  const k = key.trim();
  const u = url.trim();
  if (acc?.needs_url === true && u === '') return null;
  if (k === '' && acc?.from == null) return null;
  return { key: k === '' ? undefined : k, url: acc?.needs_url === true ? u : undefined };
}

/** The sign-in he started ended signed in (the core's accounts): its check, once. */
export function afterSignIn(s: PayState, accounts: readonly Account[], check: ProviderCheck | null): { s: PayState; cmd: AppCmd } | null {
  if (s.signing === null) return null;
  const a = accounts.find((x) => x.id === s.signing);
  if (a?.state !== 'signed_in') return null;
  return startCheck(s, s.signing, undefined, check);
}

/** Where the check of the last send is: none sent, waiting (a sign-in or the call), or its end. */
export function phase(s: PayState, check: ProviderCheck | null): 'none' | 'signing' | 'checking' | 'works' | 'failed' {
  if (s.signing !== null) return 'signing';
  if (s.sent === null) return 'none';
  if (check === null || check === s.sent.base || check.id !== s.sent.id) return 'checking';
  if (check.state === 'works') return 'works';
  if (check.state === 'failed') return 'failed';
  return 'checking';
}

/** The step goes on once a check of his works (main then has a model that answered). */
export function ready(s: PayState, check: ProviderCheck | null): boolean {
  return phase(s, check) === 'works';
}

/**
 * '← back' or esc inside the step: one level up (a key field to its provider list, a check to the
 * list); a browser sign-in under way is cancelled on the way (sign_in_cancel), so he can pick
 * another way (Matthias, Slack: stuck on 'waiting for you to sign in to OpenRouter'). Null: the
 * list, the step itself goes back.
 */
export function upOf(s: PayState, accounts: readonly Account[]): { s: PayState; cmd: AppCmd | null } | null {
  const v = s.view;
  if (v.t === 'list') return null;
  if (v.t === 'key') {
    const acc = accounts.find((a) => a.id === v.id);
    if (v.id === OPENROUTER) return { s: { ...s, view: { t: 'openrouter' } }, cmd: null };
    return { s: { ...s, view: { t: 'which', coding: acc?.coding === true } }, cmd: null };
  }
  const cmd: AppCmd | null = s.signing !== null ? { cmd: 'sign_in_cancel', id: s.signing } : null;
  return { s: { ...s, view: { t: 'list' }, signing: null }, cmd };
}
