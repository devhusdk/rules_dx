#!/usr/bin/env bash
set -euo pipefail

if ! declare -F rlocation >/dev/null 2>&1; then
  _dx_runfiles_bash="bazel_tools/tools/bash/runfiles/runfiles.bash"
  source "${RUNFILES_DIR:-/dev/null}/$_dx_runfiles_bash" 2>/dev/null ||
    source "$(grep -sm1 "^$_dx_runfiles_bash " "${RUNFILES_MANIFEST_FILE:-/dev/null}" 2>/dev/null | cut -f2- -d' ')" 2>/dev/null ||
    source "$0.runfiles/$_dx_runfiles_bash" 2>/dev/null ||
    source "$(grep -sm1 "^$_dx_runfiles_bash " "$0.runfiles_manifest" 2>/dev/null | cut -f2- -d' ')" 2>/dev/null ||
    source "$(grep -sm1 "^$_dx_runfiles_bash " "$0.exe.runfiles_manifest" 2>/dev/null | cut -f2- -d' ')" 2>/dev/null ||
    true
  unset _dx_runfiles_bash
fi

dx_workspace_root() {
  if [[ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ]]; then
    printf '%s\n' "$BUILD_WORKSPACE_DIRECTORY"
    return 0
  fi
  git rev-parse --show-toplevel
}

dx_runfiles_root() {
  if [[ -n "${RUNFILES_DIR:-}" ]]; then
    printf '%s\n' "$RUNFILES_DIR"
    return 0
  fi
  if [[ -n "${TEST_SRCDIR:-}" ]]; then
    printf '%s\n' "$TEST_SRCDIR"
    return 0
  fi
  echo "FAIL: no Bazel runfiles root (RUNFILES_DIR and TEST_SRCDIR are unset)" >&2
  return 1
}

