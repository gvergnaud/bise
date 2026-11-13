// The first launch's model step (the TUI's key step, onboarding.rs: "how do you want to pay for
// the models?"): the keys found first, then a plan he has (ChatGPT), OpenRouter (sign in or a
// key), an API key (which provider), a coding plan key; then the key pasted or the sign-in, its
// live check (provider_use -> provider_check, the core's firstkey.rs) and "it works" with the
// model's own first words. Pure: the rows, where the step is, what a pick sends. Owner: amb-home.
import type { Account, AppCmd, CoreEv, GatewayModel } from '@/proto';
import type { ProviderCheck, Sent } from '@/settings';

export const CHATGPT = 'chatgpt';
export const OPENROUTER = 'openrouter';
/** the LiteLLM gateway's provider (the core's models.toml, onboarding-models) */
export const GATEWAY = 'litellm';

/**
 * Where the step is: the list, a provider list, OpenRouter's two ways, a key field, a check; a gateway's
 * URL and key (`gateway`), its model list (`models`; `up`: where back goes, the view he came from:
 * the list from its ready row, else the gateway's fields; tasks-ux m_64555).
 */
export type View =
  | { t: 'list' }
  | { t: 'which'; coding: boolean }
  | { t: 'openrouter' }
  | { t: 'key'; id: string }
  | { t: 'check'; id: string }
  | { t: 'gateway' }
  | { t: 'models'; up: 'list' | 'gateway' };

/** The gateway's list he asked for: its URL and key as he gave them (none: the key found); `base`: the list event shown when he asked (its answer is a newer one); `wait`: the fields wait on it (back drops it). */
export interface GwSent {
  url: string;
  key: string | undefined;
  base: GatewayList | null;
  wait: boolean;
}

export interface PayState {
  view: View;
  /** the last provider_use he sent (its answer is the one newer than `base`) */
  sent: Sent | null;
  /** a sign-in started here (its browser is open): its provider_use goes once accounts say signed in */
  signing: string | null;
  /** the last gateway_models he sent */
  gw: GwSent | null;
}

export const PAY_START: PayState = { view: { t: 'list' }, sent: null, signing: null, gw: null };

export type GatewayList = Extract<CoreEv, { ev: 'gateway_list' }>;
export type GatewayFound = Extract<CoreEv, { ev: 'gateway_found' }>;

export type Row =
  | { t: 'found'; acc: Account }
  | { t: 'chatgpt'; acc: Account | null }
  | { t: 'openrouter' }
  | { t: 'gateway'; acc: Account | null; url: string | null }
  | { t: 'api' }
  | { t: 'coding' };

/**
 * The list's rows in two groups (designer m_52236, Gabriel: what works already goes on top):
 * `ready`, what already works here (a key found in his shell, auth.json or an old .env, ready; ChatGPT
 * signed in; a LiteLLM gateway whose URL and key are both found); `other`, the other ways (a key found
 * that can't answer yet first: the foundry proxy with no URL; then ChatGPT to sign in, OpenRouter, the
 * LiteLLM gateway, an API key, a coding plan key). The gateway is one row, never two (designer m_64550).
 */
export function rows(accounts: readonly Account[], gwUrl: string | null = null): { ready: Row[]; other: Row[] } {
  const ready: Row[] = [];
  const other: Row[] = [];
  for (const acc of accounts.filter((a) => a.kind === 'key' && a.from != null && a.id !== GATEWAY)) (acc.state === 'signed_in' ? ready : other).push({ t: 'found', acc });
  const chatgpt = accounts.find((a) => a.id === CHATGPT) ?? null;
  if (chatgpt !== null) (chatgpt.state === 'signed_in' ? ready : other).push({ t: 'chatgpt', acc: chatgpt });
  if (accounts.some((a) => a.id === OPENROUTER)) other.push({ t: 'openrouter' });
  const gwAcc = accounts.find((a) => a.id === GATEWAY) ?? null;
  // its URL: the one found in his tools' configs, else none (a URL in config.toml makes the account ready)
  const gwReady = gwAcc?.from != null && (gwUrl !== null || gwAcc.needs_url !== true);
  (gwReady ? ready : other).push({ t: 'gateway', acc: gwAcc, url: gwUrl });
  other.push({ t: 'api' });
  if (accounts.some((a) => a.coding === true)) other.push({ t: 'coding' });
  return { ready, other };
}

