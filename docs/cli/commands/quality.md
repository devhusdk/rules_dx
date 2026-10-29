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
Rust binary, test and clippy actions wait for each dependency's full rlib, not its `.rmeta`.
Only library actions start early. To turn pipelining off, set
`--@rules_rust//rust/settings:pipelined_compilation=false` in your `.bazelrc`.
A bare `bazel build` with the quality aspect also needs `--@rules_rust//rust/settings:rustc_output_diagnostics=true` for the metadata (rmeta) diagnostics group to exist. `dx` sets it for you.

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
- `--output text|diff|json`: result shape. JSON `command_finished` carries
  the Bazel `actions` counts of the probe and the build together, and text ends with a
  `dx: <total> actions, ...` line on stderr.
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

A target that only reads a file never conflicts with a target that fixes it
when both saw the same original bytes. The reading target's findings on that
file are reported against the original bytes (snapshot `initial`).

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
targets on stderr, followed by one `next:` line. JSON reports a `change` with effect `refused` and reason
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

In check mode, JSON output adds an info `notice` with code `changes_proposed`
when files need changes. Its `message` is the `--apply` command to run. The
notice is absent with `--apply`.

A failed Bazel build lists each failed action the way `dx build` does: a
`build_failure` event in JSON, or a `dx: build failure:` line on stderr. A Rust
compile error in a `Clippy` or `Rustc` action carries its `path`, `line`,
`column` and `message`; see [`dx build`](build-test-coverage.md). If the
Bazel event file is missing or ends early, the run reports a
`build_events_incomplete` notice. When every failed action is a compile
error, the run reports `bazel_failed` with
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
| `go` | `gofumpt` | disabled | `gotype` |
| `graphql` | `prettier` | disabled | disabled |
| `html` | `prettier` | disabled | disabled |
| `java` | `google_java_format` | `checkstyle`, `pmd`, `spotbugs` | `javac` |
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
| `svelte` | `prettier` | disabled | disabled |
| `text` | disabled | `keep_sorted` | disabled |
| `toml` | `taplo` | `taplo` | disabled |
| `typescript` | `biome` | `biome` | `tsc` |
| `vue` | `prettier` | disabled | disabled |

### Wired tools

Every wired tool, with the verbs it serves and the families its classes belong
to.

| Tool ID | Verbs | Families |
| --- | --- | --- |
| `ansible_lint` | lint | ansible |
| `biome` | format, lint | javascript, json, typescript |
| `buf` | format, lint | protobuf |
| `buildifier` | format, lint | starlark |
| `capnp` | lint, typecheck | capnp |
| `cfn_lint` | lint | cloudformation |
| `checkmake` | lint | make |
| `checkstyle` | lint | java |
| `clang_format` | format | cc, cuda, objc |
| `clang_syntax` | typecheck | cc, objc |
| `clang_tidy` | lint | cc |
| `cmakelint` | lint | cmake |
| `clippy` | lint | rust |
| `cppcheck` | lint | cc |
| `csc` | typecheck | csharp |
| `csharpier` | format | csharp |
| `cue` | format, typecheck | cue |
| `deadnix` | lint | nix |
| `dfmt` | format | d |
| `dialyzer` | typecheck | erlang, elixir |
| `luals` | typecheck | lua |
| `phpcs` | lint | php |
| `phpstan` | typecheck | php |
| `dart` | format, lint, typecheck | dart |
| `dscanner` | lint | d |
| `djlint` | format, lint | html_template |
| `elvis` | lint | erlang |
| `erlfmt` | format | erlang |
| `errcheck` | lint | go |
| `eslint` | lint | astro, javascript, mdx, svelte, vue |
| `fantomas` | format | fsharp |
| `flake8` | lint | python |
| `flang` | typecheck | fortran |
| `flatc` | lint, typecheck | flatbuffers |
| `fortitude` | lint | fortran |
| `fsc` | typecheck | fsharp |
| `fsharplint` | lint | fsharp |
| `gersemi` | format | cmake |
| `gofumpt` | format | go |
| `gotype` | typecheck | go |
| `google_java_format` | format | java |
| `govet` | lint | go |
| `gplint` | lint | gherkin |
| `graphql_eslint` | lint | graphql |
| `hadolint` | lint | dockerfile |
| `hlint` | lint | haskell |
| `javac` | typecheck | java |
| `jsonnet_lint` | lint | jsonnet |
| `jsonnetfmt` | format | jsonnet |
| `julia_format` | format | julia |
| `keep_sorted` | lint | text |
| `ktfmt` | format | kotlin |
| `ktlint` | lint | kotlin |
| `kubeconform` | lint | kubernetes |
| `latexindent` | format | latex |
| `ldc` | typecheck | d |
| `markdown_check` | lint | markdown |
| `mbake` | format | make |
| `credo` | lint | elixir |
| `mix_format` | format | elixir |
| `modfmt` | format | go_module |
| `nixfmt` | format | nix |
| `notebook_outputs` | lint | jupyter |
| `nvcc_syntax` | typecheck | cuda |
| `ormolu` | format | haskell |
| `opa` | format, typecheck | rego |
| `perlcritic` | lint | perl |
| `perltidy` | format | perl |
| `pkl` | format | pkl |
| `pmd` | lint | java |
| `prettier` | format | ansible, api_spec, astro, css, dockerfile, gherkin, graphql, html, javascript, json, kubernetes, php, sql, svelte, typescript, vue, xml |
| `psscriptanalyzer` | format, lint | powershell |
| `pydoclint` | lint | python |
| `pylint` | lint | python |
| `qmlformat` | format | qml |
| `qmllint` | lint, typecheck | qml |
| `redocly` | lint | api_spec |
| `regal` | lint | rego |
| `remark` | format | mdx |
| `roslyn` | lint | csharp |
| `rubocop` | lint | ruby |
| `ruff` | audit, format, lint | python, jupyter (format and lint only) |
| `rustc` | typecheck | rust |
| `rustfmt` | format | rust |
| `scalafix` | lint | scala |
| `scalafmt` | format | scala |
| `shellcheck` | lint | shell |
| `shfmt` | format | shell |
| `spotbugs` | lint | java |
| `sqlfluff` | format, lint | sql |
| `standardrb` | format | ruby |
| `staticcheck` | lint | go |
| `statix` | lint | nix |
| `stylelint` | lint | css |
| `stylua` | format | lua |
| `taplo` | format, lint | toml |
| `terraform` | format | terraform |
| `tflint` | lint | terraform |
| `tsc` | typecheck | typescript, tsx |
| `ty` | typecheck | python, jupyter |
| `typstyle` | format | typst |
| `vale` | lint | markdown |
| `yamlfmt` | format | yaml |
| `yamllint` | lint | yaml |
| `zig_ast_check` | lint | zig |
| `zig_fmt` | format | zig |

C, C++ and Objective-C (`clang_syntax`) typecheck is opt-in (`typecheck = ["clang_syntax"]` on the `cc` family). It replays each compile action of the target with `-fsyntax-only`, using the exact command line, inputs and environment Bazel configured, so includes, defines and copts match the real build. The compiler is whatever the action names, so a GCC toolchain also works; a toolchain without a readable command line (clang-cl/MSVC, params files) fails with an explicit error. Headers are checked only when the target compiles them (`--features=parse_headers`); a header-only library without that feature fails instead of passing unchecked. The tool has no fix support.

