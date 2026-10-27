# Audit And Update Commands

## `dx security` And `dx license`

```text
dx security [--here] [--affected] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
dx license [--here] [--affected] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif=<dest>|spdx=<dest>] [scope...]
```

`dx security` checks secrets plus dependency vulnerabilities. `dx license`
checks dependency licenses. No scope means `//...`. Use `--here` for the
current dir tree, or `--affected` for the sets that own targets your git
changes reach (see [Scope Defaults](scope-defaults.md)). With no changes, or
for `dx license` no owned set, they print `no affected targets` and exit `0`.
When files changed but no set owns them, `dx security` runs only the secrets
scan. Neither command changes files. `--check` and Bazel options do not apply.

`dx security` reads the lockfile of each set it owns and matches it against
`.dx/advisory/<family>.json`. Sets: `cargo`, `go`, `maven`, `npm`, `npm-adopt`,
`npm-adopt-polyglot`, `npm-tools`, `nuget`, `powershell`, `ruby`. The
`npm-adopt`, `npm-adopt-polyglot`, and `npm-tools` sets share the `npm` snapshot,
and the `ruby` set uses the `rubygems` snapshot. The `powershell` set has no
advisory snapshot. A run that selects it fails with `failed to assess powershell`
when `PSGallery.lock.json` lists modules, unless `security.toml` accepts it with
an `[[uncovered]]` entry, and lists it under `no advisory coverage` otherwise.
Sets without a snapshot are listed as `no advisory coverage for <sets>`.
An advisory range or locked version that dx cannot parse marks the package
unassessed with `unsupported version syntax`, and the run exits `1`.

Create or refresh the snapshots from the workspace root:

```text
bazel run @rules_dx//cli/advisory_prep --
```

