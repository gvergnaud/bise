// stays JS: the release config, read by electron-builder's own loader (no TS there).
// Packaging bise desktop (electron-builder; `pnpm dist`). Owner: amb-mac.
// The app carries what it runs, so nothing is looked up on PATH:
//   Contents/Resources/bin/bise          the bise it runs (src/main/paths.ts binPath); in a
//                                        release, a link into bin/bise.app (dev.bise.cli), made
//                                        after the pack by scripts/desktop_release.py cli_bundle
//   Contents/Resources/bin/VERSION, repl-live, sb-core, bend-jsrt, prompts/, plugins/, kit/,
//                                        computer-use/extension: the runtime (BISE_PACK_ROOT),
//                                        a version dir's layout; the core installs it as
//                                        <prefix>/versions/<id> and runs hubs from there
//   Contents/MacOS/bise-mac-helper       the native helper, next to the app's executable
//                                        so macOS charges its permissions to the app
//                                        (src/main/helper/run.ts helperPath)
//   Contents/Resources/pearl/*.png       the tray's pearl frames (src/main/tray.ts)
//   Contents/Resources/VERSION           the app's build (id=, built= UTC with a Z), from
//                                        BISE_PACK_VERSION when given: the updater says it to
//                                        the core (src/main/update); none = a dev build, never
//                                        offered an update
//   Contents/Info.plist                  CFBundleShortVersionString and CFBundleVersion from that
//                                        VERSION too (bundleVersions), never package.json's 0.0.0
// Its icon is resources/icon.icns (his pick on page app-icon: the pink pillow with the sidebar's
// ':*', each size from 16 to 1024 drawn at its size, the mark thicker when small); the same for
// every build (his 'bise try', the release and its DMG, dev). resources/icon.png is its 1024 px,
// docs/brand/app-icon/icon.svg its source.
// A try build (amb-tools' 'bise-ambient --try') gets its own identity so it
// never shares his app's userData, single-instance lock or permissions:
// BISE_PACK_APP_ID (default dev.bise.desktop) and BISE_PACK_NAME (default
// 'bise'; it names the .app, the menus and ~/Library/Application Support/<name>).
// Both binaries and the runtime are built before, never here (the Rust build
// takes the heavy lock): BISE_PACK_BISE, BISE_PACK_HELPER and BISE_PACK_ROOT
// give their paths; a missing one stops the build with its name.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

function need(name) {
  const p = process.env[name];
  if (p === undefined || p === '' || !existsSync(p)) {
    throw new Error(`electron-builder.mjs: ${name} must be the path of a built binary (got '${p ?? ''}')`);
  }
  return p;
}

const bise = need('BISE_PACK_BISE');
const helper = need('BISE_PACK_HELPER');
// the runtime bise runs its hubs with (architect m_18354): a version dir's layout (VERSION,
// repl-live, sb-core, bend-jsrt, prompts/, plugins/, kit/, computer-use/extension), packed next to
// bise in Contents/Resources/bin, so the app finds it with no env (approot: VERSION next to the
// executable). Its core installs it once as <prefix>/versions/<id> and runs hubs from there, never
// from inside the .app. Required: a bundle without it could start no hub.
const root = need('BISE_PACK_ROOT');
for (const f of ['VERSION', 'repl-live', 'sb-core']) {
  if (!existsSync(join(root, f))) throw new Error(`electron-builder.mjs: BISE_PACK_ROOT (${root}) has no ${f}: not a runtime`);
}
// whole for the artifact viewer (office/dist with the xlsx grid, built, never committed: a root
// without it shipped an app whose xlsx viewer imported a 404, xlsx-fix): scripts/runtime_check.sh
// says what is missing, and no pack goes on without it
try {
  execFileSync('bash', [fileURLToPath(new URL('../../scripts/runtime_check.sh', import.meta.url)), root], { stdio: ['ignore', 'inherit', 'pipe'] });
} catch (e) {
  throw new Error(`electron-builder.mjs: BISE_PACK_ROOT (${root}) is not whole: ${String(e.stderr ?? e.message).trim()}`);
}
// optional: the release's VERSION file (make-release writes id= and built=)
const version = process.env['BISE_PACK_VERSION'] ? need('BISE_PACK_VERSION') : null;

