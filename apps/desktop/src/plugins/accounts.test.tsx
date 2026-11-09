// @vitest-environment happy-dom
// A connector's accounts (designer m_57870, m_57934): each row's title is his name for it (its
// connection's name as the sub; no title: the name alone), 'Default' as a chip, its status; an
// expired one has Reconnect in its row; ⋯ renames, makes default, reconnects, removes (asked
// first). 'Connect another account' asks for the row's next_account, waits until the rows list
// it, then names it. Every change goes out as a `plugin_account_*` command.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { AppCmd, Plugin } from '@/proto';
import { settings } from '@/proto/words.gen';
import { AccountsGroup, accountTitle, hasAccounts, menuOf, statusOf } from './AccountsGroup';
import { gives } from './model';

const gmail = (over: Partial<Plugin> = {}): Plugin => ({
  id: 'mistral:gmail', name: 'mistral:gmail', kind: 'mistral', scope: 'mistral', title: 'Gmail', state: 'loaded', on: true, login: 'in',
  accounts: [
    { name: 'default', title: 'gabriel@gmail.com', default: true, status: 'valid' },
    { name: 'account-2', status: 'invalid' },
  ],
  next_account: 'account-3',
  ...over,
});

const row = (n: string) => `plugins.account.mistral:gmail.${n}`;

afterEach(() => {
  cleanup();
  delete (window as { bise?: unknown }).bise;
});

/** window.bise.popupMenu answering `pick` */
function menu(pick: string) {
  (window as { bise?: unknown }).bise = { popupMenu: () => Promise.resolve(pick) };
}

describe('the pure parts', () => {
  it('a title, else the name; the group only on a ready connector with accounts', () => {
    expect(accountTitle({ name: 'account-2', title: ' ', status: 'valid' })).toBe('account-2');
    expect(accountTitle({ name: 'default', title: 'perso', status: 'valid' })).toBe('perso');
    expect(hasAccounts(gmail())).toBe(true);
    expect(hasAccounts(gmail({ state: 'available', on: false, accounts: [], next_account: null }))).toBe(false);
    expect(hasAccounts({ ...gmail(), kind: 'plugin' })).toBe(false);
  });

  it('status words and tone; no Make default on the default', () => {
    expect(statusOf({ name: 'a', status: 'valid' })).toEqual({ words: settings.plugins_account_works, tone: 'ink' });
    expect(statusOf({ name: 'a', status: 'invalid' })).toEqual({ words: settings.plugins_account_expired, tone: 'err' });
    // no status from Mistral is never a failure (designer m_58028)
    expect(statusOf({ name: 'a', status: 'error' })).toEqual({ words: settings.plugins_account_not_checked, tone: 'faint' });
    expect(statusOf({ name: 'a', status: 'unknown' }).tone).toBe('faint');
    expect(menuOf({ name: 'a', default: true, status: 'valid' }).map((i) => 'id' in i ? i.id : '')).toEqual(['rename', 'reconnect', 'remove']);
    expect(menuOf({ name: 'a', status: 'valid' }).map((i) => 'id' in i ? i.id : '')).toEqual(['rename', 'default', 'reconnect', 'remove']);
  });

  it('the list says 2 accounts, never 1', () => {
    expect(gives(gmail())).toContain(settings.plugins_n_accounts({ n: 2 }));
    expect(gives(gmail({ accounts: [{ name: 'default', default: true, status: 'valid' }] }))).not.toContain('account');
  });
});