CUDA (`nvcc_syntax`) typecheck is opt-in (`typecheck = ["nvcc_syntax"]` on the `cuda` family). It replays each `CudaCompile` action with the exact nvcc command line, inputs and environment Bazel configured, changing only `-c` to `-cuda` (nvcc's front-end stage, which stops before the host compile), the output to `/dev/null`, and dropping dependency-file and keep-temporary flags. Nothing else is translated, so `-gencode`, `-Xcompiler` and `-ccbin` match the real build. Device compilation and ptxas still run for every `-gencode` architecture, so the check is heavy; it is only cheaper than the build by the host compile. Host-compiler-only warnings (for example `-W` flags passed through `-Xcompiler`) are not reported. Diagnostics inside a header owned by another target are dropped because nvcc prints no include chain; the failing exit then reports an error without a located finding. Params files and header-only `cuda_library` targets (which have no compile action) fail with an explicit error. The tool has no fix support.

Fortran (`fortran`) has no formatter. Fortran targets are tagged `no-format`, so `dx format` skips them. The `fortitude` linter is described under Fortran below. Typecheck is opt-in, see Fortran and D typechecking.

OCaml (`ocaml`) has no formatter and no linter. OCaml targets are tagged `no-format`, `no-lint` and `no-typecheck`, so `dx format`, `dx lint` and `dx typecheck` skip them.

## Tool overrides

The three commands run every wired tool from its managed executable. Pass
another target after `--` to select your own build of one tool. The override
keeps the tool ID, so findings read the same.

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:tool_ruff=//tools:my_ruff
```

- `ruff`, `terraform`, `hadolint`, `opa`, `regal`, `gofumpt`, `gotype`, `qmlformat`, `qmllint`,
  `julia_format`, `zig_ast_check`, `zig_fmt`, `dart`, `dfmt`, `dscanner`, `perlcritic`, `perltidy`, `erlfmt`, `elvis`, `dialyzer`,
  `luals`, `phpcs`, `phpstan`, `mix_format`, `credo`, `ormolu`
  and `nixfmt` have override flags.
- The `ruff`, `terraform`, `hadolint`, `opa`, `regal`, `ormolu` and `nixfmt` alternates
  must be a single executable file.
  Anything else fails the analysis with the tool named.
- The `gofumpt`, `gotype`, `qmlformat`, `qmllint`, `julia_format`, `zig_ast_check`, `zig_fmt`, `dart`, `dfmt`,
  `dscanner`, `perlcritic`, `perltidy`, `erlfmt`, `elvis`, `dialyzer`, `luals`, `phpcs`, `phpstan`, `mix_format` and `credo` alternates must be
  executable targets, such as
  `@dx_qt//:qmlformat`, `@dx_qt//:qmllint`, `@rules_dx//julia/tools:julia_format`,
  `@rules_dx//zig/tools:zig`, `@rules_dx//dart/tools:dart`, `@rules_dx//d/tools:dfmt`, `@rules_dx//d/tools:dscanner`,
  `@rules_dx//perl/tools:perlcritic`, `@rules_dx//perl/tools:perltidy`,
  `@rules_dx//erlang/tools:erlfmt`, `@rules_dx//erlang/tools:elvis`,
  `@rules_dx//erlang/tools:dialyzer`, `@rules_dx//lua/tools:luals`, `@rules_dx//php/tools:phpcs`, `@rules_dx//php/tools:phpstan`,
  `@rules_dx//elixir/tools:mix_format` and
  `@rules_dx//elixir/tools:credo`.
  A target without an executable fails the analysis.
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

### Julia formatting

`julia_format` runs JuliaFormatter 2.2.0 on the Julia version its Manifest was
resolved for. Select `julia_format` in a `julia` family. No flag is needed, and
the formatter packages are fetched only for Julia targets.

```python
quality_family(
    name = "julia_family",
    family_id = "julia",
    disabled = ["audit", "lint", "typecheck"],
    format = ["julia_format"],
)
```

A Julia-class target that is not one of the wrappers fails the analysis and
names the flag. The check never writes. `--apply` rewrites unformatted files. A
file with a syntax error reports an error at its line and column and stays
untouched. Use the wrappers from `@rules_dx//julia/rules:defs.bzl`.

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

### D formatting

`dfmt` uses the pinned dfmt that `d_library`, `d_binary` and `d_test` carry.
Select `dfmt` in a `d` family. No flag is needed. It runs on Linux x86_64
only.

```python
quality_family(
    name = "d_family",
    family_id = "d",
    disabled = ["audit", "lint", "typecheck"],
    format = ["dfmt"],
)
```

```sh
bazel run @rules_dx//:dx -- format
bazel run @rules_dx//:dx -- format --apply
```

To use your own dfmt, set `--@rules_dx//config:tool_dfmt=@rules_dx//d/tools:dfmt`.

dfmt has no check mode, so the check formats each file to standard output,
compares it with the file and never writes. `--apply` runs `dfmt --inplace`. A
file with a syntax error reports an error at its line and column and stays
untouched. dx runs dfmt with its default style and LF line endings and ignores
`.editorconfig` files for now.

### D linting

`dscanner` uses the pinned D-Scanner that `d_library`, `d_binary` and `d_test`
carry. Select `dscanner` in a `d` family. No flag is needed. It runs on Linux
x86_64 only.

```python
quality_family(
    name = "d_family",
    family_id = "d",
    disabled = ["audit", "format", "typecheck"],
    lint = ["dscanner"],
)
```

```sh
bazel run @rules_dx//:dx -- lint
```

To use your own D-Scanner, set
`--@rules_dx//config:tool_dscanner=@rules_dx//d/tools:dscanner`.

dx runs `dscanner --styleCheck` with the upstream default checks, so findings
include its documentation and style rules. It ignores `dscanner.ini` files for
now. A file with a syntax error reports an error at its line and column. There
is no `--apply` fix.

### Zig linting

`zig_ast_check` runs `zig ast-check` with the Zig compiler that `zig_library`,
`zig_binary` and `zig_test` carry. Select `zig_ast_check` in a `zig` family. No
flag is needed.

```python
quality_family(
    name = "zig_family",
    family_id = "zig",
    disabled = ["audit", "format", "typecheck"],
    lint = ["zig_ast_check"],
)
```

```sh
bazel run @rules_dx//:dx -- lint
```

To use your own compiler, set
`--@rules_dx//config:tool_zig_ast_check=@rules_dx//zig/tools:zig`.

`zig ast-check` takes one file per run, so dx starts one process per file. It
reports syntax errors and AST-level errors such as unused locals at their line
and column. Columns count UTF-8 bytes. Its `note:` lines are context and are not
findings. A failed run, such as an unreadable file, is an error and never reads
as clean. There is no `--apply` fix.

### Perl formatting and linting

`perltidy` uses the pinned Perl::Tidy 20260826 that `perl_library`,
`perl_binary` and `perl_test` carry. Select `perltidy` in a `perl` family. No
flag is needed. Typecheck stays deferred for Perl.

```python
quality_family(
    name = "perl_family",
    family_id = "perl",
    disabled = ["audit", "typecheck"],
    format = ["perltidy"],
    lint = ["perlcritic"],
)
```

To use your own perltidy, set
`--@rules_dx//config:tool_perltidy=@rules_dx//perl/tools:perltidy`.

The check formats each file to standard output, compares it with the file and
never writes. `--apply` runs `perltidy -b -bext=/`, so no backup or `.ERR` file
is left. A file perltidy cannot parse reports a syntax error and stays
untouched. dx runs perltidy with its default style and LF line endings and
always passes `--noprofile`, so `.perltidyrc` and `$PERLTIDY` are ignored.

`perlcritic` uses the pinned Perl::Critic 1.156 that the same wrappers carry,
and is check-only: it has no autofix. dx always passes `--noprofile`, so
`.perlcriticrc` and `$PERLCRITIC` are ignored, and it runs severity 5 policies
only. Findings carry the policy name as the rule, such as
`TestingAndDebugging::RequireUseStrict`, at the physical file and line.
To use your own perlcritic, set
`--@rules_dx//config:tool_perlcritic=@rules_dx//perl/tools:perlcritic`.


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

### Erlang linting

`elvis` has no default executable. Select `elvis` in an `erlang` family and
point the flag at the pinned Elvis 6.0.0 escript.

```starlark
quality_family(
    name = "erlang_family",
    family_id = "erlang",
    lint = ["elvis"],
)
```

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:tool_elvis=@rules_dx//erlang/tools:elvis
```

Without the flag, a policy that selects `elvis` fails the analysis and names
the flag. Each finding is a warning at its line and column, with the rule as
`elvis/<rule>`. Elvis never fixes, so `--apply` changes nothing. Elvis does not
report syntax errors; `erlfmt` does.

Without a bound config, dx checks every `.erl` file with the `erl_files`
ruleset and every `.hrl` file with the `hrl_files` ruleset. To change the
rules, bind an `elvis_config`:

```starlark
load("@rules_dx//quality:native_config.bzl", "elvis_config")

elvis_config(
    name = "elvis_config",
    src = "elvis.config",
)

erlang_app(
    name = "erlang_app",
    app_name = "math",
    aspect_hints = [":elvis_config"],
)
```

The `files` globs in a bound config match workspace-relative paths, such as
`**/src/*.erl`. A file no glob matches is not checked. A config group that
matches none of the checked files fails the run unless it sets
`allow_no_files => true`. Do not set `output_format` in the config.

### Erlang and Elixir type checking

`dialyzer` has no default executable. Select `dialyzer` in an `erlang` family
and point the flag at the pinned OTP dialyzer and its PLTs.

```starlark
quality_family(
    name = "erlang_family",
    family_id = "erlang",
    typecheck = ["dialyzer"],
)
```

```sh
bazel run @rules_dx//:dx -- typecheck -- --@rules_dx//config:tool_dialyzer=@rules_dx//erlang/tools:dialyzer
```

Without the flag, a policy that selects `dialyzer` fails the analysis and names
the flag. Dialyzer checks the compiled `.beam` files of an `erlang_app` against
the beams of its dependencies, so it sees the real compiler options, macros and
generated modules. Every `erlang_app` is compiled with `debug_info`, which
dialyzer needs. A dependency built without it fails the run and names the beam.

The base PLT covers `erts`, `kernel`, `stdlib` and the OTP applications listed
in `DIALYZER_PLT_CORE` and `DIALYZER_PLT_APPS` in `erlang/toolchain/pins.bzl`:
`compiler`, `crypto`, `syntax_tools`, `sasl`, `public_key`, `ssl`, `inets`,
`tools`, `runtime_tools` and `eunit`. Bazel builds it once per OTP version and
execution platform. A call into an OTP application outside that list is
reported as `dialyzer/unknown_function`. Each finding is a warning at its line
and column, with the rule as `dialyzer/<rule>`: `no_return`, `pattern_match`,
`invalid_contract`, `call`, `unmatched_return`, `unknown_function`,
`unknown_type`, or `dialyzer` for the rest. Only findings in the target's own
sources are reported. Dialyzer never fixes, so `--apply` changes nothing. To
silence a warning, use a `-dialyzer(...)` attribute in the source.

The same tool and flag check an `elixir` family. Select `dialyzer` in its
`typecheck` list. Dialyzer checks the compiled `.beam` files of each
`elixir_app`, which adds what the compiler does not: code against its `@spec`
contracts, and success typing. Each `elixir_app` brings its own Elixir PLT
(`//elixir/tools:elixir_plt`, the pinned Elixir `elixir` and `logger`
applications on top of the OTP core PLT) through `BeamInfo.plts`, after the
PLTs of the tool, and its `elixir` compiler beams for dialyzer's code path.
A custom `tool_dialyzer` must use the same OTP as the pin. A call into an
Elixir application outside `DIALYZER_PLT_ELIXIR` in `elixir/toolchain/pins.bzl`
is reported as `dialyzer/unknown_function`. Findings are named by the source
file the beam was built from, so two files with the same name in different
folders are reported at their own paths. To silence a warning, use
`@dialyzer {:nowarn_function, name: arity}` in the module.

```starlark
quality_family(
    name = "elixir_family",
    family_id = "elixir",
    typecheck = ["dialyzer"],
)
```

### PHP linting

`phpcs` has no default executable. Select `phpcs` in a `php` family and point
the flag at the pinned PHP_CodeSniffer 4.0.4 on the pinned PHP.

```starlark
quality_family(
    name = "php_family",
    family_id = "php",
    lint = ["phpcs"],
)
```

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:tool_phpcs=@rules_dx//php/tools:phpcs
```

Without the flag, a policy that selects `phpcs` fails the analysis and names
the flag. Without a config, phpcs checks the target's sources against the
bundled `PSR12` standard. Bind a `phpcs.xml`, `.phpcs.xml` or `phpcs.xml.dist`
ruleset with `phpcs_config` from `@rules_dx//quality:native_config.bzl` to
change that; the standard is always passed explicitly, so a stray ruleset in a
parent directory is never picked up. Only the standards bundled with phpcs
resolve: a ruleset that needs `installed_paths` or a third-party standard
fails. Each finding is at its line and column with the rule as
`phpcs/<sniff source>`, an error or a warning as phpcs reports it. phpcs fails
closed: an exit code outside 0 to 3, output that is not a report, a file it did
not check or an internal phpcs exception is an execution error, never a clean
run. phpcs does not fix here (`phpcbf` is not wired), so `--apply` changes
nothing. The platform is `linux_x86_64`, the only one with a pinned PHP.

### PHP type checking

`phpstan` has no default executable. Select `phpstan` in a `php` family and
point the flag at the pinned PHPStan 2.3.3 on the pinned PHP.

```starlark
quality_family(
    name = "php_family",
    family_id = "php",
    typecheck = ["phpstan"],
)
```

```sh
bazel run @rules_dx//:dx -- typecheck -- --@rules_dx//config:tool_phpstan=@rules_dx//php/tools:phpstan
```

Without the flag, a policy that selects `phpstan` fails the analysis and names
the flag. PHPStan analyses the target's own sources. The sources of its
`php_library` deps and the files of their Composer packages are only scanned
for the classes, functions and constants they declare, so a class from a
dependency is known without running any code. Nothing is autoloaded.

Without a config, PHPStan runs at level 5. Bind a `phpstan.neon` or
`phpstan.neon.dist` with `phpstan_config` from `@rules_dx//quality:native_config.bzl`
to change that. The config is included by a generated one that pins the cache
directory, the scan directories and a single process, so those keys cannot be
overridden. A consumer `bootstrapFiles` entry is run by PHPStan. Each finding is
an error at its line with the rule as `phpstan/<identifier>`, or `phpstan/error`
when PHPStan names none. Only findings in the target's own sources are
reported. PHPStan never fixes, so `--apply` changes nothing. The platform is
`linux_x86_64`, the only one with a pinned PHP.

### Lua type checking

`luals` has no default executable. Select `luals` in a `lua` family and point
the flag at the pinned LuaLS (lua-language-server) 3.19.2.

```starlark
quality_family(
    name = "lua_family",
    family_id = "lua",
    typecheck = ["luals"],
)
```

```sh
bazel run @rules_dx//:dx -- typecheck -- --@rules_dx//config:tool_luals=@rules_dx//lua/tools:luals
```

Without the flag, a policy that selects `luals` fails the analysis and names the
flag. LuaLS checks the target's own sources as Lua 5.4. The modules of its
`lua_library` deps are staged as a library by module name, so a `require` of a
dependency resolves; library files are never reported. Bind a `.luarc.json`
(never `.jsonc`) with `luals_config` from `@rules_dx//quality:native_config.bzl`
to set globals or diagnostics; the runtime version, the library, library
diagnostics and the log, meta and report paths are always set by dx. Each
finding has the rule `luals/<code>` at its line; severity 1 is an error, 2 a
warning and 3 or 4 info. LuaLS exits 0 with findings, so a missing report is an
execution error. LuaLS never fixes, so `--apply` changes nothing. Only
`linux_x86_64` is qualified, though archives for macOS, Linux arm64 and Windows
are pinned.

### Go typechecking

`gotype` typechecks every `go_library`, `go_binary` and `go_test`. A Bazel action
runs the `dx_gotype` driver, built on the standard library's `go/types`, once per
package. It reads the export data that rules_go compiled for the package's
dependencies and the standard library built for the same configuration, so the
build tags, GOOS, GOARCH and cgo setting match the real compile. A `go_test`
checks the internal and the external `_test` package together. Findings are
`syntax` and `typecheck` errors at their line and column in the target's own
sources. A package with no report, an import with no export data, or an error
only in generated code fails the run instead of passing it. `gotype` never
fixes, so `--apply` changes nothing. Dependencies are compiled to produce the
export data, so a type error in a dependency fails the build of its dependents
instead of being reported by them. Use `--@rules_dx//config:tool_gotype=<target>`
to replace the driver.

### Go linting

`govet` and `errcheck` lint every `go_library`, `go_binary` and `go_test`. They
are opt-in: no curated Go lint default is set. A Bazel action per tool runs the
`dx_govet` or `dx_errcheck` driver once per package, on the same inputs as
`gotype`, so the build tags, GOOS, GOARCH and cgo setting match the real
compile. The drivers run the vet and errcheck analyzers directly, so no
`go list`, module cache or network access is needed. A finding has the analyzer
name as its rule and sits at its line and column in the target's own sources.
An analyzer error, an import with no export data, or a missing report fails the
run instead of passing it. Neither tool fixes, so `--apply` changes nothing.
Facts do not cross packages yet, so analyzers that depend on them see each
package alone. Use `--@rules_dx//config:tool_govet=<target>` or
`--@rules_dx//config:tool_errcheck=<target>` to replace a driver; it must take
`-out FILE CONFIG` and write the dx unit JSON report. `staticcheck` still runs
the standalone binary.

### Java typechecking

`javac` typechecks every `java_library`, `java_binary` and `java_test`. A
Bazel action runs the JDK's javac up to flow analysis, without writing classes,
over the target's sources and `.srcjar` files. It uses the compile classpath,
boot classpath and javacopts that rules_java gives the target, minus the
ErrorProne and JavaBuilder-only flags that plain javac rejects, and runs the
target's annotation processors. The run only reports: a compiler error is a
`javac/compiler.err.*` error at its line and column in the target's own
sources, and a failed run with no error in those sources (for example an error
only in generated code) fails the run instead of passing it. `-Werror` is
honoured. `javac` never fixes, so `--apply` changes nothing. It adds a second
javac pass on top of the real compile, only under `dx typecheck` and
`dx check`; `bazel build` is unchanged. The classpath is the transitive compile
classpath, so strict-deps violations are not reported. Java sources inside
`kt_jvm_library` targets, Kotlin and Scala are not typechecked yet.

### Fortran and D typechecking

`flang` (Fortran) and `ldc` (D) are opt-in: select them with
`typecheck = ["flang"]` on a `fortran` family or `typecheck = ["ldc"]` on a `d`
family. Each one replays the compile action that the Fortran or D rules
configured, so the compiler, flags, include paths and module directories match
the real build. The replay only checks syntax and types. It writes no object
files, headers or module files into the build outputs. An error is reported at
its line and column in the target's own sources. A failed run with no error in
those sources fails the run instead of passing it. Both tools run on Linux
x86_64 only and never fix, so `--apply` changes nothing. Fortran modules that a
source takes from another source of the same target come from the real build:
a type error in the providing source fails `bazel build` instead of showing as
a finding.

### TypeScript typechecking

`tsc` typechecks every `typescript_project`. It runs the project's own
TypeScript (the `tsc` its `ts_project` uses) with its tsconfig, so `dx typecheck`
and `bazel build` agree on the version, the module resolution and the
`.d.ts` files of the dependencies. `skipLibCheck` follows
`--@aspect_rules_ts//ts:skipLibCheck`. The run only reports: a type error is a
`tsc/TS<code>` error at its line and column in the project's own sources, and
errors in dependencies or a failed run with no error in the project's sources
fail the run instead of passing it. `tsc` never fixes, so `--apply` changes
nothing. It adds a second `tsc --noEmit` run on top of `<name>_typecheck_test`,
but only under `dx typecheck`; `bazel build` is unchanged. `typescript_test`
sources and `javascript` (`checkJs`) are not typechecked yet.

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

### Elixir linting

`credo` has no default executable. Select `credo` in the `lint` list of an
`elixir` family and point the flag at the pinned Credo 1.7.19, which runs on the
pinned Elixir 1.20.4 and OTP.

```python
quality_family(
    name = "elixir_family",
    family_id = "elixir",
    disabled = ["audit", "format", "typecheck"],
    lint = ["credo"],
)
```

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:tool_credo=@rules_dx//elixir/tools:credo
```

- Without the flag, a policy that selects `credo` fails the analysis and names
  the flag.
- Credo reports every finding as a warning with the rule `credo/<Category.Check>`,
  for example `credo/Warning.IoInspect`. It never writes, so `--apply` leaves
  the sources as written.
- A file Credo cannot parse is dropped from Credo's own report. dx reports it as
  an error with the rule `credo/syntax` at the parser's line and column, so it
  never counts as clean.
- Without a bound config the run uses Credo's own non-strict default checks over
  every file Bazel owns. A `credo_config` rule from
  `@rules_dx//quality:native_config.bzl` binds a `.exs` config instead. The
  runner always passes `--config-file`, so a `.credo.exs` in a parent directory
  is never read.
- `requires` and `plugins` in a config are not supported: only the pinned Credo
  and its three dependencies are on the code path. HOME, Mix and Hex homes are
  locked down and nothing is fetched at run time.
- Consistency checks compare the files of one run. Findings can differ with how
  Bazel batches the files into runs.

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

`dx typecheck` runs the same `qmllint` invocation and keeps only the type
findings: `import`, `unresolved-type`, `missing-type`, `incompatible-type`,
`missing-property`, `read-only-property`, `required`, `restricted-type`,
`uncreatable-type`, `unresolved-alias`, `non-list-property`,
`enums-are-not-types`, `inheritance-cycle`, `alias-cycle`,
`missing-enum-entry` and `bad-signal-handler-parameters`. `dx lint` reports
everything else, including ids dx does not know. These ids no longer show up
in `dx lint`. Syntax errors and files `qmllint` cannot open appear in both.

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
linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 and windows_arm64.

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

### F# typecheck

`fsc` type-checks an F# target. It runs the pinned F# compiler a second time
with the target's sources in compile order, its dependency assemblies, its
`defines`, `langversion`, `nowarn` and `compiler_options`, so the diagnostics
match the build. Select it with `typecheck = ["fsc"]` in the `fsharp` family
and drop `typecheck` from `disabled`.

```sh
bazel run @rules_dx//:dx -- typecheck //app/...
```

Findings keep the `FS` code, line and column. A warning is reported as a
warning even though the build treats warnings as errors. An error with no
file, an error in a file the target does not own, or a failing compile with
no error diagnostic fails the run and never counts as clean. `fsc` is check
only and runs each F# target twice under `dx typecheck`, at heavy resources.
Supported on linux_x86_64.

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

The files in `srcs`, `non_arc_srcs` and `hdrs` of an `objc_library` are
checked. `objc_library` from `@rules_dx//cc/rules:defs.bzl` is the upstream
rule, so ownership comes from the upstream target, the same as for a plain
upstream `objc_library`. A check never writes. `--apply`
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

### FlatBuffers

`flatc` 25.12.19 lints `.fbs` schemas. It is built from the pinned source
archive (`@dx_flatbuffers`), not downloaded as a binary, and runs on Linux
x86_64. Select it in a `flatbuffers` family. The family is opt-in and part of
no curated default. Format is not applicable: no maintained `.fbs`
formatter exists, and `flatc` only validates.

```python
load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")

quality_family(
    name = "flatbuffers_family",
    family_id = "flatbuffers",
    lint = ["flatc"],
)

flatbuffers_library(
    name = "order",
    srcs = ["order.fbs"],
    deps = ["//item"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //api/...
```

Own schemas with `flatbuffers_library`. List the package files in `srcs` and
the libraries they include in `deps`. An include resolves only from the
`deps` closure, from a directory under the workspace root, or next to the
including file. A check parses each schema with `flatc -M --cpp`, which reads
and writes nothing, and reports every include flatc opened. An include that is
not a declared input fails with
`include '<path>' is outside the declared inputs of <schema>`, and an include
flatc cannot find is reported as `unable to load include file`. Each finding
is reported at its line and column. Warnings fail the lint. A syntax error, an
unknown type, an empty file or output the adapter cannot read fails the run
and never counts as clean. FlatBuffers has no maintained formatter, so format
is not applicable, and `--apply` has nothing to fix.

To run another `flatc`, point the flag at it:
`--@rules_dx//config:tool_flatc=//tools:flatc`. A target without an
executable fails with `set --@rules_dx//config:tool_flatc to a flatc binary`.

C++ code generation uses the pinned `flatc` through `flatbuffers_cc_library`
and does not read this flag. See the codegen pairs in
[generate](generate.md#codegen-pairs).

`flatc` lint and typecheck are supported on Linux x86_64 only. Linux arm64,
macOS arm64 and Windows x86_64 are marked unsupported until `flatc` is
qualified there.

#### Breaking-change check

`flatbuffers_conform_test` in `@rules_dx//flatbuffers/rules:conform.bzl` runs
`flatc --conform` of a `library` against a `baseline`. Both are
`flatbuffers_library` targets, and the baseline can be an in-repo snapshot or
an external previous release. A breaking change fails `bazel test` or
`bazel build` with flatc's message, for example
`schemas don't conform: field deleted: Shape.name`.

```python
load("@rules_dx//flatbuffers/rules:conform.bzl", "flatbuffers_conform_test")

flatbuffers_conform_test(
    name = "order_conform_test",
    baseline = "//api/v1:order",      # or "@schema_v1//:order"
    library = ":order",
)
```

`root` and `baseline_root` are required when a library owns more than one
schema. flatc stops at the first error and does not report deleted tables or
enums, and it skips renamed tables, so a clean result is not proof of
compatibility. flatc reports no line or column, so the failure is a build
failure and not a `dx lint` finding. The check runs on every platform because
it uses the pinned `flatc` built by Bazel.

### Cap'n Proto

`capnp` 1.5.0 lints `.capnp` schemas. It is built from the pinned source
archive (`@dx_capnproto`), not downloaded as a binary, and runs on Linux
x86_64. Select it in a `capnp` family. The family is opt-in and part of no
curated default. Format is not applicable: no maintained `.capnp` formatter
exists, and `capnp` only validates.

For C++ code generation from the same `capnp_library` targets see
[generate](generate.md#codegen-pairs).

```python
load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

quality_family(
    name = "capnp_family",
    family_id = "capnp",
    lint = ["capnp"],
)

capnp_library(
    name = "order",
    srcs = ["order.capnp"],
    deps = ["//item"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //api/...
```

Own schemas with `capnp_library`. List the package files in `srcs` and the
libraries they import in `deps`. An import resolves only from the `deps`
closure, relative to the importing file or from the workspace root
(`/pkg/x.capnp`), plus the schemas bundled with the compiler under `/capnp/`:
`/capnp/c++.capnp`, `/capnp/schema.capnp`, `/capnp/stream.capnp` and
`/capnp/compat/json.capnp`. A check runs
`capnp compile --no-standard-import -I <root> --src-prefix=<root> -o-` from a
scratch root that holds only those declared inputs, so an undeclared import is
reported as `Import failed: <path>`, and a `..` import above the root or an
absolute import outside the root cannot reach the host filesystem. The
generated request goes to stdout and is discarded, so a check never writes.
Each finding is reported at its line and column. A missing file ID, a syntax
error, an unknown type or output the adapter cannot read fails the run and
never counts as clean. Typecheck reports errors only. Cap'n Proto has no
formatter, so format is not applicable, and `--apply` has nothing to fix.
There is no breaking-change check.

To run another `capnp`, point the flag at it:
`--@rules_dx//config:tool_capnp=//tools:capnp`. A target without an
executable fails with `set --@rules_dx//config:tool_capnp to a capnp binary`.

`capnp` lint and typecheck are supported on Linux x86_64 only. Linux arm64,
macOS arm64 and Windows x86_64 are marked unsupported until `capnp` is
qualified there. Code generation for Cap'n Proto is not available yet.

### CUE

`cue fmt` (format) and `cue vet -c=false` (typecheck) run from the managed pin.
Select them in a `cue` family.
It runs on Linux x86_64 and arm64, macOS arm64 and Windows x86_64. Windows arm64 is not supported yet.

```python
quality_family(
    name = "cue_family",
    family_id = "cue",
    format = ["cue"],
    typecheck = ["cue"],
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

`dx typecheck` vets each package (one directory and package clause) from the
module root with `cue vet -c=false -E`, so schema-only packages pass and only
real conflicts, unresolved imports and syntax errors are reported. Each error
is reported at its first position in a file the target owns. Registry
dependencies are not supported (`CUE_REGISTRY=none`); imports must resolve
inside the module, so `cue.mod/module.cue` must be in `srcs` or `deps`.

### Rego

`opa fmt` and `opa check` run from the managed OPA pin, and `regal lint` runs
from the managed Regal 0.43.0 pin. Select `opa` and `regal` in a `rego` family.
They run on Linux x86_64.

```python
quality_family(
    name = "rego_family",
    family_id = "rego",
    format = ["opa"],
    lint = ["regal"],
    typecheck = ["opa"],
)
```

```sh
bazel run @rules_dx//:dx -- format //policy/...
bazel run @rules_dx//:dx -- format --apply //policy/...
bazel run @rules_dx//:dx -- typecheck //policy/...
bazel run @rules_dx//:dx -- lint //policy/...
```

Own Rego files with `rego_library` from `@rules_dx//rego/rules:defs.bzl`. List
the `.rego` files of the package in `srcs`, the `.json`, `.yaml` and `.yml` data
files in `data` and the libraries it imports in `deps`. Data files are not
checked. Format runs `opa fmt` on one file at a time. A check never writes.
`--apply` rewrites only the `.rego` files the selected targets own. An
unformatted file is reported at its first differing line. A file that does not
parse fails with the file and line. It never counts as clean.

Typecheck runs `opa check` once over the owned files and the `.rego` files of
their dependencies, so imports resolve. Only errors in owned files are
reported, with the OPA error code as the rule, the file, the line and the
column. A column counts characters from 1. Output that is not the expected JSON
fails the run. It never counts as clean.

Lint runs `regal lint` over the owned files only, so rules that look across
packages do not see dependencies. Regal rules become findings with the rule
`category/title`, such as `style/use-assignment-operator`. A Regal `error` is an
error and a `warning` is a warning. Both are reported. A policy that does not
parse fails the run and never counts as clean. Lint never writes: `--apply`
does not run `regal fix`, because it can move files.

Regal rules that look across the linted files, such as
`idiomatic/no-defined-entrypoint`, report without a file when a target has more
than one source. Those findings are anchored to the first owned file of the
batch. Turn such a rule off in the `regal_config` if it does not fit a library.

Regal never reads a `.regal` directory from the workspace, the home directory or
a parent directory, and it does not check for a newer version. To change the
rules, bind a `regal_config` to the target with `aspect_hints`. The config is a
Regal YAML file. `.regal/rules` custom rules are not loaded.

```python
load("@rules_dx//quality:native_config.bzl", "regal_config")

regal_config(
    name = "regal",
    src = ".regal/config.yaml",
)

rego_library(
    name = "policy",
    aspect_hints = [":regal"],
    srcs = ["policy.rego"],
)
```

`rego_test` runs `opa test` through `dx test`. It takes `srcs` for the
`_test.rego` files, `deps` for the `rego_library` targets they import, and
`data` for JSON and YAML files.

```python
rego_test(
    name = "policy_test",
    srcs = ["policy_test.rego"],
    deps = [":policy"],
)
```

```sh
dx test //policy/...
```

opa runs on all four supported platforms. Files are staged at their
repository-relative paths, so data namespaces match `opa test .` at the
repository root. Tests marked `todo_` and skipped tests fail unless
`args = ["--exit-zero-on-skipped"]` is set. A target with no tests fails.
`--test_filter` maps to `opa test --run`. `tool_opa` also applies to tests and
must name an executable for the target platform.

Only format fixes. Policies are Rego v1, the OPA 1.x default. A Rego v0 policy
fails to parse until it is migrated. To use another OPA or Regal executable,
point `tool_opa` or `tool_regal` at it.

```sh
bazel run @rules_dx//:dx -- typecheck -- --@rules_dx//config:tool_opa=//tools:opa
```

### Jsonnet

`jsonnetfmt` runs from the managed pin for format and `jsonnet_lint` runs
`jsonnet-lint` from the same go-jsonnet archive for lint. Select them in a
`jsonnet` family. They run on Linux x86_64.

```python
quality_family(
    name = "jsonnet_family",
    family_id = "jsonnet",
    format = ["jsonnetfmt"],
    lint = ["jsonnet_lint"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //config/...
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

`jsonnet_lint` reports unknown and unused variables, wrong call arguments,
missing object fields and unresolved imports with the file, line and column. It
has no fix, so `--apply` leaves the files alone. A run that exits with an error
or prints nothing it can place in a staged file fails and never counts as clean.

### Fortran

`fortitude` 0.10.0 runs from the managed pin for lint. Select it in a `fortran`
family. It runs on Linux x86_64, the only platform the Fortran rules build on.
It runs with `--isolated`, so a `fortitude.toml` or `fpm.toml` higher up the
tree never changes the result, and without `--force-exclude`, so a source under
a directory such as `build/` is still checked.

```python
quality_family(
    name = "fortran_family",
    family_id = "fortran",
    lint = ["fortitude"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //fortran/...
```

`fortitude` reads free-form source only. It reports a valid fixed-form `.f`
or `.F` file as a syntax error, so those files are not handed to it and are not
linted. A syntax error in a free-form file is reported as a finding with the
file, line and column. A run that exits with a code it does not recognise, an
exit that disagrees with its report, or a finding in a file that was not passed
fails and never counts as clean. `fortitude --fix` is not wired, so `--apply`
leaves the files alone. There is no Fortran formatter yet.

### Haskell

`ormolu` 0.9.0.0 runs from the managed pin for format. Select it in a `haskell`
family. It runs on Linux x86_64. The Windows x86_64 pin exists but is not
qualified. macOS and Linux arm64 have no pin, because upstream ships no
standalone binary for them. A run that formats Haskell there fails with a hint
to set `tool_ormolu`. Runs with no Haskell sources are not affected.

```python
quality_family(
    name = "haskell_family",
    family_id = "haskell",
    format = ["ormolu"],
)
```

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- format --apply //app/...
```

Own Haskell files with `haskell_library`, `haskell_binary` or `haskell_test`
from `@rules_dx//haskell/rules:defs.bzl`. A check never writes. `--apply`
rewrites only the files the selected targets own. An unformatted file is
reported at the start of the file. A parse error fails the run with the file,
line and column. It never counts as clean. Each file runs in its own `ormolu`
process. `ormolu` runs with `--no-cabal --no-dot-ormolu`, so it reads no
`.cabal` or `.ormolu` file, and language extensions must be enabled with
`LANGUAGE` pragmas in the source. Extensions set only in `compiler_flags` cause
a parse error.

To use another ormolu executable, point `tool_ormolu` at it.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_ormolu=//tools:ormolu
```

`hlint` 3.10 runs from the managed pin for lint. Select it in the same `haskell`
family. It runs on Linux x86_64 and needs `libgmp.so.10` and glibc 2.29 or newer
on the host. Without the library the run fails with a message that names it.
macOS and Linux arm64 have no pin, so a run that lints Haskell there fails with
a hint to set `tool_hlint`. Runs with no Haskell sources are not affected.

```python
quality_family(
    name = "haskell_family",
    family_id = "haskell",
    format = ["ormolu"],
    lint = ["hlint"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //app/...
```

`hlint` runs with the built-in hints only. It reads no `.hlint.yaml`, whether in
the package or in a parent folder. A parse error is reported as a finding with
rule `Parse error`. Hints map to severities as `Error`, `Warning` and
`Suggestion`, which is reported as info. Exit 0 with an empty report is clean,
and exit 1 with a non-empty report carries findings. Any other exit, a report
that disagrees with the exit, a missing or undecodable file, or a finding in a
file that was not passed fails and never counts as clean. `hlint --refactor` is
not wired, so `--apply` leaves the files alone. Columns count Unicode scalars
with a tab advancing to the next multiple of 8.

To use another hlint executable, point `tool_hlint` at it.

```sh
bazel run @rules_dx//:dx -- lint -- --@rules_dx//config:tool_hlint=//tools:hlint
```

### Typst

`typstyle` runs from the managed pin for format. Select it in a `typst` family.
It runs on Linux x86_64, Linux arm64, macOS arm64 and Windows x86_64.

```python
quality_family(
    name = "typst_family",
    family_id = "typst",
    format = ["typstyle"],
)
```

```sh
bazel run @rules_dx//:dx -- format //docs/...
bazel run @rules_dx//:dx -- format --apply //docs/...
```

Own Typst files with `typst_library` from `@rules_dx//typst/rules:defs.bzl`.
List the `.typ` files of the package in `srcs` and the libraries it imports in
`deps`. A check never writes. `--apply` rewrites only the files the selected
targets own. An unformatted file is reported at the start of the file. A
syntax error fails the run with the file named. `typstyle` exits 0 on a syntax
error, so dx reads its warning and never counts that file as clean. `typstyle`
has no config file, so it runs with its upstream defaults.

### reStructuredText

Vale lints `.rst` and `.rest` files. Vale converts them with `rst2html`, which dx
supplies from a pinned docutils. Own the files with `rst_library` from
`@rules_dx//rst/rules:defs.bzl`; the rule hands `rst2html` to the Vale run, so
the core shard needs no Python. Curated defaults do not select Vale for rst, so
select it in an `rst` family.

```python
quality_family(
    name = "rst_family",
    family_id = "rst",
    lint = ["vale"],
)
```

List the `.rst` files of the package in `srcs`. An rst file that is not owned by
`rst_library` fails the run with a message naming the rule. A converter failure
is reported and never counts as clean.

### AsciiDoc

Vale lints `.adoc`, `.asciidoc` and `.asc` files. Vale converts them with
`asciidoctor`, which dx supplies from a pinned Asciidoctor.js 3.0.4 run on the
hermetic Node toolchain, so no Ruby or host install is needed. Own the files with
`asciidoc_library` from `@rules_dx//asciidoc/rules:defs.bzl`; the rule hands
`asciidoctor` to the Vale run. Curated defaults do not select Vale for asciidoc,
so select it in an `asciidoc` family.

```python
quality_family(
    name = "asciidoc_family",
    family_id = "asciidoc",
    lint = ["vale"],
)
```

Vale is check only, and the converter starts one Node process per file. An
AsciiDoc file that is not owned by `asciidoc_library` fails the run with a
message naming the rule. A converter failure is reported and never counts as
clean.

### LaTeX

`latexindent` 4.0.3 runs from the managed pin for format. Select it in a
`latex` family. It runs on Linux x86_64, Linux arm64, macOS arm64 and Windows
x86_64.

```python
quality_family(
    name = "latex_family",
    family_id = "latex",
    format = ["latexindent"],
    disabled = ["lint", "typecheck"],
)
```

```sh
bazel run @rules_dx//:dx -- format //docs/...
bazel run @rules_dx//:dx -- format --apply //docs/...
```

Own LaTeX files with `latex_library` from `@rules_dx//latex/rules:defs.bzl`.
List the `.tex`, `.sty` and `.cls` files of the package in `srcs`. A check
never writes. `--apply` rewrites only the files the selected targets own.
`latexindent` takes one document per process, so dx runs it once per file. Its
log and backup files go to a scratch directory and never next to the sources.
An unformatted file is reported at the start of the file. A fatal error fails
the run with the file named.

`latexindent` reads a config only when one is bound. Bind a YAML settings file
with `latexindent_config` from `@rules_dx//quality:native_config.bzl`; dx
passes it with `-l`. A `localSettings.yaml` next to the sources is ignored.
`latexindent` itself skips a malformed or missing settings file without an
error, so check a new config by formatting a file it should change.

`chktex` lint is not available yet. No pinned build of `chktex` exists on any
platform, so a policy that selects it fails with:

```text
policy: family 'latex' selects lint tool 'chktex', which has no pinned build on linux_arm64, linux_x86_64, macos_arm64, windows_arm64, windows_x86_64; disable 'lint'
```

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

### Lua

`stylua` runs from the managed pin for format. Select it in a `lua` family.
It runs on Linux x86_64.

```python
quality_family(
    name = "lua_family",
    family_id = "lua",
    format = ["stylua"],
)
```

```sh
bazel run @rules_dx//:dx -- format //lua/...
bazel run @rules_dx//:dx -- format --apply //lua/...
```

Own Lua files with the rules from `@rules_dx//lua/rules:defs.bzl`. A check never
writes. `--apply` rewrites only the files the selected targets own. An
unformatted file is reported at its first differing line. A syntax error fails
the run with the file, line and column. It never counts as clean. StyLua ignores
`stylua.toml`, `.stylua.toml` and `.editorconfig` files next to the sources.
Bind a `stylua_config` from `@rules_dx//quality:native_config.bzl` to change
indent width, quote style or line width.

No Lua linter runs yet, so the Lua rules keep the `no-lint` tag. Lua targets
are type checked; see Lua type checking below.

To use another StyLua executable, point `tool_stylua` at it.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_stylua=//tools:stylua
```

### Terraform

`terraform fmt` runs from the managed pin for format. Select it in a
`terraform` family. It runs on Linux x86_64 and arm64, macOS arm64 and Windows x86_64. It never runs `init`, reads state
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
findings keep the `terraform` tool ID. The OpenTofu override is checked on
Linux x86_64 only.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_terraform=@tofu//:tofu
```

#### Terraform lint with tflint

`tflint` lints Terraform and runs from the managed pin (v0.65.0) on Linux
x86_64 and arm64, macOS arm64 and Windows x86_64. It is opt-in and check only:
select it with `lint = ["tflint"]` in the `terraform` family. Nothing selects it
by default, because the recommended preset reports a missing `required_version`
and missing `required_providers` in existing modules.

```python
quality_family(
    name = "terraform_family",
    family_id = "terraform",
    format = ["terraform"],
    lint = ["tflint"],
)
```

Only the bundled `terraform` ruleset runs, with the `recommended` preset. dx
never runs `tflint --init`, never downloads a provider ruleset and points
`TFLINT_PLUGIN_DIR` at an empty directory, so the aws, google and azurerm
rulesets are not available. Module calls are off (`call_module_type = "none"`),
so tflint reads only the module directory it is linting. It runs once per module
directory over the files the selected targets own.

dx always passes `--config`. Bind a `.tflint.hcl` with `tflint_config` from
`@rules_dx//quality:native_config.bzl`; without one, dx stages a default with the
bundled ruleset and the recommended preset. `./.tflint.hcl`, `~/.tflint.hcl` and
`TFLINT_CONFIG_FILE` are never read. A config that declares a plugin block with
`source` and `version` needs `--init`; that fails the run with tflint's message
and never counts as clean. A syntax error is reported on its file. `.tftest.hcl`
files are not linted.

```python
load("@rules_dx//quality:native_config.bzl", "tflint_config")

tflint_config(
    name = "tflint",
    src = "tflint.hcl",
)

terraform_module(
    name = "infra",
    aspect_hints = [":tflint"],
    srcs = ["main.tf"],
)
```

### Bicep

Own Bicep files with `bicep_module` from `@rules_dx//bicep/rules:defs.bzl`. It
accepts `.bicep` and `.bicepparam` files in `srcs`, other Bicep modules in
`deps`, and files read by Bicep `load` functions in `data`. Files in `data` are
never checked, and a `bicepconfig.json` there is refused: bind the configuration
with `bicep_config` instead. The pinned Bicep CLI v0.48.1 is available for all
five platforms through the managed tool hub; formatting, linting and compiling
with it are not wired into `dx format` or `dx lint` yet, so a `bicep_module`
target is owned but not yet checked.

```python
load("@rules_dx//bicep/rules:defs.bzl", "bicep_module")

bicep_module(
    name = "network",
    srcs = ["network.bicep", "network.bicepparam"],
    deps = ["//infra/common:modules"],
)
```

### CloudFormation

`cfn-lint` validates AWS CloudFormation templates against the bundled resource
schemas. It runs from a managed v1.58 pin on Linux x86_64, Linux arm64, macOS
arm64 and Windows x86_64, on the Python 3.12 that the Python tools share. It is
check-only: `lint --apply` never rewrites a template. The `cloudformation`
family lints with `cfn_lint` by default, so a `cloudformation_template` target
starts being linted once it is in the workspace. Format and typecheck stay
disabled.

Own templates with `cloudformation_template` from
`@rules_dx//cloudformation/rules:defs.bzl`. It accepts `.yaml`, `.yml`, `.json`
and `.template` files. A file is checked as a template because the rule owns
it, not because of its name. Plain YAML stays in the `yaml` family.

```python
load("@rules_dx//cloudformation/rules:defs.bzl", "cloudformation_template")

cloudformation_template(
    name = "stack",
    srcs = ["stack.yaml"],
)
```

Before cfn-lint runs, dx checks each owned file and reports these rules
without launching the tool for that file:

| Rule | Meaning |
| --- | --- |
| `dx/cloudformation-syntax` | The file is not valid YAML or JSON, is not UTF-8, or is larger than 2 MiB. |
| `dx/cloudformation-root` | The file has neither a root `AWSTemplateFormatVersion` nor a `Resources` entry with an `AWS::` type. |
| `dx/cloudformation-config` | The template's own `Metadata` sets a cfn-lint option that dx does not allow. |

cfn-lint findings keep their rule ID (for example `E3002`), and the line and
column of the offending value. Errors, warnings and informational findings map
to the dx error, warning and info severities. A nonzero exit with no finding,
an exit code that does not match the reported levels, and unreadable output are
errors and never count as clean.

To change rules, bind a `cfn_lint_config` (a cfn-lint config file) with
`aspect_hints`. dx always passes `--config-file`, so a `.cfnlintrc` in the
repository, the working directory or the home directory is never read. A
config, or a template's `Metadata`, may set only `regions`, `ignore_checks`,
`include_checks`, `mandatory_checks`, `configure_rules` and
`include_experimental`. Any other key is refused, because it could load code or
read other files.

```python
load("@rules_dx//quality:native_config.bzl", "cfn_lint_config")

cfn_lint_config(
    name = "relaxed",
    src = ".cfnlintrc",
    visibility = ["//visibility:public"],
)

cloudformation_template(
    name = "stack",
    aspect_hints = [":relaxed"],
    srcs = ["stack.yaml"],
)
```

dx never fetches resource specs, never passes `--update-specs` or
`--regions`, uses no AWS credentials, never deploys, and has no `--apply` for
CloudFormation. `AWS_REGION` and `AWS_DEFAULT_REGION` are not passed to the
tool; set `regions` in the config to change the default `us-east-1`.

```sh
bazel run @rules_dx//:dx -- lint //infra/...
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
`:global`, `:local`, `:external`, `:export`, `:import`, `composes` and
`compose-with`. SCSS and Less files parse with `postcss-scss` and
`postcss-less`.

A syntax error, an invalid config or a tool that fails to run fails the run
with the cause named. It never counts as clean. To change the rules, bind a
`stylelint_config` to the library with `aspect_hints`. The file is a JSON
config. It replaces the default rules, so add an `overrides` entry with
`customSyntax` for `*.scss` and `*.less`.

### HTML, GraphQL, JSON5 and JSONC

`prettier` runs from the managed pin for format. It runs on Linux x86_64 and arm64, macOS arm64 and Windows x86_64. Windows arm64 is not supported yet. Select
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

A `prettier_config` may come from another module, including one bound with a
`repo_name`; reference it as `@<name>//:target`. dx stages it under
`_external/<canonical repo>/...`, and that path is what messages show. `overrides`
patterns containing `/` match from that folder and do not reach your sources, so
use file-name patterns such as `*.graphql` in shared configs. `data` imports must
live in the same repo as the config.

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
`.toml`, `.mjs`, `.cjs` or `.js`. A bound config replaces dx's default options;
it does not extend them. Those defaults are the `.sql` options (`sql-formatter`,
`keywordCase: "preserve"`, and the dialect picked from names such as
`*.postgresql.sql` or `*.mysql.sql`), the XML options
(`xmlWhitespaceSensitivity: "strict"`, `xmlQuoteAttributes: "preserve"`,
`xmlSortAttributesByKey: false`) and the Gherkin options
(`escapeBackslashes: false`, `forceNewlineBetweenStepBlocks: false`). Copy the
ones you still want into your config.

`overrides` globs match from the config's directory. `.editorconfig` and
`.prettierignore` are never read. A `plugins` entry or a shared npm config
fails the run; dx loads the plugins it pins by file extension. A `.mjs`, `.cjs`
or `.js` config can import only the files listed in `data`. An unknown parser
or an invalid config fails the run and never counts as clean.
`requirePragma` still skips files without a pragma.

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

#### Building and checking a Vue app

`vue_build` and `vue_check` are Bazel targets. They run `vite`,
`@vitejs/plugin-vue`, `vue-tsc` and `typescript` from your own `node_modules`.
`dx check` does not run them.

```python
load("@rules_dx//javascript/rules:web.bzl", "javascript_web_server")
load("@rules_dx//vue/rules:defs.bzl", "vue_build", "vue_check")

vue_build(
    name = "site",
    srcs = glob(["src/**"]) + ["index.html"],
    config = "vite.config.mjs",
    node_modules = "//:node_modules",
    package_json = "package.json",
    public = glob(["public/**"]),
    ssr_entry = "src/entry-server.ts",
    deps = ["//:node_modules/pinia"],
)

vue_check(
    name = "check",
    srcs = glob(["src/**"]),
    node_modules = "//:node_modules",
    package_json = "package.json",
    tsconfig = "tsconfig.json",
    deps = ["//:node_modules/pinia"],
)

javascript_web_server(
    name = "serve",
    app = ":site",
)
```

```sh
bazel build //web:site //web:site_ssr
bazel test //web:check
bazel run //web:serve
```

`vue_build` runs a Vite client build into a `site/` tree. The build fails when
the tree has no `index.html`. `javascript_web_server` serves it. With
`ssr_entry`, a second Vite SSR build writes `site_ssr/`. The SSR bundle keeps
`vue` and other packages as imports, so code that loads it needs them at run
time.

`vue_check` runs `vue-tsc --noEmit -p <tsconfig>` once into
`check.vue_check.txt`. The file starts with `exit_code: <n>` and holds the
diagnostics, for example
`src/App.vue(6,13): error TS2322: Type 'string' is not assignable to type 'number'.`
The test fails and prints the report when the exit code is not 0. A crash or a
missing package fails the build. It never counts as clean.

Tested with `vue` 3.5.42, `vite` 8.3.5, `@vitejs/plugin-vue` 6.0.10, `vue-tsc`
3.3.13 and `typescript` 6.0.3.

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
with Redocly CLI 2.71.1. It runs on Linux x86_64 and arm64, macOS arm64 and
Windows x86_64. Windows arm64 is not supported yet. It is opt-in: select it in
the `api_spec` family.

```python
quality_family(
    name = "api_spec_family",
    family_id = "api_spec",
    lint = ["redocly"],
    format = ["prettier"],
)
```

Declare specs with `api_spec_library`. `srcs` are the roots that get linted.
`refs` are files the roots reach through `$ref`. `deps` are other
`api_spec_library` targets whose files a root may refer to. Each accepts
`.yaml`, `.yml` and `.json`.

Format is opt-in with `format = ["prettier"]` on the `api_spec` family.
Prettier formats both `srcs` and `refs`; Redocly lints only `srcs`. Files owned
by `api_spec_library` should not also sit in a `yaml_library`.

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

#### Code generation

`openapi_typescript_client(spec = ":pets")` from
`@rules_dx//api_spec/rules:codegen.bzl` generates one self-contained TypeScript
client from an OpenAPI root of the library, and `openapi_codegen_shard` binds it
into the `dx codegen` plan. The same preflight checks run first, and the root
must be OpenAPI. See `docs/cli/commands/generate.md` for the fixed flags and the
verification with `dx codegen --check`.

### Svelte

`svelte_library` from `@rules_dx//svelte/rules:defs.bzl` owns `.svelte` files.
The default policy formats them with `prettier` and `prettier-plugin-svelte`
4.1.1 with `svelte` 5.57.0 from the managed pin.

```sh
bazel run @rules_dx//:dx -- format //web/...
bazel run @rules_dx//:dx -- format --apply //web/...
```

A check never writes. `--apply` rewrites only the files the selected targets
own. The script, markup and style blocks are all formatted. An unformatted file
is reported at the start of the file. A syntax error fails the run and never
counts as clean. dx places it at the line and column in the original file by
matching the compiler's code frame against the file. When the frame matches no
single place, the error is reported at line 1, column 1. `.prettierrc`,
`.editorconfig` and other user config files are ignored. `dx check` does not run
`svelte-check` or the Svelte compiler. See "Building and checking a Svelte app"
below for the Bazel targets that do.

`eslint` lint is opt-in. Select it in the `svelte` family and bind an
`eslint_config` to the library with `aspect_hints`.

```python
quality_family(
    name = "svelte_family",
    disabled = ["audit", "typecheck"],
    family_id = "svelte",
    format = ["prettier"],
    lint = ["eslint"],
)

eslint_config(
    name = "eslint",
    src = "eslint.config.mjs",
)

svelte_library(
    name = "hello",
    srcs = ["Hello.svelte"],
    aspect_hints = [":eslint"],
)
```

The config may import `eslint-plugin-svelte` 3.23.1, `svelte-eslint-parser`
1.10.0 and `@typescript-eslint/parser` 8.72.1 by name. dx provides them. Any
other package must resolve from your workspace.

```js
import svelte from "eslint-plugin-svelte";

export default [
  ...svelte.configs.recommended,
  {
    files: ["**/*.svelte"],
    languageOptions: {
      parserOptions: { parser: "@typescript-eslint/parser" },
    },
  },
];
```

Findings point at the line and column in the `.svelte` file, including findings
in `<script lang="ts">` blocks. A `.svelte` file that no config entry matches
fails the run. It never counts as clean.

#### Building and checking a Svelte app

`svelte_build` and `svelte_check` are Bazel targets. They run `vite`,
`@sveltejs/vite-plugin-svelte`, `svelte-check` and `typescript` from your own `node_modules`.
`dx check` does not run them.

```python
load("@rules_dx//javascript/rules:web.bzl", "javascript_web_server")
load("@rules_dx//svelte/rules:defs.bzl", "svelte_build", "svelte_check")

