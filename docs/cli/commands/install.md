# `dx install`

```text
dx install --tag <tag> --archive <path> [--dir DIR] [--apply]
```

Verifies a downloaded dx release archive and installs it under a per-user
directory. It does not use Bazel (so no `@rules_dx//:dx` launcher), does not need a
workspace, and never touches
a `.dx/version` pin. Use [`dx rollback`](upgrade-rollback.md) for pins; use this
page to put the `dx` binary itself on a machine.

`dx install` never downloads anything. You download the archive, the checksum
manifest and the provenance bundle with `gh`, then point `dx install` at the
archive. The three files must sit in one directory:

- `dx-<cell>.tar.gz` (Linux, macOS) or `dx-<cell>.zip` (Windows), where
  `<cell>` is `linux-x86_64`, `linux-arm64`, `macos-arm64`, `windows-x86_64` or
  `windows-arm64`. The archive must match the host.
- `dx-sha256sums.txt`, which also lists the raw `dx-<cell>` binaries
- `dx-sha256sums.txt.sigstore.json`, one attestation over every line of the
  manifest

Output: `--output text|json`. Exit codes: 0 on success or a no-op, 2 for a
usage error, 1 when a check refuses.

Releases that carry these assets are published by the release workflow, which
refuses a tag unless the committed dx version equals it. Until
a release lists them, no published tag passes `dx install`.

## Prerequisites

GitHub CLI (`gh`) with `gh attestation verify --bundle`. Set `DX_INSTALL_GH` to
use another `gh` binary. It selects only the binary; the trust identity is
fixed and has no flag or environment override.

## Flags

- `--tag <tag>`: the release tag the archive belongs to, `v` plus a semver
  version such as `v1.2.3`. Take it from `gh release view`, not from a binary
  you already run. Required.
- `--archive <path>`: the downloaded archive. Required. Relative paths resolve
  from the directory you run `dx install` in.
- `--dir <DIR>`: the install root. Relative paths resolve from the current
  directory. Default: `$XDG_DATA_HOME/dx`, else `~/.local/share/dx`; on Windows
  `%LOCALAPPDATA%\dx`.
- `--apply`: write the install. Without it `dx install` runs every check and
  changes nothing.
- `--output text|json`.

Scopes, `--check`, `--here` and `-- bazel-options` are usage errors.

## Checks

Checks run in this order and stop at the first failure:

1. `tag`: the tag has the `v<semver>` shape.
2. `cell`: the archive name is the one this host needs.
3. `sha256`: the archive hash is listed in `dx-sha256sums.txt`.
4. `provenance`: `gh attestation verify` accepts the archive against the bundle
   for this repository, the release workflow at `refs/tags/<tag>`, the GitHub
   Actions OIDC issuer, and a non-self-hosted runner.
5. `extract` and `version`: the archive unpacks to a staging directory and the
   staged `dx --version` prints `dx <version>` for the tag.

## Layout

```text
DIR/versions/<tag>/     the extracted release and RECEIPT.json
DIR/bin/                the entry point to the active version
```

Installing a tag that is already active is a no-op. Installing another tag
switches `DIR/bin` and keeps the current and previous versions; older ones are
removed. A tag directory holding different content is never overwritten. Put
`DIR/bin` on your `PATH`; `dx install` prints a hint when it is not there but
never edits shell rc files.

## Output and exit codes

Text prints one `ok <check>: <detail>` line per passed check, then
`installed:` (or `would install` without `--apply`) with the outcome (fresh,
upgrade from X, switch from X, or no change), then `bin: <DIR>/bin`. A failure
prints `fail <check>: <message>`.

JSON prints `command_started`, one `status_event` per check (`ok` or `error`),
then `command_finished`. A refusal adds an `error` event.

Exit codes: 0 for success or a no-op; 1 when a check refuses; 2 for a usage
error; 141 for a closed stdout pipe.

## First install

Pick the tag from the release, download the three assets for your cell, verify
them with the same identity `dx install` pins, extract, and let the extracted
`dx` install itself. Shown for Linux x86_64.

```bash
tag=$(gh release view -R devhusdk/rules_dx --json tagName -q .tagName)
cell=dx-linux-x86_64
mkdir dx-download && cd dx-download
gh release download "$tag" -R devhusdk/rules_dx \
  -p "$cell.tar.gz" -p dx-sha256sums.txt -p dx-sha256sums.txt.sigstore.json
gh attestation verify "$cell.tar.gz" --bundle dx-sha256sums.txt.sigstore.json \
  --repo devhusdk/rules_dx \
  --cert-identity "https://github.com/devhusdk/rules_dx/.github/workflows/release.yml@refs/tags/$tag" \
  --cert-oidc-issuer https://token.actions.githubusercontent.com \
  --source-ref "refs/tags/$tag" --deny-self-hosted-runners
mkdir extracted && tar -xzf "$cell.tar.gz" -C extracted
./extracted/dx install --tag "$tag" --archive "$PWD/$cell.tar.gz" --apply
```

PowerShell on Windows x86_64:

```powershell
$tag = gh release view -R devhusdk/rules_dx --json tagName -q .tagName
$cell = "dx-windows-x86_64"
mkdir dx-download; cd dx-download
gh release download $tag -R devhusdk/rules_dx -p "$cell.zip" -p dx-sha256sums.txt -p dx-sha256sums.txt.sigstore.json
gh attestation verify "$cell.zip" --bundle dx-sha256sums.txt.sigstore.json `
  --repo devhusdk/rules_dx `
  --cert-identity "https://github.com/devhusdk/rules_dx/.github/workflows/release.yml@refs/tags/$tag" `
  --cert-oidc-issuer https://token.actions.githubusercontent.com `
  --source-ref "refs/tags/$tag" --deny-self-hosted-runners
Expand-Archive "$cell.zip" -DestinationPath extracted
.\extracted\dx.exe install --tag $tag --archive "$PWD\$cell.zip" --apply
```

`dx install` runs the checks again, so the manual `gh attestation verify` step
only guards the extracted binary you run first.

## Upgrade

Download the new release's assets as above, then run the installed `dx`:

```bash
dx install --tag "$tag" --archive "$PWD/$cell.tar.gz" --apply
```

Run it without `--apply` first to see the checks and the outcome.