describe('the Accounts group', () => {
  it('a row per account: title and name, Default, status; Reconnect only on the expired one', () => {
    const sent: AppCmd[] = [];
    const v = render(<AccountsGroup p={gmail()} send={(c) => sent.push(c)} />);
    expect(v.getByTestId(row('default')).textContent).toContain('gabriel@gmail.com');
    expect(v.getByTestId(`${row('default')}.name`).textContent).toBe('default');
    expect(v.getByTestId(`${row('default')}.default`).textContent).toBe(settings.plugins_account_default);
    expect(v.getByTestId(`${row('default')}.status`).textContent).toBe(settings.plugins_account_works);
    expect(v.queryByTestId(`${row('account-2')}.name`)).toBeNull();
    expect(v.queryByTestId(`${row('default')}.reconnect`)).toBeNull();
    fireEvent.click(v.getByTestId(`${row('account-2')}.reconnect`));
    expect(sent).toEqual([{ cmd: 'plugin_account_connect', id: 'mistral:gmail', account: 'account-2' }]);
    expect(v.getByTestId(`${row('account-2')}.waiting`).textContent).toBe(settings.plugins_account_waiting({ service: 'Gmail' }));
  });

  it('⋯ Make default and Rename send their commands; Remove asks in the row first', async () => {
    const sent: AppCmd[] = [];
    const v = render(<AccountsGroup p={gmail()} project="p" send={(c) => sent.push(c)} />);
    menu('default');
    await act(async () => { fireEvent.click(v.getByTestId(`${row('account-2')}.more`)); await Promise.resolve(); });
    expect(sent.at(-1)).toEqual({ cmd: 'plugin_account_default', project: 'p', id: 'mistral:gmail', account: 'account-2' });
    menu('rename');
    await act(async () => { fireEvent.click(v.getByTestId(`${row('account-2')}.more`)); await Promise.resolve(); });
    fireEvent.change(v.getByTestId(`${row('account-2')}.input`), { target: { value: ' work ' } });
    fireEvent.keyDown(v.getByTestId(`${row('account-2')}.input`), { key: 'Enter' });
    expect(sent.at(-1)).toEqual({ cmd: 'plugin_account_rename', project: 'p', id: 'mistral:gmail', account: 'account-2', title: 'work' });
    menu('remove');
    await act(async () => { fireEvent.click(v.getByTestId(`${row('account-2')}.more`)); await Promise.resolve(); });
    const n = sent.length;
    fireEvent.click(v.getByTestId(`${row('account-2')}.ask.no`));
    expect(sent.length).toBe(n);
    await act(async () => { fireEvent.click(v.getByTestId(`${row('account-2')}.more`)); await Promise.resolve(); });
    fireEvent.click(v.getByTestId(`${row('account-2')}.ask.yes`));
    expect(sent.at(-1)).toEqual({ cmd: 'plugin_account_remove', project: 'p', id: 'mistral:gmail', account: 'account-2' });
  });

  it('connect another: its steps, the login for next_account, the wait, then its name', () => {
    const sent: AppCmd[] = [];
    const v = render(<AccountsGroup p={gmail()} send={(c) => sent.push(c)} />);
    fireEvent.click(v.getByTestId('plugins.accounts.mistral:gmail.add'));
    expect(v.getByTestId('plugins.accounts.mistral:gmail.add.steps').textContent).toContain(settings.plugins_account_step_allow);
    fireEvent.click(v.getByTestId('plugins.accounts.mistral:gmail.add.open'));
    expect(sent).toEqual([{ cmd: 'plugin_account_connect', id: 'mistral:gmail', account: 'account-3' }]);
    expect(v.getByTestId('plugins.accounts.mistral:gmail.add.waiting')).toBeTruthy();
    // the rows list it: its name next, pre-filled; it never becomes the default by itself
    const p = gmail();
    v.rerender(<AccountsGroup p={{ ...p, accounts: [...(p.accounts ?? []), { name: 'account-3', status: 'valid' }], next_account: 'account-4' }} send={(c) => sent.push(c)} />);
    const input = v.getByTestId('plugins.accounts.mistral:gmail.add.input');
    // neutral and selected: the connector and its number (designer m_58028)
    expect((input as HTMLInputElement).value).toBe('Gmail 3');
    fireEvent.change(input, { target: { value: 'pro' } });
    fireEvent.click(v.getByTestId('plugins.accounts.mistral:gmail.add.done'));
    expect(sent.at(-1)).toEqual({ cmd: 'plugin_account_rename', id: 'mistral:gmail', account: 'account-3', title: 'pro' });
    expect(sent.some((c) => c.cmd === 'plugin_account_default')).toBe(false);
  });
});
