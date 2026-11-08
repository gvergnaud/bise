// @vitest-environment happy-dom
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { Account, AppCmd, Found } from '@/proto';
import { core_ev } from '@/proto/fixtures.gen';
import { settings } from '@/proto/words.gen';
import { firstSend, fnAct, ideas, next, START, stepOf, STEPS, ticked } from './model';

// the first message's composer: its props (the real one is ProseMirror's, tested in src/composer)
const composer = vi.hoisted(() => ({ props: null as null | { placeholder?: string; onSend: (o: { text: string }) => void; onReady?: (a: { fill: (t: string) => void; focus: () => void }) => void }, filled: [] as string[] }));
vi.mock('@/composer', () => ({
  Composer: (props: NonNullable<typeof composer.props>) => {
    composer.props = props;
    props.onReady?.({ fill: (t) => composer.filled.push(t), focus: () => undefined });
    return <div data-testid="setup.composer">{props.placeholder}</div>;
  },
}));
import { homePath, noRepoWords, Onboarding, type OnboardingPorts } from './Onboarding';
import { FirstRun, showsFirstRun } from './FirstRun';
import { helloOpens, startHereUrl } from './screen';

afterEach(cleanup);

const NOW = 1791100400000;
const accounts = (): Account[] => {
  const e = core_ev.find((x) => x.ev === 'accounts');
  if (e?.ev !== 'accounts') throw new Error('fixture');
  return e.items;
};
const found = (): Found[] => {
  const e = core_ev.find((x) => x.ev === 'found');
  if (e?.ev !== 'found') throw new Error('fixture');
  return e.items;
};
/** `looks`: his bim's look kept on leaving hello (page bim-pick), `telemetry`: Gabriel's note's acts
 * (hello marks it seen), apart so the steps' own commands read alone */
const ports = (): OnboardingPorts & { sent: AppCmd[]; looks: AppCmd[]; telemetry: AppCmd[]; log: string[] } => {
  const sent: AppCmd[] = [];
  const looks: AppCmd[] = [];
  const telemetry: AppCmd[] = [];
  const log: string[] = [];
  const to = (c: AppCmd) => (c.cmd === 'prefs_set' && c.key === 'bim' ? looks : c.cmd === 'telemetry' ? telemetry : sent);
  return { sent, looks, telemetry, log, send: (c) => to(c).push(c), done: () => log.push('done') };
};

/** The model step's check that works (ChatGPT, signed in in the fixture): its pick, then the answer. */
const WORKS = { ev: 'provider_check', id: 'chatgpt', name: 'ChatGPT', state: 'works', model: 'chatgpt/gpt-6.1-sol', answer: 'Hi!' } as const;
function passModel(rerender: (ui: ReactElement) => void, ui: (check: typeof WORKS) => ReactElement): void {
  fireEvent.click(screen.getByTestId('setup.pay.ready.chatgpt'));
  rerender(ui(WORKS));
  fireEvent.click(screen.getByTestId('setup.next'));
}

describe('the first launch model', () => {
  it('walks four steps: hello, model (only signed in), projects (scans, adds the ticked), first (its ⏎ is the composer\'s)', () => {
    expect([...STEPS]).toEqual(['hello', 'model', 'projects', 'first']);
    const c = { signedIn: false, found: [] as Found[], now: NOW };
    let r = next(START, c);
    expect(stepOf(r.s)).toBe('model');
    expect(next(r.s, c).s).toBe(r.s);
    r = next(r.s, { ...c, signedIn: true });
    expect(stepOf(r.s)).toBe('projects');
    expect(r.cmds).toEqual([{ cmd: 'found_scan' }]);
    r = next(r.s, { ...c, signedIn: true, found: found() });
    expect(r.cmds).toEqual([{ cmd: 'project_add', path: '/Users/camille/code/shop' }]);
    expect(stepOf(r.s)).toBe('first');
    expect(next(r.s, c)).toEqual({ s: r.s, cmds: [], done: false });
    expect(firstSend(r.s, '   ')).toBeNull();
    expect(firstSend(r.s, 'review my open PRs')).toMatchObject({ done: true, cmds: [{ cmd: 'send', text: 'review my open PRs' }, { cmd: 'prefs_set', key: 'onboarded', value: true }] });
  });

  it('the first message\'s ideas come from his ticked projects; none ticked: two to start with', () => {
    expect(ideas(['storefront', 'shop'])).toEqual(['what moved in storefront this week?', 'find the flaky tests in shop', 'review my open PRs']);
    expect(ideas(['shop'])).toEqual(['what moved in shop this week?', 'find the flaky tests in shop', 'review my open PRs']);
    expect(ideas([])).toEqual(['show me what you can do', 'set up a project for me']);
  });

  it('a recent repo comes ticked, an old one or a known one not; fn counts once he lets go', () => {
    const [shop, acme] = found();
    if (!shop || !acme) throw new Error('fixture');
    expect(ticked(START, shop, NOW)).toBe(true);
    expect(ticked(START, acme, NOW)).toBe(false);
    expect(ticked(START, shop, shop.last_ms + 31 * 86_400_000)).toBe(false);
    const down = fnAct(START, 'talk_start');
    expect([down.fn, down.tried]).toEqual(['down', false]);
    expect(fnAct(down, 'talk_end')).toMatchObject({ fn: 'up', tried: true });
    expect(fnAct(START, 'talk_end').tried).toBe(false);
  });
});

