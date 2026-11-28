# Releasing rules_dx

This page walks you through a release, from the version bump to the Bazel Central Registry (BCR), the public index that Bazel downloads modules from. Do the steps in order. Releases are `v0.0.x` for now. The examples use `0.0.1`; put your version in its place.

You need `gh` on `PATH`, logged in to `devhusdk/rules_dx` with rights to push tags, run workflows and publish releases.

## 1. Rehearse on main

A dry run builds and checks every release file without uploading anything. Run it on `main` before you bump the version:

```sh
gh workflow run release.yml --ref main -f dry_run=true -f approve=true
```

`approve=true` also checks line coverage in every cell, a cell being one platform's build of `dx`. Without it the run builds and reports only. The run summary says `published: **no**`. Fix anything it reports before you go on.

## 2. Bump the version

```sh
bazel run //deploy/private/release:bump_version -- 0.0.1
```

It sets the version in:

- `MODULE.bazel`: `module(version = ...)`
- `cli/prebuilt/pins.bzl`: `DX_VERSION`, and it empties `PINS`, which name the old version's assets
- `cli/adopt/src/version.rs`: `DX_VERSION` and `MODULE_VERSION`
- `cli/shim/src/protocol.rs`: `SHIM_VERSION`

It also points these pins at a commit of `main`, and refreshes `tools/ci/starter_pin.sha256`:

- `cli/adopt/src/scaffold.rs`: `CONSUMER_WORKFLOW_PIN`, the `reusable-consumer.yml` commit that `dx init` writes
- `examples/docs-ci/caller.yml`: the `reusable-docs.yml` commit
- `reusable-consumer.yml` and `reusable-docs.yml`: the `assert-no-secret` and `remote-cache-helper` action pins
 The commit is `origin/main` unless you pass `--workflow-ref <commit>`.

Open a pull request with the result and merge it to `main`.

## 3. Tag the release commit

Tag the merged bump commit and push the tag:

```sh
git fetch origin
git tag v0.0.1 <commit>
git push origin v0.0.1
```

The release workflow checks that `MODULE.bazel`, `cli/prebuilt/pins.bzl` and both constants in `cli/adopt/src/version.rs` say `0.0.1`. If one does not, it fails with `land the version bump first`.

The `source` job also fails when `MODULE.bazel` overrides a `bazel_dep`, such as a rule-set module like `rules_php`, with `local_path_override` or `git_override`. The error names the module. Depend on its released version in the BCR before you tag.

## 4. Create the draft release

```sh
tools/dx deploy //cli/cli:github_draft -- v0.0.1
GITHUB_PUBLISH_APPROVED=1 tools/dx deploy --apply //cli/cli:github_draft -- v0.0.1
```

The first command is a dry run: it stages the draft and prints what it would do. The second creates a draft release with the release notes and no files. The tag must already be on GitHub.

## 5. Build and upload the release files

```sh
gh workflow run release.yml --ref v0.0.1
```

Run it from the tag: the workflow only attests the files when it runs on the tag. It builds `dx` for every platform and uploads to the draft:

- the source archive `rules_dx-0.0.1.tar.gz`, its `.sha256`, its attestation `rules_dx-0.0.1.tar.gz.intoto.jsonl`, and `source.json`
- the `dx` archives, the `dx-launcher-<os>-<cpu>`, `dx-runtime-<os>-<cpu>`, `dx-<tool>-<os>-<cpu>` code generator (capnp, flatc, oapi-codegen, thrift), `dx-ldoc-main-<os>-<cpu>`, `dx-go-<os>-<cpu>` and `dx-gopackagesdriver-<os>-<cpu>` binaries, the Windows `dx-msvc-link-windows-<cpu>` linker wrappers, the windows-arm64 `dx-tree-sitter-windows-arm64.dll` and `dx-tree-sitter-pkl-windows-arm64.dll` libraries, the linux and macOS `dx-pcov-<os>-<cpu>.so` PHP extensions, the linux-x86_64 `dx-julia-internal-linux-x86_64.so` library, `dx-capnp-LICENSE`, `dx-flatc-LICENSE`, `dx-gopackagesdriver-LICENSE`, `dx-julia-internal-LICENSE`, `dx-ldoc-main-LICENSE`, `dx-oapi-codegen-LICENSE`, `dx-pcov-LICENSE`, `dx-thrift-LICENSE`, `dx-tree-sitter-LICENSE` and `dx-tree-sitter-pkl-LICENSE`, listed in `dx-sha256sums.txt`
- the `dx-gazelle-<os>-<cpu>` binaries, listed in `dx-gazelle-sha256sums.txt`
- the `dx-<tool>-any.jar` JVM tools (avro_union, dokka_extract, javac_check, javadoc_extract, jmh_generate, kotlinc_check, maven_resolver, scalac_check, scaladoc_extract), listed in `dx-jar-sha256sums.txt`
- the `dx-<tool>-<os>-<cpu>` Haskell tools (hpc_lcov, weeder) for linux x86_64, linux arm64, macOS arm64 and Windows x86_64, `dx-hlint-linux-arm64`, `dx-hlint-macos-arm64` and `dx-ormolu-linux-arm64`, with `dx-weeder-LICENSE`, `dx-hlint-LICENSE`, `dx-ormolu-LICENSE` and the statically linked GMP's `dx-haskell-gmp-LICENSE`, `dx-haskell-gmp-source.tar.xz`, `dx-haskell-gmp-source.patch` and GHC in-tree GMP source `dx-haskell-gmp-ghc-source.tar.xz`, listed in `dx-haskell-sha256sums.txt`
- the `dx-<tool>-<os>-<cpu>` quality and coverage tools (deadnix, selene, nixfmt, kcov) with their `dx-<tool>-LICENSE` files, `dx-deadnix-source.crate`, `dx-kcov-source.tar.gz` and the `dx-<python-tool>-<os>-<cpu>.tar.gz` archives of the core Python quality tools, listed in `dx-tool-sha256sums.txt`

The source archive's `cli/prebuilt/pins.bzl` has `PINS` filled in from the release manifests, one sha256 per tool and platform in `cli/prebuilt/tools.bzl`, so consumers download the prebuilt `dx`, gazelle, dx-go, JVM tool jars, Haskell tools and quality and coverage tools. The workflow also unpacks the Linux archive and runs `dx --version`, and uploads a `release-ledger` artifact that records the release.

It fails when:

- there is no draft for the tag: go back to step 4
- the tag does not point at the commit being released
- the draft already holds a different file with the same name: delete that file from the draft
- the draft holds a file the release does not list: delete that file from the draft
- a file does not match its attestation

After you fix the cause, run the workflow again with the same command.

When the files are uploaded, the `pins` job fills `PINS` in `main`'s `cli/prebuilt/pins.bzl` from the attested manifests and pushes it to the `release/pins-v0.0.1` branch. With the `DX_UPDATE_APP_ID` and `DX_UPDATE_APP_PRIVATE_KEY` secrets it opens the pull request as that App, so CI runs on it. Otherwise the run summary prints the command. Open it and merge it:

```sh
gh pr create --base main --head release/pins-v0.0.1 --fill
```

It skips this when `main` is no longer at `0.0.1`. `rules_dx`'s own builds ignore `PINS` and build every tool from source.

## 6. Publish

Open the draft on GitHub, check the notes and files, and publish it.

