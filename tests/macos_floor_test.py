"""One oldest macOS for the app, everywhere it is written (gh-issues via main m_59053).

The app's Info.plist said 13.0 (Electron 44's own floor, electron-builder's default) while the
update feed said 14.0: a Mac on 13 got an app that opened, then a bise built for 14 that couldn't
run. The floor is what the binaries are built for (rust/.cargo/config.toml), and each place that
says it must say the same:

  - rust/.cargo/config.toml           MACOSX_DEPLOYMENT_TARGET (bise, the runtime: bins.sh)
  - apps/desktop/native/mac/build.sh  the helper's default target (ScreenCaptureKit)
  - apps/desktop/electron-builder.mjs mac.minimumSystemVersion (Info.plist LSMinimumSystemVersion)
  - scripts/desktop_feed.py           MIN_MACOS (the update feed's min_macos)
  - scripts/desktop_release.py        MACOS_TARGET (the release build's env)

Run: python3 tests/macos_floor_test.py
"""
import os
import re
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def found(path, pattern):
    with open(os.path.join(ROOT, path)) as f:
        m = re.search(pattern, f.read(), re.M)
    return m.group(1) if m else None


PLACES = {
    "rust/.cargo/config.toml": r'^MACOSX_DEPLOYMENT_TARGET\s*=\s*\{\s*value\s*=\s*"([0-9.]+)"',
    "apps/desktop/native/mac/build.sh": r'^TARGET="\$\{MACOSX_DEPLOYMENT_TARGET:-([0-9.]+)\}"',
    "apps/desktop/electron-builder.mjs": r"^\s*minimumSystemVersion:\s*'([0-9.]+)'",
    "scripts/desktop_feed.py": r'^MIN_MACOS\s*=\s*"([0-9.]+)"',
    "scripts/desktop_release.py": r'^MACOS_TARGET\s*=\s*"([0-9.]+)"',
}


def main():
    seen = {p: found(p, rx) for p, rx in PLACES.items()}
    missing = [p for p, v in seen.items() if v is None]
    if missing:
        print("macos_floor: no macOS floor found in %s" % ", ".join(missing))
        return 1
    floor = seen["rust/.cargo/config.toml"]
    off = {p: v for p, v in seen.items() if v != floor}
    if off:
        print("macos_floor: the binaries need macOS %s, but %s" % (floor, "; ".join("%s says %s" % (p, v) for p, v in off.items())))
        return 1
    print("macos_floor: ok, %s in all %d places" % (floor, len(seen)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
