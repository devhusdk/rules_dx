# Releasing rules_dx

Run these steps in order. Replace `X.Y.Z` with the version.

## 1. Bump the version

Set `X.Y.Z` by hand in these four places and merge to `main`:

- `MODULE.bazel`: `module(version = ...)`
- `cli/prebuilt/pins.bzl`: `DX_VERSION`
- `cli/adopt/src/version.rs`: `DX_VERSION`
- `cli/adopt/src/version.rs`: `MODULE_VERSION`

The `source` job in `release.yml` compares all four with the tag. A mismatch fails with `land the version bump first`.

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

The workflow builds every cell, writes `dx-sha256sums.txt`, attests it, and uploads the assets to the draft. It fails when:

- there is no release for the tag (go back to step 3)
- the release already holds a different file of the same name (delete that asset by hand)
- the release holds an asset outside the manifest, the source archive, its `.sha256` or `source.json` (delete that asset by hand)
- an archive does not verify against the attestation bundle

## 5. Fill the pins

```
bazel run //tools/ci:fill_pins_bin -- vX.Y.Z
```

This writes the sha256 values from the attested manifest into `cli/prebuilt/pins.bzl`. Commit the change and merge it to `main`. The release workflow runs `tools/ci` tests that fail if the pins and the assets disagree.

## 6. Publish

Check the draft on GitHub, then publish it. The upload job has already unpacked the Linux archive and run `dx --version` as a final check.

## 7. Submit to the Bazel Central Registry

Check the entry first:

```
BCR_DRY_RUN=1 bazel run //deploy/release:bcr_demo
```

It prints the pull request it would open and submits nothing. A failure names the field that is missing or does not match the module. Open the BCR pull request for `rules_dx` at `X.Y.Z` with the source archive and `source.json` from the release.
