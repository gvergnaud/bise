// Settings › Privacy's usage counts (page usage-metrics-plan, step 6; designer m_56350), first on
// the page: his share switch, 'See what's sent' (the sheet: the ID, the next packet and those of
// the last 7 days in plain JSON), 'Reset ID' and 'Delete my data' with its inline confirm. Every act
// is a telemetry {act} to the core, which runs bise_usage::control as `bise telemetry` does; the
// view comes back as the core's telemetry event (the only place the window gets the ID).
// Owner: telemetry.
import { useEffect, useState } from 'react';
import type { AppCmd, TelemetryOff } from '@/proto';
import type { EvOf } from '@/store';
import { Act, Button, Group, Sheet } from '@/ds';
import { settings } from '@/proto/words.gen';
import { shareCmd, shareOf } from '@/setup/TelemetryNote';
import type { Prefs } from './prefs';

export type TelemetryView = EvOf<'telemetry'>;

const SHOW: AppCmd = { cmd: 'telemetry', act: 'show' };

/** How long a reset's words stay in its row, ms. */
export const RESET_FLASH_MS = 3000;

/**
 * Why the switch can't be turned on here (a variable, a dev build, a test): its line; null when
 * he can (it's his pref, or nothing is sent yet: the switch still says his choice). Pure.
 */
export function forcedOff(off: TelemetryOff | undefined): string | null {
  switch (off) {
    case 'env':
      return settings.privacy_off_env;
    case 'do_not_track':
      return settings.privacy_off_dnt;
    case 'dev_build':
      return settings.privacy_off_dev;
    case 'ci':
    case 'test_home':
      return settings.privacy_off_test;
    case undefined:
    case 'not_yet':
    case 'offscreen':
    case 'pref':
    case 'note_not_seen':
    case 'unknown':
      return null;
  }
}

/** The switch's line: why it's forced off, else nothing sent yet, else what it shares. Pure. */
export function shareSub(view: TelemetryView | null): string {
  return forcedOff(view?.off) ?? (view?.off === 'not_yet' ? settings.privacy_not_yet : settings.privacy_share_sub);
}

/** The delete row's line after its act, else its own. Pure. */
export function deleteSub(done: TelemetryView['done']): string {
  if (done === 'asked') return settings.privacy_delete_asked;
  if (done === 'here') return settings.privacy_delete_here;
  return settings.privacy_delete_sub;
}

export function TelemetryGroup({ prefs, view, send }: { prefs: Prefs | null; view: TelemetryView | null; send: (cmd: AppCmd) => void }) {
  const [seen, setSeen] = useState(false);
  const [confirm, setConfirm] = useState(false);
  // the page asks once for why it may be off (a variable, a dev build): the core's view
  useEffect(() => {
    send(SHOW);
  }, [send]);
  // a reset's words for 3 s: the view that answered a reset, until its timer
  const done = view?.done;
  const [last, setLast] = useState(view);
  const [flashed, setFlashed] = useState<TelemetryView | null>(null);
  if (view !== last) {
    setLast(view);
    if (done === 'reset') setFlashed(view);
  }
  useEffect(() => {
    if (flashed === null) return;
    const t = setTimeout(() => { setFlashed(null); }, RESET_FLASH_MS);
    return () => { clearTimeout(t); };
  }, [flashed]);
  const flash = flashed !== null;
  const forced = forcedOff(view?.off) !== null;
  return (
    <>
      <Group
        title={settings.privacy_usage}
        testId="settings.privacy.usage"
        items={[
          {
            id: 'share',
            label: settings.privacy_share,
            sub: shareSub(view),
            toggle: {
              on: !forced && shareOf(prefs),
              ...(forced ? { state: 'disabled' as const } : {}),
              onChange: (on) => { send(shareCmd(on)); },
              testId: 'settings.pref.telemetry_share',
            },
          },
          {
            id: 'seen',
            label: settings.privacy_seen,
            sub: settings.privacy_seen_sub,
            control: (
              <Act onClick={() => { send(SHOW); setSeen(true); }} testId="settings.telemetry.seen">
                {`${settings.privacy_seen} ›`}
              </Act>
            ),
          },
          {
            id: 'reset',
            label: settings.privacy_reset,
            sub: flash ? settings.privacy_reset_done : settings.privacy_reset_sub,
            control: (
              <Act onClick={() => { send({ cmd: 'telemetry', act: 'reset' }); }} testId="settings.telemetry.reset">
                {settings.privacy_reset_act}
              </Act>
            ),
          },
          {
            id: 'delete',
            label: settings.privacy_delete,
            sub: confirm ? settings.privacy_delete_ask : deleteSub(done),
            control: confirm ? (
              <span className="tm-confirm">
                <Act tone="err" onClick={() => { send({ cmd: 'telemetry', act: 'delete' }); setConfirm(false); }} testId="settings.telemetry.delete.yes">
                  {settings.privacy_delete_yes}
                </Act>
                <Act onClick={() => { setConfirm(false); }} testId="settings.telemetry.delete.no">
                  {settings.privacy_delete_no}
                </Act>
              </span>
            ) : (
              <Act onClick={() => { setConfirm(true); }} testId="settings.telemetry.delete">
                {settings.privacy_delete_act}
              </Act>
            ),
          },
        ]}
      />
      <SeenSheet open={seen} view={view} onClose={() => { setSeen(false); }} />
    </>
  );
}

/** 'See what's sent': the ID on top, then the next packets and those of the last 7 days, as JSON. */
export function SeenSheet({ open, view, onClose }: { open: boolean; view: TelemetryView | null; onClose: () => void }) {
  const next = view?.next ?? [];
  const sent = view?.sent ?? [];
  const pending = view?.pending ?? 0;
  const copy = (text: string) => { void navigator.clipboard.writeText(text); };
  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={settings.seen_title}
      sub={settings.seen_sub}
      testId="settings.telemetry.sheet"
      onPrimary={onClose}
      foot={
        <Button kind="pri" onClick={onClose} testId="settings.telemetry.sheet.close">
          {settings.seen_close}
        </Button>
      }
    >
      <div className="tm-seen">
        <div className="tm-h">{settings.seen_id}</div>
        <div className="tm-id" data-testid="settings.telemetry.sheet.id">{view?.id ?? settings.seen_no_id}</div>
        {pending > 0 && <div className="tm-dim">{settings.seen_pending({ n: pending })}</div>}
        <div className="tm-h">{settings.seen_next}</div>
        {next.length === 0 && <div className="tm-dim">{settings.seen_none}</div>}
        {next.map((json, i) => (
          <Packet key={`n${String(i)}`} json={json} copy={copy} testId={`settings.telemetry.sheet.next.${String(i)}`} />
        ))}
        <div className="tm-h">{settings.seen_last}</div>
        {sent.length === 0 && <div className="tm-dim">{settings.seen_none}</div>}
        {sent.map((s, i) => (
          <Packet key={`s${String(i)}`} head={settings.seen_sent_on({ day: s.day, on: s.on })} json={s.json} copy={copy} testId={`settings.telemetry.sheet.sent.${String(i)}`} />
        ))}
      </div>
    </Sheet>
  );
}

function Packet({ head, json, copy, testId }: { head?: string; json: string; copy: (text: string) => void; testId: string }) {
  return (
    <div className="tm-packet" data-testid={testId}>
      <div className="tm-packet-head">
        <span className="tm-dim">{head}</span>
        <Act onClick={() => { copy(json); }} testId={`${testId}.copy`}>
          {settings.seen_copy}
        </Act>
      </div>
      <pre>{json}</pre>
    </div>
  );
}
