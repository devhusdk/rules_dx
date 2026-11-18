# `dx fetch`

Use `dx fetch` to download everything your targets need while you are online, so you can build and test later without a network.

```sh
dx fetch //...
dx fetch --affected
dx fetch --vendor //...
```

A *target* is one thing Bazel can build or test. A *repository* is a dependency Bazel downloads, such as a toolchain or a crate. Bazel keeps downloaded repositories in its *repository cache*.

`dx fetch` runs `bazel fetch` with the same `.bazelrc`, cache settings and build mode as `dx build`. The repositories land in the repository cache. Afterwards, `dx build` and `dx test` with their [offline flag](build-test-coverage.md#flags) use only that cache, with no network.

After Bazel succeeds, `dx fetch` also puts the prebuilt dx binary that `tools/dx` runs into the dx cache: `$DX_CACHE_DIR/bin/<version>/dx-<os>-<arch>` (`.exe` on Windows). Without `DX_CACHE_DIR` the cache is `$XDG_CACHE_HOME/dx`, else `~/.cache/dx`. dx copies the running binary there only when its sha256 matches the pin in `.dx/pins`. When the file is already there, nothing changes.

## Usage

```text
dx fetch [--vendor[=DIR]] [--here] [--affected [--since <ref>|--base <ref>]] [--debug|--release] [scope ...] [-- bazel-options ...]
```

## Flags

| Flag | What it does |
| --- | --- |
| `--vendor[=DIR]` | Run `bazel vendor` into `DIR` instead of `bazel fetch`. Default `vendor_src`. |
| `--here` | Use the current directory tree. |
| `--affected` | Use the targets your git changes reach. |
| `--since <ref>` | With `--affected`: diff from this ref instead of the merge-base. Without `--affected`, exit `2`. |
| `--base <ref>` | With `--affected`: take the merge-base with this branch instead of the default branch. Not with `--since`. |
| `--debug` | Fetch for the `dx_build_debug` build mode. |
| `--release` | Fetch for the `dx_build_release` build mode. No flag uses `dx_build_dev`. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--build-slots-wait`, `--bazel-startup-option` | Global flags. |

Fetch with the build mode you build with. A repository only a release build needs is not fetched by `dx fetch`.

To build from a vendor directory, add `common --vendor_dir=vendor_src` to `.bazelrc`.

## Scopes

No scope means `//...`. Pass labels, patterns, files, or directories. A file fetches for its owning targets.

`--here` does not combine with scopes or `--affected`. With scopes, `--affected` keeps only the affected targets inside them. See [Scope defaults](scope-defaults.md).

Args after `--` go to Bazel. Bazel startup options after `--` exit `2`.

## Offline builds

The offline flag of `dx build` and `dx test` passes `--repository_disable_download` to Bazel and to the queries that resolve the scope. A repository missing from the cache then fails the build instead of downloading.

## Output

Text prints Bazel's output, then one line about the dx binary on stderr:

- `dx: cached dx <version> at <path>`
- `dx: dx <version> is already cached at <path>`
- `dx: no prebuilt dx (<asset>) is pinned in MODULE.bazel and .dx/pins; tools/dx runs dx through Bazel`
- `dx: warning: dx_not_cached: <reason>` when the binary could not be cached.

JSON emits a `notice` with code `dx_cached` (severity `info`) or `dx_not_cached` (`info` when no prebuilt dx applies, `warning` otherwise).

A running dx that is not the pinned binary, such as a local build, is never copied. The warning then ends with `next: run tools/dx while online so it downloads the pinned dx`.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Operational failure, or the dx binary could not be written to the cache. |
| `2` | Usage, scope, or owner error. |
| Bazel's code | Bazel failed. |
