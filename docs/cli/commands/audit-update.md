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

Create or refresh the snapshots from the workspace root:

```text
bazel run @rules_dx//cli/advisory_prep --
```

A snapshot is fresh only on the day it was prepared. A missing or stale
snapshot fails with `advisory_refresh_failed` and names this command. See
[Advisory Snapshots](../../github-ci.md#advisory-snapshots) for its flags.

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

Output: `--output text|json`. Text prints one line per finding:
`<path>: <severity>: <message> [<tool>/<rule>]`. The lines go to stderr when a
report goes to stdout. JSON emits one `diagnostic` event per finding with
`tool`, `rule`, `path`, `severity`, and `message`. A vulnerability has tool
`vuln`, rule `vuln/<advisory>`, the lockfile as `path`, and a message that
names `<package>@<version>`, the advisory, its severity, and the fixed
version. A license finding has tool `license`, rule `license/<license>`, and a
message that starts with `<package>@<version> license <license>`. A failed
family emits an `error` event. Its code is `advisory_refresh_failed` for a
missing, stale, or corrupt advisory snapshot, `offline_required` when
`--offline` blocks a refresh, and `audit_failed` for findings or any other
incomplete assessment. Reports: `dx security` writes
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
Combine both flags for both policies. Exit codes: 0 success, 2 usage or scope
errors, 1 findings at or above `--fail-on`, an incomplete assessment, or an
operational failure.

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
whose updaters run Bazel. `update` never touches the Bazelrc
preset fragment: verify it with
`bazel run //tools/bazelrc:preset_update -- --verify-only` and regenerate it
with `bazel run //tools/bazelrc:preset_update`.

The preset sets these resource defaults:

- `startup --host_jvm_args=-XX:MaxHeapFreeRatio=30` and `-XX:MinHeapFreeRatio=10`: idle servers return heap to the OS.
- `startup --host_jvm_args=-XX:MaxRAMPercentage=25`: the server heap is up to 25% of RAM, or of the cgroup limit when there is one.
- `common --local_resources=memory=HOST_RAM*.5` and `common --local_resources=cpu=HOST_CPUS*.75`: actions use half the RAM and three quarters of the cores.
- `startup --max_idle_secs=900`: a server shuts down after 15 minutes idle.
- `startup --shutdown_on_low_sys_mem`: an idle server shuts down when system memory runs low. It needs `--max_idle_secs`.
- `common --experimental_disk_cache_gc_max_age=14d`: prunes disk cache entries older than 14 days. It only applies when a disk cache is set.
- `common --experimental_disk_cache_gc_max_size=20G`: caps the disk cache at `20G`. It only applies when a disk cache is set.
- `common --repo_contents_cache_gc_max_age=60d`: keeps repo contents for 60 days.
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
Output: `--output text|json`. `json` reports one event per set plus
`command_finished`. Text prints a per-set line for every failure plus a summary
count of updated, current, pinned, unsupported, failed, and blocked sets.
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
writing. A `github-actions` version is
the resolved commit SHA, never a tag, and the pin is rewritten in every
`.github/workflows` file that declares the action.

`cargo`, `go`, `maven`, `npm`, and `nuget` refresh automatically through
`dx update --apply <set>`. A failed refresh keeps the widen and exits `1`. `bazel`
and `github-actions` are file-only and refresh nothing. Review the pin diff
and run `bazel build //...`.

A set that needs a refresh under `--offline` fails before widening with
`offline_required`. `--frozen` always fails before widening with
`frozen_locked`, because a bump changes resolution by definition. A major
bump also needs `dx migrate --from <old> --to <new>`.

`--check`, `--fail-on`, `--report`, and Bazel options do not apply.
`--bazel-startup-option` applies to the Bazel run that refreshes the lock.
Output: `--output text|json`. Exit codes: 0 success, 2 usage or scope errors,
1 a pending widen without `--apply` or an operational failure.

```sh
bazel run @rules_dx//:dx -- bump --apply cargo:anyhow 1.0.100
bazel run @rules_dx//:dx -- bump --apply go:github.com/google/go-cmp 0.7.0
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
(`uv`, `npm`, or `bazel`), workspace-relative `manifests`, `locks`, and
owning `scopes`. Manifests are named for the ecosystem: `pyproject.toml` for
`uv`, `package.json` for `npm`, `MODULE.bazel` for `bazel`; any other name
fails with exit `2`. Two sets may share one ecosystem. Paths stay inside the workspace: absolute
paths, `..`, and `\` fail. Two writable sets cannot own the same manifest
or lock; mark shared read-only ownership with `writable = false`.
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
are not listed. Archives declared only through module extension tags are not
listed.

No advisory source maps Bazel modules. A run with only `bazel` sets exits
`0` with `clean; no advisory coverage for <name>` and an info diagnostic with
the input counts. Read the diagnostic: nothing was checked for advisories. A missing or unreadable lock, a
`bazel_dep` absent from the lock, a declared version above the selected one,
an override still resolved from a registry, a `git_override` without a
commit, an `include` that cannot be read or points outside the workspace,
an archive without `sha256` or `integrity`, and a computed
`module_name` fail the set with `failed to assess <name>` and exit `1`.

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
changes nothing; change pins with `dx bump` or `bazel mod deps`.

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
it is never reported current. In CI, `bazel run @rules_dx//cli/tool_resolve --
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
PEP 440 `versions` range. `dx bump` names a configured set only
to refuse it: widening is unsupported for configured ecosystems.

```sh
bazel run @rules_dx//:dx -- update --check frontend
bazel run @rules_dx//:dx -- update --apply worker
bazel run @rules_dx//:dx -- update --apply web:@types/node
bazel run @rules_dx//:dx -- security apps/frontend
bazel run @rules_dx//:dx -- license --output=json
```
