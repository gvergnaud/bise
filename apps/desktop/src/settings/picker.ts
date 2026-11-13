// The system's folder picker as a port (his try round 3, point 2): settings · add project's 'choose…'
// and the first launch's 'choose a folder…' both take it. The bridge is amb-mac's
// (window.bise.pickFolder: a path, or null when he cancels; in test mode
// BISE_DESKTOP_TEST_PICK_FOLDER answers it without a dialog). No bridge: no port, no button.
// Owner: amb-kit.

/** His home folder as the bridge knows it (the found repos say ~/code/shop), else none. */
export function biseHome(): string | undefined {
  return (globalThis as { bise?: { home?: string } }).bise?.home;
}

/** System Settings › Privacy & Security › Files & Folders, when the shell has the bridge (a guarded folder macOS refused). */
export function openFilesSettings(): void {
  const bise = (globalThis as { bise?: { privacySettings?: (pane: 'files') => void } }).bise;
  bise?.privacySettings?.('files');
}

/** The picker port when the shell has one, else nothing. */
export function pickerPort(): { pickFolder?: () => Promise<string | null> } {
  const bise = (globalThis as { bise?: { pickFolder?: () => Promise<string | null> } }).bise;
  const pick = bise?.pickFolder;
  return pick ? { pickFolder: () => pick() } : {};
}
