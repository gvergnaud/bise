// A ready Mistral connector's accounts, the 'Accounts' group at the top of its page (designer
// m_57870, m_57934; page multi-comptes, pick A): one row per account, his name for it as its title
// and the connection's name as its sub (no title: the name alone, one line); at its end 'Default'
// as a quiet chip, its status ('✓ works' in ink, '✗ expired' in err with Reconnect in the row),
// then ⋯ (Rename, Make default, Reconnect, Remove…: a remove asks in the row first, it is final on
// Mistral). Under them, 'Connect another account' opens a sheet: three steps, his browser on
// Mistral's login for the row's `next_account`, 'waiting for Gmail…' until the rows list it, then
// its name (pre-filled, editable) and Done. A new account never becomes the default by itself.
// The rows are the truth: every change is the core's (`plugin_account_*`), the sheet follows them.
// Owner: multi-accounts-build.
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { Button, GroupLabel, Input, Sheet, sentenceCase, tipAttrs } from '@/ds';
import type { AppCmd, ConnectorAccount, Plugin } from '@/proto';
import type { BiseApi, PopupItem } from '@/shared/api';
import { settings } from '@/proto/words.gen';
import { installed, titleOf } from './model';

const bise = (): Partial<BiseApi> | undefined => (window as { bise?: Partial<BiseApi> }).bise;

/** What a row says first: his name for the account, else its connection's name. Pure. */
export const accountTitle = (a: ConnectorAccount): string => (a.title != null && a.title.trim() !== '' ? a.title : a.name);

/** Whether a connector's page has the group: a ready connector Mistral lists accounts for. Pure. */
export const hasAccounts = (p: Plugin): boolean => p.kind === 'mistral' && installed(p) && ((p.accounts ?? []).length > 0 || p.next_account != null);

/** An account's status words and their tone: err only for a failure; no status from Mistral is
 *  faint, never a failure (designer m_58028). Pure. */
export function statusOf(a: ConnectorAccount): { words: string; tone: 'ink' | 'err' | 'faint' } {
  switch (a.status) {
    case 'valid': return { words: settings.plugins_account_works, tone: 'ink' };
    case 'invalid': return { words: settings.plugins_account_expired, tone: 'err' };
    default: return { words: settings.plugins_account_not_checked, tone: 'faint' };
  }
}

/** A new account's name before he gives one: the connector and its number ('Gmail 3'). Pure. */
export const newName = (p: Plugin): string => settings.plugins_account_name_default({ service: titleOf(p), n: (p.accounts ?? []).length });

/** The ⋯ menu's items for an account (no 'Make default' on the default). Pure. */
export function menuOf(a: ConnectorAccount): PopupItem[] {
  return [
    { id: 'rename', label: settings.plugins_account_rename },
    ...(a.default === true ? [] : [{ id: 'default', label: settings.plugins_account_make_default }]),
    { id: 'reconnect', label: settings.plugins_account_reconnect },
    { id: 'remove', label: settings.plugins_account_remove },
  ];
}

type Mode = { kind: 'idle' } | { kind: 'rename' } | { kind: 'remove' } | { kind: 'reconnect' };

