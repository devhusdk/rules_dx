# Config Reference

Use this page to see which config file each tool reads and to set up `dx.toml`, `security.toml` and `license.toml`.

All three files live in the workspace root, the directory that holds `MODULE.bazel`. All three are optional. An unknown key fails the run. Dates are `YYYY-MM-DD`. In an `[[exception]]`, `[[uncovered]]` or `[[inventory]]` entry, every key whose default is `none` is required.

## `dx config`

Lists the config file each tool reads and the base dx writes for it under `.dx/config/`.

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

`<tool>` is a tool id, such as `ruff` or `hlint`. A tool's root file is its config file in the workspace root, such as `ruff.toml`. The base is the file dx renders for it, such as `.dx/config/ruff/base.toml`.

### Forms

| Form | What it does |
| --- | --- |
| `dx config` | Prints one line per tool your workspace uses. |
| `dx config <tool>` | Prints every file the tool reads, in order: the root file when there is one, then the base. Works for any tool with a base. |
| `dx config diff <tool>` | Prints a unified diff from the base to the root file. The `dx: override of base` header line is ignored. Prints nothing when the copy is unchanged. |
| `dx config override <tool>` | Copies the base to the tool's root file and adds a `dx: override of base <sha>` comment on top. `<sha>` is the git blob sha of the base. This is the only form that writes. |

`dx config override <tool>` refuses when:

- the tool can extend its base. dx prints the line to add to your root file instead.
- the root file already exists. Remove it first.

### Output

`dx config` prints one line per tool:

```text
ruff  ruff.toml  extends .dx/config/ruff/base.toml  edited
taplo  -  tier base .dx/config/taplo/base.toml
```

| Column | Meaning |
| --- | --- |
| Tool | The tool id. |
| Root file | The root file, or `-` when the workspace has none. |
| Relation | `extends <base>`: the root file extends the base. `override of <base>`: the root file replaces the base. `tier base <base>`: the tool reads the base itself. |
| `edited` | Shown when the root file starts with a `dx: override of base <sha>` comment. |

With `--output json`, dx prints `command_started`, one `config_file` event per file, then `command_finished`. Each `config_file` event has:

| Field | Meaning |
| --- | --- |
| `tool` | The tool id. |
| `file` | The file path. |
| `role` | `root` or `base`. |
| `base` | The relation, such as `extends .dx/config/ruff/base.toml`, or `tier base`. |
| `edited` | `true` when the root file has the override comment. |
| `patch` | `dx config diff` only: the unified diff as one string, empty when unchanged. |

- `dx config` emits each tool's root file, or its base when the tool has no root file.
- `dx config <tool>` emits every file the tool reads.
- `dx config diff <tool>` emits the root file with `patch`.
- `dx config override <tool>` emits the root file it wrote.

A failure emits one `error` with code `config_failed`.

Reports: none.

### Flags

| Flag | What it does |
| --- | --- |
| `-o, --output text\|json` | Choose the output format. Default `text`. |
| `--workspace <dir>` | Run in this workspace. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots auto\|<n>\|off` | Set build slots. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

Scopes: none. `dx config` takes a tool name, not scopes.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | A refused `override`, a missing base file, or a `dx.toml` dx cannot load. |
| `2` | Usage error: an unknown tool, `diff` on a tool with no root file, a missing tool name, or an extra word. |
| `3` | dx could not run git or read a file. |

## dx.toml

Picks the tier, the tools of each language family, and what dx leaves out.

With no `dx.toml`, every family found in the tree runs its curated tools at the `standard` tier. dx writes the `.dx/` directory from this file. Never edit `.dx/` by hand.

Start the file with the `#:schema` line below. An editor with a TOML schema plugin then completes every family and tool id. The schema is [`dx.schema.json`](https://github.com/devhusdk/rules_dx/blob/main/cli/dx_config/schema/dx.schema.json).

