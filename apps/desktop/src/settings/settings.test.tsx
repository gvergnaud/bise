// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { Account, AppCmd, Found, Model, ProjectRow, RoleRow, SetupGauge } from '@/proto';
import type { CoreCmd } from '@/shared/api';
import { heldReady, modelOrSetup, needsSetup, releaseHeld, type Held } from './hold';
import { Approvals, checkerWords, ruleNote, ruleWhat } from './approvals';
import { Accounts, expiredWords } from './accounts';
import type { ApprovalsCmd, ApprovalsEv } from './approvals';
import type { ApprovalRule } from '@/proto';
import { core_ev, hub_ev } from '@/proto/fixtures.gen';
import { Store, StoreProvider } from '@/store';
import { Settings, type SettingsPorts } from './Settings';
import { appName, excludedApps, flag, getPref, knownApps, onboarded, putPref, type Prefs } from './prefs';
import { addPaths, offers, ordered, projectSub } from './projects';
import { KEY_SECTIONS } from './keys';
import { notifyRow } from './pages';
import { capsOf, chromeCase, homeOf } from './chrome';
import { stepOf } from './computer';
import { focusOf } from './setupPage';
import { fixButton, TRY_MS } from './gauge';
import { article } from './keycheck';
import { FALLBACK, fixLine, joinAnd } from './roleList';
import type { ComputerUse } from './computer';
import { settings } from '@/proto/words.gen';

/** a role's model picker (the ds Select): open it, then pick that option */
function pickRole(role: string, value: string): void {
  const id = `settings.role.${role}.model`;
  fireEvent.click(screen.getByTestId(id));
  fireEvent.click(screen.getByTestId(`${id}.menu.${value}`));
}

afterEach(cleanup);

const ev = <T extends (typeof core_ev)[number]['ev']>(t: T) => {
  const e = core_ev.find((x) => x.ev === t);
  if (e?.ev !== t) throw new Error(`no ${t} fixture`);
  return e as Extract<(typeof core_ev)[number], { ev: T }>;
};
const prefs = (): Prefs => {
  const p = ev('prefs').prefs;
  if (p === null || typeof p !== 'object' || Array.isArray(p)) throw new Error('prefs');
  return { ...p, app_names: { 'com.apple.MobileSMS': 'Messages', 'com.bitwarden.desktop': 'Bitwarden' } };
};
const accounts = (): Account[] => ev('accounts').items;
// the last projects line: the one a replay of the fixtures ends with (the first has a down hub)
const projects = (): ProjectRow[] => {
  const e = core_ev.filter((x) => x.ev === 'projects').at(-1);
  if (e?.ev !== 'projects') throw new Error('no projects fixture');
  return e.projects;
};
const found = (): Found[] => ev('found').items;

const ports = (): SettingsPorts & { sent: AppCmd[]; went: string[] } => {
  const sent: AppCmd[] = [];
  const went: string[] = [];
  return { sent, went, send: (c) => sent.push(c), back: () => went.push('back'), go: (p) => went.push(p) };
};

const view = (page: string, p = ports(), extra: Partial<Parameters<typeof Settings>[0]> = {}, store?: Store) => {
  const node = <Settings page={page} prefs={prefs()} accounts={accounts()} projects={projects()} found={found()} computerUse={null} perms={{}} ports={p} {...extra} />;
  return render(store === undefined ? node : <StoreProvider store={store}>{node}</StoreProvider>);
};

/** A store with the fixtures' projects and found (the add sheet reads the store's own). */
const storeWith = (): Store => {
  const s = new Store(null);
  s.apply({ ev: 'projects', projects: projects() });
  s.apply(ev('found'));
  return s;
};

describe('prefs', () => {
  it('reads and puts dotted keys without touching the rest', () => {
    const p = prefs();
    expect(getPref(p, 'quiet.batch_min')).toBe(20);
    expect(getPref(p, 'nope.x')).toBeUndefined();
    const q = putPref(p, 'quiet.call', false);
    expect(getPref(q, 'quiet.call')).toBe(false);
    expect(getPref(q, 'quiet.batch_min')).toBe(20);
    expect(getPref(p, 'quiet.call')).toBe(true);
    expect(flag(null, 'context.shot')).toBe(true);
    expect(onboarded(null)).toBe(false);
    expect(onboarded(p)).toBe(true);
  });

  it("names the apps from the core's table, else the id's last part", () => {
    const p = prefs();
    expect(appName(p, 'com.apple.MobileSMS')).toBe('Messages');
    expect(appName(p, 'com.example.Notes')).toBe('Notes');
    expect(excludedApps(p)).toEqual(['1Password']);
    expect(knownApps(p)).toEqual(['com.apple.MobileSMS', 'com.bitwarden.desktop', '1Password']);
  });
});

describe('projects', () => {
  it('orders his projects without bise, offers found repos not his yet, adds the typed path once', () => {
    expect(ordered(projects()).map((p) => p.name)).toEqual(['acme']);
    expect(offers(found(), projects()).map((f) => f.name)).toEqual(['shop']);
    expect(addPaths(['/a'], ' /b ')).toEqual(['/a', '/b']);
    expect(addPaths(['/a'], '/a')).toEqual(['/a']);
  });
  it("a row says why its hub couldn't start, never a silent row (architect m_18354)", () => {
    const [p] = ordered(projects());
    if (p === undefined) throw new Error('no project');
    expect(projectSub(p)).toBe(p.path);
    const why = 'the hub did not start: sb-core: no such file (its log: /h/hub.err)';
    expect(projectSub({ ...p, down: why })).toBe(`${p.path} · ${why}`);
    expect(projectSub({ ...p, down: why, missing: true }), 'a gone folder says that first').toBe(`${p.path} · its folder is gone`);
  });
});

