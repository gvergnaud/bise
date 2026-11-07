// settings · Setup (page models-setup, pick B): Models and Setup in one page. On top the gauge,
// Next and Test everything; then the features (gauge.tsx); under them, folded, the roles table
// (which model does what) and his accounts and keys. /models opens it with the roles unfolded,
// /voice at Dictation and fn (the route's page: 'models', 'dictation'). The page remembers whether
// the roles are open (this window's localStorage: view state, never a setting). A fix is his click:
// a key opens the accounts, a model the roles, a permission the helper, a test setup_test. Owner:
// setup-page.
import { useEffect, useRef, useState, type ReactNode } from 'react';
import type { AppCmd, FeatureId, SetupFix } from '@/proto';
import { Press } from '@/ds';
import { settings } from '@/proto/words.gen';
import { Accounts } from './accounts';
import { Roles } from './roles';
import { modelRow, needsSetup } from './hold';
import { GaugeView, type GaugePorts } from './gauge';
import type { SettingsData, SettingsPorts } from './Settings';
import { KeychainRow } from './keychain';

/** Where the route opens the page: the roles unfolded (/models), a feature's row (/voice). Pure. */
export function focusOf(page: string | undefined): { roles: boolean; at: FeatureId | null } {
  if (page === 'models') return { roles: true, at: null };
  if (page === 'dictation') return { roles: false, at: 'dictation' };
  return { roles: false, at: null };
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
  const accountOf = (provider: string) => data.accounts.find((a) => (a.provider ?? a.id) === provider);
  const fix = (f: SetupFix) => {
    switch (f.t) {
      case 'add_key':
      case 'credit':
        if (f.url !== undefined && f.url !== '' && f.t === 'credit') window.open(f.url, '_blank', 'noopener');
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
  };
  const g = data.gauge ?? null;
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
          />
          {data.keychain != null && <KeychainRow k={data.keychain} send={ports.send} titled={false} />}
        </Fold>
      </div>
    </>
  );
}
