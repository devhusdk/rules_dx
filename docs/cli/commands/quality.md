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
| `--apply` | Write validated fixes. Text output prints `Applied: <path>` per file, then `Applied N file(s).` |
| `--unsafe` | With `--apply` or `--diff`, also include the fixes a tool marks unsafe (ruff unsafe fixes, clippy `MaybeIncorrect`). Without either, exit `2`. JSON `command_started` carries `"unsafe_fixes": true`. A tool whose adapter does not map unsafe fixes yet still applies only its safe fixes. |
| `--breaking-against[=REF]` | `lint` only. Also run `buf breaking` against the merge base of `HEAD` and `REF` (default `origin/HEAD`). `dx` builds the base image with the pinned `buf`; the lint action runs unsandboxed and uncached. |
| `--online` | `lint` only. Check external links too: `lychee` drops `--offline`. Needs network. |
| `--diff` | Print a unified patch of the proposed fixes instead of findings. |
| `--fail-on info\|warning\|error` | Severity that fails the run. Default `warning`. |
| `--report sarif=<dest>` | Write a SARIF report. Repeatable. `lint` and `typecheck` only; `format` has no report. |
| `-o, --output text\|json` | Result shape. Default `text`. |
| `--include-edits` | Add `source_digest` and full `edits` to JSON `change` events. Needs `--output json`, else exit `2`. |
| `--here` | Scope to the current directory tree. |
| `--affected` | Scope to targets your changes reach. With none, prints `no affected targets` and exits `0`. With `--diff`, the line goes to stderr and stdout stays empty. |
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

All three commands set `--@rules_dx//config:clippy_output_diagnostics=true` and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for either, or for the matching `@rules_rust//rust/settings` flag, exits `2`. Use the repo name your `MODULE.bazel` gives rules_dx. These are the settings `dx build` and `dx test` use, so switching between commands keeps Bazel's analysis cache. `dx lint` reports unused crate dependencies from its own clippy run, without changing the Bazel configuration.

For Rust pipelining, set `--@rules_rust//rust/settings:pipelined_compilation=true` or `=false` in `.bazelrc`.

## Output

Text prints one line per finding:

```text
path:line:column: severity: message [tool/rule]
```

Lines and columns count Unicode characters. A finding without a position prints `path:`. A finding without a file prints `(no file):`. Identical findings print once with `(xN)`. Text ends with a `dx: <total> actions, ...` line on stderr.

When files need changes, text prints `N file(s) need changes. Apply them with: dx <command> --apply ...`. Failures print a `next:` line with the fix.

`--diff` prints the patch on stdout. `Not applied:` and `Conflict:` lines go to stderr.

JSON prints one `change` per file with `path`, `kind`, `edit_count` and `effect` (`proposed`, `applied` or `refused`). A refused change has a `reason`: `stale_source`, `unreadable_source`, `invalid_edits`, `write_failed`, `incomplete_collection`, `conflicting_digests` or `conflicting_candidates`. A check with proposed changes adds an info `notice` with code `changes_proposed`; its `message` is the `--apply` command. Findings from several targets print once with a `count` and every target in `origins`.

A stdout SARIF report (`--report sarif=-`) works only with text output and moves findings to stderr. Relative destinations anchor at the workspace root. Colliding destinations and missing parent dirs exit `2`.

Failures:

- `dx: incomplete_results: <reason>` (JSON: an `incomplete_results` error) means a target failed, a result was invalid, or the build events ended early. Fixes are never applied.
- `dx: not_converged: <target> ... did not converge: OSCILLATION|ITERATION_LIMIT` means a target's fixes oscillated or hit the ten-round limit. That target proposes no fixes.
- A failed build reports `bazel_failed` (compile errors) or `analysis_failed`, with a `build_failure` event or a `dx: build failure:` line.

Several targets that fix the same file apply identical fixes once. Fixes that disagree leave the file unchanged and print a `Conflict:` line.

`--apply` removes unused `deps` entries and adds missing direct deps in one write per BUILD file. Missing deps are added only for main-repo labels; a finding for an external repository stays reported.