describe('Settings', () => {
  it('a switch sends prefs_set and flips at once', () => {
    const p = ports();
    view('quiet', p);
    const t = screen.getByTestId('settings.pref.quiet.call');
    expect(t.getAttribute('aria-checked')).toBe('true');
    fireEvent.click(t);
    expect(p.sent).toEqual([{ cmd: 'prefs_set', key: 'quiet.call', value: false }]);
    expect(screen.getByTestId('settings.pref.quiet.call').getAttribute('aria-checked')).toBe('false');
  });

  it("his bim's look (page bim-pick): general shows it first; a pick is kept in prefs' bim at once, a hat the hair can't take greyed", () => {
    const p = ports();
    view('general', p);
    expect(screen.getByTestId('settings.me')).toBeTruthy();
    // never picked: today's main (the light skin, short dark hair)
    expect(screen.getByTestId('settings.me.skin.1').getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByTestId('settings.me.hair.short').getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByTestId('settings.me.skin.5'));
    expect(p.sent.at(-1)).toEqual({ cmd: 'prefs_set', key: 'bim', value: { skin: 5, hair: 'short', color: 1, wear: 'none', hat: 0 } });
    expect(screen.getByTestId('settings.me.skin.5').getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByTestId('settings.me.wear.cap'));
    fireEvent.click(screen.getByTestId('settings.me.hair.pigtails'));
    // pigtails take no hat: the cap goes, the hats grey
    expect(p.sent.at(-1)).toEqual({ cmd: 'prefs_set', key: 'bim', value: { skin: 5, hair: 'pigtails', color: 1, wear: 'none', hat: 0 } });
    expect(screen.getByTestId<HTMLButtonElement>('settings.me.wear.cap').disabled).toBe(true);
    fireEvent.click(screen.getByTestId('settings.me.color.5'));
    expect(p.sent.at(-1)).toEqual({ cmd: 'prefs_set', key: 'bim', value: { skin: 5, hair: 'pigtails', color: 5, wear: 'none', hat: 0 } });
  });

  it("his bim's accessory colour (bim-hat-colour): greyed with nothing on the head, kept in prefs' bim once he wears one", () => {
    const p = ports();
    view('general', p);
    const dot = (i: number) => screen.getByTestId<HTMLButtonElement>(`settings.me.hat.${String(i)}`);
    // nothing on the head: the row stays, its dots greyed, none ringed, and its tip says why
    expect(dot(0).getAttribute('aria-pressed')).toBe('false');
    expect(dot(3).disabled).toBe(true);
    expect(screen.getByTestId('settings.me.hat').getAttribute('data-tip')).toBe(settings.me_hat_no);
    fireEvent.click(screen.getByTestId('settings.me.wear.beanie'));
    // a hat on: today's sky blue ringed again
    expect(dot(0).getAttribute('aria-pressed')).toBe('true');
    expect(dot(3).disabled).toBe(false);
    expect(screen.getByTestId('settings.me.hat').getAttribute('data-tip')).toBeNull();
    fireEvent.click(dot(3));
    expect(p.sent.at(-1)).toEqual({ cmd: 'prefs_set', key: 'bim', value: { skin: 1, hair: 'short', color: 1, wear: 'beanie', hat: 3 } });
    expect(dot(3).getAttribute('aria-pressed')).toBe('true');
    expect(dot(0).getAttribute('aria-pressed')).toBe('false');
  });

  it('esc goes back; an unknown page is general', () => {
    const p = ports();
    view('nope', p);
    expect(screen.getByTestId('settings.page.general')).toBeTruthy();
    fireEvent.keyDown(screen.getByTestId('settings.root'), { key: 'Escape' });
    expect(p.went).toEqual(['back']);
  });

  it("general: macOS notifications' row asks when never asked, opens System Settings when off or asked, says on, and is gone without the api", () => {
    expect(notifyRow(null)).toBeNull();
    expect(notifyRow('never')?.act).toBe('ask');
    expect(notifyRow('asked')?.act).toBe('open');
    expect(notifyRow('off')).toEqual({ sub: 'off in macOS: questions wait in bise until you look', act: 'open', button: 'open System Settings' });
    expect(notifyRow('on')?.act).toBeNull();
    const acts: string[] = [];
    const p = { ...ports(), notify: (a: 'ask' | 'open') => acts.push(a) };
    view('general', p, { notify: 'off' });
    fireEvent.click(screen.getByTestId('settings.notify.open'));
    cleanup();
    view('general', p, { notify: 'never' });
    fireEvent.click(screen.getByTestId('settings.notify.ask'));
    expect(acts).toEqual(['open', 'ask']);
    cleanup();
    view('general', p, { notify: 'on' });
    expect(screen.getByTestId('settings.notify.on').textContent).toBe('on');
    cleanup();
    view('general', p);
    expect(screen.queryByTestId('settings.notify.sub')).toBeNull();
  });

  it('privacy: an excluded app toggles in the list of ids', () => {
    const p = ports();
    view('privacy', p);
    fireEvent.click(screen.getByTestId('settings.app.com.apple.MobileSMS'));
    // the page also asks the usage counts' view once (telemetry.tsx): not this list's
    expect(p.sent.filter((c) => c.cmd !== 'telemetry')).toEqual([{ cmd: 'prefs_set', key: 'excluded_apps', value: ['1Password', 'com.apple.MobileSMS'] }]);
  });

  it('models: sign in on his click only, a key is sent once and never shown', () => {
    const p = ports();
    view('models', p);
    fireEvent.click(screen.getByTestId('settings.fold.accounts.head'));
    expect(screen.getByTestId('settings.account.chatgpt.ok')).toBeTruthy();
    expect(p.sent).toEqual([]);
    fireEvent.click(screen.getByTestId('settings.key.add'));
    const f = screen.getByTestId<HTMLInputElement>('settings.key.field');
    expect(f.type).toBe('password');
    f.value = 'sk-test';
    fireEvent.keyDown(f, { key: 'Enter' });
    expect(p.sent).toEqual([{ cmd: 'provider_use', id: 'anthropic-key', key: 'sk-test' }]);
    // the field stays while the core checks, and a second ⏎ sends nothing more
    expect(screen.getByTestId('settings.key.help').textContent).toMatch(/checking/);
    fireEvent.keyDown(f, { key: 'Enter' });
    expect(p.sent.length).toBe(1);
  });

  it('S.2: a key the live check refuses says so in place, the field kept; the key that works closes it', () => {
    const p = ports();
    const r = view('models', p);
    fireEvent.click(screen.getByTestId('settings.fold.accounts.head'));
    fireEvent.click(screen.getByTestId('settings.key.add'));
    const f = screen.getByTestId<HTMLInputElement>('settings.key.field');
    f.value = 'sk-wrong';
    fireEvent.keyDown(f, { key: 'Enter' });
    const again = (extra: Partial<Parameters<typeof Settings>[0]>) => {
      r.rerender(<Settings page="models" prefs={prefs()} accounts={extra.accounts ?? accounts()} projects={projects()} found={found()} computerUse={null} perms={{}} ports={p} {...extra} />);
    };
    const refused = { ev: 'provider_check', id: 'anthropic-key', name: 'Anthropic', state: 'failed', model: 'anthropic/claude-opus-5-5', pasted: true, why: 'wrong_key', said: 'invalid x-api-key' } as const;
    again({ check: refused });
    const help = screen.getByTestId('settings.key.help');
    expect(help.textContent).toBe('Anthropic says this key is wrong: “invalid x-api-key”. check it, then ⏎ again');
    expect(help.getAttribute('role')).toBe('alert');
    expect(screen.getByTestId<HTMLInputElement>('settings.key.field').value).toBe('sk-wrong');
    expect(document.body.textContent).not.toContain('sk-wrong');
    // he fixes it and sends again: one more provider_use, checking until its answer
    const g = screen.getByTestId<HTMLInputElement>('settings.key.field');
    fireEvent.change(g, { target: { value: 'sk-right' } });
    fireEvent.keyDown(g, { key: 'Enter' });
    expect(p.sent.filter((c) => c.cmd === 'provider_use').length).toBe(2);
    expect(screen.getByTestId('settings.key.help').textContent).toMatch(/checking/);
    // it works: the field goes, the row says added
    again({ check: { ...refused, state: 'works', why: undefined, said: undefined, main: 'anthropic/claude-opus-5-5' }, accounts: accounts().map((a) => (a.id === 'anthropic-key' ? { ...a, state: 'signed_in' } : a)) });
    expect(screen.queryByTestId('settings.key.field')).toBeNull();
  });

  it('a subscription signed out signs in on click, with a spinner until the core answers', () => {
    const p = ports();
    const signedOut = accounts().map((a) => ({ ...a, state: 'signed_out' as const }));
    view('models', p, { accounts: signedOut });
    fireEvent.click(screen.getByTestId('settings.fold.accounts.head'));
    fireEvent.click(screen.getByTestId('settings.account.chatgpt'));
    expect(p.sent).toEqual([{ cmd: 'sign_in', id: 'chatgpt' }]);
    expect(screen.getByTestId('settings.account.chatgpt').getAttribute('aria-busy')).toBe('true');
  });

  it("projects: one line each, the ⌘N key at the row's right, never a box over the name (his shot, designer m_41841)", () => {
    view('projects', ports(), { home: '/Users/camille' }, storeWith());
    const row = screen.getByTestId('settings.projects.acme-1a2b3c4d');
    const key = [...row.querySelectorAll('.b-kbd')];
    expect(key.map((k) => k.textContent)).toEqual(['⌘1']);
    // the key sits in the row's end, beside the ⋯, never in the name's column (where it stretched)
    expect(key[0]?.parentElement?.classList.contains('sp-end')).toBe(true);
    expect(row.querySelector('.sp-name .b-kbd')).toBeNull();
    // its folder said short, the whole one in the row's tip
    expect(screen.getByTestId('settings.project.acme-1a2b3c4d.path').textContent).toBe('~/acme');
    expect(screen.getByTestId('settings.project.acme-1a2b3c4d.path').getAttribute('data-tip')).toBe('/Users/camille/acme');
  });

  it('projects: remove asks once; rename; add a found repo with land', () => {
    const p = ports();
    view('projects', p, {}, storeWith());
    // remove and rename sit in the row's ⋯ menu (designer m_41841)
    fireEvent.click(screen.getByTestId('settings.project.acme-1a2b3c4d.more'));
    fireEvent.click(screen.getByTestId('settings.project.acme-1a2b3c4d.remove'));
    expect(p.sent).toEqual([]);
    fireEvent.click(screen.getByTestId('settings.project.acme-1a2b3c4d.remove.yes'));
    fireEvent.click(screen.getByTestId('settings.project.acme-1a2b3c4d.more'));
    fireEvent.click(screen.getByTestId('settings.project.acme-1a2b3c4d.rename'));
    const n = screen.getByTestId<HTMLInputElement>('settings.project.acme-1a2b3c4d.name');
    n.value = 'engine';
    fireEvent.keyDown(n, { key: 'Enter' });
    fireEvent.click(screen.getByTestId('settings.project.add'));
    fireEvent.click(screen.getByTestId('settings.add.found.shop'));
    fireEvent.click(screen.getByTestId('settings.add.land'));
    fireEvent.click(screen.getByTestId('settings.add.go'));
    expect(p.sent).toEqual([
      { cmd: 'project_remove', project: 'acme-1a2b3c4d' },
      { cmd: 'project_rename', project: 'acme-1a2b3c4d', name: 'engine' },
      // the sheet looks in his folders again at each open
      { cmd: 'found_scan' },
      { cmd: 'project_add', path: '/Users/camille/code/shop', land: true },
    ]);
  });

  it('the add sheet: its chrome in sentence case, the scan head never in capitals (designer m_43052, ui-review m_45318)', () => {
    view('projects', ports(), {}, storeWith());
    fireEvent.click(screen.getByTestId('settings.project.add'));
    const sheet = screen.getByTestId('settings.add');
    expect(sheet.textContent).toContain('Add a project');
    expect(screen.getByTestId('settings.add.cancel').textContent).toBe('Cancel');
    expect(screen.getByTestId('settings.add.go').textContent).toMatch(/^Add\b/u);
    // a state's words, sentence-cased as the head: 'Looking in your folders…', 'Found in your folders · 2'
    const head = screen.getByTestId('settings.add.scan').textContent;
    expect(head.charAt(0)).toBe(head.charAt(0).toUpperCase());
    expect(head).not.toBe(head.toUpperCase());
    expect(screen.getByTestId('settings.add.scan').className).toBe('settings-add-scan');
  });

  it('keys: every row of the table shows, no key twice in a section', () => {
    view('keys');
    for (const s of KEY_SECTIONS) {
      const g = screen.getByTestId(`settings.keys.${s.id}`);
      // row titles in sentence case (designer m_43882)
      for (const r of s.rows) expect(g.textContent).toContain(chromeCase(r.action));
      expect(new Set(s.rows.map((r) => r.keys)).size).toBe(s.rows.length);
    }
  });
});