function AccountRow({ a, service, testId, send }: { a: ConnectorAccount; service: string; testId: string; send: (cmd: 'connect' | 'default' | 'remove' | { title: string }) => void }) {
  const [mode, setMode] = useState<Mode>({ kind: 'idle' });
  const [title, setTitle] = useState(accountTitle(a));
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => { if (mode.kind === 'rename') field.current?.focus(); }, [mode.kind]);
  // a reconnect waits until the rows say it works again
  const reconnecting = mode.kind === 'reconnect' && a.status !== 'valid';
  const reconnect = () => { send('connect'); setMode({ kind: 'reconnect' }); };
  const more = () => {
    void bise()?.popupMenu?.(menuOf(a)).then((x) => {
      if (x === 'rename') { setTitle(accountTitle(a)); setMode({ kind: 'rename' }); }
      if (x === 'default') send('default');
      if (x === 'reconnect') reconnect();
      if (x === 'remove') setMode({ kind: 'remove' });
    });
  };
  const save = () => {
    if (title.trim() === '') return;
    send({ title: title.trim() });
    setMode({ kind: 'idle' });
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Enter') { e.preventDefault(); save(); }
    if (e.key === 'Escape') { e.preventDefault(); setMode({ kind: 'idle' }); }
  };
  const st = statusOf(a);
  const named = accountTitle(a) !== a.name;
  return (
    <>
      <div className={`plg-row part plg-account${named ? '' : ' one-line'}`} data-testid={testId}>
        <span className="plg-row-words">
          <span className="plg-row-name"><span className="plg-title">{accountTitle(a)}</span></span>
          {named && <span className="plg-row-what" data-testid={`${testId}.name`}>{a.name}</span>}
        </span>
        <span className="plg-row-end">
          {a.default === true && <span className="plg-chip" data-testid={`${testId}.default`}>{settings.plugins_account_default}</span>}
          {reconnecting ? (
            <span className="plg-dim" data-testid={`${testId}.waiting`}>{settings.plugins_account_waiting({ service })}</span>
          ) : (
            <span className={`plg-${st.tone}`} data-testid={`${testId}.status`}
              {...(st.tone === 'faint' ? tipAttrs(settings.plugins_account_not_checked_tip) : {})}>{st.words}</span>
          )}
          {(mode.kind === 'idle' || mode.kind === 'reconnect') && !reconnecting && a.status === 'invalid' && (
            <Button size="sm" onClick={reconnect} testId={`${testId}.reconnect`}>{settings.plugins_account_reconnect}</Button>
          )}
          <Button size="sm" kind="ghost" onClick={more} testId={`${testId}.more`}>⋯</Button>
        </span>
      </div>
      {mode.kind === 'rename' && (
        <div className="plg-row part plg-setting-field" data-testid={`${testId}.field`}>
          <Input ref={field} look="plg-setting-input" value={title} aria-label={settings.plugins_account_rename}
            autoComplete="off" spellCheck={false} onChange={(e) => { setTitle(e.target.value); }} onKeyDown={onKey} data-testid={`${testId}.input`} />
          <span className="plg-acts">
            <Button kind="pri" size="sm" disabled={title.trim() === ''} onClick={save} testId={`${testId}.save`}>{settings.plugins_account_save}</Button>
            <Button kind="ghost" size="sm" onClick={() => { setMode({ kind: 'idle' }); }} testId={`${testId}.cancel`}>{settings.plugins_account_cancel}</Button>
          </span>
        </div>
      )}
      {mode.kind === 'remove' && (
        <div className="plg-row part plg-setting-ask" role="group" data-testid={`${testId}.ask`}>
          <span className="plg-row-words">{settings.plugins_account_remove_ask({ name: accountTitle(a) })}</span>
          <span className="plg-acts">
            <Button kind="danger" size="sm" onClick={() => { setMode({ kind: 'idle' }); send('remove'); }} testId={`${testId}.ask.yes`}>{settings.plugins_account_remove_yes}</Button>
            <Button kind="ghost" size="sm" onClick={() => { setMode({ kind: 'idle' }); }} testId={`${testId}.ask.no`}>{settings.plugins_account_remove_no}</Button>
          </span>
        </div>
      )}
    </>
  );
}

type Add = { kind: 'closed' } | { kind: 'steps' } | { kind: 'waiting'; name: string } | { kind: 'name'; name: string };

