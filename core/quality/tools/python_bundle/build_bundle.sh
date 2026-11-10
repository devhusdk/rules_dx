#!/usr/bin/env bash
# Builds one relocatable Python quality tools bundle.
# usage: build_bundle.sh OUT UV PYTHON_MARKER UV_PLATFORM WINDOWS SKIP_PACKAGES TOOLS_DIR GROUPS...
# Each group is NAME=PROJECT_DIR=TOOL[:MAIN],TOOL[:MAIN]... with TOOL launchers backed by MAIN.py.
set -euo pipefail
out="$(realpath -m "$1")"
uv="$(realpath "$2")"
marker="$3"
platform="$4"
windows="$5"
skip="$6"
sources="$7"
shift 7
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
root="$work/bundle"
mkdir -p "$root/bin" "$root/site-packages" "$root/tools"
cp -RL "$(dirname "$marker")" "$root/python"
export UV_CACHE_DIR="$work/uv-cache" HOME="$work" UV_PYTHON_DOWNLOADS=never
cp "$sources/tool_bootstrap.py" "$root/tools/"
for group in "$@"; do
  name="${group%%=*}"
  rest="${group#*=}"
  project="${rest%%=*}"
  tools="${rest#*=}"
  emit=()
  for pkg in $skip; do emit+=(--no-emit-package "$pkg"); done
  "$uv" export --frozen --no-emit-project --format requirements-txt --project "$project" ${emit[@]+"${emit[@]}"} -o "$work/$name.txt" >/dev/null
  "$uv" pip install --require-hashes --only-binary :all: --no-deps --python-platform "$platform" --python-version 3.12 \
    --python "$PYTHON_EXEC" --target "$root/site-packages/$name" -r "$work/$name.txt"
  IFS=, read -ra list <<<"$tools"
  for entry in "${list[@]}"; do
    tool="${entry%%:*}"
    main="${entry#*:}"
    case "$skip" in *ansible*) [ "$tool" = ansible_lint ] && continue ;; esac
    cp "$sources/${main}_main.py" "$root/tools/"
    if [ "$windows" = true ]; then
      printf '@echo off\r\nset "PYTHONPATH=%%~dp0..\\site-packages\\%s;%%~dp0..\\tools"\r\n"%%~dp0..\\python\\python.exe" "%%~dp0..\\tools\\%s_main.py" %%*\r\n' "$name" "$main" > "$root/bin/$tool.cmd"
    else
      printf '#!/bin/sh\nhere=$(cd "$(dirname "$0")/.." && pwd)\nPYTHONPATH="$here/site-packages/%s:$here/tools" exec "$here/python/bin/python3" "$here/tools/%s_main.py" "$@"\n' "$name" "$main" > "$root/bin/$tool"
      chmod 755 "$root/bin/$tool"
    fi
  done
done
find "$root" -name __pycache__ -type d -prune -exec rm -rf {} +
mkdir -p "$(dirname "$out")"
(cd "$root" && LC_ALL=C tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner -cf - .) | gzip -n > "$out"
