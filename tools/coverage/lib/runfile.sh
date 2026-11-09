runfile() {
  local root=${RUNFILES_DIR:-${TEST_SRCDIR:-}}
  if [ -n "$root" ] && [ -e "$root/$1" ]; then
    printf '%s\n' "$root/$1"
  elif [ -n "${RUNFILES_MANIFEST_FILE:-}" ]; then
    grep -m1 "^$1 " "$RUNFILES_MANIFEST_FILE" | cut -d' ' -f2-
  fi
}

dx_coverage_passthrough() {
  if [ -z "${COVERAGE_DIR:-}" ]; then
    exec "$@"
  fi
}