describe('chrome words (designer m_43882)', () => {
  it('a row title or a button takes a capital, the brand never', () => {
    expect(chromeCase('accessibility')).toBe('Accessibility');
    expect(chromeCase('open chrome://extensions')).toBe('Open chrome://extensions');
    expect(chromeCase('bise')).toBe('bise');
    expect(chromeCase('bise Computer Use')).toBe('bise Computer Use');
    expect(chromeCase('')).toBe('');
  });
  it('one keycap per key, the separators between them (designer m_43941)', () => {
    expect(capsOf('⌘K')).toEqual([{ text: '⌘K', sep: false }]);
    expect(capsOf('⌘[ / ⌘]').map((c) => c.text)).toEqual(['⌘[', '/', '⌘]']);
    expect(capsOf('⌥0 / ⌥1 … ⌥9').map((c) => (c.sep ? `(${c.text})` : c.text))).toEqual(['⌥0', '(/)', '⌥1', '(…)', '⌥9']);
  });
  it("a numbered help step's number goes to the mark column, its words hang", () => {
    expect(stepOf('1  ⏎ opens chrome://extensions')).toEqual({ n: '1', text: '⏎ opens chrome://extensions' });
    expect(stepOf("turn on bise Computer Use in the list. i'll see it")).toEqual({ n: '', text: "turn on bise Computer Use in the list. i'll see it" });
  });
  it('paths say ~ from the shell home, else from the folder holding bise\'s home project', () => {
    expect(homeOf('/Users/c', ['/Users/c/acme'], '/Users/c/bise')).toBe('/Users/c');
    expect(homeOf('/tmp/test-home', ['/Users/camille/acme'], '/Users/camille/bise')).toBe('/Users/camille');
    expect(homeOf(undefined, ['/Users/camille/acme'], undefined)).toBeUndefined();
  });
});

