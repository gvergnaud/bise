// The first launch's model step, drawn (pay.ts is its logic; PayStep.tsx: not Pay.tsx, which a case-blind disk takes for pay.ts): the TUI's key step in the window.
// The list (keys found, ChatGPT, OpenRouter, an API key, a coding plan key), a provider list, a
// key field with its keys page, then the live check: checking, failed (why, the provider's own
// words, the page that fixes it) or "it works" with what the model said and what each role runs.
// Owner: amb-home.
import { useRef, type KeyboardEvent, type ReactNode } from 'react';
import type { Account, AppCmd, KeyFrom } from '@/proto';
import { Act, Button, Field, Group, Help, Onb, type GroupItem } from '@/ds';
import { failedWords, fromWords, retries, shortModel, type ProviderCheck } from '@/settings';
import { common, settings } from '@/proto/words.gen';
import { CHATGPT, OPENROUTER, keyFieldSend, keyProviders, openRouterSignIn, phase, pickRow, rows, startCheck, type PayState, type Row } from './pay';
import { hasWebGL } from './island/Island';

/** where the model's name goes in first_works' words (a sentinel no name holds) */
const MODEL_AT = '\u0001';

export interface PayProps {
  /** the Onb frame's step dots and test id */
  frame: { steps: number; step: number; testId: string };
  accounts: readonly Account[];
  /** Claude Code is signed in with a Claude plan: the list says its monthly API credits work here (no Anthropic key yet) */
  claudePlan?: boolean;
  check: ProviderCheck | null;
  s: PayState;
  set: (s: PayState) => void;
  send: (cmd: AppCmd) => void;
  /** the step's button: on to the next step (enabled once a check works) */
  go: () => void;
  /** a link in his browser (keys, billing) */
  open?: (url: string) => void;
  /** the core refused the sign-in he started (error {cmd: sign_in}): its words */
  signInError?: string | null;
  /** '← back' from here (the waiting view's 'cancel'): the sign-in cancelled, back to the list */
  back?: () => void;
}

