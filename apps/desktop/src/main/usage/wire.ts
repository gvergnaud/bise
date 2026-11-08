// Main's side of the app's usage counts (page usage-metrics-plan, step 3): the clock (clock.ts) fed
// by the windows' focus and blur, powerMonitor's suspend, resume, lock and unlock, his input (the
// windows' signal with no content, and his fn talk); the gestures (batch.ts) of the windows and of
// main itself (a renderer or a child process gone, the core's crash). Once a minute and before
// quit, one usage_time to the core, which counts it only when sharing is allowed. Never started
// in an off-screen run (index.ts): nothing of his Mac is read there. Owner: telemetry.
import { app, ipcMain, powerMonitor } from 'electron';
import type { AppGesture } from '../../proto';
import { IPC } from '../../shared/api';
import { UsageBatch } from './batch';
import { UsageClock, type ClockEvent } from './clock';

/** How often main tells the core. */
export const USAGE_EVERY_MS = 60_000;

let clock: UsageClock | null = null;
let batch: UsageBatch | null = null;
let send: ((cmd: unknown) => boolean) | null = null;

/** One gesture of main's own (a crash it saw); nothing when usage isn't started. */
export function countGesture(g: AppGesture): void {
  batch?.gesture(g);
}

/** His input main sees itself (his fn talk); nothing when usage isn't started. */
export function usageInput(): void {
  clock?.on('input', Date.now());
}

/** Tells the core what waits; kept for the next time when the core is down. */
export function flushUsage(): void {
  if (clock === null || batch === null || send === null) return;
  batch.add(clock.take(Date.now()));
  const cmd = batch.cmd();
  if (cmd !== null && send(cmd)) batch.sent();
}

/** Starts the clock and its listeners; `to` sends a command to the core (false: it's down). */
export function startUsage(to: (cmd: unknown) => boolean): void {
  if (clock !== null) return;
  const c = new UsageClock(Date.now());
  const b = new UsageBatch();
  clock = c;
  batch = b;
  send = to;
  const on = (e: ClockEvent) => (): void => {
    c.on(e, Date.now());
  };
  app.on('browser-window-focus', on('focus'));
  app.on('browser-window-blur', on('blur'));
  powerMonitor.on('suspend', on('suspend'));
  powerMonitor.on('resume', on('resume'));
  powerMonitor.on('lock-screen', on('lock'));
  powerMonitor.on('unlock-screen', on('unlock'));
  ipcMain.on(IPC.usageInput, on('input'));
  ipcMain.on(IPC.usageGesture, (_e, g: unknown) => {
    b.gesture(g);
  });
  app.on('render-process-gone', (_e, _contents, d) => {
    if (d.reason !== 'clean-exit') b.gesture('crash_renderer');
  });
  app.on('child-process-gone', (_e, d) => {
    if (d.reason !== 'clean-exit') b.gesture('crash_child');
  });
  setInterval(flushUsage, USAGE_EVERY_MS).unref();
}
