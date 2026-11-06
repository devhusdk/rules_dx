#!/usr/bin/env bash
# Builds the dx release archive and the pure-Go dx gazelle twice and qualifies them as one release cell.
# usage: release_cell.sh OUT
# Reads CELL, WORKFLOW_REF, WORKFLOW_SHA, RUN_ID, JOB and SHA, plus RUNNER_TEMP and GITHUB_WORKSPACE.
# Run by release-cells.yml and by evidence.yml with release_cell=true.
# The variables come from the workflow env; the body is kept as release-cells.yml ran it.
# shellcheck disable=SC2154,SC2292
set -euo pipefail
if [ -z "$WORKFLOW_REF" ] || [ -z "$WORKFLOW_SHA" ]; then
  echo "job.workflow_ref or job.workflow_sha is empty" >&2
  exit 1
fi
out="$1"
rm -rf "$out"
mkdir -p "$out"
bazel build --config=dx_build_release //cli/cli:dx //deploy/release:release_cell //deploy/release:release_matrix //deploy/release:dx_host_cell //deploy/release:dx_host_cell_sbom
symbols=""
case "$CELL" in dx-windows-*) symbols="$CELL-pdb.zip" ;; esac
if [ -n "$symbols" ]; then
  bazel build --config=dx_build_release //deploy/release:dx_host_cell_pdb //deploy/release:dx_host_cell_pdb_sbom
fi
gazelle_flags=(--config=dx_build_release --@rules_go//go/config:pure)
bazel build "${gazelle_flags[@]}" @dx_gazelle//:gazelle
root="$(bazel info --config=dx_build_release execution_root)"
gazelle_asset="dx-gazelle-${CELL#dx-}"
cp "$root/$(bazel cquery "${gazelle_flags[@]}" --output=files @dx_gazelle//:gazelle)" "$out/$gazelle_asset"
dx="$root/$(bazel cquery --config=dx_build_release --output=files //cli/cli:dx)"
tool="$root/$(bazel cquery --config=dx_build_release --output=files //deploy/release:release_cell)"
matrix="$root/$(bazel cquery --config=dx_build_release --output=files //deploy/release:release_matrix)"
built="$root/$(bazel cquery --config=dx_build_release --output=files //deploy/release:dx_host_cell)"
version="$("$tool" version --exe "$dx")"
archive_name="$(basename "$built")"
cp "$built" "$out/$archive_name"
sbom="$root/$(bazel cquery --config=dx_build_release --output=files //deploy/release:dx_host_cell_sbom)"
cp "$sbom" "$out/$archive_name.spdx.json"
if command -v sha256sum >/dev/null 2>&1; then
  digest="$(cd "$out" && sha256sum -- "$archive_name" | cut -d' ' -f1)"
  digest="${digest#\\}"
else
  digest="$(cd "$out" && shasum -a 256 -- "$archive_name" | cut -d' ' -f1)"
fi
printf '%s  %s\n' "$digest" "$archive_name" > "$out/$archive_name.sha256"
if [ -n "$symbols" ]; then
  built_pdb="$root/$(bazel cquery --config=dx_build_release --output=files //deploy/release:dx_host_cell_pdb)"
  cp "$built_pdb" "$out/$symbols"
  cp "$root/$(bazel cquery --config=dx_build_release --output=files //deploy/release:dx_host_cell_pdb_sbom)" "$out/$symbols.spdx.json"
  printf '%s  %s\n' "$(sha256sum -- "$out/$symbols" | cut -d' ' -f1)" "$symbols" > "$out/$symbols.sha256"
fi
# The determinism input is a second, independent build: its own output base, no caches.
rebuild_base="$RUNNER_TEMP/rebuild-base"
rm -rf "$rebuild_base"
bazel --output_base="$rebuild_base" build --config=dx_build_release --disk_cache= --remote_cache= //deploy/release:dx_host_cell ${symbols:+//deploy/release:dx_host_cell_pdb}
rebuild_root="$(bazel --output_base="$rebuild_base" info --config=dx_build_release execution_root)"
rebuilt="$rebuild_root/$(bazel --output_base="$rebuild_base" cquery --config=dx_build_release --output=files //deploy/release:dx_host_cell)"
cp "$rebuilt" "$RUNNER_TEMP/rebuilt-$archive_name"
symbols_args=()
if [ -n "$symbols" ]; then
  rebuilt_pdb="$rebuild_root/$(bazel --output_base="$rebuild_base" cquery --config=dx_build_release --output=files //deploy/release:dx_host_cell_pdb)"
  cp "$rebuilt_pdb" "$RUNNER_TEMP/rebuilt-$symbols"
  symbols_args=(--symbols "$out/$symbols" --symbols-rebuild "$RUNNER_TEMP/rebuilt-$symbols")
fi
bazel --output_base="$rebuild_base" build "${gazelle_flags[@]}" --disk_cache= --remote_cache= @dx_gazelle//:gazelle
rebuilt_gazelle="$rebuild_root/$(bazel --output_base="$rebuild_base" cquery "${gazelle_flags[@]}" --output=files @dx_gazelle//:gazelle)"
if ! cmp "$out/$gazelle_asset" "$rebuilt_gazelle"; then
  echo "$gazelle_asset differs between two builds" >&2
  exit 1
fi
bazel --output_base="$rebuild_base" shutdown
status=0
"$tool" qualify \
  --matrix "$matrix" \
  --cell "$CELL" \
  --archive "$out/$archive_name" \
  --rebuild "$RUNNER_TEMP/rebuilt-$archive_name" \
  --work "$RUNNER_TEMP/qualify-work" \
  --sha "$SHA" \
  --version "$version" \
  --workflow-ref "$WORKFLOW_REF" \
  --workflow-sha "$WORKFLOW_SHA" \
  --run-id "$RUN_ID" \
  --job "$JOB" \
  --consumer "$GITHUB_WORKSPACE/deploy/release/consumer" \
  --checkout "$GITHUB_WORKSPACE" \
  --out "$out/$CELL.json" \
  ${symbols_args[@]+"${symbols_args[@]}"} || status=$?
exit "$status"
