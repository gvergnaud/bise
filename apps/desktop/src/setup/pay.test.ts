import { describe, expect, it } from 'vitest';
import type { Account } from '@/proto';
import { failedWords, keyField, type ProviderCheck } from '@/settings';
import { afterList, afterSignIn, enterOf, gatewayList, gatewayUse, gwModels, gwPhase, keyFieldSend, keyProviders, PAY_START, phase, pickRow, ready, rows, upOf, startCheck, type GatewayList, type Row } from './pay';

const acc = (id: string, kind: 'key' | 'subscription', extra: Partial<Account> = {}): Account => ({ id, label: id, kind, state: 'signed_out', provider: id, ...extra });
const ACCOUNTS: Account[] = [
  acc('chatgpt', 'subscription'),
  acc('anthropic', 'key', { label: 'Anthropic', keys_url: 'https://keys.test' }),
  acc('openrouter', 'key', { label: 'OpenRouter' }),
  acc('openai', 'key', { label: 'OpenAI', state: 'signed_in', from: { t: 'env', var: 'OPENAI_API_KEY' } }),
  acc('kimi-code', 'key', { label: 'Kimi', coding: true }),
];
const must = (a: Account | undefined): Account => {
  if (a === undefined) throw new Error('no account');
  return a;
};
const acct = (i: number): Account => must(ACCOUNTS[i]);
const check = (state: ProviderCheck['state'], extra: Partial<ProviderCheck> = {}): ProviderCheck => ({ ev: 'provider_check', id: 'anthropic', name: 'Anthropic', state, model: 'anthropic/claude-opus-5-5', ...extra });

