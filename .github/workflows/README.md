# CI Workflows

- `reusable-consumer.yml`: nine checks plus the `dx-ci (aggregate)` gate.
- `reusable-docs.yml`: `docs-check` plus `docs-publish` (GitHub Pages).
- `ci.yml`: this repository's own `ci` gate and the `gate` check (`dx check --affected`), required on main by `.github/rulesets/main.json`.
- `issue-labels.yml`: daily warning for open issues older than one day that have no P0 to P3 label. Writes a job summary. Runs on `ubuntu-slim`.
- `evidence.yml`: native evidence on hosted runners, dispatched per run. The `evidence-<platform>` artifact holds `evidence.log` and each test's log, XML and undeclared outputs.
- `android.yml`: runs the Rust Android APK on a hosted x86_64 emulator, dispatched per run.
- `apple.yml`: Apple toolchain tests on a hosted macOS runner.
- `bump.yml`: `widen-one-requirement loop` for one dependency per run.
- `ghcr.yml`: prebuilt devcontainer images.
- `release-cells.yml`: reusable, read-only. Builds, independently rebuilds and qualifies one archive per `//deploy/release:release_matrix` cell on its native runner (a consumer Bazel build included), then verifies all records. Artifacts: `release-cell-<name>` per cell and the combined `release-cells`; outputs: `version`, `sha`, `workflow_ref`. It fails when any cell is rejected, and a rejected cell never stops the others.
- `release.yml`: calls `release-cells.yml` for the five platform archives, builds the `rules_dx-<version>.tar.gz` source archive (after `release_driver` checks the tag against the candidate commit), then uploads them with the raw binaries, `.sha256` files, a `dx-sha256sums.txt` manifest, and `source.json` to the release. The `dry_run` dispatch input builds and qualifies everything and uploads nothing. No asset is ever replaced once uploaded: a rerun skips identical assets and fails on a different one.

## Main health

`ci.yml` also runs on a schedule (`17 3,15 * * *`, UTC) against the default branch head, so main always has a recent full result. Main pushes share one concurrency group (`ci-ci-main`, no cancel): one run keeps running and the newest push replaces only the waiting one. Scheduled runs use per-SHA groups, so a push never replaces them, and publishing stays push-only. A red scheduled run is a signal, not an outage; route its causes to #1371. GitHub may delay or drop scheduled runs under load.

The platform table is `PLATFORMS` in `tools/ci/input_validation.py`; `input_validation_test` pins every other copy to it.

Land a PR with `gh pr checks <n> --required --watch --fail-fast`, then `gh pr merge <n> --squash`, never `--admin`.

The consumer contract is [GitHub CI](../../docs/github-ci.md).
