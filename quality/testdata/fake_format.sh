#!/usr/bin/env bash
set -euo pipefail
has_fix=0
has_check=0
has_diff=0
has_reformat=0
has_pos_check=0
files=()
skip_next=0
for arg in "$@"; do
  if [[ "$skip_next" == "1" ]]; then
    skip_next=0
    continue
  fi
  case "$arg" in
    --config|--config-path) skip_next=1 ;;
    --check) has_check=1 ;;
    --diff|-d) has_diff=1 ;;
    --reformat)
      has_fix=1
      has_reformat=1
      ;;
    -w|--write|--fix|-i) has_fix=1 ;;
    check) has_pos_check=1 ;;
    -*) ;;
    *) if [[ -f "$arg" ]]; then files+=("$arg"); fi ;;
  esac
done
if [[ "${#files[@]}" == "0" ]]; then
  exit 0
fi
ext="${files[0]##*.}"
mode=diff
case "$ext" in
  cs|qml) mode=relpath ;;
  fs) mode=json ;;
  go) mode=diff0 ;;
  css|less|scss|feature|sql|xml) mode=warn ;;
esac
want_fix=0
case "$ext" in
  proto) if [[ "$has_diff" == "0" ]]; then want_fix=1; fi ;;
  html) if [[ "$has_reformat" == "1" && "$has_check" == "0" ]]; then want_fix=1; fi ;;
  fs) if [[ "$has_pos_check" == "0" ]]; then want_fix=1; fi ;;
  cs|qml) if [[ "$has_pos_check" == "0" && "$has_check" == "0" ]]; then want_fix=1; fi ;;
  scala) if [[ "$has_check" == "0" ]]; then want_fix=1; fi ;;
  *) if [[ "$has_fix" == "1" ]]; then want_fix=1; fi ;;
esac
relpath() {
  python3 -c 'import os,sys; print(os.path.relpath(sys.argv[1], sys.argv[2]))' "$1" "$PWD"
}
if [[ "$want_fix" == "1" ]]; then
  for f in "${files[@]}"; do
    if grep -q -F "BADFMT" "$f"; then
      sed 's/BADFMT/fixed/g' "$f" >"$f.dxtmp" && mv "$f.dxtmp" "$f"
    fi
  done
  exit 0
fi
case "$mode" in
  relpath)
    dirty=0
    for f in "${files[@]}"; do
      if grep -q -F "BADFMT" "$f"; then
        relpath "$f"
        dirty=1
      fi
    done
    if [[ "$dirty" == "1" ]]; then exit 1; fi
    exit 0
    ;;
  json)
    entries=()
    dirty=0
    for f in "${files[@]}"; do
      rel="$(relpath "$f")"
      if grep -q -F "BADFMT" "$f"; then
        entries+=("{\"path\": \"$rel\", \"status\": \"needs-formatting\"}")
        dirty=1
      else
        entries+=("{\"path\": \"$rel\", \"status\": \"unchanged\"}")
      fi
    done
    joined=""
    for e in "${entries[@]}"; do
      if [[ -z "$joined" ]]; then joined="$e"; else joined="$joined, $e"; fi
    done
    printf '{"files": [%s]}\n' "$joined"
    if [[ "$dirty" == "1" ]]; then exit 99; fi
    exit 0
    ;;
  warn)
    dirty=0
    for f in "${files[@]}"; do
      if grep -q -F "BADFMT" "$f"; then
        rel="${f#"$PWD"/}"
        if [[ "$rel" == "$f" ]]; then
          rel="${f##*/}"
        fi
        printf -- "[warn] %s\n" "$rel" >&2
        dirty=1
      fi
    done
    if [[ "$dirty" == "1" ]]; then exit 1; fi
    exit 0
    ;;
  *)
    dirty=0
    for f in "${files[@]}"; do
      if grep -q -F "BADFMT" "$f"; then
        printf -- "--- a/%s\n+++ b/%s\n@@ -1 +1 @@\n-BADFMT\n+fixed\n" "$f" "$f"
        dirty=1
      fi
    done
    if [[ "$mode" == "diff0" ]]; then exit 0; fi
    if [[ "$dirty" == "1" ]]; then exit 1; fi
    exit 0
    ;;
esac