describe('the model step (pay.ts)', () => {
  it('what already works first (a key found ready, ChatGPT signed in), then the other ways: ChatGPT, OpenRouter, an API key, a coding plan key', () => {
    const t = (r: Row) => (r.t === 'found' ? `found:${r.acc.id}` : r.t);
    const g = rows(ACCOUNTS);
    expect(g.ready.map(t)).toEqual(['found:openai']);
    expect(g.other.map(t)).toEqual(['chatgpt', 'openrouter', 'gateway', 'api', 'coding']);
    // ChatGPT signed in works already: it goes up; a key found that can't answer (no base URL) stays with the other ways
    const more = [...ACCOUNTS.map((a) => (a.id === 'chatgpt' ? { ...a, state: 'signed_in' } : a)), acc('foundry', 'key', { needs_url: true, from: { t: 'env', var: 'ANTHROPIC_FOUNDRY_API_KEY' } })];
    expect(rows(more).ready.map(t)).toEqual(['found:openai', 'chatgpt']);
    expect(rows(more).other.map(t)).toEqual(['found:foundry', 'openrouter', 'gateway', 'api', 'coding']);
    expect(rows(ACCOUNTS.filter((a) => a.coding !== true && a.id !== 'openrouter' && a.from == null))).toEqual({ ready: [], other: [{ t: 'chatgpt', acc: acct(0) }, { t: 'gateway', acc: null, url: null }, { t: 'api' }] });
    // which provider: a key found first, then the rest by name
    expect(keyProviders(ACCOUNTS, false)).toEqual({ found: [acct(3)], rest: [acct(1), acct(2)] });
    expect(keyProviders(ACCOUNTS, true).rest.map((a) => a.id)).toEqual(['kimi-code']);
  });

  it('a ready row is checked in place (the list stays); ⏎ takes the first ready row until a check runs', () => {
    const r = pickRow(PAY_START, { t: 'found', acc: acct(3) }, null);
    expect(r.s.view).toEqual({ t: 'list' });
    expect(enterOf(PAY_START, ACCOUNTS, null)).toEqual(r);
    // checking: ⏎ waits; failed: ⏎ checks it again; nothing ready: none
    expect(enterOf(r.s, ACCOUNTS, null)).toBeNull();
    expect(enterOf(r.s, ACCOUNTS, check('failed', { id: 'openai' }))?.cmd).toEqual({ cmd: 'provider_use', id: 'openai' });
    expect(enterOf(PAY_START, ACCOUNTS.filter((a) => a.from == null), null)).toBeNull();
    expect(enterOf({ ...PAY_START, view: { t: 'which', coding: false } }, ACCOUNTS, null)).toBeNull();
  });

  it('a found key is checked with no key sent; a pasted one is sent once; only its own answer counts', () => {
    const f = pickRow(PAY_START, { t: 'found', acc: acct(3) }, null);
    expect(f.cmd).toEqual({ cmd: 'provider_use', id: 'openai' });
    const older = check('works');
    const r = startCheck(PAY_START, 'anthropic', 'sk-1', older);
    expect(r.cmd).toEqual({ cmd: 'provider_use', id: 'anthropic', key: 'sk-1' });
    expect(phase(r.s, older)).toBe('checking');
    expect(phase(r.s, check('checking'))).toBe('checking');
    expect(phase(r.s, check('works', { id: 'openai' }))).toBe('checking');
    expect(phase(r.s, check('failed', { why: 'wrong_key' }))).toBe('failed');
    expect(ready(r.s, check('works'))).toBe(true);
    expect(ready(PAY_START, check('works'))).toBe(false);
  });

  it('ChatGPT not signed in: its browser sign-in, then the plan check once accounts say signed in', () => {
    const r = pickRow(PAY_START, { t: 'chatgpt', acc: acct(0) }, null);
    expect(r.cmd).toEqual({ cmd: 'sign_in', id: 'chatgpt' });
    expect(phase(r.s, null)).toBe('signing');
    expect(afterSignIn(r.s, ACCOUNTS, null)).toBeNull();
    const signed = ACCOUNTS.map((a) => (a.id === 'chatgpt' ? { ...a, state: 'signed_in' as const } : a));
    const next = afterSignIn(r.s, signed, null);
    expect(next?.cmd).toEqual({ cmd: 'provider_use', id: 'chatgpt' });
    expect(next && afterSignIn(next.s, signed, null)).toBeNull();
    expect(pickRow(PAY_START, { t: 'chatgpt', acc: must(signed[0]) }, null).cmd).toEqual({ cmd: 'provider_use', id: 'chatgpt' });
  });

  it('back (or esc) goes one level up: a key field to its provider list, a check to the list, the list out; a sign-in under way is cancelled', () => {
    expect(upOf({ ...PAY_START, view: { t: 'key', id: 'kimi-code' } }, ACCOUNTS)).toEqual({ s: { ...PAY_START, view: { t: 'which', coding: true } }, cmd: null });
    expect(upOf({ ...PAY_START, view: { t: 'key', id: 'openrouter' } }, ACCOUNTS)?.s.view).toEqual({ t: 'openrouter' });
    expect(upOf({ ...PAY_START, view: { t: 'check', id: 'x' }, signing: 'x' }, ACCOUNTS)).toEqual({ s: PAY_START, cmd: { cmd: 'sign_in_cancel', id: 'x' } });
    // a check with no sign-in under way: nothing to cancel
    expect(upOf({ ...PAY_START, view: { t: 'check', id: 'x' } }, ACCOUNTS)).toEqual({ s: PAY_START, cmd: null });
    expect(upOf(PAY_START, ACCOUNTS)).toBeNull();
  });
});

