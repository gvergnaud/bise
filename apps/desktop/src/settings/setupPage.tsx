// settings · Setup (page models-setup, pick B): Models and Setup in one page. On top the gauge,
// Next and Test everything; then the features (gauge.tsx); under them, folded, the roles table
// (which model does what) and his accounts and keys. /models opens it with the roles unfolded,
// /voice at Dictation and fn (the route's page: 'models', 'dictation'). The page remembers whether
// the roles are open (this window's localStorage: view state, never a setting). A fix is his click:
// a key opens its field in the accounts (added or replaced, with the steps to get one; once it
// works, the features it fixes test again), a model the roles, a permission the helper, a test
// setup_test. Owner: setup-page.
import { useEffect, useRef, useState, type ReactNode } from 'react';
import type { Account, AppCmd, FeatureId, SetupFix, SetupGauge } from '@/proto';
import { Press } from '@/ds';
import { settings } from '@/proto/words.gen';
import { Accounts, keyAccountOf, type KeyAsk } from './accounts';
import { Roles } from './roles';
import { modelRow, needsSetup } from './hold';
import { FEATURE_WORDS, GaugeView, providerName, type GaugePorts } from './gauge';
import { keyField, type ProviderCheck } from './keycheck';
import type { SettingsData, SettingsPorts } from './Settings';
import { KeychainRow } from './keychain';

/** Where the route opens the page: the roles unfolded (/models), a feature's row (/voice). Pure. */
export function focusOf(page: string | undefined): { roles: boolean; at: FeatureId | null } {
  if (page === 'models') return { roles: true, at: null };
  if (page === 'dictation') return { roles: false, at: 'dictation' };
  return { roles: false, at: null };
}

/** A key the add_key fix asked for: what the accounts' field shows (`ask`), the account it goes
 * to, the check the store held at the click (older answers don't count) and the models a key that
 * works tests again. */
export interface Asked {
  ask: KeyAsk;
  id: string;
  base: ProviderCheck | null;
  models: readonly string[];
}

/** The add_key fix for `provider`: the features it would fix (every row whose fix is that key),
 * their models and names, its keys page. Pure. */
export function askOf(g: SetupGauge, provider: string, url: string, accounts: readonly Account[], base: ProviderCheck | null): Asked {
  const rows = g.rows.filter((r) => r.fix?.t === 'add_key' && r.fix.provider === provider);
  const models = [...new Set(rows.flatMap((r) => r.parts.map((p) => p.model).filter((m) => m !== '')))];
  const what = rows.map((r) => FEATURE_WORDS[r.id].toLowerCase()).join(', ');
  return {
    ask: { provider, name: providerName(g, provider), url, what },
    id: keyAccountOf(accounts, provider)?.id ?? provider,
    base,
    models,
  };
}

const ROLES_OPEN = 'bise.settings.setup.roles';

function rolesOpenKept(): boolean {
  try {
    return window.localStorage.getItem(ROLES_OPEN) === '1';
  } catch {
    return false;
  }
}

function keepRolesOpen(on: boolean): void {
  try {
    window.localStorage.setItem(ROLES_OPEN, on ? '1' : '0');
  } catch {
    // no storage (a test): the page forgets it
  }
}

/** A folded part: its head opens and closes it. */
function Fold({ title, open, onToggle, testId, children }: { title: string; open: boolean; onToggle: () => void; testId: string; children: ReactNode }) {
  return (
    <section className="st-fold" data-open={open || undefined} data-testid={testId}>
      <Press look="st-fold-head" aria-expanded={open} onClick={onToggle} data-testid={`${testId}.head`}>
        <span className="chev" aria-hidden="true">{open ? '▾' : '▸'}</span>
        {title}
      </Press>
      {open && <div className="st-fold-body">{children}</div>}
    </section>
  );
}

