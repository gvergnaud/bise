// settings · computer use (bar P.2, the TUI's /computer-use): on or off
// (the built-in `computer` plugin), and while it is on, its setup's steps
// as the TUI's screen shows them (Chrome, its extension, the live test,
// then the helper app's accessibility and screen recording), each with
// the button of its fix, only on his click. The page asks the core to
// check while it is open (`check` now and every 30 s, `leave` when it
// closes; the core stops by itself 2 min after the last check).
// Owner: amb-home.
import { useEffect, useRef, useState } from 'react';
import type { AppCmd, CoreEv } from '@/proto';
import { AppIcon, Button, Group, Help, Toggle, type AppIconName, type GroupItem } from '@/ds';
import { chromeCase } from './chrome';
import { ProfilesGroup } from './cuProfiles';
import { settings } from '@/proto/words.gen';

export type ComputerUse = Extract<CoreEv, { ev: 'computer_use' }>;

/** how often the open page says it is still open */
export const KEEP_MS = 30_000;

/** how long the core's flash shows under the button he clicked (designer m_43882) */
export const FLASH_MS = 3000;

/** a step's mark, a line icon each (designer m_43882): done check in ink, waits ask in the accent (nm-thread's '?' icon), the rest dot in text-3, failed alert in red */
const MARK: Record<string, AppIconName> = { done: 'check', waits: 'ask', checking: 'dot', failed: 'alert', not_yet: 'dot' };

/**
 * A numbered step's number and words ('1  ⏎ opens chrome://extensions · …', the core's help line):
 * the number goes in the mark column, the words hang under their first word. A line with no number:
 * none. Pure.
 */
export function stepOf(line: string): { n: string; text: string } {
  const m = /^(\d+)\s+(.*)$/su.exec(line);
  return m === null ? { n: '', text: line } : { n: m[1] ?? '', text: m[2] ?? '' };
}

export function Computer({ cu, send }: { cu: ComputerUse | null; send: (cmd: AppCmd) => void }) {
  // the screen hands a new `send` on each render (each core event): the check follows the page's
  // mount, never that identity, or each event would `leave` then `check` from scratch, the core
  // would answer with no steps, and so on forever (stuck on checking, the steps flickering)
  const sendRef = useRef(send);
  useEffect(() => {
    sendRef.current = send;
  }, [send]);
  useEffect(() => {
    const say = (act: string) => {
      sendRef.current({ cmd: 'computer_use', act });
    };
    say('check');
    const keep = window.setInterval(() => {
      say('check');
    }, KEEP_MS);
    return () => {
      window.clearInterval(keep);
      say('leave');
    };
  }, []);
  const on = cu?.on ?? false;
  // the core's flash ('opened · path copied') shows 3 s under the button he clicked, never at rest
  const [asked, setAsked] = useState<string | null>(null);
  useEffect(() => {
    if (asked === null) return undefined;
    const t = window.setTimeout(() => {
      setAsked(null);
    }, FLASH_MS);
    return () => {
      window.clearTimeout(t);
    };
  }, [asked]);
  const [removing, setRemoving] = useState(false);
  const row = (r: ComputerUse['rows'][number]): GroupItem => ({
    id: r.id,
    // designer (via cu-settings m_42955, m_43882): the step's line icon in a 16 px column on the
    // title's line, the sub lines aligned to the title's text, its numbered steps hanging on the column
    label: (
      <span className="settings-cu-t">
        <span className={`settings-cu-st ${r.state}`} aria-label={r.state} data-tip={r.state}>
          <AppIcon name={MARK[r.state] ?? 'dot'} size={14} />
        </span>
        {chromeCase(r.label)}
      </span>
    ),
    sub: (
      <span className="settings-cu-sub">
        {r.detail}
        {r.state !== 'done' &&
          (r.help ?? []).map((h) => {
            const s = stepOf(h);
            return (
              <span key={h} className="settings-cu-help">
                <span className="settings-cu-n">{s.n}</span>
                <span className="settings-cu-w">{s.text}</span>
              </span>
            );
          })}
      </span>
    ),
    control:
      r.action != null && r.fix != null ? (
        <span className="settings-cu-act">
          <Button
            state={r.state === 'checking' ? 'loading' : undefined}
            onClick={() => {
              if (r.fix != null) send({ cmd: 'computer_use', act: 'fix', fix: r.fix });
              setAsked(r.id);
            }}
            testId={`settings.cu.${r.id}`}
          >
            {chromeCase(r.action)}
          </Button>
          {asked === r.id && cu?.flash != null && (
            <span className="settings-cu-flash" data-testid={`settings.cu.${r.id}.flash`}>
              {cu.flash}
            </span>
          )}
        </span>
      ) : undefined,
  });
  const rows = cu?.rows ?? [];
  const web = rows.filter((r) => r.apps !== true);
  const apps = rows.filter((r) => r.apps === true);
  return (
    <>
      <Group
        testId="settings.cu"
        items={[
          {
            id: 'on',
            label: settings.cu_label,
            sub: on ? (cu?.ready === true ? settings.cu_ready : settings.cu_set_up) : settings.cu_off,
            control: (
              <Toggle
                on={on}
                label={settings.cu_switch}
                onChange={(v) => {
                  send({ cmd: 'computer_use', act: v ? 'on' : 'off' });
                }}
                testId="settings.cu.switch"
              />
            ),
          },
        ]}
      />
      {on && web.length === 0 && <Help>{settings.cu_checking}</Help>}
      {on && web.length > 0 && <Group title={settings.cu_web} testId="settings.cu.web" items={web.map(row)} />}
      {on && <ProfilesGroup profiles={cu?.profiles ?? []} send={send} />}
      {on && apps.length > 0 && <Group title={settings.cu_apps} testId="settings.cu.apps" items={apps.map(row)} />}
      {cu?.said != null && <Help err>{cu.said}</Help>}
      {/* the removal, a row at the page's end that asks once (designer m_43882): no red link */}
      {on && (
        <Group
          testId="settings.cu.remove"
          items={[
            {
              id: 'remove',
              label: settings.cu_remove_label,
              sub: removing ? settings.cu_remove_ask : settings.cu_remove_sub,
              control: removing ? (
                <span className="settings-row-acts">
                  <Button
                    kind="danger"
                    onClick={() => {
                      send({ cmd: 'computer_use', act: 'uninstall' });
                      setRemoving(false);
                    }}
                    testId="settings.cu.uninstall"
                  >
                    {settings.cu_remove_yes}
                  </Button>
                  <Button
                    onClick={() => {
                      setRemoving(false);
                    }}
                    testId="settings.cu.remove.keep"
                  >
                    {settings.cu_remove_keep}
                  </Button>
                </span>
              ) : (
                <Button
                  kind="danger"
                  onClick={() => {
                    setRemoving(true);
                  }}
                  testId="settings.cu.remove.ask"
                >
                  {settings.cu_remove_button}
                </Button>
              ),
            },
          ]}
        />
      )}
    </>
  );
}