dx_resolve_runfile() {
  local rel="$1" stripped base cand
  stripped="$(printf '%s' "$rel" | sed -e 's|^\(\.\./\)*||')"
  if [[ "$rel" = /* ]] && [[ -e "$rel" ]]; then
    printf '%s\n' "$rel"
    return 0
  fi
  if declare -F rlocation >/dev/null 2>&1; then
    for cand in "_main/$rel" "_main/$stripped" "rules_dx/$rel" "rules_dx/$stripped" "$rel" "$stripped"; do
      if base="$(rlocation "$cand" 2>/dev/null)" && [[ -n "$base" && -e "$base" ]]; then
        printf '%s\n' "$base"
        return 0
      fi
    done
  fi
  if [[ -n "${TEST_SRCDIR:-}" ]]; then
    for base in "$TEST_SRCDIR/_main/$rel" "$TEST_SRCDIR/$rel" "$TEST_SRCDIR/$stripped" "$TEST_SRCDIR/rules_dx/$rel" "$TEST_SRCDIR/rules_dx/$stripped"; do
      if [[ -e "$base" ]]; then
        printf '%s\n' "$base"
        return 0
      fi
    done
  fi
  if [[ -n "${RUNFILES_DIR:-}" && "${RUNFILES_DIR:-}" != "${TEST_SRCDIR:-}" ]]; then
    for base in "$RUNFILES_DIR/_main/$rel" "$RUNFILES_DIR/$rel" "$RUNFILES_DIR/$stripped" "$RUNFILES_DIR/rules_dx/$rel" "$RUNFILES_DIR/rules_dx/$stripped"; do
      if [[ -e "$base" ]]; then
        printf '%s\n' "$base"
        return 0
      fi
    done
  fi
  if [[ -n "${TEST_SRCDIR:-}" ]]; then
    local found
    found="$(find "${TEST_SRCDIR:-/nonexistent}" -path "*$rel" -print -quit 2>/dev/null || true)"
    if [[ -n "$found" ]]; then
      printf '%s\n' "$found"
      return 0
    fi
    if [[ "$stripped" != "$rel" ]]; then
      found="$(find "${TEST_SRCDIR:-/nonexistent}" -path "*$stripped" -print -quit 2>/dev/null || true)"
      if [[ -n "$found" ]]; then
        printf '%s\n' "$found"
        return 0
      fi
    fi
  fi
  if [[ -n "${BUILD_WORKSPACE_DIRECTORY:-}" && -e "$BUILD_WORKSPACE_DIRECTORY/$rel" ]]; then
    printf '%s\n' "$BUILD_WORKSPACE_DIRECTORY/$rel"
    return 0
  fi
  if [[ -n "${BUILD_WORKSPACE_DIRECTORY:-}" && -e "$BUILD_WORKSPACE_DIRECTORY/$stripped" ]]; then
    printf '%s\n' "$BUILD_WORKSPACE_DIRECTORY/$stripped"
    return 0
  fi
  for base in "$(pwd)/bazel-bin/$rel" "$(pwd)/bazel-bin/$stripped" "$(pwd)/$rel"; do
    if [[ -e "$base" ]]; then
      printf '%s\n' "$base"
      return 0
    fi
  done
  if [[ -e "$rel" ]]; then
    printf '%s\n' "$rel"
    return 0
  fi
  local ws
  if ws="$(git rev-parse --show-toplevel 2>/dev/null)"; then
    if [[ -e "$ws/$rel" ]]; then
      printf '%s\n' "$ws/$rel"
      return 0
    fi
    if [[ -e "$ws/$stripped" ]]; then
      printf '%s\n' "$ws/$stripped"
      return 0
    fi
  fi
  return 1
}

dx_test_init() {
  pass=0
  fail=0
}

ok() {
  pass=$((pass + 1))
  if [[ $# -gt 0 ]]; then
    echo "ok: $1"
  fi
}

bad() {
  echo "FAIL: $1" >&2
  fail=$((fail + 1))
}

dx_test_summary() {
  local name="$1"
  echo "$name: $pass passed, $fail failed"
  [[ "$fail" == "0" ]]
}

dx_cd_workspace() {
  workspace="$(dx_workspace_root)"
  cd "$workspace"
}

_DX_SCRATCHES=()
_DX_SCRATCH_TRAP_INSTALLED=0

_dx_cleanup_scratches() {
  local d
  if ((${#_DX_SCRATCHES[@]} == 0)); then
    return 0
  fi
  for d in "${_DX_SCRATCHES[@]}"; do
    if [[ -n "$d" && -e "$d" ]]; then
      rm -rf "$d"
    fi
  done
}

dx_mkscratch() {
  local var="$1" template="${2:-}" dir
  if [[ -n "$template" ]]; then
    dir="$(mktemp -d "$template")"
  else
    dir="$(mktemp -d)"
  fi
  _DX_SCRATCHES+=("$dir")
  if [[ "$_DX_SCRATCH_TRAP_INSTALLED" == "0" ]]; then
    trap '_dx_cleanup_scratches' EXIT
    _DX_SCRATCH_TRAP_INSTALLED=1
  fi
  printf -v "$var" '%s' "$dir"
}

dx_mktemp_file() {
  local var="$1" template="${2:-}" file
  if [[ -n "$template" ]]; then
    file="$(mktemp "$template")"
  else
    file="$(mktemp)"
  fi
  _DX_SCRATCHES+=("$file")
  if [[ "$_DX_SCRATCH_TRAP_INSTALLED" == "0" ]]; then
    trap '_dx_cleanup_scratches' EXIT
    _DX_SCRATCH_TRAP_INSTALLED=1
  fi
  printf -v "$var" '%s' "$file"
}

dx_realpath() {
  if command -v realpath >/dev/null 2>&1; then
    realpath "$1"
  elif command -v readlink >/dev/null 2>&1 && readlink -f "$1" >/dev/null 2>&1; then
    readlink -f "$1"
  else
    python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$1"
  fi
}

portable_realpath() {
  dx_realpath "$@"
}

dx_sha256_file() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | cut -d' ' -f1
  else
    python3 -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$file"
  fi
}

dx_sha256_stdin() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 | cut -d' ' -f1
  else
    python3 -c 'import hashlib,sys; print(hashlib.sha256(sys.stdin.buffer.read()).hexdigest())'
  fi
}

dx_tree_sha256() {
  local dir="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$dir" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | cut -d' ' -f1)
  elif command -v shasum >/dev/null 2>&1; then
    (cd "$dir" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 shasum -a 256 | shasum -a 256 | cut -d' ' -f1)
  else
    python3 -c '
import hashlib, os, sys
root = sys.argv[1]
names = []
for base, _dirs, files in os.walk(root):
  for f in files:
    rel = os.path.relpath(os.path.join(base, f), root)
    names.append(rel)
names.sort()
lines = []
for rel in names:
  with open(os.path.join(root, rel), "rb") as fh:
    lines.append(hashlib.sha256(fh.read()).hexdigest() + "  " + rel)
print(hashlib.sha256(("\n".join(lines) + "\n").encode()).hexdigest())
' "$dir"
  fi
}

dx_sha256_check() {
  local sums="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$sums"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$sums"
  else
    python3 -c '
import hashlib, sys
ok = True
for line in open(sys.argv[1]):
  line = line.rstrip("\n")
  if not line:
    continue
  want, _, path = line.partition("  ")
  if not want or not path:
    want, _, path = line.partition(" *")
  try:
    got = hashlib.sha256(open(path, "rb").read()).hexdigest()
  except OSError:
    print(path + ": FAILED open") 
    ok = False
    continue
  if got != want:
    print(path + ": FAILED") 
    ok = False
sys.exit(0 if ok else 1)
' "$sums"
  fi
}

dx_now_secs() {
  if [[ -n "${EPOCHREALTIME:-}" ]]; then
    printf '%s' "${EPOCHREALTIME}"
  elif date +%s.%N >/dev/null 2>&1; then
    date +%s.%N
  else
    date +%s
  fi
}

now_secs() {
  dx_now_secs
}

dx_replace() {
  local expr="$1" file="$2" tmp
  tmp="$file.tmp"
  sed -e "$expr" "$file" >"$tmp" && mv "$tmp" "$file"
}

dx_python3() {
  if command -v python3 >/dev/null 2>&1; then
    printf '%s\n' "python3"
  else
    printf '%s\n' "python"
  fi
}

dx_hermetic_grep_py() {
  local cand
  if [[ -n "${RUNFILES_DIR:-}" ]]; then
    for cand in "$RUNFILES_DIR/_main/tools/sh/hermetic_grep.py" "$RUNFILES_DIR/tools/sh/hermetic_grep.py"; do
      if [[ -f "$cand" ]]; then
        printf '%s\n' "$cand"
        return 0
      fi
    done
  fi
  if [[ -n "${TEST_SRCDIR:-}" ]]; then
    for cand in "$TEST_SRCDIR/_main/tools/sh/hermetic_grep.py" "$TEST_SRCDIR/tools/sh/hermetic_grep.py"; do
      if [[ -f "$cand" ]]; then
        printf '%s\n' "$cand"
        return 0
      fi
    done
  fi
  if [[ -n "${BUILD_WORKSPACE_DIRECTORY:-}" && -f "${BUILD_WORKSPACE_DIRECTORY}/tools/sh/hermetic_grep.py" ]]; then
    printf '%s\n' "${BUILD_WORKSPACE_DIRECTORY}/tools/sh/hermetic_grep.py"
    return 0
  fi
  local top
  if top="$(git rev-parse --show-toplevel 2>/dev/null)"; then
    if [[ -f "$top/tools/sh/hermetic_grep.py" ]]; then
      printf '%s\n' "$top/tools/sh/hermetic_grep.py"
      return 0
    fi
  fi
  if [[ -f "tools/sh/hermetic_grep.py" ]]; then
    printf '%s\n' "tools/sh/hermetic_grep.py"
    return 0
  fi
  echo "dx_hermetic_grep: cannot locate tools/sh/hermetic_grep.py" >&2
  return 1
}

dx_hermetic_grep() {
  local py
  py="$(dx_hermetic_grep_py)" || return 1
  "$(dx_python3)" "$py" "$@"
}

dx_grep_contains() {
  local file="$1"
  shift
  dx_hermetic_grep contains "$file" --fixed -- "$@"
}

dx_grep_absent() {
  local file="$1"
  shift
  dx_hermetic_grep absent "$file" --fixed -- "$@"
}

dx_grep_re_contains() {
  local file="$1"
  shift
  dx_hermetic_grep contains "$file" --re -- "$@"
}

dx_grep_re_absent() {
  local file="$1"
  shift
  dx_hermetic_grep absent "$file" --re -- "$@"
}

dx_tree_contains() {
  local includes=() excludes=() allows=() allow_paths=() patterns=() roots=()
  local in_roots=0 arg
  for arg in "$@"; do
    if [[ "$arg" == "--" && "$in_roots" == "0" ]]; then
      in_roots=1
      continue
    fi
    if [[ "$in_roots" == "1" ]]; then
      roots+=("$arg")
      continue
    fi
    case "$arg" in
    --include=*) includes+=(--include "${arg#--include=}") ;;
    --exclude=*) excludes+=(--exclude "${arg#--exclude=}") ;;
    --allow=*) allows+=(--allow "${arg#--allow=}") ;;
    --allow-path=*) allow_paths+=(--allow-path "${arg#--allow-path=}") ;;
    *) patterns+=("$arg") ;;
    esac
  done
  if [[ "${#roots[@]}" == "0" ]]; then
    roots=(.)
  fi
  if [[ "${DX_TREE_RE:-0}" == "1" ]]; then
    dx_hermetic_grep tree-contains --re "${includes[@]}" "${excludes[@]}" "${allows[@]}" "${allow_paths[@]}" --roots "${roots[@]}" -- "${patterns[@]}"
  else
    dx_hermetic_grep tree-contains --fixed "${includes[@]}" "${excludes[@]}" "${allows[@]}" "${allow_paths[@]}" --roots "${roots[@]}" -- "${patterns[@]}"
  fi
}

dx_tree_absent() {
  local includes=() excludes=() allows=() allow_paths=() patterns=() roots=()
  local in_roots=0 arg
  for arg in "$@"; do
    if [[ "$arg" == "--" && "$in_roots" == "0" ]]; then
      in_roots=1
      continue
    fi
    if [[ "$in_roots" == "1" ]]; then
      roots+=("$arg")
      continue
    fi
    case "$arg" in
    --include=*) includes+=(--include "${arg#--include=}") ;;
    --exclude=*) excludes+=(--exclude "${arg#--exclude=}") ;;
    --allow=*) allows+=(--allow "${arg#--allow=}") ;;
    --allow-path=*) allow_paths+=(--allow-path "${arg#--allow-path=}") ;;
    *) patterns+=("$arg") ;;
    esac
  done
  if [[ "${#roots[@]}" == "0" ]]; then
    roots=(.)
  fi
  if [[ "${DX_TREE_RE:-0}" == "1" ]]; then
    dx_hermetic_grep tree-absent --re "${includes[@]}" "${excludes[@]}" "${allows[@]}" "${allow_paths[@]}" --roots "${roots[@]}" -- "${patterns[@]}"
  else
    dx_hermetic_grep tree-absent --fixed "${includes[@]}" "${excludes[@]}" "${allows[@]}" "${allow_paths[@]}" --roots "${roots[@]}" -- "${patterns[@]}"
  fi
}

dx_context_contains() {
  local file="$1" anchor="$2"
  shift 2
  local after="" patterns=() arg
  for arg in "$@"; do
    case "$arg" in
    -A) continue ;;
    [0-9]*) after="$arg" ;;
    *) patterns+=("$arg") ;;
    esac
  done
  if [[ -z "$after" ]]; then
    echo "dx_context_contains: want FILE ANCHOR -A N PATTERN..." >&2
    return 2
  fi
  local anchor_flag="--anchor-fixed" mode_flag="--fixed"
  [[ "${DX_CONTEXT_ANCHOR_RE:-0}" == "1" ]] && anchor_flag="--anchor-re"
  [[ "${DX_CONTEXT_RE:-0}" == "1" ]] && mode_flag="--re"
  # shellcheck disable=SC2086
  dx_hermetic_grep context-contains "$file" "$anchor" -A "$after" $anchor_flag $mode_flag -- "${patterns[@]}"
}

dx_context_absent() {
  local file="$1" anchor="$2"
  shift 2
  local after="" patterns=() arg
  for arg in "$@"; do
    case "$arg" in
    -A) continue ;;
    [0-9]*) after="$arg" ;;
    *) patterns+=("$arg") ;;
    esac
  done
  if [[ -z "$after" ]]; then
    echo "dx_context_absent: want FILE ANCHOR -A N PATTERN..." >&2
    return 2
  fi
  local anchor_flag="--anchor-fixed" mode_flag="--fixed"
  [[ "${DX_CONTEXT_ANCHOR_RE:-0}" == "1" ]] && anchor_flag="--anchor-re"
  [[ "${DX_CONTEXT_RE:-0}" == "1" ]] && mode_flag="--re"
  # shellcheck disable=SC2086
  dx_hermetic_grep context-absent "$file" "$anchor" -A "$after" $anchor_flag $mode_flag -- "${patterns[@]}"
}

dx_extract_quoted() {
  dx_hermetic_grep extract-quoted "$@"
}

dx_extract_re() {
  dx_hermetic_grep extract-re "$@"
}

dx_bash_pin() {
  local major="${BASH_VERSINFO[0]:-0}" minor="${BASH_VERSINFO[1]:-0}"
  echo "bash ${BASH_VERSION:-unknown} (floor 3.2+)"
  if [[ "$major" -gt 3 ]] || [[ "$major" == "3" && "$minor" -ge 2 ]]; then
    return 0
  fi
  echo "FAIL: bash floor 3.2+ required, found ${BASH_VERSION:-unknown}" >&2
  return 1
}

dx_expect_file() {
  local file="$1"
  if [[ -f "$file" ]]; then
    ok
  else
    bad "missing file $file"
  fi
  return 0
}

dx_expect_contains() {
  local file="$1"
  shift
  local missing="" lit
  if [[ ! -f "$file" ]]; then
    bad "missing file $file (want literals: $*)"
    return 0
  fi
  for lit in "$@"; do
    if ! dx_hermetic_grep contains "$file" --fixed -- "$lit" >/dev/null 2>&1; then
      missing="$missing [$lit]"
    fi
  done
  if [[ -z "$missing" ]]; then
    ok
  else
    bad "$file missing literals:$missing"
  fi
  return 0
}

dx_expect_absent() {
  local file="$1"
  shift
  local present="" lit
  if [[ ! -f "$file" ]]; then
    bad "missing file $file (want absence of: $*)"
    return 0
  fi
  for lit in "$@"; do
    if dx_hermetic_grep contains "$file" --fixed -- "$lit" >/dev/null 2>&1; then
      present="$present [$lit]"
    fi
  done
  if [[ -z "$present" ]]; then
    ok
  else
    bad "$file must not contain:$present"
  fi
  return 0
}

if [[ -z "${LLVM_PROFILE_FILE:-}" ]]; then
  dx_mkscratch _DX_PROFRAW_DIR
  export LLVM_PROFILE_FILE="$_DX_PROFRAW_DIR/profraw_%m_%p.profraw"
fi
