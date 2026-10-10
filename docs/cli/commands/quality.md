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
- `--apply`: write validated fixes. Without it files stay untouched.
- `--fail-on info|warning|error`: severity that fails. Default `warning`.
- `--report sarif=<dest>`: write a SARIF report for `lint` and `typecheck`.
  Repeatable. `dx format` has no report format. A relative destination anchors
  at the workspace root; an absolute one writes as given. Colliding destinations
  fail before anything runs, parents must exist, and a failed write keeps the
  previous file bytes.
- `--output text|diff|json`: result shape.

Exit codes: `0` success, `2` usage or scope errors, `1` findings at or above
`--fail-on`, an incomplete result set, or a failed report write. Bazel
failures report `1`, not Bazel's code.

```sh
bazel run @rules_dx//:dx -- lint --check //...
bazel run @rules_dx//:dx -- format --here
bazel run @rules_dx//:dx -- typecheck --fail-on error //cli/...
bazel run @rules_dx//:dx -- format --apply //cli/...
```

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

Set `baseline` in `dx.toml` to a workspace-relative JSON file to adopt
incrementally. Listed findings still print, marked `(suppressed)`, and new
findings still fail. Counts print in text, ride a `baseline` notice event in
JSON, and mark suppressed results in SARIF.

```toml
[dx]
baseline = "quality-baseline.json"
```

```sh
bazel run @rules_dx//:dx -- lint --check //...
bazel run @rules_dx//:dx -- lint --apply //...
```

- A finding matches by tool, rule, file, message, and source line. Moved
  lines still match. Renames and tool or rule changes need a refresh.
- A missing baseline file reads as empty. A malformed file fails the run
  with `baseline_invalid`. Findings without a file never suppress.
- A listed finding that no longer appears fails the run with
  `baseline_stale`, but only when the analysis ran complete: entries for
  files outside the run scope are left alone, and incomplete results never
  prove staleness.
- Ordinary checks never write the baseline file. Pass `--apply` to refresh
  it: stale entries in the analyzed scope are pruned and new findings are
  added. Other entries are preserved.
