# CI Workflows

- `reusable-consumer.yml`: nine checks plus the `dx-ci (aggregate)` gate.
- `reusable-docs.yml`: `docs-check` plus `docs-publish` (GitHub Pages).
- `gate.yml`: the `gate` check (`dx check --affected`) on pull requests and `merge_group`, required on main by `.github/rulesets/main.json`. Its steps live in `.github/actions/gate`, which `ci.yml` also calls.
- `ci.yml`: this repository's own `ci` gate. A failed Windows preflight build uploads `rmeta-diagnostics-<platform>` (#4850).
- `issue-labels.yml`: daily warning for open issues older than one day that have no P0 to P3 label. Writes a job summary. Runs on `ubuntu-slim`.
- `evidence.yml`: native evidence on hosted runners, dispatched per run. The `evidence-<platform>` artifact holds `evidence.log` and each test's log, XML and undeclared outputs. Set `platforms` to `["macos_arm64"]` for Apple runs. Set `android` to boot an emulator and run the Rust Android device scenarios on `linux_x86_64`.
- `dx-update.yml`: weekly `tools/dx update --pr` for dependency updates. `dx init` writes the same file.
- `verify-pins.yml`: weekly `//quality/artifacts:update -- --verify-only`. It fails naming each stale record.
- `ghcr.yml`: prebuilt devcontainer images.
- `release-cells.yml`: reusable, read-only. Builds, independently rebuilds and qualifies one archive per `//deploy/release:release_matrix` cell on its native runner (a consumer Bazel build included), then verifies all records. Artifacts: `release-cell-<name>` per cell and the combined `release-cells`; outputs: `version`, `sha`, `workflow_ref`. It fails when any cell is rejected, and a rejected cell never stops the others.
- `release.yml`: calls `release-cells.yml` for the five platform archives, builds the `rules_dx-<version>.tar.gz` source archive (after `release_driver` checks the tag against the candidate commit), then attests the manifest and uploads them through `release_driver` with the raw binaries, `.sha256` files, a `dx-sha256sums.txt` manifest, and `source.json` to the release. The `dry_run` dispatch input builds and qualifies everything and uploads nothing. No asset is ever replaced once uploaded: a rerun skips identical assets and fails on a different one.

## Main health

`ci.yml` runs only on `workflow_dispatch`, a daily schedule (`17 3 * * *`, UTC) against the default branch head, and `merge_group`. There are no push or pull-request triggers; `gate.yml` runs `gate` on pull requests. Runs share one concurrency group per ref (`ci-ci-<ref>`, no cancel): a new run on main replaces only the queued one. The repository cache prune runs after scheduled or dispatched main runs. A red scheduled run opens or updates one issue titled `ci: scheduled run is failing` (the `scheduled-report` job). The next green scheduled run closes it. GitHub may delay or drop scheduled runs under load.

The platform table is `PLATFORMS` in `tools/ci/input_validation.rs`; `input_validation_test` pins every other copy to it.

Land a PR with `gh pr checks <n> --required --watch --fail-fast`, then `gh pr merge <n> --squash`, never `--admin`.

The consumer contract is [GitHub CI](../../docs/github-ci.md).
