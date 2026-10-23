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
- `--include-edits`: add `source_digest` and full `edits` to JSON `change`
  events. Needs `--output json`; otherwise exits `2`.

Text prints one line per finding:
`path:line:column: severity: message [tool/rule]`. Lines and columns count
Unicode characters in the checked file. A finding without a verified position
prints `path:`; a finding without a file prints `(no file):`. Identical
findings print once with `(xN)`. `--quiet` never hides findings or failures.

When a check finds changes, text prints
`N file(s) need changes. Apply them with: dx <command> --apply ...` with your
workspace, scope, and Bazel options quoted for the shell. Credential-bearing
options are left out. Failures print a `next:` line with the fix and a check
command to run again.

JSON prints one compact `change` per file with `path`, `kind`, `edit_count`,
and `effect` (`proposed`, `applied`, or `refused` with a `reason`). Identical
findings from several targets print once, with a `count` and every target in
`origins`.

When several targets fix the same file, identical fixes apply once. Fixes that
disagree leave the file unchanged, name the targets on stderr, and report
`not_applied` with reason `conflicting_candidates` or `conflicting_digests`.
Other files still apply.

The result set is incomplete when a target fails, a result is invalid, Bazel
announces a target or aspect it never reports, an output group is marked
incomplete, or the build events end before Bazel's last event. Targets Bazel
skips as incompatible are not missing. An incomplete run prints
`dx: incomplete_results: <reason>` on stderr, or an `incomplete_results`
error event in JSON, and never applies fixes.

A target whose fixes oscillate or hit the ten-round limit did not converge.
The run fails even without findings and prints
`dx: not_converged: <target> in configuration <id> did not converge: <cause> after <n> round(s) of <tools>`
on stderr, or a `not_converged` error event in JSON. The cause is
`OSCILLATION` or `ITERATION_LIMIT`. SARIF marks that tool's run
`executionSuccessful: false` with the same message. Such a target proposes no
fixes, and `--apply` does not refresh the baseline. Fixes from other targets
still apply.

Exit codes: `0` success, `2` usage or scope errors, `1` findings at or above
`--fail-on`, an incomplete result set, a target that did not converge, a
conflicting fix, a failed report write, or a missing, malformed, or stale
baseline. Bazel failures report `1`, not Bazel's code.

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

- `ruff`, `qmlformat` and `qmllint` have override flags.
- The `ruff` alternate must be a single executable file. Anything else fails
  the analysis with the tool named.
- The `qmlformat` and `qmllint` alternates must be executable targets, such as
  `@dx_qt//:qmlformat` and `@dx_qt//:qmllint`. A target without an executable
  fails the analysis.
- An override that prints another diagnostic shape fails the run with the
  tool named.

### QML formatting

`qmlformat` has no default executable. Enable the managed Qt SDK from
`qt/README.md`, select `qmlformat` in a `qml` family, and point the flag at
the SDK tool.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_qmlformat=@dx_qt//:qmlformat
bazel run @rules_dx//:dx -- format --apply -- --@rules_dx//config:tool_qmlformat=@dx_qt//:qmlformat
```

Without the flag, a policy that selects `qmlformat` fails the analysis and
names the flag. To set formatting options, bind a `qmlformat_config` to the
QML targets with `aspect_hints`. The file must be named `.qmlformat.ini`. dx
passes it to `qmlformat` with `-s`.

### QML linting

`qmllint` has no default executable. Enable the managed Qt SDK from
`qt/README.md`, select `qmllint` in a `qml` family, and point the flag at the
SDK tool.

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:tool_qmllint=@dx_qt//:qmllint
```

Without the flag, a policy that selects `qmllint` fails the analysis and names
the flag. Findings keep the line and column `qmllint` reports. Info, warning
and critical messages show as info, warning and error. A file that `qmllint`
does not report fails the run.

Import context comes from the `qmldir` files in the QML target and all its
transitive `deps`, including external repositories. dx stages them and passes
their module roots with `-I`. A `qmldir` whose directory does not end in its
module URI is not importable and is skipped. `QML_IMPORT_PATH` is not used. To
set lint options, bind a config ending in `.qmllint.ini` to the QML targets
with `aspect_hints`. dx copies it next to each linted file as `.qmllint.ini`.
Without one, dx runs `qmllint` with `--ignore-settings`.

### Scala linting

`scalafix` runs from the managed pin. It is not in the default Scala lint set.
Select it in a `scala` family.

```python
quality_family(
    name = "scala_family",
    family_id = "scala",
    disabled = ["audit", "format", "typecheck"],
    lint = ["scalafix"],
)
```

Register the SemanticDB toolchain in `MODULE.bazel`:

```python
register_toolchains("@rules_dx//scala/toolchain:semanticdb")
```

Without it, the analysis fails and names this line. Bind a `scalafix_config`
to every Scala target with `aspect_hints`. The file is your `.scalafix.conf`.
A Scala target without one fails the analysis.

```sh
bazel run @rules_dx//:dx -- lint //app/...
bazel run @rules_dx//:dx -- lint --apply //app/...
```

dx gives `scalafix` the compiled SemanticDB of the target and its
dependencies, so semantic rules such as `ExplicitResultTypes` work. Lint
messages keep the rule ID, line and column. A rule that rewrites code shows as
a `Patch` error at the first changed line, and `--apply` writes the rewrite. A
run that exits with a parse error, missing SemanticDB, an unknown rule or an
empty rule list fails with the cause named. It never counts as clean.

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
outside the checked tool and file scope stay unevaluated. An incomplete run never goes
green through a baseline.

Pass `--apply` to refresh the file. Refresh adds new findings, prunes fixed
entries in scope, and keeps the rest. An entry is in scope only for the
tool and file that run analyzed. Refresh never writes after an incomplete
run, a result that did not converge, or a finding whose source file cannot
be read; the file stays byte for byte as it was. A malformed file fails
instead of being rewritten.

Adopt in small steps. Run the check, review the findings, record them with
`--apply`, then keep the file current on later runs.
