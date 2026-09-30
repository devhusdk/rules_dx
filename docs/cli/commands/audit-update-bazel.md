# Audit, Update, And Bazel Commands

## `dx bazel`

```text
dx bazel <bazel arguments...>
```

Runs Bazel directly through the repo launcher. Args after `bazel` go to Bazel
unchanged, even tokens that look like `dx` flags. Put `dx` flags before
`bazel`. The exit code is Bazel's own, except a launch failure or a signal
is `1`.

Output: `--output text`.

```sh
bazel run //cli/cli:dx -- bazel build //...
bazel run //cli/cli:dx -- --dry-run bazel query //...
```

## `dx security` And `dx license`

```text
dx security [--here] [--offline|--frozen] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
dx license [--here] [--offline|--frozen] [--fail-on info|warning|error] [--report sarif=<dest>|spdx=<dest>] [scope...]
```

`dx security` checks secrets plus dependency vulnerabilities. `dx license`
checks dependency licenses. No scope means `//...`. Use `--here` for the
current dir tree. Neither command changes files. `--check` and Bazel options
do not apply.

Output: `--output text|json`. Reports: `dx security` writes
`--report sarif=<dest>`. `dx license` writes `--report sarif=<dest>` or
`--report spdx=<dest>`. Repeat the flag for more files. Use `-` for stdout.

```sh
bazel run //cli/cli:dx -- security //...
bazel run //cli/cli:dx -- license --fail-on error //...
```

`--offline` and `--frozen` run cache-only with no network fetches. Exit
codes: 0 success, 2 usage or scope errors, 1 operational failures.

## `dx update`

```text
dx update [--check] [--offline|--frozen] [set...]
```

Updates dependencies per set through the qualified resolvers. No selector
updates all sets. Sets: `cargo`, `go`, `maven`, `npm`, `npm-adopt`,
`npm-adopt-polyglot`, `npm-tools`, `nuget`, `uv`, `uv-adopt`,
`uv-adopt-polyglot`, `uv-tools`. Selectors are `set`, `set:package`, or a
label/path. `go` is a pinned no-op success. `--check` fails if the preset is
stale and ignores selectors. `--offline` and `--frozen` run cache-only with
no network fetches. `--fail-on`, `--report`, and Bazel options do not apply.
Output: `--output text|json`. `json` reports per set plus a summary count.
Exit codes: 0 success, 2 usage or scope errors, 1 operational failures.

```sh
bazel run //cli/cli:dx -- update --check
bazel run //cli/cli:dx -- update go
bazel run //cli/cli:dx -- update uv uv-tools
bazel run //cli/cli:dx -- update npm-tools
bazel run //cli/cli:dx -- update npm-adopt npm-adopt-polyglot uv-adopt uv-adopt-polyglot
bazel run //cli/cli:dx -- --dry-run update
```

## `dx bump`

```text
dx bump [--offline|--frozen] <set:package> <version>
```

Widens one declared requirement to a new version. Takes exactly one
`set:package` plus one version. Sets: `bazel`, `cargo`, `github-actions`,
`go`, `maven`, `npm`, `nuget`. The package must already be declared in the
manifest, or the run fails without writing.

`cargo`, `go`, `maven`, `npm`, and `nuget` refresh automatically through
`dx update <set>`. A failed refresh keeps the widen and exits `1`. `bazel`
and `github-actions` are file-only and refresh nothing. Review the pin diff
and run `bazel build //...`.

`--offline` and `--frozen` run cache-only. A set that needs a refresh then
fails before widening with `offline_required`. A major bump also needs
`dx migrate --from <old> --to <new>`.

`--check`, `--fail-on`, `--report`, and Bazel options do not apply.
Output: `--output text|json`. Exit codes: 0 success, 2 usage or scope errors,
1 operational failures.

```sh
bazel run //cli/cli:dx -- bump cargo:anyhow 1.0.100
bazel run //cli/cli:dx -- bump go:github.com/google/go-cmp 0.7.0
```
