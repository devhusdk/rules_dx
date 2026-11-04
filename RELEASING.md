# Releasing rules_dx

Run these steps in order. Replace `X.Y.Z` with the version.

## 1. Bump the version

Set `X.Y.Z` by hand in these places and merge to `main`. `bazel run //deploy/release:bump_version -- X.Y.Z` edits them all:

- `MODULE.bazel`: `module(version = ...)`
- `cli/prebuilt/pins.bzl`: `DX_VERSION`
- `cli/adopt/src/version.rs`: `DX_VERSION`
- `cli/adopt/src/version.rs`: `MODULE_VERSION`
- each `families/<f>/MODULE.bazel`: `module(version = ...)` and the `bazel_dep` on `rules_dx`

The `source` job in `release.yml` compares the first four with the tag, and the source archive step compares every family module with it. A mismatch fails with `land the version bump first`.

## 2. Tag

```
git tag vX.Y.Z <commit>
git push origin vX.Y.Z
```

The tag must name the commit you want to release. The `source` job fails if it does not.

## 3. Create the draft

```
GH_RELEASE_LIVE=1 GH_RELEASE_APPROVED=1 bazel run //cli/cli:github_draft -- vX.Y.Z
```

This creates a draft with release notes only. It carries no binaries. It needs `gh` on `PATH` and logged in, and the tag on GitHub. Without both variables it stages a dry run and publishes nothing.

## 4. Run the release workflow

```
gh workflow run release.yml --ref vX.Y.Z
```

The release has one source archive, `rules_dx-X.Y.Z.tar.gz`, and one source file per module: `source.json` for `rules_dx` and `rules_dx_<family>.source.json` for each module under `families/`. A family's `strip_prefix` is `rules_dx-X.Y.Z/families/<f>`, so every module points at the same archive.

The workflow builds every cell, writes `dx-sha256sums.txt`, fills the sha256 values into the `cli/prebuilt/pins.bzl` of the release source archive, attests the manifest, and uploads the assets to the draft through `release_driver`. `main` keeps the pins empty. It fails when:

- there is no release for the tag (go back to step 3)
- the release already holds a different file of the same name (delete that asset by hand)
- the release holds an asset outside the manifest, the source archive, its `.sha256`, `source.json` or a `rules_dx_<family>.source.json` (delete that asset by hand)
- an archive does not verify against the attestation bundle

## 5. Publish

Check the draft on GitHub, then publish it. The assets job has already unpacked the Linux archive and run `dx --version` as a final check.

Publishing starts `release-consumer.yml`. It builds the quickstart consumer against the published source archive on all five platforms and checks that `@rules_dx//:dx` is the prebuilt download and prints the released version. Do not submit to the BCR (step 6) until it is green. To rerun it, dispatch it with the tag.

## 6. Submit to the Bazel Central Registry

Check the entry first:

```
BCR_DRY_RUN=1 bazel run //deploy/release:bcr_demo
```

It prints the pull request it would open and submits nothing. A failure names the field that is missing or does not match the module. Run it for `rules_dx` and for each `rules_dx_<family>`. Open one BCR pull request that adds every module at `X.Y.Z`, because each family depends on `rules_dx` at the same version. Each module takes its own source file from the release, and each family also needs its `.bcr/families/<f>/` templates.
