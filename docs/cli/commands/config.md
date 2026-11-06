# Config Reference

dx reads two files from the workspace root: `security.toml` for `dx security`
and `licenses.toml` for `dx license`. Both are optional. An unknown key fails
the run. Defaults come from flags and `DX_` variables (see
[Command Reference](README.md)).

In every table, a key without a default is required. Dates are `YYYY-MM-DD`.
`versions` uses the range syntax of the `set` ecosystem.

## security.toml

Used by `dx security`. See [Audit, Update](audit-update.md).

### security.toml top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer | `1` | Must be `1` when set. |
| `exception` | array of tables | none | Accepted advisories. |
| `uncovered` | array of tables | none | Accepted sets with no advisory source. |
| `extends` | string | none | Workspace-relative path of a base, such as `.dx/config/security/standard.toml`. A `fail_on` set here replaces the base's. `exception` and `uncovered` entries are added to the base's. A base may not have `extends`, `exception` or `uncovered`. A missing base fails; run `dx init --apply`. |
| `fail_on` | array of strings | `["vulnerability", "unsound"]` | Advisory kinds that fail the run: `vulnerability`, `unsound`, `unmaintained`, `notice`, `yanked`. Other kinds are reported as `info`. The strict base adds `unmaintained`, `notice` and `yanked`. |

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

### licenses.toml top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer | `1` | Must be `1` when set. |
| `extends` | string | none | Workspace-relative path of a base, such as `.dx/config/licenses/standard.toml`. A key this file sets replaces the base's key; a key it leaves out is inherited. `policy.sets.<set>` replaces per set, `distribution.internal` replaces, and `exception` and `inventory` entries are added to the base's. A base may not have `extends`, `exception` or `inventory`. A missing base fails; run `dx init --apply`. |
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