describe('Onboarding', () => {
  it("hello: bise's typed ':*' (mono, no pearl, no canvas), its words, four dots; with no model, skip leads to the model step, which has none (Isidor, 28 oct)", () => {
    const p = ports();
    render(<Onboarding accounts={accounts()} found={found()} perms={{}} ports={p} now={NOW} />);
    const hello = screen.getByTestId('setup.step.hello');
    const mark = screen.getByTestId('setup.mark');
    expect(mark.classList.contains('b-kiss')).toBe(true);
    expect(mark.textContent).toBe(':*');
    expect(mark.querySelector('.b-kiss-s')?.textContent).toBe('*');
    expect(hello.querySelector('[data-bise-pearl], canvas')).toBeNull();
    expect(hello.textContent).toContain("hi. i'm bise.");
    expect(hello.textContent).toContain('i run your projects with a team of agents, and i only ask you what needs you.');
    expect(hello.querySelectorAll('.steps i').length).toBe(4);
    expect(screen.queryByTestId('setup.back')).toBeNull();
    fireEvent.keyDown(screen.getByTestId('setup.root'), { key: 'Enter' });
    expect(screen.getByTestId('setup.step.model')).toBeTruthy();
    expect(screen.queryByTestId('setup.perms')).toBeNull();
    // no model yet: the model step has no skip (the first launch always ends with a model that answers)
    expect(screen.queryByTestId('setup.skip')).toBeNull();
    expect(p.sent).toEqual([]);
    cleanup();
    // skip on hello with no model: the model step, never out
    const q = ports();
    render(<Onboarding accounts={accounts()} found={found()} perms={{}} ports={q} now={NOW} />);
    fireEvent.click(screen.getByTestId('setup.skip'));
    expect(screen.getByTestId('setup.step.model')).toBeTruthy();
    expect(q.log).toEqual([]);
    cleanup();
    // main has a model already: skip writes onboarded and opens bise
    const p2 = ports();
    render(<Onboarding accounts={accounts()} found={found()} perms={{}} ports={p2} now={NOW} hasModel />);
    fireEvent.click(screen.getByTestId('setup.skip'));
    expect(p2.sent).toEqual([{ cmd: 'prefs_set', key: 'onboarded', value: true }]);
    // he left hello without picking: his bim got one of the ready-made looks (page bim-pick)
    expect(p2.looks).toHaveLength(1);
    expect(p2.log.at(-1)).toBe('done');
  });

  it("hello offers 12 ready-made looks for his bim (page bim-pick): a click keeps it in prefs' bim; none picked, start keeps one at random", () => {
    let p = ports();
    const { unmount } = render(<Onboarding accounts={accounts()} found={[]} perms={{}} ports={p} now={NOW} />);
    const tiles = screen.getByTestId('setup.me').querySelectorAll('button');
    expect(tiles.length).toBe(12);
    fireEvent.click(screen.getByTestId('setup.me.3'));
    expect(p.looks).toEqual([{ cmd: 'prefs_set', key: 'bim', value: { skin: 4, hair: 'braids', color: 0, wear: 'none', hat: 0 } }]);
    expect(screen.getByTestId('setup.me.3').getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByTestId('setup.next'));
    expect(p.looks).toHaveLength(1);
    unmount();
    p = ports();
    vi.spyOn(Math, 'random').mockReturnValue(0.99);
    render(<Onboarding accounts={accounts()} found={[]} perms={{}} ports={p} now={NOW} />);
    fireEvent.click(screen.getByTestId('setup.next'));
    vi.restoreAllMocks();
    expect(p.looks).toEqual([{ cmd: 'prefs_set', key: 'bim', value: { skin: 0, hair: 'short', color: 4, wear: 'none', hat: 0 } }]);
  });

  it('the model step waits for a key that works (live check), then goes on; esc goes a level, then a step back', () => {
    const p = ports();
    const out = accounts().map((a) => ({ ...a, state: 'signed_out' as const }));
    const { rerender } = render(<Onboarding accounts={out} found={[]} perms={{}} ports={p} now={NOW} />);
    const root = screen.getByTestId('setup.root');
    fireEvent.keyDown(root, { key: 'Enter' });
    expect(screen.getByTestId('setup.step.model')).toBeTruthy();
    expect(screen.getByTestId('setup.step.model').textContent).toContain('which model should i use?');
    // nothing works yet: ⏎ stays
    fireEvent.keyDown(root, { key: 'Enter' });
    expect(screen.getByTestId('setup.step.model')).toBeTruthy();
    // an API key -> which provider -> its key field -> provider_use, checking
    fireEvent.click(screen.getByTestId('setup.pay.api'));
    fireEvent.click(screen.getByTestId('setup.pay.which.anthropic-key'));
    const f = screen.getByTestId<HTMLInputElement>('setup.pay.key');
    expect(f.type).toBe('password');
    f.value = 'sk-test';
    fireEvent.keyDown(f, { key: 'Enter' });
    expect(p.sent).toEqual([{ cmd: 'provider_use', id: 'anthropic-key', key: 'sk-test' }]);
    expect(screen.getByTestId('setup.step.model').textContent).toContain('checking your key');
    const base = { ev: 'provider_check', id: 'anthropic-key', name: 'Anthropic', model: 'anthropic/claude-opus-5-5', pasted: true } as const;
    rerender(<Onboarding accounts={out} found={[]} perms={{}} ports={p} now={NOW} check={{ ...base, state: 'failed', why: 'wrong_key', said: 'invalid x-api-key' }} />);
    // the failure shows on the paste step, under its field; no 'try again' for a wrong key
    expect(screen.getByTestId('setup.pay.failed').textContent).toContain('Anthropic says this key is wrong: “invalid x-api-key”');
    expect(screen.queryByTestId('setup.pay.again')).toBeNull();
    expect(document.body.textContent).not.toContain('sk-test');
    const g = screen.getByTestId<HTMLInputElement>('setup.pay.key');
    g.value = 'sk-right';
    fireEvent.keyDown(g, { key: 'Enter' });
    expect(p.sent.at(-1)).toEqual({ cmd: 'provider_use', id: 'anthropic-key', key: 'sk-right' });
    rerender(<Onboarding accounts={accounts()} found={[]} perms={{}} ports={p} now={NOW} check={{ ...base, state: 'works', label: 'opus 5.5', answer: 'Hi there!', ms: 1234, main: 'anthropic/claude-opus-5-5', lines: ['main uses anthropic/claude-opus-5-5.'] }} />);
    expect(screen.getByTestId('setup.step.model').textContent).toContain('it works: opus 5.5 answered.');
    // the live check's line: ✓, the model's own words, how long the call took
    expect(screen.getByTestId('setup.pay.answer').textContent).toBe('✓ it said: “Hi there!” · 1.2 s');
    expect(screen.getByTestId('setup.step.model').querySelector('[data-bise-pearl], canvas')).toBeNull();
    fireEvent.keyDown(root, { key: 'Enter' });
    expect(screen.getByTestId('setup.step.projects')).toBeTruthy();
    expect(p.sent.at(-1)).toEqual({ cmd: 'found_scan' });
    fireEvent.keyDown(root, { key: 'Escape' });
    expect(screen.getByTestId('setup.step.model')).toBeTruthy();
  });

  it("'← back' on OpenRouter's sign-in cancels it and goes to the list, to pick another way (Matthias, Slack: stuck on 'waiting for you to sign in')", () => {
    const p = ports();
    const out: Account[] = [...accounts().map((a) => ({ ...a, state: 'signed_out' as const })), { id: 'openrouter', kind: 'key', label: 'OpenRouter', state: 'signed_out' }];
    render(<Onboarding accounts={out} found={[]} perms={{}} ports={p} now={NOW} />);
    fireEvent.click(screen.getByTestId('setup.next'));
    fireEvent.click(screen.getByTestId('setup.pay.openrouter'));
    fireEvent.click(screen.getByTestId('setup.pay.openrouter.signin'));
    expect(p.sent).toEqual([{ cmd: 'sign_in', id: 'openrouter' }]);
    expect(screen.getByTestId('setup.step.model').textContent).toContain('waiting for you to sign in to OpenRouter in your browser');
    fireEvent.click(screen.getByTestId('setup.back'));
    // still the model step, back on its list (never hello), the browser's sign-in cancelled
    expect(screen.getByTestId('setup.pay')).toBeTruthy();
    expect(screen.getByTestId('setup.step.model').textContent).not.toContain('waiting for you to sign in');
    expect(p.sent.at(-1)).toEqual({ cmd: 'sign_in_cancel', id: 'openrouter' });
    // another way from there: an API key
    fireEvent.click(screen.getByTestId('setup.pay.api'));
    expect(screen.getByTestId('setup.pay.which')).toBeTruthy();
    // back twice more: the list, then hello
    fireEvent.click(screen.getByTestId('setup.back'));
    expect(screen.getByTestId('setup.pay')).toBeTruthy();
    fireEvent.click(screen.getByTestId('setup.back'));
    expect(screen.getByTestId('setup.step.hello')).toBeTruthy();
    expect(p.sent.filter((c) => c.cmd === 'sign_in_cancel')).toHaveLength(1);
    // the waiting view's own way out, where he looks (designer m_52216): 'cancel  esc' does what back does
    fireEvent.click(screen.getByTestId('setup.next'));
    fireEvent.click(screen.getByTestId('setup.pay.openrouter'));
    fireEvent.click(screen.getByTestId('setup.pay.openrouter.signin'));
    expect(screen.getByTestId('setup.pay.signin.cancel').parentElement?.textContent).toBe('cancelesc');
    fireEvent.click(screen.getByTestId('setup.pay.signin.cancel'));
    expect(screen.getByTestId('setup.pay')).toBeTruthy();
    expect(p.sent.filter((c) => c.cmd === 'sign_in_cancel')).toHaveLength(2);
    // esc where the focus is once the sign-in's button has gone (ui-review m_52241: it fell on the body); no dead 'continue' while it waits (designer m_52243)
    fireEvent.click(screen.getByTestId('setup.pay.openrouter'));
    fireEvent.click(screen.getByTestId('setup.pay.openrouter.signin'));
    expect(screen.queryByTestId('setup.next')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId('setup.root'));
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
    expect(screen.getByTestId('setup.pay')).toBeTruthy();
    expect(p.sent.filter((c) => c.cmd === 'sign_in_cancel')).toHaveLength(3);
    expect(screen.getByTestId('setup.next')).toBeTruthy();
  });

  it("with the island drawn, 'it works' keeps the model's name whole and says how long it took (its words are the bubble's)", () => {
    // hasWebGL asks only whether the name exists; the scene then fails in happy-dom and the island goes
    vi.stubGlobal('WebGLRenderingContext', vi.fn());
    try {
      const p = ports();
      const out = accounts().map((a) => ({ ...a, state: 'signed_out' as const }));
      const { rerender } = render(<Onboarding accounts={out} found={[]} perms={{}} ports={p} now={NOW} />);
      fireEvent.keyDown(screen.getByTestId('setup.root'), { key: 'Enter' });
      fireEvent.click(screen.getByTestId('setup.pay.api'));
      fireEvent.click(screen.getByTestId('setup.pay.which.anthropic-key'));
      const f = screen.getByTestId<HTMLInputElement>('setup.pay.key');
      f.value = 'sk-test';
      fireEvent.keyDown(f, { key: 'Enter' });
      const base = { ev: 'provider_check', id: 'anthropic-key', name: 'Anthropic', model: 'anthropic/claude-sonnet-4-5', pasted: true } as const;
      rerender(<Onboarding accounts={accounts()} found={[]} perms={{}} ports={p} now={NOW} check={{ ...base, state: 'works', label: 'sonnet 4.5', answer: 'Hi there!', ms: 1234, lines: [] }} />);
      const step = screen.getByTestId('setup.step.model');
      expect(step.querySelector('.setup-model')?.textContent).toBe('sonnet 4.5');
      expect(step.textContent).toContain('it works: sonnet 4.5 answered.');
      expect(screen.getByTestId('setup.pay.answer').textContent).toBe('✓ answered in 1.2 s');
      expect(step.textContent).not.toContain('Hi there!');
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('the model step: ChatGPT signs in in his browser, then its plan is checked once', () => {
    const p = ports();
    const out = accounts().map((a) => ({ ...a, state: 'signed_out' as const }));
    const { rerender } = render(<Onboarding accounts={out} found={[]} perms={{}} ports={p} now={NOW} />);
    const root = screen.getByTestId('setup.root');
    fireEvent.keyDown(root, { key: 'Enter' });
    fireEvent.click(screen.getByTestId('setup.pay.chatgpt'));
    expect(p.sent).toEqual([{ cmd: 'sign_in', id: 'chatgpt' }]);
    rerender(<Onboarding accounts={accounts()} found={[]} perms={{}} ports={p} now={NOW} />);
    expect(p.sent).toEqual([{ cmd: 'sign_in', id: 'chatgpt' }, { cmd: 'provider_use', id: 'chatgpt' }]);
    expect(screen.getByTestId('setup.step.model').textContent).toContain('checking your plan');
    rerender(<Onboarding accounts={accounts()} found={[]} perms={{}} ports={p} now={NOW} />);
    expect(p.sent.length).toBe(2);
  });

  it("the model step: a Claude plan seen in Claude Code points at its monthly API credits, only without an Anthropic key", () => {
    const out = accounts().map((a) => ({ ...a, state: 'signed_out' as const }));
    const { rerender } = render(<Onboarding accounts={out} claudePlan found={[]} perms={{}} ports={ports()} now={NOW} />);
    fireEvent.keyDown(screen.getByTestId('setup.root'), { key: 'Enter' });
    const note = screen.getByTestId('setup.pay.claude').textContent;
    expect(note).toContain(settings.first_claude_plan);
    expect(note).toContain(settings.first_claude_credits_how);
    // an Anthropic key set up: nothing to say
    const keyed = out.map((a) => (a.id === 'anthropic-key' ? { ...a, provider: 'anthropic', state: 'signed_in' as const } : a));
    rerender(<Onboarding accounts={keyed} claudePlan found={[]} perms={{}} ports={ports()} now={NOW} />);
    expect(screen.queryByTestId('setup.pay.claude')).toBeNull();
    // no Claude plan: nothing either
    rerender(<Onboarding accounts={out} found={[]} perms={{}} ports={ports()} now={NOW} />);
    expect(screen.queryByTestId('setup.pay.claude')).toBeNull();
  });

  it("what already works goes on top, under 'Ready on this Mac', ✓ and where it comes from; ⏎ checks the first in place, a failure stays under its row (Gabriel m_52208, designer m_52236)", () => {
    const p = ports();
    const withEnv = accounts().map((a) => (a.kind === 'key' ? { ...a, state: 'signed_in' as const, from: { t: 'env' as const, var: 'ANTHROPIC_API_KEY' } } : a));
    const { rerender } = render(<Onboarding accounts={withEnv} found={[]} perms={{}} ports={p} now={NOW} />);
    const root = screen.getByTestId('setup.root');
    fireEvent.keyDown(root, { key: 'Enter' });
    const step = screen.getByTestId('setup.step.model');
    expect(step.textContent).toContain(settings.onb_found_title);
    // the ready group first, with its head: the key found, then ChatGPT signed in; the other ways under their own head
    const ready = screen.getByTestId('setup.pay.ready');
    expect(ready.querySelector('.b-gt')?.textContent).toBe(settings.onb_ready_head);
    const rows = [...ready.querySelectorAll('[data-row]')];
    expect(rows.map((r) => r.getAttribute('data-row'))).toEqual(['found-anthropic-key', 'chatgpt']);
    expect(rows.every((r) => r.querySelector('.setup-ready')?.textContent === '✓')).toBe(true);
    expect(rows[0]?.textContent).toContain(settings.onb_found_from({ var: 'ANTHROPIC_API_KEY' }));
    expect(rows[0]?.querySelector('code')?.textContent).toBe('ANTHROPIC_API_KEY');
    expect(rows[0]?.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByTestId('setup.pay').querySelector('.b-gt')?.textContent).toBe(settings.onb_other_head);
    expect(ready.compareDocumentPosition(screen.getByTestId('setup.pay')) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // ⏎ takes the first: its check, no key sent, on the list; 'checking' under its row
    fireEvent.keyDown(root, { key: 'Enter' });
    expect(p.sent).toEqual([{ cmd: 'provider_use', id: 'anthropic-key' }]);
    expect(screen.getByTestId('setup.pay.ready.found-anthropic-key').textContent).toContain(settings.first_checking_key);
    // a check with no provider name (ui-review m_52268's fake core): the account's name, never 'undefined'
    rerender(<Onboarding accounts={withEnv} found={[]} perms={{}} ports={p} now={NOW} check={{ ev: 'provider_check', id: 'anthropic-key', name: '', state: 'failed', why: 'wrong_key', model: 'anthropic/claude-opus-5-5', url: 'https://keys.test' }} />);
    // the failure under the row, the row still in its group, its ✓ gone (its slot kept), still ⏎'s row; it ends on the keys page's link (designer m_52270)
    const row = screen.getByTestId('setup.pay.ready.found-anthropic-key');
    const fail = row.querySelector('[data-testid="setup.pay.failed.anthropic-key"]');
    expect(fail?.textContent).toContain(settings.first_found_wrong({ key: settings.first_the_key_in({ var: 'ANTHROPIC_API_KEY' }), provider: 'Anthropic key' }));
    expect(fail?.textContent).not.toContain('undefined');
    expect(row.querySelector('[data-testid="setup.pay.link.fix"]')?.textContent).toBe(`${settings.first_paste_another_get.replace(/[:\s]+$/, '')} ↗`);
    expect(row.querySelector('.setup-ready')?.textContent).toBe('');
    expect(row.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByTestId('setup.pay.ready')).toBeTruthy();
    // ChatGPT signed in: its plan after the email
    expect(screen.getByTestId('setup.pay.ready.chatgpt').textContent).toContain(settings.first_signed_in({ email: 'camille@example.com', plan: 'Plus' }));
    // a click checks it again
    fireEvent.click(screen.getByTestId('setup.pay.ready.found-anthropic-key'));
    expect(p.sent.at(-1)).toEqual({ cmd: 'provider_use', id: 'anthropic-key' });
    // 'which provider?': the one with a key found first, ✓ and its variable; the rest after
    fireEvent.click(screen.getByTestId('setup.pay.api'));
    const which = screen.getByTestId('setup.pay.which.found.anthropic-key');
    expect(which.querySelector('.setup-ready')).toBeTruthy();
    expect(which.textContent).toContain(settings.onb_found_at({ at: 'ANTHROPIC_API_KEY' }));
    cleanup();
    // ChatGPT signed in with no plan known: the email alone, never 'ChatGPT' twice
    render(<Onboarding accounts={accounts().map((a) => ({ ...a, plan: null }))} found={[]} perms={{}} ports={ports()} now={NOW} />);
    fireEvent.keyDown(screen.getByTestId('setup.root'), { key: 'Enter' });
    expect(screen.getByTestId('setup.pay.ready.chatgpt').textContent).toBe(`✓ ChatGPT${settings.onb_signed_in({ email: 'camille@example.com' })}`);
    cleanup();
    // nothing found: one list, no heads, today's line
    render(<Onboarding accounts={accounts().map((a) => ({ ...a, state: 'signed_out' as const }))} found={[]} perms={{}} ports={ports()} now={NOW} />);
    fireEvent.keyDown(screen.getByTestId('setup.root'), { key: 'Enter' });
    expect(screen.queryByTestId('setup.pay.ready')).toBeNull();
    expect(screen.getByTestId('setup.pay').querySelector('.b-gt')).toBeNull();
    expect(screen.getByTestId('setup.step.model').textContent).toContain(settings.onb_pay_sub);
  });

  it('the projects step (round 3): a filter narrows the found list; choose a folder… adds it ticked and it is added on ⏎', async () => {
    const p = ports();
    const picked: string[] = [];
    const pickFolder = () => {
      picked.push('asked');
      return Promise.resolve('/Users/camille/notes');
    };
    // the fixture's shop (recent, ticked) and a second repo not his yet, older (unticked); acme is known: not listed
    const list: Found[] = [...found(), { name: 'billing', path: '/Users/camille/code/billing', git: true, known: false, last_ms: NOW - 90 * 86_400_000 }];
    const ui = (check: typeof WORKS | null = null) => <Onboarding accounts={accounts()} found={list} perms={{}} ports={{ ...p, pickFolder }} now={NOW} check={check} />;
    const { rerender } = render(ui());
    fireEvent.click(screen.getByTestId('setup.next'));
    passModel(rerender, ui);
    expect(screen.getByTestId('setup.step.projects')).toBeTruthy();
    expect(screen.getByTestId('setup.step.projects').textContent).toContain('i found these repos in your folder. tick the ones i should work in.');
    // the rows' checks (the rows themselves carry setup.found.<path>)
    const names = () =>
      [...screen.getByTestId('setup.found').querySelectorAll('[data-testid^="setup.found."], [data-testid^="setup.chosen."]')]
        .map((e) => e.getAttribute('data-testid') ?? '')
        .filter((id) => !id.includes('/'));
    expect(names()).toEqual(['setup.found.shop', 'setup.found.billing']);
    fireEvent.change(screen.getByTestId('setup.filter'), { target: { value: 'BILL' } });
    expect(names()).toEqual(['setup.found.billing']);
    fireEvent.change(screen.getByTestId('setup.filter'), { target: { value: 'nothing-like-it' } });
    expect(screen.getByTestId('setup.filter.none').textContent).toContain('no repo matches');
    fireEvent.change(screen.getByTestId('setup.filter'), { target: { value: '' } });
    fireEvent.click(screen.getByTestId('setup.choose'));
    await screen.findByTestId('setup.chosen.notes');
    expect(picked).toEqual(['asked']);
    expect(names()).toEqual(['setup.chosen.notes', 'setup.found.shop', 'setup.found.billing']);
    expect(screen.getByTestId('setup.next').textContent).toBe('add 2 projects');
    fireEvent.click(screen.getByTestId('setup.next'));
    expect(p.sent).toEqual([
      { cmd: 'provider_use', id: 'chatgpt' },
      { cmd: 'project_add', path: '/Users/camille/notes' },
      { cmd: 'project_add', path: '/Users/camille/code/shop' },
    ]);
  });

  it('paths read with ~ for his home, a long one cut in the middle keeping the last folders', () => {
    expect(homePath('/Users/camille/code/shop', '/Users/camille')).toBe('~/code/shop');
    expect(homePath('/Users/camille', '/Users/camille/')).toBe('~');
    expect(homePath('/opt/src/shop', '/Users/camille')).toBe('/opt/src/shop');
    expect(homePath('/Users/camille/work/clients/acme-industries/platform/storefront', '/Users/camille')).toBe('~/…/acme-industries/platform/storefront');
  });

  it('no picker in the shell: no choose a folder… button', () => {
    const pp = ports();
    const ui = (check: typeof WORKS | null = null) => <Onboarding accounts={accounts()} found={found()} perms={{}} ports={pp} now={NOW} check={check} />;
    const { rerender } = render(ui());
    fireEvent.click(screen.getByTestId('setup.next'));
    passModel(rerender, ui);
    expect(screen.queryByTestId('setup.choose')).toBeNull();
  });

  it('first: a real composer; an idea from his ticked projects fills it, never sends; ⏎ sends to bise, onboarded, opens bise', () => {
    const p = ports();
    const props = { accounts: accounts(), found: found(), perms: {}, ports: p, now: NOW };
    const { rerender } = render(<Onboarding {...props} />);
    fireEvent.click(screen.getByTestId('setup.next'));
    passModel(rerender, (check) => <Onboarding {...props} check={check} />);
    fireEvent.click(screen.getByTestId('setup.next'));
    const first = screen.getByTestId('setup.step.first');
    expect(first.textContent).toContain('what should i do first?');
    expect(screen.getByTestId('setup.composer').textContent).toBe('write to bise, then ⏎');
    expect(screen.getByTestId('setup.fnline').textContent).toBe('or hold fn and say it, from any app.');
    expect(screen.getByTestId('setup.fnline').querySelector('.b-kbd')?.textContent).toBe('fn');
    // no button: the composer is the step's way on; 'open bise' top right, no skip
    expect(screen.queryByTestId('setup.next')).toBeNull();
    expect(screen.queryByTestId('setup.skip')).toBeNull();
    expect(screen.getByTestId('setup.open').textContent).toBe('open bise');
    expect([...screen.getByTestId('setup.ideas').children].map((c) => c.textContent)).toEqual(['what moved in shop this week?', 'find the flaky tests in shop', 'review my open PRs']);
    const before = p.sent.length;
    fireEvent.click(screen.getByTestId('setup.idea.0'));
    expect(composer.filled.at(-1)).toBe('what moved in shop this week?');
    expect(p.sent.length).toBe(before);
    // ⏎ in the composer: his words to bise, then the first launch ends and bise opens
    fireEvent.keyDown(screen.getByTestId('setup.root'), { key: 'Enter' });
    expect(p.log).toEqual([]);
    act(() => {
      composer.props?.onSend({ text: 'what moved in shop this week?' });
    });
    expect(p.sent.slice(before)).toEqual([
      { cmd: 'send', text: 'what moved in shop this week?' },
      { cmd: 'prefs_set', key: 'onboarded', value: true },
    ]);
    expect(p.log).toEqual(['done']);
  });

  it('first: no project ticked gives two ideas; open bise leaves without asking', () => {
    const p = ports();
    const props = { accounts: accounts(), found: [] as Found[], perms: {}, ports: p, now: NOW };
    const { rerender } = render(<Onboarding {...props} />);
    fireEvent.click(screen.getByTestId('setup.next'));
    passModel(rerender, (check) => <Onboarding {...props} check={check} />);
    fireEvent.click(screen.getByTestId('setup.next'));
    expect([...screen.getByTestId('setup.ideas').children].map((c) => c.textContent)).toEqual(['show me what you can do', 'set up a project for me']);
    fireEvent.click(screen.getByTestId('setup.open'));
    expect(p.sent.at(-1)).toEqual({ cmd: 'prefs_set', key: 'onboarded', value: true });
    expect(p.sent.some((c) => c.cmd === 'send')).toBe(false);
    expect(p.log).toEqual(['done']);
  });

  it('first: holding fn there works too: once he lets go with the mic allowed, bise has it and opens', () => {
    const p = ports();
    const props = { accounts: accounts(), found: found(), perms: { mic: 'allowed', accessibility: 'allowed', screen: 'allowed' } as const, ports: p, now: NOW };
    const { rerender } = render(<Onboarding {...props} />);
    fireEvent.click(screen.getByTestId('setup.next'));
    passModel(rerender, (check) => <Onboarding {...props} check={check} />);
    fireEvent.click(screen.getByTestId('setup.next'));
    rerender(<Onboarding {...props} check={WORKS} fn={{ act: 'talk_start', n: 1 }} />);
    expect(p.log).toEqual([]);
    rerender(<Onboarding {...props} check={WORKS} fn={{ act: 'talk_end', n: 2 }} />);
    expect(p.sent.at(-1)).toEqual({ cmd: 'prefs_set', key: 'onboarded', value: true });
    expect(p.log).toEqual(['done']);
  });
});

describe('the start-here page after hello (S.9)', () => {
  const art = (id: string, kind: string, url: string) => ({ id, kind, title: id, agent: 'main', version: 1, at_ms: NOW, url, new: true });
  it("is the hub's start-here page among the home's artifacts, by its url", () => {
    expect(startHereUrl([art('perf-notes', 'page', 'http://h/p/perf-notes'), art('start-here', 'page', 'http://h/p/start-here')])).toBe('http://h/p/start-here');
    expect(startHereUrl([art('perf-notes', 'page', 'http://h/p/perf-notes')])).toBeNull();
    expect(startHereUrl([art('start-here', 'file', 'http://h/a/start-here')])).toBeNull();
    expect(startHereUrl([art('start-here', 'page', '')])).toBeNull();
  });

  it('opens once, on a fresh home only, after hello however it ended, once its url is known', () => {
    expect(helloOpens(true, false, true, 'http://h/p/start-here')).toBe(true);
    expect(helloOpens(false, false, true, 'http://h/p/start-here')).toBe(false); // onboarded before this window
    expect(helloOpens(true, true, true, 'http://h/p/start-here')).toBe(false); // already opened
    expect(helloOpens(true, false, false, 'http://h/p/start-here')).toBe(false); // hello still up
    expect(helloOpens(true, false, true, null)).toBe(false); // the page not known yet
  });
});

describe('the first-run text (L1 / M18)', () => {
  it("shows the TUI's words; its suggestion hands DEMO to fill, never a send", () => {
    const filled: string[] = [];
    render(<FirstRun onFill={(t) => filled.push(t)} />);
    expect(screen.getByTestId('setup.firstrun').textContent).toContain("what's on your mind?");
    fireEvent.click(screen.getByTestId('setup.firstrun.demo'));
    expect(filled).toEqual(['show me what you can do']);
  });

  it('once the composer holds it, the last line says how to go on', () => {
    render(<FirstRun ready onFill={() => undefined} />);
    expect(screen.getByTestId('setup.firstrun.ready').textContent).toBe('⏎ try it · or just type your own');
    expect(screen.queryByTestId('setup.firstrun.demo')).toBeNull();
  });

  it('shows while he said nothing and main is the only agent', () => {
    expect(showsFirstRun(false, 1)).toBe(true);
    expect(showsFirstRun(true, 1)).toBe(false);
    expect(showsFirstRun(false, 2)).toBe(false);
  });
});

describe('the projects step with no repo to show (ambient-lead m_12999)', () => {
  it("an empty filter says there's no repo yet, never 'matches \"\"' or 'clear the filter'; a filter that matches none says so", () => {
    expect(noRepoWords('')).toBe('no repos in this folder. choose a folder…, or continue without one.');
    expect(noRepoWords('   ')).toBe('no repos in this folder. choose a folder…, or continue without one.');
    expect(noRepoWords('bill')).toBe('no repo matches “bill”. choose a folder…, or clear the filter.');
  });
});
