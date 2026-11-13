// His switch 'Read skills in Documents, Desktop or Downloads' atop the skills folders group
// (skills-scan-tcc, architect m_64243; look and words designer m_64245, m_64248): shown only when a
// skill links into a folder macOS guards. Off (his default) the core lists those skills by their folder
// name and never reads them; his click on sends `skills_read_guarded {on}`, the core reads them once
// (macOS may ask him then) and, when macOS says no, turns it back off: the line says 'macOS said no to
// Documents' with 'Open Settings' (Privacy & Security › Files & Folders), the setup's words.
import type { AppCmd, Plugin } from '@/proto';
import { Act, Toggle } from '@/ds';
import { settings } from '@/proto/words.gen';
import { openFilesSettings } from '@/settings/picker';

/** The skills linked into a guarded folder, and those folders in first-seen order. Pure. */
export function guardedOf(rows: readonly Plugin[]): { n: number; folders: string[] } {
  const folders: string[] = [];
  let n = 0;
  for (const p of rows) {
    for (const x of p.parts ?? []) {
      if (x.kind !== 'skill' || x.guarded == null) continue;
      n += 1;
      if (!folders.includes(x.guarded.folder)) folders.push(x.guarded.folder);
    }
  }
  return { n, folders };
}

/** `Documents`, `Documents and Desktop`, `Documents, Desktop and Downloads`. Pure. */
export function folderList(folders: readonly string[]): string {
  if (folders.length <= 1) return folders[0] ?? '';
  return settings.plugins_and({ a: folders.slice(0, -1).join(', '), b: folders[folders.length - 1] ?? '' });
}

/** Whether he turned it on, from his prefs (`skills.read_guarded`). Pure. */
export function readGuardedOn(prefs: unknown): boolean {
  const skills = (prefs as { skills?: { read_guarded?: unknown } } | null)?.skills;
  return skills?.read_guarded === true;
}

export interface GuardedSwitchProps {
  rows: readonly Plugin[];
  on: boolean;
  /** the folders macOS refused at his last turn on (the plugins event's skills_refused) */
  refused: readonly string[];
  project?: string;
  send: (cmd: AppCmd) => void;
  openSettings?: () => void;
}

export function GuardedSwitch({ rows, on, refused, project, send, openSettings = openFilesSettings }: GuardedSwitchProps) {
  const { n, folders } = guardedOf(rows);
  if (n === 0 && refused.length === 0) return null;
  const at = project === undefined ? {} : { project };
  return (
    <div className="plg-row plg-guarded" data-testid="plugins.guarded">
      <span className="plg-row-words">
        <span className="plg-title">{settings.plugins_read_guarded}</span>
        {refused.length > 0 ? (
          <span className="plg-row-what plg-guarded-said" data-testid="plugins.guarded.refused">
            {settings.onb_refused_in({ folder: folderList(refused) })}
            <Act onClick={openSettings} testId="plugins.guarded.settings">{settings.onb_open_settings}</Act>
          </span>
        ) : (
          <span className="plg-row-what" data-testid="plugins.guarded.count">{settings.plugins_guarded_count({ n, folders: folderList(folders) })}</span>
        )}
      </span>
      <span className="plg-row-end">
        <Toggle on={on} label={settings.plugins_read_guarded} onChange={(v) => { send({ cmd: 'skills_read_guarded', ...at, on: v }); }} testId="plugins.guarded.toggle" />
      </span>
    </div>
  );
}
