#!/bin/sh
# Runs the check-only replay of one compile action.
#
# usage: compile_check_exec.sh LOG STATUS [OPTION]... -- STEP [--dx-next-- STEP]...
#
#   --append IDX DEST --line L ...  copy STEP's argv[IDX] (the first step only) to DEST and
#                                   add one line per --line, then run with DEST in its place
#   --alias FROM TO                 replace FROM with TO (fixed strings) in the log
#
# Steps run in order and stop at the first nonzero exit. Their stderr is collected in LOG
# and the last exit code is written to STATUS. Only POSIX sh builtins are used so the replay
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
alias_count=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --append)
      append_idx=$2
      append_dest=$3
      shift 3
      ;;
    --line)
      append_text="$append_text$2
"
      shift 2
      ;;
    --alias)
      eval "alias_from_$alias_count=\$2"
      eval "alias_to_$alias_count=\$3"
      alias_count=$((alias_count + 1))
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

raw="$log.raw"
: > "$raw"
code=0
first=1

# run_step COUNT ARG...: runs the first COUNT of ARG... as one command.
run_step() {
  count=$1
  shift
  total=$#
  if [ "$count" -eq 0 ]; then
    echo "compile_check_exec: empty step" >> "$raw"
    code=2
    return 1
  fi
  r=0
  while [ "$r" -lt "$count" ]; do
    set -- "$@" "$1"
    shift
    r=$((r + 1))
  done
  shift $((total - count))
  if [ "$first" -eq 1 ] && [ -n "$append_idx" ]; then
    if [ "$append_idx" -ge "$count" ]; then
      echo "compile_check_exec: no file at argv[$append_idx] to append to" >> "$raw"
      code=2
      return 1
    fi
    r=0
    while [ "$r" -lt "$append_idx" ]; do
      set -- "$@" "$1"
      shift
      r=$((r + 1))
    done
    if [ ! -f "$1" ]; then
      echo "compile_check_exec: no file at argv[$append_idx] to append to" >> "$raw"
      code=2
      return 1
    fi
    content=$(while IFS= read -r l || [ -n "$l" ]; do printf '%s\n' "$l"; done < "$1")
    printf '%s\n%s' "$content" "$append_text" > "$append_dest"
    shift
    set -- "$append_dest" "$@"
    r=$((count - append_idx))
    while [ "$r" -gt 0 ]; do
      set -- "$@" "$1"
      shift
      r=$((r - 1))
    done
  fi
  first=0
  "$@" 2>> "$raw"
  code=$?
  [ "$code" -eq 0 ]
}

while :; do
  n=0
  for arg in "$@"; do
    if [ "$arg" = "--dx-next--" ]; then
      break
    fi
    n=$((n + 1))
  done
  run_step "$n" "$@" || break
  shift "$n"
  if [ "$#" -eq 0 ]; then
    break
  fi
  shift
done

: > "$log"
while IFS= read -r line || [ -n "$line" ]; do
  i=0
  while [ "$i" -lt "$alias_count" ]; do
    eval "from=\$alias_from_$i"
    eval "to=\$alias_to_$i"
    if [ -n "$from" ]; then
      out=""
      while :; do
        case $line in
          *"$from"*)
            out=$out${line%%"$from"*}$to
            line=${line#*"$from"}
            ;;
          *) break ;;
        esac
      done
      line=$out$line
    fi
    i=$((i + 1))
  done
  printf '%s\n' "$line" >> "$log"
done < "$raw"
rm -f "$raw"
echo "$code" > "$status"