svelte_build(
    name = "site",
    srcs = glob(["src/**"]) + ["index.html"],
    config = "vite.config.mjs",
    node_modules = "//:node_modules",
    package_json = "package.json",
    public = glob(["public/**"]),
    ssr_entry = "src/entry-server.ts",
    deps = ["//:node_modules/clsx"],
)

svelte_check(
    name = "check",
    srcs = glob(["src/**"]),
    node_modules = "//:node_modules",
    package_json = "package.json",
    tsconfig = "tsconfig.json",
    deps = ["//:node_modules/clsx"],
)

javascript_web_server(
    name = "serve",
    app = ":site",
)
```

```sh
bazel build //web:site //web:site_ssr
bazel test //web:check
bazel run //web:serve
```

`svelte_build` runs a Vite client build into a `site/` tree. The build fails when
the tree has no `index.html`. `javascript_web_server` serves it. With
`ssr_entry`, a second Vite SSR build writes `site_ssr/`. The SSR bundle keeps
`svelte` and other packages as imports, so code that loads it needs them at run
time.

`svelte_check` runs `svelte-check --tsconfig <tsconfig> --output machine` once into
`check.svelte_check.txt`. The file starts with `exit_code: <n>` and holds the
machine-format diagnostics without timestamps, for example
`ERROR "src/App.svelte" 5:11 "Type 'string' is not assignable to type 'number'."`
The test fails and prints the report when the exit code is not 0. svelte-check exits 1 on both diagnostics and a crash, so the
report is written only when the output has a `COMPLETED` summary line. A crash or a
missing package fails the build. It never counts as clean.

Tested with `svelte` 5.57.0, `vite` 8.3.5, `@sveltejs/vite-plugin-svelte` 7.3.1, `svelte-check`
4.7.7 and `typescript` 6.0.3.

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

#### Building and checking an MDX app

`mdx_build` and `mdx_check` are Bazel targets. They run `vite`,
`@mdx-js/rollup` and `@mdx-js/mdx` from your own `node_modules`. `dx check`
does not run them. `mdx_library` does not compile.

```python
load("@rules_dx//javascript/rules:web.bzl", "javascript_web_server")
load("@rules_dx//mdx/rules:defs.bzl", "mdx_build", "mdx_check")

