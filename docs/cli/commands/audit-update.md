# Audit And Update Commands

## `dx security` And `dx license`

`dx security` checks secrets and dependency vulnerabilities. `dx license` checks dependency licenses. Neither changes files.

```sh
tools/dx security
tools/dx security --affected
tools/dx security --offline --fail-on warning
tools/dx license --fail-on error //...
tools/dx security --report sarif=security.sarif quality/tools/python
```

```text
dx security [--here] [--affected] [--offline] [--frozen] [--fail-on info|warning|error|uncovered] [--report sarif=<dest>] [scope...]
dx license  [--here] [--affected] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
```

| Flag | What it does |
| --- | --- |
| `--affected` | Runs the sets that own targets your git changes reach. Not with scopes or `--here`. |
| `--here` | Uses the current directory tree. Not with scopes. |
| `--offline` | Uses pinned cached snapshots. Fails with `offline_required` when missing. |
| `--frozen` | Forbids manifest and lock resolution changes. |
| `--fail-on info\|warning\|error` | Fails at this severity. |
| `--fail-on uncovered` | `dx security` only: also fails when a known lock file (`mix.lock`, `pubspec.lock`, `poetry.lock` and others) has no dependency set. Without it, each such file prints one warning line. An `[[uncovered]]` entry whose `set` is the file path silences it. |
| `--report sarif=<dest>` | Writes SARIF. Repeatable. `-` is stdout. |
| `--output text\|json` | Sets the output format. |

Scopes: dependency-set, package or target selectors. No scope means `//...`. See [Scope Defaults](scope-defaults.md). With `--affected` and no changes, or no owned set for `dx license`, they print `no affected targets` and exit `0`. With changes that no set owns, `dx security` runs only the secrets scan. Bazel options do not apply.

A scope selects sets by path. A `...` scope selects the nearest set above it in the table plus every set under it. A package selects its path's set. Each set reads its whole lockfile. A scope that owns no set exits `2`.

```text
//cli/..., //tools/docs/..., //env/..., //generation/..., //quality/..., //rust/...    cargo
//csharp/..., //fsharp/..., //third_party/dotnet/...                                  nuget
//go/..., //third_party/go/...                                                         go
//java/..., //kotlin/..., //scala/..., //third_party/jvm/...                          maven
//javascript/..., //typescript/..., //astro/..., //svelte/..., //vue/..., //mdx/...   npm
//quality/tools/javascript/...                                                        npm-tools
//python/...                                                                           uv
//quality/tools/python/...                                                            uv-tools
//examples/adopt-polyglot/...                                                         npm-adopt-polyglot, uv-adopt-polyglot
```

Adopt fixtures under `tests/fixtures/adopt-*` select their own `-adopt` set. `//...` and `MODULE.bazel` select every set.

### Security

`dx security` matches each set's lockfile against `.dx/advisory/<family>.json`.

- Sets with snapshots: `cargo`, `go`, `maven`, `npm`, `npm-adopt`, `npm-adopt-polyglot`, `npm-tools`, `nuget`. The `npm-*` sets share the `npm` snapshot. Configured `uv` sets use `pypi`, `ruby` sets use `rubygems`.
- Sets without a snapshot print `no advisory coverage for <sets>`.
- A version dx cannot parse marks the package unassessed with `unsupported version syntax`, and the run exits `1`.
- npm reads `pnpm-lock.yaml` (`lockfileVersion` 5.x, 6.0, 9.0), `package-lock.json` and `yarn.lock`. Each finding names its lockfile.

Create or refresh snapshots from the workspace root:

```sh
bazel run @rules_dx//cli/advisory_prep --
```

A missing or stale snapshot fails with `advisory_refresh_failed`.

Cargo sets also get a `cargo-yanked` snapshot, built from the crates.io sparse index (`--cargo-index` overrides it), that lists the locked yanked versions. A yanked version is a `yanked` finding: it fails under the strict base and is `info` under standard. A missing yanked snapshot only warns with `yanked_unprepared`. uv sets get a `pypi-yanked` snapshot the same way, from the PyPI JSON API (`--pypi-index` overrides it); a version is yanked when every file of it is. Other families have no yanked data yet and report nothing.

Secrets are scanned with the pinned `@rules_dx//quality/artifacts:gitleaks`. If `DX_GITLEAKS_BIN` holds an absolute path, dx runs it. Otherwise dx resolves the label through Bazel. A relative path is rejected and `PATH` is never searched. If no artifact is found, the run exits `1` with `secrets auditor unavailable`. Export the path with:

```sh
export "$(bazel run @rules_dx//cli/tool_resolve -- --label @rules_dx//quality/artifacts:gitleaks --var DX_GITLEAKS_BIN --print)"
```

A `.gitleaks.toml` in the workspace root is merged into the scan config and reported as a warning. A secret finding has the Gitleaks rule id, for example `gitleaks/aws-key`. Files git ignores are skipped. Outside git, every file is scanned.

`security.toml` in the workspace root exempts findings. Without it nothing is exempted. `schema_version` is `1`. Keys are in [Config Reference](config.md). Each advisory has a kind (`vulnerability`, `unsound`, `unmaintained` or `notice`). Only kinds in `fail_on` fail the run; the default is `vulnerability` and `unsound`, so unmaintained and notice advisories are reported as `info` unless `security.toml` extends the strict base. An exception for an advisory that does not fail still counts as a match.

```toml
[[exception]]
advisory = "GHSA-aaaa-bbbb-cccc"
package = "some-package"
set = "npm"
versions = ">=1.2.0, <2.0.0"
reason = "Reviewed; no reachable code path."
expires = "2027-03-01"

[[uncovered]]
set = "powershell"
reason = "PowerShell Gallery publishes no advisories."
expires = "2027-04-24"
```

An `[[exception]]` needs `advisory`, `package`, `set`, `versions`, `reason` and `expires`. An `[[uncovered]]` entry needs `set`, `reason` and `expires`. An expired entry fails the run. An exception that matches no finding in its set fails the run. `versions` uses the set ecosystem's range syntax. Bad entries print as `<file>:<line>: <entry>: <problem>`.

### License

`dx license` reads `licenses.toml` from the workspace root. Without it every scope is distributed and only `MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC` and `Unlicense` pass. `schema_version` is `1`. An unknown key fails the run.

