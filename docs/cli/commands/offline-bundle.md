# Offline Bundle

`offline_bundle` assembles a vendored bundle for an airgapped host. The
`bootstrap_offline` tool reads that bundle, installs Bazel from it, and
populates the advisory snapshots under `.dx/advisory`. It is not a `dx`
command; the rest of `dx` runs through `@rules_dx//:dx`:

```sh
bazel run @rules_dx//:dx -- version
```

## The Macro

```text
load("@rules_dx//deploy/offline:offline.bzl", "offline_bundle")

offline_bundle(
    name = "offline_demo",
    advisory = {"cargo": ":demo/cargo.json"},
    advisory_meta = {"cargo": ":demo/cargo.meta.json"},
    launchers = [":demo/bazelisk-linux-amd64"],
)
```

- `name`: the bundle target and the tree directory it produces.
- `advisory`: a dict of advisory set to snapshot label. Sets are `cargo`,
  `npm`, `maven`, `nuget`, `go`, `rubygems`. Another set fails at load time.
- `advisory_meta`: a dict of advisory set to the identity file `advisory_prep`
  wrote next to that snapshot (`<set>.meta.json`). It records the `retrieved_at`
  date and the snapshot `sha256`. Every set in `advisory` needs one, and no
  other set may appear; a mismatch fails at load time.
- `launchers`: Bazelisk launcher labels. Each file must be named for the host
  it serves: `bazelisk-darwin-amd64`, `bazelisk-darwin-arm64`,
  `bazelisk-linux-amd64`, `bazelisk-linux-arm64` or
  `bazelisk-windows-amd64.exe`. Another name fails at load time. Ship every
  host you install on.

## The Output Tree

Build the bundle and read it from `bazel-bin/<pkg>/<name>/`:

```text
bazel build //deploy/offline:offline_demo
```

```text
<name>/advisory/<set>.json
<name>/advisory/<set>.meta.json
<name>/advisory/SHA256SUMS
<name>/bazelisk/<launcher>
<name>/bazelisk/SHA256SUMS
```

Each `SHA256SUMS` lists the files next to it as `<sha256>  <file>`.

## Installing From A Bundle

```text
bootstrap_offline --bundle DIR [--install-dir DIR] [--workspace DIR]
```

- `--bundle DIR`: the bundle tree. Required.
- `--install-dir DIR`: where `bazel` (`bazel.exe` on Windows) is installed.
  Defaults to `$HOME/.local/bin`.
- `--workspace DIR`: the workspace that receives `.dx/advisory`. Defaults to
  the current directory.
- `-h`, `--help`: print the usage line on stdout and exit 0.

The tool checks every `SHA256SUMS`, installs the launcher for the host, copies
each snapshot to `.dx/advisory/<set>.json`, and writes
`.dx/advisory/<set>.meta.json` with the set, the `file://` URL of the snapshot
in the bundle, its sha256, the retrieval date and the workspace path. The URL
is percent-encoded, so any path the host allows is recorded faithfully.

The retrieval date is the `retrieved_at` the bundle records in
`advisory/<set>.meta.json`, not the day of the install. The manifest pins that
file, and its `sha256` must match the snapshot. An old bundle therefore keeps
its old date, and `dx audit` reports `advisory_refresh_failed` for it until
the bundle is rebuilt from a fresh snapshot.

It prints three `bootstrap-offline:` lines on stdout: the installed launcher
with its sha256, the number of snapshots populated, and a note that the first
Bazel module fetch still needs network once.

## Refusals And Exit Codes

- 0: success, or `--help`.
- 1: a refusal or a bad argument. The reason goes to stderr as
  `bootstrap-offline: <reason>`; a bad argument adds the usage line. Nothing
  goes to stdout.

Refusals include: a missing or absent `--bundle`, a bundle without
`bazelisk/` or `advisory/`, an unsupported host, a missing launcher for the
host, a manifest that names a missing file or no files, a checksum mismatch,
an advisory snapshot the manifest does not pin, and an unknown advisory set.
A snapshot with no `<set>.meta.json` next to it, an identity the manifest does
not pin, an identity with no valid `YYYY-MM-DD` `retrieved_at` or one after
today's date (check the host clock), an identity whose `sha256` is missing or
is not the snapshot's, and an identity with no snapshot are refused too.