describe('computer use', () => {
  const cu = () => {
    const e = core_ev.find((x) => x.ev === 'computer_use' && x.on);
    if (e?.ev !== 'computer_use') throw new Error('fixture');
    return e;
  };

  it('checks while open, leaves when closed; a fix only on his click; on and off', () => {
    const p = ports();
    const { unmount } = view('computer', p, { computerUse: cu() });
    expect(p.sent).toEqual([{ cmd: 'computer_use', act: 'check' }]);
    expect(screen.getByTestId('settings.cu.web').textContent).toContain('add bise to Chrome');
    expect(screen.getByTestId('settings.cu.apps').textContent).toContain('Accessibility');
    expect(screen.queryByTestId('settings.cu.browser')).toBeNull();
    // every step's mark is a line icon (designer m_43882): no ?, ✓ or · glyph
    for (const g of ['settings.cu.web', 'settings.cu.apps']) {
      for (const st of screen.getByTestId(g).querySelectorAll('.settings-cu-st')) expect(st.querySelector('svg')).not.toBeNull();
    }
    // the core's flash never shows at rest; 3 s under the button he clicked
    expect(screen.queryByTestId('settings.cu.extension.flash')).toBeNull();
    fireEvent.click(screen.getByTestId('settings.cu.extension'));
    expect(screen.getByTestId('settings.cu.extension.flash').textContent).toBe(cu().flash);
    fireEvent.click(screen.getByTestId('settings.cu.switch'));
    unmount();
    expect(p.sent.slice(1)).toEqual([
      { cmd: 'computer_use', act: 'fix', fix: 'add_extension' },
      { cmd: 'computer_use', act: 'off' },
      { cmd: 'computer_use', act: 'leave' },
    ]);
  });

  // the screen (screen.tsx) hands the page a new `send` on each of its renders, and it renders on
  // every core event: the page checks once while it is open, it never leaves and checks again
  it('a re-render with a new send neither leaves nor checks again (the stuck check)', () => {
    const sent: AppCmd[] = [];
    const draw = () => <Settings page="computer" prefs={prefs()} accounts={accounts()} projects={projects()} found={found()} computerUse={{ ...cu() }} perms={{}} ports={{ ...ports(), send: (c) => sent.push(c) }} />;
    const { rerender } = render(draw());
    for (let i = 0; i < 3; i++) rerender(draw());
    expect(sent).toEqual([{ cmd: 'computer_use', act: 'check' }]);
  });

  // a stand-in for the core's acts (ambient/core/setup.rs computer_use): `leave` drops what it
  // checked, a `check` with nothing open starts over with no steps; each act answers the page
  // (a new event, so a new render of the screen, so a new send)
  it('the steps, once checked, stay drawn while the screen re-renders (the flicker)', () => {
    let open = false;
    let rows: ComputerUse['rows'] = [];
    const queue: ComputerUse[] = [];
    const answer = () => queue.push({ ev: 'computer_use', on: true, ready: false, rows });
    const send = (c: AppCmd) => {
      if (c.cmd !== 'computer_use') return;
      if (c.act === 'leave') open = false;
      if (c.act === 'check' && !open) {
        open = true;
        rows = [];
      }
      if (c.act !== 'leave') answer();
    };
    const draw = (e: ComputerUse) => <Settings page="computer" prefs={prefs()} accounts={accounts()} projects={projects()} found={found()} computerUse={e} perms={{}} ports={{ ...ports(), send: (c) => { send(c); } }} />;
    const { rerender } = render(draw({ ev: 'computer_use', on: true, ready: false, rows: [] }));
    const settle = () => {
      for (let n = 0; n < 10 && queue.length > 0; n++) rerender(draw(queue.shift() as ComputerUse));
    };
    settle();
    // the poll's first answer: the steps
    rows = cu().rows;
    answer();
    settle();
    expect(screen.getByTestId('settings.cu.web').textContent).toContain('add bise to Chrome');
    // any other event re-renders the screen
    rerender(draw({ ev: 'computer_use', on: true, ready: false, rows }));
    settle();
    expect(screen.queryByTestId('settings.cu.web')?.textContent).toContain('add bise to Chrome');
    expect(screen.queryByText(settings.cu_checking)).toBeNull();
  });

  it('off: no steps, the switch turns it on', () => {
    const p = ports();
    view('computer', p, { computerUse: { ev: 'computer_use', on: false, ready: false, rows: [] } });
    expect(screen.queryByTestId('settings.cu.web')).toBeNull();
    fireEvent.click(screen.getByTestId('settings.cu.switch'));
    expect(p.sent.at(-1)).toEqual({ cmd: 'computer_use', act: 'on' });
  });
});

