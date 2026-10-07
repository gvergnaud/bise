#!/usr/bin/env python3
"""check-release.py: is this release what install.sh and `bise update` read? (BISE-220)

  check-release.py <dir> --url <channel> [--version <v>] [--commit <sha>]
      [--targets "darwin-arm64 darwin-x86_64"] [--assets <name>...]

<dir> is a release laid out by make-release.sh (CI's, or publish-release.sh
--local's). Checked, from what packaging/install.sh and bise_home::release
read:
  - latest.json: version (--version), id, commit (--commit), built,
    published; targets: exactly --targets (default: the ones present), each
    {url, file, sha256, size, macos, id, commit, built} with
    file = url = bise-<id>-<target>.tar.gz (relative to the channel) and the
    top id/commit;
  - each tarball in <dir>: its size and sha256, and its .sha256 file
    ("<sha256>  <file>");
  - install.sh: packaging/install.sh with DIST_URL_DEFAULT='<channel>';
  - the release's files (--assets, default: the files of <dir>) are exactly
    install.sh, latest.json, the tarballs and their .sha256.
Exit 1 with one line per problem.
"""
import argparse, hashlib, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ENTRY = ["url", "file", "sha256", "size", "macos", "id", "commit", "built"]
TOP = ["version", "id", "commit", "built", "published", "targets"]


def main():
    ap = argparse.ArgumentParser(description="check a bise release folder")
    ap.add_argument("dir")
    ap.add_argument("--url", required=True)
    ap.add_argument("--version")
    ap.add_argument("--commit")
    ap.add_argument("--targets")
    ap.add_argument("--assets", nargs="*")
    a = ap.parse_args()
    bad = []
    d = a.dir
    try:
        m = json.load(open(os.path.join(d, "latest.json")))
    except Exception as e:  # noqa: BLE001 - any read/parse error is the answer
        print(f"check-release: latest.json: {e}")
        return 1
    for k in TOP:
        if k not in m:
            bad.append(f"latest.json has no {k}")
    if a.version is not None and m.get("version") != a.version:
        bad.append(f"latest.json version {m.get('version')!r}, not {a.version!r}")
    if a.commit and m.get("commit") != a.commit:
        bad.append(f"latest.json commit {m.get('commit')!r}, not {a.commit!r}")
    targets = m.get("targets") if isinstance(m.get("targets"), dict) else {}
    want = a.targets.split() if a.targets else sorted(targets)
    if sorted(targets) != sorted(want) or not want:
        bad.append(f"latest.json targets {sorted(targets)}, not {sorted(want)}")
    files = {"install.sh", "latest.json"}
    for t, e in sorted(targets.items()):
        for k in ENTRY:
            if k not in e:
                bad.append(f"{t}: no {k}")
        name = f"bise-{m.get('id')}-{t}.tar.gz"
        if e.get("file") != name or e.get("url") != name:
            bad.append(f"{t}: file/url {e.get('file')!r}/{e.get('url')!r}, not {name!r}")
        for k in ("id", "commit"):
            if e.get(k) != m.get(k):
                bad.append(f"{t}: {k} {e.get(k)!r}, not the release's {m.get(k)!r}")
        sha = str(e.get("sha256", ""))
        if not re.fullmatch(r"[0-9a-f]{64}", sha):
            bad.append(f"{t}: bad sha256 {sha!r}")
        files |= {name, name + ".sha256"}
        p = os.path.join(d, name)
        if os.path.isfile(p):
            h = hashlib.sha256(open(p, "rb").read()).hexdigest()
            if h != sha:
                bad.append(f"{name}: sha256 {h}, latest.json says {sha}")
            if os.path.getsize(p) != e.get("size"):
                bad.append(f"{name}: {os.path.getsize(p)} bytes, latest.json says {e.get('size')}")
            try:
                s = open(p + ".sha256").read()
            except OSError:
                s = None
            if s != f"{h}  {name}\n":
                bad.append(f"{name}.sha256 is {s!r}, not '{h}  {name}'")
    # nix-sources.json (the flake's nix/sources.json once published): the
    # same version, id and built, and each linux target's file and sha256
    files.add("nix-sources.json")
    try:
        n = json.load(open(os.path.join(d, "nix-sources.json")))
        for k in ("version", "id", "built"):
            if n.get(k) != m.get(k):
                bad.append(f"nix-sources.json {k} {n.get(k)!r}, not latest.json's {m.get(k)!r}")
        nix = {"linux-x86_64": "x86_64-linux", "linux-arm64": "aarch64-linux"}
        want_sys = {nix[t] for t in targets if t in nix}
        got_sys = {k for k in n if k not in ("version", "id", "built")}
        if got_sys != want_sys:
            bad.append(f"nix-sources.json systems {sorted(got_sys)}, not {sorted(want_sys)}")
        for t, s in nix.items():
            e, ns = targets.get(t), n.get(s)
            if e and isinstance(ns, dict):
                if not str(ns.get("url", "")).endswith("/" + str(e.get("file"))):
                    bad.append(f"nix-sources.json {s}: url {ns.get('url')!r} is not {e.get('file')!r}")
                if ns.get("sha256") != e.get("sha256"):
                    bad.append(f"nix-sources.json {s}: sha256 {ns.get('sha256')!r}, latest.json says {e.get('sha256')!r}")
    except Exception as e:  # noqa: BLE001 - any read/parse error is the answer
        bad.append(f"nix-sources.json: {e}")
    try:
        got = open(os.path.join(d, "install.sh")).read()
        src = open(os.path.join(HERE, "install.sh")).read()
        stamped = src.replace("\nDIST_URL_DEFAULT=''\n", f"\nDIST_URL_DEFAULT='{a.url}'\n", 1)
        if stamped == src or got != stamped:
            bad.append(f"install.sh is not packaging/install.sh stamped with {a.url}")
    except OSError as e:
        bad.append(f"install.sh: {e}")
    assets = set(a.assets) if a.assets is not None else set(os.listdir(d))
    if assets != files:
        extra, missing = sorted(assets - files), sorted(files - assets)
        bad.append(f"the release's files: extra {extra}, missing {missing}")
    for b in bad:
        print(f"check-release: {b}")
    if not bad:
        print(f"check-release: ok, {m.get('version')} ({m.get('id')}): {' '.join(sorted(targets))}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
