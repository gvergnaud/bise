// N1 (the TUI's /keychain): where his keys and sign-ins are kept, under the accounts of settings ·
// Setup. Owner: amb-home.
import type { AppCmd } from '@/proto';
import { Group, Toggle } from '@/ds';
import { settings } from '@/proto/words.gen';
import type { SettingsData } from './Settings';

/**
 * N1 (the TUI's /keychain [on|off], designer m_17435's words): where his keys and sign-ins are kept,
 * the macOS keychain or files in ~/.bise. The switch moves them (AppCmd keychain, the core's thread);
 * while it runs the switch waits and says so; the move's lines (item names and counts) under it, a
 * failed one in the error tone.
 */
export function KeychainRow({ k, send, titled = true }: { k: NonNullable<SettingsData['keychain']>; send: (cmd: AppCmd) => void; titled?: boolean }) {
  const on = k.store === 'keychain';
  // a move's own lines are the core's (secrets_cli, the terminal's words); with none, where they are
  const said = k.moving ? settings.keychain_moving : (k.lines?.join(' ') ?? '');
  const now = on ? settings.keychain_on : settings.keychain_off;
  return (
    <Group
      {...(titled ? { title: settings.keychain_title } : {})}
      testId="settings.keychain"
      items={[
        {
          id: 'keychain',
          label: settings.keychain_label,
          sub: (
            <span data-testid="settings.keychain.sub" className={k.failed === true && !k.moving ? 'settings-err' : undefined}>
              {said !== '' ? said : now}
            </span>
          ),
          control: (
            <Toggle
              on={on}
              state={k.moving ? 'disabled' : undefined}
              label={settings.keychain_switch}
              onChange={(v) => {
                send({ cmd: 'keychain', to: v ? 'on' : 'off' });
              }}
              testId="settings.keychain.toggle"
            />
          ),
        },
      ]}
    />
  );
}