export function SetupPage({ data, ports, page }: { data: SettingsData; ports: SettingsPorts; page?: string | undefined }) {
  const focus = focusOf(page);
  const [rolesOpen, setRolesOpen] = useState(() => focus.roles || rolesOpenKept());
  const [accountsOpen, setAccountsOpen] = useState(false);
  const rolesRef = useRef<HTMLDivElement>(null);
  const accountsRef = useRef<HTMLDivElement>(null);
  // /models lands on the roles, /voice on Dictation and fn: scrolled into view once, at arrival
  useEffect(() => {
    const el = focus.roles ? rolesRef.current : focus.at !== null ? document.querySelector(`[data-testid="settings.feature.${focus.at}"]`) : null;
    el?.scrollIntoView({ block: 'start' });
  }, [focus.roles, focus.at]);
  const openRoles = () => {
    setRolesOpen(true);
    keepRolesOpen(true);
    requestAnimationFrame(() => rolesRef.current?.scrollIntoView({ block: 'start' }));
  };
  const openAccounts = () => {
    setAccountsOpen(true);
    requestAnimationFrame(() => accountsRef.current?.scrollIntoView({ block: 'start' }));
  };
  const g = data.gauge ?? null;
  const accountOf = (provider: string) => data.accounts.find((a) => (a.provider ?? a.id) === provider);
  // the key the fix asked for: its field opens in the accounts; once a key there works, the
  // features that failed for it test again (his paste is the click a test needs)
  const [asked, setAsked] = useState<Asked | null>(null);
  const check = data.check ?? null;
  const retested = useRef<Asked | null>(null);
  useEffect(() => {
    if (asked === null || retested.current === asked || keyField({ id: asked.id, base: asked.base }, check) !== 'added') return;
    retested.current = asked;
    ports.send({ cmd: 'setup_test', models: [...asked.models] });
  }, [asked, check, ports]);
  const fix = (f: SetupFix) => {
    switch (f.t) {
      case 'add_key':
        if (g !== null) setAsked(askOf(g, f.provider, f.url ?? '', data.accounts, check));
        openAccounts();
        return;
      case 'credit':
        if (f.url !== undefined && f.url !== '') ports.open?.(f.url);
        openAccounts();
        return;
      case 'sign_in': {
        const a = accountOf(f.provider);
        if (a !== undefined) ports.send({ cmd: 'sign_in', id: a.id });
        else openAccounts();
        return;
      }
      case 'pick_model':
      case 'remove_env':
        openRoles();
        return;
      case 'allow_perm':
        ports.perm?.(f.perm, f.open ? 'open' : 'ask');
        return;
      case 'turn_on':
        // the checker back to its default (it follows small jobs, or Jev with its key)
        ports.send({ cmd: 'role_set', role: 'classify' });
        return;
      case 'restart':
      case 'test_again':
      default:
        ports.send({ cmd: 'setup_test' });
    }
  };
  const gp: GaugePorts = {
    fix,
    test: (models) => {
      ports.send(models === undefined ? { cmd: 'setup_test' } : { cmd: 'setup_test', models: [...models] });
    },
    roles: openRoles,
    ...(ports.copy ? { copy: ports.copy } : {}),
    ...(ports.tryIt ? { tryIt: ports.tryIt } : {}),
  };
  const models = data.models ?? [];
  const held = data.held ?? null;
  return (
    <>
      {g !== null && (
        <GaugeView g={g} ports={gp} at={focus.at} />
      )}
      <div ref={rolesRef}>
        <Fold
          title={settings.fold_roles}
          open={rolesOpen}
          onToggle={() => {
            setRolesOpen(!rolesOpen);
            keepRolesOpen(!rolesOpen);
          }}
          testId="settings.fold.roles"
        >
          <Roles
            titled={false}
            roles={data.roles ?? []}
            models={models}
            error={data.roleError ?? null}
            set={(cmd: AppCmd, model) => {
              const provider = model !== null ? needsSetup(model, models) : null;
              if (provider !== null && model !== null && ports.hold) {
                const row = modelRow(model, models);
                ports.hold({ provider, providerName: row?.provider ?? provider, model: row?.label ?? model, cmd });
              } else {
                ports.send(cmd);
              }
            }}
          />
        </Fold>
      </div>
      <div ref={accountsRef}>
        <Fold title={settings.fold_accounts} open={accountsOpen || held !== null} onToggle={() => { setAccountsOpen(!accountsOpen); }} testId="settings.fold.accounts">
          <Accounts
            accounts={data.accounts}
            send={ports.send}
            check={data.check ?? null}
            want={held !== null ? { provider: held.provider, providerName: held.providerName, model: held.model } : null}
            signin={data.signin ?? null}
            {...(ports.copy ? { copy: ports.copy } : {})}
            {...(ports.cancelSignIn ? { cancelSignIn: ports.cancelSignIn } : {})}
            {...(ports.open ? { open: ports.open } : {})}
            ask={asked?.ask ?? null}
          />
          {data.keychain != null && <KeychainRow k={data.keychain} send={ports.send} titled={false} />}
        </Fold>
      </div>
    </>
  );
}