```toml
#:schema https://raw.githubusercontent.com/devhusdk/rules_dx/main/cli/dx_config/schema/dx.schema.json
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
| `tier` | string | `"standard"` | `standard` or `strict`. The tier for every family that does not set its own, and the base for `security.toml` and `license.toml`. |
| `skip_tools` | array of strings | `[]` | Tool ids dropped from every family, such as `["typos"]`. An unknown id fails the run. |
| `families` | table | empty | Per-family settings, keyed by family id. An unknown id fails the run. |
| `scope` | table | empty | What dx leaves out. |
| `large_files` | table | empty | Files the large-file check skips. |
| `check` | table | empty | Extra checks for `dx check`. |
| `affected` | table | empty | Settings for `--affected`. See below. |

### dx.toml `[families.<id>]`

Replaces the curated tools of one family. A capability you leave out keeps the curated tools. The curated tools are listed in [Default tools](quality.md#default-tools).

| Key | Type | Default | Meaning |
|---|---|---|---|
| `tier` | string | top-level `tier` | `standard` or `strict` for this family. |
| `lint` | array of strings | curated | Tool ids that lint the family. |
| `format` | array of strings | curated | Tool ids that format the family. |
| `typecheck` | array of strings | curated | Tool ids that typecheck the family. |
| `audit` | array of strings | curated | Tool ids that audit the family, for `dx security` and `dx license`. Their policy lives in `security.toml` and `license.toml`. |
| `disabled` | boolean | `false` | Turns the whole family off. |

A plain list replaces the curated tools: `lint = ["clippy"]`. A list whose entries all start with `+` or `-` edits the curated tools instead, so tools added to the curated set later still reach you: `lint = ["+yamllint", "-typos"]`. Mixing the two forms fails the run. An unknown tool id fails the run.

### dx.toml `[scope]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `exclude` | array of strings | `[]` | Bazel label patterns dx leaves out. A target is one thing Bazel builds, and a label names it, such as `//app:server`. `//third_party/...` means every target under `third_party/`. |

### dx.toml `[large_files]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `allow` | array of strings | `[]` | Paths or `dir/` prefixes the large-file check skips. Lock files are always skipped. |

### dx.toml `[check]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `commits` | string | off | `"conventional"` makes `dx check --affected` report each commit subject since the merge base that is not a conventional commit. See [Check](check.md). |

### dx.toml `[affected]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `base` | string | detected | The branch, such as `"origin/develop"`, whose merge base with `HEAD` `--affected` diffs from. Left out, dx uses `origin/HEAD`, else `main` or `master`. A base given on the command line overrides it, see [Scope defaults](scope-defaults.md#--affected). |

## security.toml

Sets the policy for `dx security`. dx merges this file over a base picked by the `tier` in `dx.toml`. The commands are in [Audit, Update](audit-update.md).

`versions` uses the version range syntax of the dependency set's ecosystem.

### security.toml top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer | none | Must be `1`. |
| `exception` | array of tables | none | Accepted advisories. |
| `uncovered` | array of tables | none | Accepted dependency sets with no advisory source. |
| `fail_on` | array of strings | the tier's | Advisory kinds that fail the run: `vulnerability`, `unsound`, `unmaintained`, `notice`, `yanked`. Other kinds are reported as `info`. `standard` fails on `vulnerability` and `unsound`. `strict` adds `unmaintained`, `notice` and `yanked`. A list set here replaces the tier's. |

### security.toml `[[exception]]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `advisory` | string | none | Advisory id, such as `GHSA-aaaa-bbbb-cccc`. |
| `package` | string | none | Package the advisory hits. |
| `set` | string | none | Dependency set that owns the package. |
| `versions` | string | none | Affected version range. |
| `reason` | string | none | Why you accept the risk. |
| `expires` | string | none | Date the exception ends. |

Three `set` values are not dependency sets:

- `set = "oci"` covers a finding from the `oci_deploy` image scan. `package` is the image package. `versions` is `*` or a range that holds the installed version.
- `set = "trivy_config"` covers a `trivy_config` lint finding. `advisory` is the trivy ID, such as `DS-0002`. `package` is the workspace-relative file, or `*` for every file. `versions` must be `*`. `dx lint` drops covered findings until `expires`.
- `set = "trivy_image_config"` covers a `trivy_image_config` lint finding. `package` is the workspace-relative `BUILD.bazel` of the image, or `*`. The other fields work as for `trivy_config`.

### security.toml `[[uncovered]]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `set` | string | none | Dependency set with no advisory source. |
| `reason` | string | none | Why you accept the gap. |
| `expires` | string | none | Date the acceptance ends. |

## license.toml

Sets the policy for `dx license`. dx merges this file over a base picked by the `tier` in `dx.toml`. The commands are in [Audit, Update](audit-update.md).

A list you set here replaces the tier's list. A list you leave out comes from the tier. The `strict` tier denies what `standard` only reviews.

### license.toml top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer | none | Must be `1`. |
| `policy` | table | empty | License tiers. |
| `distribution` | table | empty | Which scopes are internal. |
| `exception` | array of tables | none | Accepted license findings. |
| `inventory` | array of tables | none | Licenses you declare by hand. |

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
| `reason` | string | none | Why you accept the license. |
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
