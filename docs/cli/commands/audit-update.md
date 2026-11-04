# Audit And Update Commands

## `dx security` And `dx license`

```text
dx security [--here] [--affected] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
dx license [--here] [--affected] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
```

`dx security` checks secrets plus dependency vulnerabilities. `dx license`
checks dependency licenses. No scope means `//...`. Use `--here` for the
current dir tree, or `--affected` for the sets that own targets your git
changes reach (see [Scope Defaults](scope-defaults.md)). With no changes, or
for `dx license` no owned set, they print `no affected targets` and exit `0`.
When files changed but no set owns them, `dx security` runs only the secrets
scan. Neither command changes files. Bazel options do not apply.

`dx security` reads the lockfile of each set it owns and matches it against
`.dx/advisory/<family>.json`. Sets: `cargo`, `go`, `maven`, `npm`, `npm-adopt`,
`npm-adopt-polyglot`, `npm-tools`, `nuget`. The `npm-adopt`,
`npm-adopt-polyglot`, and `npm-tools` sets share the `npm` snapshot.
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
and `nuget`. A package under a path in the table selects
that path's set, so `//cli/cli` selects `cargo`. Each set then reads its own
lockfile, so a scope narrows which sets run, never which packages inside a
lockfile.

```text
//cli/..., //tools/docs/..., //env/..., //generation/..., //quality/..., //rust/...      cargo
//rust/tests/fixtures/adopt-rust/...                                                             cargo
//csharp/..., //fsharp/..., //third_party/dotnet/...                                  nuget
//csharp/tests/fixtures/adopt-csharp/..., //fsharp/tests/fixtures/adopt-fsharp/...                              nuget
//go/..., //third_party/go/...                                                         go
//go/tests/fixtures/adopt-go/...                                                               go
//java/..., //kotlin/..., //scala/..., //third_party/jvm/...                          maven
//java/tests/fixtures/adopt-java/..., //kotlin/tests/fixtures/adopt-kotlin/..., //scala/tests/fixtures/adopt-scala/...    maven
//javascript/..., //typescript/..., //astro/..., //svelte/..., //vue/..., //mdx/...   npm
//quality/tools/javascript/...                                                        npm-tools
//javascript/tests/fixtures/adopt-js-ts/...                                                            npm-adopt
//examples/adopt-polyglot/...                                                         npm-adopt-polyglot
//examples/adopt-polyglot/...                                                         uv-adopt-polyglot
//python/..., //python/tests/fixtures/hello/...                                       uv
//python/tests/fixtures/adopt-python/...                                                           uv-adopt
//quality/tools/python/...                                                            uv-tools
```

A scope that owns no set exits `2` and names the audited sets. `//...` and
`MODULE.bazel` select every set. The four built-in `uv` sets resolve but have no
snapshot, so they report `no advisory coverage`. Configured `uv` sets are
audited against the `pypi` snapshot. A path the table does not name, and that
has no path from the table under it, owns no set.

`dx security` and `dx license` read `pnpm-lock.yaml`, `package-lock.json`, and
`yarn.lock` for the `npm` set, and the lockfile of every other set. Each finding names the lockfile it
was read from. `pnpm-lock.yaml` is read at `lockfileVersion` 5.x, 6.0, and 9.0; a
package key that cannot be resolved fails the command with a lock parse error
naming the key.

Every key of `security.toml` and `licenses.toml` is listed in the
[Config Reference](config.md).

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

Every bad entry is reported on its own line, as `<file>:<line>: <entry>: <problem>`:

```text
security.toml:12: GHSA-aaaa-bbbb-cccc on some-package: risk exception expired 2026-01-01 (audit date 2026-11-03); renewal needs review
```

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

`dx security` scans secrets with the pinned `@rules_dx//quality/artifacts:gitleaks` artifact.
When `DX_GITLEAKS_BIN` holds an absolute path, dx runs that file. When it is
unset, dx resolves the label through Bazel in the workspace; under `--offline`
that resolution adds `--repository_disable_download`. A relative path is
rejected and `PATH` is never searched. If no artifact is found, the run exits
`1` with `secrets auditor unavailable` and prints this command, which exports
the path in the current shell:

