# CI Workflows

- `reusable-consumer.yml`: nine checks plus the `dx-ci (aggregate)` gate.
- `reusable-docs.yml`: `docs-check` plus `docs-publish` (GitHub Pages).
- `gate.yml`: the `gate` check (`dx check --affected`) on pull requests and `merge_group`, required on main by `.github/rulesets/main.json`. Its steps live in `.github/actions/gate`, which `ci.yml` also calls.
- `ci.yml`: this repository's own `ci` gate. A failed Windows preflight build uploads `rmeta-diagnostics-<platform>` (#4850).
- `issue-labels.yml`: daily warning for open issues older than one day, except pinned ones, that have no P0 to P3 label. Writes a job summary. Runs on `ubuntu-slim`.
  A second job warns about labels named in `CONTRIBUTING.md` or issue template front matter that do not exist in the repository (#6643).
- `evidence.yml`: native evidence on hosted runners, dispatched per run. The `evidence-<platform>` artifact holds `evidence.log` and each test's log, XML and undeclared outputs. Set `platforms` to `["macos_arm64"]` for Apple runs. Set `android` to boot an emulator and run the Rust Android device scenarios on `linux_x86_64`. Set `qualify` on `windows_x86_64` to run `//cc/context:qualify` for llvm and msvc.
- `evidence-command.yml`: a comment on an issue or pull request whose first line starts with `/evidence`, from an owner, member or collaborator, dispatches `evidence.yml` and replies with the run URL. On a pull request it runs on the PR branch, otherwise on `main`. Arguments are `platforms=`, `task=`, `scope=`, `test_timeout=`, `displays=`, `android=` and `qualify=`, for example `/evidence platforms=["macos_arm64"] task=test scope=//haskell/...`. `evidence.yml` validates them and a rejected run fails there.
- `dx-update.yml`: weekly `tools/dx update --pr` for dependency updates. `dx init` writes the same file.
- `verify-pins.yml`: weekly `//quality/artifacts:update -- --verify-only`. It fails naming each stale record.
- `ghcr.yml`: prebuilt devcontainer images.
- `release-cells.yml`: reusable, read-only. Builds, independently rebuilds and qualifies one archive per `//deploy/release:release_matrix` cell on its native runner (a consumer Bazel build included), then verifies all records. Artifacts: `release-cell-<name>` per cell and the combined `release-cells`; outputs: `version`, `sha`, `workflow_ref`. It fails when any cell is rejected, and a rejected cell never stops the others.
- `release.yml`: calls `release-cells.yml` for the five platform archives, builds the `rules_dx-<version>.tar.gz` source archive (after `release_driver` checks the tag against the candidate commit), then attests the manifest and uploads them through `release_driver` with the raw binaries, `.sha256` files, a `dx-sha256sums.txt` manifest, and `source.json` to the release. The `dry_run` dispatch input builds and qualifies everything and uploads nothing. No asset is ever replaced once uploaded: a rerun skips identical assets and fails on a different one.
- `release-consumer.yml`: runs when a release is published, or by dispatch with a `tag`. On the five release platforms it builds the quickstart consumer against the published source archive and checks that `@rules_dx//:dx` is the prebuilt download and prints the released version. Wait for it to be green before submitting to the BCR.
- `upgrade-e2e.yml`: when a release is published, or by dispatch with optional `from` and `to`. Runs `tools/ci/upgrade_e2e.sh`: a throwaway consumer on the second-latest BCR release runs `dx init`, upgrades to the latest and back with `dx upgrade --to <version> --apply`, and runs `bazel test //...` after each step. It needs two published releases on the BCR.

## Main health

`ci.yml` runs only on `workflow_dispatch`, a daily schedule (`17 3 * * *`, UTC) against the default branch head, and `merge_group`. There are no push or pull-request triggers; `gate.yml` runs `gate` on pull requests. Runs share one concurrency group per ref (`ci-ci-<ref>`, no cancel): a new run on main replaces only the queued one. The repository cache prune runs after scheduled or dispatched main runs. A red scheduled run opens or updates one issue titled `ci: scheduled run is failing` (the `scheduled-report` job). The next green scheduled run closes it. GitHub may delay or drop scheduled runs under load.

The platform table is `PLATFORMS` in `tools/ci/input_validation.rs`; `input_validation_test` pins every other copy to it.

Land a PR with `gh pr checks <n> --required --watch --fail-fast`, then `gh pr merge <n> --squash`, never `--admin`.

The consumer contract is [GitHub CI](../../docs/github-ci.md).
