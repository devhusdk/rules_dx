#!/usr/bin/env bash
# Runs the pinned opam against the pinned opam-repository snapshot as the only repository.
# `opam.sh snapshot-dir` prints the snapshot checkout instead.
set -euo pipefail

runfiles="${RUNFILES_DIR:-$0.runfiles}"
opam="$runfiles/$1"
repo_file="$runfiles/$2"
shift 2
snapshot="$(dirname "$(readlink -f "$repo_file")")"

if [ "${1:-}" = "snapshot-dir" ]; then
    echo "$snapshot"
    exit 0
fi

export OPAMROOT="${OPAMROOT:?set OPAMROOT to a directory outside the workspace}"
export OPAMYES=1
if [ ! -d "$OPAMROOT" ]; then
    "$opam" init --bare --no-setup --disable-sandboxing --kind=local "file://$snapshot" >/dev/null
    "$opam" switch create dx --empty >/dev/null
fi
exec "$opam" "$@"
