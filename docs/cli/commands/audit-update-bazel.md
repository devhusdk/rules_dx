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

`dx security` reads the lockfile of each set it owns and matches it against
`.dx/advisory/<family>.json`. Sets: `cargo`, `go`, `maven`, `npm`, `npm-adopt`,
`npm-adopt-polyglot`, `npm-tools`, `nuget`, `powershell`, `ruby`. The
`npm-adopt`, `npm-adopt-polyglot`, and `npm-tools` sets share the `npm` snapshot,
the `ruby` set uses the `rubygems` snapshot, and the `powershell` set uses the
`nuget` snapshot. Sets without a snapshot are listed as
`no advisory coverage for <sets>`.

A scope selects sets by path, and a path can select more than one. A `...` scope
selects its own set plus every set nested under it, so `//quality/...` selects
`cargo`, `npm-tools`, and `uv-tools`. A scope without `...` names one package
and selects only that package's set. Each set then reads its own lockfile, so a
scope narrows which sets run, never which packages inside a lockfile.

```text
//cli/..., //docs/ir/..., //env/..., //generation/..., //quality/..., //rust/...      cargo
//examples/adopt-rust/...                                                             cargo
//csharp/..., //fsharp/..., //third_party/dotnet/...                                  nuget
//examples/adopt-csharp/..., //examples/adopt-fsharp/...                              nuget
//go/..., //third_party/go/...                                                         go
//examples/adopt-go/...                                                               go
//powershell/..., //third_party/powershell/..., //examples/adopt-powershell/...      powershell
//ruby/..., //third_party/ruby/..., //examples/adopt-ruby/...                        ruby
//java/..., //kotlin/..., //scala/..., //third_party/jvm/...                          maven
//examples/adopt-java/..., //examples/adopt-kotlin/..., //examples/adopt-scala/...    maven
//javascript/..., //typescript/..., //astro/..., //svelte/..., //vue/..., //mdx/...   npm
//quality/tools/javascript/...                                                        npm-tools
//examples/adopt-js-ts/...                                                            npm-adopt
//examples/adopt-polyglot/...                                                         npm-adopt-polyglot
//examples/adopt-polyglot/...                                                         uv-adopt-polyglot
//python/..., //python/tests/fixtures/hello/...                                       uv
//examples/adopt-python/...                                                           uv-adopt
//quality/tools/python/...                                                            uv-tools
```

A scope that owns no set exits `2` and names the audited sets. `//...` and
`MODULE.bazel` select every set. The four `uv` sets resolve but have no
snapshot, so they report `no advisory coverage`. Every path outside the table
owns no set.

`dx security` and `dx license` read `pnpm-lock.yaml`, `package-lock.json`, and
`yarn.lock` for the `npm` set, and the lockfile of every other set. The `ruby`
set reads `third_party/ruby/Gemfile.lock` and
`examples/adopt-ruby/Gemfile.lock`. The `powershell` set reads
`third_party/powershell/PSGallery.lock.json`. Each finding names the lockfile it
was read from.

`dx security` reads `security.toml` from the workspace root. Without the file
no finding is exempted. Each `[[exception]]` needs `advisory`, `package`, `set`,
`versions`, `reason`, and `expires`. `schema_version` is `1`.

```toml
[[exception]]
advisory = "GHSA-aaaa-bbbb-cccc"
package = "some-package"
set = "npm"
versions = ">=1.2.0, <2.0.0"
reason = "Reviewed; no reachable code path."
expires = "2027-03-01"
```

An `expires` date on or before the audit date fails the run. An exception that
matches no current finding in its own set fails the run, so remove it when the
finding goes away. `versions` uses the range syntax of the `set` ecosystem.

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
`npm-adopt-polyglot`, `npm-tools`, `nuget`, `powershell`, `ruby`, `uv`,
`uv-adopt`, `uv-adopt-polyglot`, `uv-tools`. Selectors are `set`, `set:package`,
or a label/path. `go`, `powershell`, and `ruby` are pinned no-op successes.
`ruby` pins come from `bundle lock` on the seed host; `powershell` pins are
hand-written in `third_party/powershell/PSGallery.lock.json`. `--check` fails if
the preset is stale and ignores selectors. `--offline` and `--frozen` run cache-only with no network
fetches. `--fail-on`, `--report`, and Bazel options do not apply.
Output: `--output text|json`. `json` reports per set plus a summary count.
Exit codes: 0 success, 2 usage or scope errors, 1 operational failures.

```sh
bazel run //cli/cli:dx -- update --check
bazel run //cli/cli:dx -- update go
bazel run //cli/cli:dx -- update ruby
bazel run //cli/cli:dx -- update powershell
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