/** `which provider?`: the providers a key is pasted for (the coding plans after `a coding plan key`): the ones with a key found first, then the rest by name. */
export function keyProviders(accounts: readonly Account[], coding: boolean): { found: Account[]; rest: Account[] } {
  const all = accounts.filter((a) => a.kind === 'key' && (a.coding === true) === coding);
  return {
    found: all.filter((a) => a.from != null),
    rest: all.filter((a) => a.from == null).sort((a, b) => a.label.localeCompare(b.label)),
  };
}

/** A ready row's provider id (a key found's account, ChatGPT). */
export function readyId(r: Row): string {
  return r.t === 'found' ? r.acc.id : r.t === 'chatgpt' ? (r.acc?.id ?? CHATGPT) : r.t === 'gateway' ? GATEWAY : r.t;
}

/** The ready row ⏎ takes, shown selected: the one he last checked (a failure: again), else the first. Undefined: none ready. */
export function enterRow(s: PayState, accounts: readonly Account[], gwUrl: string | null = null): string | undefined {
  const ids = rows(accounts, gwUrl).ready.map(readyId);
  return ids.find((id) => id === s.sent?.id) ?? ids[0];
}

/** ⏎ on the list before anything works: its ready row (enterRow), checked (designer m_52236: it comes selected). Null: none, or a check runs. */
export function enterOf(s: PayState, accounts: readonly Account[], check: ProviderCheck | null, gwUrl: string | null = null, list: GatewayList | null = null): { s: PayState; cmd: AppCmd | null } | null {
  const id = enterRow(s, accounts, gwUrl);
  const row = rows(accounts, gwUrl).ready.find((r) => readyId(r) === id);
  const ph = phase(s, check);
  if (s.view.t !== 'list' || row === undefined || ph === 'checking' || ph === 'works' || ph === 'signing') return null;
  return pickRow(s, row, check, list);
}

/** What his pick of a row does: a command to send (a check, a sign-in, a gateway's list), and where the step goes. `list`: the gateway list shown now. */
export function pickRow(s: PayState, r: Row, check: ProviderCheck | null, list: GatewayList | null = null): { s: PayState; cmd: AppCmd | null } {
  switch (r.t) {
    case 'gateway':
      // its URL and key found: its models at once, back goes to the list; else its fields (tasks-ux m_64555)
      if (r.url !== null && r.acc?.from != null) {
        const sent = gatewayList(s, r.url, undefined, list);
        return { s: { ...sent.s, view: { t: 'models', up: 'list' } }, cmd: sent.cmd };
      }
      return { s: { ...s, view: { t: 'gateway' } }, cmd: null };
    case 'found':
      // a key with no base URL (the foundry proxy): its field asks for the URL first (Isidor, 28 oct)
      if (r.acc.needs_url === true) return { s: { ...s, view: { t: 'key', id: r.acc.id } }, cmd: null };
      // a ready one is checked in place: its checking and its failure show under its row (designer m_52236)
      return onList(startCheck(s, r.acc.id, undefined, check));
    case 'chatgpt':
      // signed in already: check the plan, in place; else its browser sign-in, then the check
      if (r.acc?.state === 'signed_in') return onList(startCheck(s, CHATGPT, undefined, check));
      return { s: { ...s, signing: CHATGPT, view: { t: 'check', id: CHATGPT } }, cmd: { cmd: 'sign_in', id: CHATGPT } };
    case 'openrouter':
      return { s: { ...s, view: { t: 'openrouter' } }, cmd: null };
    case 'api':
      return { s: { ...s, view: { t: 'which', coding: false } }, cmd: null };
    case 'coding':
      return { s: { ...s, view: { t: 'which', coding: true } }, cmd: null };
  }
}

