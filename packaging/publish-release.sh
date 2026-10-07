#!/usr/bin/env bash
# publish-release.sh — make a release of bise on GitHub Releases of
# gvergnaud/bise (BISE-217, BISE-220). CI is the one publisher: this
# script tags, CI builds, the user publishes.
#
#   packaging/publish-release.sh [<tag>] [--rev <rev>] [--whats-new <file>]
#       [--publish] [--repo <owner/repo>] [--remote <git remote>] [--dry-run]
#   packaging/publish-release.sh <tag> --whats-new <file>
#       (a draft or a published release: its notes only)
#   packaging/publish-release.sh --local [<tag>]
#       [--rev <rev>] [--add <tarball>]... [--from <tarball>]... [--draft]
#       [--notes <text>] [--whats-new <file>] [--dry-run]
#
#   <tag>      vX.Y.Z (default v<YYYY.M.D>, then -2, -3... when taken)
#   --rev      the commit to release (default HEAD); it must be on GitHub
#              already (git push first)
#   --publish  make the draft the latest release (every install and
#              `bise update` read it next); with a tag already built, only
#              that: publish-release.sh <tag> --publish
#   --whats-new  what's new for the users, 3-5 plain lines in a file
#              (blank lines and # comments dropped): written into the
#              release's latest.json ("notes") and on top of its GitHub
#              notes. Every installed bise reads latest.json within the
#              hour and shows these lines in its new-release item
#              (update-card), so write them for users: what they can do
#              now, no commit hashes. Works on the draft before
#              --publish, and on a published release (fixes its notes).
#   --dry-run  say what would be done, change nothing
#   --next-tag print the tag a release would take now (default above),
#              change nothing (/release-bise's preview, BISE-235)
#   --url      the channel (default below; tests use a stand-in)
#
# Steps: tag <rev> and push the tag -> .github/workflows/release.yml
# builds darwin arm64 + x86_64, installs each in a clean HOME
# (test-install.sh) and makes a DRAFT release (ci-release.sh:
# make-release.sh + check-release.py) -> this script watches the run
# (gh run watch), shows the draft and checks its latest.json and
# install.sh -> --publish (or gh release edit <tag> --draft=false). A
# draft is not the latest release: nobody sees it before that. Run again
# with the same tag to resume (a pushed tag is not pushed again).
# Private repo: macOS minutes cost 10x (a cold run is long: the V8 engine).
#
# --local: the emergency path, CI down: build-dist.sh here (this Mac's
# arch only), make-release.sh, check-release.py, gh release create <tag>
# --target <commit> (published, or --draft). --add <tarball>: another
# archive of the same commit (gh run download <run> -R gvergnaud/bise -n
# dist-darwin-x86_64 -D /tmp/x86); --from: publish these archives, build
# nothing. The tag it makes starts release.yml too: CI refuses to touch a
# published release, and replaces a draft's files with both arches.
#
# The channel is the LATEST release's download URL:
#   https://github.com/<repo>/releases/latest/download
# Private repo: installs and updates read it through the reader's `gh`
# login (or GH_TOKEN); public: plain curl. bise.dev/install serves a copy
# of the stamped install.sh (site/install.sh): this script says when that
# copy is stale.

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$HERE" && git rev-parse --show-toplevel)"
SITE_INSTALL="$REPO_DIR/site/install.sh"
repo=gvergnaud/bise channel="" tag="" rev=HEAD notes="" whats_new="" draft=0 dry=0 local=0 publish=0 remote="" next=0
adds=() froms=()
while [ $# -gt 0 ]; do
  case "$1" in
    --rev) rev="$2"; shift ;;
    --add) adds+=("$2"); shift ;;
    --from) froms+=("$2"); shift ;;
    --repo) repo="$2"; shift ;;
    --remote) remote="$2"; shift ;;
    --url) channel="$2"; shift ;;
    --notes) notes="$2"; shift ;;
    --whats-new) whats_new="$2"; shift ;;
    --draft) draft=1 ;;
    --local) local=1 ;;
    --publish) publish=1 ;;
    --dry-run) dry=1 ;;
    --next-tag) next=1 ;;
    -h|--help) sed -n '2,57p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "publish-release: unknown argument $1" >&2; exit 2 ;;
    *) tag="$1" ;;
  esac
  shift
