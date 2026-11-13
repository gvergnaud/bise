#!/bin/bash
# desktop-try.sh: the user's first try of bise desktop on a bise home of its own
# (~/.bise/try/desktop), his repos as throwaway clones, his model login read where it is.
#
#   scripts/desktop-try.sh --real   (the promoted bundle on his own ~/.bise and hubs)
#   scripts/desktop-try.sh --try [repo…] [--fresh] [--replace]
#   scripts/desktop-try.sh --try --stop
#   scripts/desktop-try.sh --candidate <sha>   (a gate candidate in its own dir)
#   scripts/desktop-try.sh --candidate <sha> --team-sign   (+ a copy signed by his team, for a Finder check)
#   scripts/desktop-try.sh --promote <sha>     (the gate passed: it becomes his try)
#
# ~/.local/bin/bise-ambient (scripts/bise-ambient) runs the try checkout's copy for
# 'bise-ambient --try|--real …'. Everything is in
# scripts/desktop_try.py (its header says what is isolated and why).
exec /usr/bin/env python3 -B "$(cd "$(dirname "$0")" && pwd)/desktop_try.py" "$@"
