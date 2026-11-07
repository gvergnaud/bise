#!/bin/sh
# cc-extra.sh — the C compiler bins.sh hands bend (as $CC) when a binary
# needs clang flags bend does not pass (bend runs `$CC -std=c11 -O3 <c>
# ... -o <bin>` and reads no CFLAGS). Runs $BISE_CC (the caller's $CC,
# else clang) with the same arguments and $BISE_CC_EXTRA after them;
# `--version` (bend's probe of the clang) goes through unchanged.
cc="${BISE_CC:-clang}"
case "$1" in --version) exec "$cc" "$@" ;; esac
# shellcheck disable=SC2086 # the flags split into words on purpose
exec "$cc" "$@" $BISE_CC_EXTRA
