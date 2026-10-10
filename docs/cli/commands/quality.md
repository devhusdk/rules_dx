# Quality Commands

```sh
bazel run @rules_dx//:dx -- lint //...
bazel run @rules_dx//:dx -- typecheck //...
bazel run @rules_dx//:dx -- format //...
```

No scope means `//...`. Use `--here` for the current dir tree. All three
check files by default and report drift without writing. Pass `--apply` to
write validated fixes. Args after `--` go to Bazel unchanged.

```text
dx lint [--here] [--check] [--apply] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
dx typecheck [--here] [--check] [--apply] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
dx format [--here] [--check] [--apply] [--fail-on info|warning|error] [scope...]
```

- `--check`: report findings without writing files. Same as the default.
- `--apply`: write validated fixes and refresh the configured baseline.
  Without it files stay untouched.
- `--fail-on info|warning|error`: severity that fails. Default `warning`.
- `--report sarif=<dest>`: write a SARIF report for `lint` and `typecheck`.
  Repeatable. `dx format` has no report format. A relative destination anchors
  at the workspace root; an absolute one writes as given. Colliding destinations
  fail before anything runs, parents must exist, and a failed write keeps the
  previous file bytes.
- `--output text|diff|json`: result shape.

Exit codes: `0` success, `2` usage or scope errors, `1` findings at or above
`--fail-on`, an incomplete result set, a failed report write, or a missing,
malformed, or stale baseline. Bazel failures report `1`, not Bazel's code.

```sh
bazel run @rules_dx//:dx -- lint --check //...
bazel run @rules_dx//:dx -- format --here
bazel run @rules_dx//:dx -- typecheck --fail-on error //cli/...
bazel run @rules_dx//:dx -- format --apply //cli/...
```

## Dry run

Pass `--dry-run` to print the plan without running it.

```sh
bazel run @rules_dx//:dx -- lint --dry-run //...
bazel run @rules_dx//:dx -- lint --dry-run --output=json //...
```

Text prints the scope summary, the policy origin, the selected aspect count,
and the unknown execution facts. JSON emits an `operation` event with phase
`plan` and a `provenance` object. The object names the command, mode, scope,
policy origin, aspects, settings, reports, and the redacted Bazel inputs.
Execution platform and toolchain read `unknown` until Bazel analyzes them.
Validation is not performed.

## Policy

The three commands check with the workspace policy that
`--@rules_dx//config:workspace` names. Without that flag the policy is
`@rules_dx//quality:default_workspace_policy`. Pass another target after `--`
to select your own.

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:workspace=//quality:my_policy
bazel run @rules_dx//:dx -- format --check //... -- --@rules_dx//config:workspace=//quality:my_policy
```

A policy is a `workspace_policy` of `quality_family` sections, loaded from
`@rules_dx//quality:policy.bzl`. Every family states each capability it uses:
a tool ID, or the capability in `disabled`.

```python
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "toml_family",
    family_id = "toml",
    disabled = ["typecheck"],
    format = ["taplo"],
    lint = ["taplo"],
)

workspace_policy(
    name = "my_policy",
    families = [":toml_family"],
)
```

- A family that leaves a capability neither selected nor disabled fails the
  analysis with the capability named.
- A family that selects a tool with no wired executable fails with the wired
  tools listed for that capability.
- `disabled = ["format"]` runs no formatter for that family.

## Tool overrides

The three commands run every wired tool from its managed executable. Pass
another target after `--` to select your own build of one tool. The override
keeps the tool ID, so findings read the same.

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:tool_ruff=//tools:my_ruff
```

- `ruff` is the tool with an override flag today.
- The alternate must be a single executable file. Anything else fails the
  analysis with the tool named.
- An override that prints another diagnostic shape fails the run with the
  tool named.

## Baselines

Opt in to a baseline to keep known findings while new ones fail. Set the
file in `dx.toml`:

```toml
[quality]
baseline = "quality/baseline.json"
```

The path stays inside the workspace and names a `.json` file. Commit the
file with the selection. A missing file fails checks. Pass `--apply` once to
write it.

The file lists known diagnostics with tool, rule, path, message, context,
and count:

```json
{
  "schema_version": 1,
  "entries": [
    {
      "tool": "ruff",
      "rule": "F401",
      "path": "src/app.py",
      "message": "unused import",
      "context": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "count": 1
    }
  ]
}
```

Identity is tool, rule, path, message, and source context. Line numbers
alone never match, so moving a line keeps its entry. A rename or a rule
change needs a refresh instead.

Checks print `Baseline <file>: <total> total, <new> new, <suppressed>
suppressed.` Suppressed findings print with `(baselined)` and stay in JSON
and SARIF output. JSON marks them with `"baseline": "suppressed"` and
carries the counts in `command_finished`. SARIF carries the counts in each
run. New findings fail as usual. Fixed entries turn stale and fail. Entries
outside the checked scope stay unevaluated. An incomplete run never goes
green through a baseline.

Pass `--apply` to refresh the file. Refresh adds new findings, prunes fixed
entries in scope, and keeps the rest. A malformed file fails instead of
being rewritten.

Adopt in small steps. Run the check, review the findings, record them with
`--apply`, then keep the file current on later runs.