/** 'Connect another account': its steps, the wait for the rows to list it, then its name. */
function AddSheet({ add: asked, setAdd, p, service, start, rename }: { add: Add; setAdd: (a: Add) => void; p: Plugin; service: string; start: (name: string) => void; rename: (name: string, title: string) => void }) {
  // null until he types: then the pre-filled name
  const [title, setTitle] = useState<string | null>(null);
  const field = useRef<HTMLInputElement>(null);
  // the rows list the account he logged in: its name next
  const add: Add = asked.kind === 'waiting' && (p.accounts ?? []).some((a) => a.name === asked.name) ? { kind: 'name', name: asked.name } : asked;
  const close = () => { setAdd({ kind: 'closed' }); };
  const open = () => {
    const name = p.next_account;
    if (name == null) return;
    start(name);
    setTitle(null);
    setAdd({ kind: 'waiting', name });
  };
  // its name, pre-filled once it arrives and selected so typing replaces it
  const shown = title ?? newName(p);
  useEffect(() => { if (add.kind === 'name') field.current?.select(); }, [add.kind]);
  const done = () => {
    if (add.kind !== 'name' || shown.trim() === '') return;
    rename(add.name, shown.trim());
    close();
  };
  const tid = `plugins.accounts.${p.id}.add`;
  return (
    <Sheet open={add.kind !== 'closed'} onClose={close} testId={`${tid}.sheet`}
      title={add.kind === 'name' ? settings.plugins_account_name_it : settings.plugins_account_add_title({ service })}
      {...(add.kind === 'name' ? { sub: settings.plugins_account_name_hint } : {})}
      onPrimary={add.kind === 'steps' ? open : add.kind === 'name' ? done : undefined}
      foot={<>
        <Button kind="ghost" onClick={close} testId={`${tid}.cancel`}>{settings.plugins_account_cancel}</Button>
        {add.kind === 'steps' && <Button kind="pri" onClick={open} testId={`${tid}.open`}>{settings.plugins_account_open({ service })}</Button>}
        {add.kind === 'name' && <Button kind="pri" disabled={shown.trim() === ''} onClick={done} testId={`${tid}.done`}>{settings.plugins_account_done}</Button>}
      </>}>
      {add.kind === 'name' ? (
        <Input ref={field} look="plg-setting-input plg-account-name" value={shown} aria-label={settings.plugins_account_name_it} autoComplete="off" spellCheck={false}
          onChange={(e) => { setTitle(e.target.value); }} data-testid={`${tid}.input`} />
      ) : (
        <ol className="plg-steps" data-testid={`${tid}.steps`}>
          <li>{settings.plugins_account_step_open({ service })}<span className="plg-dim">{` ${settings.plugins_account_step_open_hint}`}</span></li>
          <li>{settings.plugins_account_step_allow}</li>
          <li>{settings.plugins_account_step_back}</li>
        </ol>
      )}
      {add.kind === 'waiting' && <p className="plg-dim" data-testid={`${tid}.waiting`}>{settings.plugins_account_waiting({ service })}</p>}
    </Sheet>
  );
}

export function AccountsGroup({ p, project, send }: { p: Plugin; project?: string; send: (cmd: AppCmd) => void }) {
  const [add, setAdd] = useState<Add>({ kind: 'closed' });
  if (!hasAccounts(p)) return null;
  const at = project === undefined ? {} : { project };
  const id = p.id === '' ? p.name : p.id;
  const service = titleOf(p);
  const accounts = p.accounts ?? [];
  const act = (account: string) => (cmd: 'connect' | 'default' | 'remove' | { title: string }) => {
    if (cmd === 'connect') send({ cmd: 'plugin_account_connect', ...at, id, account });
    else if (cmd === 'default') send({ cmd: 'plugin_account_default', ...at, id, account });
    else if (cmd === 'remove') send({ cmd: 'plugin_account_remove', ...at, id, account });
    else send({ cmd: 'plugin_account_rename', ...at, id, account, title: cmd.title });
  };
  return (
    <div className="plg-group" data-testid={`plugins.accounts.${id}`}>
      <GroupLabel count={accounts.length}>{sentenceCase(settings.plugins_accounts_head)}</GroupLabel>
      {accounts.length > 0 && (
        <div className="plg-list">
          {accounts.map((a) => (
            <AccountRow key={a.name} a={a} service={service} testId={`plugins.account.${id}.${a.name}`} send={act(a.name)} />
          ))}
        </div>
      )}
      {p.next_account != null && (
        <div className="plg-account-add">
          <Button onClick={() => { setAdd({ kind: 'steps' }); }} testId={`plugins.accounts.${id}.add`}>{settings.plugins_account_add}</Button>
        </div>
      )}
      <AddSheet add={add} setAdd={setAdd} p={p} service={service}
        start={(name) => { act(name)('connect'); }} rename={(name, title) => { act(name)({ title }); }} />
    </div>
  );
}
