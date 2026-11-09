// settings · computer use's Profiles group (designer m_58124, page profils-chrome): his browser
// profiles with the bise extension, from two (one profile: no group, as before). One row each: the
// profile's name, 'Chrome · <email>' under it (data: their case kept), then 'Default' as a quiet
// chip on one, its status ('✓ connected' in ink, '○ not open' faint: a closed Chrome is no
// failure), then ⋯ (Make default, Forget this profile). Under them, 'Connect another profile' opens
// a sheet of three steps that waits for the new profile to connect. The rows are the core's
// (`computer_use.profiles`); every change is his click (`computer_use {act: default|forget}`).
// Owner: chrome-profiles.
import { useState } from 'react';
import { Button, Group, Sheet, sentenceCase, tipAttrs, type GroupItem } from '@/ds';
import type { AppCmd, CuProfile, CuProfileKey } from '@/proto';
import type { BiseApi, PopupItem } from '@/shared/api';
import { settings } from '@/proto/words.gen';

const bise = (): Partial<BiseApi> | undefined => (window as { bise?: Partial<BiseApi> }).bise;

/** A profile's id in test ids and React keys: 'chrome/Profile 1'. Pure. */
export const profileId = (k: CuProfileKey): string => `${k.browser}/${k.dir}`;

/** Whether the page shows the group: two profiles or more (one: nothing to choose). Pure. */
export const hasProfiles = (ps: readonly CuProfile[]): boolean => ps.length > 1;

/** A row's sub: the browser, and the account when one is signed in. Pure. */
export const profileSub = (p: CuProfile): string => (p.email != null && p.email !== '' ? settings.cu_profile_sub({ browser: p.browser_name, email: p.email }) : p.browser_name);

/** The ⋯ menu's items (no 'Make default' on the default). Pure. */
export function profileMenu(p: CuProfile): PopupItem[] {
  return [...(p.default === true ? [] : [{ id: 'default', label: settings.cu_profile_make_default }]), { id: 'forget', label: settings.cu_profile_forget }];
}

/** The profiles that connected since `known` (the add sheet's wait): their names. Pure. */
export function newcomers(ps: readonly CuProfile[], known: readonly string[]): string[] {
  return ps.filter((p) => p.connected === true && !known.includes(profileId(p.key))).map((p) => p.name);
}

type Add = { kind: 'closed' } | { kind: 'waiting'; known: string[] };

export function ProfilesGroup({ profiles, send }: { profiles: readonly CuProfile[]; send: (cmd: AppCmd) => void }) {
  const [add, setAdd] = useState<Add>({ kind: 'closed' });
  if (!hasProfiles(profiles)) return null;
  const act = (act: 'default' | 'forget', key: CuProfileKey) => {
    send({ cmd: 'computer_use', act, profile: key });
  };
  const row = (p: CuProfile): GroupItem => {
    const id = `settings.cu.profile.${profileId(p.key)}`;
    const more = () => {
      void bise()?.popupMenu?.(profileMenu(p)).then((x) => {
        if (x === 'default' || x === 'forget') act(x, p.key);
      });
    };
    return {
      id: profileId(p.key),
      label: p.name,
      sub: profileSub(p),
      control: (
        <span className="settings-cu-profile-end" data-testid={id}>
          {p.default === true && (
            <span className="settings-cu-chip" data-testid={`${id}.default`} {...tipAttrs(settings.cu_profile_default_tip)}>
              {settings.cu_profile_default}
            </span>
          )}
          {p.connected === true ? (
            <span className="settings-cu-ink" data-testid={`${id}.status`}>{settings.cu_profile_connected}</span>
          ) : (
            <span className="settings-cu-faint" data-testid={`${id}.status`} {...tipAttrs(settings.cu_profile_not_open_tip)}>
              {settings.cu_profile_not_open}
            </span>
          )}
          <Button size="sm" kind="ghost" onClick={more} testId={`${id}.more`}>⋯</Button>
        </span>
      ),
    };
  };
  const came = add.kind === 'waiting' ? newcomers(profiles, add.known) : [];
  const close = () => { setAdd({ kind: 'closed' }); };
  return (
    <>
      <Group title={sentenceCase(settings.cu_profiles_head)} testId="settings.cu.profiles" items={profiles.map(row)} />
      <div className="settings-cu-profile-add">
        <Button onClick={() => { setAdd({ kind: 'waiting', known: profiles.filter((p) => p.connected === true).map((p) => profileId(p.key)) }); }} testId="settings.cu.profiles.add">
          {settings.cu_profile_add}
        </Button>
      </div>
      <Sheet open={add.kind !== 'closed'} onClose={close} testId="settings.cu.profiles.add.sheet" title={settings.cu_profile_add_title}
        onPrimary={came.length > 0 ? close : undefined}
        foot={came.length > 0
          ? <Button kind="pri" onClick={close} testId="settings.cu.profiles.add.done">{settings.cu_profile_done}</Button>
          : <Button kind="ghost" onClick={close} testId="settings.cu.profiles.add.cancel">{settings.cu_profile_cancel}</Button>}>
        <ol className="settings-cu-steps" data-testid="settings.cu.profiles.add.steps">
          <li>{settings.cu_profile_step_open}<span className="settings-cu-faint">{` ${settings.cu_profile_step_open_hint}`}</span></li>
          <li>{settings.cu_profile_step_add}</li>
          <li>{settings.cu_profile_step_back}</li>
        </ol>
        {came.length === 0
          ? <p className="settings-cu-faint" data-testid="settings.cu.profiles.add.waiting">{settings.cu_profile_waiting}</p>
          : <p className="settings-cu-ink" data-testid="settings.cu.profiles.add.came">{`✓ ${came.join(', ')}`}</p>}
      </Sheet>
    </>
  );
}