A snapshot is fresh only on the day it was prepared. A missing or stale
snapshot fails with `advisory_refresh_failed` and names this command. See
[Advisory Snapshots](#advisory-snapshots) for its flags.

A scope selects sets by path, and a path can select more than one. A `...` scope
selects the set of the nearest path above it in the table plus every set nested
under it, so `//quality/...` selects `cargo`, `npm-tools`, and `uv-tools`, and
`//third_party/...`, a path the table does not name, selects `go`, `maven`,
`nuget`, `powershell`, and `ruby`. A package under a path in the table selects
that path's set, so `//cli/cli` selects `cargo`. Each set then reads its own
lockfile, so a scope narrows which sets run, never which packages inside a
lockfile.

```text
//cli/..., //tools/docs/..., //env/..., //generation/..., //quality/..., //rust/...      cargo
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
was read from. `pnpm-lock.yaml` is read at `lockfileVersion` 5.x, 6.0, and 9.0; a
package key that cannot be resolved fails the command with a lock parse error
naming the key.

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

An `[[uncovered]]` entry accepts a set that has no advisory source. It needs
`set`, `reason`, and `expires`. The run stays complete and still lists the set
under `no advisory coverage`. An `expires` date on or before the audit date
fails the run.

```toml
[[uncovered]]
set = "powershell"
reason = "PowerShell Gallery publishes no advisories."
expires = "2027-04-24"
```

Advisory freshness and exception windows are judged against today's UTC date. Set
`DX_AUDIT_TODAY` to `YYYY-MM-DD` to pin that date and make a run reproducible.
A value that is not ten digits and dashes is ignored.

`dx security` scans secrets with the pinned `@dx_tools//:gitleaks` artifact.
When `DX_GITLEAKS_BIN` holds an absolute path, dx runs that file. When it is
unset, dx resolves the label through Bazel in the workspace. A relative path is
rejected and `PATH` is never searched. If no artifact is found, the run exits
`1` with `secrets auditor unavailable` and prints this command, which exports
the path in the current shell:

```text
export "$(bazel run @rules_dx//cli/tool_resolve -- --label @dx_tools//:gitleaks --var DX_GITLEAKS_BIN --print)"
```

`dx license` does not read it.

A `.gitleaks.toml` in the workspace root is merged into the config dx hands the
scan. It is the only user config the scan reads. Each scan that uses it reports
a warning naming the file, because the config carries no hash pin. A secret
finding is an error whose rule is the Gitleaks rule id, for example
`gitleaks/aws-key`.

The scan skips files and directories that git ignores. Untracked files that git
does not ignore are scanned, and so are tracked files inside an ignored
directory. Outside a git repository, or when git is unavailable, every file is
scanned.

`dx license` reads `licenses.toml` from the workspace root. Without the file
every scope is distributed and only `MIT`, `Apache-2.0`, `BSD-2-Clause`,
`BSD-3-Clause`, `ISC`, and `Unlicense` pass. `schema_version` is `1` and an
unknown key fails the run.

`[policy]` holds `blocked`, `[policy.distributed]` holds `allow`, `review`, and
`deny`, and `[policy.sets.<set>]` adds `review` ids for one set. An id in two
lists fails the run. `[distribution] internal` lists label patterns. `//...`
covers every scope in the main repo, `//pkg/...` covers `pkg` and everything
under it, `//pkg:all` covers the targets in `pkg`, and any other entry matches
one label. A run is internal only when an entry covers every scope, including
the `--here` and `--affected` scopes, and every other run is distributed. An internal scope fails only on `blocked`, so
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
all match. An `expires` date on or before the audit date fails the run. An
exception that matches no finding in its own set fails the run too. Exceptions
for sets outside the run are still checked for fields and expiry, then skipped.
`versions` uses the range syntax of
the `set` ecosystem.

Output: `--output text|json`. Text prints one line per finding:
`<path>: <severity>: <message> [<tool>/<rule>]`. The lines go to stderr when a
report goes to stdout. JSON emits one `diagnostic` event per finding with
`tool`, `rule`, `path`, `severity`, and `message`. A vulnerability has tool
`vuln`, rule `vuln/<advisory>`, the lockfile as `path`, and a message that
names `<package>@<version>`, the advisory, its severity, and the fixed
version. Its `package` has `name`, `version`, and `fixed`, the list of fixed
versions, empty when none exists. A license finding has tool `license`, rule
`license/<license>`, a message that starts with
`<package>@<version> license <license>`, and a `package` with `name`,
`version`, and `license`. A failed family emits an `error` event. Its code is `advisory_refresh_failed` for a
missing, stale, or corrupt advisory snapshot, `offline_required` when
`--offline` blocks a refresh, and `audit_failed` for findings or any other
incomplete assessment. Reports: `dx security` writes
`--report sarif=<dest>`. `dx license` writes `--report sarif=<dest>` or
`--report spdx=<dest>`. Repeat the flag for more files. Use `-` for stdout.
The SPDX report is an SPDX 2.3 JSON document with `creationInfo` naming
`Tool: dx`. Each audited scope label is a package `SPDXRef-Root-<n>`, numbered
over the sorted labels, and the document has one
`SPDXRef-DOCUMENT DESCRIBES SPDXRef-Root-<n>` relationship per label. Every
package sets `downloadLocation` to `NOASSERTION`.
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
Combine both flags for both policies. Exit codes: 0 success, 2 usage or scope
errors, 1 findings at or above `--fail-on`, an incomplete assessment, or an
operational failure.

## Advisory Snapshots

```text
bazel run @rules_dx//cli/advisory_prep --
```

The command converts one archive per advisory family the dependency locks need,
so a repository with only `Cargo.lock` prepares the `cargo` snapshot alone. It
writes `<out>/<family>.json` and `<out>/<family>.meta.json`. The sidecar holds
the set, the source URL, the `sha256` of the payload, the stamped date and the
payload path. The payload is the same for the same advisories whatever order
the archive lists them in.

Relative paths anchor at the workspace root.

- `--workspace <dir>`: dependency set locks to read. Default the workspace root.
- `--out <dir>`: where the snapshots are written. Default `.dx/advisory` under it.
- `--date YYYY-MM-DD`: date every sidecar carries. Default today, UTC.
- `--max-time <seconds>`: wall clock limit for one download. Default 600.
- `--connect-timeout <seconds>`: connect limit for one download. Default 30.
- `--retries <count>`: attempts one download may spend. Default 3.
- `--retry-delay <seconds>`: pause between two attempts. Default 5.
- `--archive FAMILY=PATH`: convert a local archive instead of fetching it.
  Repeatable.
- `--status-file <file>`: append `status`, `ecosystems` and `families` as
  `key=value` lines.

A repository whose locks need no advisory ecosystem prepares nothing. The
command prints `no advisory ecosystems needed`, exits `0` and reports
`status=empty` and `ecosystems=0`. Dependency sets without a lock are not
audited.

Only `https` and `file` URLs are fetched, a redirect stays on `https`, and an
archive entry is refused when its length or checksum disagrees with the
archive. Exit codes: `0` success, `2` usage error, `1` preparation failure. A
failure prints one line starting with `advisory_prepare_failed:` naming the
family and the archive entry, path or limit it could not use, and writes no
snapshot for that family.

## Hermetic Tool Resolution

```text
bazel run @rules_dx//cli/tool_resolve -- --label @dx_tools//:gitleaks --var DX_GITLEAKS_BIN --print
```

The command runs `bazel cquery --output=files` for the label, takes the last
output line as the artifact path, joins it with `bazel info output_base`, and
refuses a missing file.

- `--label <label>`: pinned tool label to resolve. Required.
- `--var <name>`: environment variable receiving the absolute path. Required.
- `--bazel <path>`: Bazel binary the resolution runs through. Default `bazel`.
- `--workspace <dir>`: directory the inner Bazel commands run in. Default
  `BUILD_WORKSPACE_DIRECTORY`, the `bazel run` invocation root, else the
  current directory.
- `--env-file <path>`: environment file receiving `VAR=path`. Default
  `GITHUB_ENV`.
- `--print`: write `VAR=path` to standard output instead of a file.
- Arguments after `--` reach the inner cquery unchanged.

Exit codes: `0` success, `2` usage error, `1` resolution failure. A failure
prints one line starting with `tool_resolve_failed:` naming the label and the
cquery, output-base, missing-file or environment write it could not use, and
exports nothing.

## `dx update`

```text
dx update [--check] [--apply] [--offline] [--frozen] [selector ...]
```

Updates dependencies per set through the qualified resolvers. Checks by
default; `--apply` writes updates. Sets: `cargo`, `go`, `maven`, `npm`,
`npm-adopt`, `npm-adopt-polyglot`, `npm-tools`, `nuget`, `powershell`, `ruby`, `uv`,
`uv-adopt`, `uv-adopt-polyglot`, `uv-tools`. Selectors are `set`, `set:package`,
or a label/path. `go`, `powershell`, and `ruby` pins are manual: `update`
reports them pinned and changes nothing. `ruby` pins come from `bundle lock`
on the seed host, `powershell` pins are hand-written in
`third_party/powershell/PSGallery.lock.json`, and `go` pins track Gazelle and
widen through `dx bump` with a `go` selector.
`--check` validates the selected sets without writing. The `uv` set runs a
read-only lockfile check and reports current, and a stale lock reports failed.
`go`, `powershell`, and `ruby` report pinned. Every other resolvable set reports
unsupported and fails the run; refresh those with `dx update --apply <set>`.
No selector checks all sets, so a bare `dx update` reports the unsupported sets
and exits `1`. Checks never take a [build slot](README.md#build-slots).
`--apply` takes one only for `cargo`, `maven`, `npm`, `npm-tools` and `nuget`,
whose updaters run Bazel. `update` never touches the Bazel preset. The preset
is three generated files under `tools/bazelrc/`, imported in this order at the
top of the root `.bazelrc`, so later lines in `.bazelrc` override them:

- `correctness.bazelrc`: Bzlmod, failure output, coverage, and runfiles flags.
- `performance.bazelrc`: the host compilation mode and the `dx_*` compilation configs.
- `resources.bazelrc`: the memory, CPU, and cache limits listed below.

Every flag has a one-line comment above it. Verify the files with
`bazel run //tools/bazelrc:preset_update -- --verify-only` and regenerate them
with `bazel run //tools/bazelrc:preset_update`.

`resources.bazelrc` sets these defaults:

- `startup --host_jvm_args=-XX:MaxHeapFreeRatio=30` and `-XX:MinHeapFreeRatio=10`: idle servers return heap to the OS.
- `startup --host_jvm_args=-XX:MaxRAMPercentage=25`: the server heap is up to 25% of RAM, or of the cgroup limit when there is one.
- `common --local_resources=memory=HOST_RAM*.5` and `common --local_resources=cpu=HOST_CPUS*.75`: actions use half the RAM and three quarters of the cores.
- `startup --max_idle_secs=900`: a server shuts down after 15 minutes idle.
- `startup --shutdown_on_low_sys_mem`: an idle server shuts down when system memory runs low. It needs `--max_idle_secs`.
- `common --experimental_disk_cache_gc_max_age=14d`: prunes disk cache entries older than 14 days. It only applies when a disk cache is set.
- `common --experimental_disk_cache_gc_max_size=20G`: caps the disk cache at `20G`. It only applies when a disk cache is set.
- `common --repo_contents_cache_gc_max_age=60d`: keeps repo contents for 60 days. `dx worktree prune` cleans the downloads in the repository cache.
- `build --experimental_shrink_worker_pool` and `build --experimental_total_worker_memory_limit_mb=4096`: cap worker memory per server.

Override any of them in `user.bazelrc`.

`--offline` forbids network: the run uses pinned cached snapshots and fails
with `offline_required` when they are missing. `--frozen` forbids manifest
and lock resolution changes: `--apply` fails with `frozen_locked` when a
resolver would rewrite them. `--check` writes nothing, so `--frozen` does not
change it. Combine both flags for both policies.
`--fail-on`, `--report`, and Bazel options do not apply.
`--bazel-startup-option` applies to the cargo, maven, npm, npm-tools, and
nuget sets, which run Bazel.
Output: `--output text|json`. `json` emits `command_started`, then per set
either one `notice` (`update_set_success`, `update_set_current`,
`update_set_pinned`, or `update_set_blocked`) or one `error` (`update_failed`,
`update_set_unsupported`, `offline_required`, `frozen_locked`, or
`backend_unavailable`). An applied set also emits one `change` event per edited
file, before its success notice. A failed run then emits an `update_recovery`
notice, and every run ends with `command_finished`. Text prints
`dx: <code>: <message>` on stderr for every failure plus a summary count of updated, current, pinned, unsupported, failed, and blocked sets.
A failed run prints an `update_recovery` notice with the rerun command. Text
goes to stderr; JSON is a `notice` whose `scope` lists the sets to retry. After
`--apply` the command is `dx update --apply <sets>`, plus
`git checkout -- <locks>` to discard kept successes. After `--check` it is
`dx update <sets>`. In a consumer workspace the restore command uses the lock
paths declared in `dx.toml`.
Exit codes: 0 success, 2 usage or scope errors, 1 when any set is unsupported,
failed, or blocked, or on an operational failure.

```sh
bazel run @rules_dx//:dx -- update --check
bazel run @rules_dx//:dx -- update --apply go
bazel run @rules_dx//:dx -- update --apply ruby
bazel run @rules_dx//:dx -- update --apply powershell
bazel run @rules_dx//:dx -- update --apply uv uv-tools
bazel run @rules_dx//:dx -- update --apply npm-tools
bazel run @rules_dx//:dx -- update --apply npm-adopt npm-adopt-polyglot uv-adopt uv-adopt-polyglot
```

## `dx bump`

```text
dx bump [--apply] [--offline] [--frozen] <set:package> <version>
```

Widens one declared requirement to a new version. Checks by default;
`--apply` widens. Without `--apply`, bump exits `0` when the manifest already
declares the version and exits `1` with `bump drift` when a widen is pending.
Takes exactly one
`set:package` plus one version. Sets: `bazel`, `cargo`, `github-actions`,
`go`, `maven`, `npm`, `nuget`. The package must already be declared in the
manifest, or the run fails without writing. A `bazel` bump rewrites the one
literal `version` of the matching top-level `bazel_dep` in `MODULE.bazel`
and keeps every other byte. Duplicate declarations, a missing or computed
version or name, and a `MODULE.bazel` that does not parse fail without
writing. A `cargo` bump rewrites the crate in every manifest of the
`crate.from_cargo` hub in `MODULE.bazel` whose `cargo_lockfile` is
`rust/tests/fixtures/hello/Cargo.lock`, if that manifest declares the crate.
It fails without writing if no manifest declares it, or if any manifest
declares it ambiguously or in a non-registry form. `go` edits
`third_party/go/go.mod` and `nuget` edits
`third_party/dotnet/paket.dependencies`. A `github-actions` version is
the resolved commit SHA, never a tag, and the pin is rewritten in every
`.github/workflows` file that declares the action.

A configured `bazel` set from `dx.toml` is selected as `<set>:<module>`. It
rewrites the one literal `version` of that `bazel_dep` in the set's own
`MODULE.bazel`. A read-only set and `<set>:.bazelversion` fail without
writing. Configured `uv` and `npm` sets fail with `bump is not supported`.

`cargo`, `go`, `maven`, `npm`, and `nuget` refresh automatically through
`dx update --apply <set>`. A failed refresh keeps the widen and exits `1`. `bazel`
and `github-actions` are file-only and refresh nothing. Review the pin diff
and run `bazel build //...`.

A version that is already declared reports `bump_no_change`, writes and
refreshes nothing, and exits `0`, also under `--frozen` or `--offline`. Only
manifests whose pin changes are written. When a widen is pending, a set that
needs a refresh under `--offline` fails before widening with
`offline_required`, and `--frozen` fails before widening with
`frozen_locked`. A major bump also needs `dx migrate --from <old> --to <new>`.

`--check`, `--fail-on`, `--report`, and Bazel options do not apply.
`--bazel-startup-option` applies to the Bazel run that refreshes the lock.
Output: `--output text|json`. Exit codes: 0 success, 2 usage or scope errors,
1 a pending widen without `--apply` or an operational failure.

```sh
bazel run @rules_dx//:dx -- bump --apply cargo:anyhow 1.0.100
bazel run @rules_dx//:dx -- bump --apply go:github.com/google/go-cmp 0.7.0
bazel run @rules_dx//:dx -- bump --apply bazel-modules:rules_cc 0.3.0
```

## `dx add`

```text
dx add [--apply] [--offline] [--frozen] <set:package> <version>
```

Declares one new requirement at an exact version. Checks by default;
`--apply` adds. Without `--apply`, add exits `1` with `add pending` and
writes nothing. Takes exactly one `set:package` plus one version, never a
batch. Sets: `bazel`, `cargo`, `go`, `maven`, `npm`, `nuget`. The package
must not be declared yet: a declared package fails in both modes and names
the `dx bump` command that changes its version.

Each set writes the same manifest `dx bump` widens and keeps every other byte:

- `bazel`: a `bazel_dep` after the last top-level `bazel_dep` in
  `MODULE.bazel`.
- `cargo`: a `[dependencies]` entry, kept sorted when the table already is.
- `go`: a `require` line in the first direct `require` block of `go.mod`.
- `maven`: a coordinate at the end of the `artifacts` of the
  `maven.install` that feeds the default `@maven` repository.
- `npm`: a `dependencies` entry in `package.json`.
- `nuget`: a `nuget` line in the main group of `paket.dependencies`.

A configured `bazel` set from `dx.toml` is selected as `<set>:<module>` and
adds to the set's own `MODULE.bazel`. A read-only set fails without writing.
Configured `uv` and `npm` sets fail with `add is not supported`.
`github-actions` and `bazel:.bazelversion` are usage errors: add the `uses:`
step by hand, or change the pin with `dx bump`.

`cargo`, `maven`, `npm`, and `nuget` refresh automatically through
`dx update --apply <set>`. `go` refreshes as a no-op and does not update
`go.sum`. `bazel` is file-only. A failed refresh keeps the add and exits `1`.
A set that needs a refresh under `--offline` fails before writing with
`offline_required`. `--frozen` always fails before writing with
`frozen_locked`.

`--check`, `--fail-on`, `--report`, and Bazel options do not apply.
`--bazel-startup-option` applies to the Bazel run that refreshes the lock.
Output: `--output text|json`. Exit codes: 0 success, 2 usage or scope errors,
1 a pending add without `--apply`, a declared package, or an operational
failure.

```sh
bazel run @rules_dx//:dx -- add --apply cargo:serde 1.0.200
bazel run @rules_dx//:dx -- add --apply npm:vue 3.5.42
bazel run @rules_dx//:dx -- add --apply bazel-modules:rules_go 0.50.0
```

## Consumer dependency sets (`dx.toml`)

A workspace outside `rules_dx` declares its own dependency sets in a
committed `dx.toml` at the workspace root. When `dx.toml` declares at least
one `[[dependency_set]]`, `update`, `security`, `license`, and `bump` resolve
names and scopes from it instead of the built-in sets. No built-in set is
selected. A `dx.toml` with only `[dx]` or `[quality]` tables keeps the built-in
sets. A `dx.toml` with neither sets nor those tables fails with exit `2`.

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
(`uv`, `npm`, `bazel`, `ruby`, `powershell`, or `cargo`), workspace-relative
`manifests`, `locks`, and owning `scopes`. A `cargo` record also names its
`repository`; any other ecosystem fails with exit `2` when it sets one. Manifests are named for the
ecosystem: `pyproject.toml` for `uv`, `package.json` for `npm`,
`MODULE.bazel` for `bazel`, `Gemfile` for `ruby`,
`PSGallery.requirements.psd1` for `powershell`, `Cargo.toml` for `cargo`; any other name
fails with exit `2`. Two sets may share one ecosystem. Paths stay inside the workspace: absolute
paths, `..`, and `\` fail. A manifest or lock whose symlink resolves outside
the workspace fails `security` and `license` for that set with exit `1`.
Two writable sets cannot own the same manifest or lock; mark shared read-only ownership with `writable = false`.
`schema_version` must be `1`. Unknown keys, unknown ecosystems, duplicate
names, empty records, and manifests spread across directories fail with
exit `2` before anything runs.

A `bazel` set audits the resolved Bazel module graph. Point `manifests` at
`MODULE.bazel` and `locks` at `MODULE.bazel.lock`.

```toml
[[dependency_set]]
name = "bazel-modules"
ecosystem = "bazel"
manifests = ["MODULE.bazel"]
locks = ["MODULE.bazel.lock"]
scopes = ["."]
```

`dx security` and `dx license` list every selected module from the lock's
`source.json` entries, plus `archive_override`, `git_override`,
`local_path_override`, and literal `http_archive`, `http_file`, and
`http_jar` calls in `MODULE.bazel` and the files it pulls in with `include`.
Downloads through any other `use_repo_rule`, such as `git_repository`, are
listed as `repo_rule` inputs named by commit, `sha256`, or tag, and
`unpinned` when none is set.
Each input is named by registry and `source.json` digest, archive `sha256`
or integrity, git commit, or local path. Modules that lost version selection
are not listed.
Repos that module extensions record under `moduleExtensions` in
`MODULE.bazel.lock`, from any module in the graph, are listed as
`module_extension` inputs under their repo name. Each is named by `sha256`,
integrity, or commit, and `unpinned` when none is set. Generated repos with
no URL are skipped. Extensions marked reproducible are not in the lock, so
their downloads are not listed.

No advisory source maps Bazel modules. A run with only `bazel` sets exits
`0` with `clean; no advisory coverage for <name>` and an info diagnostic with
the input counts per source, including the `module extension` count. Read the diagnostic: nothing was checked for advisories. A missing or unreadable lock, a
`bazel_dep` absent from the lock, a declared version above the selected one,
an override still resolved from a registry, a `git_override` without a
commit, an `include` that is not a `//` label naming a `.MODULE.bazel` file,
cannot be read, or points outside the workspace,
an archive without `sha256` or `integrity`, a computed `module_name`, a
malformed `sha256` or `integrity` on an extension repo, and two extensions
that generate one repo at the same version from different URLs fail the set with `failed to assess <name>` and exit `1`.

`dx license` reports `UNKNOWN` for every input unless `licenses.toml`
declares it with `[[inventory]]` and `set = "bazel"`. The SPDX report carries
each input's origin as an external reference. A module that two `bazel`
sets share is listed once with every origin.

```toml
[[inventory]]
package = "rules_cc"
set = "bazel"
license = "Apache-2.0"
versions = "0.2.1"
text_present = true
```

`dx update --check` reports a `bazel` set as pinned. `dx update --apply`
changes nothing; change a pin with `dx bump <set>:<module> <version>` and
refresh the lock with `bazel mod deps`.

An `npm` set locks one `package.json` with the workspace's own pnpm. The
root `MODULE.bazel` must load it:

```starlark
bazel_dep(name = "aspect_rules_js", version = "3.5.2")

pnpm = use_extension("@aspect_rules_js//npm:extensions.bzl", "pnpm")
use_repo(pnpm, "pnpm")
```

```toml
[[dependency_set]]
name = "web"
ecosystem = "npm"
manifests = ["apps/web/package.json"]
locks = ["apps/web/pnpm-lock.yaml"]
scopes = ["apps/web"]
```

`dx update --apply` runs
`bazel run @pnpm//:pnpm -- --dir <absolute set directory> install --lockfile-only`
from the workspace root and writes that set's `pnpm-lock.yaml` only; no
`node_modules` is created. `set:package` selectors such as
`web:@types/node` run `update <packages> --lockfile-only` instead.
`dx update --check` runs `install --lockfile-only --frozen-lockfile`, writes
nothing, and fails the set with `updater exited <code>` when the lock is stale.
`--check --offline` adds pnpm `--offline` and Bazel
`--repository_disable_download`. `--apply --offline` fails with
`offline_required`, `--apply --frozen` fails with `frozen_locked`, and
`--check` with a `set:package` selector is unsupported. `--bazel-startup-option`
values reach the `bazel` call. pnpm runs with `npm_config_globalconfig` and
`npm_config_userconfig` set to `/dev/null` (`NUL` on Windows), so user and
global `.npmrc` files are ignored; a `.npmrc` in the set directory still
applies. The set is unsupported before pnpm starts when `locks` does not list
`<set directory>/pnpm-lock.yaml` or when a parent directory inside the
workspace holds a `pnpm-workspace.yaml`.

`dx security` matches every `pnpm-lock.yaml` package against
`.dx/advisory/npm.json`; a missing snapshot fails the set with
`failed to assess <name>`. `dx license` reports `UNKNOWN` for npm packages
unless `licenses.toml` declares them with `[[inventory]]` and `set = "npm"`.

A `ruby` set and a `powershell` set are pinned: dx reads their committed
locks and never resolves them.

```toml
[[dependency_set]]
name = "gems"
ecosystem = "ruby"
manifests = ["apps/rb/Gemfile"]
locks = ["apps/rb/Gemfile.lock"]
scopes = ["apps/rb"]

[[dependency_set]]
name = "modules"
ecosystem = "powershell"
manifests = ["tools/ps/PSGallery.requirements.psd1"]
locks = ["tools/ps/PSGallery.lock.json"]
scopes = ["tools/ps"]
```

`locks` must list the lock beside the manifest, `<set directory>/Gemfile.lock`
or `<set directory>/PSGallery.lock.json`; otherwise `dx update` reports the
set unsupported and names the missing path. `dx update --check` and
`dx update --apply` report both as pinned, also under `--offline` and
`--frozen`, and start no process. A `set:package` selector is unsupported:
regenerate `Gemfile.lock` with `bundle lock`, or `PSGallery.lock.json` by
hand. `dx security` matches every `Gemfile.lock` gem against
`.dx/advisory/rubygems.json`; a missing snapshot fails the set with
`failed to assess <name>`, and `--offline` fails it with `offline_required`.
No advisory source covers PowerShell Gallery modules, so a `powershell` set
whose lock lists modules fails `dx security` unless `security.toml` accepts
it with an `[[uncovered]]` entry naming the set. `dx license` reports
`UNKNOWN` for gems and modules unless `licenses.toml` declares them with
`[[inventory]]` and `set = "ruby"` or `set = "powershell"`.

A `cargo` set locks one `Cargo.toml` with the workspace's own rules_rust
crate_universe; dx ships no cargo. `repository` is the `name` given to
`crate.from_cargo`, and the root `MODULE.bazel` must list it in `use_repo`
without a rename. Set `lockfile` so the hub keeps its cargo-bazel lock in the
set, and list both locks:

```starlark
crate = use_extension("@rules_rust//crate_universe:extensions.bzl", "crate")
crate.from_cargo(
    name = "crates",
    cargo_lockfile = "//apps/rs:Cargo.lock",
    lockfile = "//apps/rs:cargo-bazel-lock.json",
    manifests = ["//apps/rs:Cargo.toml"],
)
use_repo(crate, "crates")
```

```toml
[[dependency_set]]
name = "crates"
ecosystem = "cargo"
manifests = ["apps/rs/Cargo.toml"]
locks = ["apps/rs/Cargo.lock", "apps/rs/cargo-bazel-lock.json"]
scopes = ["apps/rs"]
repository = "crates"
```

`repository` starts with a letter and uses `[A-Za-z0-9_.-]` only. A missing
`repository`, an invalid one, and two `cargo` sets sharing one fail with exit
`2`. `locks` must list `<set directory>/Cargo.lock`; otherwise `dx update`
reports the set unsupported and names the missing path.

`dx update --apply` runs `bazel fetch --repo=@<repository>` from the workspace
root with `CARGO_BAZEL_REPIN=workspace`, `REPIN=workspace`,
`CARGO_BAZEL_REPIN_ONLY=<repository>`, and `CARGO_BAZEL_ISOLATED=true`, so
crate_universe re-resolves `Cargo.toml` into that hub's `Cargo.lock` and
cargo-bazel lock only, without upgrading other crates. `dx update --check`
runs the same command with `CARGO_BAZEL_REPIN=false` and `REPIN=false`;
crate_universe then fails the set with `updater exited <code>` when the lock
no longer matches the manifest. Both variables are always set, so a `REPIN`
exported in the shell does not turn a check into a write. `--check --offline`
adds `--repository_disable_download`, `--apply --offline` fails with
`offline_required`, and `--apply --frozen` fails with `frozen_locked`. A
`set:package` selector is unsupported: select the set without a package.
`--bazel-startup-option` values reach the `bazel` call. Bazel evaluates
every `crate.from_cargo` hub of the workspace together, so a stale hub fails
the check and update of every `cargo` set; update the stale set first.

Every check that runs Bazel (`npm` and `cargo`) keeps the bytes of the set's
manifests and locks. If the check changed any of them, dx restores them and
fails the set with `check_wrote: <paths>`. A `crate.from_cargo` hub without
`lockfile` always repins, so its check ends in `check_wrote`; declare
`lockfile` and list it in `locks`.

`dx security` matches every `Cargo.lock` crate from crates.io against
`.dx/advisory/cargo.json`; a missing snapshot fails the set with
`failed to assess <name>`, and `--offline` fails it with `offline_required`.
`dx license` reads crate licenses from a declared `*.json` lock, and a
`licenses.toml` `[[inventory]]` entry with `set = "cargo"` overrides them.

Selectors are set names (`frontend`), `set:package` (`frontend:anyio`),
or scopes (`apps/frontend`, `//apps/frontend/...`, `//...`). No selector
selects every configured set. A scope selects every set whose scope covers
it; overlapping scopes select every owner and the run names them all.
Unknown names and unowned paths fail with exit `2`.

`dx update` and `dx update --check` run the pinned uv `0.12.23`. Set
`DX_UV_BIN` to the absolute path of the `@dx_tools//:uv` artifact and
`DX_UV_PYTHON` to the absolute path of the `@rules_dx//cli/update:python_interpreter`
interpreter. A relative path is rejected and `PATH` is never searched. A uv
that does not report `0.12.23` is rejected. Without a usable `DX_UV_BIN` or
`DX_UV_PYTHON` the set fails with `backend_unavailable` and the run exits `1`;
JSON reports an `error` event with that code. It is never reported current. In CI, `bazel run @rules_dx//cli/tool_resolve --
--label @dx_tools//:uv --var DX_UV_BIN` exports the uv path and the same
command with `--label @rules_dx//cli/update:python_interpreter --var
DX_UV_PYTHON` exports the interpreter.

`dx update --apply` runs `uv lock` in the set directory and writes that set's
lock only. `dx update --check` runs `uv lock --check`, writes nothing, and
fails the set with `updater exited 1` when the lock is stale. Both pass
`--no-config --no-cache --no-python-downloads --python <DX_UV_PYTHON>` and
start uv with an empty environment, so `HOME`, `XDG_CONFIG_HOME`, `UV_*`
variables, and user `uv.toml` files are ignored. `--check --offline` adds
`--offline`; a lock that needs a download to verify then fails the set with
`updater exited 2`. `--apply --offline` fails with `offline_required` and
`--apply --frozen` fails with `frozen_locked`; `--check --frozen` runs.
A `uv.toml` in the set directory or any parent inside the workspace, or a
`[tool.uv]` key other than `sources`, `workspace`, `dev-dependencies`,
`default-groups`, `conflicts`, `package`, `managed`, `override-dependencies`,
or `constraint-dependencies`, makes the set unsupported before uv starts.
Selective `set:package` updates are unsupported for `uv`: the set refreshes
as a whole. Updating a read-only set fails; checking one works.
`dx security` and `dx license` assess the configured locks. `dx security`
matches every `uv.lock` package, direct and transitive, against
`.dx/advisory/pypi.json`, using PEP 440 versions and PEP 503 names. Workspace
members are skipped. Packages from git, path, url, directory, or a registry
other than pypi.org are reported as unassessed and the run exits `1`, as does
an unparseable locked version. A missing lock fails the family. A stale,
missing, or failed advisory snapshot fails the set with
`failed to assess <name>`, and `--offline` fails with `offline_required` when
the snapshot is missing. A `security.toml` exception uses `set = "<name>"` and a
PEP 440 `versions` range. `dx bump` refuses configured `uv`, `npm`, `ruby`,
`powershell`, and `cargo` sets;
configured `bazel` sets widen as described under `dx bump`.

```sh
bazel run @rules_dx//:dx -- update --check frontend
bazel run @rules_dx//:dx -- update --apply worker
bazel run @rules_dx//:dx -- update --apply web:@types/node
bazel run @rules_dx//:dx -- security apps/frontend
bazel run @rules_dx//:dx -- license --output=json
```
