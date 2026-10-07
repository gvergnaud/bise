#!/usr/bin/env bash
# make-release.sh — lay out a release channel (BISE-170/171): the folder
# a host serves as is, and what `curl -fsSL <url>/install.sh | sh` and
# `bise update` read.
#
#   make-release.sh --out <dir> [--url <base url>] [--version <name>]
#       [--whats-new <file>] <tarball>...
#
#   <tarball>   build-dist.sh archives, one per target (darwin-arm64,
#               darwin-x86_64), all of the same version
#   --url       the channel's public base URL, stamped into install.sh
#               (DIST_URL_DEFAULT); default file://<out> (local tests)
#   --version   the release's name (a tag without v); default: the id
#   --whats-new what's new, for the users: 3-5 plain lines (blank lines
#               and # comments dropped), latest.json's "notes"; an
#               installed bise shows them in its new-release item
#   --nix-url   where the tarballs stay for good (a tag's download URL),
#               for nix-sources.json; default --url
#
# Output in <out> (existing tarballs of other versions are kept, so an
# update can still find its version; latest.json names only these):
#   install.sh                    packaging/install.sh, channel stamped
#   latest.json                   {version, id, commit, built, published,
#                                  notes?: [line...],
#                                  targets: {<os-arch>: {url, file, sha256,
#                                  size, macos, id, commit, built}}}
#   bise-<id>-<os-arch>.tar.gz    + .sha256
#   nix-sources.json              {version, id, built, <nix system>: {url,
#                                  sha256}} for the linux targets: what the
#                                  flake's nix/sources.json becomes once
#                                  the release is published
# The tarball URLs are relative to the channel (bise_home::release
# resolves them), so the same folder works at any URL. Nothing is
# uploaded: publishing is copying <out> to the host.

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
out="" url="" version="" whats_new="" nix_url=""
tarballs=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift ;;
    --url) url="$2"; shift ;;
    --version) version="$2"; shift ;;
    --whats-new) whats_new="$2"; shift ;;
    --nix-url) nix_url="$2"; shift ;;
    -h|--help) sed -n '2,35p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "make-release: unknown argument $1" >&2; exit 2 ;;
    *) tarballs+=("$1") ;;
  esac
  shift
done
[ -n "$out" ] && [ ${#tarballs[@]} -gt 0 ] || { echo "usage: make-release.sh --out <dir> [--url <url>] [--version <name>] <tarball>..." >&2; exit 2; }
mkdir -p "$out"
out="$(cd "$out" && pwd)"
url="${url:-file://$out}"
url="${url%/}"

json_str() { printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g')"; }

targets="" id="" commit="" built=""
for t in "${tarballs[@]}"; do
  f="$(basename "$t")"
  name="${f%.tar.gz}"
  v="$(tar -xzOf "$t" "$name/app/VERSION")" || { echo "make-release: $f has no $name/app/VERSION" >&2; exit 1; }
  get() { printf '%s\n' "$v" | sed -n "s/^$1=//p" | head -n 1; }
  target="$(get target)"
  [ -n "$target" ] || { echo "make-release: $f: VERSION has no target=" >&2; exit 1; }
  [ -z "$id" ] || [ "$id" = "$(get id)" ] || { echo "make-release: $f is $(get id), not $id" >&2; exit 1; }
  id="$(get id)"; commit="$(get commit)"; built="$(get built)"
  [ "$(cd "$(dirname "$t")" && pwd)" = "$out" ] || cp "$t" "$out/$f"
  sum="$(shasum -a 256 "$out/$f" | cut -d' ' -f1)"
  (cd "$out" && printf '%s  %s\n' "$sum" "$f" > "$f.sha256")
  # wc, not stat: GNU stat -f is another command (CI assembles on Linux)
  size="$(wc -c < "$out/$f" | tr -d ' ')"
  entry="$(json_str "$target"): {\"url\": $(json_str "$f"), \"file\": $(json_str "$f"), \"sha256\": $(json_str "$sum"), \"size\": $size, \"macos\": $(json_str "$(get macos)"), \"id\": $(json_str "$id"), \"commit\": $(json_str "$commit"), \"built\": $(json_str "$built")}"
  targets="${targets:+$targets,
    }$entry"
done
version="${version:-$id}"
notes=""
if [ -n "$whats_new" ]; then
  [ -f "$whats_new" ] || { echo "make-release: no file $whats_new" >&2; exit 1; }
  while IFS= read -r l || [ -n "$l" ]; do
    l="$(printf '%s' "$l" | sed 's/^[[:space:]]*//; s/^[-*•][[:space:]]*//; s/[[:space:]]*$//')"
    case "$l" in ""|"#"*) continue ;; esac
    notes="${notes:+$notes, }$(json_str "$l")"
  done < "$whats_new"
  [ -z "$notes" ] || notes="
  \"notes\": [$notes],"
fi
cat > "$out/latest.json.tmp" <<EOF
{
  "version": $(json_str "$version"),
  "id": $(json_str "$id"),
  "commit": $(json_str "$commit"),
  "built": $(json_str "$built"),
  "published": $(json_str "$(date -u +%Y-%m-%dT%H:%M:%SZ)"),$notes
  "targets": {
    $targets
  }
}
EOF
plutil -lint -s "$out/latest.json.tmp" 2>/dev/null || python3 -m json.tool "$out/latest.json.tmp" >/dev/null
mv "$out/latest.json.tmp" "$out/latest.json"
# nix-sources.json: the same release for the Nix flake (nix/package.nix),
# from latest.json, never by hand; its URLs must not move (--nix-url: the
# tag's download URL), the systems are Nix's names
python3 - "$out/latest.json" "${nix_url:-$url}" > "$out/nix-sources.json.tmp" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
base = sys.argv[2].rstrip("/")
nix = {"linux-x86_64": "x86_64-linux", "linux-arm64": "aarch64-linux"}
out = {"version": m["version"], "id": m["id"], "built": m["built"]}
for t, e in sorted(m["targets"].items()):
    if t in nix:
        out[nix[t]] = {"url": f"{base}/{e['file']}", "sha256": e["sha256"]}
print(json.dumps(out, indent=2))
PY
mv "$out/nix-sources.json.tmp" "$out/nix-sources.json"
# the installer, with this channel as its default
q="$(printf '%s' "$url" | sed "s/'/'\\\\''/g; s/[&|]/\\\\&/g")"
sed "s|^DIST_URL_DEFAULT=''|DIST_URL_DEFAULT='$q'|" "$HERE/install.sh" > "$out/install.sh.tmp"
grep -q "^DIST_URL_DEFAULT='$q'" "$out/install.sh.tmp" || { echo "make-release: cannot stamp the URL into install.sh" >&2; exit 1; }
chmod 644 "$out/install.sh.tmp"
mv "$out/install.sh.tmp" "$out/install.sh"
echo "release $version ($id) in $out: curl -fsSL $url/install.sh | sh" >&2
echo "$out"