```toml
schema_version = 1

[policy]
blocked = ["AGPL-3.0-only", "SSPL-1.0"]

[policy.distributed]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unlicense"]
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

- `[policy] blocked`, `[policy.distributed] allow|review|deny` and `[policy.sets.<set>] review` list license ids. An id in two lists fails the run.
- `[distribution] internal` lists label patterns: `//...` is every scope, `//pkg/...` is `pkg` and below, `//pkg:all` is the targets in `pkg`, anything else is one label. A run is internal only when entries cover every scope, including `--here` and `--affected`. An internal run fails only on `blocked`. A distributed run fails on all lists and unlisted ids.
- `MIT`, `Apache-2.0`, `BSD-2-Clause` and `BSD-3-Clause` also need their text, or the finding is `license/missing-notice-text`.
- `[[inventory]]` needs `package`, `set`, `license`, `versions`. `text_present` says the text ships with the package. `text = "<workspace-relative path>"` names a vendored file, implies `text_present`, and must be a `/`-separated path with no leading `/` or `\` and no empty, `.` or `..` parts. Inventory overrides licenses read from `cargo-bazel-lock.json` and `package-lock.json`. For other sets it is the only source; a package with no entry is `UNKNOWN`.
- `[[exception]]` needs `package`, `set`, `license`, `versions`, `reason`, `expires`. It covers a finding when `package`, `set` and `license` match. Expiry and unmatched rules are as for `security.toml`.

Add `#:schema https://raw.githubusercontent.com/devhusdk/rules_dx/main/cli/audit/schema/security.schema.json` (or `licenses.schema.json`) at the top of the file for editor checks.

Freshness and expiry use today's UTC date. Set `DX_AUDIT_TODAY=YYYY-MM-DD` to pin it.

### Output

Text prints `<path>: <severity>: <message> [<tool>/<rule>]` per finding. Lines go to stderr when a report goes to stdout.

JSON emits one `diagnostic` event per finding with `tool`, `rule`, `path`, `severity`, `message`.

- Vulnerability: tool `vuln`, rule `vuln/<advisory>`, `package` with `name`, `version`, `fixed`.
- License: tool `license`, rule `license/<license>`, `package` with `name`, `version`, `license`.
- A clean family emits a `notice`: `audit_security_clean` or `audit_license_clean`.
- A failed family emits an `error`: `advisory_refresh_failed` (missing, stale or corrupt snapshot), `offline_required`, or `audit_failed`.

A relative report destination anchors at the workspace root. Colliding destinations fail before anything runs. Parent directories must exist. A failed write keeps the previous file.

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Findings at or above `--fail-on`, an incomplete assessment, or an operational failure. |
| `2` | Usage or scope error. |

## Advisory Snapshots

```sh
bazel run @rules_dx//cli/advisory_prep --
bazel run @rules_dx//cli/advisory_prep -- --out .dx/advisory --date 2026-11-03
bazel run @rules_dx//cli/advisory_prep -- --archive cargo=/path/cargo.zip
```

It converts one archive per advisory family the dependency locks need and writes `<out>/<family>.json` and `<out>/<family>.meta.json`. The sidecar holds the set, source URL, payload `sha256`, stamped date and payload path. Relative paths anchor at the workspace root.

| Flag | What it does |
| --- | --- |
| `--workspace <dir>` | Dependency set locks to read. Default the workspace root. |
| `--out <dir>` | Snapshot directory. Default `.dx/advisory`. |
| `--date YYYY-MM-DD` | Date every sidecar carries. Default today, UTC. |
| `--max-time <seconds>` | Limit for one download. Default 600. |
| `--connect-timeout <seconds>` | Connect limit for one download. Default 30. |
| `--retries <count>` | Attempts per download. Default 3. |
| `--retry-delay <seconds>` | Pause between attempts. Default 5. |
| `--archive FAMILY=PATH` | Converts a local archive instead of fetching. Repeatable. |
| `--status-file <file>` | Appends `status`, `ecosystems` and `families` as `key=value` lines. |

With no advisory ecosystem needed it prints `no advisory ecosystems needed`, exits `0` and reports `status=empty`, `ecosystems=0`. Sets without a lock are not audited. Only `https` and `file` URLs are fetched.

A failure prints `advisory_prepare_failed: <family and reason>` and writes no snapshot for that family.

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Preparation failure. |
| `2` | Usage error. |

## Tool Resolution

```sh
bazel run @rules_dx//cli/tool_resolve -- --label @rules_dx//quality/artifacts:gitleaks --var DX_GITLEAKS_BIN --print
```

It runs `bazel cquery --output=files` for the label and writes the absolute artifact path to a variable.

| Flag | What it does |
| --- | --- |
| `--label <label>` | Pinned tool label. Required. |
| `--var <name>` | Variable that receives the path. Required. |
| `--bazel <path>` | Bazel binary. Default `bazel`. |
| `--workspace <dir>` | Directory for the inner Bazel commands. Default `BUILD_WORKSPACE_DIRECTORY`, else the current directory. |
| `--env-file <path>` | File that receives `VAR=path`. Default `GITHUB_ENV`. |
| `--print` | Prints `VAR=path` instead of writing a file. |
| `-- <args>` | Passed to the inner cquery. |

A failure prints `tool_resolve_failed: <reason>` and exports nothing. Exit `0` success, `1` resolution failure, `2` usage error.

## `dx update`

Updates dependencies per set. Checks by default; `--apply` writes.

```sh
tools/dx update
tools/dx update --apply go
tools/dx update --apply dx_pypi-python dx_pypi-tooling
tools/dx update --outdated
tools/dx update --pr --group=package dx_crates
tools/dx update --apply dx_crates:anyhow 1.0.100
```

```text
dx update [--apply [--prune] [--add-missing]] [--offline] [--frozen] [selector ...]
dx update --outdated [selector ...]
dx update --pr [--group=package|set|all] [--labels=<a,b>] [--automerge=none|patch|minor|all]
  [--min-age=<days>] [--ignore=<set:package>] [--allow-major] [--prereleases]
  [--max-prs=<n>] [selector ...]
dx update [--apply] [--offline] [--frozen] [--floor --reason=<text>] <set:package> <version>
```

