# CI Workflows

- `reusable-consumer.yml`: nine checks plus the `dx-ci (aggregate)` gate.
- `reusable-docs.yml`: `docs-check` plus `docs-publish` (GitHub Pages).
- `ci.yml`: this repository's own `ci` gate.
- `cancel-stale.yml`: cancels a closed PR's ci run through its concurrency group, then sweeps runs of closed PRs and main runs replaced by a newer commit while they still have no jobs. Runs on `ubuntu-slim`.
- `evidence.yml`: native evidence on hosted runners, dispatched per run. The `evidence-<platform>` artifact holds `evidence.log` and each test's log, XML and undeclared outputs.
- `android.yml`: runs the Rust Android APK on a hosted x86_64 emulator, dispatched per run.
- `apple.yml`: Apple toolchain tests on a hosted macOS runner.
- `bump.yml`: `widen-one-requirement loop` for one dependency per run.
- `ghcr.yml`: prebuilt devcontainer images.
- `publish-dry-run.yml`: the release path without a release.
- `release.yml`: builds dx for five platforms and the `rules_dx-<version>.tar.gz` source archive (after `release_driver` checks the tag against the candidate commit), then uploads them with `.sha256` files, a `dx-sha256sums.txt` manifest of the five binaries, and `source.json` to the release. No asset is ever replaced once uploaded: a rerun skips identical assets and fails on a different one.

## Qualifying a SHA

Main pushes are cancelled by `cancel-stale.yml` as newer commits land, so a main run may never finish. To get a full `ci` run at one fixed SHA:

1. Push the SHA to a `qualify/<sha>` branch, then dispatch `ci` on that branch (`gh workflow run ci.yml --ref qualify/<sha>`). `ci.yml` takes no dispatch inputs, so the matrix is the required one. It uses `--config=ci`, like a main push, and writes to the remote cache.
2. Record one triage table row per cell: cell, job URL, runner image and versions (the `Record the runner image` step), first root-cause category, owning issue. List runner outages separately.
3. Do not rerun unchanged deterministic failures.
4. Red-aggregate drill: on a throwaway branch make one required job fail and confirm `ci` is red (`ci_gate_fixtures` prove this statically).
5. Also run a fork PR to confirm no secrets are needed.

The platform table is `PLATFORMS` in `tools/ci/input_validation.py`; `input_validation_test` pins every other copy to it.

The consumer contract is [GitHub CI](../../docs/github-ci.md).