```text
export "$(bazel run @rules_dx//cli/tool_resolve -- --label @rules_dx//quality/artifacts:gitleaks --var DX_GITLEAKS_BIN --print)"
```

`dx license` does not read it.

Put `#:schema https://raw.githubusercontent.com/devhusdk/rules_dx/main/cli/audit/schema/security.schema.json` at the top of `security.toml` to get editor completion and checks. For `licenses.toml`, use `.../licenses.schema.json` in the same place.

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

An entry may also set `text = "<workspace-relative path>"` to a vendored license
file. It implies `text_present`, must be a `/`-separated path with no leading
`/`, `\`, or empty, `.`, or `..` components, and feeds the `notice` report. Older
`dx` binaries reject a `licenses.toml` that uses `text`.

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
`version`, and `license`. A family with no findings emits an info `notice`
with code `audit_security_clean` or `audit_license_clean`. A failed family emits an `error` event. Its code is `advisory_refresh_failed` for a
missing, stale, or corrupt advisory snapshot, `offline_required` when
`--offline` blocks a refresh, and `audit_failed` for findings or any other
incomplete assessment. Reports: `dx security` and `dx license` write
`--report sarif=<dest>`. Repeat the flag for more files. Use `-` for stdout.
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
bazel run @rules_dx//cli/tool_resolve -- --label @rules_dx//quality/artifacts:gitleaks --var DX_GITLEAKS_BIN --print
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
dx update [--apply] [--offline] [--frozen] [selector ...]
dx update [--apply] [--offline] [--frozen] [--floor --reason=<text>] <set:package> <version>
```

