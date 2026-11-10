#!/usr/bin/env bash
# Hermetic smoke test for the dx perlcritic shim.
set -u
bin="$PWD/$1"
clean="$2"
dirty="$PWD/$3"
args=(--noprofile --severity=5 --nocolor --quiet '--verbose=%f\t%l\t%p\t%m\n')

"$bin" "${args[@]}" "$clean" || { echo "clean fixture failed: $?" >&2; exit 1; }

out=$("$bin" "${args[@]}" "$dirty")
rc=$?
[ "$rc" -eq 2 ] || { echo "dirty fixture exit $rc, want 2: $out" >&2; exit 1; }
case "$out" in
*TestingAndDebugging::RequireUseStrict*) ;;
*) echo "missing RequireUseStrict: $out" >&2; exit 1 ;;
esac

# --noprofile must ignore a .perlcriticrc in the working directory.
work=$(mktemp -d)
printf '[-TestingAndDebugging::RequireUseStrict]\n' >"$work/.perlcriticrc"
cd "$work" || exit 1
out=$("$bin" "${args[@]}" "$dirty")
rc=$?
[ "$rc" -eq 2 ] || { echo "profile not ignored, exit $rc: $out" >&2; exit 1; }
