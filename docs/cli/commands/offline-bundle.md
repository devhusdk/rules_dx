# Offline bundle

Use an offline bundle to set up Bazel and the security advisory data on a host with no network access.

The `offline_bundle` macro builds the bundle on a connected machine. A macro is a Bazel function you call in a `BUILD.bazel` file to declare targets. The `bootstrap_offline` tool installs Bazel from the bundle on the offline host and fills `.dx/local/advisory`. It is not a `dx` command.

```sh
bazel build //:offline_demo
bazel run @rules_dx//deploy:bootstrap_offline -- --bundle bazel-bin/offline_demo
bazel run @rules_dx//deploy:bootstrap_offline -- --bundle DIR --install-dir ~/bin --workspace .
```

## Macro

```text
load("@rules_dx//deploy:defs.bzl", "offline_bundle")

offline_bundle(
    name = "offline_demo",
    advisory = {"cargo": ":demo/cargo.json"},
    advisory_meta = {"cargo": ":demo/cargo.meta.json"},
    launchers = [":demo/bazelisk-linux-amd64"],
)
```

| Attribute | What it does |
| --- | --- |
| `name` | Bundle target and tree directory. |
| `advisory` | Dict of advisory set to snapshot label. Sets: `cargo`, `npm`, `maven`, `nuget`, `go`, `rubygems`. Another set fails at load time. |
| `advisory_meta` | Dict of advisory set to the `<set>.meta.json` written by `advisory_prep`. Holds `retrieved_at` and the snapshot `sha256`. Needs exactly the sets in `advisory`. |
| `launchers` | Bazelisk labels, named `bazelisk-darwin-amd64`, `bazelisk-darwin-arm64`, `bazelisk-linux-amd64`, `bazelisk-linux-arm64`, `bazelisk-windows-amd64.exe` or `bazelisk-windows-arm64.exe`. Ship every host you install on. |

## Output tree

Read the bundle from `bazel-bin/<pkg>/<name>/`:

```text
<name>/advisory/<set>.json
<name>/advisory/<set>.meta.json
<name>/advisory/SHA256SUMS
<name>/bazelisk/<launcher>
<name>/bazelisk/SHA256SUMS
```

Each `SHA256SUMS` lists `<sha256>  <file>`.

## `bootstrap_offline`

```text
bootstrap_offline --bundle DIR [--install-dir DIR] [--workspace DIR]
```

| Flag | What it does |
| --- | --- |
| `--bundle DIR` | Bundle tree. Required. |
| `--install-dir DIR` | Where `bazel` (`bazel.exe` on Windows) is installed. Default `$HOME/.local/bin`, or `%USERPROFILE%\.local\bin`. Required when neither variable is set. |
| `--workspace DIR` | Workspace that receives `.dx/local/advisory`. Default the current directory. |
| `-h`, `--help` | Prints the usage line and exits `0`. |

It checks every `SHA256SUMS`, installs the launcher for the host, copies each snapshot to `.dx/local/advisory/<set>.json`, and writes `.dx/local/advisory/<set>.meta.json` (set, `file://` URL, sha256, retrieval date, `path`).

The retrieval date is the `retrieved_at` from the bundle, not the install day. `dx security` and `dx license` report `advisory_refresh_failed` for an old bundle until you rebuild it from a fresh snapshot.

It prints three `bootstrap-offline:` lines on stdout: the installed launcher with its sha256, the number of snapshots, and a note that the first Bazel module fetch still needs the network. Run [`dx fetch`](fetch.md) once while online; after that `dx build --offline` and `dx test --offline` need no network.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success, or `--help`. |
| `1` | Refusal or bad argument. stderr has `bootstrap-offline: <reason>`; nothing goes to stdout. |

Refusals:

- Missing `--bundle`, or a bundle without `bazelisk/` or `advisory/`.
- Unsupported host, or no launcher for the host.
- A manifest that names a missing file or no files, or a checksum mismatch.
- A snapshot or identity the manifest does not pin, or an unknown advisory set.
- A snapshot with no `<set>.meta.json`, or an identity with no snapshot.
- An identity with no valid `YYYY-MM-DD` `retrieved_at`, a date after today, or a `sha256` that is missing or not the snapshot's.
