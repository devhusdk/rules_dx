#!/usr/bin/env bash
set -u
exec_sh="$1"
tmp="${TEST_TMPDIR:-$(mktemp -d)}"
fail() { echo "FAIL: $1" >&2; exit 1; }

fake="$tmp/fake"
cat >"$fake" <<'FAKE'
#!/usr/bin/env bash
# fake STDERR-TEXT EXIT-CODE [FILE-TO-DUMP]
printf '%s\n' "$1" >&2
[ -n "${3:-}" ] && cat "$3" >&2
exit "$2"
FAKE
chmod +x "$fake"

# The last step's exit code is the status; stderr of every step reaches the log.
"$exec_sh" "$tmp/a.log" "$tmp/a.exit" -- "$fake" one 0 --dx-next-- "$fake" two 3 || fail "helper exits 0"
[ "$(cat "$tmp/a.exit")" = 3 ] || fail "status is the last exit code"
[ "$(cat "$tmp/a.log")" = "$(printf 'one\ntwo')" ] || fail "both steps log"

# A failing step stops the sequence.
"$exec_sh" "$tmp/b.log" "$tmp/b.exit" -- "$fake" one 1 --dx-next-- "$fake" two 0
[ "$(cat "$tmp/b.exit")" = 1 ] || fail "first failure is the status"
[ "$(cat "$tmp/b.log")" = one ] || fail "later steps do not run"

# Aliases replace fixed strings, including slashes, dots and regex characters.
"$exec_sh" "$tmp/c.log" "$tmp/c.exit" --alias 'bazel-out/k8/bin/p/__obazl/M.ml' 'p/m.ml' --alias 'a.b[1]*' 'X' \
  -- "$fake" 'File "bazel-out/k8/bin/p/__obazl/M.ml", line 2, a.b[1]* and aXb' 1
[ "$(cat "$tmp/c.log")" = 'File "p/m.ml", line 2, X and aXb' ] || fail "aliases are fixed strings: $(cat "$tmp/c.log")"
"$exec_sh" "$tmp/c2.log" "$tmp/c2.exit" --alias 'dir with space/x' 'y' -- "$fake" 'in dir with space/x:1' 1
[ "$(cat "$tmp/c2.log")" = 'in y:1' ] || fail "aliases handle spaces"

# Append keeps the multiline format and leaves the original untouched.
printf -- '-a\n-b\n' >"$tmp/flags"
"$exec_sh" "$tmp/d.log" "$tmp/d.exit" --append 3 "$tmp/flags.out" --line '-c' --line '-d e' \
  -- "$fake" x 0 "$tmp/flags" || fail "append exits 0"
[ "$(cat "$tmp/flags")" = "$(printf -- '-a\n-b')" ] || fail "the original is unchanged"
[ "$(cat "$tmp/flags.out")" = "$(printf -- '-a\n-b\n-c\n-d e')" ] || fail "append adds one line per flag: $(cat "$tmp/flags.out")"

# Append to a missing file fails closed in the log and status.
"$exec_sh" "$tmp/e.log" "$tmp/e.exit" --append 3 "$tmp/none.out" --line '-c' -- "$fake" x 0 "$tmp/missing"
[ "$(cat "$tmp/e.exit")" = 2 ] || fail "missing append source is an error"

# An unknown option is rejected.
if "$exec_sh" "$tmp/f.log" "$tmp/f.exit" --bogus -- "$fake" x 0 2>/dev/null; then fail "unknown option must fail"; fi
echo PASS