done
say() { echo "publish-release: $*" >&2; }
die() { say "error: $*"; exit 1; }
if [ -n "$whats_new" ]; then
  [ -f "$whats_new" ] || die "no file $whats_new (--whats-new)"
  whats_new="$(cd "$(dirname "$whats_new")" && pwd)/$(basename "$whats_new")"
  n="$(grep -cvE '^[[:space:]]*(#|$)' "$whats_new" || true)"
  [ "$n" -ge 1 ] || die "$whats_new has no line (--whats-new: 3-5 plain lines)"
  [ "$n" -le 5 ] || say "$whats_new has $n lines: the new-release item shows the first 5"
fi
# --whats-new on the release <tag> on GitHub: its latest.json gets the
# lines ("notes", re-uploaded), its GitHub notes get them on top
put_whats_new() {  # <tag>
  local d="$work/whats-new"
  rm -rf "$d"; mkdir -p "$d"
  gh release download "$1" -R "$repo" -p latest.json -D "$d" || die "cannot download $1's latest.json"
  python3 - "$d/latest.json" "$whats_new" <<'PY' || die "cannot write the notes into latest.json"
import json, sys
m = json.load(open(sys.argv[1]))
lines = [l.strip().lstrip("-*•").strip() for l in open(sys.argv[2]) if l.strip() and not l.strip().startswith("#")]
m["notes"] = lines
json.dump(m, open(sys.argv[1], "w"), indent=2)
PY
  if [ "$dry" = 1 ]; then
    say "dry run: would upload $1's latest.json with these notes:"; cat "$d/latest.json" >&2
    return 0
  fi
  gh release upload "$1" "$d/latest.json" --clobber -R "$repo" >/dev/null || die "cannot upload $1's latest.json"
  local body
  body="$(gh release view "$1" -R "$repo" --json body --jq .body)"
  case "$body" in
    "What's new:"*) body="${body#*$'\n\n'}" ;;
  esac
  gh release edit "$1" -R "$repo" --notes "What's new:
$(grep -vE '^[[:space:]]*(#|$)' "$whats_new" | sed 's/^[[:space:]]*//; s/^[-*•][[:space:]]*//; s/^/- /')

$body" >/dev/null || die "cannot edit $1's notes"
  say "$1: what's new written (latest.json and the GitHub notes)"
}
channel="${channel:-https://github.com/$repo/releases/latest/download}"
if [ "$local" = 0 ] && { [ ${#adds[@]} -gt 0 ] || [ ${#froms[@]} -gt 0 ] || [ "$draft" = 1 ]; }; then
  die "--add, --from and --draft go with --local (CI makes a draft already)"
fi

command -v gh >/dev/null 2>&1 || die "the GitHub CLI is needed: brew install gh, then gh auth login"
gh auth status >/dev/null 2>&1 || die "gh is not logged in: run gh auth login"
gh repo view "$repo" --json name >/dev/null 2>&1 || die "gh cannot read $repo (another account? gh auth status)"

# is the draft's install.sh the one bise.dev/install serves?
site_check() {
  cmp -s "$1" "$SITE_INSTALL" \
    || say "bise.dev/install is stale: cp '$1' '$SITE_INSTALL', commit, and ask designer to deploy the site"
}
# the release's state: none, draft or published
state() {
  local d
  d="$(gh release view "$1" -R "$repo" --json isDraft --jq .isDraft 2>/dev/null)" || { echo none; return; }
  [ "$d" = true ] && echo draft || echo published
}

# the sha of tag <tag> on GitHub, empty when GitHub has no such tag: on
# a 404, gh api exits 1 but prints GitHub's error body on stdout
# ({"message":"Not Found",...}), so only a successful call's hex counts
remote_tag() {  # <tag>
  local s
  s="$(gh api "repos/$repo/git/ref/tags/$1" --jq .object.sha 2>/dev/null)" || return 0
  case "$s" in *[!0-9a-f]* | "") ;; *) echo "$s" ;; esac
}

# the tag: given, else today's (the first one free)
if [ -z "$tag" ]; then
  [ "$publish" = 0 ] || [ "$local" = 1 ] || [ "$next" = 1 ] || die "--publish needs the tag"
  base="v$(date -u +%Y.%-m.%-d)" tag="$base" n=1
  while gh release view "$tag" -R "$repo" >/dev/null 2>&1 \
     || [ -n "$(remote_tag "$tag")" ]; do
    n=$((n + 1)); tag="$base-$n"
  done
fi
case "$tag" in v?*) ;; *) die "a release tag is v<version> (release.yml runs on v* tags), not $tag" ;; esac
[ "$next" = 0 ] || { echo "$tag"; exit 0; }
version="${tag#v}"
work="${TMPDIR:-/tmp}/bise-publish-$tag"

