// Gabriel's note about the usage counts (page usage-metrics-plan, step 6; designer m_56350): the
// short note (A) in a quiet raised block, its 'why?' opening the long one (B) in a sheet, and the
// share switch on its last line, so turning it off is one click where it's announced. Drawn on
// hello (Onboarding.tsx) and, once, docked over the home's composer for an install that never
// saw it (HomeNote). Drawing it marks it seen (telemetry {act: note_seen}): without that, bise
// counts nothing. The words live once in bise_proto (words/settings/setup.rs). Owner: telemetry.
import { Fragment, useEffect, useId, useState, type ReactNode } from 'react';
import type { AppCmd } from '@/proto';
import { Act, Button, Sheet, Toggle } from '@/ds';
import { settings } from '@/proto/words.gen';
import { flag, type Prefs } from '@/settings/prefs';

/** The variable and the command that turn it off, drawn as code. */
export const OFF_ENV = 'BISE_TELEMETRY=0';
export const OFF_CMD = 'bise telemetry off';

/** Two private-use characters the words never hold: where the code goes. */
export const HOLES = ['\uE000', '\uE001'] as const;

/**
 * Words made with HOLES in place of `codes`, cut into text and code parts, in order. Pure.
 * `holes(settings.onb_telemetry_off({ env: HOLES[0] }), [OFF_ENV])`.
 */
export function holes(words: string, codes: readonly string[]): readonly { text: string; code: boolean }[] {
  const out: { text: string; code: boolean }[] = [];
  for (const part of words.split(/([\uE000\uE001])/)) {
    const i = HOLES.indexOf(part as (typeof HOLES)[number]);
    if (i >= 0) out.push({ text: codes[i] ?? '', code: true });
    else if (part !== '') out.push({ text: part, code: false });
  }
  return out;
}

function Coded({ parts }: { parts: readonly { text: string; code: boolean }[] }) {
  return (
    <>
      {parts.map((p, i) => (
        <Fragment key={i}>{p.code ? <code>{p.text}</code> : p.text}</Fragment>
      ))}
    </>
  );
}

/** His `telemetry.share` (on unless he said no), from the core's prefs. Pure. */
export const shareOf = (prefs: Prefs | null): boolean => flag(prefs, 'telemetry.share', true);

/** The command for his switch. Pure. */
export const shareCmd = (on: boolean): AppCmd => ({ cmd: 'telemetry', act: on ? 'on' : 'off' });

export const NOTE_SEEN: AppCmd = { cmd: 'telemetry', act: 'note_seen' };

export interface TelemetryNoteProps {
  share: boolean;
  send: (cmd: AppCmd) => void;
  /** the home's 'Got it' (none on hello: its start button moves on) */
  done?: () => void;
  testId: string;
}

export function TelemetryNote({ share, send, done, testId }: TelemetryNoteProps) {
  const [why, setWhy] = useState(false);
  // shown is seen: drawn once, the core may count (never before it was drawn)
  useEffect(() => {
    send(NOTE_SEEN);
  }, [send]);
  const id = useId();
  return (
    <div className="tm-note" data-testid={testId}>
      <div className="tm-t">{settings.onb_telemetry_title}</div>
      <p>{settings.onb_telemetry_why_line}</p>
      <p>{settings.onb_telemetry_what}</p>
      <p>{settings.onb_telemetry_never}</p>
      <p>
        <Coded parts={holes(settings.onb_telemetry_off({ env: HOLES[0] }), [OFF_ENV])} />{' '}
        <Act onClick={() => { setWhy(true); }} testId={`${testId}.why`}>
          {settings.onb_telemetry_why}
        </Act>
      </p>
      <label className="tm-share" htmlFor={id}>
        <span>{settings.privacy_share}</span>
        <Toggle id={id} on={share} onChange={(on) => { send(shareCmd(on)); }} testId={`${testId}.share`} />
      </label>
      <div className="tm-foot">
        <span className="tm-sign">{settings.onb_telemetry_sign}</span>
        {done !== undefined && (
          <Button kind="ghost" size="sm" onClick={done} testId={`${testId}.done`}>
            {settings.onb_telemetry_got_it}
          </Button>
        )}
      </div>
      <WhySheet open={why} onClose={() => { setWhy(false); }} />
    </div>
  );
}

/** B, the long note: why bise counts, what it sends, what never, the ID, the way out. */
export function WhySheet({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={settings.onb_telemetry_b_title}
      testId="telemetry.why"
      onPrimary={onClose}
      foot={
        <Button kind="pri" onClick={onClose} testId="telemetry.why.close">
          {settings.seen_close}
        </Button>
      }
    >
      <div className="tm-why">
        <p>{settings.onb_telemetry_b_why}</p>
        <p>{settings.onb_telemetry_b_what}</p>
        <p>{settings.onb_telemetry_b_never}</p>
        <p>{settings.onb_telemetry_b_id}</p>
        <p>
          <Coded parts={holes(settings.onb_telemetry_b_off({ cmd: HOLES[0], env: HOLES[1] }), [OFF_CMD, OFF_ENV])} />
        </p>
        <p className="tm-sign">{settings.onb_telemetry_sign}</p>
      </div>
    </Sheet>
  );
}

/**
 * Whether the home shows the note: latched from the first prefs of this window (an install that
 * never saw it), so marking it seen as it's drawn doesn't take it away; 'Got it' or his first
 * message does. Returns [shown, dismiss].
 */
export function useHomeNote(prefs: Prefs | null): readonly [boolean, () => void] {
  const [state, setState] = useState<'unknown' | 'show' | 'gone'>('unknown');
  // latched at the first prefs (state set while rendering: React's derived-state pattern)
  if (state === 'unknown' && prefs !== null) setState(homeNoteDue(prefs) ? 'show' : 'gone');
  return [state === 'show', () => { setState('gone'); }];
}

/** An install that went through (or skipped) the first launch and never saw the note. Pure. */
export function homeNoteDue(prefs: Prefs): boolean {
  return flag(prefs, 'onboarded', false) && !flag(prefs, 'telemetry.note_seen', false);
}

/** The note docked over the home's composer, once (designer m_56350: never a modal, never pink). */
export function HomeNote({ prefs, shown, dismiss, send }: { prefs: Prefs | null; shown: boolean; dismiss: () => void; send: (cmd: AppCmd) => void }): ReactNode {
  if (!shown) return null;
  return (
    <div className="tm-dock">
      <TelemetryNote share={shareOf(prefs)} send={send} done={dismiss} testId="home.telemetry" />
    </div>
  );
}