mdx_build(
    name = "site",
    srcs = glob(["src/**"]) + ["index.html"],
    config = "vite.config.mjs",
    node_modules = "//:node_modules",
    package_json = "package.json",
    ssr_entry = "src/entry-server.jsx",
    deps = ["//:node_modules/preact"],
)

mdx_check(
    name = "check",
    srcs = glob(["src/**/*.mdx"]),
    node_modules = "//:node_modules",
    package_json = "package.json",
)

javascript_web_server(
    name = "serve",
    app = ":site",
)
```

```js
import mdx from "@mdx-js/rollup";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [{ enforce: "pre", ...mdx({ jsxImportSource: "preact" }) }],
  oxc: { jsx: { runtime: "automatic", importSource: "preact" } },
});
```

```sh
bazel build //web:site //web:site_ssr
bazel test //web:check
bazel run //web:serve
```

`mdx_build` runs a Vite client build into a `site/` tree. The build fails when
the tree has no `index.html`. With `ssr_entry`, a second Vite SSR build writes
`site_ssr/`. The SSR bundle keeps `preact` and other packages as imports, so
code that loads it needs them at run time. A compile error fails the build with
`<package>/<file>:<line>:<column>: <reason>`.

`mdx_check` compiles each `.mdx` file in `srcs` with `@mdx-js/mdx` into
`check.mdx_check.txt`. It keeps going after a broken file. The file starts with
`exit_code: <n>` (0 clean, 1 errors) and has one line per message, for example
`` web/src/Broken.mdx:5:1: Expected a closing tag for `<div>` (5:1-5:6) [mdast-util-mdx-jsx:end-tag-mismatch] ``.
The test fails and prints the report when the exit code is not 0. A crash or a
missing package fails the build. It never counts as clean. Tested with `vite`
8.3.5, `@mdx-js/rollup` 3.1.1, `@mdx-js/mdx` 3.1.1 and `preact` 10.29.8.

### SQL, XML and Gherkin

`prettier` runs from the managed pin with the `prettier-plugin-sql`,
`@prettier/plugin-xml` and `prettier-plugin-gherkin` plugins for format. It runs on Linux x86_64 and arm64, macOS arm64 and Windows x86_64. Windows arm64 is not supported yet.
Select it in the `sql`, `xml` and `gherkin` families.

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
`trino` and `tsql`. To format every file of a `sql_library` as one dialect,
set `dialect = "postgresql"` (any name above). The attribute wins over a bound
`prettier_config` `language`. A file whose suffix names a different dialect is an
analysis error. Without the attribute, the suffix decides. The pinned options
keep SQL keywords in the case you wrote them. XML namespaces, prefixes, attribute quotes, attribute order and
text stay as written. Gherkin keeps its step text and tags. `.prettierrc`,
`.editorconfig` and other user config files are ignored.

### Gherkin lint

`gplint` 2.6.0 runs from the managed pin for lint in the `gherkin` family. It
runs on Linux x86_64 and never writes. Prettier owns layout, so the default
rules cover meaning only: unnamed features and scenarios, empty files and
backgrounds, outlines without examples, unused variables, duplicate, superfluous
and homogenous tags, and scenario names repeated inside one feature.

```python
quality_family(
    name = "gherkin_family",
    family_id = "gherkin",
    format = ["prettier"],
    lint = ["gplint"],
)
```

A syntax error is reported as a finding on its line. A crash, a missing report
or an invalid config fails the run with the cause named. It never counts as
clean. To change the rules, bind a `gplint_config` to the library with
`aspect_hints`. The file is a `.gplintrc` or `.json` rule map and replaces the
default rules. Rules that compare files, such as `no-dupe-feature-names`, give
results that depend on which files share a batch, so avoid them.

### SQL lint and format

`sqlfluff` 4.4.0 runs from the managed pin for lint (check only) and format. It runs on
Linux x86_64 and arm64, macOS arm64 and Windows x86_64. Select it in the `sql`
family. It is not a default: a `sql_library` without a `sqlfluff` config keeps
building as before.

```python
load("@rules_dx//quality:native_config.bzl", "sqlfluff_config")

