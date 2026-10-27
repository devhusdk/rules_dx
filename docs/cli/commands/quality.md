# Quality Commands

```sh
bazel run @rules_dx//:dx -- lint //...
bazel run @rules_dx//:dx -- typecheck //...
bazel run @rules_dx//:dx -- format //...
```

No scope means `//...`. Use `--here` for the current dir tree, or `--affected` for the targets your
git changes reach (see [Scope Defaults](scope-defaults.md)). With nothing
affected they print `no affected targets` and exit `0`. All three
check files by default and report drift without writing. Pass `--apply` to
write validated fixes. Args after `--` go to Bazel, and dx always adds
`--keep_going` and `--remote_download_outputs=toplevel` after them. Pass
`--remote_download_outputs=all` or `--remote_download_all` to download all outputs instead;
the last of these wins. All three also pass
`--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
dx spells them with the name your MODULE.bazel gives rules_dx, for example
`--@my_dx//config:clippy_output_diagnostics=true` when `repo_name = "my_dx"`.
These args fail the same way: `--aspects`, `--output_groups`,
`--build_event_json_file`, any `--build_event_json_file_path_conversion` other
than `false`, `--nokeep_going`, any `--keep_going` other than bare
`--keep_going`, `--remote_download_minimal`, any `--remote_download_outputs`
other than `toplevel` or `all`, `--test_arg`, and the startup
options `--bazelrc`, `--home_rc`, `--nohome_rc`, `--system_rc`, `--nosystem_rc`,
`--output_base`, `--output_user_root`, `--host_jvm_args` and `--server_jvm_out`.
Pass `--output_base` or `--output_user_root` with
[`--bazel-startup-option`](README.md#global-flags) instead.
Before the build, dx runs a cheap probe with the same flags. It names the
language checks each target in scope needs, and the build then applies only
those. A Rust-only change does not configure or fetch the C++, .NET, JVM, Node,
PowerShell or Ruby tools. If the probe fails for any reason, dx applies every
check, as it would without the probe.
With `--@rules_rust//rust/settings:pipelined_compilation=true` in your `.bazelrc`,
`dx typecheck` reads library diagnostics from the metadata build and skips their codegen.

```text
dx lint [--here] [--affected] [--check] [--apply] [--fail-on info|warning|error] [--output text|diff|json] [--include-edits] [--report sarif=<dest>] [scope...] [-- bazel-options ...]
dx typecheck [--here] [--affected] [--check] [--apply] [--fail-on info|warning|error] [--output text|diff|json] [--include-edits] [--report sarif=<dest>] [scope...] [-- bazel-options ...]
dx format [--here] [--affected] [--check] [--apply] [--fail-on info|warning|error] [--output text|diff|json] [--include-edits] [scope...] [-- bazel-options ...]
```

- `--check`: report findings without writing files. Same as the default.
- `--apply`: write validated fixes and refresh the configured baseline.
  Without it files stay untouched.
  After `--apply`, findings point at files as written. Unwritten files keep
  their checked positions.
- `--fail-on info|warning|error`: severity that fails. Default `warning`.
- `--report sarif=<dest>`: write a SARIF report for `lint` and `typecheck`.
  Repeatable. `dx format` has no report format. A relative destination anchors
  at the workspace root; an absolute one writes as given. Colliding destinations
  fail before anything runs, parents must exist, and a failed write keeps the
  previous file bytes. Use `-` for stdout. Only one stdout report is allowed,
  and only with text output: `--output json` or `--output diff` exits `2`. With
  a stdout report, text findings and summaries go to stderr.
  SARIF results carry `partialFingerprints` (`dx/v1`), `ruleIndex`, and `fixes` for machine-applicable replacements.
  Runs set `columnKind` to `unicodeCodePoints`.
- `dx lint` without `--apply` also fails with exit `1` when the checkout holds
  build output or a file over 1 MiB, as described for `dx check`.
- `--output text|diff|json`: result shape.
- `--include-edits`: add `source_digest` and full `edits` to JSON `change`
  events. Needs `--output json`; otherwise exits `2`.

Text prints one line per finding:
`path:line:column: severity: message [tool/rule]`. Lines and columns count
Unicode characters in the checked file. A finding without a verified position
prints `path:`; a finding without a file prints `(no file):`. Identical
findings print once with `(xN)`. `--quiet` never hides findings or failures.

`--output diff` prints a unified patch of the proposed fixes on stdout and no
finding lines. `Not applied:`, `Conflict:` and `Baseline stale:` lines go to
stderr.

When a check finds changes, text prints
`N file(s) need changes. Apply them with: dx <command> --apply ...` with your
workspace, scope, and Bazel options quoted for the shell. Credential-bearing
options are left out. Failures print a `next:` line with the fix and a check
command to run again. Inside `dx check` and `dx fix`, these lines name
`dx fix --apply` or the umbrella command with your scope instead.

JSON prints one compact `change` per file with `path`, `kind`, `edit_count`,
and `effect` (`proposed`, `applied`, or `refused` with a `reason`). The
`reason` is `stale_source`, `unreadable_source`, `invalid_edits`,
`write_failed`, `incomplete_collection`, `conflicting_digests` or
`conflicting_candidates`. A refused change fails the run with exit `1`, even
without findings; files that did apply keep their new bytes, and `--apply`
does not refresh the baseline. Identical
findings from several targets print once, with a `count` and every target in
`origins`.

When several targets fix the same file, identical fixes apply once. Fixes that
disagree leave the file unchanged and print a `Conflict:` line naming the
targets on stderr. JSON reports a `change` with effect `refused` and reason
`conflicting_candidates` or `conflicting_digests`.
Other files still apply.

The result set is incomplete when a target fails, a result is invalid, Bazel
announces a target or aspect it never reports, an output group is marked
incomplete, or the build events end before Bazel's last event. Targets Bazel
skips as incompatible are not missing. `lint`, `typecheck`, and `format` skip targets that
do not fit the target platform, even named or affected ones; see
[Scope Defaults](scope-defaults.md). An incomplete run prints
`dx: incomplete_results: <reason>` on stderr, or an `incomplete_results`
error event in JSON, and never applies fixes.

A failed Bazel build lists each failed action the way `dx build` does: a
`build_failure` event in JSON, or a `dx: build failure:` line on stderr. A Rust
compile error in a `Clippy` or `Rustc` action carries its `path`, `line`,
`column` and `message`; see [`dx build`](build-test-coverage.md). When every
failed action is a compile error, the run reports `bazel_failed` with
`<command> build failed with Bazel exit <n>; results are incomplete`, and text
output adds `next: fix the compile errors above, then run: ...`. Other failed
builds report `analysis_failed`. The results are incomplete either way.

A target whose fixes oscillate or hit the ten-round limit did not converge.
The run fails even without findings and prints
`dx: not_converged: <target> in configuration <id> did not converge: <cause> after <n> round(s) of <tools>`
on stderr, or a `not_converged` error event in JSON. The cause is
`OSCILLATION` or `ITERATION_LIMIT`. SARIF marks that tool's run
`executionSuccessful: false` with the same message. Such a target proposes no
fixes, and `--apply` does not refresh the baseline. Fixes from other targets
still apply.

Exit codes: `0` success, `2` usage or scope errors, `1` findings at or above
`--fail-on`, a check (no `--apply`) with any proposed change, whatever
`--fail-on` says, an incomplete result set, a target that did not converge, a
conflicting fix, a fix that was not applied, a failed report write, or a missing, malformed, or stale
baseline. Bazel failures report `1`, not Bazel's code.

```sh
bazel run @rules_dx//:dx -- lint --check //...
bazel run @rules_dx//:dx -- format --here
bazel run @rules_dx//:dx -- lint --affected
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

### Default policy

`default_workspace_policy` selects these tools. A capability marked
`disabled` runs no tool for that family.

| Family | Format | Lint | Typecheck |
| --- | --- | --- | --- |
| `astro` | `prettier` | disabled | disabled |
| `cc` | `clang_format` | disabled | disabled |
| `dockerfile` | disabled | `hadolint` | disabled |
| `go` | `gofumpt` | disabled | disabled |
| `graphql` | `prettier` | disabled | disabled |
| `html` | `prettier` | disabled | disabled |
| `java` | `google_java_format` | `checkstyle`, `pmd`, `spotbugs` | disabled |
| `javascript` | `biome` | `biome` | disabled |
| `json` | `prettier` | `biome` | disabled |
| `kotlin` | `ktfmt` | `ktlint` | disabled |
| `markdown` | disabled | `markdown_check`, `vale` | disabled |
| `mdx` | `remark` | disabled | disabled |
| `python` | `ruff` | `pydoclint`, `ruff` | `ty` |
| `rust` | `rustfmt` | `clippy` | `rustc` |
| `scala` | `scalafmt` | disabled | disabled |
| `shell` | `shfmt` | `shellcheck` | disabled |
| `starlark` | `buildifier` | `buildifier` | disabled |
| `text` | disabled | `keep_sorted` | disabled |
| `toml` | `taplo` | `taplo` | disabled |
| `typescript` | `biome` | `biome` | disabled |
| `vue` | `prettier` | disabled | disabled |

### Wired tools

Every wired tool, with the verbs it serves and the families its classes belong
to.

| Tool ID | Verbs | Families |
| --- | --- | --- |
| `biome` | format, lint | javascript, json, typescript |
| `buf` | format, lint | protobuf |
| `buildifier` | format, lint | starlark |
| `checkstyle` | lint | java |
| `clang_format` | format | cc, cuda, objc |
| `clang_tidy` | lint | cc |
| `clippy` | lint | rust |
| `cppcheck` | lint | cc |
| `csharpier` | format | csharp |
| `cue` | format | cue |
| `djlint` | format, lint | html_template |
| `erlfmt` | format | erlang |
| `errcheck` | lint | go |
| `eslint` | lint | astro, javascript, mdx, vue |
| `fantomas` | format | fsharp |
| `flake8` | lint | python |
| `fsharplint` | lint | fsharp |
| `gleam_format` | format | gleam |
| `gofumpt` | format | go |
| `google_java_format` | format | java |
| `govet` | lint | go |
| `graphql_eslint` | lint | graphql |
| `hadolint` | lint | dockerfile |
| `jsonnetfmt` | format | jsonnet |
| `keep_sorted` | lint | text |
| `ktfmt` | format | kotlin |
| `ktlint` | lint | kotlin |
| `markdown_check` | lint | markdown |
| `mix_format` | format | elixir |
| `modfmt` | format | go_module |
| `pkl` | format | pkl |
| `pmd` | lint | java |
| `prettier` | format | astro, css, gherkin, graphql, html, javascript, json, sql, typescript, vue, xml |
| `psscriptanalyzer` | lint | powershell |
| `pydoclint` | lint | python |
| `pylint` | lint | python |
| `qmlformat` | format | qml |
| `qmllint` | lint | qml |
| `redocly` | lint | api_spec |
| `remark` | format | mdx |
| `roslyn` | lint | csharp |
| `rubocop` | lint | ruby |
| `ruff` | audit, format, lint | python |
| `rustc` | typecheck | rust |
| `rustfmt` | format | rust |
| `scalafix` | lint | scala |
| `scalafmt` | format | scala |
| `shellcheck` | lint | shell |
| `shfmt` | format | shell |
| `spotbugs` | lint | java |
| `standardrb` | format | ruby |
| `staticcheck` | lint | go |
| `stylelint` | lint | css |
| `taplo` | format, lint | toml |
| `terraform` | format | terraform |
| `ty` | typecheck | python |
| `vale` | lint | markdown |
| `yamlfmt` | format | yaml |
| `yamllint` | lint | yaml |
| `zig_fmt` | format | zig |

Fortran (`fortran`) has no formatter and no linter. Fortran targets are tagged `no-format`, `no-lint` and `no-typecheck`, so `dx format`, `dx lint` and `dx typecheck` skip them.

OCaml (`ocaml`) has no formatter and no linter. OCaml targets are tagged `no-format`, `no-lint` and `no-typecheck`, so `dx format`, `dx lint` and `dx typecheck` skip them.

## Tool overrides

The three commands run every wired tool from its managed executable. Pass
another target after `--` to select your own build of one tool. The override
keeps the tool ID, so findings read the same.

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:tool_ruff=//tools:my_ruff
```

- `ruff`, `terraform`, `hadolint`, `gofumpt`, `qmlformat`, `qmllint`,
  `zig_fmt`, `erlfmt`, `mix_format` and `gleam_format` have override flags.
- The `ruff`, `terraform` and `hadolint` alternates must be a single
  executable file.
  Anything else fails the analysis with the tool named.
- The `gofumpt`, `qmlformat`, `qmllint`, `zig_fmt`, `erlfmt`, `mix_format` and
  `gleam_format` alternates must be executable targets, such as
  `@dx_qt//:qmlformat`, `@dx_qt//:qmllint`, `@rules_dx//zig/tools:zig`,
  `@rules_dx//erlang/tools:erlfmt`, `@rules_dx//elixir/tools:mix_format` and
  `@rules_dx//gleam/tools:gleam_format`. A target without an executable fails
  the analysis.
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

### Zig formatting

`zig_fmt` uses the pinned Zig compiler that `zig_library`, `zig_binary` and
`zig_test` carry. Select `zig_fmt` in a `zig` family. No flag is needed.

```python
quality_family(
    name = "zig_family",
    family_id = "zig",
    disabled = ["audit", "lint", "typecheck"],
    format = ["zig_fmt"],
)
```

```sh
bazel run @rules_dx//:dx -- format
bazel run @rules_dx//:dx -- format --apply
```

To use your own compiler, set the flag:

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_zig_fmt=@rules_dx//zig/tools:zig
```

A Zig-class target that is not one of those three wrappers fails the analysis
and names the flag. The check runs `zig fmt --check` and never writes.
`--apply` runs `zig fmt`. A file with a syntax error reports an error at its
line and column and stays untouched. Use the wrappers from
`@rules_dx//zig/rules:defs.bzl`.

### Erlang formatting

`erlfmt` has no default executable. Select `erlfmt` in an `erlang` family and
point the flag at the pinned erlfmt 1.8.0 escript.

```python
quality_family(
    name = "erlang_family",
    family_id = "erlang",
    disabled = ["audit", "lint", "typecheck"],
    format = ["erlfmt"],
)
```

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/tools:erlfmt
bazel run @rules_dx//:dx -- format --apply -- --@rules_dx//config:tool_erlfmt=@rules_dx//erlang/tools:erlfmt
```

Without the flag, a policy that selects `erlfmt` fails the analysis and names
the flag. The check runs `erlfmt --check` and never writes. `--apply` runs
`erlfmt --write`. A file with a syntax error reports an error at its line and
column and stays untouched. A `%% erlfmt-ignore` comment keeps the next form
as written. Erlang sources come from the rules in
`@rules_dx//erlang/rules:defs.bzl`.

### Elixir formatting

`mix_format` has no default executable. Select `mix_format` in an `elixir`
family and point the flag at the pinned Elixir 1.20.4 formatter.

```python
quality_family(
    name = "elixir_family",
    family_id = "elixir",
    disabled = ["audit", "lint", "typecheck"],
    format = ["mix_format"],
)
```

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_mix_format=@rules_dx//elixir/tools:mix_format
bazel run @rules_dx//:dx -- format --apply -- --@rules_dx//config:tool_mix_format=@rules_dx//elixir/tools:mix_format
```

Without the flag, a policy that selects `mix_format` fails the analysis and
names the flag. The check runs `mix format --check-formatted` with the default
formatter options and never writes. `--apply` runs `mix format`. A file with a
syntax error reports an error at its line and column and stays untouched. The
formatter reads no `.formatter.exs`, Mix project or Hex home. Elixir sources
come from the rules in `@rules_dx//elixir/rules:defs.bzl`.

### Gleam formatting

`gleam_format` has no default executable. Select `gleam_format` in a `gleam`
family and point the flag at the pinned Gleam 1.19.2 formatter.

```python
quality_family(
    name = "gleam_family",
    family_id = "gleam",
    disabled = ["audit", "lint", "typecheck"],
    format = ["gleam_format"],
)
```

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_gleam_format=@rules_dx//gleam/tools:gleam_format
bazel run @rules_dx//:dx -- format --apply -- --@rules_dx//config:tool_gleam_format=@rules_dx//gleam/tools:gleam_format
```

Without the flag, a policy that selects `gleam_format` fails the analysis and
names the flag. The check runs `gleam format --check` and never writes.
`--apply` runs `gleam format`. A file with a syntax error reports an error at
its line and column and stays untouched. The formatter reads no `gleam.toml`
or Hex cache. Gleam sources come from the rules in
`@rules_dx//gleam/rules:defs.bzl`.

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

### Java lint

Java lint runs `checkstyle`, `pmd` and `spotbugs`.

`checkstyle` has no default ruleset. Bind a `checkstyle_config` to each Java
target, or lint fails. `pmd` uses `rulesets/java/quickstart.xml` unless a
`pmd_config` is bound. For both, `src` must be a checked-in `.xml` file and
`data` lists any extra files it reads. A target takes at most one config per
tool.

```starlark
load("@rules_java//java:defs.bzl", "java_library")
load("@rules_dx//quality:native_config.bzl", "checkstyle_config", "pmd_config")

checkstyle_config(
    name = "checkstyle_cfg",
    src = "checkstyle.xml",
)

pmd_config(
    name = "pmd_cfg",
    src = "pmd-ruleset.xml",
)

java_library(
    name = "lib",
    srcs = ["Lib.java"],
    aspect_hints = [
        ":checkstyle_cfg",
        ":pmd_cfg",
    ],
)
```

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

That toolchain is a plain `scala_toolchain`. If you already register your own,
keep it and set these two attributes on it instead:

```python
scala_toolchain(
    name = "my_scala_toolchain",
    enable_semanticdb = True,
    semanticdb_bundle_in_jar = True,
)
```

Without one of the two, the analysis fails and names the fix. Bind a `scalafix_config`
to every Scala target with `aspect_hints`. The file is your `.scalafix.conf`.
A Scala target without one fails the analysis.

```sh
bazel run @rules_dx//:dx -- lint //app/...
bazel run @rules_dx//:dx -- lint --apply //app/...
```

dx gives `scalafix` the compiled SemanticDB of the target and its
dependencies, so semantic rules such as `ExplicitResultTypes` work. This
includes compile-only (`neverlink`) dependencies and test targets. Lint
messages keep the rule ID, line and column. A rule that rewrites code shows as
a `Patch` error at the first changed line, and `--apply` writes the rewrite. A
run that exits with a parse error, missing SemanticDB, an unknown rule or an
empty rule list fails with the cause named. It never counts as clean.

### C# formatting

`csharpier` runs from the managed pin. Select it in a `csharp` family.

```python
quality_family(
    name = "csharp_family",
    family_id = "csharp",
    disabled = ["audit", "lint", "typecheck"],
    format = ["csharpier"],
)
```

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- format --apply //app/...
```

A check never writes. `--apply` rewrites only the C# files the selected
targets own. To change options, bind a `csharpier_config` to the C# targets
with `aspect_hints`. An unformatted file is reported at the start of the file. A syntax
error is reported at its line and column with its compiler code and never
counts as clean.

### C# lint

`roslyn` compiles the target again with the SDK analyzers and reports their
findings. The compile uses the target's own sources, references, defines and
compiler options. It needs no install step. Select it in a `csharp` family.

```python
quality_family(
    name = "csharp_family",
    family_id = "csharp",
    disabled = ["audit", "typecheck"],
    format = ["csharpier"],
    lint = ["roslyn"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //app/...
```

The analysis uses the target framework, references, and analyzers the target
builds with. The default is the SDK analysis level for that framework. To
change severities, pass a `.globalconfig` file in `analyzer_configs` on the
C# target. A compile error is reported at its line and column with its
compiler code and never counts as clean. Findings in generated files under
`bazel-out` or in external repositories are not reported, but a compile error
there still fails the check. An analyzer that fails to run fails
the check. Diagnostics suppressed in source with `#pragma warning disable` or
`[SuppressMessage]` are not reported.

Roslyn is check only. `lint --apply` changes no files. Supported on
linux_x86_64.

### F# formatting and lint

`fantomas` formats and `fsharplint` lints. Both run from the managed pin
with no install step. Select them in an `fsharp` family.

```python
quality_family(
    name = "fsharp_family",
    family_id = "fsharp",
    disabled = ["audit", "typecheck"],
    format = ["fantomas"],
    lint = ["fsharplint"],
)
```

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- format --apply //app/...
bazel run @rules_dx//:dx -- lint //app/...
```

A format check never writes. `--apply` rewrites only the F# files the
selected targets own. An unformatted file is reported at the start of the
file. `fsharplint` has no fixes, so `lint --apply` changes nothing.

`fsharplint` checks each target with its sources in compile order, its
dependency assemblies, its `defines` and its `langversion`. Rules that need
type information see the same code the compiler sees. Findings keep the
`FL` rule ID, line and column.

To change lint rules, bind an `fsharplint_config` to the F# targets with
`aspect_hints`.

```python
load("@rules_dx//quality:native_config.bzl", "fsharplint_config")

fsharplint_config(
    name = "relaxed",
    src = "relaxed.fsharplint.json",
    visibility = ["//visibility:public"],
)

fsharp_library(
    name = "core",
    aspect_hints = ["//:relaxed"],
    srcs = ["Greeter.fs"],
)
```

A syntax error is reported by both tools as an error at its line and column
with its `FS` compiler code. It never counts as clean. A tool that fails to
run, or prints output dx cannot read, fails the run with the cause named.

### Objective-C

`clang_format` formats Objective-C from the same managed pin as C and C++.
Select it in an `objc` family. The family has two classes: `objc` for `.m`
files and `objcpp` for `.mm` files. A `.h` file stays in the `cc` family. No
lint tool is wired for Objective-C yet. The `objc` family is not in
`default_workspace_policy`, so add it to your own policy.

```python
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "objc_family",
    family_id = "objc",
    disabled = ["lint", "typecheck"],
    format = ["clang_format"],
)

workspace_policy(
    name = "my_policy",
    families = [":objc_family"],
)
```

```sh
bazel run @rules_dx//:dx -- format --check //ios/... -- --@rules_dx//config:workspace=//quality:my_policy
bazel run @rules_dx//:dx -- format --apply //ios/... -- --@rules_dx//config:workspace=//quality:my_policy
```

Own the files with `objc_library` from `@rules_dx//cc/rules:defs.bzl`. The
files in `srcs`, `non_arc_srcs` and `hdrs` are checked. A plain upstream
`objc_library` is checked the same way. A check never writes. `--apply`
rewrites only the files the selected targets own. An unformatted file is
reported at the start of the file.

To change the style, bind a `clang_format_config` to the targets with
`aspect_hints`. The file is your `.clang-format`.

```python
load("@rules_dx//quality:native_config.bzl", "clang_format_config")

clang_format_config(
    name = "objc_style",
    src = ".clang-format",
    visibility = ["//visibility:public"],
)

objc_library(
    name = "greeter",
    aspect_hints = ["//:objc_style"],
    srcs = ["Greeter.m"],
    hdrs = ["Greeter.h"],
)
```

The config needs a `Language: ObjC` section or a section with no `Language`
key. A config with only a `Language: Cpp` section fails the run with the
cause named. It never counts as clean. A `.h` file with Objective-C content
is checked in the `cc` family, and clang-format reads it as Objective-C. Under
a `cc` config with only a `Language: Cpp` section, that file fails the run.

### Protocol Buffers

`buf` runs from the managed pin for format and lint. Select it in a
`protobuf` family. It runs on Linux x86_64 and needs `diff` in `/usr/bin` or
`/bin`.

```python
quality_family(
    name = "protobuf_family",
    family_id = "protobuf",
    format = ["buf"],
    lint = ["buf"],
)
```

```sh
bazel run @rules_dx//:dx -- format --apply //api/...
bazel run @rules_dx//:dx -- lint //api/...
```

Targets are `proto_library` rules. Imports resolve through the target's
`deps`. A check never writes. `--apply` rewrites only the `.proto` files the
selected targets own. Lint uses the upstream `STANDARD` rules and reports
each finding at its line and column with the rule ID. A syntax error or an
import that does not resolve fails format and lint with the cause named and
never counts as clean. A syntax error in an imported `.proto` fails lint of
every target that imports it, with that file, line and column named. An
unformatted file is reported at the start of the file. To change the rules, bind a `buf_config` to the `proto_library` targets
with `aspect_hints`. The file is your `buf.yaml`.

### CUE

`cue fmt` runs from the managed pin for format. Select it in a `cue` family.
It runs on Linux x86_64.

```python
quality_family(
    name = "cue_family",
    family_id = "cue",
    format = ["cue"],
)
```

```sh
bazel run @rules_dx//:dx -- format //config/...
bazel run @rules_dx//:dx -- format --apply //config/...
```

Own CUE files with `cue_library` from `@rules_dx//cue/rules:defs.bzl`. List the
package files in `srcs` and the libraries it imports in `deps`. Put the
`cue.mod/module.cue` of your module in the `srcs` of a `cue_library` and list
that library in `deps`. Both are staged as import context. A check never
writes. `--apply` rewrites only the `.cue` files the selected targets own. An
unformatted file is reported at the start of the file. A syntax error fails the
run with the file, line and column. It never counts as clean. `cue` takes no
config file.

### Jsonnet

`jsonnetfmt` runs from the managed pin for format. Select it in a `jsonnet`
family. It runs on Linux x86_64.

```python
quality_family(
    name = "jsonnet_family",
    family_id = "jsonnet",
    format = ["jsonnetfmt"],
)
```

```sh
bazel run @rules_dx//:dx -- format //config/...
bazel run @rules_dx//:dx -- format --apply //config/...
```

Own Jsonnet files with `jsonnet_library` from
`@rules_dx//jsonnet/rules:defs.bzl`. List the `.jsonnet` and `.libsonnet` files
of the package in `srcs` and the libraries it imports in `deps`. The imported
libraries are staged beside the entry file. A check never writes. `--apply`
rewrites only the files the selected targets own. An unformatted file is
reported at the start of the file. A syntax error fails the run with the file,
line and column. It never counts as clean. `jsonnetfmt` stops at the first
syntax error, so fix it and run again to see the next one. `jsonnetfmt` takes
no config file.

### Pkl

`pkl format` runs from the managed pin for format. Select it in a `pkl` family.
It runs on Linux x86_64.

```python
quality_family(
    name = "pkl_family",
    family_id = "pkl",
    format = ["pkl"],
)
```

```sh
bazel run @rules_dx//:dx -- format //config/...
bazel run @rules_dx//:dx -- format --apply //config/...
```

Own Pkl files with `pkl_library` from `@rules_dx//pkl/rules:defs.bzl`. List the
`.pkl` files of the package in `srcs` and the libraries it imports in `deps`. A
check never writes. `--apply` rewrites only the files the selected targets own.
An unformatted file is reported at the start of the file. A syntax error fails
the run with the file, line and column. It never counts as clean. `pkl format`
checks syntax only and does not resolve imports. It takes no config file.

To use another Pkl executable, point `tool_pkl` at it.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_pkl=//tools:pkl
```

### Terraform

`terraform fmt` runs from the managed pin for format. Select it in a
`terraform` family. It runs on Linux x86_64. It never runs `init`, reads state
or contacts a backend.

```python
quality_family(
    name = "terraform_family",
    family_id = "terraform",
    format = ["terraform"],
)
```

```sh
bazel run @rules_dx//:dx -- format //infra/...
bazel run @rules_dx//:dx -- format --apply //infra/...
```

Own Terraform files with `terraform_module` from
`@rules_dx//terraform/rules:defs.bzl`. List the `.tf`, `.tfvars`, `.tftest.hcl` and
`.tfmock.hcl` files of the module in `srcs` and the modules it calls in `deps`. A check never
writes. `--apply` rewrites only the files the selected targets own. An
unformatted file is reported at the start of the file. A syntax error fails the
run with the file and line. It never counts as clean. `terraform fmt` takes no
config file.

To use OpenTofu, point `tool_terraform` at a `tofu` executable file. The
findings keep the `terraform` tool ID.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_terraform=@tofu//:tofu
```

### Go

`gofumpt` formats Go. The default policy selects it for the `go` family, so
`dx format` checks Go files with no extra setup. It is built from source with
the rules_go toolchain. It is qualified on Linux x86_64 only for now.

```sh
bazel run @rules_dx//:dx -- format //cmd/...
bazel run @rules_dx//:dx -- format --apply //cmd/...
```

Own Go files with `go_library`, `go_binary` and `go_test` from
`@rules_dx//go/rules:defs.bzl`. A check never writes. `--apply` rewrites only
the files the selected targets own. An unformatted file is reported at the
start of the file. A syntax error fails the run and never counts as clean.
The error does not name the file or position yet:

```text
pipeline failed: invalid tool output for gofumpt: gofumpt output is outside the pinned grammar: exit 2 with no diff markers
```

dx passes `-lang` from the Go SDK the rules_go toolchain resolves, for example
`-lang=go1.26` for Go 1.26.10. The check and `--apply` use the same version, so
rewrites that depend on the language version, such as `0644` to `0o644`, agree.
`gofumpt` takes no config file.

To use your own build, point `tool_gofumpt` at an executable target. The
findings keep the `gofumpt` tool ID.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_gofumpt=//tools:gofumpt
```

### CSS, SCSS and Less

`stylelint` runs from the managed pin for lint. It runs on Linux x86_64. Select
it in the `css` family. Format stays with `prettier`.

```python
quality_family(
    name = "css_family",
    family_id = "css",
    format = ["prettier"],
    lint = ["stylelint"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //web/...
bazel run @rules_dx//:dx -- format --apply //web/...
```

Own stylesheets with `javascript_library`. It accepts `.css`, `.scss` and
`.less` in `srcs`. Lint is check-only: `lint --apply` never rewrites a
stylesheet. Each finding shows its line and column with the rule name. The
default rules are the Stylelint correctness checks. They accept CSS Modules
`:global`, `:local`, `:export`, `:import` and `composes`. SCSS and Less files
parse with `postcss-scss` and `postcss-less`.

A syntax error, an invalid config or a tool that fails to run fails the run
with the cause named. It never counts as clean. To change the rules, bind a
`stylelint_config` to the library with `aspect_hints`. The file is a JSON
config. It replaces the default rules, so add an `overrides` entry with
`customSyntax` for `*.scss` and `*.less`.

### HTML, GraphQL, JSON5 and JSONC

`prettier` runs from the managed pin for format. It runs on Linux x86_64. Select
it in the `html`, `graphql` and `json` families. JSON5 and JSONC files belong
to the `json` family.

```python
quality_family(
    name = "graphql_family",
    disabled = ["lint", "typecheck"],
    family_id = "graphql",
    format = ["prettier"],
)
```

```sh
bazel run @rules_dx//:dx -- format //web/...
bazel run @rules_dx//:dx -- format --apply //web/...
```

Own these files with `javascript_library`. It accepts `.html`, `.graphql`,
`.gql`, `.json5` and `.jsonc` in `srcs`. A check never writes. `--apply`
rewrites only the files the selected targets own. Comments in JSON5 and JSONC
stay, and both gain trailing commas. Scripts inside HTML are formatted too.
An unformatted file is reported at the start of the file. A syntax error fails the run with the file, line and
column. It never counts as clean. Without a config, `prettier` runs with its
default options.

To change options, bind a `prettier_config` to the targets with `aspect_hints`.

```python
load("@rules_dx//quality:native_config.bzl", "prettier_config")

prettier_config(
    name = "prettier_options",
    src = ".prettierrc.json",
    visibility = ["//visibility:public"],
)

javascript_library(
    name = "pages",
    srcs = ["index.html"],
    aspect_hints = ["//:prettier_options"],
)
```

Accepted file names end in `.prettierrc`, `.json`, `.json5`, `.yaml`, `.yml`,
`.toml`, `.mjs`, `.cjs` or `.js`. List files the config imports in `data`.
`overrides` globs match from the config's directory. `.editorconfig` and
`.prettierignore` are not read, and plugins are not loaded. An unknown parser or
an invalid config fails the run. `requirePragma` still skips files without a
pragma.

### Vue

`vue_library` from `@rules_dx//vue/rules:defs.bzl` owns `.vue` files. The
default policy formats them with `prettier` from the managed pin. It runs on
Linux x86_64.

```sh
bazel run @rules_dx//:dx -- format //web/...
bazel run @rules_dx//:dx -- format --apply //web/...
```

A check never writes. `--apply` rewrites only the files the selected targets
own. The template, script and style blocks are all formatted. An unformatted
file is reported at the start of the file. A syntax error fails the run with
the file, line and column. It never counts as clean. `.prettierrc`,
`.editorconfig` and other user config files are ignored.

`eslint` lint is opt-in. Select it in the `vue` family and bind an
`eslint_config` to the library with `aspect_hints`.

```python
quality_family(
    name = "vue_family",
    disabled = ["audit", "typecheck"],
    family_id = "vue",
    format = ["prettier"],
    lint = ["eslint"],
)

eslint_config(
    name = "eslint",
    src = "eslint.config.mjs",
)

vue_library(
    name = "hello",
    srcs = ["Hello.vue"],
    aspect_hints = [":eslint"],
)
```

The config may import `eslint-plugin-vue` 10.12.0, `vue-eslint-parser` 10.4.1
and `@typescript-eslint/parser` 8.72.1 by name. dx provides them. Any other
package must resolve from your workspace.

```js
import pluginVue from "eslint-plugin-vue";

export default [
  ...pluginVue.configs["flat/essential"],
  {
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: { parser: "@typescript-eslint/parser" },
    },
  },
];
```

Findings point at the line and column in the `.vue` file. A `.vue` file that no
config entry matches fails the run. It never counts as clean.

### GraphQL lint

`graphql_eslint` checks GraphQL schemas and operations against the schema. It
runs `@graphql-eslint/eslint-plugin` 4.4.2 with `graphql` 16.14.2 on the managed
ESLint 10.10.0 pin. It runs on Linux x86_64. Linux arm64, macOS arm64 and
Windows x86_64 are not supported yet. It is opt-in: select it in the `graphql`
family next to `prettier`, which keeps format.

```python
quality_family(
    name = "graphql_family",
    disabled = ["typecheck"],
    family_id = "graphql",
    format = ["prettier"],
    lint = ["graphql_eslint"],
)
```

The tool needs a schema, so it needs a config. Bind a `graphql_eslint_config`
to the library with `aspect_hints`, and list the schema and any shared
documents in `data`:

```python
load("@rules_dx//quality:native_config.bzl", "graphql_eslint_config")

graphql_eslint_config(
    name = "graphql_lint",
    src = "graphql_lint.json",
    data = [
        "fragments.graphql",
        "schema.graphql",
    ],
)
```

```json
{
  "schema": "graph/schema.graphql",
  "documents": ["graph/fragments.graphql"],
  "rules": {
    "operations": {
      "@graphql-eslint/fields-on-correct-type": "error",
      "@graphql-eslint/known-fragment-names": "error"
    }
  }
}
```

The config is a JSON object with these keys. Any other key fails the run.

- `schema` (required): one workspace-relative path or a list of them. Each
  path must name a file. Globs, URLs, absolute paths and `..` are rejected.
- `documents` (optional): the same shape. These are the operations and
  fragments the checked files may use. The files a target owns are always
  added.
- `rules` (optional): `{"schema": {...}, "operations": {...}}`. Each map
  replaces its default rule set. Every rule ID must start with
  `@graphql-eslint/`.
- `baseline`: reserved for the breaking-change check. It is not supported yet,
  so setting it fails the run.

A file named in `schema` gets the schema rules. Every other file gets the
operation rules. The default schema rules are `known-type-names`,
`known-directives`, `known-argument-names`, `provided-required-arguments`,
`unique-type-names`, `unique-field-definition-names`,
`unique-enum-value-names`, `unique-directive-names`,
`unique-directive-names-per-location`, `unique-operation-types`,
`lone-schema-definition` and `possible-type-extension`. The default operation
rules are the GraphQL validation rules: `executable-definitions`,
`fields-on-correct-type`, `fragments-on-composite-type`,
`known-argument-names`, `known-directives`, `known-fragment-names`,
`known-type-names`, `lone-anonymous-operation`, `no-fragment-cycles`,
`no-undefined-variables`, `no-unused-variables`, `one-field-subscriptions`,
`overlapping-fields-can-be-merged`, `possible-fragment-spread`,
`provided-required-arguments`, `scalar-leafs`, `unique-argument-names`,
`unique-directive-names-per-location`, `unique-input-field-names`,
`unique-variable-names`, `value-literals-of-correct-type`,
`variables-are-input-types` and `variables-in-allowed-position`.
`no-unused-fragments` is not a default: each target is linted on its own, so a
target that only defines shared fragments would report them as unused. Style
rules are off unless you add them.

```sh
bazel run @rules_dx//:dx -- lint //graph/...
bazel run @rules_dx//:dx -- format --apply //graph/...
```

Lint is check-only: `lint --apply` never rewrites a file. Each finding has the
file, line, column, the rule ID such as
`@graphql-eslint/fields-on-correct-type`, and the message. A syntax error is
reported at its line and column. A schema that does not build is reported at
the start of each checked file. A missing config, an invalid config, a path
that names no file, or a tool that fails to run fails the run with the cause
named. It never counts as clean. The tool never reads `graphql.config.*`
files and never fetches a schema over the network.

### OpenAPI and AsyncAPI lint

`redocly` lints OpenAPI 3.0, 3.1 and 3.2 and AsyncAPI 2.6 and 3.0 documents
with Redocly CLI 2.71.1. It runs on Linux x86_64. Linux arm64, macOS arm64 and
Windows x86_64 are not supported yet. It is opt-in: select it in the
`api_spec` family.

```python
quality_family(
    name = "api_spec_family",
    family_id = "api_spec",
    lint = ["redocly"],
)
```

Declare specs with `api_spec_library`. `srcs` are the roots that get linted.
`refs` are files the roots reach through `$ref`. `deps` are other
`api_spec_library` targets whose files a root may refer to. Each accepts
`.yaml`, `.yml` and `.json`.

```python
load("@rules_dx//api_spec/rules:defs.bzl", "api_spec_library")

api_spec_library(
    name = "pets",
    srcs = ["openapi.yaml"],
    refs = ["schemas/pet.yaml"],
    deps = ["//shared"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //pets/...
```

Each root is linted on its own with every file its `$ref` graph reaches. A
finding in the root has its line, column, rule ID such as
`operation-operationId-unique`, and message. A finding in a ref lands on line
1 of the root, and its message starts with the ref's own `file:line:col:`.

Before Redocly runs, dx checks each root and every file it reaches. Each
problem is an error finding on the root:

- `dx/api-spec-syntax`: the file is not valid YAML or JSON, or holds more than
  one YAML document.
- `dx/api-spec-root`: the root has no `openapi` or `asyncapi` key, names an
  unsupported version, or is Swagger 2.0.
- `dx/api-spec-remote-ref`: a `$ref` names a URL. Lint runs offline. Vendor
  the file and list it in `refs`.
- `dx/api-spec-unresolved-ref`: a `$ref` names a file that is not in `srcs`,
  `refs` or `deps`, an absolute path, or a path that leaves the workspace.

The default config extends Redocly's `spec` ruleset and sets
`no-unresolved-refs`, `operation-operationId-unique`,
`path-parameters-defined`, `no-identical-paths` and
`operation-parameters-unique` to `error`. To use your own rules, bind a
`redocly_config` to the library with `aspect_hints`. The file ends in `.yaml`
or `.yml`.

```python
load("@rules_dx//quality:native_config.bzl", "redocly_config")

redocly_config(
    name = "redocly",
    src = "redocly.yaml",
)
```

```yaml
extends:
  - recommended
rules:
  info-license: off
```

`extends` may only name the built-in rulesets `all`, `minimal`,
`recommended`, `recommended-strict` and `spec`. A config with `plugins` or
`resolve` at any depth fails the run. An unknown key or rule fails the run.

Lint is check-only: `lint --apply` never rewrites a file. A config dx rejects,
a config Redocly rejects, output dx cannot read, or a tool that fails to run
fails the run with the cause named. It never counts as clean. Telemetry and
update notices are off.

### Astro

`astro_library` from `@rules_dx//astro/rules:defs.bzl` owns `.astro` files. The
default policy formats them with `prettier` and `prettier-plugin-astro` 1.1.2
from the managed pin. It runs on Linux x86_64.

```sh
bazel run @rules_dx//:dx -- format //web/...
bazel run @rules_dx//:dx -- format --apply //web/...
```

A check never writes. `--apply` rewrites only the files the selected targets
own. The frontmatter, template, style and script blocks are all formatted. An
unformatted file is reported at the start of the file. A syntax error fails the
run with the file, line and column. It never counts as clean. `.prettierrc`,
`.editorconfig` and other user config files are ignored. `astro check` and
`astro build` are not run.

`eslint` lint is opt-in. Select it in the `astro` family and bind an
`eslint_config` to the library with `aspect_hints`.

```python
quality_family(
    name = "astro_family",
    disabled = ["audit", "typecheck"],
    family_id = "astro",
    format = ["prettier"],
    lint = ["eslint"],
)

eslint_config(
    name = "eslint",
    src = "eslint.config.mjs",
)

astro_library(
    name = "hello",
    srcs = ["Hello.astro"],
    aspect_hints = [":eslint"],
)
```

The config may import `eslint-plugin-astro` 3.2.1, `astro-eslint-parser` 3.2.0
and `@typescript-eslint/parser` 8.72.1 by name. dx provides them. Any other
package must resolve from your workspace.

```js
import eslintPluginAstro from "eslint-plugin-astro";

export default [
  ...eslintPluginAstro.configs.recommended,
  {
    files: ["**/*.astro"],
    languageOptions: {
      parserOptions: { parser: "@typescript-eslint/parser" },
    },
  },
];
```

Findings point at the line and column in the `.astro` file, including findings
in `<script>` blocks. A `.astro` file that no config entry matches fails the
run. It never counts as clean.

#### Building and checking an app

`astro_build` and `astro_check` are Bazel targets. They run the `astro` and
`@astrojs/check` from your own `node_modules`. `dx check` does not run them.

```python
load("@rules_dx//astro/rules:defs.bzl", "astro_build", "astro_check")
load("@rules_dx//javascript/rules:web.bzl", "javascript_web_server")

astro_build(
    name = "site",
    srcs = glob(["src/**"]),
    config = "astro.config.mjs",
    node_modules = "//:node_modules",
    package_json = "package.json",
    public = glob(["public/**"]),
    deps = ["//:node_modules/@astrojs/preact", "//:node_modules/preact"],
)

astro_check(
    name = "check",
    srcs = glob(["src/**"]),
    config = "astro.config.mjs",
    node_modules = "//:node_modules",
    package_json = "package.json",
    tsconfig = "tsconfig.json",
    deps = ["//:node_modules/@astrojs/preact", "//:node_modules/preact"],
)

javascript_web_server(
    name = "serve",
    app = ":site",
)
```

```sh
bazel build //web:site
bazel test //web:check
bazel run //web:serve
```

`astro_build` writes the static site to a `site/` tree. `javascript_web_server`
serves it. `astro_check` runs `astro check` once into
`check.astro_check.txt`. The file starts with `exit_code: <n>` and holds the
diagnostics, for example
`src/pages/index.astro:7:15 - error ts(2322): ...`. The test fails and prints
the report when the exit code is not 0. A crash or a missing package fails the
build. It never counts as clean.

`package_json` must list the integrations your config uses. Astro reads it to
bundle them. Tested with `astro` 7.3.8, `@astrojs/check` 0.9.12,
`@astrojs/preact` 6.0.6 and `typescript` 6.0.3.

### MDX

`mdx_library` from `@rules_dx//mdx/rules:defs.bzl` owns `.mdx` files. The
default policy formats them with `remark` from the managed pin (remark-stringify
11.0.0 with remark-mdx 3.1.1, remark-gfm 4.0.1 and remark-frontmatter 5.0.0). It
runs on Linux x86_64. MDX has no typecheck.

```sh
bazel run @rules_dx//:dx -- format //docs/...
bazel run @rules_dx//:dx -- fix --apply //docs/...
```

A check never writes. `--apply` rewrites only the files the selected targets
own. An unformatted file is reported at the start of the file. A parse error,
such as an unclosed JSX tag or a bad `{expression}`, fails the run with the
file, line and column. It never counts as clean.

The formatter parses its own output again and compares the syntax trees. If
formatting would change what the document means, the run fails with "would
change the document tree" and no file is written. `.remarkrc` files and remark
plugins are ignored.

The formatter reads GFM (task lists, strikethrough, tables, footnotes) and YAML
`---` or TOML `+++` frontmatter, so it keeps them as written. It aligns table
columns and leaves bare URLs such as `www.example.com` as they are. Compile with
remark-gfm and remark-frontmatter so the page renders the same way.

The default style matches Prettier's Markdown style: `-` bullets, `_emphasis_`,
`**strong**`, `---` rules and backtick fences. To change it, bind a
`remark_config` to the library with `aspect_hints`. The file is a JSON object of
remark-stringify options: `bullet`, `bulletOther`, `bulletOrdered`, `closeAtx`,
`emphasis`, `fence`, `fences`, `incrementListMarker`, `listItemIndent`,
`quote`, `resourceLink`, `rule`, `ruleRepetition`, `ruleSpaces`, `setext`,
`strong` and `tightDefinitions`. Your keys replace the defaults one by one. Any
other key, such as `plugins`, fails the run.

```python
remark_config(
    name = "remark",
    src = "remark.json",
)
```

```json
{ "bullet": "*" }
```

`eslint` lint is opt-in. Select it in the `mdx` family and bind an
`eslint_config` to the library with `aspect_hints`. The config may import
`eslint-plugin-mdx` 3.8.2 by name. dx provides it. Any other package must
resolve from your workspace.

```python
quality_family(
    name = "mdx_family",
    disabled = ["audit", "typecheck"],
    family_id = "mdx",
    format = ["remark"],
    lint = ["eslint"],
)

mdx_library(
    name = "docs",
    srcs = glob(["*.mdx"]),
    aspect_hints = [":eslint", ":remark"],
)
```

```js
import * as mdx from "eslint-plugin-mdx";

export default [
  {
    ...mdx.flat,
    files: ["**/*.mdx"],
    languageOptions: {
      ...mdx.flat.languageOptions,
      parserOptions: { ...mdx.flat.languageOptions.parserOptions, ignoreRemarkConfig: true },
    },
    rules: { ...mdx.flat.rules, "mdx/remark": "off", "no-unused-vars": "error" },
  },
];
```

Keep `ignoreRemarkConfig: true` and `"mdx/remark": "off"` so lint does not read
`.remarkrc` files. Findings point at the line and column in the `.mdx` file.

The formatter exits 0 when every file is formatted, 1 when a file is not, and 2
on a parse error, a changed syntax tree or a bad `remark_config`. dx reports
each of these as a finding or a failed run.

### SQL, XML and Gherkin

`prettier` runs from the managed pin with the `prettier-plugin-sql`,
`@prettier/plugin-xml` and `prettier-plugin-gherkin` plugins for format. It runs
on Linux x86_64. Select it in the `sql`, `xml` and `gherkin` families.

```python
quality_family(
    name = "sql_family",
    disabled = ["lint", "typecheck"],
    family_id = "sql",
    format = ["prettier"],
)
```

```sh
bazel run @rules_dx//:dx -- format //db/...
bazel run @rules_dx//:dx -- format --apply //db/...
```

Own the files with `sql_library` from `@rules_dx//sql/rules:defs.bzl`,
`xml_library` from `@rules_dx//xml/rules:defs.bzl` and `gherkin_library` from
`@rules_dx//gherkin/rules:defs.bzl`. They accept `.sql`; `.xml`, `.xsd`, `.xsl`,
`.xslt` and `.wsdl`; and `.feature` in `srcs`, and other libraries in `deps`. A
check never writes. `--apply` rewrites only the files the selected targets own.
An unformatted file is reported at the start of the file. A syntax error fails
the run with the file, line and column where the tool gives one. It never counts
as clean.

SQL files use the generic `sql` dialect. To pick a dialect, put it before the
extension: `schema.postgresql.sql`. The dialects are `bigquery`, `clickhouse`,
`db2`, `db2i`, `hive`, `mariadb`, `mysql`, `n1ql`, `plsql`, `postgresql`,
`redshift`, `singlestoredb`, `snowflake`, `spark`, `sqlite`, `transactsql`,
`trino` and `tsql`. The pinned options keep SQL keywords in the case you
wrote them. XML namespaces, prefixes, attribute quotes, attribute order and
text stay as written. Gherkin keeps its step text and tags. `.prettierrc`,
`.editorconfig` and other user config files are ignored.

### PowerShell

`psscriptanalyzer` runs from the managed PSScriptAnalyzer 1.25.0 pin for lint.
It runs on Linux x86_64. Select it in the `powershell` family.

```python
quality_family(
    name = "powershell_family",
    family_id = "powershell",
    lint = ["psscriptanalyzer"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //scripts/...
```

Own scripts and modules with `pwsh_library` and `pwsh_binary` from
`@rules_dx//powershell/rules:defs.bzl`. `pwsh_library` accepts `.ps1`, `.psm1`
and `.psd1` in `srcs`. `pwsh_binary` accepts `.ps1`. A `using module` or
`Import-Module` of a module in `deps` resolves in lint against the target's
declared modules only. Lint never searches modules installed on the host. When
you run or test the target, the host's module path is still searched after the
declared modules. For Pester
tests, add `@rules_dx//powershell/gallery:pester` to the `deps` of a
`pwsh_test`.

Lint is check-only: `lint --apply` never rewrites a script. Each finding shows
its line and column with the rule name. A script that does not parse is
reported as an error.

An invalid config or a tool that fails to run fails the run with the cause
named. It never counts as clean. To change the rules, bind a
`psscriptanalyzer_config` to the target with `aspect_hints`. The file is a
`.psd1` settings file. Unknown rule names in `IncludeRules` or `ExcludeRules`
are errors.

### Ruby formatting and lint

`standardrb` formats and `rubocop` lints. Both run from the gems of a
`rules_ruby` bundle you declare, never from ambient gems or Bundler. Linux x86_64
only. Select them in a `ruby` family. Only `standardrb` formats.

Declare your own Ruby toolchain. Copy `third_party/ruby/Gemfile`, `Gemfile.lock`
and its `gem_checksums` into your module, then point the tools at the bundle in
`.bazelrc`.

```python
bazel_dep(name = "rules_ruby", version = "0.28.0")

ruby = use_extension("@rules_ruby//ruby:extensions.bzl", "ruby")
ruby.toolchain(
    name = "ruby",
    portable_ruby = True,
    version = "3.4.9",
)
ruby.bundle_fetch(
    name = "dx_bundle",
    gem_checksums = {...},
    gemfile = "//:Gemfile",
    gemfile_lock = "//:Gemfile.lock",
)
use_repo(ruby, "dx_bundle", "ruby_toolchains")

register_toolchains("@ruby_toolchains//:all")
```

```text
build --@rules_dx//quality/tools/ruby:rubocop=@dx_bundle//bin:rubocop
build --@rules_dx//quality/tools/ruby:standardrb=@dx_bundle//bin:standardrb
```

Without these flags the Ruby tools exit `2` and say which flag is missing.
Other languages are unaffected.

```python
quality_family(
    name = "ruby_family",
    family_id = "ruby",
    disabled = ["audit", "typecheck"],
    format = ["standardrb"],
    lint = ["rubocop"],
)
```

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- format --apply //app/...
bazel run @rules_dx//:dx -- lint //app/...
```

A check never writes. `--apply` rewrites only the Ruby files the selected
targets own. `rubocop` has no fixes, so `lint --apply` changes nothing.
Lint findings keep the cop name and line. A syntax error is reported as an
error at its line and never counts as clean. An unreadable config fails the run.
A cop crash or a configuration error fails the target, even when every
offense is baselined.

To change options, bind a `rubocop_config` or `standardrb_config` to the Ruby
targets with `aspect_hints`.

```python
load("@rules_dx//quality:native_config.bzl", "rubocop_config")

rubocop_config(
    name = "relaxed",
    src = "relaxed.rubocop.yml",
    visibility = ["//visibility:public"],
)

ruby_library(
    name = "core",
    aspect_hints = ["//:relaxed"],
    srcs = ["greeter.rb"],
)
```

### YAML

`yamlfmt` formats and `yamllint` lints. Both run from managed pins on Linux
x86_64. Select them in a `yaml` family. Only `yamlfmt` has fixes.

```python
quality_family(
    name = "yaml_family",
    family_id = "yaml",
    format = ["yamlfmt"],
    lint = ["yamllint"],
)
```

`yamllint` runs from a Python 3.12 toolchain. Declare the same toolchain in your
`MODULE.bazel` as `rules_dx` does: `python_interpreters.toolchain` and
`python.toolchain` with `python_version = "3.12"`, plus `rules_python` and
`aspect_rules_py`.

```sh
bazel run @rules_dx//:dx -- format //config/...
bazel run @rules_dx//:dx -- format --apply //config/...
bazel run @rules_dx//:dx -- lint //config/...
```

Own YAML files with `yaml_library` from `@rules_dx//yaml/rules:defs.bzl`. List
the `.yaml` and `.yml` files of the package in `srcs`. A check never writes.
`--apply` rewrites only the files the selected targets own. `yamllint` is
check-only. Multiple documents and anchors are supported.

An unformatted file is a warning. A parse error is an error with the file and
line, and it never counts as clean. `yamllint` findings keep the rule name,
line and column.

To change options, bind a `yamlfmt_config` or `yamllint_config` to the YAML
targets with `aspect_hints`.

```python
load("@rules_dx//quality:native_config.bzl", "yamlfmt_config", "yamllint_config")

yamlfmt_config(
    name = "indent4",
    src = "indent4.yamlfmt",
    visibility = ["//visibility:public"],
)

yamllint_config(
    name = "relaxed",
    src = "relaxed.yamllint",
    visibility = ["//visibility:public"],
)

yaml_library(
    name = "app",
    aspect_hints = [
        "//:indent4",
        "//:relaxed",
    ],
    srcs = ["app.yaml"],
)
```

### HTML templates

`djlint` formats and lints HTML templates. It runs from a managed pin on Linux
x86_64. Select it in an `html_template` family. Only the format capability has
fixes. Lint is check-only.

```python
quality_family(
    name = "html_template_family",
    family_id = "html_template",
    format = ["djlint"],
    lint = ["djlint"],
)
```

`djlint` runs from a Python 3.12 toolchain. Declare the same toolchain in your
`MODULE.bazel` as `rules_dx` does: `python_interpreters.toolchain` and
`python.toolchain` with `python_version = "3.12"`, plus `rules_python` and
`aspect_rules_py`.

```sh
bazel run @rules_dx//:dx -- format //web/...
bazel run @rules_dx//:dx -- format --apply //web/...
bazel run @rules_dx//:dx -- lint //web/...
```

Own templates with `html_template_library` from
`@rules_dx//html_template/rules:defs.bzl`. List the `.html`, `.htm`, `.jinja`,
`.jinja2`, `.j2`, `.njk`, `.hbs`, `.gohtml` and `.tmpl` files of the package in
`srcs`. Use `deps` for libraries that your templates include from. A check never
writes. `--apply` rewrites only the files the selected targets own.

An unformatted file is a warning. Lint findings keep the rule code, line and
column. A profile that `djlint` does not support, or a config file it cannot
load, is an error and never counts as clean.

To change options, bind a `djlint_config` to the template targets with
`aspect_hints`. The file is named `djlint.toml` or `.djlintrc`. Set the dialect with `profile`: `all`, `angular`,
`askama`, `django`, `golang`, `handlebars`, `html`, `jinja`, `liquid`,
`nunjucks` or `tera`. Without a bound config, configs in the workspace are
ignored.

```python
load("@rules_dx//quality:native_config.bzl", "djlint_config")

djlint_config(
    name = "indent2",
    src = ".djlintrc",
    visibility = ["//visibility:public"],
)

html_template_library(
    name = "web",
    aspect_hints = ["//:indent2"],
    srcs = ["templates/page.html"],
)
```

### Dockerfiles

`hadolint` lints Dockerfiles. It runs from a managed pin on Linux x86_64, Linux
arm64, macOS arm64 and Windows x86_64. The default policy selects it in the
`dockerfile` family. There is no formatter.

```python
quality_family(
    name = "dockerfile_family",
    family_id = "dockerfile",
    lint = ["hadolint"],
)
```

Own Dockerfiles with `dockerfile_library` from
`@rules_dx//dockerfile/rules:defs.bzl`. List the files of the package in
`srcs`. Accepted names are `Dockerfile`, `Containerfile`, `Dockerfile.*`,
`Containerfile.*`, `*.Dockerfile`, `*.dockerfile`, `*.Containerfile` and
`*.containerfile`. Any other name fails the analysis with the accepted names
listed. `.dockerignore` is not a Dockerfile.

```python
load("@rules_dx//dockerfile/rules:defs.bzl", "dockerfile_library")

dockerfile_library(
    name = "image",
    srcs = ["Dockerfile"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //images/...
```

`hadolint` is check-only: `lint --apply` never rewrites a Dockerfile. Findings
keep the rule code, line and column. `DL` codes are hadolint rules. `SC` codes
come from the ShellCheck that hadolint links for `RUN` lines. A Dockerfile that
does not parse is a `DL1000` error and never counts as clean.

`hadolint` never reads a `.hadolint.yaml` from the workspace, the home
directory or the XDG config directory, and `HADOLINT_*` variables from the host
do not reach it. To change the rules, bind a `hadolint_config` to the target
with `aspect_hints`. Use `ignored`, `override`, `trustedRegistries`, `label-schema`
or `strict-labels`. Do not set `format`, `output` or `no-color`: the run reads
JSON, so another format fails with the tool named. A config that hadolint cannot
parse is an error, not a fallback to the defaults. `failure-threshold` and
`no-fail` change only the exit code. Every finding is still reported.

```python
load("@rules_dx//quality:native_config.bzl", "hadolint_config")

hadolint_config(
    name = "hadolint",
    src = "hadolint.yaml",
)

dockerfile_library(
    name = "image",
    aspect_hints = [":hadolint"],
    srcs = ["Dockerfile"],
)
```

Inline `# hadolint ignore=DL3008` comments are honoured.

### Shell

`shfmt` formats and `shellcheck` lints shell sources. Both run in the `shell`
family.

```python
quality_family(
    name = "shell_family",
    family_id = "shell",
    format = ["shfmt"],
    lint = ["shellcheck"],
)
```

Today only targets that feed the `shell` family explicitly (for example through
`real_source_target`) reach `shellcheck` and `shfmt`. Plain rules_shell
`sh_binary`, `sh_library` and `sh_test` sources are not classified as `shell`
yet, so `dx lint` does not lint them.

`shellcheck` lint is check-only: `lint --apply` never rewrites a script.
`shfmt` formatting is the part that `--apply` rewrites.

`shellcheck` never reads a `.shellcheckrc` from the host. A file in a parent
directory of the script is ignored, so a lint result does not depend on the
machine it runs on. To change the rules, bind a `shellcheck_config` to the
target with `aspect_hints`. The file is passed with `--rcfile`. Without a config, `--norc` is passed.
The user-level `~/.shellcheckrc` and XDG config are ignored too. Name the file
something other than `.shellcheckrc`.

```python
load("@rules_dx//quality:native_config.bzl", "shellcheck_config")

shellcheck_config(
    name = "shellcheck_rc",
    src = "lint.shellcheckrc",
)
```

## Baselines

Opt in to a baseline to keep known findings while new ones fail. Set the
file in `dx.toml`:

```toml
[quality]
baseline = "quality/baseline.json"
```

`dx.local.toml` can override the selection; CI reads only `dx.toml`.

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

A finding without a path records the Bazel `target` that produced it:

```json
{
  "tool": "audit",
  "rule": "",
  "path": "",
  "target": "//app:lint",
  "message": "known advisory",
  "context": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "count": 1
}
```

Identity is tool, rule, path, message, and source context, plus the target for
a finding without a path. Line numbers
alone never match, so moving a line keeps its entry. A rename or a rule
change needs a refresh instead.

Checks print `Baseline <file>: <total> total, <new> new, <suppressed>
suppressed.` Suppressed findings print with `(baselined)` and stay in JSON
and SARIF output. JSON marks them with `"baseline": "suppressed"` and
carries the counts in `command_finished`. SARIF carries the counts in each
run. New findings fail as usual. Fixed entries turn stale and fail. Entries
outside the checked tool and file scope, or tool and target scope for a
finding without a path, stay unevaluated. An incomplete run never goes
green through a baseline.

Pass `--apply` to refresh the file. Refresh adds new findings, prunes fixed
entries in scope, and keeps the rest. An entry is in scope only for the
tool and file that run analyzed, or the tool and target for an entry without a
path. Refresh never writes after an incomplete
run, a result that did not converge, or a finding whose source file cannot
be read; the file stays byte for byte as it was. A malformed file fails
instead of being rewritten.

Adopt in small steps. Run the check, review the findings, record them with
`--apply`, then keep the file current on later runs.