`dx lint` without `--apply` also fails with exit `1` when the checkout holds build output or a file over 1 MiB, as `dx check` does.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Findings at or above `--fail-on`; any proposed change in a check; an incomplete result set; a target that did not converge; a conflicting or unapplied fix; a failed report write; a Bazel failure. |
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

A policy is a `dx_generated_policy`. `families` maps a family ID to the tool IDs it selects per capability (`lint`, `typecheck`, `format`, `audit`), the capabilities it turns off in `disabled`, and an optional tier `profile`. A capability left unset, or a tool with no wired executable, fails analysis. `families` merges onto the curated defaults: a key replaces only that family. Set `defaults = False` to use only the listed families.

```python
load("@rules_dx//quality/private:generated_policy.bzl", "dx_generated_policy")

dx_generated_policy(
    name = "my_policy",
    families = {
        "toml": {
            "disabled": ["typecheck"],
            "format": ["taplo"],
            "lint": ["taplo"],
        },
    },
    profile = "standard",
)
```

`profile` (`standard` or `strict`) picks the tier base config each tier-active tool runs on when nothing binds a config for it. `configs` binds native config targets such as `ruff_config` for every target; a config here wins over the tier base, and a tool may be listed once. See [Tool config files](#tool-config-files).

```python
dx_generated_policy(
    name = "policy",
    configs = [":ruff_config"],
    profile = "strict",
)
```

These fail: an unknown key in a family spec, an unknown capability in `disabled`, a capability both disabled and selected, and a tool listed twice in `configs`.

### Default policy

| Family | Format | Lint | Typecheck |
| --- | --- | --- | --- |
| `astro` | `prettier` | `typos` | `tsc` |
| `cc` | `clang_format` | `clang_tidy`, `typos` | disabled |
| `core` | `yamlfmt` | `actionlint`, `zizmor` | disabled |
| `dockerfile` | disabled | `hadolint`, `typos` | disabled |
| `go` | `gofumpt` | `errcheck`, `govet`, `staticcheck`, `typos` | `gotype` |
| `graphql` | `prettier` | `typos` | disabled |
| `html` | `prettier` | `typos` | disabled |
| `java` | `google_java_format` | `pmd`, `spotbugs`, `typos` | `javac` |
| `javascript` | `biome` | `biome`, `typos` | disabled |
| `json` | `biome` | `biome` | disabled |
| `kotlin` | `ktfmt` | `detekt`, `typos` | `kotlinc` |
| `markdown` | disabled | `lychee`, `markdown_check`, `typos` | disabled |
| `mdx` | `remark` | `lychee`, `typos` | disabled |
| `python` | `ruff` | `ruff`, `typos` | `ty` |
| `rust` | `rustfmt` | `clippy`, `typos` | `rustc` |
| `scala` | `scalafmt` | `typos` | `scalac` |
| `shell` | `shfmt` | `shellcheck`, `typos` | disabled |
| `starlark` | `buildifier` | `buildifier`, `typos` | disabled |
| `svelte` | `prettier` | `typos` | disabled |
| `text` | disabled | `keep_sorted`, `typos` | disabled |
| `toml` | `taplo` | `taplo`, `typos` | disabled |
| `typescript` | `biome` | `biome`, `typos` | `tsc` |
| `vue` | `prettier` | `typos` | disabled |
| `yaml` | `yamlfmt` | `yamllint` | disabled |

Families outside this table are opt-in: select their tools in your own policy.

## Tools

The `Not on` column lists platforms where the tool does not run. Empty means all five: `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`. The [support matrix](../../support-matrix.md) lists each language's platforms.

### Override a tool

Every tool runs from a managed executable. To use your own build, pass its target after `--`. The tool ID stays the same, so findings read the same.

```sh
tools/dx lint -- --@rules_dx//config:tool_<name>=//tools:my_tool
```

Tools with an override flag: `actionlint`, `ruff`, `terraform`, `hadolint`, `opa`, `regal`, `gofumpt`, `gotype`, `qmlformat`, `qmllint`, `julia_format`, `zig_ast_check`, `zig_fmt`, `swift_format`, `swift_lint`, `dart`, `perlcritic`, `perltidy`, `erlfmt`, `elvis`, `dialyzer`, `jet`, `julialint`, `luals`, `phpcs`, `phpstan`, `sorbet`, `mix_format`, `credo`, `ormolu`, `nixfmt`, `flatc`.

The `actionlint`, `ruff`, `terraform`, `hadolint`, `opa`, `regal`, `ormolu` and `nixfmt` alternates must be a single executable file. The others must be executable targets. Anything else fails analysis with the tool named.

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
| `ldc` | Typecheck D | |
| `ffmt` | Format Fortran | |
| `fortitude` | Lint Fortran | |
| `flang` | Typecheck Fortran (opt-in) | |
| `swift_format` | Format Swift | |
| `swift_lint` | Lint Swift | |

`clang_syntax` replays each compile action with `-fsyntax-only`, using the command line Bazel configured. Headers are checked only when the target compiles them (`--features=parse_headers`). Toolchains without a readable command line (clang-cl, params files) fail with an explicit error. Select it with `typecheck = ["clang_syntax"]` on the `cc` family. It has no fix support.

`nvcc_syntax` replays each `CudaCompile` action with `-c` changed to `-cuda`. Device compilation still runs for every `-gencode` architecture, so the check is heavy. Select it with `typecheck = ["nvcc_syntax"]` on the `cuda` family. Params files and header-only `cuda_library` targets fail with an explicit error.

`clang_tidy` on MSVC toolchains runs in clang-cl mode and names flags it cannot model: `/Yc`, `/Yu`, `/clr`, `/ZW`.

`clang_tidy` uses a dx default `.clang-tidy` when no `clang_tidy_config` is bound. It enables `misc-unused-parameters`, `misc-unused-using-decls` and `readability-redundant-declaration`. Findings from these checks are reported as info.

`standard` and `strict` clang-tidy tiers live in `quality/tiers/clang_tidy/` as `standard.clang-tidy` and `strict.clang-tidy`. Standard turns on `bugprone-*`, `performance-*` and the unused checks. Strict adds `modernize-*`, `readability-*`, `cppcoreguidelines-*`, `misc-*` and `objc-*`. clang-tidy cannot extend a file at another path, so a tier is used as a full copy: bind it with `clang_tidy_config`, and copy it to the root `.clang-tidy` for clangd. Neither file sets `WarningsAsErrors`.

`cppcheck` enables `warning,performance,portability` in the standard profile and adds `style` in strict. `spotbugs` runs at default effort in standard and max effort in strict, and the profile's filter XML sets the confidence threshold. A bound `spotbugs_config` is used as an `-exclude` filter with `-low`, so it exposes every low-confidence finding.

### JVM and .NET

| Tool | Checks | Not on |
| --- | --- | --- |
| `google_java_format` | Format Java | |
| `pmd`, `spotbugs` | Lint Java | |
| `javac` | Typecheck Java | |
| `ktfmt` | Format Kotlin | |
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
| `pydoclint` | Lint Python | |
| `ty` | Typecheck Python and Jupyter | |
| `rubocop` | Format and lint Ruby | `windows_x86_64`, `windows_arm64` |
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
| `eslint` | Lint Astro, JavaScript, MDX, Svelte, TSX, TypeScript, Vue | |
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
| `actionlint` | Lint GitHub workflows | |
| `zizmor` | Lint GitHub workflows | |
| `ansible_lint` | Lint Ansible | `windows_x86_64`, `windows_arm64` |
| `kubeconform` | Validate Kubernetes manifests | |
| `helm` | Lint Helm charts and validate rendered manifests | |
| `terraform` | Format and typecheck Terraform | |
| `tflint` | Lint Terraform | |
| `bicep` | Format and lint Bicep | |
| `cfn_lint` | Lint CloudFormation | |
| `trivy_config` | Lint Dockerfiles, Kubernetes, Helm charts, Terraform, CloudFormation and Bicep (through `bicep build`) for misconfigurations; opt-in | |
| `nixfmt` | Format Nix | all but `linux_x86_64` |
| `deadnix` | Lint Nix | |
| `opa` | Format and typecheck Rego | |
| `regal` | Lint Rego | |
| `cue` | Format and typecheck CUE | |
| `pkl` | Format and typecheck Pkl | |
| `pkl_lsp` | Lint Pkl | |
| `jsonnetfmt`, `jsonnet_lint` | Format and lint Jsonnet | |
| `redocly` | Lint OpenAPI and AsyncAPI | |
| `gplint` | Lint Gherkin | |
| `notebook_outputs` | Lint Jupyter notebooks for stored outputs | |
| `keep_sorted` | Lint `keep-sorted` blocks in text | |
| `typos` | Spell check source, docs and text, and fix with `--apply`. File name typos are reported but `--apply` does not rename files. Add words to a `_typos.toml` at the repo root | |

### Schema languages

| Tool | Checks | Not on |
| --- | --- | --- |
| `buf` | Format and lint Protocol Buffers; breaking changes with `dx lint --breaking-against` | |
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

Select `flatc` in a `flatbuffers` family of your policy. The family is opt-in.

```python
load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")
load("@rules_dx//quality/private:generated_policy.bzl", "dx_generated_policy")

dx_generated_policy(
    name = "policy",
    families = {
        "flatbuffers": {"lint": ["flatc"]},
    },
    profile = "standard",
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

Each tool reads its own native config file at the repo root. The table lists the file, whether dx can extend a tier base from it, and where the tier comes from. Tag a target `no-lint` or `no-format` to skip it.

A bound ruff config runs with its own directory as the project root, the same as ruff's own discovery, so `src`, `exclude` and `per-file-ignores` resolve against that directory.

### Change a rule

Edit the tool's own root file. Where the Base column says `extends`, the file extends the dx tier base, so your edit is the only thing you keep. Where it says `override copy`, the root file is a copy of the base, and you edit the copy.

<!-- tool-config-table:begin (generated from quality/tools.bzl; run `bazel run //docs:update_tool_configs`) -->

| Tool | Level | Native file(s) | Base | Tier source | Shared with |
| --- | --- | --- | --- | --- | --- |
| `ansible_lint` | catalog | `*.ansible-lint`, `*.yml`, `*.yaml` | none | ansible-lint moderate and production profiles |  |
| `bicep` | curated | `bicepconfig.json` | none | dx |  |
| `biome` | curated | `*.json` | none | dx |  |
| `buf` | catalog | `buf.yaml` | override copy | dx |  |
| `buildifier` | curated | `.buildifier.json` | override copy | dx |  |
| `cfn_lint` | curated | `*.cfnlintrc`, `*.yaml`, `*.yml` | none | dx |  |
| `checkmake` | catalog | `checkmake.ini` | override copy | dx |  |
| `checkstyle` | catalog | `*.xml` | none | dx |  |
| `clang_format` | curated | `*.clang-format` | none | dx |  |
| `clang_tidy` | curated | `*.clang-tidy` | none | dx |  |
| `clippy` | curated |  | none | dx |  |
| `cmakelint` | catalog | `.cmakelintrc` | override copy | dx |  |
| `cppcheck` | catalog | `*.txt` | none | dx |  |
| `credo` | catalog | `*.exs` | none | dx |  |
| `csharpier` | catalog | `*.yaml` | none |  |  |
| `dart` | curated | `analysis_options.yaml` | extends | dx |  |
| `deadnix` | curated | `*.txt` | none | dx |  |
| `detekt` | curated | `*.yml`, `*.yaml` | none | dx |  |
| `djlint` | catalog | `djlint.toml`, `*.djlintrc` | none | dx |  |
| `elvis` | catalog | `elvis.config`, `*.config` | none | dx |  |
| `errcheck` | curated | `*.txt` | none | dx |  |
| `eslint` | catalog | `eslint.config.mjs`, `eslint.config.js`, `eslint.config.cjs`, `eslint.config.ts`, `.eslintrc*` | extends | dx |  |
| `ffmt` | curated | `ffmt.toml` | none | dx |  |
| `fortitude` | curated | `*.toml` | none | dx |  |
| `fsharplint` | catalog | `*.json` | none |  |  |
| `gersemi` | catalog | `.gersemirc` | override copy | dx |  |
| `govet` | curated | `*.json` | none | dx |  |
| `gplint` | catalog | `*.gplintrc`, `*.json` | none | dx |  |
| `graphql_eslint` | catalog | `*.json` | none | dx |  |
| `hadolint` | curated | `*.yaml`, `*.yml` | none | dx |  |
| `hlint` | catalog | `.hlint.yaml` | override copy | dx |  |
| `julialint` | curated | `*.toml` | none | dx |  |
| `kubeconform` | curated | `kubernetes.kubeconform.json` | none | dx |  |
| `latexindent` | catalog | `*.yaml`, `*.yml` | none | dx |  |
| `luals` | catalog | `*.json` | none | dx |  |
| `markdown_check` | curated | `*.txt` | none | dx |  |
| `mbake` | catalog | `.bake.toml` | override copy | dx |  |
| `perlcritic` | catalog | `*.perlcriticrc`, `perlcriticrc` | none | dx |  |
| `phpcs` | catalog | `*.xml`, `*.xml.dist` | none | dx |  |
| `phpstan` | catalog | `*.neon`, `*.neon.dist` | none | phpstan level 6 and phpstan-strict-rules |  |
| `pmd` | curated | `*.xml` | none | dx |  |
| `prettier` | curated | `*.prettierrc`, `*.json`, `*.json5`, `*.yaml`, `*.yml`, `*.toml`, `*.mjs`, `*.cjs`, `*.js` | none | dx |  |
| `psscriptanalyzer` | catalog | `PSScriptAnalyzerSettings.psd1` | override copy | dx |  |
| `pydoclint` | catalog | `*.toml` | override copy | dx |  |
| `qmlformat` | catalog | `*.qmlformat.ini` | none | dx |  |
| `qmllint` | catalog | `*.qmllint.ini` | none | dx |  |
| `redocly` | catalog | `*.yaml`, `*.yml` | none | redocly spec and all rulesets |  |
| `regal` | catalog | `.regal/config.yaml` | none | dx |  |
| `remark` | curated | `*.json` | none | dx |  |
| `roslyn` | catalog | `*.globalconfig` | none |  |  |
| `rubocop` | catalog | `*.yml`, `*.yaml` | none | dx |  |
| `ruff` | curated | `ruff.toml`, `.ruff.toml` | extends | dx |  |
| `rustfmt` | curated | `*.toml` | none | dx |  |
| `scalafix` | catalog | `*.conf` | none | dx |  |
| `scalafmt` | curated | `*.conf` | none | dx |  |
| `selene` | catalog | `*.toml` | none | dx |  |
| `shellcheck` | curated | `*.shellcheckrc`, `shellcheckrc` | none | dx |  |
| `spotbugs` | curated | `*.xml` | none | dx |  |
| `sqlfluff` | catalog | `.sqlfluff` | override copy | dx |  |
| `staticcheck` | curated | `staticcheck.conf` | none | dx |  |
| `stylelint` | catalog | `.stylelintrc.json`, `.stylelintrc*`, `stylelint.config.*` | extends | dx |  |
| `stylua` | catalog | `*.toml` | none | dx |  |
| `swift_format` | catalog | `*.swift-format`, `*.json` | none | dx |  |
| `swift_lint` | catalog | `*.swift-format`, `*.json` | none | dx |  |
| `taplo` | curated | `.taplo.toml` | override copy | dx |  |
| `tflint` | catalog | `*.hcl` | none | dx |  |
| `trivy_config` | catalog | `*.txt` | none | dx |  |
| `tsc` | curated | `tsconfig.json` | extends | dx |  |
| `ty` | curated | `*.toml` | none | dx |  |
| `typos` | curated | `typos.toml` | override copy | dx |  |
| `vale` | catalog | `*.ini` | none | dx |  |
| `yamlfmt` | curated | `.yamlfmt` | override copy | dx |  |
| `yamllint` | curated | `.yamllint` | override copy | dx |  |
| `zanuda` | catalog | `*.txt` | none | dx |  |
| `zizmor` | curated | `*.txt` | none | dx |  |

<!-- tool-config-table:end -->

Formatters run on their tool defaults. dx ships no style for them, and `dx init` writes no formatter file.
