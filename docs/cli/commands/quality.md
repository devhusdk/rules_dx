# Quality Commands

`dx lint` runs linters, `dx typecheck` runs type checkers and compilers in check mode, and `dx format` checks formatting. Each reports drift without writing files.

```sh
tools/dx lint //...
tools/dx lint --affected
tools/dx typecheck --fail-on error //cli/...
tools/dx format --here
tools/dx format --apply //cli/...
```

```text
dx lint [flags] [scope...] [-- bazel-options ...]
dx typecheck [flags] [scope...] [-- bazel-options ...]
dx format [flags] [scope...] [-- bazel-options ...]
```

## Flags

| Flag | What it does |
| --- | --- |
| `--apply` | Write validated fixes and refresh the `--baseline` file. Text output prints `Applied: <path>` per file, then `Applied N file(s).` |
| `--online` | `lint` only. Check external links too: `lychee` drops `--offline`. Needs network. |
| `--diff` | Print a unified patch of the proposed fixes instead of findings. |
| `--fail-on info\|warning\|error` | Severity that fails the run. Default `warning`. |
| `--baseline <file>` | Suppress findings already in this workspace-relative `.json` file. See [Baselines](#baselines). |
| `--report sarif=<dest>` | Write a SARIF report. Repeatable. `lint` and `typecheck` only; `format` has no report. |
| `-o, --output text\|json` | Result shape. Default `text`. |
| `--include-edits` | Add `source_digest` and full `edits` to JSON `change` events. Needs `--output json`, else exit `2`. |
| `--here` | Scope to the current directory tree. |
| `--affected` | Scope to targets your changes reach. With none, prints `no affected targets` and exits `0`. |
| `--workspace <dir>` | Use this workspace dir. |
| `-q, --quiet` | Hide summaries. Findings and failures still print. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots auto\|n\|off` | Set build slots. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. Use it for `--output_base` and `--output_user_root`. |

## Scopes

No scope means `//...`. Pass Bazel labels or patterns, or workspace-relative files and dirs. `--here` and `--affected` cannot be combined with explicit scopes or with each other. Targets that do not fit the target platform are skipped, even when named. See [Scope Defaults](scope-defaults.md).

## Bazel options

Args after `--` go to Bazel. dx adds `--keep_going` and `--remote_download_outputs=toplevel` after them. Pass `--remote_download_outputs=all` or `--remote_download_all` to download all outputs.

These args exit `2`: `--aspects`, `--output_groups`, `--build_event_json_file`, `--nokeep_going`, `--remote_download_minimal`, `--test_arg`, and the startup options `--bazelrc`, `--home_rc`, `--nohome_rc`, `--system_rc`, `--nosystem_rc`, `--output_base`, `--output_user_root`, `--host_jvm_args` and `--server_jvm_out`.

All three commands set `--@rules_dx//config:clippy_output_diagnostics=true` and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for either, or for the matching `@rules_rust//rust/settings` flag, exits `2`. Use the repo name your `MODULE.bazel` gives rules_dx.

For Rust pipelining, set `--@rules_rust//rust/settings:pipelined_compilation=true` or `=false` in `.bazelrc`.

## Output

Text prints one line per finding:

```text
path:line:column: severity: message [tool/rule]
```

Lines and columns count Unicode characters. A finding without a position prints `path:`. A finding without a file prints `(no file):`. Identical findings print once with `(xN)`. Text ends with a `dx: <total> actions, ...` line on stderr.

When files need changes, text prints `N file(s) need changes. Apply them with: dx <command> --apply ...`. Failures print a `next:` line with the fix.

`--diff` prints the patch on stdout. `Not applied:`, `Conflict:` and `Baseline stale:` lines go to stderr.

JSON prints one `change` per file with `path`, `kind`, `edit_count` and `effect` (`proposed`, `applied` or `refused`). A refused change has a `reason`: `stale_source`, `unreadable_source`, `invalid_edits`, `write_failed`, `incomplete_collection`, `conflicting_digests` or `conflicting_candidates`. A check with proposed changes adds an info `notice` with code `changes_proposed`; its `message` is the `--apply` command. Findings from several targets print once with a `count` and every target in `origins`.

A stdout SARIF report (`--report sarif=-`) works only with text output and moves findings to stderr. Relative destinations anchor at the workspace root. Colliding destinations and missing parent dirs exit `2`.

Failures:

- `dx: incomplete_results: <reason>` (JSON: an `incomplete_results` error) means a target failed, a result was invalid, or the build events ended early. Fixes are never applied.
- `dx: not_converged: <target> ... did not converge: OSCILLATION|ITERATION_LIMIT` means a target's fixes oscillated or hit the ten-round limit. That target proposes no fixes.
- A failed build reports `bazel_failed` (compile errors) or `analysis_failed`, with a `build_failure` event or a `dx: build failure:` line.

Several targets that fix the same file apply identical fixes once. Fixes that disagree leave the file unchanged and print a `Conflict:` line.

`dx lint` without `--apply` also fails with exit `1` when the checkout holds build output or a file over 1 MiB, as `dx check` does.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Findings at or above `--fail-on`; any proposed change in a check; an incomplete result set; a target that did not converge; a conflicting or unapplied fix; a failed report write; a missing, malformed or stale baseline; a Bazel failure. |
| `2` | Usage or scope error. |

## Policy

The commands check with the policy named by `--@rules_dx//config:workspace`. Default: `@rules_dx//quality:default_workspace_policy`.

```sh
tools/dx lint -- --@rules_dx//config:workspace=//quality:my_policy
```

To set it once, add to `.bazelrc`:

```text
common --@rules_dx//config:workspace=//:policy
```

A policy is a `workspace_policy` of `quality_family` sections. Each family names a tool ID or lists the capability in `disabled`. A capability left unset, or a tool with no wired executable, fails analysis.

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

`extends` starts from another policy. A family in `families` replaces the base family with the same `family_id`, or adds one. `disabled` on the policy drops base family IDs.

```python
workspace_policy(
    name = "policy",
    extends = "@rules_dx//quality:default_workspace_policy",
    families = [":python_family"],
    disabled = ["rust"],
)
```

These fail: `disabled` without `extends`, a disabled ID the base lacks, a family both disabled and in `families`, and a family listed twice.

### Default policy

| Family | Format | Lint | Typecheck |
| --- | --- | --- | --- |
| `astro` | `prettier` | `typos` | `tsc` |
| `cc` | `clang_format` | `typos` | disabled |
| `dockerfile` | disabled | `hadolint`, `typos` | disabled |
| `go` | `gofumpt` | `typos` | `gotype` |
| `graphql` | `prettier` | `typos` | disabled |
| `html` | `prettier` | `typos` | disabled |
| `java` | `google_java_format` | `checkstyle`, `pmd`, `spotbugs`, `typos` | `javac` |
| `javascript` | `biome` | `biome`, `typos` | disabled |
| `json` | `prettier` | `biome` | disabled |
| `kotlin` | `ktfmt` | `ktlint`, `typos` | `kotlinc` |
| `markdown` | disabled | `lychee`, `markdown_check`, `typos`, `vale` | disabled |
| `mdx` | `remark` | `typos` | disabled |
| `python` | `ruff` | `pydoclint`, `ruff`, `typos`, `vulture` | `ty` |
| `rust` | `rustfmt` | `clippy`, `typos` | `rustc` |
| `scala` | `scalafmt` | `typos` | `scalac` |
| `shell` | `shfmt` | `shellcheck`, `typos` | disabled |
| `starlark` | `buildifier` | `buildifier`, `typos` | disabled |
| `svelte` | `prettier` | `typos` | disabled |
| `text` | disabled | `keep_sorted`, `typos` | disabled |
| `toml` | `taplo` | `taplo`, `typos` | disabled |
| `typescript` | `biome` | `biome`, `typos` | `tsc` |
| `vue` | `prettier` | `typos` | disabled |

Families outside this table are opt-in: select their tools in your own policy.

## Tools

The `Not on` column lists platforms where the tool does not run. Empty means all five: `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`. The [support matrix](../../support-matrix.md) lists each language's platforms.

### Override a tool

Every tool runs from a managed executable. To use your own build, pass its target after `--`. The tool ID stays the same, so findings read the same.

```sh
tools/dx lint -- --@rules_dx//config:tool_<name>=//tools:my_tool
```

Tools with an override flag: `ruff`, `terraform`, `hadolint`, `opa`, `regal`, `gofumpt`, `gotype`, `qmlformat`, `qmllint`, `julia_format`, `zig_ast_check`, `zig_fmt`, `swift_format`, `swift_lint`, `dart`, `dfmt`, `dscanner`, `perlcritic`, `perltidy`, `erlfmt`, `elvis`, `dialyzer`, `jet`, `julialint`, `luals`, `phpcs`, `phpstan`, `sorbet`, `mix_format`, `credo`, `ormolu`, `nixfmt`, `flatc`.

The `ruff`, `terraform`, `hadolint`, `opa`, `regal`, `ormolu` and `nixfmt` alternates must be a single executable file. The others must be executable targets. Anything else fails analysis with the tool named.

### Native languages

| Tool | Checks | Not on |
| --- | --- | --- |
| `clang_format` | Format C, C++, CUDA, Objective-C | |
| `clang_tidy` | Lint C, C++, CUDA, Objective-C | |
| `cppcheck` | Lint C and C++ | |
| `clang_syntax` | Typecheck C, C++, Objective-C with `-fsyntax-only` | Objective-C: not on Linux or Windows |
| `nvcc_syntax` | Typecheck CUDA with `nvcc -cuda` | |
| `rustfmt` | Format Rust | |
| `clippy` | Lint Rust | |
| `rustc` | Typecheck Rust | |
| `gofumpt` | Format Go | |
| `govet`, `staticcheck`, `errcheck` | Lint Go | |
| `gotype` | Typecheck Go | |
| `modfmt` | Format `go.mod` (family `go_module`, from `go_module_srcs`) | |
| `zig_fmt` | Format Zig | |
| `zig_ast_check` | Lint Zig | |
| `zig` | Typecheck Zig | |
| `dfmt` | Format D | |
| `dscanner` | Lint D | |
| `ldc` | Typecheck D | |
| `ffmt` | Format Fortran | |
| `fortitude` | Lint Fortran | |
| `flang` | Typecheck Fortran (opt-in) | |
| `swift_format` | Format Swift | |
| `swift_lint` | Lint Swift | |

`clang_syntax` replays each compile action with `-fsyntax-only`, using the command line Bazel configured. Headers are checked only when the target compiles them (`--features=parse_headers`). Toolchains without a readable command line (clang-cl, params files) fail with an explicit error. Select it with `typecheck = ["clang_syntax"]` on the `cc` family. It has no fix support.

`nvcc_syntax` replays each `CudaCompile` action with `-c` changed to `-cuda`. Device compilation still runs for every `-gencode` architecture, so the check is heavy. Select it with `typecheck = ["nvcc_syntax"]` on the `cuda` family. Params files and header-only `cuda_library` targets fail with an explicit error.

`clang_tidy` on MSVC toolchains runs in clang-cl mode and names flags it cannot model: `/Yc`, `/Yu`, `/clr`, `/ZW`.

`clang_tidy` uses a dx default `.clang-tidy` when no `clang_tidy_config` is bound. It enables `misc-unused-parameters`, `misc-unused-using-decls` and `readability-redundant-declaration`. `cppcheck` runs with `--enable=unusedFunction`. Findings from these checks are reported as info. cppcheck sees only the files of one target, so a function used only from another target is reported as unused.

### JVM and .NET

| Tool | Checks | Not on |
| --- | --- | --- |
| `google_java_format` | Format Java | |
| `checkstyle`, `pmd`, `spotbugs` | Lint Java | |
| `javac` | Typecheck Java | |
| `ktfmt` | Format Kotlin | |
| `ktlint` | Lint Kotlin | |
| `kotlinc` | Typecheck Kotlin | |
| `scalafmt` | Format Scala | |
| `scalafix` | Lint Scala | |
| `scalac` | Typecheck Scala | |
| `csharpier` | Format C# | |
| `roslyn` | Lint C# | |
| `csc` | Typecheck C# | |
| `fantomas` | Format F# | |
| `fsharplint` | Lint F# | |
| `fsc` | Typecheck F# | |

### Scripting languages

| Tool | Checks | Not on |
| --- | --- | --- |
| `ruff` | Format and lint Python and Jupyter; audit | |
| `flake8`, `pylint`, `pydoclint` | Lint Python | |
| `ty` | Typecheck Python and Jupyter | |
| `rubocop` | Lint Ruby | `windows_x86_64`, `windows_arm64` |
| `debride` | Report Ruby methods nothing calls (info) | `windows_x86_64`, `windows_arm64` |
| `standardrb` | Format Ruby | `windows_x86_64`, `windows_arm64` |
| `sorbet` | Typecheck Ruby | `windows_x86_64`, `windows_arm64` |
| `phpcs` | Lint PHP | `windows_arm64` |
| `phpstan` | Typecheck PHP | `windows_arm64` |
| `perltidy` | Format Perl | `windows_arm64` |
| `perlcritic` | Lint Perl | `windows_arm64` |
| `stylua` | Format Lua | |
| `selene` | Lint Lua | |
| `luals` | Typecheck Lua | |
| `julia_format` | Format Julia | `windows_arm64` |
| `julialint` | Lint Julia | `windows_arm64` |
| `jet` | Typecheck Julia | |
| `mix_format` | Format Elixir | |
| `credo` | Lint Elixir | |
| `dialyzer` | Typecheck Erlang and Elixir | `windows_x86_64`, `windows_arm64` |
| `erlfmt` | Format Erlang | `windows_x86_64`, `windows_arm64` |
| `elvis` | Lint Erlang | `windows_x86_64`, `windows_arm64` |
| `xref` | Lint unused exports in Erlang and Elixir | `windows_x86_64`, `windows_arm64` |
| `ormolu` | Format Haskell | `linux_arm64`, `macos_arm64` |
| `hlint` | Lint Haskell | `linux_arm64`, `macos_arm64` |
| `ghc` | Typecheck Haskell | |
| `topiary` | Format OCaml | |
| `zanuda` | Lint OCaml | all but `linux_x86_64` |
| `ocaml` | Typecheck OCaml (opt-in) | |
| `dart` | Format, lint and typecheck Dart | `windows_arm64` |
| `psscriptanalyzer` | Format and lint PowerShell | `windows_x86_64`, `windows_arm64` |
| `shfmt` | Format shell | |
| `shellcheck` | Lint shell | |
| `qmlformat` | Format QML | `windows_x86_64`, `windows_arm64` |
| `qmllint` | Lint and typecheck QML | `windows_x86_64`, `windows_arm64` |

### Web

| Tool | Checks | Not on |
| --- | --- | --- |
| `biome` | Format and lint JavaScript, TypeScript, JSON | |
| `eslint` | Lint Astro, JavaScript, MDX, Svelte, Vue | |
| `knip` | Lint unused files, exports and dependencies in JavaScript and TypeScript | |
| `tsc` | Typecheck Astro, JavaScript, JSX, MDX, Svelte, TSX, TypeScript, Vue | |
| `prettier` | Format Astro, CSS, HTML, JSON, Svelte, TypeScript, Vue, GraphQL | |
| `stylelint` | Lint CSS, SCSS, Less | |
| `graphql_eslint` | Lint GraphQL | |
| `remark` | Format MDX | |
| `djlint` | Format and lint HTML templates | |

### Config, data and build files

| Tool | Checks | Not on |
| --- | --- | --- |
| `taplo` | Format and lint TOML | |
| `yamlfmt`, `yamllint` | Format and lint YAML | |
| `xmllint` | Lint XML | |
| `prettier` | Format XML, Ansible, Kubernetes, Dockerfile, Gherkin, PHP, SQL, API specs | |
| `sqlfluff` | Format and lint SQL | |
| `buildifier` | Format and lint Starlark | |
| `cmakelint`, `gersemi` | Lint and format CMake | |
| `checkmake`, `mbake` | Lint and format Make | |
| `hadolint` | Lint Dockerfiles | |
| `ansible_lint` | Lint Ansible | `windows_x86_64`, `windows_arm64` |
| `kubeconform` | Validate Kubernetes manifests | |
| `helm` | Lint Helm charts and validate rendered manifests | |
| `terraform` | Format and typecheck Terraform | |
| `tflint` | Lint Terraform | |
| `bicep` | Format and lint Bicep | |
| `cfn_lint` | Lint CloudFormation | |
| `nixfmt` | Format Nix | all but `linux_x86_64` |
| `deadnix`, `statix` | Lint Nix | |
| `opa` | Format and typecheck Rego | |
| `regal` | Lint Rego | |
| `cue` | Format and typecheck CUE | |
| `pkl` | Format and typecheck Pkl | |
| `pkl_lsp` | Lint Pkl | `windows_arm64` |
| `jsonnetfmt`, `jsonnet_lint` | Format and lint Jsonnet | |
| `redocly` | Lint OpenAPI and AsyncAPI | |
| `gplint` | Lint Gherkin | |
| `notebook_outputs` | Lint Jupyter notebooks for stored outputs | |
| `keep_sorted` | Lint `keep-sorted` blocks in text | |
| `typos` | Spell check source, docs and text, and fix with `--fix`. Add words to a `_typos.toml` at the repo root | |

### Schema languages

| Tool | Checks | Not on |
| --- | --- | --- |
| `buf` | Format and lint Protocol Buffers | |
| `flatc` | Lint and typecheck FlatBuffers | |
| `capnp` | Lint and typecheck Cap'n Proto | `linux_arm64`, `macos_arm64`, `windows_x86_64` |
| `thrift` | Lint and typecheck Thrift | `linux_arm64`, `macos_arm64`, `windows_x86_64` |
| `avro_tools` | Lint and typecheck Avro | |

Schema languages have no formatter, so `--apply` has nothing to fix for them.

### Docs and markup

| Tool | Checks | Not on |
| --- | --- | --- |
| `lychee` | Check links in Markdown, MDX, reStructuredText, AsciiDoc and HTML; local paths and `#anchors`, and with `dx lint --online` external links too | |
| `markdown_check` | Lint Markdown | |
| `vale` | Lint Markdown, AsciiDoc, reStructuredText prose | |
| `typstyle` | Format Typst | |
| `tinymist` | Lint Typst | |
| `latexindent` | Format LaTeX | |
| `chktex` | Lint LaTeX | |

### FlatBuffers

Select `flatc` in a `flatbuffers` family. The family is opt-in.

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
tools/dx lint //api/...
```

List the package files in `srcs` and the libraries they include in `deps`. An include that is not a declared input fails with `include '<path>' is outside the declared inputs of <schema>`. An include `flatc` cannot find reports `unable to load include file`. Warnings fail the lint.

#### Breaking-change check

`flatbuffers_conform_test` runs `flatc --conform` of a `library` against a `baseline`. Both are `flatbuffers_library` targets. A breaking change fails `bazel test` with flatc's message, for example `schemas don't conform: field deleted: Shape.name`.

```python
load("@rules_dx//flatbuffers/rules:conform.bzl", "flatbuffers_conform_test")

flatbuffers_conform_test(
    name = "order_conform_test",
    baseline = "//api/v1:order",      # or "@schema_v1//:order"
    library = ":order",
)
```

Set `root` and `baseline_root` when a library owns more than one schema.

## Tool config files

Bind a config with the `*_config` rule from `@rules_dx//quality:native_config.bzl` and add it to the target's `aspect_hints`. A tool marked required fails analysis until one is bound. Tag a target `no-lint` or `no-format` to skip it.

| Tool | Required | Config it reads | Files it ignores | How to bind |
| --- | --- | --- | --- | --- |
| `biome` | yes | `biome.json` | | `biome_config` |
| `buildifier` | yes | `.buildifier.json` | | `buildifier_config` |
| `checkstyle` | yes | `checkstyle.xml` | | `checkstyle_config` |
| `eslint` | yes | `eslint.config.mjs` | | `eslint_config` |
| `graphql-eslint` | yes | `graphql_lint.json` | | `graphql_eslint_config` |
| `kubeconform` | yes | `kubernetes.kubeconform.json` | | `kubernetes_schemas` from `@rules_dx//kubernetes/rules:defs.bzl` |
| `helm` | yes | `kubernetes.kubeconform.json` | | `kubernetes_schemas`, shared with `kubeconform` |
| `ruff` | yes | `ruff.toml` | | `ruff_config` |
| `scalafix` | no | `.scalafix.conf`, default `rules = [RemoveUnused]` | | `scalafix_config` |
| `scalafmt` | yes | `.scalafmt.conf` | | `scalafmt_config` |
| `sqlfluff` | yes | `.sqlfluff` | | `sqlfluff_config` |
| `vale` | yes | `.vale.ini` | | `vale_config` |
| `pmd` | no | `.xml` ruleset, default unused-code rules at info (`quality/tools/jvm/pmd_default.xml`) | | `pmd_config` |
| `clang-format` | no | `.clang-format` | | `clang_format_config` |
| `djlint` | no | `djlint.toml` or `.djlintrc` | Workspace configs | `djlint_config` |
| `latexindent` | no | YAML settings file | `localSettings.yaml` | `latexindent_config` |
| `shellcheck` | no | `.shellcheckrc` | Host and parent-directory `.shellcheckrc`, `~/.shellcheckrc`, XDG config | `shellcheck_config` |
| `stylua` | no | `stylua.toml` | `stylua.toml`, `.stylua.toml` and `.editorconfig` next to the sources | `stylua_config` |
| `remark` | no | `.json` | `.remarkrc` files and remark plugins | `remark_config` |
| `prettier` (SQL, XML, Gherkin) | no | `.prettierrc` | `.prettierrc`, `.editorconfig` and other user configs | `prettier_config` |
| `prettier` (Vue, Svelte, Astro) | no | none | `.prettierrc`, `.editorconfig` and other user configs | none |
| `dfmt` | no | none | `.editorconfig` | none |
| `dscanner` | no | none | `dscanner.ini` | none |
| `perltidy` | no | none | `.perltidyrc`, `$PERLTIDY` | none |
| `perlcritic` | no | none | `.perlcriticrc`, `$PERLCRITIC` | none |
| OCaml formatting | no | none | `.ocamlformat` | none |

## Baselines

A baseline keeps known findings while new ones fail. Pass the file with `--baseline` to `dx lint`, `dx typecheck`, `dx format`, `dx check` or `dx watch`.

```sh
tools/dx lint --baseline quality/baseline.json //...
```

The path is workspace-relative, stays inside the workspace, and names a `.json` file. Any other path exits `2`. A missing file fails checks. Pass `--apply` once to write it, then commit it.

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

A finding without a path records the Bazel `target` instead of `path`.

Checks print `Baseline <file>: <total> total, <new> new, <suppressed> suppressed.` Suppressed findings print with `(baselined)` and stay in JSON and SARIF, where JSON marks them `"baseline": "suppressed"`. New findings fail. Fixed entries turn stale and fail. An incomplete run never goes green through a baseline.

`--apply` adds new findings, prunes fixed entries in scope, and keeps the rest. It writes nothing after an incomplete run, a target that did not converge, or an unreadable source file. A malformed file fails and is not rewritten.
