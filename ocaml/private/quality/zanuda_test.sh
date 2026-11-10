#!/usr/bin/env bash
# Runs the built zanuda on typed-tree files compiled by the pinned toolchain.
set -euo pipefail
zanuda="$1"
dirty_list="$2"
clean_list="$3"
stdlib_cmi="$4"
cd "${RUNFILES_DIR:-${TEST_SRCDIR}}/${TEST_WORKSPACE}"
# zanuda type checks against the compiler's standard library at run time.
export OCAMLLIB="$PWD/$(dirname "$stdlib_cmi")"
scratch="${TEST_TMPDIR:-$(mktemp -d)}"
lint=(-no-top_file_license)

fail() { echo "FAIL: $*" >&2; exit 1; }

version="$("$zanuda" -version)"
# The dune-project of zanuda 2.1.0 still says 2.0.0, and that is what build-info embeds.
[[ "$version" == *"2.0.0"* ]] || fail "unexpected version: $version"

"$zanuda" -cmt-list "$dirty_list" "${lint[@]}" -ordjsonl "$scratch/dirty.jsonl" >/dev/null 2>&1 || fail "dirty fixture: zanuda exited non-zero"
for id in if_bool list_length_comparisons; do
  grep -q "\"value\":\"$id\"" "$scratch/dirty.jsonl" || { cat "$scratch/dirty.jsonl" >&2; fail "dirty fixture: no $id finding"; }
done
# Typed and untyped findings alike name the source the list gives, not the compile-time path.
listed="$(awk -F '\t' 'NR == 1 {print $2}' "$dirty_list")"
while IFS= read -r path; do
  [[ "$path" == "$listed" ]] || fail "dirty fixture: finding names $path, want $listed"
done < <(grep -o '"path":"[^"]*"' "$scratch/dirty.jsonl" | sed 's/^"path":"//; s/"$//')
grep -q '"path"' "$scratch/dirty.jsonl" || fail "dirty fixture: no finding names a path"

"$zanuda" -cmt-list "$clean_list" "${lint[@]}" -ordjsonl "$scratch/clean.jsonl" >/dev/null 2>&1 || fail "clean fixture: zanuda exited non-zero"
[[ ! -s "$scratch/clean.jsonl" ]] || { cat "$scratch/clean.jsonl" >&2; fail "clean fixture: unexpected findings"; }

# A missing or corrupt .cmt must not look like a clean run.
awk -F '\t' '{print $1 "\t" $2 "\t" "does-not-exist.cmt"}' "$clean_list" >"$scratch/missing.list"
if "$zanuda" -cmt-list "$scratch/missing.list" "${lint[@]}" -ordjsonl "$scratch/missing.jsonl" >/dev/null 2>&1; then
  fail "missing .cmt: zanuda exited zero"
fi
echo "not a cmt file" >"$scratch/corrupt.cmt"
awk -F '\t' -v cmt="$scratch/corrupt.cmt" '{print $1 "\t" $2 "\t" cmt}' "$clean_list" >"$scratch/corrupt.list"
if "$zanuda" -cmt-list "$scratch/corrupt.list" "${lint[@]}" -ordjsonl "$scratch/corrupt.jsonl" >/dev/null 2>&1; then
  fail "corrupt .cmt: zanuda exited zero"
fi
echo "PASS"
