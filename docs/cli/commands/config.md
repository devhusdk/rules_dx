# Config Reference

dx reads two files from the workspace root: `security.toml` for `dx security`
and `licenses.toml` for `dx license`. Both are optional. Each is merged over a
base that ships with dx; the top-level `tier` in `dx.toml` picks it:
`standard` (the default) or `strict`. An unknown key fails the run. Defaults come from flags and `DX_` variables (see
[Command Reference](README.md)).

In every table, a key without a default is required. Dates are `YYYY-MM-DD`.
`versions` uses the range syntax of the `set` ecosystem.

## dx.toml `[large_files]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `allow` | array of strings | `[]` | Paths or `dir/` prefixes the large-file check skips. Lock files are always skipped. |

## dx.toml `[check]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `commits` | string | off | `"conventional"` makes `dx check --affected` warn about each commit subject since the merge base that is not a conventional commit. See [Check, Clean](check-clean.md). |

## security.toml

Used by `dx security`. See [Audit, Update](audit-update.md).

### security.toml top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer | none | Must be `1`. |
| `exception` | array of tables | none | Accepted advisories. |
| `uncovered` | array of tables | none | Accepted sets with no advisory source. |
| `fail_on` | array of strings | the tier's | Advisory kinds that fail the run: `vulnerability`, `unsound`, `unmaintained`, `notice`, `yanked`. Other kinds are reported as `info`. Standard fails on `vulnerability` and `unsound`; strict adds `unmaintained`, `notice` and `yanked`. A list set here replaces the tier's. |

### security.toml `[[exception]]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `advisory` | string | none | Advisory id, for example `GHSA-aaaa-bbbb-cccc`. |
| `package` | string | none | Package the advisory hits. |
| `set` | string | none | Dependency set that owns the package. |
| `versions` | string | none | Affected version range. |
| `reason` | string | none | Why the risk is accepted. |
| `expires` | string | none | Date the exception ends. |

Two sets are not dependency sets:

- `set = "oci"` covers a finding from the `oci_deploy` image scan. `package` is the image package and `versions` is `*` or a range holding the installed version.
- `set = "trivy_config"` covers a `trivy_config` lint finding. `advisory` is the trivy ID, such as `DS-0002`, `package` is the workspace-relative file or `*` for every file, and `versions` must be `*`. `dx lint` drops covered findings until `expires`.

### security.toml `[[uncovered]]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `set` | string | none | Dependency set with no advisory source. |
| `reason` | string | none | Why the gap is accepted. |
| `expires` | string | none | Date the acceptance ends. |

## licenses.toml

Used by `dx license`. See [Audit, Update](audit-update.md).

A list this file sets replaces the tier's list; a list it leaves out comes
from the tier. The strict tier denies what standard only reviews.

### licenses.toml top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer | none | Must be `1`. |
| `policy` | table | empty | License tiers. |
| `distribution` | table | empty | Which scopes are internal. |
| `exception` | array of tables | none | Accepted license findings. |
| `inventory` | array of tables | none | Licenses declared by hand. |

### licenses.toml `[policy]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `blocked` | array of strings | `[]` | License ids that fail every scope. |
| `distributed` | table | empty | Tiers for distributed scopes. |
| `sets` | table of tables | empty | Per-set tiers, keyed by set name. |

### licenses.toml `[policy.distributed]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `allow` | array of strings | `[]` | License ids that pass. |
| `review` | array of strings | `[]` | License ids that need review. |
| `deny` | array of strings | `[]` | License ids that fail. |

### licenses.toml `[policy.sets.<set>]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `review` | array of strings | `[]` | Extra ids that need review in this set. |

### licenses.toml `[distribution]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `internal` | array of strings | `[]` | Label patterns for internal scopes. |

### licenses.toml `[[exception]]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `package` | string | none | Package the finding names. |
| `set` | string | none | Dependency set that owns the package. |
| `license` | string | none | License id being accepted. |
| `versions` | string | none | Affected version range. |
| `reason` | string | none | Why the license is accepted. |
| `expires` | string | none | Date the exception ends. |

### licenses.toml `[[inventory]]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `package` | string | none | Package the entry covers. |
| `set` | string | none | Dependency set that owns the package. |
| `license` | string | none | License id of the package. |
| `versions` | string | none | Version range the entry covers. |
| `text_present` | boolean | `false` | The license text ships with the package. |
| `text` | string | none | Workspace-relative path of a vendored license file. Implies `text_present`. |
