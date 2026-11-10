#!/usr/bin/env bash
# Guards the source-built selene: Lua 5.2-5.4 syntax must parse (upstream
# release binaries drop the lua52/53/54 features, Kampfkarren/selene#645) and
# real lints must still fire, with the staged default config.
set -euo pipefail

selene="$1"
config_dir="$(dirname "$2")"
valid="$3"
invalid="$4"
[[ "$selene" == /* ]] || selene="$PWD/$selene"
cfg="$PWD/$config_dir"
[[ "$config_dir" == /* ]] && cfg="$config_dir"
run=(--config "$cfg/selene.toml" --display-style json2 --color Never --no-exclude --num-threads 1)

version="$("$selene" --version)"
[[ "$version" == "selene 0.32.0" ]] || { echo "unexpected version: $version" >&2; exit 1; }

out="$("$selene" "${run[@]}" "$valid")" || { echo "lua54 fixture rejected:" >&2; echo "$out" >&2; exit 1; }
grep -q '"type":"Summary","errors":0,"warnings":0,"parse_errors":0' <<<"$out" \
  || { echo "lua54 fixture not clean:" >&2; echo "$out" >&2; exit 1; }

set +e
out="$("$selene" "${run[@]}" "$invalid")"
rc=$?
set -e
[[ $rc -eq 1 ]] || { echo "expected exit 1 for lint failure, got $rc" >&2; exit 1; }
grep -q '"code":"undefined_variable"' <<<"$out" \
  || { echo "undefined_variable not reported:" >&2; echo "$out" >&2; exit 1; }