describe('the live check in words (settings/keycheck.ts)', () => {
  it('the key field: checking until its own answer', () => {
    const base = check('failed');
    expect(keyField(null, null)).toBe('typing');
    expect(keyField({ id: 'anthropic', base }, base)).toBe('checking');
    expect(keyField({ id: 'anthropic', base }, check('failed'))).toBe('refused');
    expect(keyField({ id: 'anthropic', base }, check('works'))).toBe('added');
  });

  it("each failure says the TUI's words, with the provider's own and the page that fixes it", () => {
    const w = failedWords(check('failed', { why: 'wrong_key', said: 'invalid x-api-key', url: 'https://keys.test', pasted: true }), null, false);
    expect(w).toEqual({ line: 'Anthropic says this key is wrong: “invalid x-api-key”', said: null, fix: { text: 'copy it again from ', url: 'https://keys.test' } });
    expect(failedWords(check('failed', { why: 'no_access', said: 'nope' }), null, false).said).toBe('Anthropic said: “nope”');
    expect(failedWords(check('failed', { why: 'wrong_key' }), 'ANTHROPIC_API_KEY', false).line).toBe("the key in ANTHROPIC_API_KEY doesn't work. Anthropic says it's wrong.");
    expect(failedWords(check('failed', { why: 'no_credit', url: 'https://bill.test' }), null, false)).toMatchObject({ line: 'the key works, but your Anthropic account has no credit yet.', fix: { url: 'https://bill.test' } });
    expect(failedWords(check('failed', { why: 'unreachable', said: 'connection refused' }), null, false).line).toBe("i couldn't reach Anthropic: connection refused.");
    expect(failedWords(check('failed', { why: 'model' }), null, false).line).toBe("Anthropic doesn't know claude-opus-5-5. pick another model.");
  });

  it("a found key with no base URL (Isidor's foundry proxy) opens its field for the URL; the URL rides with the check", () => {
    const foundry = acc('foundry', 'key', { label: 'Anthropic (foundry proxy)', needs_url: true, from: { t: 'env', var: 'ANTHROPIC_FOUNDRY_API_KEY' } });
    const f = pickRow(PAY_START, { t: 'found', acc: foundry }, null);
    expect(f.cmd).toBeNull();
    expect(f.s.view).toEqual({ t: 'key', id: 'foundry' });
    // no URL: nothing to check yet; the found key needs no paste
    expect(keyFieldSend(foundry, '', '')).toBeNull();
    expect(keyFieldSend(foundry, '', ' https://proxy.test ')).toEqual({ key: undefined, url: 'https://proxy.test' });
    const r = startCheck(PAY_START, 'foundry', undefined, null, 'https://proxy.test');
    expect(r.cmd).toEqual({ cmd: 'provider_use', id: 'foundry', base_url: 'https://proxy.test' });
    expect(r.s.view).toEqual({ t: 'key', id: 'foundry' });
    // a provider with a URL: the key alone, never a URL; no key found and none pasted: nothing
    expect(keyFieldSend(acct(1), 'sk-1', 'https://x.test')).toEqual({ key: 'sk-1', url: undefined });
    expect(keyFieldSend(acct(1), '  ', '')).toBeNull();
  });

  it("back from 'it works' goes to the view he picked from, never to hello (Gabriel, onboarding-models point 1)", () => {
    // a ready row checked in place: works, then back shows the list again, the check off the screen
    const r = pickRow(PAY_START, { t: 'found', acc: acct(3) }, null);
    const works = check('works', { id: 'openai' });
    expect(phase(r.s, works)).toBe('works');
    const up = upOf(r.s, ACCOUNTS, works);
    expect(up).toEqual({ s: { ...r.s, sent: null, view: { t: 'list' } }, cmd: null });
    expect(up === null ? null : phase(up.s, works)).toBe('none');
    // a typed key: back to its field
    const typed = startCheck({ ...PAY_START, view: { t: 'key', id: 'anthropic' } }, 'anthropic', 'sk-fake', null);
    expect(upOf(typed.s, ACCOUNTS, check('works'))?.s.view).toEqual({ t: 'key', id: 'anthropic' });
    // a plan's check view: back to the list
    const plan = startCheck(PAY_START, 'chatgpt', undefined, null);
    expect(upOf(plan.s, ACCOUNTS, check('works', { id: 'chatgpt' }))?.s.view).toEqual({ t: 'list' });
    // nothing works yet on the list: the step itself goes back (hello)
    expect(upOf(PAY_START, ACCOUNTS, null)).toBeNull();
  });

  const listed = (models: GatewayList['models'], url = 'https://gw.test/v1'): GatewayList => ({ ev: 'gateway_list', state: 'listed', url, models });
  const MODELS: GatewayList['models'] = [
    { id: 'azure_ai/claude-opus-5-5', label: 'opus 5.5', known: true, recommended: true, context: 1_000_000 },
    { id: 'mistral/mistral-large-4', label: 'mistral-large-4', known: true },
    { id: 'my-own-model', label: 'my-own-model' },
  ];

  it('the LiteLLM gateway: one row, ready only when its URL and key are found (designer m_64550)', () => {
    const gw = acc('litellm', 'key', { label: 'LiteLLM gateway', needs_url: true, from: { t: 'env', var: 'LITELLM_API_KEY' } });
    const t = (r: Row) => (r.t === 'found' ? `found:${r.acc.id}` : r.t);
    // its key found, no URL: under the other ways, once (never also as a found key)
    expect(rows([...ACCOUNTS, gw]).ready.map(t)).toEqual(['found:openai']);
    expect(rows([...ACCOUNTS, gw]).other.map(t)).toEqual(['chatgpt', 'openrouter', 'gateway', 'api', 'coding']);
    // its URL found too: a ready row, ⏎ lists its models at once (the key found: none sent)
    const g = rows([...ACCOUNTS, gw], 'https://gw.test');
    expect(g.ready.map(t)).toEqual(['found:openai', 'gateway']);
    const row = g.ready[1];
    if (row === undefined) throw new Error('no row');
    const p = pickRow(PAY_START, row, null, null);
    expect(p.cmd).toEqual({ cmd: 'gateway_models', url: 'https://gw.test' });
    expect(p.s.view).toEqual({ t: 'models', up: 'list' });
    // no URL found: its fields
    const other = rows([...ACCOUNTS, gw]).other.find((r) => r.t === 'gateway');
    if (other === undefined) throw new Error('no row');
    expect(pickRow(PAY_START, other, null).s.view).toEqual({ t: 'gateway' });
  });

  it('the gateway path: connect lists, the fields wait, the list opens once; back from each view to the one he came from (tasks-ux m_64555)', () => {
    const s0 = { ...PAY_START, view: { t: 'gateway' } as const };
    const sent = gatewayList(s0, ' https://gw.test ', ' sk-fake-gw ', null);
    expect(sent.cmd).toEqual({ cmd: 'gateway_models', url: 'https://gw.test', key: 'sk-fake-gw' });
    expect(gwPhase(sent.s, null)).toBe('listing');
    expect(gwPhase(sent.s, { ev: 'gateway_list', state: 'listing' })).toBe('listing');
    // a failure stays on the fields
    const failed: GatewayList = { ev: 'gateway_list', state: 'failed', why: 'wrong_key', said: 'invalid key' };
    expect(gwPhase(sent.s, failed)).toBe('failed');
    expect(afterList(sent.s, failed)).toBeNull();
    // listed: on to its models, once; back goes to the fields, URL and key kept
    const list = listed(MODELS);
    const on = afterList(sent.s, list);
    expect(on?.view).toEqual({ t: 'models', up: 'gateway' });
    if (on === null) throw new Error('no move');
    expect(afterList(on, list)).toBeNull();
    const up = upOf(on, ACCOUNTS);
    expect(up?.s.view).toEqual({ t: 'gateway' });
    expect(up?.s.gw).toMatchObject({ url: 'https://gw.test', key: 'sk-fake-gw', wait: false });
    // back while it lists: the list, and its answer moves nothing
    const away = upOf(sent.s, ACCOUNTS);
    expect(away?.s.view).toEqual({ t: 'list' });
    expect(away === null ? null : afterList({ ...away.s, view: { t: 'gateway' } }, list)).toBeNull();
    // his pick: provider_use of that model at the URL the list answered at, with the key he gave
    const use = gatewayUse(on, 'azure_ai/claude-opus-5-5', list, null);
    expect(use.cmd).toEqual({ cmd: 'provider_use', id: 'litellm', model: 'litellm/azure_ai/claude-opus-5-5', base_url: 'https://gw.test/v1', key: 'sk-fake-gw' });
    expect(use.s.view).toEqual({ t: 'models', up: 'gateway' });
    // it works: back is its list again (the pick kept on screen)
    const works = check('works', { id: 'litellm', model: 'litellm/azure_ai/claude-opus-5-5' });
    expect(upOf(use.s, ACCOUNTS, works)?.s.view).toEqual({ t: 'models', up: 'gateway' });
    // the filter, on ids and names
    expect(gwModels(list, 'OPUS').map((m) => m.id)).toEqual(['azure_ai/claude-opus-5-5']);
    expect(gwModels(list, 'mistral/').map((m) => m.id)).toEqual(['mistral/mistral-large-4']);
    expect(gwModels(failed, '')).toEqual([]);
  });
});
