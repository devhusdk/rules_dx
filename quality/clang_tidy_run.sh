#!/usr/bin/env bash
# Runs clang-tidy for the ClangTidy aspect action: clang_tidy_run.sh <log> <clang-tidy argv...>.
# Expands Apple's __BAZEL_* placeholders, which only have values at execution time.
# Output goes to the log and the exit status is always 0, as diagnostics are parsed later.
log="$1"
shift
args=()
for a in "$@"; do
  a="${a//__BAZEL_EXECUTION_ROOT__/$PWD}"
  if [[ "$a" == *__BAZEL_XCODE_* ]]; then
    if [[ -z "${DEVELOPER_DIR:-}" || -z "${SDKROOT:-}" ]]; then
      echo "clang_tidy_run: cannot expand an Xcode placeholder in '$a': DEVELOPER_DIR and SDKROOT must both be set" >&2
      exit 1
    fi
    a="${a//__BAZEL_XCODE_DEVELOPER_DIR__/$DEVELOPER_DIR}"
    a="${a//__BAZEL_XCODE_SDKROOT__/$SDKROOT}"
  fi
  args+=("$a")
done
"${args[@]}" >"$log" 2>&1
true
