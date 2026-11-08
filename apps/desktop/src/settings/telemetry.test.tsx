// @vitest-environment happy-dom
// Settings › Privacy's usage counts and Gabriel's note (usage-metrics-plan step 6): the pure words,
// the acts each control sends, and the note marked seen only once drawn. Fake data only.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { AppCmd } from '@/proto';
import { settings } from '@/proto/words.gen';
import { deleteSub, forcedOff, pretty, shareSub, TelemetryGroup, type TelemetryView } from './telemetry';
import { HOLES, holes, homeNoteDue, OFF_CMD, OFF_ENV, TelemetryNote } from '@/setup/TelemetryNote';

afterEach(cleanup);

const view = (v: Partial<TelemetryView> = {}): TelemetryView => ({ ev: 'telemetry', share: true, pending: 0, ...v });

describe('usage counts in Settings › Privacy', () => {
  it('locks the switch only when something else turns it off', () => {
    expect(forcedOff('env')).toBe(settings.privacy_off_env);
    expect(forcedOff('do_not_track')).toBe(settings.privacy_off_dnt);
    expect(forcedOff('dev_build')).toBe(settings.privacy_off_dev);
    for (const off of [undefined, 'pref', 'not_yet', 'note_not_seen', 'offscreen', 'unknown'] as const) expect(forcedOff(off)).toBeNull();
    expect(shareSub(null)).toBe(settings.privacy_share_sub);
    expect(shareSub(view({ off: 'not_yet' }))).toBe(settings.privacy_not_yet);
    expect(deleteSub('asked')).toBe(settings.privacy_delete_asked);
    expect(deleteSub(undefined)).toBe(settings.privacy_delete_sub);
    // a packet reads one key a line, never cut inside a token (designer m_56524)
    expect(pretty('{"day":"2026-11-08","n":{"agents_started":3}}')).toBe('{\n  "day": "2026-11-08",\n  "n": {\n    "agents_started": 3\n  }\n}');
    expect(pretty('not json')).toBe('not json');
  });

  it('asks the view once, and each control sends its act; delete only after its confirm', () => {
    const sent: AppCmd[] = [];
    const send = (c: AppCmd) => { sent.push(c); };
    render(<TelemetryGroup prefs={{ telemetry: { share: true } }} view={view()} send={send} />);
    expect(sent).toEqual([{ cmd: 'telemetry', act: 'show' }]);
    fireEvent.click(screen.getByTestId('settings.pref.telemetry_share'));
    fireEvent.click(screen.getByTestId('settings.telemetry.reset'));
    fireEvent.click(screen.getByTestId('settings.telemetry.delete'));
    expect(sent.map((c) => (c.cmd === 'telemetry' ? c.act : c.cmd))).toEqual(['show', 'off', 'reset']);
    expect(screen.getByText(settings.privacy_delete_ask)).toBeTruthy();
    fireEvent.click(screen.getByTestId('settings.telemetry.delete.yes'));
    expect(sent.at(-1)).toEqual({ cmd: 'telemetry', act: 'delete' });
  });

  it('a forced-off switch is off and disabled, with why', () => {
    render(<TelemetryGroup prefs={{}} view={view({ off: 'env' })} send={() => {}} />);
    const sw = screen.getByTestId('settings.pref.telemetry_share');
    expect(sw.getAttribute('aria-checked')).toBe('false');
    expect((sw as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(settings.privacy_off_env)).toBeTruthy();
  });

  it("'See what's sent' shows the ID and each packet", () => {
    render(<TelemetryGroup prefs={{}} view={view({ id: 'fake-id-1', next: ['{"day":"2026-11-07"}'], sent: [{ on: '2026-11-07', day: '2026-11-06', json: '{}' }] })} send={() => {}} />);
    fireEvent.click(screen.getByTestId('settings.telemetry.seen'));
    expect(screen.getByTestId('settings.telemetry.sheet.id').textContent).toBe('fake-id-1');
    expect(screen.getByTestId('settings.telemetry.sheet.next.0').textContent).toContain('2026-11-07');
    expect(screen.getByTestId('settings.telemetry.sheet.sent.0').textContent).toContain(settings.seen_sent_on({ day: '2026-11-06', on: '2026-11-07' }));
  });
});

describe("Gabriel's note", () => {
  it('draws its command and variable as code', () => {
    expect(holes(settings.onb_telemetry_b_off({ cmd: HOLES[0], env: HOLES[1] }), [OFF_CMD, OFF_ENV]).filter((p) => p.code).map((p) => p.text)).toEqual([OFF_CMD, OFF_ENV]);
  });

  it('is marked seen when drawn, and its switch sends his choice', () => {
    const send = vi.fn();
    render(<TelemetryNote share send={send} testId="note" />);
    expect(send).toHaveBeenCalledWith({ cmd: 'telemetry', act: 'note_seen' });
    fireEvent.click(screen.getByTestId('note.share'));
    expect(send).toHaveBeenLastCalledWith({ cmd: 'telemetry', act: 'off' });
    expect(screen.queryByTestId('note.done')).toBeNull();
  });

  it('shows on the home only for an onboarded install that never saw it', () => {
    expect(homeNoteDue({ onboarded: true })).toBe(true);
    expect(homeNoteDue({ onboarded: true, telemetry: { note_seen: true } })).toBe(false);
    expect(homeNoteDue({ onboarded: false })).toBe(false);
  });
});