function onList(r: { s: PayState; cmd: AppCmd }): { s: PayState; cmd: AppCmd } {
  return { s: { ...r.s, view: { t: 'list' } }, cmd: r.cmd };
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
export function upOf(s: PayState, accounts: readonly Account[], check: ProviderCheck | null = null): { s: PayState; cmd: AppCmd | null } | null {
  const v = s.view;
  // 'it works' (onboarding-models, point 1): back to the view he picked from, the check off the screen (what
  // it saved stays): a ready row's list, the key field of a typed key, a gateway's model list; never hello
  if (phase(s, check) === 'works') return { s: { ...s, sent: null, view: v.t === 'check' ? { t: 'list' } : v }, cmd: null };
  if (v.t === 'list') return null;
  // the gateway's fields: to the list, a list under way dropped (its answer moves nothing)
  if (v.t === 'gateway') return { s: { ...s, view: { t: 'list' }, gw: s.gw === null ? null : { ...s.gw, wait: false } }, cmd: null };
  // its models: to the view he came from (the list from its ready row, else its fields, URL and key kept)
  if (v.t === 'models') return { s: { ...s, view: v.up === 'list' ? { t: 'list' } : { t: 'gateway' }, sent: null, gw: s.gw === null ? null : { ...s.gw, wait: false } }, cmd: null };
  if (v.t === 'key') {
    const acc = accounts.find((a) => a.id === v.id);
    if (v.id === OPENROUTER) return { s: { ...s, view: { t: 'openrouter' } }, cmd: null };
    return { s: { ...s, view: { t: 'which', coding: acc?.coding === true } }, cmd: null };
  }
  const cmd: AppCmd | null = s.signing !== null ? { cmd: 'sign_in_cancel', id: s.signing } : null;
  return { s: { ...s, view: { t: 'list' }, signing: null }, cmd };
}

/** gateway_models of `url` (`key`: pasted, none = the key found): the fields wait on its answer, the one newer than `list`. */
export function gatewayList(s: PayState, url: string, key: string | undefined, list: GatewayList | null): { s: PayState; cmd: AppCmd } {
  const u = url.trim();
  const k = key?.trim();
  const cmd: AppCmd = { cmd: 'gateway_models', url: u, ...(k !== undefined && k !== '' ? { key: k } : {}) };
  return { s: { ...s, sent: null, gw: { url: u, key: k !== undefined && k !== '' ? k : undefined, base: list, wait: true } }, cmd };
}

/** Where the gateway list he asked for is: none asked, listing, or its end. */
export function gwPhase(s: PayState, list: GatewayList | null): 'none' | 'listing' | 'listed' | 'failed' {
  if (s.gw === null) return 'none';
  if (list === null || list === s.gw.base || list.state === 'listing') return 'listing';
  if (list.state === 'listed') return 'listed';
  if (list.state === 'failed') return 'failed';
  return 'listing';
}

/** The list his fields wait on answered with models: on to them, once (back goes to the fields). Null: nothing to do. */
export function afterList(s: PayState, list: GatewayList | null): PayState | null {
  if (s.gw?.wait !== true || s.view.t !== 'gateway' || gwPhase(s, list) !== 'listed') return null;
  return { ...s, view: { t: 'models', up: 'gateway' }, gw: { ...s.gw, wait: false } };
}

/** His pick on the gateway's list: provider_use of that model at the URL the list answered at, the key he gave (none: the key found); it shows under its row on the list. */
export function gatewayUse(s: PayState, model: string, list: GatewayList, check: ProviderCheck | null): { s: PayState; cmd: AppCmd } {
  const key = s.gw?.key;
  const cmd: AppCmd = { cmd: 'provider_use', id: GATEWAY, model: `${GATEWAY}/${model}`, ...(list.url != null ? { base_url: list.url } : {}), ...(key !== undefined ? { key } : {}) };
  return { s: { ...s, signing: null, sent: { id: GATEWAY, base: check } }, cmd };
}

/** The gateway's models as the list shows them: the filter on the id and the name. Pure. */
export function gwModels(list: GatewayList | null, q: string): readonly GatewayModel[] {
  const all = list?.state === 'listed' ? (list.models ?? []) : [];
  const f = q.trim().toLowerCase();
  return f === '' ? all : all.filter((m) => m.id.toLowerCase().includes(f) || m.label.toLowerCase().includes(f));
}