| Flag | What it does |
| --- | --- |
| `--apply` | Writes updates. |
| `--prune` | With `--apply`: deletes the one-line declarations of unused dependencies (the entries `--outdated` lists as `unused:`) from `cargo`, `npm`, `uv`, `go` and `ruby` manifests, then refreshes the locks as usual. Each removal prints `dx: update_pruned: <set>: <name> (removed from <manifest>:<line>)` on stderr. Multi-line and table declarations are left in place. Does not combine with `--frozen`. |
| `--add-missing` | With `--apply`: declares the imports `--outdated` lists as `missing:` for `cargo`, `npm`, `uv` and `ruby` sets, at the latest stable version from the registry, then refreshes the locks once. Each addition prints `dx: update_added: <set>: <name> <version> (added to <manifest>)` on stderr. Does not combine with `--frozen`, `--offline` or `--pr`. |
| `--outdated` | Lists packages whose newest stable version is outside the declared range. |
| `--pr` | Opens or refreshes one pull request per group. |
| `--offline` | Uses pinned cached snapshots. Fails with `offline_required` when missing. |
| `--frozen` | Forbids manifest and lock changes. `--apply` fails with `frozen_locked` when a resolver would write. |
| `--group=package\|set\|all` | Groups pull requests. Default `set`. Needs `--pr`. |
| `--labels=<a,b>` | Labels for new pull requests. Needs `--pr`. |
| `--automerge=none\|patch\|minor\|all` | Auto-merge level. Default `patch`. Needs `--pr`. |
| `--min-age=<days>` | Skips versions newer than this. Default `3`. `0` turns it off. Needs `--pr`. |
| `--ignore=<set:package>` | Skips one package. Repeatable. Needs `--pr`. |
| `--allow-major` | Includes major updates. Needs `--pr`. |
| `--prereleases` | Offers prereleases. Never auto-merged. Needs `--pr`. |
| `--max-prs=<n>` | Stops opening pull requests at `n` open `dx-update/` pull requests. Default `5`. Needs `--pr`. |
| `--floor` | Edits the `bazel_dep` floor instead of the tested override. Needs a version and `--reason`. |
| `--reason <text>` | Why the floor is the oldest supported version. Needs `--floor`. |
| `--bazel-startup-option <token>` | Passed to the cargo, maven, npm and nuget Bazel runs. Repeatable. |
| `--output text\|json` | Sets the output format. |
| `--fail-on unused` | With `--outdated`, exits `1` when an unused or missing dependency is listed. |

`--report` and Bazel options do not apply. `--fail-on info\|warning\|error` is accepted and changes nothing.

