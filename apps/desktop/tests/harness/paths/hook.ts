// Electron's own folders under the test home, set before the app's main runs (main m_59528, architect
// m_59538). Why: Electron on macOS ignores $HOME: with HOME=/tmp/x, app.getPath('home'), 'documents',
// 'downloads', 'appData' ... are all under his real home (os.homedir() follows HOME, Electron doesn't),
// so ui-review's FakeCore specs wrote ~/w and ~/Library/CloudStorage/bise/Shared/Bise growth into it.
// launch.ts loads this file with Electron's -r on every launch: it refuses (exit 3, before the app
// runs) a HOME that is his real home or that the harness did not mark as a test home, then points
// every Electron folder under that HOME. userData stays the harness's (BISE_DESKTOP_USER_DATA, set by
// the app's test mode); sessionData follows it. A CommonJS script, emitted from this .ts by
// scripts/ts_emit.ts and committed (Electron's node doesn't strip types).
// a CommonJS script Electron loads with -r: require is its import, typed by its annotation
// eslint-disable-next-line @typescript-eslint/no-require-imports, @typescript-eslint/no-unsafe-assignment
const { existsSync, mkdirSync, realpathSync }: typeof import('node:fs') = require('node:fs');
// eslint-disable-next-line @typescript-eslint/no-require-imports, @typescript-eslint/no-unsafe-assignment
const os: typeof import('node:os') = require('node:os');
// eslint-disable-next-line @typescript-eslint/no-require-imports, @typescript-eslint/no-unsafe-assignment
const path: typeof import('node:path') = require('node:path');

/** The file the harness writes in every home it makes or adopts (isolation.ts testHome, adoptHome). */
export const MARKER = '.bise-test-home';

/** Electron's folders the hook sets, each under the test HOME ('' = HOME itself). */
export const PATHS: Readonly<Record<string, string>> = {
  home: '',
  appData: 'Library/Application Support',
  documents: 'Documents',
  desktop: 'Desktop',
  downloads: 'Downloads',
  music: 'Music',
  pictures: 'Pictures',
  videos: 'Movies',
  logs: 'Library/Logs/bise-test',
  crashDumps: 'Library/Application Support/bise-test/Crashpad',
};

/**
 * Why `home` can't be a launch's HOME, or null. Pure (self/isolation.spec.ts). A test home is told by
 * the harness's marker, never by its path (a home that moves keeps it); his real home never is one.
 */
export function testHomeOk(home: string | undefined, realHome: string, exists: (p: string) => boolean): string | null {
  if (home === undefined || home === '') return 'no HOME';
  if (path.resolve(home) === path.resolve(realHome)) return `HOME is his real home ${realHome}`;
  if (!exists(path.join(home, MARKER))) return `HOME ${home} holds no ${MARKER}: not a home the harness made`;
  return null;
}

const real = (p: string): string => {
  try { return realpathSync(p); } catch { return path.resolve(p); }
};

// only in Electron's main process: a spec that imports testHomeOk (node) runs none of this
if ('electron' in process.versions && (process as { type?: string }).type === 'browser') {
  // eslint-disable-next-line @typescript-eslint/no-require-imports, @typescript-eslint/no-unsafe-assignment
  const { app }: typeof import('electron') = require('electron');
  const home = process.env['HOME'] === undefined ? undefined : real(process.env['HOME']);
  const why = testHomeOk(home, real(os.userInfo().homedir), existsSync);
  if (why !== null || home === undefined) {
    process.stderr.write(`harness paths: refused to start: ${why ?? 'no HOME'}
`);
    process.exit(3);
  }
  for (const [name, rel] of Object.entries(PATHS)) {
    const dir = path.join(home, rel);
    mkdirSync(dir, { recursive: true });
    app.setPath(name, dir);
  }
  const tmp = process.env['TMPDIR'];
  if (tmp !== undefined && tmp !== '') app.setPath('temp', tmp);
  const userData = process.env['BISE_DESKTOP_USER_DATA'];
  if (userData !== undefined && userData !== '') {
    mkdirSync(userData, { recursive: true });
    app.setPath('userData', userData);
    app.setPath('sessionData', userData);
  }
}
