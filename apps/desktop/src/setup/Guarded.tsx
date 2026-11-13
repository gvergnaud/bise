// The folders macOS guards (Documents, Desktop, Downloads) that the projects scan skipped, so a new
// install never makes macOS ask before he did anything (setup-scan-tcc). One button per folder: his
// click sends found_scan {dir}, macOS asks right after it, the repos found join the list and the
// button goes; none: 'no repos in Documents'; refused: 'macOS said no to Documents' + 'Open Settings'
// (Privacy & Security › Files & Folders). Words and look: designer m_64068. Shown in the first launch's
// projects step (Onboarding.tsx) and in Settings' add-project sheet (AddSheet.tsx).
import { useState } from 'react';
import type { AppCmd, Looked } from '@/proto';
import { Act, Button } from '@/ds';
import { settings } from '@/proto/words.gen';
import { baseName } from './model';
import './guarded.css';

/** What the scan said of the guarded folders (the store's found_folders). */
export interface Folders {
  readonly guarded: readonly string[];
  readonly looked: readonly Looked[];
}

/** One guarded folder's row: its button, its wait, or what the look found. */
export type GuardedRow =
  | { kind: 'look'; path: string; name: string }
  | { kind: 'looking'; path: string; name: string }
  | { kind: 'none'; path: string; name: string }
  | { kind: 'refused'; path: string; name: string };

/**
 * The rows, in the core's order: a folder not looked in is a button ('looking' once he clicked it);
 * a look that found repos shows nothing (they joined the list); `asked`: each folder he clicked, with
 * the answer it had then (null: none), so only a newer answer ends its wait. Pure.
 */
export function guardedRows(f: Folders, asked: ReadonlyMap<string, Looked | null>): GuardedRow[] {
  const out: GuardedRow[] = [];
  const waits = (path: string, now: Looked | null): boolean => asked.has(path) && asked.get(path) === now;
  for (const path of f.guarded) out.push({ kind: waits(path, null) ? 'looking' : 'look', path, name: baseName(path) });
  for (const l of f.looked) {
    const name = baseName(l.path);
    if (waits(l.path, l)) out.push({ kind: 'looking', path: l.path, name });
    else if (l.refused) out.push({ kind: 'refused', path: l.path, name });
    else if (l.n === 0) out.push({ kind: 'none', path: l.path, name });
  }
  return out;
}

export interface GuardedProps {
  folders: Folders;
  send: (cmd: AppCmd) => void;
  /** System Settings › Privacy & Security › Files & Folders */
  openSettings: () => void;
  testId: string;
}

/** The line and the folders' buttons; nothing when no folder was skipped. */
export function Guarded({ folders, send, openSettings, testId }: GuardedProps) {
  const [asked, setAsked] = useState<ReadonlyMap<string, Looked | null>>(new Map());
  const rows = guardedRows(folders, asked);
  if (rows.length === 0) return null;
  const look = (path: string) => {
    setAsked((m) => new Map(m).set(path, folders.looked.find((l) => l.path === path) ?? null));
    send({ cmd: 'found_scan', dir: path });
  };
  return (
    <div className="setup-guarded" data-testid={testId}>
      <span className="setup-guarded-line">{settings.onb_guarded}</span>
      <div className="setup-guarded-rows">
        {rows.map((r) => {
          const id = `${testId}.${r.name}`;
          switch (r.kind) {
            case 'look':
              return (
                <Button key={r.path} size="sm" onClick={() => { look(r.path); }} testId={id}>
                  {settings.onb_look_in({ folder: r.name })}
                </Button>
              );
            case 'looking':
              return (
                <span key={r.path} className="setup-guarded-say" data-testid={id} data-state="looking">
                  {settings.onb_looking_in({ folder: r.name })}
                </span>
              );
            case 'none':
              return (
                <span key={r.path} className="setup-guarded-say none" data-testid={id} data-state="none">
                  {settings.onb_none_in({ folder: r.name })}
                </span>
              );
            case 'refused':
              return (
                <span key={r.path} className="setup-guarded-say" data-testid={id} data-state="refused">
                  {settings.onb_refused_in({ folder: r.name })}
                  <Act onClick={openSettings} testId={`${id}.settings`}>
                    {settings.onb_open_settings}
                  </Act>
                </span>
              );
          }
        })}
      </div>
    </div>
  );
}
