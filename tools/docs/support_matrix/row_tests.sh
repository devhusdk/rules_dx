#!/usr/bin/env bash
# Writes tools/docs/support_matrix/row_tests/<platform>.json: each row test compatible with the platform, with its tags.
# Usage: tools/docs/support_matrix/row_tests.sh <platform id from languages.bzl PLATFORMS> [bazel flags]
set -euo pipefail
platform=$1
shift
cd "$(dirname "$0")/../../.."
bazel build "$@" //tools/docs/support_matrix:facts
facts=$(bazel cquery "$@" --output=files //tools/docs/support_matrix:facts 2> /dev/null)
query="set($(jq -r '[.rows[] | .ci_tests[], .consumer_tests[]] | unique | join(" ")' "$facts"))"
expr="str(target.label).removeprefix('@@') if 'IncompatiblePlatformProvider' not in providers(target) and [str(p).removeprefix('@@') for p in build_options(target)['//command_line_option:platforms']] == ['//tools/platforms:$platform'] else ''"
compatible=$(bazel cquery "$@" --platforms="//tools/platforms:$platform" "$query" --output=starlark --starlark:expr="$expr" 2> /dev/null)
bazel query "$@" "set($compatible)" --output=streamed_jsonproto 2> /dev/null |
  jq -s 'map({(.rule.name): ([.rule.attribute[] | select(.name == "tags") | .stringListValue // []] | add // [] | unique)}) | add // {}' > "tools/docs/support_matrix/row_tests/$platform.json"
