#!/usr/bin/env bash
# Merges compiler evidence lines into one markdown table of compiler row x platform.
# usage: compiler_table.sh EVIDENCE_DIR OUT PLATFORM...
# Copies every *.jsonl line under EVIDENCE_DIR to OUT and prints the table on stdout.
# A cell is failed if any line failed, else mismatch if any line is a mismatch, else passed if any line passed,
# skipped if only skipped lines exist, else no evidence.
set -euo pipefail

dir="$1"; out="$2"
shift 2

: > "$out"
while IFS= read -r file; do
  tr -d '\r' < "$file" | grep . >> "$out" || true
done < <(find "$dir" -name '*.jsonl' | sort)

jq -rs '
  def cell($row; $platform):
    [.[] | select(.row == $row and .platform == $platform) | .outcome] as $o
    | if ($o | length) == 0 then "no evidence"
      elif ($o | index("failed")) then "failed"
      elif ($o | index("mismatch")) then "mismatch"
      elif ($o | index("passed")) then "passed"
      else "skipped" end;
  . as $lines
  | ($ARGS.positional + [.[].platform] | unique) as $platforms
  | ([.[].row] | unique) as $rows
  | "| row | " + ($platforms | join(" | ")) + " |",
    "| --- |" + ($platforms | map(" --- |") | join("")),
    ($rows[] as $row
      | "| \($row) | " + ([$platforms[] as $p | $lines | cell($row; $p)] | join(" | ")) + " |")
' --args "$@" < "$out"