export function PayStep({ frame, accounts, claudePlan = false, check, s, set, send, go, open, signInError = null, back }: PayProps) {
  const run = (r: { s: PayState; cmd: AppCmd | null }) => {
    if (r.cmd !== null) send(r.cmd);
    set(r.s);
  };
  const ph = phase(s, check);
  const field = useRef<HTMLInputElement>(null);
  const urlField = useRef<HTMLInputElement>(null);
  const name = (id: string) => accounts.find((a) => a.id === id)?.label ?? id;
  const link = (text: string, url: string | null, id: string): ReactNode =>
    url === null ? (
      text
    ) : (
      <>
        {text}
        <Act onClick={() => open?.(url)} testId={`setup.pay.link.${id}`}>
          {url.replace(/^https?:\/\//, '')}
        </Act>
      </>
    );
  const cont = { text: settings.onb_continue, onClick: go, state: ph === 'works' ? undefined : ('disabled' as const), testId: 'setup.next' };
  const v = s.view;

  if (v.t === 'which') {
    // the ones with a key found first, ✓ and where it is; then a 12 px gap and the rest by name, no heads (designer m_52236)
    const { found: withKey, rest } = keyProviders(accounts, v.coding);
    const item = (a: Account): GroupItem => ({
      id: a.id,
      label: a.label,
      ...(a.from != null ? { check: READY, sub: inMono(settings.onb_found_at({ at: fromWords(a.from) }), fromWords(a.from)) } : {}),
      onPick: () => {
        set({ ...s, view: { t: 'key', id: a.id } });
      },
    });
    return (
      <Onb {...frame} title={v.coding ? settings.first_which_plan : settings.first_which_provider} cta={cont}>
        {/* the rest keep their test ids (setup.pay.which.<id>, the as-he-does run's); the found ones setup.pay.which.found.<id> */}
        <div className="setup-list">
          {withKey.length > 0 && <Group list testId="setup.pay.which.found" items={withKey.map(item)} />}
          {rest.length > 0 && (
            <div className={withKey.length > 0 ? 'setup-rest' : undefined}>
              <Group list testId="setup.pay.which" items={rest.map(item)} />
            </div>
          )}
        </div>
      </Onb>
    );
  }

  if (v.t === 'openrouter') {
    return (
      <Onb {...frame} title="OpenRouter" sub={settings.first_openrouter_sub} cta={cont}>
        <span className="setup-acts">
          <Button onClick={() => { run(openRouterSignIn(s)); }} testId="setup.pay.openrouter.signin">
            {settings.first_openrouter_sign_in}
          </Button>
          <Button onClick={() => { set({ ...s, view: { t: 'key', id: OPENROUTER } }); }} testId="setup.pay.openrouter.key">
            {settings.first_paste_a_key}
          </Button>
        </span>
      </Onb>
    );
  }

  // "it works": whichever way it came (a pasted key, a key found, a sign-in)
  if (ph === 'works' && check !== null) {
    const model = check.label != null && check.label !== '' ? check.label : shortModel(check.model ?? '');
    // the model's name whole on one line, never broken inside it (designer m_49293 #4)
    const [before = '', after = ''] = settings.first_works({ model: MODEL_AT }).split(MODEL_AT);
    const title = <>{before}<span className="setup-model">{model}</span>{after}</>;
    const secs = check.ms != null ? (check.ms / 1000).toFixed(1) : null;
    // the island's bubble says the model's words: here only how long it took; no island, the words here
    const bubble = hasWebGL();
    return (
      <Onb {...frame} title={title} cta={cont}>
        <div className="setup-works" data-testid="setup.pay.works">
          {bubble && secs !== null && (
            <p className="setup-said" data-testid="setup.pay.answer">
              <span className="setup-ok">✓</span> {settings.onb_answered_in({ secs })}
            </p>
          )}
          {!bubble && check.answer != null && (
            <p className="setup-said" data-testid="setup.pay.answer">
              <span className="setup-ok">✓</span> {settings.onb_answered({ answer: check.answer })}
              {secs !== null && <span className="setup-took">{settings.onb_took({ secs })}</span>}
            </p>
          )}
          {(check.lines ?? []).map((l) => (
            <p key={l} className="setup-dim">{withIds(l)}</p>
          ))}
        </div>
      </Onb>
    );
  }

  if (v.t === 'key') {
    const acc = accounts.find((a) => a.id === v.id);
    const url = acc?.keys_url != null && acc.keys_url !== '' ? acc.keys_url : null;
    const mine = s.sent?.id === v.id;
    const failed = mine && ph === 'failed' && check !== null ? check : null;
    const f = failed !== null ? failedWords(failed, null, false) : null;
    const needsUrl = acc?.needs_url === true;
    const found = acc?.from != null;
    const resend = () => {
      const r = keyFieldSend(acc, field.current?.value ?? '', urlField.current?.value ?? '');
      if (r !== null && ph !== 'checking') run(startCheck(s, v.id, r.key, check, r.url));
    };
    const enter = (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key !== 'Enter') return;
      e.preventDefault();
      resend();
    };
    // the provider didn't answer: retrying the same key makes sense; a wrong key: he pastes another
    const retry = failed !== null && retries(failed);
    return (
      <Onb
        {...frame}
        title={settings.first_paste_key({ provider: name(v.id) })}
        cta={retry ? { text: settings.onb_try_again, onClick: resend, testId: 'setup.pay.again' } : cont}
      >
        <div className="setup-key">
          {/* a provider with no base URL (the foundry proxy): its URL first, the key found or pasted under it */}
          {needsUrl && (
            <>
              <Help testId="setup.pay.url.why">{settings.first_url_why({ provider: name(v.id) })}</Help>
              <Field ref={urlField} autoFocus placeholder={settings.first_url_paste} state={f !== null ? 'error' : undefined} testId="setup.pay.url" onKeyDown={enter} />
            </>
          )}
          <Field
            ref={field}
            autoFocus={!needsUrl}
            type="password"
            placeholder={found ? settings.first_key_found : settings.key_paste}
            state={f !== null ? 'error' : undefined}
            testId="setup.pay.key"
            onKeyDown={enter}
          />
          {mine && ph === 'checking' && <Help testId="setup.pay.checking">{settings.first_checking_key}</Help>}
          {f !== null && (
            <div data-testid="setup.pay.failed">
              {/* only the ▲ is red: a whole red line shouts at a paste mistake (designer m_28734) */}
              <Help>
                <span className="setup-mark">▲</span> {f.line}
              </Help>
              {f.said !== null && <Help>{f.said}</Help>}
              {f.fix !== null && <Help>{link(f.fix.text, f.fix.url, 'fix')}</Help>}
            </div>
          )}
          {f === null && !(mine && ph === 'checking') && url !== null && <Help testId="setup.pay.get">{link(settings.first_get_one, url, 'keys')}</Help>}
          {f === null && !(mine && ph === 'checking') && <Help>{settings.first_goes_in_auth}</Help>}
          {needsUrl && <Button kind="pri" onClick={resend} testId="setup.pay.url.check">{settings.first_check_it}</Button>}
        </div>
      </Onb>
    );
  }

  if (v.t === 'check') {
    const plan = v.id === CHATGPT;
    const acc = accounts.find((a) => a.id === v.id);
    const from = acc?.from != null ? fromWords(acc.from) : null;
    if (ph === 'signing') {
      return (
        // no 'continue' while he signs in: a dead button; it comes back on the list (designer m_52243)
        <Onb {...frame} title={common.signing_wait({ label: name(v.id) })}>
          {/* a way out where he looks, the terminal's 'esc cancel': what '← back' does from here (designer m_52216) */}
          <p className="setup-cancel">
            <Act onClick={() => { back?.(); }} testId="setup.pay.signin.cancel">
              {settings.cancel}
            </Act>
            <span className="setup-esc">esc</span>
          </p>
          {signInError !== null && <p className="setup-dim err" data-testid="setup.pay.signin.err">{signInError}</p>}
        </Onb>
      );
    }
    if (ph === 'failed' && check !== null) {
      const f = failedWords(check, from, plan);
      // a key found that's wrong: he pastes another; a plan or no answer: the same again
      const cta =
        !plan && !retries(check)
          ? { text: settings.first_paste_a_key, onClick: () => { set({ ...s, view: { t: 'key', id: v.id } }); }, testId: 'setup.pay.paste' }
          : {
              text: settings.onb_try_again,
              onClick: () => {
                run(plan ? pickRow(s, { t: 'chatgpt', acc: acc ?? null }, check) : startCheck(s, v.id, undefined, check));
              },
              testId: 'setup.pay.again',
            };
      return (
        <Onb {...frame} title={f.line} cta={cta}>
          <div className="setup-works" data-testid="setup.pay.failed">
            {f.said !== null && <p className="setup-dim">{f.said}</p>}
            {f.fix !== null && <p className="setup-dim">{link(f.fix.text, f.fix.url, 'fix')}</p>}
          </div>
        </Onb>
      );
    }
    return <Onb {...frame} title={plan ? settings.first_checking_plan : settings.first_checking_key} cta={cont} />;
  }

  // what already works here on top, under its head; the other ways under theirs; nothing found: one list, no heads (designer m_52236)
  const { ready, other } = rows(accounts);
  const item = (r: Row, i: number): GroupItem => {
    const it = rowItem(r, () => { run(pickRow(s, r, check)); });
    if (!ready.includes(r)) return it;
    // a ready row checked in place: 'checking…' or its failure under it, the row stays (the first comes selected: ⏎ takes it)
    const id = r.t === 'found' ? r.acc.id : CHATGPT;
    const mine = s.sent?.id === id;
    const failed = mine && ph === 'failed' && check !== null ? failedWords(check, r.t === 'found' && r.acc.from != null ? fromWords(r.acc.from) : null, r.t === 'chatgpt') : null;
    const sub: ReactNode = mine && ph === 'checking' ? (r.t === 'chatgpt' ? settings.first_checking_plan : settings.first_checking_key) : it.sub;
    return {
      ...it,
      check: READY,
      sel: i === 0 && ph === 'none',
      sub:
        failed === null ? sub : (
          <>
            {sub}
            <span className="setup-row-failed" data-testid={`setup.pay.failed.${id}`}>
              <span className="setup-mark">▲</span> {failed.line}
              {failed.said !== null && <> {failed.said}</>}
              {failed.fix !== null && <> {link(failed.fix.text, failed.fix.url, 'fix')}</>}
            </span>
          </>
        ),
    };
  };
  // Claude Code's plan, no Anthropic key: its monthly API credits, and how a key uses them (the TUI's words)
  const credits = claudePlan && !accounts.some((a) => a.provider === 'anthropic' && a.state === 'signed_in');
  return (
    <Onb {...frame} title={settings.onb_model_title} sub={ready.length > 0 ? settings.onb_found_title : settings.onb_pay_sub} cta={cont}>
      {/* the other ways keep their test ids (setup.pay.<row>, the as-he-does run's); the ready ones setup.pay.ready.<row> */}
      <div className="setup-list">
        {ready.length > 0 && <Group list title={settings.onb_ready_head} testId="setup.pay.ready" items={ready.map(item)} />}
        <Group list title={ready.length > 0 ? settings.onb_other_head : undefined} testId="setup.pay" items={other.map(item)} />
      </div>
      {credits && (
        <div className="setup-works" data-testid="setup.pay.claude">
          <p className="setup-dim">{settings.first_claude_plan}</p>
          <p className="setup-dim setup-then">{settings.first_claude_credits_how}</p>
        </div>
      )}
    </Onb>
  );
}

/** A row of the list: its label, its description (the TUI's words), a click picks it. */
function rowItem(r: Row, pick: () => void): GroupItem {
  switch (r.t) {
    case 'found':
      return { id: `found-${r.acc.id}`, label: r.acc.label, sub: r.acc.from != null ? foundWords(r.acc.from) : settings.first_found, onPick: pick };
    case 'chatgpt':
      // signed in, it already works: its own name, who and which plan (designer m_52236)
      if (r.acc?.state === 'signed_in') {
        return { id: 'chatgpt', label: r.acc.label, sub: r.acc.who != null ? settings.first_signed_in({ email: r.acc.who, plan: r.acc.label }) : undefined, onPick: pick };
      }
      return { id: 'chatgpt', label: settings.first_chatgpt, sub: settings.first_chatgpt_sub, onPick: pick };
    case 'openrouter':
      return { id: 'openrouter', label: 'OpenRouter', sub: settings.first_openrouter_way, onPick: pick };
    case 'api':
      return { id: 'api', label: settings.api_key, sub: settings.first_api_key_sub, onPick: pick };
    case 'coding':
      return { id: 'coding', label: settings.first_coding, sub: settings.first_coding_sub, onPick: pick };
  }
}

/** A ready row's mark, in its lead slot: ✓ in ink, never the accent (pink waits on him; this works: designer m_52236). */
const READY = <span className="setup-ready" aria-hidden="true">✓</span>;

/** Where a found key comes from, its place in mono: "from ANTHROPIC_API_KEY in your shell", "from ~/.env". */
function foundWords(f: KeyFrom): ReactNode {
  const where = fromWords(f);
  return inMono(f.t === 'env' ? settings.onb_found_from({ var: where }) : settings.onb_found_at({ at: where }), where);
}

/** `text` with `part` in mono (a variable, a path). */
function inMono(text: string, part: string): ReactNode {
  const i = text.indexOf(part);
  if (part === '' || i < 0) return text;
  return (
    <>
      {text.slice(0, i)}
      <code>{part}</code>
      {text.slice(i + part.length)}
    </>
  );
}

/** A dim line with its model ids in mono ("main uses <code>anthropic/claude-opus-5-5</code>."). */
function withIds(line: string): ReactNode {
  const parts = line.split(/([\w.-]+\/[\w./-]*[\w-]|[a-z][\w.]*-[\w.-]*\d[\w-]*)/);
  return parts.map((p, i) => (i % 2 === 1 ? <code key={i}>{p}</code> : p));
}
