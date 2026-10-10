# Audit, Update, And Bazel Commands

## `dx bazel`

```text
dx bazel <bazel arguments...>
```

Runs Bazel directly through the repo launcher. Every arg after `bazel` goes to
Bazel unchanged, so `dx` flags do not apply. The exit code is Bazel's own,
except a launch failure or a signal is `1`.

Output: `--output text`.

```sh
bazel run @rules_dx//:dx -- bazel build //...
bazel run @rules_dx//:dx -- bazel query //...
```

## `dx security` And `dx license`

```text
dx security [--here] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
dx license [--here] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif=<dest>|spdx=<dest>] [scope...]
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
selects the set of the nearest path above it in the table plus every set nested
under it, so `//quality/...` selects `cargo`, `npm-tools`, and `uv-tools`, and
`//third_party/...`, a path the table does not name, selects `go`, `maven`,
`nuget`, `powershell`, and `ruby`. A package under a path in the table selects
that path's set, so `//cli/cli` selects `cargo`. Each set then reads its own
lockfile, so a scope narrows which sets run, never which packages inside a
lockfile.

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
snapshot, so they report `no advisory coverage`. A path the table does not name,
and that has no path from the table under it, owns no set.

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

Advisory freshness and exception windows are judged against today's UTC date. Set
`DX_AUDIT_TODAY` to `YYYY-MM-DD` to pin that date and make a run reproducible.
A value that is not ten digits and dashes is ignored.

`dx security` scans secrets with Gitleaks. Set `DX_GITLEAKS_BIN` to the absolute
path of the pinned `@dx_tools//:gitleaks` artifact. A relative path is rejected
and `PATH` is never searched. Without it the run exits `1` with `secrets
auditor unavailable`. `dx license` does not read it.

A `.gitleaks.toml` in the workspace root is passed to the scan. It is the only
config the scan reads. Each scan that uses it reports a warning naming the file,
because the config carries no hash pin. A secret finding is an error whose rule
is the Gitleaks rule id, for example `gitleaks/aws-key`.

`dx license` reads `licenses.toml` from the workspace root. Without the file
every scope is distributed and only `MIT`, `Apache-2.0`, `BSD-2-Clause`,
`BSD-3-Clause`, `ISC`, and `Unlicense` pass. `schema_version` is `1` and an
unknown key fails the run.

`[policy]` holds `blocked`, `[policy.distributed]` holds `allow`, `review`, and
`deny`, and `[policy.sets.<set>]` adds `review` ids for one set. An id in two
lists fails the run. `[distribution] internal` lists the internal scopes, and
every other scope is distributed. An internal scope fails only on `blocked`, so
`review`, `deny`, and unlisted ids pass. A distributed scope fails on all of
them. Its SARIF rule is `license/<license>` and its message names the package,
version, license, and tier.

`MIT`, `Apache-2.0`, `BSD-2-Clause`, and `BSD-3-Clause` also need their text.
Without it a distributed scope fails with the SARIF rule
`license/missing-notice-text`.

```toml
schema_version = 1

[policy]
blocked = ["AGPL-3.0-only", "SSPL-1.0"]

[policy.distributed]
allow = [
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "Unlicense",
]
review = ["LGPL-3.0-only", "MPL-2.0"]
deny = ["GPL-3.0-only", "GPL-3.0-or-later"]

[policy.sets.ruby]
review = ["Ruby"]

[distribution]
internal = ["//..."]

[[inventory]]
package = "some-gem"
set = "ruby"
license = "MIT"
versions = ">=1.0.0, <3.0.0"
text_present = true
```

Each `[[inventory]]` needs `package`, `set`, `license`, and `versions`, and
`text_present` says the license text ships with the package. It overrides the
license the `cargo` and `npm` sets read from `cargo-bazel-lock.json` and
`package-lock.json`, and it is the only license source for every other set, so
a package with no entry is `UNKNOWN`.

Each `[[exception]]` needs `package`, `set`, `license`, `versions`, `reason`,
and `expires`, and covers a finding only when `package`, `set`, and `license`
all match. An `expires` date on or before the audit date fails the run, and so
does an exception that matches no finding. `versions` uses the range syntax of
the `set` ecosystem.

Output: `--output text|json`. Reports: `dx security` writes
`--report sarif=<dest>`. `dx license` writes `--report sarif=<dest>` or
`--report spdx=<dest>`. Repeat the flag for more files. Use `-` for stdout.
A relative destination anchors at the workspace root; an absolute one writes as
given. Colliding destinations fail before anything runs, parents must exist, and
a failed write keeps the previous file bytes.

```sh
bazel run @rules_dx//:dx -- security //...
bazel run @rules_dx//:dx -- license --fail-on error //...
```

`--offline` forbids network: the run uses pinned cached snapshots and fails
with `offline_required` when they are missing. `--frozen` forbids manifest
and lock resolution changes while still allowing already pinned fetches.
Combine both flags for both policies. Older dx versions spelled offline as
`--frozen`: pass `--offline` for that behavior. Exit
codes: 0 success, 2 usage or scope errors, 1 operational failures.

## `dx update`

```text
dx update [--check] [--apply] [--offline] [--frozen] [set...]
```