// Info.plist's versions from that VERSION (`key=value` lines), else package.json's 0.0.0 stays
// in CFBundleShortVersionString and CFBundleVersion: Finder's Get Info, the crash reports, and the
// About panel's build when main sets none. The short one is the build's name (`version=`, else
// `id=`) as he reads it in About and the settings: src/shared/version.ts shortVersion's rule
// ('desktop-2026.10.2' and 'desktop-v2026.10.2' → '2026.10.2'; this file runs in plain node, so it
// can't import the .ts: change both together, version.test.ts and desktop_release_test.py check
// each). The build one is its commit (12 chars), else that name again. A VERSION without an id
// leaves both alone. Pure.
export function bundleVersions(text) {
  const kv = new Map();
  for (const line of text.split('\u000a')) {
    const i = line.indexOf('=');
    if (i > 0) kv.set(line.slice(0, i).trim(), line.slice(i + 1).trim());
  }
  const raw = kv.get('version') || kv.get('id') || '';
  if (raw === '') return {};
  const name = raw.replace(/^desktop-v?(?=\d)/, '').replace(/^v(?=\d)/, '');
  const commit = kv.get('commit') || '';
  return { bundleShortVersion: name, bundleVersion: commit === '' ? name : commit.slice(0, 12) };
}

// BISE_PACK_SIGN_DIR: bise's stable self-signed identity ('bise ambient dev'
// in <dir>/sign.keychain-db, made once by scripts/sign-identity.sh when
// missing; the Swift app's build made the same one), so macOS keeps his
// grants across rebuilds. Its
// certificate isn't trusted by the codesigning policy, so electron-builder
// won't pick it: the app is signed here, deep, after packing.
const signDir = process.env['BISE_PACK_SIGN_DIR'];

