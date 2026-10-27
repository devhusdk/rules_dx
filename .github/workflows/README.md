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

The consumer contract is [GitHub CI](../../docs/github-ci.md).
