#!/usr/bin/env bash
# ci-release.sh — the release job of .github/workflows/release.yml
# (BISE-220): CI is the one publisher of a release. From the archives of
# the build jobs, lay out the release with make-release.sh (the same
# files and latest.json as every channel), check it (check-release.py)
# and put it in a DRAFT GitHub Release of the tag. The user looks at the
# draft, then publishes it (publish-release.sh <tag> --publish, or
# gh release edit <tag> --draft=false): a draft is not the latest
# release, so installs and `bise update` see nothing until then.
#
#   ci-release.sh <tag> <dist dir> <out dir> [--repo <owner/repo>]
#       [--url <channel>] [--commit <sha>] [--targets "<t> <t>"] [--draft]
#
#   <dist dir>  build-dist.sh archives (+ their .sha256), both arches
#   --repo      default $GITHUB_REPOSITORY, else gvergnaud/bise
#   --url       the channel stamped into install.sh; default
#               https://github.com/<repo>/releases/latest/download
#   --commit    the commit the archives must name (CI: $GITHUB_SHA)
#   --targets   default "darwin-arm64 darwin-x86_64 linux-x86_64 linux-arm64"
#   --draft     upload: create the draft (gh), or replace the files of
#               the tag's draft (a rerun); a PUBLISHED release is never
#               touched (the run fails). Without it: lay out and check.

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
repo="${GITHUB_REPOSITORY:-gvergnaud/bise}" url="" commit="" draft=0
targets="darwin-arm64 darwin-x86_64 linux-x86_64 linux-arm64"
pos=()
while [ $# -gt 0 ]; do
  case "$1" in
    --repo) repo="$2"; shift ;;
    --url) url="$2"; shift ;;
    --commit) commit="$2"; shift ;;
    --targets) targets="$2"; shift ;;
    --draft) draft=1 ;;
    -h|--help) sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "ci-release: unknown argument $1" >&2; exit 2 ;;
    *) pos+=("$1") ;;
  esac
  shift
done
[ ${#pos[@]} = 3 ] || { echo "usage: ci-release.sh <tag> <dist dir> <out dir> [--draft] ..." >&2; exit 2; }
tag="${pos[0]}" dist="${pos[1]}" out="${pos[2]}"
say() { echo "ci-release: $*" >&2; }
die() { say "error: $*"; exit 1; }
case "$tag" in v?*) ;; *) die "the tag is v<version>, not $tag" ;; esac
version="${tag#v}"
url="${url:-https://github.com/$repo/releases/latest/download}"

# the archives as the build jobs made them
shopt -s nullglob
tarballs=("$dist"/*.tar.gz)
[ ${#tarballs[@]} -gt 0 ] || die "no archive in $dist"
for t in "${tarballs[@]}"; do
  [ -f "$t.sha256" ] || die "$(basename "$t") has no .sha256"
  (cd "$(dirname "$t")" && shasum -a 256 -c "$(basename "$t").sha256" >&2) || die "$(basename "$t"): checksum mismatch"
done

rm -rf "$out"
"$HERE/make-release.sh" --out "$out" --url "$url" --nix-url "https://github.com/$repo/releases/download/$tag" --version "$version" "${tarballs[@]}" >/dev/null
"$HERE/check-release.py" "$out" --url "$url" --version "$version" --targets "$targets" \
  ${commit:+--commit "$commit"} >&2
cat "$out/latest.json" >&2
[ "$draft" = 1 ] || { echo "$out"; exit 0; }

rel_commit="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["commit"])' "$out/latest.json")"
notes="bise $version (commit ${rel_commit:0:12}), macOS 14+ and Linux (glibc 2.34+; NixOS: the flake): ${targets}. Built by CI (ad-hoc signed).
Install: curl -fsSL https://bise.dev/install | sh   (private repo: gh auth login first).
Update: bise update, or /restart latest in Switchboard."
if state="$(gh release view "$tag" -R "$repo" --json isDraft --jq .isDraft 2>/dev/null)"; then
  [ "$state" = true ] || die "$repo has a published release $tag: CI never rewrites one (make a new tag)"
  say "the draft $tag exists (a rerun): replacing its files"
  gh release upload "$tag" "$out"/* --clobber -R "$repo"
  for a in $(gh release view "$tag" -R "$repo" --json assets --jq '.assets[].name'); do
    [ -e "$out/$a" ] || gh release delete-asset "$tag" "$a" -y -R "$repo"
  done
else
  gh release create "$tag" "$out"/* --draft --verify-tag --title "bise $version" --notes "$notes" -R "$repo"
fi
# the draft holds exactly these files
# shellcheck disable=SC2046
"$HERE/check-release.py" "$out" --url "$url" --version "$version" --targets "$targets" \
  --assets $(gh release view "$tag" -R "$repo" --json assets --jq '.assets[].name') >&2
say "draft $tag ready: gh release view $tag -R $repo; publish: gh release edit $tag -R $repo --draft=false"
echo "$out"
