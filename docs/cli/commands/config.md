# Config Reference

dx reads two files from the workspace root: `security.toml` for `dx security`
and `license.toml` for `dx license`. Both are optional. Each is merged over a
base that ships with dx; the top-level `tier` in `dx.toml` picks it:
`standard` (the default) or `strict`. An unknown key fails the run. Defaults come from flags and `DX_` variables (see
[Command Reference](README.md)).

In every table, a key without a default is required. Dates are `YYYY-MM-DD`.
`versions` uses the range syntax of the `set` ecosystem.

## `dx config`

Shows which config file each tool reads and how it relates to the base dx renders under `.dx/config/`.

```sh
tools/dx config
tools/dx config hlint
tools/dx config diff hlint
tools/dx config override hlint
tools/dx config --output json
```

```text
dx config [<tool> | diff <tool> | override <tool>] [--output text|json]
```

| Form | What it prints |
| --- | --- |
| `dx config` | One line per selected tool: the tool, its root file (`-` when the workspace has none), then `extends <base>`, `override of <base>` or `tier base <base>` when the tool runs on the base itself. `edited` follows when the root file starts with a `dx: override of base <sha>` comment. |
| `dx config <tool>` | Every file the tool reads, in order: the root file when there is one, then the base. |
| `dx config diff <tool>` | A unified diff from the current base to the root file, ignoring its `dx: override of base` header line; empty when the copy is unchanged. Fails when the tool has no root file. |
| `dx config override <tool>` | Copies the base to the tool's root file and starts it with a `dx: override of base <git blob sha>` comment. Refuses when the tool has an extends line (it prints the line to add instead) or when the root file already exists. The only form that writes. |

```text
ruff  ruff.toml  extends .dx/config/ruff/base.toml  edited
taplo  -  tier base .dx/config/taplo/base.toml
```

`--output json` emits one `config_file` event per file (`tool`, `file`, `role` as `root` or `base`, `base`, `edited`). The list form emits the root file of each tool, or the base when the tool has no root file. An unknown tool fails with `config_failed`.

## dx.toml

Picks the tier, the tools of each family, and what dx leaves out. Optional:
with no `dx.toml`, every family found in the tree runs its curated tools at the
standard tier. dx writes `.dx/` from it; never edit `.dx/`. The JSON Schema is
[`dx.schema.json`](https://github.com/devhusdk/rules_dx/blob/main/cli/dx_config/schema/dx.schema.json).

```toml
schema_version = 1
tier = "strict"

[families.rust]
tier = "standard"
lint = ["clippy", "typos"]

[families.go]
disabled = true

[scope]
exclude = ["//third_party/..."]
```

### dx.toml top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer | none | Must be `1`. |
| `tier` | string | `"standard"` | `standard` or `strict`. The tier for every family that does not set its own, and for `security.toml` and `license.toml`. |
| `skip_tools` | array of strings | `[]` | Tool ids dropped from every family, such as `["typos"]`. An unknown id fails the run. |
| `families` | table | empty | Per-family settings, keyed by family id. An unknown id fails the run. |
| `scope` | table | empty | What dx leaves out. |
| `large_files` | table | empty | See below. |
| `check` | table | empty | See below. |

### dx.toml `[families.<id>]`

Replaces the curated tools of one family. A capability left out keeps the
curated tools; see [Default tools](quality.md#default-tools).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `tier` | string | top-level `tier` | `standard` or `strict` for this family. |
| `lint` | array of strings | curated | Tool ids that lint the family. |
| `format` | array of strings | curated | Tool ids that format the family. |
| `typecheck` | array of strings | curated | Tool ids that typecheck the family. |
| `audit` | array of strings | curated | Tool ids that audit the family. One group for the `dx security` and `dx license` tools; the policy lives in `security.toml` and `license.toml`. |
| `disabled` | boolean | `false` | Turns the whole family off. |

A list replaces the curated tools. A list whose entries all start with `+` or `-`
edits the curated tools instead, so later additions to the curated set still
reach you: `lint = ["+yamllint", "-typos"]`. Mixing the two forms fails the run.
An unknown tool id fails the run.

### dx.toml `[scope]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `exclude` | array of strings | `[]` | Bazel label patterns, such as `//third_party/...`, that dx leaves out. |

### dx.toml `[large_files]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `allow` | array of strings | `[]` | Paths or `dir/` prefixes the large-file check skips. Lock files are always skipped. |

### dx.toml `[check]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `commits` | string | off | `"conventional"` makes `dx check --affected` warn about each commit subject since the merge base that is not a conventional commit. See [Check](check.md). |

## security.toml

Used by `dx security`. See [Audit, Update](audit-update.md).

The base comes from the `dx.toml` `tier`.

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

## license.toml

Used by `dx license`. See [Audit, Update](audit-update.md).

The base comes from the `dx.toml` `tier`.

A list this file sets replaces the tier's list; a list it leaves out comes
from the tier. The strict tier denies what standard only reviews.

### license.toml top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer | none | Must be `1`. |
| `policy` | table | empty | License tiers. |
| `distribution` | table | empty | Which scopes are internal. |
| `exception` | array of tables | none | Accepted license findings. |
| `inventory` | array of tables | none | Licenses declared by hand. |

### license.toml `[policy]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `blocked` | array of strings | `[]` | License ids that fail every scope. |
| `distributed` | table | empty | Tiers for distributed scopes. |
| `sets` | table of tables | empty | Per-set tiers, keyed by set name. |

### license.toml `[policy.distributed]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `allow` | array of strings | `[]` | License ids that pass. |
| `review` | array of strings | `[]` | License ids that need review. |
| `deny` | array of strings | `[]` | License ids that fail. |

### license.toml `[policy.sets.<set>]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `review` | array of strings | `[]` | Extra ids that need review in this set. |

### license.toml `[distribution]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `internal` | array of strings | `[]` | Label patterns for internal scopes. |

### license.toml `[[exception]]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `package` | string | none | Package the finding names. |
| `set` | string | none | Dependency set that owns the package. |
| `license` | string | none | Exact license string of the finding. |
| `versions` | string | none | Affected version range. |
| `reason` | string | none | Why the license is accepted. |
| `expires` | string | none | Date the exception ends. |

### license.toml `[[inventory]]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `package` | string | none | Package the entry covers. |
| `set` | string | none | Dependency set that owns the package. |
| `license` | string | none | License id of the package. |
| `versions` | string | none | Version range the entry covers. |
| `text_present` | boolean | `false` | The license text ships with the package. |
| `text` | string | none | Workspace-relative path of a vendored license file. Implies `text_present`. |