sqlfluff_config(
    name = "sqlfluff_config",
    src = ".sqlfluff",
)

quality_family(
    name = "sql_family",
    disabled = ["typecheck"],
    family_id = "sql",
    format = ["sqlfluff"],
    lint = ["sqlfluff"],
)

sql_library(
    name = "schema",
    srcs = ["schema.sql"],
    aspect_hints = [":sqlfluff_config"],
)
```

A config is required. A target with the `sqlfluff` stage and no bound
`sqlfluff_config` fails and names the missing config; tag it `no-lint` to skip
it. The config is the only source of the dialect and must set `dialect` under
`[sqlfluff]`. dx passes the file with `--config` and `--ignore-local-config`,
so no `.sqlfluff`, `setup.cfg`, `tox.ini`, `pyproject.toml` or home directory
config is read, and `.sqlfluffignore` files are disregarded.

The dialect names are sqlfluff's, not the `sql_library` ones. Use `postgres`
for `postgresql`, `sparksql` for `spark`, `tsql` for `transactsql` and `oracle`
for `plsql`. `db2i`, `n1ql` and `singlestoredb` have no sqlfluff dialect;
`mysql` or `ansi` are the closest. dx does not check the config dialect against
the `dialect` of the `sql_library` or the file suffix. A dialect sqlfluff 4.4.0
does not list fails the run.

Only the `raw`, `jinja` and `placeholder` templaters run. `dbt` and `python`
templaters, the `templater:dbt` and `templater:python` sections, and the
`library_path`, `load_macros_from_path` and `loader_search_path` keys are
refused, because they load project code or files the target does not own.
Macros declared inline under `[sqlfluff:templater:jinja:macros]` work.

Findings keep their sqlfluff rule code (for example `LT01`). A file sqlfluff
cannot parse or template is reported as an error finding (`PRS` or `TMP`) and
never counts as clean. Rule warnings from `warnings = ...` stay warnings.

sqlfluff skips a file larger than `large_file_skip_byte_limit` (20 KB by
default) without reporting it. dx treats a file missing from the report as a
failure, so set `large_file_skip_byte_limit = 0` in the config when a target
owns larger files.

Format is opt-in too: put `sqlfluff` in the family `format` list instead of
`prettier`. Do not list both, because they would disagree about whitespace. The
format check runs `sqlfluff lint` limited to the rules `sqlfluff format`
applies (`capitalisation`, `layout`, `ambiguous.union`, `convention.not_equal`,
`convention.coalesce`, `convention.select_trailing_comma`,
`convention.is_null`, `jinja.padding` and `structure.distinct`), and `--apply`
runs `sqlfluff format`. Both need the same bound config and go through the same
preflight. Lint stays check only: `sqlfluff fix` is not wired.

### PowerShell

`psscriptanalyzer` runs from the managed PSScriptAnalyzer 1.25.0 pin for format
and lint.
It runs on Linux x86_64. Select it in the `powershell` family.

```python
quality_family(
    name = "powershell_family",
    family_id = "powershell",
    format = ["psscriptanalyzer"],
    lint = ["psscriptanalyzer"],
)
```

```sh
bazel run @rules_dx//:dx -- format //scripts/...
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

