#!/usr/bin/env bash
# Appends one JSON line of compiler evidence per compiler row of the support matrix facts.
# usage: compiler_evidence.sh OUT PLATFORM FACTS
# Run after the tests, with DX_PRODUCERS set to producers_bin. tools/ci/producers/src/evidence.rs writes the lines.
set -euo pipefail

export DX_BAZEL_VERSION="${DX_BAZEL_VERSION:-$(bazel --version | tr -d '\r')}"
exec "$DX_PRODUCERS" --evidence "$@"