Updates dependencies per set through the qualified resolvers. Bare runs check
by default and write nothing; `--apply` authorizes the update. No selector
checks all sets. Sets: `cargo`, `go`, `maven`, `npm`, `npm-adopt`,
`npm-adopt-polyglot`, `npm-tools`, `nuget`, `powershell`, `ruby`, `uv`,
`uv-adopt`, `uv-adopt-polyglot`, `uv-tools`. Selectors are `set`, `set:package`,
or a label/path. `go`, `powershell`, and `ruby` pins are manual: `update`
reports them pinned and changes nothing. `ruby` pins come from `bundle lock`
on the seed host, `powershell` pins are hand-written in
`third_party/powershell/PSGallery.lock.json`, and `go` pins track Gazelle and
widen through `dx bump` with a `go` selector.
`--check` validates the selected sets without writing. The `uv` set runs a
read-only lockfile check and reports current or stale. Every other resolvable
set reports unavailable until its check backend lands, so refresh those with
`dx update --apply`. No selector checks all sets. `update` never touches the Bazelrc
preset fragment: verify it with
`bazel run //tools/bazelrc:preset_update -- --verify-only` and regenerate it
with `bazel run //tools/bazelrc:preset_update`.
`--offline` forbids network: the run uses pinned cached snapshots and fails
with `offline_required` when they are missing. `--frozen` forbids manifest
and lock resolution changes and fails with `frozen_locked` when a resolver
would rewrite them; checks still run because they write nothing. Combine both
flags for both policies.
`--fail-on`, `--report`, and Bazel options do not apply.
Output: `--output text|json`. `json` reports one event per set plus
`command_finished`. Text prints a per-set line for every failure plus a summary
count of updated, current, pinned, unsupported, failed, and blocked sets.
Exit codes: 0 success, 2 usage or scope errors, 1 operational failures.

```sh
bazel run @rules_dx//:dx -- update --check
bazel run @rules_dx//:dx -- update go
bazel run @rules_dx//:dx -- update ruby
bazel run @rules_dx//:dx -- update powershell
bazel run @rules_dx//:dx -- update --apply uv uv-tools
bazel run @rules_dx//:dx -- update --apply npm-tools
bazel run @rules_dx//:dx -- update --apply npm-adopt npm-adopt-polyglot uv-adopt uv-adopt-polyglot
bazel run @rules_dx//:dx -- update --dry-run
```

## `dx bump`

```text
dx bump [--check] [--apply] [--offline] [--frozen] <set:package> <version>
```

Widens one declared requirement to a new version. Bare runs check by default:
`bump` compares the widened manifest against the workspace and fails naming
the drifted files; `--apply` writes them. Takes exactly one
`set:package` plus one version. Sets: `bazel`, `cargo`, `github-actions`,
`go`, `maven`, `npm`, `nuget`. The package must already be declared in the
manifest, or the run fails without writing. A `github-actions` version is
the resolved commit SHA, never a tag, and the pin is rewritten in every
`.github/workflows` file that declares the action.

`cargo`, `go`, `maven`, `npm`, and `nuget` refresh automatically through
`dx update <set>`. A failed refresh keeps the widen and exits `1`. `bazel`
and `github-actions` are file-only and refresh nothing. Review the pin diff
and run `bazel build //...`.

A set that needs a refresh under `--offline` fails before widening with
`offline_required`. `--frozen` always fails before widening with
`frozen_locked`, because a bump changes resolution by definition. A major
bump also needs `dx migrate --from <old> --to <new>`.

`--fail-on`, `--report`, and Bazel options do not apply.
Output: `--output text|json`. Exit codes: 0 success, 2 usage or scope errors,
1 operational failures.

```sh
bazel run @rules_dx//:dx -- bump cargo:anyhow 1.0.100 --apply
bazel run @rules_dx//:dx -- bump go:github.com/google/go-cmp 0.7.0 --apply
```

## Consumer dependency sets (`dx.toml`)

A workspace outside `rules_dx` declares its own dependency sets in a
committed `dx.toml` at the workspace root. When the file exists, `update`,
`security`, `license`, and `bump` resolve names and scopes from it instead
of the built-in sets. No built-in set is selected.

```toml
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]
```

Each record names one set: a stable `name`, an `ecosystem` backend kind
(`uv`), workspace-relative `manifests`, `locks`, and owning `scopes`.
Two sets may share one ecosystem. Paths stay inside the workspace: absolute
paths, `..`, and `\` fail. Two writable sets cannot own the same manifest
or lock; mark shared read-only ownership with `writable = false`.
`schema_version` must be `1`. Unknown keys, unknown ecosystems, duplicate
names, empty records, and manifests spread across directories fail with
exit `2` before anything runs.

Selectors are set names (`frontend`), `set:package` (`frontend:anyio`),
or scopes (`apps/frontend`, `//apps/frontend/...`, `//...`). No selector
selects every configured set. A scope selects every set whose scope covers
it; overlapping scopes select every owner and the run names them all.
Unknown names and unowned paths fail with exit `2`.

`dx update` runs `uv lock --directory <dir>` per set and `dx update --check`
runs `uv lock --check --directory <dir>`, with `--offline` added when
`dx` runs offline.
Selective `set:package` updates are unsupported for `uv`: the set refreshes
as a whole. Updating a read-only set fails; checking one works.
`dx security` and `dx license` assess the configured locks. `uv` sets have
no advisory coverage, so the run reports `no advisory coverage for <names>`
and a missing lock fails the family. `dx bump` names a configured set only
to refuse it: widening is unsupported for configured ecosystems.

```sh
bazel run @rules_dx//:dx -- update --check frontend
bazel run @rules_dx//:dx -- update worker
bazel run @rules_dx//:dx -- security apps/frontend
bazel run @rules_dx//:dx -- license --output=json
```
