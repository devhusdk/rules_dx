#!/usr/bin/env bash
set -euo pipefail
out="$(realpath -m "$1")"
uv="$(realpath "$2")"
marker="$3"
platform="$4"
windows="$5"
project="$6"
package="$7"
launcher="$8"
main="$9"
shift 9
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
root="$work/bundle"
site="$root/site-packages"
mkdir -p "$root/bin" "$site" "$root/tools"
cp -RL "$(dirname "$marker")" "$root/python"
export UV_CACHE_DIR="$work/uv-cache" HOME="$work" UV_PYTHON_DOWNLOADS=never LC_ALL=C
cp "$main" "$@" "$root/tools/"
"$uv" tree --frozen --universal --no-dedupe --project "$project" --package "$package" |
  sed -E 's/^[^[:alnum:]]*//; s/[][ ].*//' | sort -u >"$work/closure.txt"
grep -qx "$package" "$work/closure.txt" || {
  echo "$project locks no $package" >&2
  exit 1
}
"$uv" export --frozen --no-emit-project --no-annotate --no-header --format requirements-txt --project "$project" -o "$work/locked.txt" >/dev/null
awk 'NR == FNR { keep[$1] = 1; next } /^[^ ]/ { name = $0; sub(/[=; ].*/, "", name); on = (name in keep) } on' \
  "$work/closure.txt" "$work/locked.txt" >"$work/requirements.txt"
"$uv" pip install --require-hashes --only-binary :all: --no-deps --python-platform "$platform" --python-version 3.12 \
  --python "$PYTHON_EXEC" --target "$site" -r "$work/requirements.txt"
rm -rf "${site:?}/bin" "${site:?}"/*-stubs
for record in "$site"/*.dist-info/RECORD; do
  grep -vE '^(bin|[^/]+-stubs)/' "$record" >"$work/RECORD" || true
  mv "$work/RECORD" "$record"
done
main="$(basename "$main")"
if [ "$windows" = true ]; then
  printf '@echo off\r\nset "PYTHONPATH=%%~dp0..\\site-packages;%%~dp0..\\tools"\r\n"%%~dp0..\\python\\python.exe" "%%~dp0..\\tools\\%s" %%*\r\n' "$main" >"$root/bin/$launcher.cmd"
else
  printf '#!/bin/sh\nhere=$(cd "$(dirname "$0")/.." && pwd)\nPYTHONPATH="$here/site-packages:$here/tools" exec "$here/python/bin/python3" "$here/tools/%s" "$@"\n' "$main" >"$root/bin/$launcher"
  chmod 755 "$root/bin/$launcher"
fi
find "$root" -name __pycache__ -type d -prune -exec rm -rf {} +
mkdir -p "$(dirname "$out")"
(cd "$root" && tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner -cf - .) | gzip -n >"$out"