describe('settings · Setup: which model does what (page models-settings, pick A)', () => {
  const gauge = () => ev('setup_gauge').gauge;
  const allOn = (): SetupGauge => ({ ...gauge(), on: 7, next: undefined, testing: [], rows: gauge().rows.map((r) => ({ ...r, state: 'on' as const, fix: undefined, parts: r.parts.map((x) => ({ ...x, state: 'on' as const, why: undefined, said: undefined })) })) });

  it('the test world: one list, first; Voice mode stopped, its fix on top, each part its status; approvals off with its own fix', () => {
    const p = ports();
    view('setup', p, { gauge: { ...gauge(), testing: [] } });
    // the list is the page's first thing, one row per role in the core's order
    expect(screen.getByTestId('settings.page.setup').querySelector('[data-testid]')?.getAttribute('data-testid')).toBe('settings.roles');
    expect([...document.querySelectorAll('.st-feature')].map((e) => e.getAttribute('data-testid'))).toEqual(
      ['talk', 'agents', 'small', 'vision', 'dictation', 'voice_mode', 'approvals'].map((id) => `settings.feature.${id}`),
    );
    expect(screen.getByTestId('settings.feature.talk').textContent).toContain(settings.role_main_name + settings.about_talk);
    // the fix line: what stopped, and the one button (the row doesn't repeat it)
    expect(screen.getByTestId('settings.fixline').textContent).toContain(settings.fixline_stopped({ what: settings.short_voice_mode, why: settings.why_no_credit({ provider: 'Mistral' }) }));
    expect(screen.getByTestId('settings.fixline.fix').textContent).toBe(settings.fix_credit({ provider: 'Mistral' }));
    expect(screen.getAllByText(settings.fix_credit({ provider: 'Mistral' }))).toHaveLength(1);
    // Voice mode: hears / thinks / speaks, each its model and status; no wall of red
    expect(screen.getByTestId('settings.feature.voice_mode.voice.model').textContent).toContain('whisper-large-v3');
    expect(screen.getByTestId('settings.feature.voice_mode.main.model').textContent).toContain(settings.part_same_as_bise);
    expect(screen.getByTestId('settings.feature.voice_mode.tts.mark').textContent).toBe(settings.stop_no_credit);
    expect(screen.queryByTestId('settings.feature.voice_mode.trouble')).toBeNull();
    expect(screen.getByTestId('settings.feature.approvals.mark').textContent).toBe(settings.mark_off);
    // nothing tested on open: a test is his click
    expect(p.sent).toEqual([]);
    fireEvent.click(screen.getByTestId('settings.gauge.test'));
    expect(p.sent).toEqual([{ cmd: 'setup_test' }]);
    // a stopped role's status is a link: it opens the troubleshooting under the row
    fireEvent.click(screen.getByTestId('settings.feature.voice_mode.tts.mark'));
    expect(screen.getByTestId('settings.feature.voice_mode.trouble').textContent).toContain('No credit left on this workspace');
    fireEvent.click(screen.getByTestId('settings.feature.voice_mode.trouble.again'));
    expect(p.sent.at(-1)).toEqual({ cmd: 'setup_test', models: ['groq/whisper-large-v3', 'mistral/voxtral-mini-tts-2603', 'chatgpt/gpt-6.1-sol'] });
    fireEvent.click(screen.getByTestId('settings.feature.approvals.fix'));
    expect(p.sent.at(-1)).toEqual({ cmd: 'role_set', role: 'classify' });
  });

  it("images run bise's model: its name, 'same as bise', never a button", () => {
    view('setup', ports(), { gauge: gauge() });
    const cell = screen.getByTestId('settings.feature.vision.model');
    expect(cell.textContent).toBe(`gpt-6.1-sol${settings.part_same_as_bise}`);
    expect(cell.getAttribute('data-tip')).toBe(settings.tip_same_as_bise);
    expect(cell.closest('button')).toBeNull();
  });

  it('a permission shows only when it is missing', () => {
    const withMic = (grant: 'allowed' | 'denied'): SetupGauge => ({
      ...allOn(),
      rows: allOn().rows.map((r) => (r.id === 'dictation' ? { ...r, state: grant === 'allowed' ? ('on' as const) : ('failed' as const), perms: [{ perm: 'mic' as const, grant }] } : r)),
    });
    const r = view('setup', ports(), { gauge: withMic('allowed') });
    expect(screen.getByTestId('settings.feature.dictation').textContent).not.toContain(settings.onb_perm_mic);
    r.unmount();
    view('setup', ports(), { gauge: withMic('denied') });
    expect(screen.getByTestId('settings.feature.dictation.mark').textContent).toBe(settings.stop_perm({ perm: settings.onb_perm_mic }));
  });

  it('copy report has the providers\' words and never a key', () => {
    const copied: string[] = [];
    view('setup', { ...ports(), copy: (t) => copied.push(t) }, { gauge: gauge() });
    fireEvent.click(screen.getByTestId('settings.feature.voice_mode.tts.mark'));
    fireEvent.click(screen.getByTestId('settings.feature.voice_mode.trouble.copy'));
    expect(copied[0]).toContain('no_credit');
    expect(copied[0]).not.toMatch(/sk-|Bearer/);
  });

  it("an expired OpenRouter key: one fix line on top, '✗ key expired' on the row; Replace opens its field with the steps; a key that works tests the row again", () => {
    const JEV = 'openrouter/typesafe/jev-1.13';
    const KEYS = 'https://openrouter.ai/settings/keys';
    const failed: SetupGauge = {
      ...gauge(),
      testing: [],
      rows: gauge().rows.map((r) =>
        r.id === 'approvals'
          ? { id: 'approvals', state: 'failed' as const, fix: { t: 'add_key' as const, provider: 'openrouter', url: KEYS }, parts: [{ model: JEV, name: 'jev-1.13', provider: 'OpenRouter', role: 'classify' as const, source: 'config' as const, state: 'failed' as const, why: 'wrong_key' as const, said: 'API key expired.' }] }
          : r,
      ),
    };
    // his key is there (it expired at OpenRouter): the accounts say it's set
    const acc: Account[] = [...accounts(), { id: 'openrouter', kind: 'key', label: 'OpenRouter', state: 'signed_in', provider: 'openrouter' }];
    const opened: string[] = [];
    const p = { ...ports(), open: (u: string) => opened.push(u) };
    const failedNext: SetupGauge = { ...failed, next: { fix: { t: 'add_key', provider: 'openrouter', url: KEYS }, unlocks: ['approvals'] } };
    const r = view('setup', p, { gauge: failedNext, accounts: acc });
    expect(screen.getByTestId('settings.fixline').textContent).toContain(settings.fixline_key({ provider: 'OpenRouter', what: settings.short_approvals }));
    expect(screen.getByTestId('settings.feature.approvals.mark').textContent).toBe(settings.stop_wrong_key);
    expect(screen.queryByTestId('settings.feature.approvals.fix')).toBeNull();
    const add = screen.getByTestId('settings.fixline.fix');
    expect(add.textContent).toBe('Replace the OpenRouter key');
    fireEvent.click(add);
    expect(screen.getByTestId('settings.key.field')).toBeTruthy();
    const guide = screen.getByTestId('settings.key.guide').textContent;
    expect(guide).toContain(settings.key_replace({ provider: 'OpenRouter' }));
    expect(guide).toContain(settings.key_step_create);
    expect(screen.getByTestId('settings.key.after').textContent).toContain(settings.key_after_test({ what: 'auto approvals' }));
    fireEvent.click(screen.getByTestId('settings.key.link'));
    expect(opened).toEqual([KEYS]);
    // he pastes a new key: checked by the core, then it works, and approvals test again
    const field = screen.getByTestId('settings.key.field');
    fireEvent.change(field, { target: { value: 'sk-or-v1-new' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(p.sent.at(-1)).toEqual({ cmd: 'provider_use', id: 'openrouter', key: 'sk-or-v1-new' });
    const works = { ev: 'provider_check' as const, id: 'openrouter', name: 'OpenRouter', model: JEV, pasted: true, state: 'works' as const };
    r.rerender(<Settings page="setup" prefs={prefs()} accounts={acc} projects={projects()} found={found()} computerUse={null} perms={{}} ports={p} gauge={failedNext} check={works} />);
    expect(p.sent.at(-1)).toEqual({ cmd: 'setup_test', models: [JEV] });
  });

  it('no key yet: the fix adds one (its article by sound); a key that failed: it replaces it', () => {
    const row = (why: 'no_key' | 'wrong_key'): SetupGauge => ({
      ...gauge(),
      rows: [{ id: 'approvals', state: 'failed', fix: { t: 'add_key', provider: 'openrouter' }, parts: [{ model: 'openrouter/typesafe/jev-1.13', provider: 'OpenRouter', role: 'classify', source: 'config', state: 'failed', why }] }],
    });
    const fix = { t: 'add_key' as const, provider: 'openrouter' };
    expect(fixButton(fix, row('no_key'))).toBe('Add an OpenRouter key');
    expect(fixButton(fix, row('wrong_key'))).toBe('Replace the OpenRouter key');
  });

  it('a provider name takes its article by sound', () => {
    expect(['OpenRouter', 'Anthropic', 'xAI', 'Mistral', 'Groq', 'Unify'].map((n) => `${article(n)} ${n}`)).toEqual(['an OpenRouter', 'an Anthropic', 'an xAI', 'a Mistral', 'a Groq', 'a Unify']);
  });

  it('all green: no fix line, every status in ink', () => {
    view('setup', ports(), { gauge: allOn() });
    expect(screen.queryByTestId('settings.fixline')).toBeNull();
    expect(document.querySelectorAll('.st-mark.err')).toHaveLength(0);
  });

  it('the fix line names the roles a fix brings back, joined in words', () => {
    expect(joinAnd(['Titles', 'Dictation', 'Voice mode'])).toBe(`Titles, Dictation ${settings.list_and} Voice mode`);
    expect(joinAnd(['Agents'])).toBe('Agents');
    // only something that stopped earns a line: a feature merely off doesn't
    const off: SetupGauge = { ...allOn(), next: { fix: { t: 'turn_on', role: 'classify' }, unlocks: ['approvals'] } };
    expect(fixLine(off)).toBeNull();
  });

  it('Try it on dictation: the mic on his click only, stopped after 3 s, then what it heard', () => {
    vi.useFakeTimers();
    const calls: string[] = [];
    let heard: { text: string; final: boolean; error: string | null } | null = null;
    const tryIt = {
      start: () => {
        calls.push('start');
        heard = { text: '', final: false, error: null };
        return 'd1';
      },
      stop: (id: string) => {
        calls.push(`stop ${id}`);
      },
      useHeard: (id: string | null) => (id === null ? null : heard),
    };
    const r = view('setup', { ...ports(), tryIt }, { gauge: gauge() });
    expect(calls).toEqual([]);
    fireEvent.click(screen.getByTestId('settings.feature.dictation.try'));
    expect(calls).toEqual(['start']);
    // while it listens: no ✓, the instruction instead (designer m_54627)
    expect(screen.queryByTestId('settings.feature.dictation.mark')).toBeNull();
    expect(screen.getByTestId('settings.feature.dictation.heard').textContent).toBe(settings.try_listening);
    vi.advanceTimersByTime(TRY_MS);
    expect(calls).toEqual(['start', 'stop d1']);
    heard = { text: 'hello bise', final: true, error: null };
    r.rerender(<Settings page="setup" prefs={prefs()} accounts={accounts()} projects={projects()} found={found()} computerUse={null} perms={{}} ports={{ ...ports(), tryIt }} gauge={gauge()} />);
    expect(screen.getByTestId('settings.feature.dictation.heard').textContent).toBe(settings.try_heard({ text: 'hello bise' }));
    expect(screen.getByTestId('settings.feature.dictation.mark')).toBeTruthy();
    vi.useRealTimers();
  });

  it('/models opens Setup on the list (its top); /voice at Dictation and fn', () => {
    view('models', ports(), { gauge: gauge(), roles: [] });
    expect(screen.getByTestId('settings.page.setup')).toBeTruthy();
    expect(screen.getByTestId('settings.roles')).toBeTruthy();
    expect(focusOf('models')).toBeNull();
    expect(focusOf('dictation')).toBe('dictation');
  });
});

describe('which model does what (V17) and a model with no key (L5)', () => {
  const MODELS: Model[] = [
    { id: 'anthropic/claude-opus-5-5', label: 'opus 5.5', provider: 'Anthropic', short: '1M', efforts: ['low', 'medium', 'high', 'max'], default_effort: 'high', provider_id: 'anthropic', key_ready: true },
    { id: 'mistral/devstral-medium-latest', label: 'devstral medium', provider: 'Mistral', short: '128k', provider_id: 'mistral', key_ready: false },
    { id: 'devstral', label: 'devstral', provider: '', short: '= mistral/devstral-medium-latest', alias_of: 'mistral/devstral-medium-latest' },
  ];
  const ROLES: RoleRow[] = [
    { role: 'main', name: 'main', about: 'your team lead', kind: 'chat', model: 'anthropic/claude-opus-5-5', effort: 'high', source: 'config' },
    { role: 'agents', name: 'agents', about: 'the tasks main starts', kind: 'chat', model: 'anthropic/claude-opus-5-5', source: 'default', follows: 'main' },
    { role: 'voice', name: 'voice', about: 'speech to text', kind: 'voice', model: 'mistral/voxtral-mini-latest', source: 'env' },
  ];
  const ACCOUNTS: Account[] = [
    { id: 'chatgpt', label: 'ChatGPT', kind: 'subscription', state: 'signed_in', provider: 'chatgpt' },
    { id: 'anthropic', label: 'Anthropic key', kind: 'key', state: 'signed_in', provider: 'anthropic' },
    { id: 'mistral', label: 'Mistral key', kind: 'key', state: 'signed_out', provider: 'mistral' },
  ];

  it('needsSetup: a model whose provider has no key names it; an alias follows its target; a ready one needs nothing', () => {
    expect(needsSetup('mistral/devstral-medium-latest', MODELS)).toBe('mistral');
    expect(needsSetup('devstral', MODELS)).toBe('mistral');
    expect(needsSetup('anthropic/claude-opus-5-5', MODELS)).toBeNull();
    expect(needsSetup('unknown/x', MODELS)).toBeNull();
    const held = { provider: 'mistral', providerName: 'Mistral', model: 'devstral medium', cmd: { cmd: 'role_set', role: 'main', model: 'mistral/devstral-medium-latest' } as AppCmd };
    expect(heldReady(held, ACCOUNTS)).toBe(false);
    expect(heldReady(held, ACCOUNTS.map((a) => (a.id === 'mistral' ? { ...a, state: 'signed_in' } : a)))).toBe(true);
  });

  it('modelOrSetup: sends a ready model at once, holds the other and opens the setup', () => {
    const sent: CoreCmd[] = [];
    const went: string[] = [];
    const ports = { send: (c: CoreCmd) => sent.push(c), setup: () => went.push('setup') };
    expect(modelOrSetup('anthropic/claude-opus-5-5', MODELS, { cmd: 'model', project: 'p', agent: 'main', model: 'anthropic/claude-opus-5-5' }, ports)).toBe('sent');
    expect(modelOrSetup('devstral', MODELS, { cmd: 'model', project: 'p', agent: 'main', model: 'devstral' }, ports)).toBe('setup');
    expect(sent.length).toBe(1);
    expect(went).toEqual(['setup']);
    releaseHeld();
  });

  const OPUS = { model: 'anthropic/claude-opus-5-5', name: 'opus 5.5', provider: 'Anthropic', state: 'on' as const, ms: 1300 };
  const GAUGE: SetupGauge = {
    on: 3,
    of: 3,
    testing: [],
    rows: [
      { id: 'talk', state: 'on', parts: [{ ...OPUS, role: 'main', source: 'config' }] },
      { id: 'agents', state: 'on', parts: [{ ...OPUS, role: 'agents', source: 'default', follows: 'main' }] },
      { id: 'dictation', state: 'on', parts: [{ role: 'voice', model: 'mistral/voxtral-mini-latest', name: 'voxtral-mini', provider: 'Mistral', source: 'env', var: 'MISTRAL_VOICE', state: 'on', ms: 400 }] },
    ],
  };

  it("each role's model by name, provider faint, raw id in the tip; its picker: the fallback first, the models by provider, the effort in its footer", () => {
    const p = ports();
    view('models', p, { gauge: GAUGE, roles: ROLES, models: MODELS, accounts: ACCOUNTS });
    const main = screen.getByTestId('settings.role.main.model');
    expect(main.textContent).toBe('opus 5.5Anthropic');
    expect(main.getAttribute('data-tip')).toBe('anthropic/claude-opus-5-5');
    expect(screen.getByTestId('settings.feature.talk.mark').textContent).toBe(settings.mark_took({ secs: '1.3' }));
    // an env var's model: not a button, it says so and the tip has the step
    const voice = screen.getByTestId('settings.role.voice.model');
    expect(voice.textContent).toBe(`voxtral-mini${settings.part_set_by({ var: 'MISTRAL_VOICE' })}`);
    expect(voice.getAttribute('data-tip')).toBe(settings.tip_set_by({ var: 'MISTRAL_VOICE' }));
    expect(voice.closest('button')).toBeNull();
    // agents follow bise: their fallback row says so, with the model it resolves to
    fireEvent.click(screen.getByTestId('settings.role.agents.model'));
    expect(screen.getByTestId(`settings.role.agents.model.menu.${FALLBACK}`).textContent).toContain(settings.role_as({ role: settings.role_main_name }));
    fireEvent.click(screen.getByTestId(`settings.role.agents.model.menu.${FALLBACK}`));
    expect(p.sent).toEqual([{ cmd: 'role_set', role: 'agents', model: null }]);
    // main's effort: in the picker's footer
    fireEvent.click(screen.getByTestId('settings.role.main.model'));
    expect(screen.getByTestId('settings.role.main.model.effort.high').getAttribute('aria-checked')).toBe('true');
    fireEvent.click(screen.getByTestId('settings.role.main.model.effort.max'));
    expect(p.sent.at(-1)).toEqual({ cmd: 'role_set', role: 'main', effort: 'max' });
  });

  it("an effort that isn't its model's default shows on the button", () => {
    view('models', ports(), { gauge: GAUGE, roles: ROLES.map((r) => (r.role === 'main' ? { ...r, effort: 'max' } : r)), models: MODELS, accounts: ACCOUNTS });
    expect(screen.getByTestId('settings.role.main.model').textContent).toBe('opus 5.5Anthropic· max');
  });

  it('L5: a model whose provider has no key is held, not sent; the setup asks for that provider, its key field open', () => {
    const p = ports();
    const held: Held[] = [];
    const r = view('models', { ...p, hold: (h) => held.push(h) }, { gauge: GAUGE, roles: ROLES, models: MODELS, accounts: ACCOUNTS });
    pickRole('main', 'mistral/devstral-medium-latest');
    expect(p.sent).toEqual([]);
    expect(held).toEqual([{ provider: 'mistral', providerName: 'Mistral', model: 'devstral medium', cmd: { cmd: 'role_set', role: 'main', model: 'mistral/devstral-medium-latest' } }]);
    r.rerender(<Settings page="models" prefs={prefs()} accounts={ACCOUNTS} projects={projects()} found={found()} computerUse={null} perms={{}} ports={p} gauge={GAUGE} roles={ROLES} models={MODELS} held={held[0] ?? null} />);
    expect(screen.getByTestId('settings.models.want').textContent).toBe("devstral medium needs Mistral: sign in or add its key here, then it's set.");
    expect(screen.getByTestId<HTMLInputElement>('settings.key.field').type).toBe('password');
    const f = screen.getByTestId<HTMLInputElement>('settings.key.field');
    f.value = 'mk-1';
    fireEvent.keyDown(f, { key: 'Enter' });
    expect(p.sent).toEqual([{ cmd: 'provider_use', id: 'mistral', key: 'mk-1' }]);
  });

  it("a role_set the core refused says so in its words", () => {
    view('models', ports(), { gauge: GAUGE, roles: ROLES, models: MODELS, accounts: ACCOUNTS, roleError: 'no model named x' });
    expect(screen.getByTestId('settings.roles.error').textContent).toBe('no model named x');
  });
});

describe('approvals (V8 / W21) on the typed HubEv approvals, and the expired sign-in (V14 / S28 / L2)', () => {
  const NOW = 1791100400000;
  // rules as the hub sends them: the toml's fields (their identity) + the TUI's words, no age
  const R: ApprovalRule[] = [
    { tool: 'bash', pattern: 'cargo test *', project: '/Users/camille/acme', added: '2026-10-02', from: 'card #12, api-v2', added_ms: NOW - 3 * 86_400_000, what: 'cargo test *', note: 'from api-v2' },
    { tool: 'edit', path: '/Users/camille/notes', what: 'edits to ~/notes', note: 'every project' },
    { tool: 'gmail.send_email', what: 'gmail.send_email', note: 'a connector' },
  ];
  const A: ApprovalsEv = {
    ev: 'approvals', project: 'acme-1a2b3c4d', mode: 'auto', env: false, checker: 'jev', checker_who: 'TypeSafe', checker_model: 'jev-1.13', repo: '/Users/camille/acme', rules: R,
  };

  it("says each rule in the TUI's words, its age counted here from added_ms", () => {
    expect(ruleWhat(R[0] as ApprovalRule)).toBe('cargo test *');
    expect(ruleWhat(R[1] as ApprovalRule)).toBe('edits to ~/notes');
    expect(ruleWhat({ tool: 'gmail.send_email' })).toBe('gmail.send_email');
    expect(ruleNote(R[0] as ApprovalRule, NOW)).toBe('3 days ago · from api-v2');
    expect(ruleNote(R[0] as ApprovalRule, NOW + 4 * 86_400_000)).toBe('7 days ago · from api-v2');
    expect(ruleNote(R[1] as ApprovalRule, NOW)).toBe('every project');
    expect(ruleNote({ tool: 'bash', added_ms: NOW }, NOW)).toBe('today');
    expect(checkerWords(A)).toBe('TypeSafe · jev-1.13');
    expect(checkerWords({ ...A, checker: 'off' })).toBe('off · every command asks you');
    expect(checkerWords({ ...A, checker: 'unknown' })).toBe('none yet');
  });

  it('the fixture approvals event draws (yolo forced by the environment, no rules)', () => {
    const ev = hub_ev.find((e) => e.ev === 'approvals' && e.env);
    expect(ev?.ev).toBe('approvals');
    if (ev?.ev !== 'approvals') return;
    render(<Approvals approvals={ev} send={null} home="/Users/me" now={NOW} />);
    expect(screen.getByTestId('settings.approvals.repo').textContent).toBe('what runs without asking you in ~/acme.');
    expect(screen.getByTestId('settings.approvals').textContent).toContain('yolo · this session (BISE_APPROVALS)');
  });

  it('the mode switches, a rule asks before it goes and is sent back whole, keep it keeps it, a refusal shows', () => {
    const sent: ApprovalsCmd[] = [];
    const r = render(<Approvals approvals={A} send={(c) => sent.push(c)} home="/Users/camille" now={NOW} />);
    expect(screen.getByTestId('settings.approvals.repo').textContent).toBe('what runs without asking you in ~/acme.');
    fireEvent.click(screen.getByTestId('settings.approvals.switch.1'));
    expect(sent).toEqual([{ cmd: 'approvals', project: 'acme-1a2b3c4d', mode: 'yolo' }]);
    fireEvent.click(screen.getByTestId('settings.approvals.rule.0.remove'));
    expect(screen.getByTestId('settings.approvals.rules').textContent).toContain('remove it?');
    fireEvent.click(screen.getByTestId('settings.approvals.rule.0.keep'));
    expect(sent.length).toBe(1);
    fireEvent.click(screen.getByTestId('settings.approvals.rule.0.remove'));
    fireEvent.click(screen.getByTestId('settings.approvals.rule.0.yes'));
    expect(sent[1]).toEqual({ cmd: 'remove_rule', project: 'acme-1a2b3c4d', rule: R[0] });
    // the hub's fresh list (the rule gone, every row a new object, as parsed JSON) folds any ask;
    // a refusal says so under the list
    fireEvent.click(screen.getByTestId('settings.approvals.rule.1.remove'));
    const fresh = R.slice(1).map((x) => ({ ...x }));
    r.rerender(<Approvals approvals={{ ...A, rules: fresh }} send={(c) => sent.push(c)} error="that rule is already gone" now={NOW} />);
    expect(screen.getByTestId('settings.approvals.rules').textContent).not.toContain('remove it?');
    expect(screen.getByTestId('settings.approvals.error').textContent).toBe('that rule is already gone');
  });

  it("the rules walk with the TUI's /approvals keys: ↑↓ home/end, ⌫ asks, esc keeps, ⏎ removes", () => {
    const sent: ApprovalsCmd[] = [];
    render(<Approvals approvals={A} send={(c) => sent.push(c)} home="/Users/camille" now={NOW} />);
    const page = screen.getByTestId('settings.approvals.rules').closest<HTMLElement>('.settings-approvals');
    if (page === null) throw new Error('no approvals page');
    const key = (k: string) => fireEvent.keyDown(page, { key: k });
    const selRow = () => page.querySelector('.b-gi.is-sel')?.getAttribute('data-row');
    expect(document.activeElement, 'the page takes the keys at mount').toBe(page);
    expect(selRow(), 'the first rule is selected').toBe('0');
    key('ArrowUp');
    expect(selRow(), '↑ at the top stays').toBe('0');
    key('ArrowDown');
    key('j');
    key('ArrowDown');
    expect(selRow(), 'it clamps at the last').toBe('2');
    key('Home');
    expect(selRow()).toBe('0');
    key('End');
    key('Backspace');
    expect(screen.getByTestId('settings.approvals.rules').textContent).toContain('remove it?');
    key('ArrowUp');
    expect(selRow(), 'the move keys stay put while it asks').toBe('2');
    key('Escape');
    expect(screen.getByTestId('settings.approvals.rules').textContent).not.toContain('remove it?');
    expect(sent).toEqual([]);
    key('ArrowUp');
    key('Delete');
    key('Enter');
    expect(sent).toEqual([{ cmd: 'remove_rule', project: 'acme-1a2b3c4d', rule: R[1] }]);
    // a click picks; hover never moves the selection (no handler for it)
    fireEvent.click(screen.getByTestId('settings.approvals.rules.0'));
    expect(selRow()).toBe('0');
  });

  it('a mode forced by the environment is said, not switched; no rule yet says how one comes; nothing sent yet says so', () => {
    const { rerender } = render(<Approvals approvals={{ ...A, env: true, rules: [] }} send={null} now={NOW} />);
    expect(screen.queryByTestId('settings.approvals.switch')).toBeNull();
    expect(screen.getByTestId('settings.approvals').textContent).toContain('auto · this session (BISE_APPROVALS)');
    expect(screen.getByTestId('settings.approvals.empty').textContent).toContain('always allow');
    rerender(<Approvals approvals={null} send={null} now={NOW} />);
    expect(screen.getByTestId('settings.approvals.none')).toBeTruthy();
  });

  it('an expired subscription names the agents it stopped; sign in again shows the wait, c copies the link, esc cancels', () => {
    const p = ports();
    const copied: string[] = [];
    const cancelled: string[] = [];
    const accts: Account[] = [{ id: 'chatgpt', label: 'ChatGPT', kind: 'subscription', state: 'expired' }];
    const r = render(<Accounts accounts={accts} send={p.send} signin={{ waiting: ['main', 't1'] }} copy={(t) => copied.push(t)} cancelSignIn={(id) => cancelled.push(id)} />);
    expect(screen.getByTestId('settings.models').textContent).toContain('your sign-in expired: main, t1 wait');
    fireEvent.click(screen.getByTestId('settings.account.chatgpt'));
    expect(p.sent).toEqual([{ cmd: 'sign_in', id: 'chatgpt' }]);
    r.rerender(<Accounts accounts={accts} send={p.send} signin={{ waiting: ['main', 't1'], signing: { id: 'chatgpt', url: 'https://auth.openai.com/x' } }} copy={(t) => copied.push(t)} cancelSignIn={(id) => cancelled.push(id)} />);
    const box = screen.getByTestId('settings.signing');
    expect(box.textContent).toContain('waiting for you to sign in to ChatGPT in your browser…');
    fireEvent.keyDown(box, { key: 'c' });
    expect(copied).toEqual(['https://auth.openai.com/x']);
    fireEvent.keyDown(box, { key: 'Escape' });
    expect(cancelled).toEqual(['chatgpt']);
  });

  it('expiredWords says who waits', () => {
    expect(expiredWords([])).toBe('your sign-in expired: sign in again');
    expect(expiredWords(['main'])).toBe('your sign-in expired: main waits');
    expect(expiredWords(['a', 'b', 'c', 'd'])).toBe('your sign-in expired: a, b and 2 more wait');
  });
});
