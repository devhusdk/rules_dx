#!/usr/bin/env bash
# Writes tools/docs/support_matrix/row_labels.json: the labels tagged row:<name>, by row.
# Writes tools/docs/support_matrix/row_tests/<platform>.json: each row test compatible with the platform, with its tags other than row:*.
# Usage: tools/docs/support_matrix/row_tests.sh <platform id from languages.bzl PLATFORMS> [bazel flags]
set -euo pipefail
platform=$1
shift
cd "$(dirname "$0")/../../.."
bazel build "$@" //tools/docs/support_matrix:facts
bazel query "$@" 'attr(tags, "row:", //...)' --output=streamed_jsonproto 2> /dev/null |
  jq -s '[.[].rule | (.name | sub("^.*:"; "")) as $short | select(([.attribute[] | select(.name == "generator_name") | .stringValue | select(. != "")][0] // $short) == $short) | {label: .name, rows: [.attribute[] | select(.name == "tags") | .stringListValue[] | select(startswith("row:")) | ltrimstr("row:")]}] | map(.rows[] as $row | {row: $row, label}) | group_by(.row) | map({(.[0].row): (map(.label) | unique)}) | add // {}' > tools/docs/support_matrix/row_labels.json
facts=$(bazel cquery "$@" --output=files //tools/docs/support_matrix:facts 2> /dev/null)
query="set($(jq -rn --slurpfile labels tools/docs/support_matrix/row_labels.json --slurpfile facts "$facts" '[$labels[0][][], $facts[0].rows[].consumer_tests[]] | unique | join(" ")'))"
expr="str(target.label).removeprefix('@@') if 'IncompatiblePlatformProvider' not in providers(target) and [str(p).removeprefix('@@') for p in build_options(target)['//command_line_option:platforms']] == ['//tools/platforms:$platform'] else ''"
compatible=$(bazel cquery "$@" --platforms="//tools/platforms:$platform" "$query" --output=starlark --starlark:expr="$expr" 2> /dev/null)
bazel query "$@" "set($compatible)" --output=streamed_jsonproto 2> /dev/null |
  jq -s 'map({(.rule.name): ([.rule.attribute[] | select(.name == "tags") | .stringListValue // []] | add // [] | unique)}) | add // {} | map_values(map(select(startswith("row:") | not)))' > "tools/docs/support_matrix/row_tests/$platform.json"
