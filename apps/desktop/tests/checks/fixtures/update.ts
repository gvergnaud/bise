// A throwaway update feed for qa-native's setup/update (bar S.6; owner:
// amb-mac). Everything goes inside the run's test HOME: a fake running app
// (<home>/Applications/bise.app, the updater's BISE_DESKTOP_UPDATE_TARGET),
// and two file:// channels (<home>/feed: stable's desktop.json naming a newer
// desktop build + its zip; <home>/feed/beta: beta's), for the core's
// BISE_DIST_URL_DESKTOP(_BETA). Never his app, his feed or his keychains.
//
// Signing: codesign signs only from keychains in his search list, so a
// throwaway identity would mean touching that list (lead m_11312: never).
// Both apps are ad-hoc signed instead: an ad-hoc app's designated
// requirement is its own cdhash, so the "newer" build that passes is an
// exact copy of the running one (newer by desktop.json's id and built, the
// only thing the core compares), and the swap is seen on disk (a new
// directory in place, the old one at <name>.app.old). The bad variants
// prove the fail-closed path: 'tampered' (a sealed file changed after
// signing: --verify fails) and 'other' (validly signed but another build:
// the running app's requirement refuses it).
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export type UpdateBuild = 'good' | 'tampered' | 'other';

export interface UpdateFeed {
  /** the fake running app (BISE_DESKTOP_UPDATE_TARGET) */
  readonly target: string;
  /** the stable channel's base URL (the core's BISE_DIST_URL_DESKTOP; beta: <feed>/beta, BISE_DIST_URL_DESKTOP_BETA) */
  readonly feedUrl: string;
  /** what desktop.json offers */
  readonly version: string;
  /** the env to add to the app's launch (the core inherits the channels' URLs) */
  readonly env: Record<string, string>;
}

/** The feed's channels: 'beta' offers the build on desktop-beta only (stable has the running one); paused: stable says paused. */
export interface FeedOpts {
  readonly channel?: 'stable' | 'beta';
  readonly paused?: boolean;
}

/** The running app's build: older than the feed's. */
export const RUNNING = { id: 'd25', built: '2026-10-07T10:00:00Z' } as const;
/** The feed's build. */
export const OFFERED = { version: '2026.10.2-99', id: 'd99', built: '2026-10-08T10:00:00Z' } as const;

const run = (cmd: string, args: string[]): string => execFileSync(cmd, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });

/**
 * A real one-file asar archive (@electron/asar's layout: a size pickle, the header JSON's pickle,
 * the data), as a packed app's Contents/Resources/app.asar: Electron's fs reads it as a folder, so
 * an updater that fs.rm's a bundle stops on it (go-back-banner); a fake bundle without one hid that.
 */
export function asarArchive(): Buffer {
  const data = Buffer.from('module.exports = 1;');
  const json = Buffer.from(JSON.stringify({ files: { 'index.js': { size: data.length, offset: '0' } } }));
  const padded = Buffer.alloc(Math.ceil(json.length / 4) * 4);
  json.copy(padded);
  const header = Buffer.alloc(8 + padded.length);
  header.writeUInt32LE(4 + padded.length, 0);
  header.writeInt32LE(json.length, 4);
  padded.copy(header, 8);
  const size = Buffer.alloc(8);
  size.writeUInt32LE(4, 0);
  size.writeUInt32LE(header.length, 4);
  return Buffer.concat([size, header, data]);
}

