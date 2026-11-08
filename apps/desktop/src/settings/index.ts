// src/settings: settings, a route of the main window (bar S.3, S.4, 3.5,
// the settings side of S.2 and 2.6; the look of bise fait mac's mockup 10,
// r2-settings). Owner: r2-settings. Settings.tsx draws a page from props
// (the core's prefs, accounts, projects, found) and sends AppCmds; nav.tsx
// is its pages' nav, which the frame's sidebar shows on the route;
// pages.tsx, accounts.tsx and projects.tsx are its pages; prefs.ts reads
// his prefs; keys.ts is the app's key list; screen.tsx feeds it from
// src/store.
export { Settings, type SettingsData, type SettingsPorts, type SettingsProps } from './Settings';
export { PAGES, KeyList, type Page } from './pages';
export { Accounts, signedIn } from './accounts';
export { article, keyField, failedWords, fromWords, retries, shortModel, type ProviderCheck, type Sent } from './keycheck';
export type { KeyError } from './screen';
export { onboarded, type Prefs } from './prefs';
export { KEY_SECTIONS } from './keys';
export { SettingsScreen, prefsOf, useCmdError } from './screen';
export { SettingsNav, leaveSettings } from './nav';
export { needsSetup, modelOrSetup, modelRow, holdForSetup, releaseHeld, useHeld, heldReady, type Held } from './hold';