Scopes: selectors are a set name (`dx_crates`), `set:package` (`dx_npm:vue`), or a label or path (`quality/tools/python`, `//quality/tools/python/...`, `//...`). No selector selects every set. Unknown names and unowned paths exit `2`. See [Dependency sets](#dependency-sets).

### Check And Apply

- A check validates the selected sets and writes nothing. It takes no [build slot](README.md#build-slots).
- `uv` sets run a read-only lock check: current, or failed when stale.
- `go` and `ruby` sets report `pinned` and change nothing. Widen them with `dx update go:<module> <version>` or `dx update <set>:<gem> <version>`. Edit a `pub` set with `dx update <set>:<package> <version>`: it rewrites the constraint of the package in `dependencies` or `dev_dependencies` of `pubspec.yaml`, keeping the `^`, exact or `>=` range form, then runs `dart pub upgrade <package>`. A package that is not declared there, or is declared as a map such as `sdk: flutter`, is refused. Edit a `cabal` set with `dx update <set>:<package> <version>`: it rewrites the bound of the package in `build-depends` of the `.cabal` file next to `cabal.project` (`^>=X`, `>=X`, `==X`, `==X.*` or `>=X && <Y`, which gets the upper bound of the new major), then runs the set's `haskell_deps_resolve`. A package that is not declared, has no bound or is declared in several places is refused. Edit a `cpan` set the same way with `dx update <set>:<Module> <version>`: it rewrites the version of the module's `requires` line in the cpanfile (keeping the operator), then runs `carton update <Module>` and the set's `cpan_compiler --incremental`. The version is decimal (`2.06`, `1.002_003`) or dotted (`v1.0.35`).
- `go` and `ruby` sets report `pinned` and change nothing. Widen them with `dx update go:<module> <version>` or `dx update <set>:<gem> <version>`. Edit a `pub` set with `dx update <set>:<package> <version>`: it rewrites the constraint of the package in `dependencies` or `dev_dependencies` of `pubspec.yaml`, keeping the `^`, exact or `>=` range form, then runs `dart pub upgrade <package>`. A package that is not declared there, or is declared as a map such as `sdk: flutter`, is refused. Edit a `cpan` set the same way with `dx update <set>:<Module> <version>`: it rewrites the version of the module's `requires` line in the cpanfile (keeping the operator), then runs `carton update <Module>` and the set's `cpan_compiler --incremental`. The version is decimal (`2.06`, `1.002_003`) or dotted (`v1.0.35`). Edit a `swiftpm` set with `dx update <set>:<package> <version>`: the package is named by its identity, the last segment of its URL in `Package.swift` without `.git`, and the `from:` or `exact:` version of that `.package(url:, ...)` is rewritten, then `swift package update <package>` refreshes `Package.resolved`. A package that is not declared, or is pinned by `branch:`, `revision:` or `path:`, is refused.
- Other sets report `unsupported` and fail the run. Refresh them with `dx update --apply <set>`.
- A bare `dx update` therefore exits `1` when any set is unsupported.
- `--apply` runs Bazel for `cargo`, `maven`, `npm` and `nuget` sets.
- In a rules_dx checkout, `tools/dx update --apply dx_crates` repins the lock when `//:dx` no longer builds against it.

Output: JSON emits `command_started`, then per set one `notice` (`update_set_success`, `update_set_current`, `update_set_pinned`, `update_set_blocked`) or one `error` (`update_failed`, `update_set_unsupported`, `offline_required`, `frozen_locked`, `backend_unavailable`). An applied set emits one `change` event per edited file before its success notice. A failed run emits an `update_recovery` notice, and every run ends with `command_finished`.

Text prints `dx: <code>: <message>` on stderr per failure, then a count of updated, current, pinned, unsupported, failed and blocked sets. A failed run prints the rerun command: `dx update --apply <sets>` plus `git checkout -- <locks>` after `--apply`, or `dx update <sets>` after a check.

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | A set is unsupported, failed or blocked, or an operational failure. |
| `2` | Usage or scope error. |

### Outdated

```sh
tools/dx update --outdated
tools/dx update --outdated dx_crates
tools/dx update --outdated github-actions bazel:.bazelversion
```

It reads registries, writes nothing and starts no Bazel. Yanked versions and prereleases are ignored. Path, git, workspace and renamed entries are skipped, as are `workspace:`, `file:`, `link:`, `portal:`, `catalog:`, `npm:`, `git`, `github:` and `http` npm specs, `owner/repo` shorthands, and PyPI direct references or requirements without a version specifier.

| Set | Source |
| --- | --- |
| Bazel module sets | `bazel_dep` against the Bazel Central Registry. |
| `cabal` | Packages in `build-depends` of the `.cabal` file next to `cabal.project`, at their `cabal.project.freeze` pin, against the newest non-deprecated version from `hackage.haskell.org/package/<name>/preferred`. `cabal outdated` is not run. |
| `cargo` | crates.io. |
| `composer` | Direct `require` and `require-dev` packages of `composer.json` against the `composer.lock` version and `repo.packagist.org/p2/<vendor>/<name>.json`. Platform requires (`php`, `ext-*`) are skipped, as are packages missing from the lock. `composer outdated` is not run. |
| `go` | Direct `require` lines of `go.mod` against proxy.golang.org. Indirect and replaced modules are skipped. |
| `maven` | Literal `maven.install` coordinates against Maven Central. Snapshot, alpha, beta, rc and milestone versions are ignored. |
| `npm` | `dependencies`, `devDependencies`, `optionalDependencies` against the npm registry. |
| `nuget` | `nuget` lines of `paket.dependencies` against nuget.org. Lists the newest stable version also when in range. |
| `ruby` | Versioned `gem` lines of the Gemfile against rubygems.org. Lists the newest version also when in range. |
| `uv` | `project.dependencies`, `project.optional-dependencies`, `dependency-groups` against PyPI. |

Other sets print an `update_outdated_unsupported` warning. A `set:package` selector limits the lookup.

`github-actions` checks every remote `uses:` pin in `.github/workflows`. `github-actions:<owner>/<repo>` checks one repo. `bazel:.bazelversion` checks `.bazelversion`. `containers` compares each `oci.pull` digest in `MODULE.bazel` with the digest its image's `latest` tag has now; `containers:<name>` checks one pull. Apply a new digest with `dx update containers:<name> sha256:<digest> --apply`. With no selector they run with the sets. A pin is read from its tag (`@v4`) or from a trailing comment (`@<sha> # v4.1.0`). Pins with no readable version, and repos with no release, print `<set>:<package>: unknown (<reason>)` and an `update_outdated_unknown` warning; the exit code is unchanged. Set `GH_TOKEN` or `GITHUB_TOKEN` to raise the 60 requests an hour limit. The token goes to `api.github.com` only.

Text prints one line per package: declared version, newest version, publish time, and the apply command. JSON emits `command_started`, one `update_outdated` notice per package (`scope` is `<set>:<package>`, `related_command` is `dx update <set>:<package> <version>`), one `update_outdated_lookup_failed` error per failed lookup, and `command_finished`. Ruby results have no `related_command`; text prints `edit Gemfile, then bundle lock`. Apply others with `dx update <set>:<package> <version> --apply`. Refresh in-range with `dx update --apply <set>`.

After the version lines, `--outdated` also lists declared dependencies that no source file under the manifest's directory uses, for `cargo`, `npm`, `uv`, `go` and `ruby` sets: `unused: <set>: <name> (declared in <manifest>:<line>)`. JSON emits one `update_unused` warning per entry with `scope` `<set>:<name>`. The exit code is unchanged. For `cargo` sets with a repository, an entry is unused when no target in the workspace depends on its label in the hub (one `bazel query` per set, so `build.rs` and `select` branches count); if the query fails, source scanning is used. A `# dx-keep` comment on the manifest line keeps that entry in `cargo`, `uv`, `go` and `ruby` manifests. A target with `tags = ["dx-keep"]` keeps every `go` and `ruby` entry that its deps name. For `go` sets it also lists imports under the manifest's directory that neither the module nor a `require` covers: `missing: <set>: <import> (imported in <file>:<line>)`; JSON emits an `update_missing` warning. Nested modules, `vendor` and `testdata` are skipped, and a `// dx-keep` comment on the import line silences a missing import. With `--fail-on=unused` the run exits `1` when any unused or missing entry is listed. For `cargo`, `npm`, `uv` and `ruby` sets it lists the same for packages that appear in the set's lock (`Cargo.lock`, `pnpm-lock.yaml`, `uv.lock`, `Gemfile.lock`) but not in the manifest, when a source under the manifest's directory imports them; workspace and path members are not candidates, and a package imported under another name (`PyYAML` as `yaml`) is not found. For `cargo` sets with a Bazel hub it also lists a hub label that a BUILD target depends on but the manifest does not declare: `missing: <set>: <name> (used by <label>)`, with `<label>` the first target that depends on it; a crate both lists find is printed once. `--add-missing` is not available yet. `dx update --apply --prune` removes the unused entries.

`--outdated` does not combine with `--apply`, `--floor`, `--reason` or a version. `--offline` fails with `offline_required`.

| Code | Meaning |
| --- | --- |
| `0` | Lookups finished, also with outdated packages. |
| `1` | A lookup failed. |
| `2` | Usage error. |

### Pull Requests

```sh
tools/dx update --pr
tools/dx update --pr --group=package --labels=dependencies dx_crates
tools/dx update --pr --automerge=none --ignore=cargo:serde --max-prs=2
```

Needs `git`, an authenticated `gh`, a clean working tree and a checked-out branch. That branch is the pull request base. Covers the same sets as `--outdated`, including GitHub Actions pins (branch `dx-update/github-actions`) and `.bazelversion`.

| `--group` | Branch | One pull request per |
| --- | --- | --- |
| `set` | `dx-update/<set>` | dependency set |
| `package` | `dx-update/<set>--<package>` | package |
| `all` | `dx-update/all` | run |

Characters outside letters, digits, `.`, `_` and `-` become `-` in branch names. Labels must exist, or the group fails.

Auto-merge runs `gh pr merge --auto --squash`. The repository must allow auto-merge, or the line ends with `auto-merge not enabled: ...` and the exit code is unchanged. A zero-major version counts a minor bump as major. A refresh that raises a pull request above the level turns auto-merge off. A version with no publish date is kept and the body says `age unknown`.

For each group, dx:

1. Resets the branch to the base commit.
2. Widens each package, then runs `dx update --apply <set>`. GitHub Actions pins and `.bazelversion` are only widened.
3. Runs `dx check --affected` and `dx test --affected`.
4. Commits. If `origin` has the same files on the same base and a pull request is open, it prints `unchanged #<n>` and stops.
5. Pushes with `--force-with-lease`. If the base moved or a newer version appeared, it rebuilds and pushes again.
6. Edits the open pull request or creates one.
7. Sets auto-merge by `--automerge`.

The body lists each package as `old -> new` with its registry link, the compare and release links when the registry names a GitHub repository (cargo and ruby read it from the crates.io and rubygems.org metadata of outdated packages only; a failed metadata fetch leaves the registry link alone; the real tag names come from `git ls-remote --tags`, matching `v`, `<package>@v`, `<package>-v` and `<package>-` forms; with no match the body links the repository's releases page only), a `prerelease` mark, and the changed lock files with line counts.

Skips print `skipped <set>:<package> <old> -> <new>: <reason>` with `ignored`, `major` or `too new` (JSON notice `update_pr_skipped`). A group over `--max-prs` prints `skipped: --max-prs=<n> reached`. A push refused for workflow files prints `skipped: token cannot push workflow files`. Neither changes the exit code. If the token may not create pull requests, the run fails and names the setting "Allow GitHub Actions to create and approve pull requests".

After the groups, dx closes open `dx-update/` pull requests on the base branch that this run covers but that have no updates left. It deletes the branch, comments `dx update: no updates left for this group.` and prints `closed #<n>`. Pull requests from forks are left alone. A group that fails any step is reset and not pushed; the next group runs. The base branch is checked out again at the end.

`--pr` does not combine with `--outdated`, `--apply`, `--floor`, `--reason`, `--offline` or a version.

| Code | Meaning |
| --- | --- |
| `0` | Every group published, unchanged or closed, or nothing outdated. |
| `1` | A group, close or lookup failed. |
| `2` | Usage error. |

### Widen Or Add One Package

```sh
tools/dx update --apply dx_crates:anyhow 1.0.100
tools/dx update --apply go:github.com/google/go-cmp 0.7.0
tools/dx update --apply bazel-modules:rules_cc 0.3.0
tools/dx update --apply --floor --reason "needs 0.3 API" bazel-modules:rules_cc 0.3.0
tools/dx update --apply npm:vue 3.5.42
```

One `set:package` and one version widen that requirement, or declare it when missing. A version starts with a digit, `v` plus a digit, or is a commit SHA. The set is a [dependency set](#dependency-sets) or the built-in `bazel`, `containers` or `github-actions`. Any other set exits `2` and lists valid names.

Without `--apply` it exits `0` when the manifest already declares the version, and `1` with `update drift` (widen) or `update pending` (add). `github-actions` and `bazel:.bazelversion` cannot add.

| Set | Edit |
| --- | --- |
| `bazel` | Tested version in the `single_version_override`, keeping patches. Falls back to the `bazel_dep` when there is no override or no `version`. `--floor --reason <text>` edits the `bazel_dep` version and the `# floor: <text>` comment lines above it. |
| `github-actions` | A tag resolves to its commit SHA through `api.github.com` (`GH_TOKEN` or `GITHUB_TOKEN` when set). The pin is written as the SHA in every workflow that declares the action; a `# v4` comment becomes the tag. `--offline` fails a tag with `offline_required`; pass a SHA. |
| `cargo` | Crate in its manifests, then repins the hub with `dx update --apply <set>`. |
| `composer` | Constraint in `require` or `require-dev` of `composer.json`, keeping the operator, then `composer update <package> --with-all-dependencies --no-install` and the digest refresh. A package that is not declared, or a constraint that is not one literal version, is refused. |
| `go` | `require` line of the `go.mod`, then regenerates `go.sum`. If go fails or would change `go.mod`, the widen stays, `go.sum` is untouched, and the report names `go mod tidy` then `dx update <set>`. |
| `hex` | Requirement of the `{:name, "<op> x.y.z"}` tuple in `mix.exs`, keeping the operator, then `mix deps.update <package>` with the pinned mix. A package that is not declared, or a requirement that is not one literal version, is refused. |
| `maven` | Literal coordinate in the set's `maven.install`, then `@<set>//:pin`. Use `<set>:<group>:<artifact> <version>`. |
| `npm`, `uv` | `package.json` or `pyproject.toml`, then refreshes the lock. |
| `nuget` | `paket.dependencies` only. Run `paket update`, then `dx update <set>`. |
| `ruby` | Literal requirement on the gem's Gemfile line, then `bundle lock --update <gem> --conservative` with `@ruby//:bundle` in the Gemfile directory. |

A missing package is inserted in the same manifest and shape: a `bazel_dep` after the last one, a `[dependencies]` entry, a `require` line, a `maven.install` coordinate, a `dependencies` entry in `package.json`, a `nuget` line, a `[project].dependencies` requirement `name>=version` in `pyproject.toml` for configured `uv` sets, or a `gem "name", "~> <major>.<minor>"` line after the last top-level `gem` in the `Gemfile` for `ruby` sets.

`--floor` is refused with exit `2` and writes nothing when:

- `--reason` is missing, or `--reason` is given without `--floor`.
- The set is not a `bazel_dep` (including `.bazelversion`).
- The floor would exceed the tested version, or the tested version would fall below the floor.

Duplicate declarations, a missing or computed version or name, and an unparsable `MODULE.bazel` fail without writing. A plain `dx update` never touches a `bazel_dep` floor or override.

`cargo`, `go`, `maven`, `npm` and `nuget` refresh through `dx update --apply <set>`. A failed refresh keeps the widen and exits `1`. `bazel`, `containers` and `github-actions` refresh nothing; review the diff and run `bazel build //...`. A version already declared reports `update_no_change`, writes nothing and exits `0`, also under `--frozen` or `--offline`. When a widen is pending, a set that needs a refresh fails before widening with `offline_required` under `--offline` and `frozen_locked` under `--frozen`.

JSON emits an `add_declared` notice for an insert, an `update_widened` notice per written manifest, and an `update_set_success` notice after the lock refresh.

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | A pending widen without `--apply`, or an operational failure. |
| `2` | Usage or scope error, or an unknown set. |

## Dependency Sets

`update`, `security` and `license` read sets from the hubs in the root `MODULE.bazel` and the files it `include()`s.

| Hub | Set name | Manifests | Locks |
|---|---|---|---|
| `crate.from_cargo` with `cargo_lockfile` | `name` | `manifests` | `cargo_lockfile`, `lockfile` |
| `maven.install` with `lock_file` | `name`, default `maven` | `MODULE.bazel` | `lock_file` |
| `npm.npm_translate_lock` with `pnpm_lock` | `name` | `package.json` beside the lock | `pnpm_lock` |
| `uv.project` | `<hub_name>-<pyproject directory name>` | `pyproject` | `lock` |
| `go_deps.from_file` with `go_mod` | `go` | `go_mod` | `go.sum` beside it |
| `ruby.bundle_fetch` | `name`, default `bundle` | `gemfile` | `gemfile_lock` |
| `composer.lock` of `@rules_dx//php/composer:extensions.bzl` | `name` | `composer.json` beside the lock | `lock`, `digests` |
| `cpan.install` of `@rules_perl//perl/cpan:extensions.bzl` | `name` | `cpanfile` beside the lock | `cpanfile.snapshot` and the `lock` JSON |
| `pub.from_lock` of `@rules_dart//dart/pub:extensions.bzl`, `flutter.pub` of rules_flutter | `name` | `pubspec.yaml` beside the lock | `pubspec.lock` |
| Conan `BazelDeps` `conan_deps_module_extension.bzl` | `conan` | `conanfile.txt` or `conanfile.py` beside the .bzl or in its parent | `conan.lock` beside the conanfile |
| `julia_pkg.from_manifest` of `@rules_julia//julia/pkg:extensions.bzl` | `name` | `Project.toml` beside the lockfile | `Manifest.toml` and `Manifest.bazel.json` beside the lockfile |
| rules_dotnet `paket` extension | `nuget` | `paket.dependencies` | `paket.lock`, `paket.<group>.bzl` |

A `julia` set: `update --outdated` compares the `[deps]` packages of `Project.toml`, as locked in `Manifest.toml`, with the General registry, and `dx update <set>:<Package> <version>` edits the `[compat]` entry (see the julia section). `security` reads the versioned packages of `Manifest.toml` (format 2.0, stdlibs skipped) and matches them against the `Julia` advisories, and the layout check needs both locks to name the same versions. A `conan` set is pinned; `security` lists the packages of `conan.lock` but does not match them against advisories. A `pub` set is pinned the same way: `security` reads the hosted packages of `pubspec.lock` and matches them against the `Pub` advisories. A `cpan` set is checked with `carton install --deployment`; `update --outdated` compares the `requires` modules of `cpanfile` with the latest release on MetaCPAN, and `security` reads the distributions of `cpanfile.snapshot` and matches them against the CPANSA advisories (see the `cpan` section). `dx update <set>:<Module> --apply` runs `carton update <Module>` and then `bazel run` of the `cpan_compiler` whose `lockfile` is the lock JSON, with `--incremental`, so both files change together; without such a compiler in the lock's `BUILD.bazel` it refuses. The layout check fails when the distributions of the lock JSON differ from those of `cpanfile.snapshot`. A `uv.project` at the workspace root is named `<hub_name>`. A set's scopes are the directories of its manifests and locks, except `MODULE.bazel`. A path that names a manifest or lock, such as `//:cargo-bazel-lock.json`, selects only its owner. The scope `.` owns the root package: `//:x` and `//:` select sets scoped `.`; `//pkg:x` and `//pkg/...` never do. Overlapping scopes select every owner.

With no hubs, `dx update` exits `2` with `no dependency sets`, and `dx security` exits non-zero with an example hub. Hub arguments must be literal. A computed argument, two sets with one name, or two sets owning one manifest or lock exit `2` before anything runs.

For every set, `--apply --offline` fails with `offline_required` and `--apply --frozen` fails with `frozen_locked`, unless noted. A check with `--offline` adds `--repository_disable_download` (Bazel sets) or the tool's offline flag. Checks that run Bazel (`npm`, `cargo`, `maven`) restore any manifest or lock they changed and fail the set with `check_wrote: <paths>`.

### npm

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

- Hubs without `pnpm_lock` are skipped. The root `MODULE.bazel` must load pnpm as shown.
- `--apply` runs `bazel run @pnpm//:pnpm -- --dir <set directory> install --lockfile-only` and writes that `pnpm-lock.yaml` only. `set:package` runs `update <packages> --lockfile-only`.
- A check runs `install --lockfile-only --frozen-lockfile`, writes nothing, and fails with `updater exited <code>` when the lock is stale. A check with `set:package` is unsupported.
- User and global `.npmrc` files are ignored. A `.npmrc` in the set directory applies.
- The set is unsupported when `locks` omits `<set directory>/pnpm-lock.yaml`, or a parent `pnpm-workspace.yaml` makes it a member. A set with its own `pnpm-workspace.yaml` is its own root. A set under a parent whose `packages` is exactly `["."]` is accepted and runs with `--ignore-workspace`.
- `dx license` reports `UNKNOWN` unless `licenses.toml` has `[[inventory]]` with `set = "npm"`.

### ruby

```starlark
ruby = use_extension("@rules_dx//ruby:extensions.bzl", "ruby")
ruby.bundle_fetch(
    name = "gems",
    gemfile = "//apps/rb:Gemfile",
    gemfile_lock = "//apps/rb:Gemfile.lock",
)
```

- `gemfile_lock` must sit beside the `Gemfile`, or the set is unsupported.
- `dx update` and `--apply` report `pinned` and start no process, also under `--offline` and `--frozen`. A `set:package` selector is unsupported; run `bundle lock`.
- `dx security` matches `Gemfile.lock` against `.dx/advisory/rubygems.json`. `dx license` needs `[[inventory]]` with `set = "ruby"`.

### julia

```starlark
julia_pkg = use_extension("@rules_julia//julia/pkg:extensions.bzl", "julia_pkg")
julia_pkg.from_manifest(
    name = "julia_app_pkgs",
    lockfile = "//julia/app:Manifest.bazel.json",
)
```

- `dx update --outdated` compares each `[deps]` package of `Project.toml` with the newest version in the General registry (`Versions.toml`), skipping yanked versions and, without `--prereleases`, prereleases. Standard libraries without a locked version are not listed.
- `dx update <set>:<Package> <version>` sets the package's `[compat]` entry in `Project.toml`, keeping a leading `=` or `~`, and adds the entry when it is absent. A package not in `[deps]`, or an entry that lists several ranges, is refused. `--apply` then runs `Pkg.update` and the set's `julia_pkg_resolve`.
- `dx security` matches `Manifest.toml` against the `Julia` advisories.

### composer

```starlark
composer = use_extension("@rules_dx//php/composer:extensions.bzl", "composer")
composer.lock(
    name = "php_deps",
    digests = "//apps/php:composer.digests.json",
    lock = "//apps/php:composer.lock",
)
```

- `composer.json` must sit beside `composer.lock`.
- `dx update <set>:<vendor/name> <version>` edits the constraint in `composer.json`, runs `composer update` and refreshes `composer.digests.json`.
- `dx security` matches `composer.lock` against `.dx/advisory/packagist.json`.
### opam

An opam set is discovered from a `<name>.opam` file with a `<name>.opam.locked` file beside it.

- `dx update <set>:<package> <version>` edits the one literal version constraint of the package in `depends:` of the `.opam` file. It keeps the operator and the filters (`{>= "1.1.0" & with-test}`), and refuses a package that `depends:` does not list, one without a constraint, and one with two constraints. The version starts with a digit and may hold `.`, `+`, `-`, `~` and `_`, such as `1.3.0` or `0.9.0~rc1`.
- `--apply` then runs `opam upgrade <package>` and `opam lock` from the pinned opam and the opam-repository snapshot. It fails with `offline_required` under `--offline` and `frozen_locked` under `--frozen`.

### swiftpm

A swiftpm set comes from a `from_package` tag of `swift_deps` in `MODULE.bazel`, with its `Package.swift` and `Package.resolved`. The first set is `swiftpm`, the next `swiftpm-2`.

- `dx update <set>` resolves `Package.swift` with the managed Swift in a copy and compares the result with `Package.resolved`.
- `dx update --apply <set>:<package> <version>` edits the `from:` or `exact:` version of the package, named by the last segment of its URL, then runs `swift package update <package>`. A package that is not declared, or pinned by `branch:` or `revision:`, is refused.
- `dx update --outdated` compares the pins with the newest semver tag on GitHub.
- `dx security` matches `Package.resolved` against `.dx/advisory/swifturl.json`.

### hex

- `mix.exs` must sit beside `mix.lock`.
- `dx update <set>` checks the lock with `mix deps.get` and `dx update --apply <set>` runs `mix deps.update --all`, both with the pinned mix.
- `dx update <set>:<package> <version>` edits the requirement in the `deps` of `mix.exs`, keeping the operator, then runs `mix deps.update <package>`.
- `dx update --outdated` compares `mix.lock` with the newest release on hex.pm.

### cargo

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

- Each tag with a literal `cargo_lockfile` is one set. Dx ships no cargo; it uses the workspace's rules_rust.
- Set `lockfile` and list it in `locks`. Without it the hub always repins and the check ends in `check_wrote`.
- One `Cargo.toml` must sit beside `Cargo.lock`. The tag must be imported with `use_repo`, renamed or not, or the set is unsupported (exit `1`).
- `--apply` runs `bazel fetch --repo=@<visible hub name>` with `CARGO_BAZEL_REPIN=workspace`, `REPIN=workspace`, `CARGO_BAZEL_REPIN_ONLY=<hub name>` and `CARGO_BAZEL_ISOLATED=true`. A check sets both repin variables to `false`, so a shell `REPIN` cannot turn a check into a write; a stale lock fails with `updater exited <code>`.
- `--apply <set>:<crate>` repins one crate. Select one crate per cargo set per run. Repin-mode names (`all`, `workspace`, `false`, `0`, `off`) are refused. A crate with several locked versions fails with `updater exited <code>`. A check with a package is unsupported.
- Bazel evaluates every `crate.from_cargo` hub together, so one stale hub fails the check and update of every cargo set. Update the stale set first.
- `dx security` matches `Cargo.lock` crates against `.dx/advisory/cargo.json`. `dx license` reads licenses from the declared `*.json` lock; `[[inventory]]` with `set = "cargo"` overrides them.

### maven

```starlark
maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = ["com.google.guava:guava:33.4.0-jre"],
    lock_file = "//third_party:maven_install.json",
)
use_repo(maven, "maven")
```

- Needs rules_jvm_external 7.1 or later and a `use_repo` entry without a rename. Several sets share `MODULE.bazel` as manifest. Create the lock file (empty is enough) before the first `--apply`.
- `--apply` runs `bazel run @<repository>//:pin` with `RULES_JVM_EXTERNAL_REPIN=1`. A check runs `bazel fetch --repo=@<repository>`; a stale lock fails with `updater exited <code>`.
- A check is unsupported when `fail_if_repin_required` is `False` or computed, the lock is empty or lacks `__INPUT_ARTIFACTS_HASH` and `__RESOLVED_ARTIFACTS_HASH`, or the host sets `REPIN` or `RULES_JVM_EXTERNAL_REPIN`.
- Host `COURSIER_MIRRORS`, `COURSIER_REPOSITORIES`, `COURSIER_CREDENTIALS`, `COURSIER_CONFIG_DIR` and `SCALA_CLI_CONFIG` are refused.
- A set takes no `set:package` for update or check. Use `dx update <set>:<group>:<artifact> <version>` to widen.
- `dx security` matches the lock's `artifacts` against `.dx/advisory/maven.json`. `dx license` needs `[[inventory]]` with `set = "maven"`.

### nuget

```starlark
paket = use_extension("//third_party/dotnet/deps:paket.main_extension.bzl", "main_extension")
```

- The set is found from the `paket` extension in the root `MODULE.bazel`. Manifest is `paket.dependencies` in the parent of the extension folder. Locks are the `paket.lock` beside it and each `paket.<group>.bzl`. Every `source` must be http(s).
- Dx copies both files to a scratch directory and runs `bazel run @rules_dx//dotnet:paket2bazel -- --dependencies-file <scratch>/paket.dependencies --output-folder <scratch>/out`. It never writes `paket.lock`.
- A check compares generated files with the declared locks byte for byte. A difference, an undeclared generated file or a missing declared file fails the set and lists the paths. `--apply` writes only the differing declared files, and nothing when an undeclared or missing file turns up.
- `--apply --frozen` runs. `--offline` fails with `offline_required` in both modes.
- A set takes no `set:package` for update. Edit `paket.dependencies`, run `paket update`, then `dx update <set>`.
- `dx security` matches `paket.lock` against `.dx/advisory/nuget.json`. `dx license` needs `[[inventory]]` with `set = "nuget"`.

### cpan

```starlark
cpan = use_extension("@rules_perl//perl/cpan:extensions.bzl", "cpan")
cpan.install(
    name = "perl_app_cpan",
    lock = "//perl/tests/fixtures/perl-app/third_party:cpanfile.snapshot.lock.json",
)
```

- The `cpanfile` and `cpanfile.snapshot` sit beside the lock JSON. The distributions of the snapshot and the lock JSON must match.
- A check runs `carton install --deployment` in the set's directory through `bazel run @rules_dx//perl/tools:carton`, with carton's `local/` tree outside the workspace. `--offline` fails with `offline_required`. A set takes no `set:package` for a check.
- `dx update --outdated` compares the `requires` modules of `cpanfile` with the latest release on MetaCPAN.
- `dx update <set>:<Module> --apply` runs `carton update <Module>`, then the `cpan_compiler` of the lock's `BUILD.bazel` with `--incremental`, so the snapshot and the lock JSON change together. Without such a compiler it is refused. `--offline` and `--frozen` fail with `offline_required` and `frozen_locked`.
- `dx update <set>:<Module> <version> --apply` first rewrites the version in the module's `requires` line of `cpanfile`, keeping the operator, then refreshes as above.
- `dx security` matches the distributions of `cpanfile.snapshot` against `.dx/advisory/cpan.json`. `bazel run @rules_dx//cli/advisory_prep --` builds it from `CPANSA::DB` through the pinned `//perl/tools:cpansa_dump`, not from the host perl.

### go

```starlark
go_deps = use_extension("@gazelle//:extensions.bzl", "go_deps")
go_deps.from_file(go_mod = "//third_party/go:go.mod")
```

- The `go.mod` must parse and have no local-path `replace`. The root `MODULE.bazel` needs `bazel_dep(name = "rules_go")` with no `repo_name`. A root `MODULE.bazel` that mentions `go_env` makes the set unsupported.
- Dx copies `go.mod` and `go.sum` to a scratch directory and runs `bazel run @rules_go//go -- -C <scratch>/out mod download all` with `GOENV=off`, `GOWORK=off`, `GOTOOLCHAIN=local`, `GOFLAGS=-modcacherw` and an empty module cache. It needs Go 1.20 or newer. It never edits `go.mod`; a changed `go.mod` fails the set.
- A check compares the generated `go.sum` with the committed one. `--apply` writes `go.sum` when bytes differ. Entries are added, never pruned; pruning is `go mod tidy`.
- It uses Go's public proxy and checksum database. Private modules are unsupported. A host `GOPROXY`, `GOPRIVATE`, `GONOPROXY`, `GONOSUMDB`, `GONOSUMCHECK`, `GOSUMDB`, `GOINSECURE`, `GOVCS` or `GOAUTH` makes the set unsupported.
- `--offline` fails with `offline_required` in both modes. `--apply --frozen` fails with `frozen_locked`; a check with `--frozen` runs.
- A set takes no `set:package` for update. Edit the `require` in `go.mod` and run `dx update <set>`, or run `dx update <set>:<module> <version>`.
- `dx security` matches `go.mod` requirements against `.dx/advisory/go.json`.

### cabal

```starlark
haskell = use_extension("//haskell/toolchain:repos.bzl", "haskell")
haskell.packages(name = "hackage", deps = "//third_party:haskell_deps.json")
```

- The manifest is `cabal.project` in the package of the `deps` label. Locks are `cabal.project.freeze` and the `haskell_deps.json` sidecar beside it. A check compares the versions in both.
- A check runs the `haskell_deps_resolve_test` next to `cabal.project`; `--apply` runs its `haskell_deps_resolve`, which keeps both lock files in step. Without those targets the set is unsupported. `--offline` fails with `offline_required`; `--apply --frozen` fails with `frozen_locked`.
- A set takes no `set:package` without a version. `dx update <set>:<package> <version> --apply` rewrites the bound in `build-depends` first, then resolves again.
- `dx update --outdated` compares the freeze pins with `hackage.haskell.org/package/<name>/preferred`.
- `dx security` matches the `any.<name> ==<version>` pins of `cabal.project.freeze` against `.dx/advisory/hackage.json`. Flags and `installed` boot packages are skipped.

### uv

- Dx runs the pinned uv `0.13.7` and the pinned interpreter. `DX_UV_BIN` and `DX_UV_PYTHON` hold absolute paths to override; otherwise dx resolves `@dx_tools//:uv` and `@rules_dx//cli/update:python_interpreter` through Bazel. A relative path is rejected, `PATH` is never searched, and a uv that does not report `0.13.7` is rejected. With no usable tool the set fails with `backend_unavailable` and the run exits `1`.

```sh
export "$(bazel run @rules_dx//cli/tool_resolve -- --label @dx_tools//:uv --var DX_UV_BIN --print)"
export "$(bazel run @rules_dx//cli/tool_resolve -- --label @rules_dx//cli/update:python_interpreter --var DX_UV_PYTHON --print)"
```

- `--apply` runs `uv lock` in the set directory and writes that lock only. A check runs `uv lock --check` and fails with `updater exited 1` when stale.
- Both pass `--no-config --no-cache --no-python-downloads --python <DX_UV_PYTHON>` with an empty environment, so `HOME`, `XDG_CONFIG_HOME`, `UV_*` and user `uv.toml` are ignored. A check with `--offline` adds `--offline`; a lock that needs a download fails with `updater exited 2`.
- A `uv.toml` in the set directory or a parent inside the workspace, or a `[tool.uv]` key other than `sources`, `workspace`, `dev-dependencies`, `default-groups`, `conflicts`, `package`, `managed`, `override-dependencies` or `constraint-dependencies`, makes the set unsupported.
- `set:package` is unsupported: the set refreshes as a whole.
- `dx security` matches every `uv.lock` package against `.dx/advisory/pypi.json` with PEP 440 versions and PEP 503 names. Workspace members are skipped. Packages from git, path, url, directory or a registry other than pypi.org are unassessed and the run exits `1`. A `security.toml` exception uses `set = "<name>"` and a PEP 440 `versions` range.
