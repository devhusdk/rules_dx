#!/usr/bin/env bash
# Writes tools/docs/support_matrix/row_labels.json and consumer_labels.json: the labels tagged row:<name> and consumer-row:<name>, by row.
# Writes tools/docs/support_matrix/row_tests/<platform>.json: each row test compatible with the platform, with its tags other than row:* and consumer-row:*.
# Usage: tools/docs/support_matrix/row_tests.sh <platform id from languages.bzl PLATFORMS> [bazel flags]
set -euo pipefail
platform=$1
shift
cd "$(dirname "$0")/../../.."
tagged=$(bazel query "$@" 'attr(tags, "row:", //...)' --output=streamed_jsonproto 2> /dev/null)
for prefix in row consumer-row; do
  jq <<<"$tagged" -s --arg prefix "$prefix" '[.[].rule | (.name | sub("^.*:"; "")) as $short | select(([.attribute[] | select(.name == "generator_name") | .stringValue | select(. != "")][0] // $short) == $short) | {label: .name, rows: [.attribute[] | select(.name == "tags") | .stringListValue[] | select(startswith($prefix + ":")) | ltrimstr($prefix + ":")]}] | map(.rows[] as $row | {row: $row, label}) | group_by(.row) | map({(.[0].row): (map(.label) | unique)}) | add // {}' > "tools/docs/support_matrix/${prefix//consumer-row/consumer}_labels.json"
done
query="set($(jq -rn --slurpfile labels tools/docs/support_matrix/row_labels.json --slurpfile consumers tools/docs/support_matrix/consumer_labels.json '[$labels[0][][], $consumers[0][][]] | unique | join(" ")'))"
expr="str(target.label).removeprefix('@@') if 'IncompatiblePlatformProvider' not in providers(target) and [str(p).removeprefix('@@') for p in build_options(target)['//command_line_option:platforms']] == ['//tools/platforms:$platform'] else ''"
compatible=$(bazel cquery "$@" --platforms="//tools/platforms:$platform" "$query" --output=starlark --starlark:expr="$expr" 2> /dev/null)
bazel query "$@" "set($compatible)" --output=streamed_jsonproto 2> /dev/null |
  jq -s 'map({(.rule.name): ([.rule.attribute[] | select(.name == "tags") | .stringListValue // []] | add // [] | unique)}) | add // {} | map_values(map(select(startswith("row:") or startswith("consumer-row:") | not)))' > "tools/docs/support_matrix/row_tests/$platform.json"