Publishing starts `release-consumer.yml`. On each platform it downloads the published assets, checks the platform archive and the source archive with `gh attestation verify` against the published bundles, and builds a small consumer project whose `bazel_dep` resolves `rules_dx` through a registry: the BCR once it lists the version, otherwise a local registry made from the published `source.json` and the archive's `MODULE.bazel`. It checks that `@rules_dx//dx/launch:dx` is the prebuilt download and prints `0.0.1`, that topiary and progenitor are the prebuilt downloads, and on `windows-arm64` that the `pkl_lsp` natives jar packs the prebuilt tree-sitter DLLs. It skips releases whose tag does not start with `v`, such as `mirror-php-<version>`. To run it again:

```sh
gh workflow run release-consumer.yml -f tag=v0.0.1
```

Wait until every `consumer` job is green.

## 7. Submit to the Bazel Central Registry

Once every `consumer` job is green, the `bcr` job in `release-consumer.yml` opens a draft pull request on the BCR that adds `rules_dx` at the released version. It reads the module root from `.bcr/config.yml`. The `bcr` job skips `v0.0.*` tags, so a `v0.0.x` release ends at step 6 and its consumers resolve `rules_dx` from the release's `source.json`. The `bcr-ledger` job then records the pull request URL, `failed`, or `not-authorized` while `BCR_REGISTRY_FORK` is unset, in the run summary and the `bcr-ledger` artifact. The `bcr` job is the only way `rules_dx` is submitted to the BCR.

Before the first release, the owner sets up two things once:

- the repository variable `BCR_REGISTRY_FORK`, the owner's fork of the registry, for example `owner/bazel-central-registry`. While it is unset the `bcr` job is skipped.
- the repository secret `BCR_PUBLISH_TOKEN`, a classic personal access token with the `repo` and `workflow` scopes that can push to that fork.

The owner reviews the BCR pull request, waits for BCR CI, and marks it ready for review.

After the BCR pull request merges, run the consumer again. Each `consumer` job then resolves `rules_dx` from the BCR alone:

```sh
gh workflow run release-consumer.yml -f tag=v0.0.1
```

If the `bcr` job fails, fix the cause and rerun the failed jobs:

```sh
gh run rerun <run-id> --failed
```

To check the entry shape without submitting:

```sh
BCR_DRY_RUN=1 bazel run //deploy/private/release:bcr_demo
```

It submits nothing. A failure names the field that is missing or does not match the module. Without `BCR_DRY_RUN=1` it exits with an error naming the `bcr` job.

## Release order with the rule-set modules

PHP, Lua and Qt rules ship as their own modules: `rules_php`, `rules_lua` and `rules_qt`. Each one depends on `rules_dx`, and `rules_dx` depends on each one. Release them in this order:

1. Release `rules_dx` with the steps above and wait until its BCR pull request is merged. A `v0.0.x` release has no BCR pull request, so the rule sets wait for the first release the BCR lists.
2. In the rule-set repository, set its `bazel_dep` on `rules_dx` to that release, then release the rule set to the BCR from its own `.bcr/` metadata. Its BCR presubmit builds the plain-Bazel fixture, with no `dx` CLI.
3. Once the rule set is in the BCR, the `dx-update.yml` run that `audit.yml` dispatches each Monday opens the floor-raise pull request on `dx-update/bazel--<rule set>`, such as `dx-update/bazel--rules_php`. It never auto-merges. To open it now, run the `dx-update` workflow by hand. Push changes to `<lang>/` on that branch for any rule-set API change, then merge it before the next `rules_dx` release.

The `upgrade-e2e` job of `audit.yml` fails when a rule set does not resolve to the floor of the `rules_dx` release, or when a rule set pulls `rules_dx` above that release.

Versions are `0.x` semver. A minor release may change the API before `1.0`.

While you work across both repositories you may point `rules_dx` at a local checkout of a rule set with `local_path_override`, or at an unreleased commit with `git_override`. Remove it before you tag: the `source` job fails and names the module.

`rules_dx` users write only `bazel_dep(name = "rules_dx", ...)` and load `@rules_dx//<lang>:defs.bzl`. `dx upgrade` moves them to new rule-set releases through `rules_dx`.