Format runs `Invoke-Formatter` and is fixable: `dx format` reports each script
that would change, and `dx fix` rewrites it. A script that does not parse, or
is not UTF-8, is reported as an error at the parse failure and is never
rewritten, so `dx fix` fails on it. A rewrite keeps the script's UTF-8 BOM, or
its lack of one, and its line endings: a script whose first line ends in CRLF
keeps CRLF on every line. Scripts that are already formatted are not touched.

Without a bound `psscriptanalyzer_config`, format uses the upstream
`CodeFormatting` preset, as `Invoke-Formatter` does by default. A bound `.psd1`
is passed as is, and `Invoke-Formatter` runs only the formatter rules it
configures under `Rules`. A config that configures none of them (for example a
lint-only `IncludeRules` list) fails the format run with the cause named,
instead of formatting nothing. Add a formatter rule such as `PSPlaceOpenBrace`
under `Rules`, or unbind the config. Lint and format share the one config, so
a formatter rule enabled under `Rules` is also reported by lint. A config that
names formatter rules only with `Enable = $false` passes this check and
formats nothing.

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
bazel_dep(name = "rules_ruby", version = "0.29.0")

ruby = use_extension("@rules_ruby//ruby:extensions.bzl", "ruby")
ruby.toolchain(
    name = "ruby",
    portable_ruby = True,
    portable_ruby_checksums = {...},
    version = "3.4.12",
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
x86_64. Select them in a `yaml` family. Only `yamlfmt` has fixes. After one
warm run, both pins come from the Bazel repository cache, so `format` and `lint`
pass with tool downloads blocked.

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

### Ansible

`ansible-lint` lints Ansible playbooks, roles and tasks. It is check-only and
opt-in: add `ansible_lint` to an `ansible` family. It runs from a managed pin on
Linux x86_64, Linux arm64 and macOS arm64. Upstream has no Windows build, so
`ansible_lint` fails on Windows with `ansible_lint: no upstream Windows build
(windows_x86_64 is unsupported); remove ansible_lint from this family on
Windows`.

```python
quality_family(
    name = "ansible_family",
    family_id = "ansible",
    lint = ["ansible_lint"],
)
```

Format is opt-in with `format = ["prettier"]` on the `ansible` family. Prettier
formats the YAML files and leaves Jinja expressions untouched. Files owned by
`ansible_library` should not also sit in a `yaml_library`.

Own the files with `ansible_library` from `@rules_dx//ansible/rules:defs.bzl`.
List the `.yml` and `.yaml` files of the package in `srcs`. Files whose path
matches an Ansible layout (playbooks, `tasks/`, `handlers/`, `vars/`, `meta/`
and so on) are linted by `ansible-lint`. Other YAML stays in the `yaml` family.
Name playbooks that do not match a layout in `playbooks`; each must also be in
`srcs`.

```sh
bazel run @rules_dx//:dx -- lint //ansible/...
```

`ansible-lint` runs offline: no collection installs, schema refresh or version
check. A syntax error or load failure is an error and never counts as clean.
Any exit code other than clean or violations fails the run with the last line
of `ansible-lint` stderr. `--apply` runs `ansible-lint --fix=all` on the staged files and keeps the
rewrites. Modules from
`ansible.builtin` need nothing extra; other collections must be bound as below.

To change rules, bind an `ansible_lint_config` (an `.ansible-lint` file) to the
targets with `aspect_hints`.

```python
load("@rules_dx//quality:native_config.bzl", "ansible_lint_config")

ansible_lint_config(
    name = "relaxed",
    src = ".ansible-lint",
    visibility = ["//visibility:public"],
)

ansible_library(
    name = "site",
    aspect_hints = ["//:relaxed"],
    srcs = ["playbooks/site.yml"],
)
```

#### Collections

Collections are never installed at check time. Pin them in a lock and bind the
fetched tree to the config. A lock is a sorted JSON file; each entry has
exactly one of `urls` or `path` (relative to the lock):

```json
{
  "collections": [
    {
      "name": "ansible.posix",
      "sha256": "<64 hex characters>",
      "urls": ["https://galaxy.ansible.com/api/v3/plugin/ansible/content/published/collections/artifacts/ansible-posix-1.5.4.tar.gz"],
      "version": "1.5.4"
    }
  ],
  "version": 1
}
```

```python
ansible = use_extension("@rules_dx//ansible:extensions.bzl", "ansible")
ansible.collections(name = "ansible_deps", lock = "//:ansible_collections.lock.json")
use_repo(ansible, "ansible_deps")
```

```python
ansible_lint_config(
    name = "relaxed",
    src = ".ansible-lint",
    data = ["@ansible_deps//:collections"],
)
```

The archives are fetched once, checked against `sha256`, and must name the
same collection and version as the lock; every declared dependency must be in
the lock. After the first fetch the repository cache serves them, so checks run
with the network denied. A checked-in `ansible_collections/<ns>/<coll>/` tree in
`data` works the same way. A collection that is not bound fails the run and is
never reported clean, and the same `ns.coll` under two roots is an error.

### Nix

`statix` and `deadnix` lint Nix files and `nixfmt` formats them. `statix` and
`deadnix` are built from source with the Rust toolchain. `nixfmt` 1.5.0 runs
from the managed pin. All three are qualified on Linux x86_64. Select them in
a `nix` family.

```python
quality_family(
    name = "nix_family",
    family_id = "nix",
    format = ["nixfmt"],
    lint = [
        "deadnix",
        "statix",
    ],
)
```

Own Nix files with `nix_library` from `@rules_dx//nix/rules:defs.bzl`. List
the `.nix` files of the package in `srcs`.

```python
load("@rules_dx//nix/rules:defs.bzl", "nix_library")

nix_library(
    name = "pkgs",
    srcs = ["default.nix"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //nix/...
bazel run @rules_dx//:dx -- lint --apply //nix/...
```

```sh
bazel run @rules_dx//:dx -- format //nix/...
bazel run @rules_dx//:dx -- format --apply //nix/...
```

Upstream publishes `nixfmt` only as a static Linux x86_64 binary, so it is
pinned for that platform alone. On any other platform a run that formats Nix
fails with a hint to set `tool_nixfmt`. Runs with no Nix sources are not
affected. To use another executable, point `tool_nixfmt` at it.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_nixfmt=//tools:nixfmt
```

`nixfmt` runs with its default RFC style at width 100 and has no config file.
It formats the files the selected targets own and never follows symlinks. A
format check never writes. A file that does not parse is an error with line and
column, never a clean run, and `--apply` leaves it unchanged. `--apply` rewrites
the unformatted files, then checks again.

A lint check never writes. `--apply` runs `statix fix` and `deadnix --edit` on the
files the selected targets own, then checks again. `statix` findings keep the
lint code, such as `W02`, with line and column. `deadnix` findings name the
unused binding, such as `unused_let_binding`. A file that does not parse is an
error from both tools and never counts as clean.

`statix` never reads a `statix.toml` from the workspace. To disable lints, bind
a `statix_config` to the target with `aspect_hints`.

```python
load("@rules_dx//quality:native_config.bzl", "statix_config")

statix_config(
    name = "statix",
    src = "statix.toml",
)

nix_library(
    name = "pkgs",
    aspect_hints = [":statix"],
    srcs = ["default.nix"],
)
```

```toml
disabled = ["empty_let_in"]
```

### Jupyter notebooks

Ruff formats and lints Python Jupyter notebooks (`.ipynb`, nbformat 4). Select
it in a `jupyter` family. The family is opt-in. It is not part of any default
policy and the Python family does not touch notebooks.

```python
quality_family(
    name = "jupyter_family",
    family_id = "jupyter",
    format = ["ruff"],
    lint = ["ruff"],
    disabled = ["audit", "typecheck"],
)
```

Own notebooks with `jupyter_library` from `@rules_dx//jupyter/rules:defs.bzl`.
List the `.ipynb` files of the package in `srcs`.

```sh
bazel run @rules_dx//:dx -- format //notebooks/...
bazel run @rules_dx//:dx -- format --apply //notebooks/...
bazel run @rules_dx//:dx -- lint //notebooks/...
```

Findings name the cell and the line inside it, for example
`cell 3, line 1: `os` imported but unused`, and point at the line in the
notebook JSON file. Markdown cells count toward the cell number. A check never
writes.

`--apply` changes only the `source` of the code cells Ruff edited. Outputs,
metadata, execution counts, indentation, key order, line endings and every other
cell stay byte for byte as they were. Ruff's own rewrite of the file is never
written back.

Only Python notebooks are checked. A notebook with another kernel language, a
format other than nbformat 4, or invalid JSON is an error and never counts as
clean. Cells that start with a cell magic are skipped by Ruff. Disable `audit`
in the family.

To type check notebooks, list `ty` under `typecheck` in the family. The code
cells are joined into one script (line magics and shell escapes become `pass`,
and `get_ipython` and `display` are declared), checked with `ty`, and each
finding is mapped back to its cell and line in the notebook file.

To change options, bind a `ruff_config` to the notebook targets with
`aspect_hints`, as for Python.

#### Notebook outputs

The `notebook_outputs` check flags code cells that keep saved outputs or an
execution count. It is off by default and is not part of any curated default.
Enable it by listing it next to Ruff under `lint` in the `jupyter` family.

```python
quality_family(
    name = "jupyter_family",
    family_id = "jupyter",
    format = ["ruff"],
    lint = ["ruff", "notebook_outputs"],
    disabled = ["audit", "typecheck"],
)
```

Two rules exist, and neither has options:

- `cell-outputs`: a code cell whose `outputs` array is not empty.
- `execution-count`: a code cell whose `execution_count` is not `null`.

Each finding starts with `cell N:` and points at the `outputs` or
`execution_count` line of the notebook file. `--apply` sets `outputs` to `[]`
and `execution_count` to `null` and changes no other byte, so metadata,
attachments, markdown cells, key order, indentation and line endings stay as
they were. The check works with any kernel language, so an R or Julia notebook
is accepted, while invalid JSON or a format other than nbformat 4 is an error.
The tool is built from this repository and needs no install.

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
`dockerfile` family. Format is opt-in with `format = ["prettier"]` on the
`dockerfile` family; Prettier runs the `dockerfmt` printer through
`prettier-plugin-sh`, which is loaded for Dockerfiles only. Shell scripts stay
with `shfmt`.

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

### PHP

Format is opt-in with `format = ["prettier"]` on the `php` family. Prettier
formats `.php` files through the pinned `@prettier/plugin-php`; it needs no PHP
runtime. Own the files with `php_library` from `@rules_dx//php/rules:defs.bzl`.

```python
quality_family(
    name = "php_family",
    family_id = "php",
    format = ["prettier"],
)
```

### Kubernetes

`kubeconform` validates Kubernetes manifests against JSON schemas. It runs from
a managed v0.8.0 pin on Linux x86_64, Linux arm64, macOS arm64 and Windows
x86_64. Select it in a `kubernetes` family. Plain YAML stays in the `yaml`
family. Format is opt-in with `format = ["prettier"]` on the `kubernetes`
family; Prettier picks its parser from the extension, so `.json` manifests stay
JSON. Files owned by a `kubernetes_manifests` target should not also sit in a
`yaml_library`.

```python
quality_family(
    name = "kubernetes_family",
    family_id = "kubernetes",
    lint = ["kubeconform"],
)
```

Own manifests with `kubernetes_manifests` from
`@rules_dx//kubernetes/rules:defs.bzl`. It accepts `.yaml`, `.yml` and `.json`
files. A file is checked as a manifest because the rule owns it, not because of
its name. Every non-empty document needs `apiVersion` and `kind`, or the run
reports `dx/kubernetes-not-manifest` at that document's line. A file that does
not parse is `dx/kubernetes-syntax`.

Schemas are offline. Declare them with `kubernetes_schemas` and bind it with
`aspect_hints`. A `kubernetes_manifests` target without that hint fails the
analysis and names `kubernetes_schemas`.

```python
load("@rules_dx//kubernetes/rules:defs.bzl", "kubernetes_manifests", "kubernetes_schemas")

kubernetes_schemas(
    name = "schemas",
    kubernetes_version = "1.32.0",
    schemas = ["deployment-apps-v1.json", "service-v1.json"],
    crd_schemas = {"widget_v1.json": "example.com"},
    strict = True,
    skip_kinds = ["Widget"],
)

kubernetes_manifests(
    name = "manifests",
    aspect_hints = [":schemas"],
    srcs = ["deploy.yaml"],
)
```

`kubernetes_version` is required and must look like `X.Y.Z`. `schemas` are
core schema files named like the yannh/kubernetes-json-schema files, for
example `deployment-apps-v1.json`. `crd_schemas` maps each `<kind>_<version>.json`
file to its API group. Duplicate names fail the analysis. `strict` defaults to
true and rejects duplicate keys in a manifest. It does not reject unknown
fields: that needs the `-standalone-strict` variants of the schema files in
`schemas`, because the schema decides which fields are allowed.
`kubernetes_version` only labels the run; the declared schema files decide
which Kubernetes version is checked. `skip_kinds` lists kinds that are not
checked.

`kubeconform` never reaches a network schema location, and a kind with no
schema is an error, not a skip. Findings keep the line and column of the
offending field. `kubeconform` is check-only: `lint --apply` never rewrites a
manifest. Missing schemas, malformed output and a non-zero exit with no result
are errors and never count as clean.

### Helm

`helm` lints a Helm chart and validates the manifests it renders. It runs from a
managed v4.3.1 pin on Linux x86_64, Linux arm64, macOS arm64 and Windows
x86_64. Select it in a `helm` family. It runs `helm lint`, then
`helm template`, then checks every rendered document with the same
`kubeconform` pin and the same offline schemas as the Kubernetes family.

```python
quality_family(
    name = "helm_family",
    family_id = "helm",
    lint = ["helm"],
)
```

Own a chart with `helm_chart` from `@rules_dx//helm/rules:defs.bzl`. The package
holding `Chart.yaml` is the chart root, so each chart needs its own package.

```python
load("@rules_dx//helm/rules:defs.bzl", "helm_chart")
load("@rules_dx//kubernetes/rules:defs.bzl", "kubernetes_schemas")

kubernetes_schemas(
    name = "schemas",
    kubernetes_version = "1.32.0",
    schemas = ["deployment-apps-v1.json", "service-v1.json"],
)

helm_chart(
    name = "chart",
    aspect_hints = [":schemas"],
    srcs = ["Chart.yaml", "values.yaml", "templates/deploy.yaml"],
    values = ["values-prod.yaml"],
    dependencies = ["charts/common-1.0.0.tgz"],
    release_name = "web",
    namespace = "prod",
)
```

`srcs` are the chart files and must include `Chart.yaml`; archives are not
allowed there. `values` are extra values files passed to Helm in order, so a
later file overrides an earlier one. `release_name` defaults to `release` and
`namespace` to `default`; both must be DNS-1123 labels. A file listed twice
fails the analysis.

Dependencies are never fetched. Vendor each subchart archive under `charts/`,
commit the matching `Chart.lock`, and list the archives in `dependencies`. A
chart that declares a dependency with no vendored archive reports an error.

Schemas come from the `kubernetes_schemas` hint, which also sets the Kubernetes
version passed to Helm. A `helm_chart` target without that hint fails the
analysis and names `kubernetes_schemas`. A native config cannot be declared for
`helm` itself.

Findings from rendered output point at line 1, column 1 of the template that
produced them and keep the resource kind, name and field in the message.
Findings from a vendored subchart point at `Chart.yaml`. A render failure
reports the template position Helm names. `helm` is check-only: `lint --apply`
never rewrites a chart. It runs with no network access, and Helm cache, config
and plugin directories live in the scratch tree. A Helm exit that is not
understood, a missing schema and malformed output are errors and never count as
clean.

### CMake

`gersemi` formats CMake files and `cmakelint` lints them. Both run from managed
pins on Linux x86_64, Linux arm64, macOS arm64 and Windows x86_64, so consumers
need no Python toolchain and no host install. They are opt-in: no default
policy selects either. dx never runs `cmake`.

```python
quality_family(
    name = "cmake_family",
    family_id = "cmake",
    format = ["gersemi"],
    lint = ["cmakelint"],
)
```

Own CMake files with `cmake_library` from `@rules_dx//cmake/rules:defs.bzl`.
List the files of the package in `srcs`. Accepted names are `CMakeLists.txt`
and `*.cmake`, matched case-sensitively. Any other name fails the analysis with
the accepted names listed. Templates such as `CMakeLists.txt.in` and
`*.cmake.in` are not accepted, and neither is `CMakeCache.txt`.

```python
load("@rules_dx//cmake/rules:defs.bzl", "cmake_library")

cmake_library(
    name = "build",
    srcs = ["CMakeLists.txt"],
)
```

```sh
bazel run @rules_dx//:dx -- format //...
bazel run @rules_dx//:dx -- format --apply //...
```

`format` reports each unformatted file; `format --apply` rewrites only the
files a `cmake_library` owns. A file that does not parse is reported as an
error at its line and column, and `--apply` leaves it unchanged. One broken
file does not change what is reported for the other files in its batch.
Unknown-command warnings from gersemi are neither findings nor errors. A
missing or invalid config, an unreadable file and a non-zero exit without a
diff are errors and never count as clean.

`gersemi` never reads a `.gersemirc` from the workspace, and it keeps no cache
and reads nothing from the home directory. To change the style, bind a
`gersemi_config` to the target with `aspect_hints`. Without one, dx passes an
empty config, which keeps every gersemi default (4-space indent, line length
80).

```python
load("@rules_dx//quality:native_config.bzl", "gersemi_config")

gersemi_config(
    name = "gersemi",
    src = ".gersemirc",
)

cmake_library(
    name = "build",
    aspect_hints = [":gersemi"],
    srcs = ["CMakeLists.txt"],
)
```

`extensions:` and `definitions:` entries in a `.gersemirc` load files that sit
next to the config. They only resolve when those files are in the
`gersemi_config` `data`, and running them is the consumer's own trust decision,
like an `eslint.config.mjs`.

The pinned binaries are PyInstaller one-file builds that unpack into the
runner's scratch `TMPDIR` on every start, so the first start of each batch
takes a moment. To use another gersemi
executable, point `tool_gersemi` at it.

```sh
bazel run @rules_dx//:dx -- format -- --@rules_dx//config:tool_gersemi=//tools:gersemi
```

#### cmakelint

`cmakelint` 1.4.3 checks CMake style: command case, whitespace, line length,
indent width and `find_package` usage. It is a style checker, so it does not
report syntax errors; `gersemi` does. `lint` reports each finding at its line,
with the category (for example `readability/mixedcase`) as the rule ID. It
reports no columns, so findings point at column 1. `cmakelint` is check-only:
`lint --apply` never rewrites a file.

```sh
bazel run @rules_dx//:dx -- lint //...
```

An output dx cannot account for is an error and never counts as clean. That
covers a missing or mismatched `Total Errors` summary, an exit code that
disagrees with the findings, a skipped file (`Ignoring file:`), a crash and an
unparsable output line.

`cmakelint` never reads a `.cmakelintrc` from the workspace or the home
directory. To change the rules, bind a `cmakelint_config` to the target with
`aspect_hints`. Without one, dx passes an empty config, which keeps every
default (2-space indent unit, line length 80). A bound config may hold only
blank lines, `#` comments and the keys `filter=`, `spaces=` and `linelength=`.
Any other line, including `quiet` and a mistyped key, fails the run, because
`cmakelint` would silently ignore it. An unreadable config fails the run too.

```python
load("@rules_dx//quality:native_config.bzl", "cmakelint_config")

cmakelint_config(
    name = "cmakelint",
    src = ".cmakelintrc",
)
```

A `# lint_cmake: -readability/mixedcase` comment at the start of a line
filters a category from that line on, within its file.

`gersemi` and `cmakelint` overlap in two places. `spaces=` makes `cmakelint`
require indents that are a multiple of its value; the default of 2 accepts the
4-space indent `gersemi` writes, but a `gersemi` `indent` that is not a multiple
of `spaces=` needs `spaces=` to match. A `gersemi` `line_length` above 80
needs `linelength=` to match, or `filter=-linelength` to drop the check.

### Make

`checkmake` lints Makefiles and `mbake` formats them. `checkmake` runs from a
managed pin on Linux x86_64, Linux arm64, macOS arm64 and Windows x86_64.
`mbake` is a pinned Python tool and runs on Linux x86_64 only. Both are opt-in:
no default policy selects either, because the `checkmake` `minphony` rule fires
on most real Makefiles. dx never runs `make`.

```python
quality_family(
    name = "make_family",
    family_id = "make",
    format = ["mbake"],
    lint = ["checkmake"],
)
```

Own Makefiles with `make_library` from `@rules_dx//make/rules:defs.bzl`. List
the files of the package in `srcs`. Accepted names are `Makefile`, `makefile`,
`GNUmakefile` and `*.mk`, matched case-sensitively. Any other name fails the
analysis with the accepted names listed. `Makefile.am`, `Makefile.in` and
`*.mak` are not accepted.

```python
load("@rules_dx//make/rules:defs.bzl", "make_library")

make_library(
    name = "build",
    srcs = ["Makefile"],
)
```

```sh
bazel run @rules_dx//:dx -- lint //...
```

`checkmake` is check-only: `lint --apply` never rewrites a Makefile. Findings
keep the rule name and line. A finding without a line is reported at line 1.
checkmake stops at the first Makefile it cannot parse, so that file is reported
as a `parse-error` and the findings of the other Makefiles in the same batch
are not shown until it is fixed.

`checkmake` never reads `$HOME/checkmake.ini` or a `checkmake.ini` from the
workspace. To change the rules, bind a `checkmake_config` to the target with
`aspect_hints`. checkmake silently falls back to its defaults when it cannot
read the config, so dx requires checkmake to confirm the exact config path it
was given and reports anything else as an error.

```python
load("@rules_dx//quality:native_config.bzl", "checkmake_config")

checkmake_config(
    name = "checkmake",
    src = "checkmake.ini",
)

make_library(
    name = "build",
    aspect_hints = [":checkmake"],
    srcs = ["Makefile"],
)
```

`mbake` formatting is the part that `--apply` rewrites:

```sh
bazel run @rules_dx//:dx -- format //...
bazel run @rules_dx//:dx -- format --apply //...
```

`mbake` never reads `~/.bake.toml` or a `.bake.toml` from the workspace. Without
a bound config dx passes a default that keeps every mbake default except
`ensure_final_newline = true`: it does not insert `.PHONY` declarations, group
them, or align variable assignments. To change the rules, bind an
`mbake_config` (a `.bake.toml`) to the target with `aspect_hints`. dx never
passes `--validate` (it runs the host `make`) and never runs `mbake update`.
mbake does not reject a Makefile as a syntax error, so `format` reports only
files that would change, at line 1. mbake output is parsed from the exit code
and the `Would reformat:` file names, and anything else is an error and never a
clean run.

```python
load("@rules_dx//quality:native_config.bzl", "mbake_config")

mbake_config(
    name = "mbake",
    src = ".bake.toml",
)

make_library(
    name = "build",
    aspect_hints = [":mbake"],
    srcs = ["Makefile"],
)
```

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