/** A minimal ad-hoc-signed .app at `app` with this VERSION and an app.asar (never run: its executable is a copy of /usr/bin/true). */
function fakeApp(app: string, version: { id: string; built: string }): void {
  rmSync(app, { recursive: true, force: true });
  mkdirSync(join(app, 'Contents', 'MacOS'), { recursive: true });
  mkdirSync(join(app, 'Contents', 'Resources'), { recursive: true });
  writeFileSync(
    join(app, 'Contents', 'Info.plist'),
    `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.bise.desktop.updatetest</string>
<key>CFBundleExecutable</key><string>bise</string>
<key>CFBundleName</key><string>bise</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
`,
  );
  cpSync('/usr/bin/true', join(app, 'Contents', 'MacOS', 'bise'));
  writeFileSync(join(app, 'Contents', 'Resources', 'app.asar'), asarArchive());
  writeFileSync(join(app, 'Contents', 'Resources', 'VERSION'), `id=${version.id}\nbuilt=${version.built}\n`);
  run('/usr/bin/codesign', ['--force', '--sign', '-', join(app, 'Contents', 'MacOS', 'bise')]);
  run('/usr/bin/codesign', ['--force', '--sign', '-', app]);
}

/**
 * The fake running app and a file:// feed offering `build` in `home` (the
 * run's throwaway HOME, made by the harness). Returns the env for the launch.
 */
export function makeUpdateFeed(home: string, build: UpdateBuild = 'good', opts: FeedOpts = {}): UpdateFeed {
  const target = join(home, 'Applications', 'bise.app');
  const feed = join(home, 'feed');
  const stage = join(home, 'feed-stage');
  rmSync(feed, { recursive: true, force: true });
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(feed, { recursive: true });
  mkdirSync(stage, { recursive: true });
  // a case left on its own: no .old of an earlier case (a failed one never reached its confirm)
  rmSync(`${target}.old`, { recursive: true, force: true });
  fakeApp(target, RUNNING);
  const next = join(stage, 'bise.app');
  if (build === 'other') {
    // signed fine, but another build: its cdhash isn't the running app's
    fakeApp(next, { id: OFFERED.id, built: OFFERED.built });
  } else {
    cpSync(target, next, { recursive: true, verbatimSymlinks: true });
    // a sealed resource changed after signing: --verify --strict fails
    if (build === 'tampered') writeFileSync(join(next, 'Contents', 'Resources', 'VERSION'), 'id=evil\n');
  }
  const zipName = 'bise-mac-arm64.zip';
  const zip = join(feed, zipName);
  run('/usr/bin/ditto', ['-c', '-k', '--keepParent', next, zip]);
  rmSync(stage, { recursive: true, force: true });
  const sha256 = createHash('sha256').update(readFileSync(zip)).digest('hex');
  // desktop.json, dist-ci's shape (m_25179): the reader's fields, and the extra ones it ignores
  const entry = { id: OFFERED.id, version: OFFERED.version, built: OFFERED.built, url: zipName, sha256, size: 1, min_macos: '13.0' };
  const manifest = (channel: 'stable' | 'beta', paused: boolean): string =>
    `${JSON.stringify({ kind: 'desktop', channel, version: OFFERED.version, runtime_id: OFFERED.id, paused, targets: { 'darwin-arm64': entry, 'darwin-x86_64': entry } }, null, 2)}\n`;
  // stable offers it unless the case is beta-only (then stable is an older build) or paused
  const beta = join(feed, 'beta');
  mkdirSync(beta, { recursive: true });
  cpSync(zip, join(beta, zipName));
  writeFileSync(join(beta, 'desktop.json'), manifest('beta', false));
  if (opts.channel === 'beta') {
    const older = { ...entry, id: RUNNING.id, built: RUNNING.built };
    writeFileSync(join(feed, 'desktop.json'), `${JSON.stringify({ kind: 'desktop', channel: 'stable', version: 'old', targets: { 'darwin-arm64': older, 'darwin-x86_64': older } })}\n`);
  } else {
    writeFileSync(join(feed, 'desktop.json'), manifest('stable', opts.paused === true));
  }
  const feedUrl = `file://${feed}`;
  return {
    target,
    feedUrl,
    version: OFFERED.version,
    env: { BISE_DESKTOP_UPDATE_TARGET: target, BISE_DIST_URL_DESKTOP: feedUrl, BISE_DIST_URL_DESKTOP_BETA: `file://${beta}` },
  };
}
