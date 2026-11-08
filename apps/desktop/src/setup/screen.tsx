// The first launch as the shell mounts it (route `onboarding`): Onboarding
// fed by the store (the core's accounts, found repos) and by the helper
// (bise-mac-helper through window.bise: its perms and fn events, a perm
// command), and the hook that sends a fresh home there once. Owner:
// amb-home.
import { useEffect, useRef, useState } from 'react';
import type { Artifact } from '@/proto';
import { useAccounts, useArtifacts, useClaudePlan, useFound, usePrefs, useProjects, useProviderCheck, useRoles, useStore } from '@/store';
import { useRoute, useRouter } from '@/app/router';
import { onboarded, prefsOf, useCmdError } from '@/settings';
import { Onboarding } from './Onboarding';
import { mainHasModel } from './model';
import { shareOf } from './TelemetryNote';
import { useHelper } from './helper';
import { pickerPort } from '@/settings/picker';

export function OnboardingScreen() {
  const store = useStore();
  const router = useRouter();
  const { perms, fn } = useHelper();
  return (
    <Onboarding
      accounts={useAccounts()}
      claudePlan={useClaudePlan()}
      found={useFound()}
      home={(globalThis as { bise?: { home?: string } }).bise?.home ?? ''}
      perms={perms}
      fn={fn}
      check={useProviderCheck()}
      hasModel={mainHasModel(useRoles())}
      share={shareOf(prefsOf(usePrefs()))}
      signInError={useCmdError('sign_in')?.text ?? null}
      ports={{
        send: (cmd) => {
          store.command(cmd);
        },
        done: () => {
          router.go({ screen: 'bise' });
        },
        open: (url) => {
          void window.bise.openExternal(url);
        },
        ...pickerPort(),
      }}
    />
  );
}


/**
 * The first prefs of a home not onboarded yet open the first launch, once,
 * wherever the window was (absent = a fresh home: the core says false).
 * The shell (amb-win's App.tsx) calls it once.
 */
export function useFirstLaunch(): void {
  const prefs = prefsOf(usePrefs());
  const router = useRouter();
  const sent = useRef(false);
  // this window saw a fresh home: after hello, the start-here page opens once (bar S.9)
  const fresh = useRef(false);
  const opened = useRef(false);
  useEffect(() => {
    if (sent.current || prefs === null) return;
    sent.current = true;
    if (!onboarded(prefs)) {
      fresh.current = true;
      if (router.get().screen !== 'onboarding') router.go({ screen: 'onboarding' });
    }
  }, [prefs, router]);
  // the hub publishes start-here once on the home's first run (first_run.rs), as main: it comes as
  // one of the home's artifacts with its url; it opens in his browser, like the old app's and like a
  // PR link (window.bise.openExternal; test mode prints 'bise-desktop: external <url>')
  const home = useProjects().find((p) => p.home);
  const url = startHereUrl(useArtifacts(home?.project ?? ''));
  // hello ended, however (its last step, skip, or he left it): the window left the onboarding
  // screen after showing it. The prefs' onboarded true counts too, but never alone: a core may
  // not send prefs again after prefs_set (qa-native c99de916's stub doesn't).
  const screen = useRoute().screen;
  const [shown, setShown] = useState(false);
  if (screen === 'onboarding' && !shown) setShown(true);
  const ended = (shown && screen !== 'onboarding') || (prefs !== null && onboarded(prefs));
  useEffect(() => {
    if (!helloOpens(fresh.current, opened.current, ended, url)) return;
    opened.current = true;
    const bise = window.bise as typeof window.bise | undefined;
    void bise?.openExternal(url ?? '');
  }, [ended, url]);
}

/** start-here opens now: a fresh home seen by this window, hello ended, not opened yet, its url known. */
export function helloOpens(fresh: boolean, opened: boolean, ended: boolean, url: string | null): boolean {
  return fresh && !opened && ended && url !== null;
}

/** The hub's start-here page among the home's artifacts (first_run.rs PAGE_ID), its url; none: null. */
export function startHereUrl(artifacts: readonly Artifact[]): string | null {
  const a = artifacts.find((x) => x.id === START_HERE && x.kind === 'page');
  return a?.url != null && a.url !== '' ? a.url : null;
}

/** first_run.rs's PAGE_ID */
export const START_HERE = 'start-here';
