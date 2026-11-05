#!/usr/bin/env bash
# Reads a "Blocked on:" line on stdin and prints one "<owner/repo> <number> <ref as written>" per reference.
# Bare #N is local to the repo given as $1. Write every cross-repo reference in full.
set -euo pipefail
local_repo=${1:?usage: blocker_refs.sh <owner/repo>}
rest=$(cat)

url='https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(pull|issues)/[0-9]+'
for ref in $(printf '%s\n' "$rest" | grep -oE "$url" || true); do
  path=${ref#https://github.com/}
  repo=${path%/*/*}
  echo "$repo ${ref##*/} $ref"
done
rest=$(printf '%s\n' "$rest" | sed -E "s#$url##g")

short='[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+#[0-9]+'
for ref in $(printf '%s\n' "$rest" | grep -oE "$short" || true); do
  echo "${ref%#*} ${ref#*#} $ref"
done
rest=$(printf '%s\n' "$rest" | sed -E "s|$short||g")

for ref in $(printf '%s\n' "$rest" | grep -oE '#[0-9]+' || true); do
  echo "$local_repo ${ref#\#} $ref"
done