function signStable(appPath) {
  const kc = join(signDir, 'sign.keychain-db');
  const pass = readFileSync(join(signDir, 'pass'), 'utf8');
  execFileSync('security', ['unlock-keychain', '-p', pass, kc]);
  const ids = execFileSync('security', ['find-identity', '-p', 'codesigning', kc], { encoding: 'utf8' });
  const id = /\b([0-9A-F]{40}) "bise ambient dev"/.exec(ids)?.[1];
  if (id === undefined) throw new Error(`electron-builder.mjs: no 'bise ambient dev' identity in ${kc}`);
  // codesign finds identities only in the search list: the keychain joins it for the signing
  const orig = execFileSync('security', ['list-keychains', '-d', 'user'], { encoding: 'utf8' })
    .split('\n')
    .map((l) => l.trim().replace(/"/g, ''))
    .filter((l) => l !== '');
  execFileSync('security', ['list-keychains', '-d', 'user', '-s', ...orig, kc]);
  try {
    execFileSync('codesign', ['--force', '--deep', '--sign', id, '--keychain', kc, appPath]);
  } finally {
    execFileSync('security', ['list-keychains', '-d', 'user', '-s', ...orig]);
  }
  execFileSync('codesign', ['--verify', '--deep', '--strict', appPath]);
}

// Electron's fuses (apple-audit item 2): left at their defaults, any local program runs its own
// code as bise (ELECTRON_RUN_AS_NODE=1, NODE_OPTIONS, --inspect) and inherits his grants (micro,
// Apple Events, Accessibility). The app uses none of the three. Flipped in afterPack, before any
// signature (signStable here, electron-builder's own after afterPack, desktop_sign.py's for a
// release): a flip breaks the seal. electron-builder's `electronFuses` runs after afterPack, so
// after signStable: not used. Kept on: GrantFileProtocolExtraPrivileges (the renderer loads from
// file://); off for now: EnableCookieEncryption (needs a migration of his cookies).
// scripts/desktop_release.py's gates read them back in the packed app (its `fuses`).
export const FUSES = {
  RunAsNode: false,
  EnableNodeOptionsEnvironmentVariable: false,
  EnableNodeCliInspectArguments: false,
  EnableEmbeddedAsarIntegrityValidation: true,
  OnlyLoadAppFromAsar: true,
};

async function lockFuses(appPath) {
  const { flipFuses, FuseVersion, FuseV1Options } = await import('@electron/fuses');
  const wire = { version: FuseVersion.V1 };
  for (const [name, on] of Object.entries(FUSES)) wire[FuseV1Options[name]] = on;
  await flipFuses(appPath, wire);
}

const appId = process.env['BISE_PACK_APP_ID'] || 'dev.bise.desktop';
const productName = process.env['BISE_PACK_NAME'] || 'bise';

export default {
  appId,
  afterPack: async (ctx) => {
    if (ctx.electronPlatformName !== 'darwin') return;
    const app = join(ctx.appOutDir, `${ctx.packager.appInfo.productFilename}.app`);
    await lockFuses(app);
    if (signDir !== undefined) signStable(app);
  },
  productName,
  // BISE_PACK_OUT: another output dir (the try's staging, while his try runs from dist: amb-tools)
  directories: { output: process.env['BISE_PACK_OUT'] || 'dist' },
  // never a source map (the nightly's coverage-map build makes them: BISE_DESKTOP_SOURCEMAP)
  files: ['out/**', '!out/**/*.map', 'package.json'],
  asar: true,
  extraResources: [
    // the runtime first, then bise over it (a snapshot's own bise never wins); its rust/
    // (an old path's hard link to bend-jsrt) would be a second 100 MB copy; its bise.app (a
    // release's, or a dev version's) neither: a release build makes bin/bise.app around this bise
    // after the pack (scripts/desktop_release.py cli_bundle, the terminal's one bundle builder)
    { from: root, to: 'bin', filter: ['**/*', '!rust/**', '!bise', '!bise.app', '!bise.app/**'] },
    { from: bise, to: 'bin/bise' },
    ...(version === null ? [] : [{ from: version, to: 'VERSION' }]),
  ],
  mac: {
    category: 'public.app-category.developer-tools',
    icon: 'resources/icon.icns',
    target: ['dir'],
    // signing: electron-builder rewrites Info.plist and adds the helper and
    // bise, which breaks Electron's own seal, so a dir build is re-signed
    // deep: with BISE_PACK_SIGN_DIR by signStable (above), else
    // BISE_PACK_IDENTITY names a codesigning identity (CSC_KEYCHAIN: its
    // keychain), else ad hoc ('-'). BISE_PACK_IDENTITY=none: not signed here
    // at all (a release: scripts/desktop_release.py, whose desktop_sign.py
    // signs every piece inside out right after; the deep ad hoc pass cost
    // 16 s for nothing). No hardened runtime here: a release re-signs it with
    // the Developer ID and its entitlements after (scripts/desktop_sign.py).
    identity: signDir !== undefined || process.env['BISE_PACK_IDENTITY'] === 'none' ? null : (process.env['BISE_PACK_IDENTITY'] ?? '-'),
    hardenedRuntime: false,
    // never notarize here: electron-builder 26 starts its own notarization as
    // soon as APPLE_API_KEY_ID/APPLE_API_ISSUER (or APPLE_ID...) are in the env,
    // on the ad hoc app; a release notarizes the signed app and DMG itself with
    // notarytool (scripts/desktop_release.py notarize)
    notarize: false,
    extraFiles: [{ from: helper, to: 'MacOS/bise-mac-helper' }],
    ...(version === null ? {} : bundleVersions(readFileSync(version, 'utf8'))),
    extendInfo: {
      NSMicrophoneUsageDescription: 'bise listens while you hold fn, to hear what you ask, and when a plugin view you allowed records.',
      NSCameraUsageDescription: 'A plugin view you allowed to use the camera shows or takes a picture.',
      NSAppleEventsUsageDescription: 'bise reads the address of the tab in front when you hold fn.',
    },
  },
};
