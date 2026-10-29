#!/bin/bash
# Runs the check-only replay of one compile action.
#
# usage: compile_check_exec.sh LOG STATUS [OPTION]... -- STEP [--dx-next-- STEP]...
#
#   --append IDX DEST --line L ...  copy STEP's argv[IDX] (the first step only) to DEST and
#                                   add one line per --line, then run with DEST in its place
#   --alias FROM TO                 replace FROM with TO (fixed strings) in the log
#
# Steps run in order and stop at the first nonzero exit. Their stderr is collected in LOG
# and the last exit code is written to STATUS. Only bash builtins are used so the replay
# does not depend on PATH.

if [ "$#" -lt 3 ]; then
  echo "compile_check_exec: usage: LOG STATUS [OPTION]... -- STEP [--dx-next-- STEP]..." >&2
  exit 2
fi
log=$1
status=$2
shift 2

append_idx=""
append_dest=""
append_text=""
from=()
to=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --append)
      append_idx=$2
      append_dest=$3
      shift 3
      ;;
    --line)
      append_text+="$2"$'\n'
      shift 2
      ;;
    --alias)
      from+=("$2")
      to+=("$3")
      shift 3
      ;;
    --)
      shift
      break
      ;;
    *)
      echo "compile_check_exec: unknown option $1" >&2
      exit 2
      ;;
  esac
done

steps=()
current=()
for arg in "$@"; do
  if [ "$arg" = "--dx-next--" ]; then
    steps+=("${#current[@]}")
    steps+=("${current[@]}")
    current=()
  else
    current+=("$arg")
  fi
done
steps+=("${#current[@]}")
steps+=("${current[@]}")

raw="$log.raw"
: > "$raw"
code=0
index=0
first=1
while [ "$index" -lt "${#steps[@]}" ]; do
  count=${steps[$index]}
  argv=("${steps[@]:$((index + 1)):$count}")
  index=$((index + 1 + count))
  if [ "$count" -eq 0 ]; then
    echo "compile_check_exec: empty step" >> "$raw"
    code=2
    break
  fi
  if [ "$first" -eq 1 ] && [ -n "$append_idx" ]; then
    if [ "$append_idx" -ge "$count" ] || [ ! -f "${argv[$append_idx]}" ]; then
      echo "compile_check_exec: no file at argv[$append_idx] to append to" >> "$raw"
      code=2
      break
    fi
    content=$(< "${argv[$append_idx]}")
    printf '%s\n%s' "$content" "$append_text" > "$append_dest"
    argv[append_idx]=$append_dest
  fi
  first=0
  "${argv[@]}" 2>> "$raw"
  code=$?
  if [ "$code" -ne 0 ]; then
    break
  fi
done

: > "$log"
while IFS= read -r line || [ -n "$line" ]; do
  i=0
  while [ "$i" -lt "${#from[@]}" ]; do
    line=${line//"${from[$i]}"/${to[$i]}}
    i=$((i + 1))
  done
  printf '%s\n' "$line" >> "$log"
done < "$raw"
rm -f "$raw"
echo "$code" > "$status"