Updates dependencies per set through the qualified resolvers. The sets come
from the hubs in the root `MODULE.bazel`; see [Dependency sets](#dependency-sets).
Checks by default; `--apply` writes updates. In `rules_dx` the sets are
`bundle`, `dx_crates`, `dx_dokka`, `dx_npm`, `dx_npm_tools`, `dx_pypi-hello`,
`dx_pypi-python`, `dx_pypi-python-service`, `dx_pypi-sqlfluff`,
`dx_pypi-tooling`, `dx_quality_crates`, `dx_scalafix`, `go`, `maven`,
`npm_node_app`, and `nuget`. Selectors are `set`, `set:package`, or a
label/path. `go` and `ruby` pins are manual: `update` reports them pinned and
changes nothing. `ruby` pins come from `bundle lock` on the seed host, and `go`
pins track Gazelle and widen through `dx update go:<module> <version>`.
Without `--apply`, `dx update` validates the selected sets and writes nothing. A `uv` set runs a
read-only lockfile check and reports current, and a stale lock reports failed.
`go` and `ruby` sets report pinned. Every other resolvable set reports
unsupported and fails the run; refresh those with `dx update --apply <set>`.
No selector checks all sets, so a bare `dx update` reports the unsupported sets
and exits `1`. Checks never take a [build slot](README.md#build-slots).
In a rules_dx checkout, `tools/dx update --apply dx_crates` repins the lock itself
when `//:dx` no longer builds against it.
`--apply` takes one only for `cargo`, `maven`, `npm`, and `nuget` sets,
whose updaters run Bazel. `update` never touches the Bazel preset. The preset
is three generated files under `tools/bazelrc/`, imported in this order at the
top of the root `.bazelrc`, so later lines in `.bazelrc` override them:

- `correctness.bazelrc`: Bzlmod, failure output, coverage, runfiles, and telemetry opt-out flags.
- `performance.bazelrc`: the host compilation mode and the `dx_*` compilation configs.
- `resources.bazelrc`: the memory, CPU, and cache limits listed below.

Every flag has a one-line comment above it. Verify the files with
`bazel run //tools/bazelrc:preset_update -- --verify-only` and regenerate them
with `bazel run //tools/bazelrc:preset_update`.
Consumer workspaces get the same files under `.dx/bazelrc/`, written by dx before each command.

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
resolver would rewrite them. A check writes nothing, so `--frozen` does not
change it. Combine both flags for both policies.
`--fail-on`, `--report`, and Bazel options do not apply.
`--bazel-startup-option` applies to the cargo, maven, npm, and nuget sets,
which run Bazel.
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
`git checkout -- <locks>` to discard kept successes. After a check it is
`dx update <sets>`. The restore command names the locks of the failed sets.
Exit codes: 0 success, 2 usage or scope errors, 1 when any set is unsupported,
failed, or blocked, or on an operational failure.

```sh
bazel run @rules_dx//:dx -- update
bazel run @rules_dx//:dx -- update --apply go
bazel run @rules_dx//:dx -- update --apply bundle
bazel run @rules_dx//:dx -- update --apply dx_pypi-python dx_pypi-tooling
bazel run @rules_dx//:dx -- update --apply dx_npm_tools
```

## Widen or add one package

```text
dx update [--apply] [--offline] [--frozen] [--floor --reason=<text>] <set:package> <version>
```

`dx update` with exactly one `set:package` and one version widens that
requirement to the version, or declares it when missing. A version starts with
a digit, `v` plus a digit, or is a commit SHA. Checks by default; `--apply`
writes. Without `--apply`, it exits `0` when the manifest already declares the
version and exits `1` with `update drift` when a widen is pending. The set is a
[dependency set](#dependency-sets) from `MODULE.bazel`, or one of the built-in
sets `bazel` and `github-actions`. Any other set name exits `2` and lists the
valid names. A package the manifest does not declare yet is
added at that exact version. `github-actions`, `bazel:.bazelversion` and
`uv` sets cannot add; the run fails without writing.

A `bazel` module has two versions. The floor, in its `bazel_dep`, is the oldest
version consumers may resolve. The tested version, in a root-only
`single_version_override` next to it, is what the root builds with. By default
a `bazel` widen edits the tested version. It rewrites the literal `version` of
the module's `single_version_override` and keeps its patches. When there is no
override, or the override has no `version`, it rewrites the `bazel_dep`
instead. `--floor --reason <text>` edits the floor: it rewrites the
`bazel_dep` version and replaces the `# floor: <text>` comment lines directly
above it. Every other byte is kept. These are refused with exit `2`, and nothing
is written:

- `--floor` without `--reason`, or `--reason` without `--floor`.
- `--floor` on any set other than a `bazel_dep` (including `.bazelversion`).
- A floor above the tested version.
- A tested version below the floor.

Duplicate declarations, a missing or computed version or name, and a
`MODULE.bazel` that does not parse fail without writing. `maven` widens and
a plain `dx update` never touch a `bazel_dep` floor or an override. A `github-actions` version is
the resolved commit SHA, never a tag, and the pin is rewritten in every
`.github/workflows` file that declares the action.

A [dependency set](#dependency-sets) is selected as `<set>:<package>`. `uv` and `npm` sets widen `pyproject.toml` or `package.json`
and refresh their lock. A `cargo` set widens the crate in its
manifests and repins the whole hub with `dx update --apply <set>`. A
`maven` set edits only the literal coordinate in its own `maven.install` and
runs `@<set>//:pin`. A `nuget`
set edits `paket.dependencies` only; nothing re-resolves `paket.lock`, so the
report names the next steps: `paket update`, then `dx update <set>`. A
`go` set edits only the `require` line of its single `go.mod`, then regenerates
`go.sum` through the same staged `go mod download all` run as `dx update`. If go fails,
or go would change `go.mod` (for example a widen that needs new indirect requires),
the widen stays, `go.sum` is untouched, and the report names
`go mod tidy` followed by `dx update <set>`. Each
refusal that `dx update` makes for the set (missing files, `--offline`,
`--frozen`, host environment) happens before any write. `ruby` sets fail with
`update is not supported`.

`cargo`, `go`, `maven`, `npm`, and `nuget` refresh automatically through
`dx update --apply <set>`. A failed refresh keeps the widen and exits `1`. `bazel`
and `github-actions` are file-only and refresh nothing. Review the pin diff
and run `bazel build //...`.

A version that is already declared reports `update_no_change`, writes and
refreshes nothing, and exits `0`, also under `--frozen` or `--offline`. Only
manifests whose pin changes are written. When a widen is pending, a set that
needs a refresh under `--offline` fails before widening with
`offline_required`, and `--frozen` fails before widening with
`frozen_locked`.

A missing package is inserted in the same manifest and shape the widen edits:
a `bazel_dep` after the last one, a `[dependencies]` entry, a `require` line,
a `maven.install` coordinate, a `dependencies` entry in `package.json`, or a
`nuget` line in `paket.dependencies`. It then refreshes like a widen. Without
`--apply`, it exits `1` with `update pending`.

`--fail-on`, `--report`, and Bazel options do not apply.
`--bazel-startup-option` applies to the Bazel run that refreshes the lock.
Output: `--output text|json`. JSON emits an `add_declared` notice for an insert, a `update_widened` notice for each
written manifest and an `update_set_success` notice after the lock refresh.
Exit codes: 0 success, 2 usage or scope errors or an unknown set, 1 a pending
widen without `--apply` or an operational failure.

```sh
bazel run @rules_dx//:dx -- update --apply dx_crates:anyhow 1.0.100
bazel run @rules_dx//:dx -- update --apply go:github.com/google/go-cmp 0.7.0
bazel run @rules_dx//:dx -- update --apply bazel-modules:rules_cc 0.3.0
bazel run @rules_dx//:dx -- update --apply npm:vue 3.5.42
```

## Dependency sets

`update`, `security`, and `license` read dependency sets from the hubs
in the root `MODULE.bazel` and the files it pulls in with `include`. Each hub
is one set:

| Hub | Set name | Manifests | Locks |
|---|---|---|---|
| `crate.from_cargo` with `cargo_lockfile` | `name` | `manifests` | `cargo_lockfile`, `lockfile` |
| `maven.install` with `lock_file` | `name`, default `maven` | `MODULE.bazel` | `lock_file` |
| `npm.npm_translate_lock` with `pnpm_lock` | `name` | `package.json` beside the lock | `pnpm_lock` |
| `uv.project` | `<hub_name>-<pyproject directory name>` | `pyproject` | `lock` |
| `go_deps.from_file` with `go_mod` | `go` | `go_mod` | `go.sum` beside it |
| `ruby.bundle_fetch` | `name`, default `bundle` | `gemfile` | `gemfile_lock` |
| rules_dotnet `paket` extension | `nuget` | `paket.dependencies` | `paket.lock`, `paket.<group>.bzl` |

A `uv.project` whose `pyproject` is at the workspace root is named
`<hub_name>`. Each set's scopes are the directories of its manifests and
locks, except `MODULE.bazel`. A path or label that names a manifest or lock,
such as `//:cargo-bazel-lock.json`, selects only the set that owns it. A
workspace with none of these hubs makes `dx update` exit `2` with
`no dependency sets`. `dx security` in a workspace other than rules_dx exits
non-zero with the same message and an example hub. Hub arguments must be
literal; a computed one, two sets with one name, and two sets that own one
manifest or lock fail with exit `2` before anything runs.

An `npm` set locks the `package.json` beside the `pnpm_lock` of one
`npm.npm_translate_lock` hub with the workspace's own pnpm. The set is named
by the tag's `name`, and hubs without `pnpm_lock` are skipped. The root
`MODULE.bazel` must also load pnpm:

```starlark
bazel_dep(name = "aspect_rules_js", version = "3.5.2")

npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(
    name = "web",
    pnpm_lock = "//apps/web:pnpm-lock.yaml",
)

pnpm = use_extension("@aspect_rules_js//npm:extensions.bzl", "pnpm")
use_repo(pnpm, "pnpm")
```

`dx update --apply` runs
`bazel run @pnpm//:pnpm -- --dir <absolute set directory> install --lockfile-only`
from the workspace root and writes that set's `pnpm-lock.yaml` only; no
`node_modules` is created. `set:package` selectors such as
`web:@types/node` run `update <packages> --lockfile-only` instead.
`dx update` runs `install --lockfile-only --frozen-lockfile`, writes
nothing, and fails the set with `updater exited <code>` when the lock is stale.
A check with `--offline` adds pnpm `--offline` and Bazel
`--repository_disable_download`. `--apply --offline` fails with
`offline_required`, `--apply --frozen` fails with `frozen_locked`, and
A check with a `set:package` selector is unsupported. `--bazel-startup-option`
values reach the `bazel` call. pnpm runs with `npm_config_globalconfig` and
`npm_config_userconfig` set to `/dev/null` (`NUL` on Windows), so user and
global `.npmrc` files are ignored; a `.npmrc` in the set directory still
applies. The set is unsupported before pnpm starts when `locks` does not list
`<set directory>/pnpm-lock.yaml` or when a parent directory inside the
workspace holds a `pnpm-workspace.yaml` that makes the set a member. pnpm uses
the nearest workspace file, so a set with its own `pnpm-workspace.yaml` is its
own root and is accepted. A set with none of its own under a parent whose
`packages` is exactly `["."]` is not a member of that workspace either; dx
accepts it and passes `--ignore-workspace` to pnpm so the parent lock stays
untouched.

`dx security` matches every `pnpm-lock.yaml` package against
`.dx/advisory/npm.json`; a missing snapshot fails the set with
`failed to assess <name>`. `dx license` reports `UNKNOWN` for npm packages
unless `licenses.toml` declares them with `[[inventory]]` and `set = "npm"`.

A `ruby` set is pinned: dx reads its committed lock and never resolves it.
Each `ruby.bundle_fetch` tag is one set, named by the tag's `name` (default
`bundle`):

```starlark
ruby = use_extension("@rules_dx//ruby:extensions.bzl", "ruby")
ruby.bundle_fetch(
    name = "gems",
    gemfile = "//apps/rb:Gemfile",
    gemfile_lock = "//apps/rb:Gemfile.lock",
)
```

`gemfile_lock` must be the lock beside the `Gemfile`; otherwise `dx update`
reports the set unsupported and names the missing path. `dx update` and
`dx update --apply` report it as pinned, also under `--offline` and
`--frozen`, and start no process. A `set:package` selector is unsupported:
regenerate `Gemfile.lock` with `bundle lock`. `dx security` matches every
`Gemfile.lock` gem against `.dx/advisory/rubygems.json`; a missing snapshot
fails the set with `failed to assess <name>`, and `--offline` fails it with
`offline_required`. `dx license` reports `UNKNOWN` for gems unless
`licenses.toml` declares them with `[[inventory]]` and `set = "ruby"`.

A `cargo` set locks the `Cargo.toml` files of one rules_rust crate_universe
hub with the workspace's own rules_rust; dx ships no cargo. Each
`crate.from_cargo` tag with a literal `cargo_lockfile` is one set, named by the
tag's `name`. Its manifests are the hub's `manifests`, and its locks are
`cargo_lockfile` and `lockfile`. Set `lockfile` so the hub keeps its
cargo-bazel lock in the set:

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

One `Cargo.toml` must sit beside `Cargo.lock`. dx refuses the set as
unsupported (exit `1`) unless the tag is imported with `use_repo` (renamed or
not). The hub is fetched under the name `use_repo` gives it
(`use_repo(crate, crates = "dx_crates")` is fetched as `@crates`), while
`CARGO_BAZEL_REPIN_ONLY` carries the hub's own `name` (`dx_crates`), which is
what crate_universe compares it with.

`dx update --apply` runs `bazel fetch --repo=@<visible name of the hub>` from the workspace
root with `CARGO_BAZEL_REPIN=workspace`, `REPIN=workspace`,
`CARGO_BAZEL_REPIN_ONLY=<repository>`, and `CARGO_BAZEL_ISOLATED=true`, so
crate_universe re-resolves `Cargo.toml` into that hub's `Cargo.lock` and
cargo-bazel lock only, without upgrading other crates. `dx update`
runs the same command with `CARGO_BAZEL_REPIN=false` and `REPIN=false`;
crate_universe then fails the set with `updater exited <code>` when the lock
no longer matches the manifest. Both variables are always set, so a `REPIN`
exported in the shell does not turn a check into a write. A check with `--offline`
adds `--repository_disable_download`, `--apply --offline` fails with
`offline_required`, and `--apply --frozen` fails with `frozen_locked`.
`--apply <set>:<crate>` sets `CARGO_BAZEL_REPIN` and `REPIN` to the crate name,
which crate_universe runs as `cargo update --package <crate>` before it
regenerates the cargo-bazel lock. Select one crate per cargo set per run;
crate names crate_universe reads as repin modes (`all`, `workspace`, `false`,
`0`, `off`, and similar) are refused, and a name with several locked versions
fails with `updater exited <code>`. A check with a package is unsupported:
select the set without a package.
`--bazel-startup-option` values reach the `bazel` call. Bazel evaluates
every `crate.from_cargo` hub of the workspace together, so a stale hub fails
the check and update of every `cargo` set; update the stale set first.

Every check that runs Bazel (`npm`, `cargo`, and `maven`) keeps the bytes of the set's
manifests and locks. If the check changed any of them, dx restores them and
fails the set with `check_wrote: <paths>`. A `crate.from_cargo` hub without
`lockfile` always repins, so its check ends in `check_wrote`; declare
`lockfile` and list it in `locks`.

`dx security` matches every `Cargo.lock` crate from crates.io against
`.dx/advisory/cargo.json`; a missing snapshot fails the set with
`failed to assess <name>`, and `--offline` fails it with `offline_required`.
`dx license` reads crate licenses from a declared `*.json` lock, and a
`licenses.toml` `[[inventory]]` entry with `set = "cargo"` overrides them.

A `maven` set pins one rules_jvm_external hub with the workspace's own
rules_jvm_external (7.1 or later); dx ships no coursier. Each `maven.install`
tag with a literal `lock_file` is one set, named by the tag's `name` (default
`maven`). The hub needs a `use_repo` entry without a rename. Several maven
sets share `MODULE.bazel` as their manifest; each still owns its lock.
Create the lock file (empty is enough) before the first `--apply`:

```starlark
maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = ["com.google.guava:guava:33.4.0-jre"],
    lock_file = "//third_party:maven_install.json",
)
use_repo(maven, "maven")
```

`dx update --apply` runs `bazel run @<repository>//:pin` from the workspace
root with `RULES_JVM_EXTERNAL_REPIN=1` and nothing else; `REPIN` is never set
because crate_universe also reads it. `dx update` runs
`bazel fetch --repo=@<repository>` with no repin variable, so a stale lock
fails the set with `updater exited <code>`. That only works while
`fail_if_repin_required` is true (the default), and a check is refused as
unsupported when it is `False` or computed, when the lock is empty or lacks
the `__INPUT_ARTIFACTS_HASH` and `__RESOLVED_ARTIFACTS_HASH` signatures
(upstream only prints a note for those), or when the host sets `REPIN` or
`RULES_JVM_EXTERNAL_REPIN` (any value, even `false`, stops the failure).
Host `COURSIER_MIRRORS`, `COURSIER_REPOSITORIES`, `COURSIER_CREDENTIALS`,
`COURSIER_CONFIG_DIR`, and `SCALA_CLI_CONFIG` are refused for both modes
because they redirect resolution. A `--repo_env=REPIN=...` in a bazelrc, and
`~/.m2/settings.xml`, cannot be detected; the `check_wrote` guard is the
backstop for the former. A check with `--offline` adds
`--repository_disable_download`, `--apply --offline` fails with
`offline_required`, and `--apply --frozen` fails with `frozen_locked`.
`maven` sets take no `set:package`: pins are exact in `MODULE.bazel`.
`dx update <set>:<group>:<artifact> <version>` widens one coordinate; `dx security` matches the lock's `artifacts`
against `.dx/advisory/maven.json`, failing the set with `failed to assess
<name>` when the snapshot is missing, and `dx license` reads
`[[inventory]]` entries with `set = "maven"`.

A `nuget` set regenerates the `paket2bazel` output of rules_dotnet. dx finds it
from the `paket` extension the root `MODULE.bazel` uses: either a
`paket.<group>_extension.bzl` file, or an extension named `paket` whose file
loads the generated `paket.<group>.bzl` files. The set's manifest is the
`paket.dependencies` in the parent of that folder, and its locks are the
`paket.lock` beside it plus every `paket.<group>.bzl`. dx ignores the
`paket.<group>_extension.bzl` files `paket2bazel` also writes. The root
Every `source` in `paket.dependencies` must be http(s). Violations are
`unsupported`.

```starlark
paket = use_extension("//third_party/dotnet/deps:paket.main_extension.bzl", "main_extension")
```

dx copies `paket.dependencies` and `paket.lock` into a scratch directory outside
the workspace and runs
`bazel run @rules_dx//dotnet:paket2bazel -- --dependencies-file <scratch>/paket.dependencies --output-folder <scratch>/out`
with the Bazel startup options but no rules_dx settings, then removes the scratch
directory. `paket2bazel` never resolves: it renders `paket.lock`, which dx never
writes. A check compares the generated files byte for byte with the declared
locks; a difference, an undeclared generated file, or a declared file that was not
generated fails the set and lists the paths. `--apply` writes only the declared
files that differ, and writes nothing when an undeclared or missing file turns up.
`--apply --frozen` runs, because the lock cannot change. `--offline` fails with
`offline_required` in both modes: Paket has no offline switch, so dx cannot prove
it stays off the network. `nuget` sets take no `set:package`: pins are exact in
`paket.lock`; edit `paket.dependencies`, run `paket update`, then `dx update
<set>`. `dx update <set>:<package> <version>` edits `paket.dependencies` only; `dx security` matches `paket.lock` against
`.dx/advisory/nuget.json`, failing the set with `failed to assess <name>` when the
snapshot is missing, and `dx license` reads `[[inventory]]` entries with
`set = "nuget"`.

A `go` set refreshes `go.sum` for the `go.mod` of a `go_deps.from_file` hub.
The `go.mod` must parse and carry no local-path `replace` (the scratch copy
cannot see the replaced directory), and the root `MODULE.bazel` must have
`bazel_dep(name = "rules_go")` with no `repo_name`. Violations are `unsupported`.

```starlark
go_deps = use_extension("@gazelle//:extensions.bzl", "go_deps")
go_deps.from_file(go_mod = "//third_party/go:go.mod")
```

dx copies `go.mod` and `go.sum` into `<scratch>/out`, outside the workspace, and runs
`bazel run @rules_go//go -- -C <scratch>/out mod download all` with the Bazel startup
options but no rules_dx settings, then removes the scratch directory. `go mod download
all` walks the module graph, which is the contract Gazelle's `go_deps.from_file` needs:
a `go.sum` entry for every module in the build list. The run is forced to
`GOENV=off`, `GOWORK=off`, `GOTOOLCHAIN=local`, `GOFLAGS=-modcacherw`, and an empty
`GOMODCACHE`, `GOPATH` and `GOCACHE` under the scratch directory. It never edits
`go.mod`; a changed `go.mod` fails the set. A check compares the generated `go.sum`
with the committed one and fails on any difference; `--apply` writes `go.sum` only when
the bytes differ. `go.sum` entries are added, never pruned: pruning is `go mod tidy`,
which stays your call. The run always uses Go's default public proxy and checksum database; dx does not read
`go_deps.config(go_env = ...)`, so private modules are not supported, and a root
`MODULE.bazel` that mentions `go_env` makes the set `unsupported`. A host `GOPROXY`, `GOPRIVATE`,
`GONOPROXY`, `GONOSUMDB`, `GONOSUMCHECK`, `GOSUMDB`, `GOINSECURE`, `GOVCS` or `GOAUTH`
makes the set `unsupported` before anything launches, because host values can weaken checksum
verification or leak private module names. Every
run starts from an empty module cache, so `--offline` fails with `offline_required` in
both modes; `--apply --frozen` fails with `frozen_locked` because `go.sum` is a lock
write, while a check with `--frozen` runs. The `-C` flag needs a Go SDK of 1.20 or newer; an
older SDK fails the set. `go` sets take no `set:package`: sums cover the whole module
graph, so edit the `require` in `go.mod` and run `dx update <set>`, or let
`dx update <set>:<module> <version>` widen the `require` and refresh `go.sum`; `dx security` matches the requirements in `go.mod` against
`.dx/advisory/go.json`, failing the set with `failed to assess <name>` when the snapshot
is missing.

Selectors are set names (`dx_crates`), `set:package` (`dx_npm:vue`),
or scopes (`quality/tools/python`, `//quality/tools/python/...`, `//...`). No
selector selects every set. A scope selects every set whose scope covers
it; overlapping scopes select every owner and the run names them all. The
scope `.` owns the root package only: `//:x` and `//:` select the sets scoped
`.`, while `//pkg:x` and `//pkg/...` never do; `//...` selects every set.
Unknown names and unowned paths fail with exit `2`.

`dx update` and `dx update` run the pinned uv `0.13.7` and the pinned
interpreter. When `DX_UV_BIN` holds an absolute path, dx runs that file; when
it is unset, dx resolves `@dx_tools//:uv` through Bazel in the workspace.
`DX_UV_PYTHON` works the same way for
`@rules_dx//cli/update:python_interpreter`. A relative path is rejected and
never falls back to Bazel, and `PATH` is never searched. A uv that does not
report `0.13.7` is rejected. When neither source gives a usable tool the set
fails with `backend_unavailable` and the run exits `1`; JSON reports an
`error` event with that code. It is never reported current. Under `--offline`
the Bazel resolution adds `--repository_disable_download`, so cached
repositories still resolve. Runs with no uv set and rejected applies launch no
Bazel. These commands export the paths in the current shell:

```text
export "$(bazel run @rules_dx//cli/tool_resolve -- --label @dx_tools//:uv --var DX_UV_BIN --print)"
export "$(bazel run @rules_dx//cli/tool_resolve -- --label @rules_dx//cli/update:python_interpreter --var DX_UV_PYTHON --print)"
```

`dx update --apply` runs `uv lock` in the set directory and writes that set's
lock only. `dx update` runs `uv lock --check`, writes nothing, and
fails the set with `updater exited 1` when the lock is stale. Both pass
`--no-config --no-cache --no-python-downloads --python <DX_UV_PYTHON>` and
start uv with an empty environment, so `HOME`, `XDG_CONFIG_HOME`, `UV_*`
variables, and user `uv.toml` files are ignored. A check with `--offline` adds
`--offline`; a lock that needs a download to verify then fails the set with
`updater exited 2`. `--apply --offline` fails with `offline_required` and
`--apply --frozen` fails with `frozen_locked`; a check with `--frozen` runs.
A `uv.toml` in the set directory or any parent inside the workspace, or a
`[tool.uv]` key other than `sources`, `workspace`, `dev-dependencies`,
`default-groups`, `conflicts`, `package`, `managed`, `override-dependencies`,
or `constraint-dependencies`, makes the set unsupported before uv starts.
Selective `set:package` updates are unsupported for `uv`: the set refreshes
as a whole.
`dx security` and `dx license` assess the set's locks. `dx security`
matches every `uv.lock` package, direct and transitive, against
`.dx/advisory/pypi.json`, using PEP 440 versions and PEP 503 names. Workspace
members are skipped. Packages from git, path, url, directory, or a registry
other than pypi.org are reported as unassessed and the run exits `1`, as does
an unparseable locked version. A missing lock fails the family. A stale,
missing, or failed advisory snapshot fails the set with
`failed to assess <name>`, and `--offline` fails with `offline_required` when
the snapshot is missing. A `security.toml` exception uses `set = "<name>"` and a
PEP 440 `versions` range. `dx update <set:package> <version>` refuses `ruby` sets.

```sh
bazel run @rules_dx//:dx -- update dx_pypi-tooling
bazel run @rules_dx//:dx -- update --apply dx_npm_tools
bazel run @rules_dx//:dx -- update --apply dx_npm:@types/node
bazel run @rules_dx//:dx -- security quality/tools/python
bazel run @rules_dx//:dx -- license --output=json
```
