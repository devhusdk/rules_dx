# Audit And Update Commands

Use `dx security`, `dx license` and `dx update` to check your dependencies for vulnerabilities, leaked secrets and license problems, and to update them.

```sh
tools/dx security
tools/dx security --affected
tools/dx license
tools/dx update --outdated
tools/dx update --apply
tools/dx update --pr
```

All three commands work on dependency sets. A dependency set is one lock file plus the manifests it locks, such as `Cargo.lock` and `Cargo.toml`. You declare it in `MODULE.bazel`, the file where a Bazel workspace lists its modules, through a hub: a module extension call such as `crate.from_cargo` that turns the lock into one external repository. [Dependency Sets](#dependency-sets) lists every hub dx reads.

## `dx security` And `dx license`

`dx security` checks secrets and dependency vulnerabilities. `dx license` checks dependency licenses. Neither changes files, except with `--accept`.

```sh
tools/dx security
tools/dx security --affected
tools/dx security --offline --fail-on warning
tools/dx security --report sarif=security.sarif //apps/web/...
tools/dx license --fail-on error //...
tools/dx license --report spdx=sbom.spdx.json --report cyclonedx=sbom.cdx.json
```

```text
dx security [--here] [--affected [--since <ref>|--base <ref>]] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif=<dest>] [--accept <advisory>[:<package>]|uncovered:<set> --reason <text> --expires <YYYY-MM-DD>] [scope...]
dx security --verify <file> --repo <owner/name> [--bundle <file>.sigstore.json]
dx license  [--here] [--affected [--since <ref>|--base <ref>]] [--offline] [--frozen] [--fail-on info|warning|error] [--report sarif|spdx|cyclonedx=<dest>] [--accept <package> --reason <text> --expires <YYYY-MM-DD>] [scope...]
```

### Flags

| Flag | What it does |
| --- | --- |
| `--affected` | Runs the sets that own targets your git changes reach. Not with scopes or `--here`. |
| `--since <ref>` | With `--affected`: compares against this ref instead of the default branch merge-base. Without `--affected`, exits `2`. |
| `--base <ref>` | With `--affected`: takes the merge-base with this branch instead of the default branch. Not with `--since`. |
| `--here` | Uses the current directory tree. Not with scopes. |
| `--offline` | Runs without network. Uses cached pinned snapshots, or fails with `offline_required`. |
| `--frozen` | Keeps manifest and lock resolution unchanged. |
| `--fail-on info\|warning\|error` | Fails on findings at this severity or above. Default `warning`. |
| `--report sarif=<dest>` | Writes SARIF, the static analysis results format code hosts read. Repeatable. `-` is stdout. |
| `--report spdx=<dest>` | `dx license` only. Writes an SPDX 2.3 JSON SBOM (software bill of materials) of the scoped dependencies and their licenses. |
| `--report cyclonedx=<dest>` | `dx license` only. Writes a CycloneDX 1.6 JSON SBOM of the scoped dependencies with purl, version and license. |
| `--accept <finding>` | Adds an exception for a finding instead of reporting it. Needs `--reason` and `--expires`. See [Accept a finding](#accept-a-finding). |
| `--reason <text>` | Why the accepted finding is allowed. Needs `--accept`. |
| `--expires <YYYY-MM-DD>` | When the exception must be reviewed again. Needs `--accept`. |
| `--verify <file>` | `dx security` only. Checks a downloaded asset against its Sigstore bundle. Needs `--repo`. |
| `--repo <owner/name>` | The GitHub repository that must have signed the asset. Needs `--verify`. |
| `--bundle <file>` | The Sigstore bundle. Default `<file>.sigstore.json`. Needs `--verify`. |
| `-o`, `--output text\|json` | Sets the output format. |
| `--workspace <dir>` | Uses this workspace directory. |
| `-q`, `--quiet` | Hides summaries. |
| `-v`, `--verbose` | Shows more logs. |
| `--color auto\|always\|never` | Sets color output. |
| `--build-slots auto\|<n>\|off` | Sets build slots. |
| `--build-slots-wait <duration>` | Gives up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Passes one Bazel startup option. Repeatable. |

Bazel options after `--` do not apply.

### Scopes

A scope is a dependency set name, a package or a target. A target is something Bazel can build, named by a label such as `//apps/web:server`. `//...` means every package in the workspace. With no scope, both commands run `//...`. See [Scope Defaults](scope-defaults.md).

- A scope selects sets by path. A set's paths are the directories of its manifests and locks, except `MODULE.bazel`.
- A `...` scope selects the nearest set above it plus every set under it. A package selects its path's set.
- A path that names a manifest or lock, such as `//:cargo-bazel-lock.json`, selects only its owner.
- The root package belongs to sets whose path is `.`. `//:x` and `//:` select them; `//pkg:x` and `//pkg/...` never do.
- Overlapping paths select every owner. `//...` and `MODULE.bazel` select every set.
- Each set reads its whole lock file.
- A scope that owns no set exits `2`.

With `--affected` and no changes, or no owned set for `dx license`, they print `no affected targets` and exit `0`. With changes that no set owns, `dx security` runs only the secrets scan.

### Output

Text prints one line per finding:

```text
<path>: <severity>: <message> [<tool>/<rule>]
```

Lines go to stderr when a report goes to stdout.

JSON emits one `diagnostic` event per finding with `tool`, `rule`, `path`, `severity` and `message`.

- Vulnerability: tool `vuln`, rule `vuln/<advisory>`, `package` with `name`, `version` and `fixed`.
- License: tool `license`, rule `license/<license>`, `package` with `name`, `version` and `license`.
- Secret: the Gitleaks rule id, for example `gitleaks/aws-key`.
- A clean family emits a `notice`: `audit_security_clean` or `audit_license_clean`.
- A failed family emits an `error`: `advisory_refresh_failed` (missing, stale or corrupt snapshot), `offline_required` or `audit_failed`.

Reports:

- A relative destination is relative to the workspace root. Parent directories must exist.
- Two reports with the same destination fail before anything runs.
- A failed write keeps the previous file.
- SPDX and CycloneDX reports are sorted and dated from `SOURCE_DATE_EPOCH` or `DX_AUDIT_TODAY`, so reruns match byte for byte. Pass both to get them from one audit.

### Exit codes

`dx security` and `dx license` share these.

| Code | Meaning |
| --- | --- |
| `0` | Success, or `no affected targets`. |
| `1` | A finding at or above `--fail-on`, an unassessed package, `offline_required`, an expired or unmatched exception, a failed `--verify`, or another operational failure. |
| `2` | Usage error, a scope that owns no set, or an `--accept` that matches no finding. |
| `3` | A `security.toml` or `license.toml` that dx cannot read or write. |

### Vulnerabilities

`dx security` matches each set's lock file against the advisory snapshot its family pins in `.dx/pins`. See [Advisory Snapshots](#advisory-snapshots).

- Families with snapshots: `cargo`, `go`, `maven`, `npm`, `nuget`, `pypi` for `uv` sets, `rubygems` for `ruby` sets, and the families named under [Dependency Sets](#dependency-sets). The `npm-*` sets share the `npm` snapshot.
- Sets without a snapshot print `no advisory coverage for <sets>`.
- A version dx cannot parse marks the package unassessed with `unsupported version syntax`, and the run exits `1`.
- npm reads `pnpm-lock.yaml` (`lockfileVersion` 9.0), `package-lock.json` (`lockfileVersion` 3) and `yarn.lock`. Each finding names its lock file.
- A lock file without a dependency set (`mix.lock`, `pubspec.lock`, `poetry.lock` and others) is a warning finding. An `[[uncovered]]` entry whose `set` is the file path silences it.
- Each advisory has a kind: `vulnerability`, `unsound`, `unmaintained` or `notice`. Only kinds in `fail_on` of `security.toml` fail the run. The default is `vulnerability` and `unsound`, so `unmaintained` and `notice` advisories are `info`, unless `dx.toml` sets `tier = "strict"`.

Yanked versions:

- Cargo sets also get a `cargo-yanked` snapshot from the crates.io sparse index. `uv` sets get a `pypi-yanked` snapshot from the PyPI JSON API; a version is yanked when every file of it is.
- A locked yanked version is a `yanked` finding. It fails under the strict tier and is `info` under standard.
- A set without its `advisory cargo-yanked ...` or `advisory pypi-yanked ...` pin warns with `yanked_unprepared`.
- Other families have no yanked data and report nothing.

### Secrets

`dx security` scans the workspace with the pinned Gitleaks, `@rules_dx//core/quality/artifacts:gitleaks`.

- Set `DX_GITLEAKS_BIN` to an absolute path to run another copy. A relative path is rejected and `PATH` is never searched.
- Without it, dx resolves the label through Bazel. If no artifact is found, the run exits `1` with `secrets auditor unavailable`.
- Without a `.gitleaks.toml`, the scan uses `.dx/config/gitleaks/base.toml`, which dx generates.
- A `.gitleaks.toml` in the workspace root must start with `[extend]` and `path = ".dx/config/gitleaks/base.toml"`. Every `[[allowlists]]` entry needs a non-empty `description`. The file is reported as a warning.
- A `.gitleaksignore` file has no effect.
- Files git ignores are skipped. Outside git, every file is scanned.

To export the Gitleaks path:

```sh
export "$(bazel run @rules_dx//dx/tool_resolve -- --label @rules_dx//core/quality/artifacts:gitleaks --var DX_GITLEAKS_BIN --print)"
```

### Container images

When the scope holds `oci_deploy` targets, `dx security` builds each image and scans it with the pinned trivy against the trivy pin.

- trivy is `DX_TRIVY` when set, else `@rules_dx//core/quality/artifacts:trivy`.
- Each image adds one line, such as `image //app:image: 0 critical, 2 high, 0 medium, 0 low`. Each finding has the rule `trivy/<advisory>`.
- Critical and high findings are errors, medium findings are warnings and low findings are info. `--fail-on` decides which fail the run.
- An `[[exception]]` covers an image finding when its `set` is `oci` or a label pattern that matches the `oci_deploy` target, such as `//app/...`.
- With no `oci_deploy` in scope, nothing is built and the output is unchanged.
- Under `--offline`, a trivy pin that is not cached fails with `advisory_refresh_failed`.

### `security.toml`

`security.toml` in the workspace root exempts findings. Without it nothing is exempted. It needs `schema_version = 1`. Keys are in [Config Reference](config.md).

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

- An `[[exception]]` needs `advisory`, `package`, `set`, `versions`, `reason` and `expires`. `versions` uses the range syntax of the set's ecosystem.
- An `[[uncovered]]` entry needs `set`, `reason` and `expires`.
- An expired entry fails the run.
- An exception that matches no finding in its set fails the run. An exception for an advisory that would not fail still counts as a match.
- Bad entries print as `<file>:<line>: <entry>: <problem>`.

### `license.toml`

`dx license` reads `license.toml` from the workspace root. Without it, every scope is distributed and only `MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC` and `Unlicense` pass. `schema_version` is `1`. An unknown key fails the run.

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
- `[distribution] internal` lists label patterns: `//...` is every scope, `//pkg/...` is `pkg` and below, `//pkg:all` is the targets in `pkg`, anything else is one label. A run is internal only when the entries cover every scope, also with `--here` and `--affected`.
- An internal run fails only on `blocked`. A distributed run fails on every list and on unlisted ids.
- `MIT`, `Apache-2.0`, `BSD-2-Clause` and `BSD-3-Clause` also need their license text, or the finding is `license/missing-notice-text`.
- `[[inventory]]` needs `package`, `set`, `license` and `versions`. `text_present` says the text ships with the package. `text = "<path>"` names a vendored text file and implies `text_present`. The path is workspace-relative, `/`-separated, with no leading `/` or `\` and no empty, `.` or `..` parts.
- Inventory overrides licenses read from `cargo-bazel-lock.json` and `package-lock.json`. For other sets it is the only source; a package with no entry is `UNKNOWN`.
- `[[exception]]` needs `package`, `set`, `license`, `versions`, `reason` and `expires`. It covers a finding when `package`, `set` and `license` match. Expiry and unmatched entries fail as in `security.toml`.

Add `#:schema https://raw.githubusercontent.com/devhusdk/rules_dx/main/cli/audit/schema/security.schema.json` (or `license.schema.json`) at the top of either file for editor checks.

Expiry and snapshot age use today's UTC date. Set `DX_AUDIT_TODAY=YYYY-MM-DD` to pin it.

### Accept a finding

```sh
tools/dx security --accept GHSA-aaaa-bbbb-cccc:some-package --reason "Reviewed; no reachable code path." --expires 2027-03-01
tools/dx security --accept uncovered:powershell --reason "No advisory source." --expires 2027-03-01
tools/dx license --accept some-package --reason "Reviewed by legal." --expires 2027-03-01
```

`--accept` runs the audit and appends an `[[exception]]` to `security.toml` or `license.toml` for each finding it names. The entry's `versions` pins the installed version. `--accept uncovered:<set>` appends an `[[uncovered]]` entry.

- `--reason` and `--expires` are required. Either one without `--accept` exits `2`.
- An `--accept` that matches no finding exits `2` and leaves the file unchanged.
- An entry that is already there is not added again.
- The edit keeps comments and order. A missing file is created with the `#:schema` line and `schema_version = 1`.

### Verify a downloaded asset

```sh
tools/dx security --verify dx-linux-x86_64.tar.gz --repo devhusdk/rules_dx --bundle dx-sha256sums.txt.sigstore.json
```

`dx security --verify <file> --repo <owner/name>` checks one downloaded asset, such as a release archive, against its Sigstore bundle. It runs no audit, and it does not combine with scopes, `--here`, `--affected`, `--report` or `--accept`.

- A bundle from `cosign sign-blob` is checked with `cosign verify-blob`. An attestation bundle from `actions/attest` is checked with `cosign verify-blob-attestation` for SLSA provenance v1.
- cosign is the cosign rules_dx pins. Set `DX_COSIGN_BIN` to an absolute path to use another copy.
- The signing certificate must come from a GitHub Actions workflow in `<owner/name>` (`https://token.actions.githubusercontent.com`). The signer identity is printed.
- `gh attestation verify <file> --repo <owner/name>` runs too, so the [GitHub CLI](https://cli.github.com/) must be on `PATH`.
- A changed byte, a different signer, a missing bundle or a missing `gh` exits `1`.

## Advisory Snapshots

An advisory snapshot is a JSON file of known advisories for one family. Each snapshot is one line in `.dx/pins`:

```text
advisory <family> <YYYY-MM-DD> <sha256> <url>
```

- dx reads the cache `.dx/local/advisory/<sha256>/<family>.json`. On a miss it downloads `<url>` (`https://` or `file://`), checks the sha256 and fills the cache. `--offline` never downloads.
- A family with no line fails with `advisory_pin_missing`.
- A download or cached file with another sha256 fails with `advisory_pin_mismatch`.
- A pin older than 14 days adds an `advisory_pin_old` warning with the pin date. Nothing else depends on today's date, so one commit gives one result.
- dx's launchers ignore `advisory` lines.

To move the pins to the newest snapshots, run `dx update advisory`. See [Advisory pins](#advisory-pins).

### Build snapshots

```sh
bazel run @rules_dx//dx/advisory_prep --
bazel run @rules_dx//dx/advisory_prep -- --out .dx/local/advisory --date 2026-11-03
bazel run @rules_dx//dx/advisory_prep -- --archive cargo=/path/cargo.zip
bazel run @rules_dx//dx/advisory_prep -- --pin <dir> --pin-url-base <url>
bazel run @rules_dx//dx/advisory_prep -- --help
```

`advisory_prep` converts one archive per advisory family your locks need. It writes `<out>/<family>.json` and the sidecar `<out>/<family>.meta.json`, which holds the set, source URL, payload `sha256`, stamped date and payload path. Relative paths are relative to the workspace root.

| Flag | What it does |
| --- | --- |
| `--workspace <dir>` | Workspace with the locks to read. Default the workspace root. |
| `--out <dir>` | Snapshot directory. Default `.dx/local/advisory`. |
| `--date YYYY-MM-DD` | Date every sidecar carries. Default today, UTC. |
| `--max-time <seconds>` | Limit for one download. Default `600`. |
| `--connect-timeout <seconds>` | Connect limit for one download. Default `30`. |
| `--retries <count>` | Attempts per download. Default `3`. |
| `--retry-delay <seconds>` | Pause between attempts. Default `5`. |
| `--archive FAMILY=PATH` | Converts a local archive instead of fetching. Repeatable. |
| `--pin <dir>` | Also writes `<dir>/<date>/<family>.json` per family and prints the `advisory` lines for `.dx/pins`. The URLs point under the `devhusdk/rules_dx-advisory` release downloads unless you pass another base URL. |
| `--trivy BINARY` | Also packs the trivy cache. With `--pin`, writes `<dir>/<date>/trivy.tar.gz` and prints an `advisory trivy ...` line. |
| `--cargo-index URL` | Sparse index for the `cargo-yanked` snapshot. Default `https://index.crates.io`. |
| `--pypi-index URL` | PyPI JSON API for the `pypi-yanked` snapshot. Default `https://pypi.org/pypi`. |
| `--status-file <file>` | Appends `status`, `ecosystems` and `families` as `key=value` lines. |

With `--pin`, upload the files as assets of the release `<date>` and add the printed lines to `.dx/pins`. `--pin` prints the `cargo-yanked` and `pypi-yanked` lines too.

- With no advisory ecosystem needed, it prints `no advisory ecosystems needed`, exits `0` and reports `status=empty`, `ecosystems=0`.
- Sets without a lock are not audited. Only `https` and `file` URLs are fetched.
- A failure prints `advisory_prepare_failed: <family and reason>` and writes no snapshot for that family.

The trivy pin: `oci_deploy` image scans fetch the archive, check its sha256 and unpack it to `.dx/local/advisory/<sha256>/trivy`. `DX_TRIVY_CACHE` overrides the pin with a cache directory. Without either, trivy uses its own cache. A trivy pin older than 14 days adds a warning.

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Preparation failure. |
| `2` | Usage error. |

## Tool Resolution

```sh
bazel run @rules_dx//dx/tool_resolve -- --label @rules_dx//core/quality/artifacts:gitleaks --var DX_GITLEAKS_BIN --print
```

`tool_resolve` runs `bazel cquery --output=files` for the label and writes the absolute artifact path to a variable. An `external/` path is read under `bazel info output_base`. Any other path is a built output, read under `bazel info execution_root`, so build the label first.

| Flag | What it does |
| --- | --- |
| `--label <label>` | Pinned tool label. Required. |
| `--var <name>` | Variable that receives the path. Required. |
| `--bazel <path>` | Bazel binary. Default `bazel`. |
| `--workspace <dir>` | Directory for the inner Bazel commands. Default `BUILD_WORKSPACE_DIRECTORY`, else the current directory. |
| `--env-file <path>` | File that receives `VAR=path`. Default `GITHUB_ENV`. |
| `--print` | Prints `VAR=path` instead of writing a file. |
| `-- <args>` | Passed to the inner cquery. |

A failure prints `tool_resolve_failed: <reason>` and exports nothing. Exit `0` on success, `1` on a resolution failure, `2` on a usage error.

## `dx update`

`dx update` checks your dependency sets by default. `--apply` writes the updates.

```sh
tools/dx update
tools/dx update --apply
tools/dx update --apply go
tools/dx update --outdated
tools/dx update --apply --prune --add-missing crates
tools/dx update --pr crates
tools/dx update --apply crates:anyhow 1.0.100
```

```text
dx update [--apply [--prune] [--add-missing]] [--offline] [--frozen] [selector ...]
dx update --outdated [--fail-on info|warning|error] [selector ...]
dx update --pr [--automerge=none|patch|minor|all] [--min-age=<days>] [--allow-major] [--max-prs=<n>] [--frozen] [selector ...]
dx update [--apply] [--offline] [--frozen] [--floor --reason=<text>] <set:package> <version>
dx update --apply <set:package>
```

### Flags

| Flag | What it does |
| --- | --- |
| `--apply` | Writes the updates instead of only checking. |
| `--prune` | Needs `--apply`. Removes unused dependencies from the manifests. Not with `--frozen` or `--pr`. |
| `--add-missing` | Needs `--apply`. Declares imported but undeclared dependencies at their latest stable version. Not with `--frozen`, `--offline` or `--pr`. |
| `--outdated` | Lists packages whose newest stable version is outside the declared range. Not with `--apply`, `--floor`, `--reason` or a version. |
| `--pr` | Opens or refreshes one pull request per dependency set. Not with `--outdated`, `--apply`, `--floor`, `--reason`, `--offline` or a version. |
| `--automerge=none\|patch\|minor\|all` | Auto-merge level. Default `patch`. Needs `--pr`. |
| `--min-age=<days>` | Skips versions published fewer than this many days ago. Default `3`. `0` turns it off. Needs `--pr`. |
| `--allow-major` | Includes major version updates. Needs `--pr`. |
| `--max-prs=<n>` | Opens no new pull request once `n` `dx-update/` pull requests are open. Default `5`. Needs `--pr`. |
| `--floor` | Edits the `bazel_dep` floor instead of the tested override. Needs a version and `--reason`. |
| `--reason <text>` | Why the floor is the oldest supported version. Needs `--floor`. |
| `--offline` | Runs without network. A set that needs a download fails with `offline_required`. |
| `--frozen` | Keeps manifests and locks unchanged. `--apply` fails with `frozen_locked` when a resolver would write. |
| `--fail-on info\|warning\|error` | Only `--outdated` reads it. Unused and missing dependencies are warnings, so `warning` (the default) and `info` exit `1` when any is listed. |
| `--bazel-startup-option <token>` | Passed to the Bazel runs of `cargo`, `maven`, `npm` and `nuget` sets. Repeatable. |
| `-o`, `--output text\|json` | Sets the output format. JSON streams events while the command runs. |
| `--workspace`, `-q`, `--quiet`, `-v`, `--verbose`, `--color`, `--build-slots`, `--build-slots-wait` | Global flags, as for `dx security`. |

`--report` and Bazel options after `--` do not apply.

### Selectors

- A set name, such as `crates` or `go`.
- `set:package`, such as `web:vue`, to limit the run to one package.
- A label or path, such as `apps/web`, `//apps/web/...` or `//...`.
- The built-in names `bazel`, `containers`, `github-actions`, `pins` and `advisory`.

No selector selects every set. Unknown names and unowned paths exit `2` and list the valid names.

### Check and apply

A check validates the selected sets and writes nothing. It takes no [build slot](README.md#build-slots). `--apply` refreshes the locks.

- `--apply` runs Bazel for `cargo`, `maven` and `npm` sets.
- `uv`, `ruby`, `cabal` and `cpan` sets run their lock tool in check mode: current, or failed when stale.
- `ansible`, `dub`, `fpm`, `luarocks`, `nuget`, `powershell` and `zig` sets run the resolver their `setup.hubs` entry names: a check passes `--check` and fails the set when the lock is stale; `--apply` rewrites the lock. `--offline` fails with `offline_required`, and a `set:package` selector is refused.
- `go` sets compare a freshly generated `go.sum` with the committed one.
- `bicep` sets report `pinned` and change nothing.
- Sets dx cannot check report `unsupported` and fail the run. Refresh them with `dx update --apply <set>`. A bare `dx update` exits `1` when any set is unsupported.
- Checks that run Bazel (`npm`, `cargo`, `maven`) restore any manifest or lock they changed and fail the set with `check_wrote: <paths>`.
- A check with `--offline` adds `--repository_disable_download` for Bazel sets, or the tool's offline flag.
- For every set, `--apply --offline` fails with `offline_required` and `--apply --frozen` fails with `frozen_locked`, unless [Dependency Sets](#dependency-sets) says otherwise.
- In a rules_dx checkout, `tools/dx update --apply dx_crates` repins the lock when `//:dx` fails to build against it.

`--prune` deletes the one-line declarations that `--outdated` lists as `unused:` from `cargo`, `npm`, `uv`, `go` and `ruby` manifests, then refreshes the locks. Multi-line and table declarations stay. Each removal prints on stderr:

```text
dx: update_pruned: <set>: <name> (removed from <manifest>:<line>)
```

`--add-missing` declares the imports that `--outdated` lists as `missing:` for `cargo`, `npm`, `uv` and `ruby` sets, at the latest stable version, then refreshes the locks once. Each addition prints on stderr:

```text
dx: update_added: <set>: <name> <version> (added to <manifest>)
```

### Output

JSON emits `command_started`, then per set one `notice` (`update_set_success`, `update_set_current`, `update_set_pinned`, `update_set_blocked`) or one `error` (`update_failed`, `update_set_unsupported`, `offline_required`, `frozen_locked`, `backend_unavailable`). An applied set emits one `change` event per edited file before its success notice. A failed run emits an `update_recovery` notice. Every run ends with `command_finished`.

Text prints `dx: <code>: <message>` on stderr per failure, then a count of updated, current, pinned, unsupported, failed and blocked sets. A failed run prints the rerun command: `dx update --apply <sets>` plus `git checkout -- <locks>` after `--apply`, or `dx update <sets>` after a check.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | A set is unsupported, failed or blocked; a pending widen without `--apply`; a failed lookup; an unused or missing dependency at `--fail-on`; or another operational failure. |
| `2` | Usage or scope error, or an unknown set. |
| `3` | A read or write failure, or a `git` or `gh` that cannot start or times out. |

A missing or non-UTF-8 manifest, `.bazelversion`, workflow directory or commit JSON file, a refused edit, a dirty working tree, and a `git` or `gh` that exits nonzero all exit `1`.

### Outdated

```sh
tools/dx update --outdated
tools/dx update --outdated crates
tools/dx update --outdated github-actions bazel:.bazelversion
```

`--outdated` reads registries, writes nothing and starts no Bazel build. It ignores yanked versions and prereleases. It skips path, git, workspace and renamed entries; the npm specs `workspace:`, `file:`, `link:`, `portal:`, `catalog:`, `npm:`, `git`, `github:`, `http` and `owner/repo`; and PyPI direct references or requirements without a version specifier.

| Set | Source |
| --- | --- |
| Bazel modules | `bazel_dep` against the Bazel Central Registry. |
| `cabal` | `build-depends` of the `.cabal` file next to `cabal.project`, at their `cabal.project.freeze` pin, against the newest non-deprecated version on `hackage.haskell.org/package/<name>/preferred`. |
| `cargo` | crates.io. |
| `composer` | Direct `require` and `require-dev` of `composer.json`, at their `composer.lock` version, against `repo.packagist.org/p2/<vendor>/<name>.json`. Skips `php`, `ext-*` and packages missing from the lock. |
| `cpan` | `requires` modules of `cpanfile` against the latest release on MetaCPAN. |
| `dub` | Selections of `dub.selections.json` against `code.dlang.org` (`/api/packages/<name>/latest`). Skips branch pins such as `~master`. Prints no apply command. |
| `go` | Direct `require` lines of `go.mod` against proxy.golang.org. Skips indirect and replaced modules. |
| `hex` | `mix.lock` against hex.pm. |
| `julia` | `[deps]` of `Project.toml`, as locked in `Manifest.toml`, against the General registry (`Versions.toml`). Skips standard libraries without a locked version. |
| `maven` | Literal `maven.install` coordinates, at their locked version, against Maven Central. Ignores snapshot, alpha, beta, rc and milestone versions. |
| `npm` | `dependencies`, `devDependencies` and `optionalDependencies` against the npm registry. |
| `nuget` | `nuget` lines of `paket.dependencies` against nuget.org. Lists the newest stable version also when in range. |
| `ruby` | Versioned `gem` lines of the Gemfile against rubygems.org. Lists the newest version also when in range. |
| `swiftpm` | Pins against the newest semver tag on GitHub. |
| `uv` | `project.dependencies`, `project.optional-dependencies` and `dependency-groups` against PyPI. |

Other sets print an `update_outdated_unsupported` warning. A `set:package` selector limits the lookup.

Built-in selectors also run with no selector, except `bcr:newest`:

- `github-actions` checks every remote `uses:` pin in `.github/workflows`. `github-actions:<owner>/<repo>` checks one repo. A pin is read from its tag (`@v4`) or from a trailing comment (`@<sha> # v4.1.0`).
- `bazel:.bazelversion` checks `.bazelversion`.
- `containers` compares each `oci.pull` digest in `MODULE.bazel` with the current digest of the image's `latest` tag. `containers:<name>` checks one pull. Apply a new digest with `dx update containers:<name> sha256:<digest> --apply`.
- `bcr:newest` writes `core/modules/bcr_newest.json` from the newest stable BCR version of each `bazel_dep` in `MODULE.bazel`.
- Pins with no readable version, and repos with no release, print `<set>:<package>: unknown (<reason>)` and an `update_outdated_unknown` warning. The exit code is unchanged.
- Set `GH_TOKEN` or `GITHUB_TOKEN` to raise GitHub's limit of 60 requests an hour. The token goes to `api.github.com` only.

Text prints one line per package: declared version, newest version, publish time and the apply command. Ruby lines say `edit Gemfile, then bundle lock`. JSON emits `command_started`, one `update_outdated` notice per package (`scope` is `<set>:<package>`, `related_command` is `dx update <set>:<package> <version>`, absent for Ruby), one `update_outdated_lookup_failed` error per failed lookup, and `command_finished`.

Apply a listed version with `dx update <set>:<package> <version> --apply`. Refresh in-range versions with `dx update --apply <set>`.

When a `maven` resolution picked another version than an artifact asks for, it prints `<set>:<group>:<artifact>: overridden (...)` and an `update_outdated_overridden` warning naming both. The exit code is unchanged.

Unused and missing dependencies:

- For `cargo`, `npm`, `uv`, `go` and `ruby` sets, `--outdated` also lists declared dependencies that nothing under the manifest's directory uses: `unused: <set>: <name> (declared in <manifest>:<line>)`. JSON emits an `update_unused` warning with `scope` `<set>:<name>`.
- `cargo` sets with a hub: an entry is unused when no target depends on its hub label, found with one `bazel query` per set, so `build.rs` and `select` branches count. A crate also counts as used through its binary tool label (`@crates//:ripgrep__rg`). If the query fails, dx scans the sources instead.
- `go`, `ruby` and `cc` sets: an entry is used when any target's deps name it.
- `npm`: a dependency is used when a BUILD target depends on its `node_modules` label.
- Entries in the manifest's own ignore list are not reported: `[package.metadata.cargo-machete] ignored` for `cargo`, `[tool.deptry.per_rule_ignores] DEP002` for `uv`.
- Targets tagged `dx-keep` are skipped.
- `go` sets also list imports under the manifest's directory that no `require` covers: `missing: <set>: <import> (imported in <file>:<line>)`. Nested modules, `vendor` and `testdata` are skipped. Add a `require` to `go.mod` to fix it.
- `cargo`, `npm`, `uv` and `ruby` sets list packages that the lock (`Cargo.lock`, `pnpm-lock.yaml`, `uv.lock`, `Gemfile.lock`) holds but the manifest does not declare, when a source under the manifest's directory imports them. Workspace and path members are not candidates. A package imported under another name (`PyYAML` as `yaml`) is not found.
- `cargo` sets with a hub also list a hub label that a target depends on but the manifest does not declare: `missing: <set>: <name> (used by <label>)`, with the first such target. A crate both checks find prints once.
- JSON emits an `update_missing` warning per missing entry.
- The run exits `1` when any unused or missing entry is listed, unless `--fail-on=error`.
- Fix them with `dx update --apply --prune` and `dx update --apply --add-missing`.

`--offline` fails with `offline_required`.

| Code | Meaning |
| --- | --- |
| `0` | Lookups finished, also with outdated packages. |
| `1` | A lookup failed, or an unused or missing entry at `--fail-on`. |
| `2` | Usage error. |

### Pull requests

```sh
tools/dx update --pr
tools/dx update --pr --automerge=none --max-prs=2
```

`--pr` needs `git`, an authenticated `gh`, a clean working tree and a checked-out branch. That branch is the pull request base. It covers the same sets as `--outdated`, including GitHub Actions pins and `.bazelversion`.

Each dependency set gets one pull request on `dx-update/<set>`, such as `dx-update/github-actions`. Characters other than letters, digits, `.`, `_` and `-` become `-` in branch names.

For each set, dx:

1. Resets the branch to the base commit.
2. Widens each package, then runs `dx update --apply <set>`. GitHub Actions pins and `.bazelversion` are only widened.
3. Runs `dx check --affected` and `dx test --affected`.
4. Commits. If `origin` has the same files on the same base and a pull request is open, it prints `unchanged #<n>` and stops.
5. Pushes with `--force-with-lease`. If the base moved or a newer version appeared, it rebuilds and pushes again.
6. Edits the open pull request or creates one.
7. Sets auto-merge by `--automerge`.

A set that fails any step is reset and not pushed; the next set runs. The base branch is checked out again at the end.

Auto-merge:

- It runs `gh pr merge --auto --squash`. The repository must allow auto-merge, or the line ends with `auto-merge not enabled: ...` and the exit code is unchanged.
- For a `0.x` version, a minor bump counts as major.
- A refresh that raises a pull request above the level turns auto-merge off.
- A version with no publish date is kept, and the body says `age unknown`.

The body lists each package as `old -> new` with:

- its registry link;
- compare and release links when the registry names a GitHub repository (cargo and ruby read it from the crates.io and rubygems.org metadata). Tag names come from `git ls-remote --tags`, matching `v`, `<package>@v`, `<package>-v` and `<package>-` forms. With no match, the body links the releases page only. A failed metadata fetch leaves only the registry link;
- a `prerelease` mark;
- the changed lock files with line counts.

Skips:

- A skipped package prints `skipped <set>:<package> <old> -> <new>: <reason>` with `major` or `too new`. JSON emits an `update_pr_skipped` notice.
- A set over `--max-prs` prints `skipped: --max-prs=<n> reached`.
- A push refused for workflow files prints `skipped: token cannot push workflow files`.
- Skips do not change the exit code.
- If the token may not create pull requests, the run fails and names the setting "Allow GitHub Actions to create and approve pull requests".

After the sets, dx closes open `dx-update/` pull requests on the base branch that this run covers but that have no updates left. It deletes the branch, comments `dx update: no updates left for this group.` and prints `closed #<n>`. Pull requests from forks are left alone.

| Code | Meaning |
| --- | --- |
| `0` | Every set published, unchanged or closed, or nothing outdated. |
| `1` | A set, close or lookup failed. |
| `2` | Usage error. |

### Widen or add one package

```sh
tools/dx update --apply crates:anyhow 1.0.100
tools/dx update --apply go:github.com/google/go-cmp 0.7.0
tools/dx update --apply bazel:rules_cc 0.3.0
tools/dx update --apply --floor --reason "needs 0.3 API" bazel:rules_cc 0.3.0
tools/dx update --apply web:vue 3.5.42
tools/dx update --apply web:left-pad
```

One `set:package` and one version widen that requirement, or declare the package when the set lacks it. A version starts with a digit, or with `v` and a digit, or is a commit SHA. The set is a [dependency set](#dependency-sets) or a built-in: `bazel`, `containers`, `github-actions` or `pins`. Any other set exits `2` and lists the valid names.

- Without `--apply`, it exits `0` when the manifest already declares the version, and `1` with `update drift` (widen) or `update pending` (add).
- A version already declared reports `update_no_change`, writes nothing and exits `0`, also under `--frozen` or `--offline`.
- `github-actions` and `bazel:.bazelversion` cannot add.
- `--apply set:package` with no version declares a missing `cargo`, `npm`, `uv` or `ruby` package at its latest stable version, prints `dx: update_added: ...` and locks it. A package the set already declares stays a plain selector. `--offline` and `--frozen` skip the add.

| Set | Edit |
| --- | --- |
| `bazel` | Tested version in the `single_version_override`, keeping patches. Falls back to the `bazel_dep` when there is no override or no `version`. `--floor --reason <text>` edits the `bazel_dep` version and the `# floor: <text>` lines above it. |
| `containers` | The `oci.pull` digest. |
| `github-actions` | A tag resolves to its commit SHA through `api.github.com` (`GH_TOKEN` or `GITHUB_TOKEN` when set). The SHA is written in every workflow that uses the action; a `# v4` comment becomes the tag. `--offline` fails a tag with `offline_required`; pass a SHA. |
| `pins` | A `pins.json` that a top-level `<dir>/lang.json` lists: version, URLs and sha256 of every entry. |
| `cabal` | Bound in `build-depends`, then resolves again. |
| `cargo` | Crate in every manifest that declares it, then `dx update --apply <set>`. |
| `composer` | Constraint in `require` or `require-dev` of `composer.json`, keeping the operator, then `composer update <package> --with-all-dependencies --no-install` and the digest refresh. |
| `cpan` | Version on the module's `requires` line of `cpanfile`, keeping the operator, then `carton update <Module>` and the cpan lock step. Versions are decimal (`2.06`, `1.002_003`) or dotted (`v1.0.35`). |
| `go` | `require` line of `go.mod`, then regenerates `go.sum`. If go fails or would change `go.mod`, the widen stays, `go.sum` is untouched, and the report names `go mod tidy` then `dx update <set>`. |
| `hex` | Requirement of the `{:name, "<op> x.y.z"}` tuple in `mix.exs`, keeping the operator, then `mix deps.update <package>` with the pinned mix. |
| `julia` | `[compat]` entry in `Project.toml`. See [julia](#julia). |
| `maven` | Literal coordinate in the set's `maven.install`, then `@<set>//:pin`. Use `<set>:<group>:<artifact> <version>`. |
| `npm`, `uv` | `package.json` or `pyproject.toml`, then refreshes the lock. |
| `nuget` | `paket.dependencies` only. Run `paket update`, then `dx update <set>`. |
| `opam` | Constraint in `depends:` of the `.opam` file. See [opam](#opam). |
| `pub` | Constraint in `dependencies` or `dev_dependencies` of `pubspec.yaml`, then `dart pub upgrade <package>`. See [pub](#pub). |
| `ruby` | Literal requirement on the gem's Gemfile line, then the ruby lock step with the gem upgraded. |
| `swiftpm` | `from:` or `exact:` version in `Package.swift`, then `swift package update <package>`. See [swiftpm](#swiftpm). |

A package that is not declared, or whose constraint is not one literal version, is refused unless the set can add it.

A missing package goes into the same manifest in the same shape: a `bazel_dep` after the last one, a `[dependencies]` entry, a `require` line, a `maven.install` coordinate, a `dependencies` entry in `package.json`, a `nuget` line, a `name>=version` requirement in `[project].dependencies` of `pyproject.toml`, or a `gem "name", "~> <major>.<minor>"` line after the last top-level `gem` in the `Gemfile`.

Refresh:

- `cargo`, `go`, `maven`, `npm` and `nuget` refresh through `dx update --apply <set>`. A failed refresh keeps the widen and exits `1`.
- `bazel`, `containers`, `github-actions` and `pins` refresh nothing. Review the diff and run `bazel build //...`.
- When a widen is pending, a set that needs a refresh fails before widening: `offline_required` under `--offline`, `frozen_locked` under `--frozen`.

`--floor` is refused with exit `2` and writes nothing when:

- `--reason` is missing, or `--reason` is given without `--floor`.
- The set is not a `bazel_dep`, including `.bazelversion`.
- The floor would exceed the tested version, or the tested version would fall below the floor.

Duplicate declarations, a missing or computed version or name, and a `MODULE.bazel` dx cannot parse fail without writing. A plain `dx update` never touches a `bazel_dep` floor or override.

JSON emits an `add_declared` notice for an insert, an `update_widened` notice per written manifest, and an `update_set_success` notice after the lock refresh.

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | A pending widen without `--apply`, or an operational failure. |
| `2` | Usage or scope error, or an unknown set. |

### Advisory pins

```sh
tools/dx update advisory
tools/dx update advisory --apply
tools/dx update advisory --pr
```

The `advisory` selector moves the `advisory` lines of `.dx/pins` to the newest release of `devhusdk/rules_dx-advisory`. It reads that release through the GitHub API and downloads no snapshot.

- Without flags it writes nothing. It prints, per pinned family, the pin date, its age and whether the newest release is newer, then the branch `dx-update/advisory`, the pin diff, and the advisories `dx security` reports with the new pins and not with the old ones.
- `--apply` rewrites only the `advisory` lines whose family the release has a pin for. Every other line stays.
- `--pr` does the same on the branch `dx-update/advisory` and opens or refreshes its pull request, with the pin diff and the new advisories in the body. It closes the pull request when the pins are current. It needs the same `git` and `gh` as other pull requests and never turns on auto-merge.

To get bump pull requests on a schedule, call the reusable workflow from a scheduled workflow with `advisory_pr: true`. Its `advisory-bump` job runs `dx update advisory --pr`.

To review a bump pull request, read the diff of `.dx/pins` and the list of new advisories in the body. Fix each new advisory, or accept it with `dx security --accept`, before you merge. A pull request with no new advisories only refreshes the data.

For rules_dx maintainers: `.github/workflows/advisory-publish.yml` builds the snapshots every day with `advisory_prep --pin`, creates the release `YYYY-MM-DD` with the archives as assets and the pin lines as the body, and runs `dx update advisory --pr`. It publishes nothing when every snapshot sha256 equals the newest release. It needs the `devhusdk/rules_dx-advisory` repository and the `ADVISORY_RELEASE_TOKEN` secret, a token with release write access to it.

## Dependency Sets

`dx update`, `dx security` and `dx license` read sets from the hubs in the root `MODULE.bazel` and the files it `include()`s.

| Hub | Set name | Manifests | Locks |
|---|---|---|---|
| `crate.from_cargo` with `cargo_lockfile` | `name` | `manifests` | `cargo_lockfile`, `lockfile` |
| `maven.install` with `lock_file` | `name`, default `maven` | `MODULE.bazel` | `lock_file` |
| `npm.npm_translate_lock` with `pnpm_lock` | `name` | `package.json` beside the lock | `pnpm_lock` |
| `uv.project` | `<hub_name>-<pyproject directory name>`, or `<hub_name>` at the workspace root | `pyproject` | `lock` |
| `go_deps.from_file` with `go_mod` | `go` | `go_mod` | `go.sum` beside it |
| `ruby.lock` of `@rules_dx//ruby:extensions.bzl` | `name` | `Gemfile` beside the lock | `lock` and `Gemfile.lock` beside it |
| `composer.lock` of `@rules_dx//php:extensions.bzl` | `name` | `composer.json` beside the lock | `lock` |
| `dub.lock` of `@rules_dx//d:extensions.bzl` | `name` | `dub.json` beside `lock` | `lock` |
| `fpm.lock` of `@rules_dx//fortran:extensions.bzl` | `name` | `fpm.toml` beside the lock | `lock` |
| `luarocks.lock` of `@rules_dx//lua:extensions.bzl` | `name` | `lock` | |
| `zig.lock` of `@rules_dx//zig:extensions.bzl` | `name` | `build.zig.zon` beside `lock` | `lock` |
| `cpan.lock` of `@rules_dx//perl:extensions.bzl` | `name` | `cpanfile` beside the lock | `cpanfile.snapshot` and `cpan.lock.json` |
| `pub.from_lock` of `@rules_dart//dart/pub:extensions.bzl`, `@rules_flutter//flutter/pub:extensions.bzl` or `@rules_dx//flutter:extensions.bzl`, `flutter.pub` of rules_flutter | `name` | `pubspec.yaml` beside the lock | `pubspec.lock` |
| `julia.from_manifest` of `@rules_dx//julia:extensions.bzl` or `pkg.from_manifest` of `@rules_julia//julia/pkg:extensions.bzl` | `name` | `Project.toml` beside the lockfile | `Manifest.toml` and `Manifest.bazel.json` beside the lockfile |
| `nuget.lock` of `@rules_dx//csharp:extensions.bzl` | `nuget` | `paket.dependencies` | `nuget.lock.json`, `paket.lock` |

The sections below also cover `swiftpm`, `opam`, `hex`, `cabal`, `ansible` and `bicep` sets.

- Hub arguments must be literal. A computed argument, two sets with one name, or two sets that own one manifest or lock exit `2` before anything runs.
- With no hubs, `dx update` exits `2` with `no dependency sets`, and `dx security` fails and prints an example hub.

### npm

```starlark
bazel_dep(name = "aspect_rules_js", version = "3.6.0")

npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(
    name = "web",
    pnpm_lock = "//apps/web:pnpm-lock.yaml",
)

pnpm = use_extension("@aspect_rules_js//npm:extensions.bzl", "pnpm")
use_repo(pnpm, "pnpm")
```

- Hubs without `pnpm_lock` are skipped. The root `MODULE.bazel` must load pnpm as shown.
- `--apply` runs `bazel run @pnpm//:pnpm -- --dir <set directory> install --lockfile-only` and writes that `pnpm-lock.yaml` only. With `set:package` it runs `update <packages> --lockfile-only`.
- A check runs `install --lockfile-only --frozen-lockfile`, writes nothing, and fails with `updater exited <code>` when the lock is stale. A check with `set:package` is unsupported.
- User and global `.npmrc` files are ignored. A `.npmrc` in the set directory applies.
- The set is unsupported when `locks` omits `<set directory>/pnpm-lock.yaml`, or when a parent `pnpm-workspace.yaml` makes it a member. A set with its own `pnpm-workspace.yaml` is its own root. A set under a parent whose `packages` is exactly `["."]` is accepted and runs with `--ignore-workspace`.
- `dx license` reports `UNKNOWN` unless `license.toml` has `[[inventory]]` with `set = "npm"`.

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

- Each tag with a literal `cargo_lockfile` is one set. dx ships no cargo; it uses your workspace's rules_rust.
- Set `lockfile` and list it in `locks`. Without it, the hub always repins and the check ends in `check_wrote`.
- One `Cargo.toml` must sit beside `Cargo.lock`. The tag must be imported with `use_repo`, renamed or not, or the set is unsupported (exit `1`).
- `--apply` runs `bazel fetch --repo=@<visible hub name>` with `CARGO_BAZEL_REPIN=workspace`, `REPIN=workspace`, `CARGO_BAZEL_REPIN_ONLY=<hub name>` and `CARGO_BAZEL_ISOLATED=true`.
- A check sets both repin variables to `false`, so a shell `REPIN` cannot turn a check into a write. It sets `CARGO_BAZEL_DIGEST_CHECK=1` so crate_universe verifies `cargo-bazel-lock.json` against its inputs. A stale lock fails with `updater exited <code>`.
- `--apply <set>:<crate>` repins one crate. Select one crate per cargo set per run. Repin-mode names (`all`, `workspace`, `false`, `0`, `off`) are refused. A crate with several locked versions fails with `updater exited <code>`. A check with a package is unsupported.
- Bazel evaluates every `crate.from_cargo` hub together, so one stale hub fails the check and update of every cargo set. Update the stale set first.
- `dx security` matches `Cargo.lock` crates against the `cargo` snapshot. `dx license` reads licenses from the declared `*.json` lock; `[[inventory]]` with `set = "cargo"` overrides them.

### maven

```starlark
maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = ["com.google.guava:guava:33.4.0-jre"],
    known_contributing_modules = ["protobuf"],
    lock_file = "//third_party:maven_install.json",
)
use_repo(maven, "maven")
```

- Needs rules_jvm_external 7.1 or later and a `use_repo` entry without a rename. Several sets share `MODULE.bazel` as manifest.
- `@maven` holds what you list plus the few artifacts protobuf adds to it. `known_contributing_modules` silences the warning about protobuf's.
- `dx generate --apply` creates what the lock needs: `third_party/BUILD.bazel` when it is missing, the lock pinned with `bazel run @maven//:pin` when it is missing or empty, and `# gazelle:dx_external maven third_party/maven_install.json maven` in the root `BUILD.bazel`. A lock at the root needs no directive. With a renamed `use_repo` it only prints what to add.
- `--apply` runs `bazel run @<repository>//:pin` with `RULES_JVM_EXTERNAL_REPIN=1`. A check runs `bazel fetch --repo=@<repository>`; a stale lock fails with `updater exited <code>`.
- A check is unsupported when `fail_if_repin_required` is `False` or computed, the lock is empty or lacks `__INPUT_ARTIFACTS_HASH` and `__RESOLVED_ARTIFACTS_HASH`, or the host sets `REPIN` or `RULES_JVM_EXTERNAL_REPIN`.
- Host `COURSIER_MIRRORS`, `COURSIER_REPOSITORIES`, `COURSIER_CREDENTIALS`, `COURSIER_CONFIG_DIR` and `SCALA_CLI_CONFIG` are refused.
- A set takes no `set:package` for update or check. Widen with `dx update <set>:<group>:<artifact> <version>`.
- `--outdated` compares the version in the lock, not the one in `MODULE.bazel`.
- `dx security` matches the lock's `artifacts` against the `maven` snapshot. `dx license` needs `[[inventory]]` with `set = "maven"`.

### nuget

```starlark
nuget = use_extension("@rules_dx//csharp:extensions.bzl", "nuget")
nuget.lock(
    name = "nuget",
    lock = "//third_party/dotnet:nuget.lock.json",
)
```

- The manifest is `paket.dependencies` beside the lock. The locks are `nuget.lock.json` and the `paket.lock` beside it.
- `dx update --apply <set>` runs the pinned paket resolver, which writes both locks. `dx update` runs it in check mode.
- `--apply --frozen` fails with `frozen_locked` and `--offline` fails with `offline_required`. A set takes no `set:package` for update.
- `dx security` matches `paket.lock` against the `nuget` snapshot. `dx license` needs `[[inventory]]` with `set = "nuget"`.

### uv

dx runs the pinned uv `0.13.7` and the pinned Python interpreter.

- `DX_UV_BIN` and `DX_UV_PYTHON` take absolute paths to override them. Otherwise dx resolves `@dx_tools//:uv` and `@rules_dx//cli/update:python_interpreter` through Bazel.
- A relative path is rejected, `PATH` is never searched, and a uv that does not report `0.13.7` is rejected. With no usable tool, the set fails with `backend_unavailable` and the run exits `1`.

```sh
export "$(bazel run @rules_dx//dx/tool_resolve -- --label @dx_tools//:uv --var DX_UV_BIN --print)"
export "$(bazel run @rules_dx//dx/tool_resolve -- --label @rules_dx//cli/update:python_interpreter --var DX_UV_PYTHON --print)"
```

- `--apply` runs `uv lock` in the set directory and writes that lock only. A check runs `uv lock --check` and fails with `updater exited 1` when stale.
- Both pass `--no-config --no-cache --no-python-downloads --python <DX_UV_PYTHON>` with an empty environment, so `HOME`, `XDG_CONFIG_HOME`, `UV_*` and user `uv.toml` are ignored. A check with `--offline` adds `--offline`; a lock that needs a download fails with `updater exited 2`.
- A `uv.toml` in the set directory or a parent inside the workspace makes the set unsupported. So does a `[tool.uv]` key other than `sources`, `workspace`, `dev-dependencies`, `default-groups`, `conflicts`, `package`, `managed`, `override-dependencies` or `constraint-dependencies`.
- `set:package` without a version is unsupported: the set refreshes as a whole.
- `dx security` matches every `uv.lock` package against the `pypi` snapshot with PEP 440 versions and PEP 503 names. Workspace members are skipped. Packages from git, path, url, directory or a registry other than pypi.org are unassessed and the run exits `1`. A `security.toml` exception uses `set = "<name>"` and a PEP 440 `versions` range.

### go

```starlark
go_deps = use_extension("@gazelle//:extensions.bzl", "go_deps")
go_deps.from_file(go_mod = "//third_party/go:go.mod")
```

- The `go.mod` must parse and have no local-path `replace`. The root `MODULE.bazel` needs `bazel_dep(name = "rules_go")` with no `repo_name`. A root `MODULE.bazel` that mentions `go_env` makes the set unsupported.
- dx copies `go.mod` and `go.sum` to a scratch directory and runs `bazel run @rules_go//go -- -C <scratch>/out mod download all` with `GOENV=off`, `GOWORK=off`, `GOTOOLCHAIN=local`, `GOFLAGS=-modcacherw` and an empty module cache. It needs Go 1.20 or newer. It never edits `go.mod`; a changed `go.mod` fails the set.
- A check compares the generated `go.sum` with the committed one. `--apply` writes `go.sum` when the bytes differ. Entries are added, never pruned; prune with `go mod tidy`.
- It uses Go's public proxy and checksum database. Private modules are unsupported. A host `GOPROXY`, `GOPRIVATE`, `GONOPROXY`, `GONOSUMDB`, `GONOSUMCHECK`, `GOSUMDB`, `GOINSECURE`, `GOVCS` or `GOAUTH` makes the set unsupported.
- `--offline` fails with `offline_required` in both modes. `--apply --frozen` fails with `frozen_locked`; a check with `--frozen` runs.
- A set takes no `set:package` without a version. Edit the `require` in `go.mod` and run `dx update <set>`, or run `dx update <set>:<module> <version>`.
- `dx security` matches `go.mod` requirements against the `go` snapshot.

### ruby

```starlark
ruby = use_extension("@rules_dx//ruby:extensions.bzl", "ruby")
ruby.lock(
    name = "gems",
    lock = "//apps/rb:ruby.lock.json",
)
```

- `dx update` runs the pinned Ruby resolver in check mode: current, or failed when stale.
- `dx update --apply <set>` runs the pinned Ruby resolver, which rewrites the lock. `<set>:<gem>` upgrades each named gem. A check with `<set>:<gem>` is refused.
- `--offline` and `--frozen` are refused.
- `dx security` matches `Gemfile.lock` against the `rubygems` snapshot. `dx license` needs `[[inventory]]` with `set = "ruby"`.

### julia

```starlark
julia = use_extension("@rules_dx//julia:extensions.bzl", "julia")
julia.from_manifest(
    name = "julia_app_pkgs",
    lockfile = "//julia/app:Manifest.bazel.json",
)
```

- `dx update <set>:<Package> <version>` sets the package's `[compat]` entry in `Project.toml`, keeping a leading `=` or `~`, and adds the entry when it is absent. A package not in `[deps]`, or an entry that lists several ranges, is refused. `--apply` then runs `Pkg.update` and the set's `julia_pkg_resolve`.
- `dx security` reads the versioned packages of `Manifest.toml` (format 2.0, stdlibs skipped) and matches them against the `Julia` advisories.
- The layout check needs both locks to name the same versions.

### composer

```starlark
composer = use_extension("@rules_dx//php:extensions.bzl", "composer")
composer.lock(
    name = "php_deps",
    lock = "//apps/php:composer.lock.json",
)
```

- `composer.json` must sit beside `composer.lock.json`.
- `dx update <set>:<vendor/name> <version>` edits the constraint in `composer.json` and runs the pinned Composer resolver, which rewrites `composer.lock.json` and `composer.lock`.
- `dx security` matches `composer.lock` against the `packagist` snapshot.

### cpan

```starlark
cpan = use_extension("@rules_dx//perl:extensions.bzl", "cpan")
cpan.lock(
    name = "perl_app_cpan",
    lock = "//perl/app/third_party:cpan.lock.json",
)
```

- The `cpanfile` and `cpanfile.snapshot` sit beside `cpan.lock.json`. The distributions of the snapshot and the lock must match, or the layout check fails.
- A check runs `carton install --deployment` in the set's directory through the pinned carton, with carton's `local/` tree outside the workspace. `--offline` fails with `offline_required`. A check takes no `set:package`.
- `dx update <set>:<Module> --apply` runs `carton update <Module>`, then the pinned cpan resolver, so the snapshot and `cpan.lock.json` change together. `--offline` and `--frozen` fail with `offline_required` and `frozen_locked`.
- `dx update <set>:<Module> <version> --apply` first rewrites the version on the module's `requires` line, keeping the operator, then refreshes as above.
- `dx security` matches the distributions of `cpanfile.snapshot` against the `cpan` snapshot. `advisory_prep` builds it from `CPANSA::DB` through a pinned CPANSA dump, not from the host perl.

### pub

- A pub set is pinned. `dx security` reads the hosted packages of `pubspec.lock` and matches them against the `Pub` advisories.
- `dx update <set>:<package> <version>` rewrites the package's constraint in `dependencies` or `dev_dependencies` of `pubspec.yaml`, keeping the `^`, exact or `>=` form, then runs `dart pub upgrade <package>`. A package that is not declared there, or is declared as a map such as `sdk: flutter`, is refused.

### luarocks

- `dx security` lists the rocks of `luarocks.lock.json` and reports the set as uncovered.
- `dx update --apply <set>` runs the pinned luarocks resolver, which resolves the `dependencies` of the `*.rockspec` from luarocks.org and rewrites `luarocks.lock.json`. `dx update` runs it in check mode.

### dub

```starlark
dub = use_extension("@rules_dx//d:extensions.bzl", "dub")
dub.lock(
    name = "dub_deps",
    lock = "//d:dub.lock.json",
)
```

- `dub.json` must sit beside `dub.lock.json`.
- `dx update --apply <set>` runs the pinned dub resolver, which resolves `dub.json` against `code.dlang.org`, downloads each archive and rewrites `dub.lock.json` and `dub.selections.json`. `dx update` runs it in check mode. `<set>:<package>` is refused.
- `dx security` and `dx license` list the packages of `dub.selections.json`. There is no advisory source for dub.

### fpm

```starlark
fpm = use_extension("@rules_dx//fortran:extensions.bzl", "fpm")
fpm.lock(
    name = "fpm_deps",
    lock = "//f:fpm.lock.json",
)
```

- `fpm.toml` must sit beside `fpm.lock.json`.
- `dx update --apply <set>` runs the pinned fpm resolver, which resolves the git dependencies of `fpm.toml` and of every archive they name and rewrites `fpm.lock.json`. `dx update` runs it in check mode. It updates the whole set; `<set>:<package>` is refused, and `dx update --outdated` checks no registry.
- `dx security` and `dx license` list the packages of `fpm.lock.json`. There is no advisory source for fpm.

### zig

With `zig.lock.json` beside the root `build.zig.zon`, `dx generate --apply` adds this:

```starlark
zig = use_extension("@rules_dx//zig:extensions.bzl", "zig")
zig.lock(
    name = "zig",
    lock = "//:zig.lock.json",
)
use_repo(zig, "zig")
```

- `zig.lock.json` is `lock`; `build.zig.zon` is the manifest and must sit beside it.
- `dx update --apply <set>` runs the pinned Zig resolver, which downloads each archive dependency and rewrites `zig.lock.json`. `dx update` runs it in check mode. It does not move versions, `<set>:<package>` is refused, and `dx update --outdated` checks no registry.
- `dx security` has no advisory source for zig.

### opam

An opam set comes from a `<name>.opam` file with a `<name>.opam.locked` file beside it.

- `dx update <set>:<package> <version>` edits the one literal version constraint of the package in `depends:` of the `.opam` file. It keeps the operator and the filters (`{>= "1.1.0" & with-test}`). It refuses a package that `depends:` does not list, one without a constraint, and one with two constraints. The version starts with a digit and may hold `.`, `+`, `-`, `~` and `_`, such as `1.3.0` or `0.9.0~rc1`.
- `--apply` then runs `opam upgrade <package>` and `opam lock` from the pinned opam and the opam-repository snapshot. It fails with `offline_required` under `--offline` and `frozen_locked` under `--frozen`.

### swiftpm

A swiftpm set comes from a `swiftpm.lock` tag in `MODULE.bazel` and is named after it. Its manifest is `Package.swift` beside the lock file. Its locks are `Package.resolved` and the lock file.

- `dx update <set>` resolves `Package.swift` with the managed Swift in a copy and compares the result with `Package.resolved`.
- `dx update --apply <set>:<package> <version>` edits the `from:` or `exact:` version of the package, then runs `swift package update <package>`. The package is named by its identity: the last segment of its URL, without `.git`. A package that is not declared, or is pinned by `branch:`, `revision:` or `path:`, is refused.
- `dx security` matches `Package.resolved` against the `swifturl` snapshot.

### hex

- `mix.exs` must sit beside `mix.lock`.
- `dx update <set>` checks the lock with `mix deps.get`. `dx update --apply <set>` runs `mix deps.update --all`. Both use the pinned mix.
- `dx update <set>:<package> <version>` edits the requirement in the `deps` of `mix.exs`, keeping the operator, then runs `mix deps.update <package>`.

### cabal

```starlark
cabal = use_extension("//haskell:extensions.bzl", "cabal")
cabal.lock(name = "hackage", lock = "//third_party:cabal.lock.json")
```

- The manifest is `cabal.project` in the package of the `deps` label. The locks are `cabal.project.freeze` and the `cabal.lock.json` beside it.
- A check runs the cabal lock step with `--check`. `--apply` runs it and keeps both lock files in step. `--offline` fails with `offline_required`; `--apply --frozen` fails with `frozen_locked`.
- A set takes no `set:package` without a version. `dx update <set>:<package> <version> --apply` rewrites the bound in `build-depends` first, then resolves again.
- `dx security` matches the `any.<name> ==<version>` pins of `cabal.project.freeze` against the `hackage` snapshot. Flags and `installed` boot packages are skipped.
- A `stack.yaml.lock` next to `cabal.project` joins the set's locks. `dx security` also reads the `hackage:` entries it completes under `packages:`.

### ansible

```starlark
ansible = use_extension("@rules_dx//ansible:extensions.bzl", "ansible")
ansible.lock(
    name = "ansible_collections",
    lock = "//:ansible.lock.json",
)
```

- `ansible.lock.json` is `lock`; `requirements.yml` is the manifest and must sit beside it. Each `collections` entry has a `name` and an exact `version`, and every dependency of a collection is listed too.
- `dx update --apply <set>` runs the pinned Galaxy resolver, which downloads each Galaxy archive and rewrites `ansible.lock.json`. `dx update` runs it in check mode. It does not move versions, and `<set>:<namespace.collection>` is refused. `--offline` fails with `offline_required`; `--apply --frozen` fails with `frozen_locked`.

### bicep

```starlark
bicep = use_extension("@rules_dx//bicep:extensions.bzl", "bicep")
bicep.lock(
    name = "bicep",
    lock = "//infra:bicep.lock.json",
)
use_repo(bicep, "bicep")
```

- The manifests are the `*.bicep` files beside `bicep.lock.json`. Each `br:HOST/PATH:TAG` or `br/public:PATH:TAG` module reference is locked to its manifest and layer sha256. One module takes one tag per set. A check and an update report `pinned`.
- To move a module, edit the tag in the `.bicep` file and run `dx generate --apply`. dx never picks a registry tag.
- A locked module is `@bicep//HOST/PATH`, for example `@bicep//mcr.microsoft.com/bicep/avm/res/storage/storage-account`.