# ---------------------------------------------------------------- --local
if [ "$local" = 1 ]; then
  [ "$(state "$tag")" = none ] || die "$repo has a release $tag already (gh release delete $tag -R $repo --cleanup-tag, or another tag)"
  rm -rf "$work"; mkdir -p "$work/build" "$work/release"
  tarballs=()
  if [ ${#froms[@]} -gt 0 ]; then
    tarballs=("${froms[@]}")
  else
    commit="$(cd "$REPO_DIR" && git rev-parse --verify "$rev^{commit}")" || die "no commit $rev"
    gh api "repos/$repo/commits/$commit" --jq .sha >/dev/null 2>&1 \
      || die "$commit is not on GitHub: git push origin main (or pass --rev a pushed commit)"
    say "building $commit (this Mac: $(uname -m); a few minutes the first time)"
    tarballs+=("$(BISE_CHANNEL=stable "$HERE/build-dist.sh" "$commit" --out "$work/build")")
  fi
  [ ${#adds[@]} -eq 0 ] || tarballs+=("${adds[@]}")
  for t in "${tarballs[@]}"; do [ -f "$t" ] || die "no archive $t"; done
  "$HERE/make-release.sh" --out "$work/release" --url "$channel" --version "$version" ${whats_new:+--whats-new "$whats_new"} "${tarballs[@]}" >/dev/null
  "$HERE/check-release.py" "$work/release" --url "$channel" --version "$version" >&2 || die "the release is not what install.sh and bise update read"
  commit="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["commit"])' "$work/release/latest.json")"
  gh api "repos/$repo/commits/$commit" --jq .sha >/dev/null 2>&1 \
    || die "$commit (the archives' commit) is not on GitHub: push it first"
  targets="$(python3 -c 'import json,sys; print(" ".join(sorted(json.load(open(sys.argv[1]))["targets"])))' "$work/release/latest.json")"
  say "release $tag: $targets (commit ${commit:0:12}) in $work/release"
  ls -la "$work/release" >&2
  if [ -n "$whats_new" ] && [ -z "$notes" ]; then
    notes="What's new:
$(grep -vE '^[[:space:]]*(#|$)' "$whats_new" | sed 's/^[[:space:]]*//; s/^[-*•][[:space:]]*//; s/^/- /')

"
  fi
  notes="${notes}bise $version (commit ${commit:0:12}), macOS 14+: $targets.
Install: curl -fsSL https://bise.dev/install | sh   (private repo: gh auth login first).
Update: /update in bise, or bise update."
  args=(release create "$tag" "$work/release"/* -R "$repo" --target "$commit" --title "bise $version" --notes "$notes")
  [ "$draft" = 0 ] || args+=(--draft)
  if [ "$dry" = 1 ]; then
    say "dry run, nothing published; the command:"
    printf ' %q' gh "${args[@]}" >&2; echo >&2
    exit 0
  fi
  gh "${args[@]}"
  say "published: https://github.com/$repo/releases/tag/$tag"
  [ "$draft" = 1 ] || say "the latest release now: every install reads $channel/latest.json"
  site_check "$work/release/install.sh"
  exit 0
fi

# ---------------------------------------------------------------- CI path
st="$(state "$tag")"
if [ "$st" = published ]; then
  [ -z "$whats_new" ] || put_whats_new "$tag"
  say "$tag is published already: $channel"
  exit 0
fi

# 1. the tag, on a pushed commit (skipped when GitHub has it)
remote_sha="$(remote_tag "$tag")"
if [ -z "$remote_sha" ]; then
  commit="$(cd "$REPO_DIR" && git rev-parse --verify "$rev^{commit}")" || die "no commit $rev"
  gh api "repos/$repo/commits/$commit" --jq .sha >/dev/null 2>&1 \
    || die "$commit is not on GitHub: git push origin main (or pass --rev a pushed commit)"
  if [ -z "$remote" ]; then
    for r in $(git -C "$REPO_DIR" remote); do
      case "$(git -C "$REPO_DIR" remote get-url "$r")" in
        *github.com[:/]"$repo"|*github.com[:/]"$repo".git) remote="$r"; break ;;
      esac
    done
    [ -n "$remote" ] || die "no git remote points to github.com/$repo (--remote <name>)"
  fi
  local_sha="$(git -C "$REPO_DIR" rev-parse -q --verify "refs/tags/$tag^{commit}" || true)"
  [ -z "$local_sha" ] || [ "$local_sha" = "$commit" ] || die "the local tag $tag is on ${local_sha:0:12}, not ${commit:0:12}"
  if [ "$dry" = 1 ]; then
    say "dry run: would tag ${commit:0:12} $tag, git push $remote $tag, watch release.yml, show the draft"
    exit 0
  fi
  [ -n "$local_sha" ] || git -C "$REPO_DIR" tag -a "$tag" -m "bise $version" "$commit"
  git -C "$REPO_DIR" push "$remote" "refs/tags/$tag"
  say "pushed $tag (${commit:0:12}): release.yml builds both arches (~15-60 min; macOS minutes cost 10x on a private repo)"
elif [ "$dry" = 1 ]; then
  say "dry run: $tag is on GitHub (${remote_sha:0:12}), release $st: would watch release.yml, show the draft$([ "$publish" = 1 ] && echo ", publish it")"
  exit 0
fi

# 2. the run of release.yml for this tag: watch it
if [ "$st" = none ]; then
  run=""
  for _ in $(seq 60); do
    run="$(gh run list -R "$repo" --workflow release.yml --branch "$tag" --limit 1 --json databaseId --jq '.[0].databaseId // empty' 2>/dev/null || true)"
    [ -n "$run" ] && break
    sleep 2
  done
  [ -n "$run" ] || die "no release.yml run for $tag after 2 min: gh run list -R $repo --workflow release.yml (Actions off?)"
  say "watching run $run: gh run view $run -R $repo --web"
  gh run watch "$run" -R "$repo" --exit-status --interval 30 >&2 \
    || die "the run failed: gh run view $run -R $repo --log-failed (fix, then a new tag; or --local)"
  st="$(state "$tag")"
  [ "$st" != none ] || die "the run passed but $repo has no release $tag: gh run view $run -R $repo"
fi

# 3. the draft: show it, check its latest.json and install.sh
say "the draft:"
gh release view "$tag" -R "$repo" >&2
rm -rf "$work"; mkdir -p "$work"
[ -z "$whats_new" ] || put_whats_new "$tag"
gh release download "$tag" -R "$repo" -p latest.json -p install.sh -p nix-sources.json -D "$work" \
  || die "cannot download the draft's latest.json and install.sh"
# shellcheck disable=SC2046
"$HERE/check-release.py" "$work" --url "$channel" --version "$version" --targets "darwin-arm64 darwin-x86_64 linux-x86_64 linux-arm64" \
  --assets $(gh release view "$tag" -R "$repo" --json assets --jq '.assets[].name') >&2 \
  || die "the draft is not what install.sh and bise update read: fix before publishing"
site_check "$work/install.sh"

# 4. publish
if [ "$publish" = 1 ] && [ "$st" = draft ]; then
  gh release edit "$tag" -R "$repo" --draft=false --latest >/dev/null
  say "published $tag: the latest release, every install reads $channel/latest.json"
  # the flake (nix/sources.json) installs this release from now on: the
  # release's nix-sources.json (make-release.sh), committed here, never by hand
  root="$(git rev-parse --show-toplevel)"
  if [ -f "$work/nix-sources.json" ] && ! cmp -s "$work/nix-sources.json" "$root/nix/sources.json"; then
    mkdir -p "$root/nix" && cp "$work/nix-sources.json" "$root/nix/sources.json"
    git -C "$root" commit -q -m "nix: sources for ${tag#v}" -- nix/sources.json \\n      && say "committed nix/sources.json for ${tag#v}: push main so 'nix profile install github:$repo' gets it"
  fi
else
  say "not published: try it (gh release download $tag -R $repo), then: $0 $tag --publish"
fi
