# Quality commands

Use `dx lint`, `dx typecheck` and `dx format` to find and fix code problems before review.

`dx lint` runs linters. `dx typecheck` runs type checkers and compilers in check mode. `dx format` checks formatting. Each reports what it finds and writes nothing until you pass `--apply`.

```sh
tools/dx lint //...
tools/dx lint --affected
tools/dx typecheck --fail-on error //cli/...
tools/dx format --here
tools/dx format --apply //cli/...
```

## Usage

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
| `--breaking-against[=REF]` | `lint` only. Also run `buf breaking` against the merge base of `HEAD` and `REF` (default `origin/HEAD`). `dx` builds the base image with the pinned `buf` and passes it to the lint action as the `//config:buf_breaking_against` label; the action stays sandboxed and cached. |
| `--online` | `lint` only. Check external links too. After the build, `dx` runs the `lychee` stage of each lint request on the host with `http` and `https` links enabled; the build stays offline and cached. Needs network. |
| `--diff` | Print a unified patch of the proposed fixes instead of findings. |
| `--fail-on info\|warning\|error` | Severity that fails the run. Default `warning`. |
| `--report sarif=<dest>` | Write a SARIF report. Repeatable. `lint` and `typecheck` only; `format` has no report. |
| `-o, --output text\|json` | Result shape. Default `text`. |
| `--include-edits` | Add `source_digest` and full `edits` to JSON `change` events. Needs `--output json`, else exit `2`. |
| `--max-findings <n>` | Print at most `n` `diagnostic` events per tool in JSON output. Default `50`; `0` prints all. Needs `--output json`, else exit `2`. A `findings_truncated` notice says how many were left out, and `command_finished` carries `findings.omitted`, `findings.by_tool` and `findings.by_rule` with exact counts. |
| `--here` | Scope to the current directory tree. |
| `--affected` | Scope to targets your changes reach. With none, prints `no affected targets` and exits `0`. With `--diff`, the line goes to stderr and stdout stays empty. |
| `--since <ref>` | With `--affected`: diff from this ref instead of the merge-base. Without `--affected`, exit `2`. |
| `--base <ref>` | With `--affected`: take the merge-base with this branch instead of the default branch. Not with `--since`. |
| `--workspace <dir>` | Use this workspace dir. |
| `-q, --quiet` | Hide summaries. Findings and failures still print. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots auto\|n\|off` | Set build slots. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. Use it for `--output_base` and `--output_user_root`. |

## Scopes

No scope means `//...`, every target in the workspace. A Bazel target is one buildable unit, such as a library or a test. Pass Bazel labels such as `//cli:dx` or patterns such as `//cli/...`, or workspace-relative files and dirs. `--here` cannot be combined with explicit scopes or with `--affected`. `--affected` with scopes keeps only the affected targets inside them. When every scope is a file or directory, findings and fixes are limited to those paths; the rest of the owning target is checked but neither reported nor rewritten. A `clippy.toml` scope stands for its package. A file that a `.bazelrc` flag names, or one that reaches such a target, stands for `//...`. A label or pattern among the scopes reports its whole target. With `--affected`, `--apply` writes only the changed files and the `BUILD` file of each one's package. A fix to any other file is refused as `outside_scope`. Targets that do not fit the target platform are skipped, even when named. See [Scope defaults](scope-defaults.md).

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

Lines and columns count Unicode characters. A finding without a position prints `path:`. A finding without a file prints `(no file):`. Identical findings print once with `(xN)`. Text ends with a summary on stderr, such as `2 error(s), 5 warning(s) in 4 file(s), 3 fixable with --apply`, or `no findings in N target(s)` when clean, where N counts the targets a tool reported on, followed by a `dx: <total> actions, ...` line. With color on, the severity word is red for errors and yellow for warnings.

When files need changes, text prints `N file(s) need changes. Apply them with: dx <command> --apply ...`. Failures print a `next:` line with the fix.

`--diff` prints the patch on stdout. `Not applied:` and `Conflict:` lines go to stderr.

JSON prints one `change` per file with `path`, `kind`, `edit_count` and `effect` (`proposed`, `applied` or `refused`). A refused change has a `reason`: `stale_source`, `unreadable_source`, `invalid_edits`, `write_failed`, `incomplete_collection`, `conflicting_digests`, `conflicting_candidates`, `outside_scope` or `outside_workspace`. A fix to a linked file is written to the file the link points to; `outside_workspace` means the link points outside the workspace, so the fix is not written. A check with proposed changes adds an info `notice` with code `changes_proposed`; its `message` is the `--apply` command. A tool that was skipped on this host adds a warning `notice` with code `tool_skipped`; its `message` names the tool and the reason, as the text-mode `dx: note:` line does. Findings from several targets print once with a `count` and every target in `origins`.

A stdout SARIF report (`--report sarif=-`) works only with text output and moves findings to stderr. Relative destinations anchor at the workspace root. Colliding destinations and missing parent dirs exit `2`.

Failures:

- `dx: incomplete_results: <reason>` (JSON: an `incomplete_results` error) means a target failed, a result was invalid, or the build events ended early. Fixes are never applied.
- `dx: not_converged: <target> ... did not converge: OSCILLATION|ITERATION_LIMIT` means a target's fixes oscillated or hit the ten-round limit. That target proposes no fixes.
- A failed build reports `bazel_failed` for compile errors, or `analysis_failed`, with a `build_failure` event or a `dx: build failure:` line.

Several targets that fix the same file apply identical fixes once. Fixes that disagree leave the file unchanged and print a `Conflict:` line.

`--apply` removes unused `deps` entries and adds missing direct deps in one write per BUILD file. Missing deps are added only for main-repo labels; a finding for an external repository stays reported.

`dx lint` without `--apply` also fails with exit `1` when the checkout holds build output or a file over 1 MiB, as `dx check` does.

### Severities and rule IDs

A finding is `info`, `warning` or `error`. `--fail-on` fails the run when any finding is at or above the given severity: `info` fails on everything, `warning` (default) on warnings and errors, `error` only on errors.

The text `[tool/rule]` suffix is the tool name and its rule code, for example `ruff/F401`. JSON diagnostics carry them as separate `tool` and `rule` fields.

### Silence one finding

dx has no suppression of its own. Use the tool's native inline form:

| Tool | Form |
| --- | --- |
| ruff | `# noqa: <code>` |
| clippy | `#[expect(clippy::<lint>, reason = "...")]`; the strict tier denies `#[allow]` |
| biome | `// biome-ignore <rule>: <reason>` |
| staticcheck | `//lint:ignore <check> <reason>` |
| shellcheck | `# shellcheck disable=SC<n>` |

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Findings at or above `--fail-on`; any proposed change in a check; an incomplete result set; a target that did not converge; a conflicting or unapplied fix; a failed report write. |
| `2` | Usage or scope error. |
| Bazel's code | Bazel failed. |
| `128`+N | A signal stopped dx. |

## Choose tools and tier

Pick the tier and the tools in `dx.toml` at the workspace root. dx writes
`.dx/BUILD.bazel` from it, with the workspace policy and a config target for
each root tool config. Never edit `.dx/`; it is generated.

```toml
schema_version = 1
tier = "standard"

[families.toml]
lint = ["taplo"]
format = ["taplo"]
typecheck = []

[families.go]
disabled = true

[scope]
exclude = ["//third_party/..."]
```

`tier` (`standard` or `strict`) picks the base config each tool runs on. A
family can set its own `tier`. `[families.<id>]` replaces the tools of one
capability (`lint`, `format`, `typecheck`, `audit`); a capability left out
keeps the default tools. `disabled = true` turns the family off. `[scope]
exclude` lists Bazel label patterns dx leaves out. Without `dx.toml`, every
family found in the tree runs the default tools at the strict tier. Set
`tier = "standard"` to opt down. See the
[config reference](config.md#dxtoml) for every key.

To change a rule, edit the tool's root config file. See
[Tool config files](#tool-config-files).

### Default tools

| Family | Format | Lint | Typecheck |
| --- | --- | --- | --- |
| `astro` | `prettier` | `typos` | `tsc` |
| `bicep` | `bicep` | `bicep`, `trivy_config`, `typos` | disabled |
| `cc` | `clang_format` | `clang_tidy`, `typos` | disabled |
| `cloudformation` | disabled | `cfn_lint`, `trivy_config`, `typos` | disabled |
| `core` | `yamlfmt` | `actionlint`, `zizmor` | disabled |
| `dockerfile` | disabled | `hadolint`, `typos` | disabled |
| `go` | `gofumpt` | `errcheck`, `govet`, `staticcheck`, `typos` | `gotype` |
| `graphql` | `prettier` | `typos` | disabled |
| `helm` | disabled | `helm`, `trivy_config`, `typos` | disabled |
| `html` | `prettier` | `typos` | disabled |
| `java` | `google_java_format` | `pmd`, `spotbugs`, `typos` | `javac` |
| `javascript` | `biome` | `biome`, `typos` | disabled |
| `json` | `biome` | `biome` | disabled |
| `kotlin` | `ktfmt` | `detekt`, `typos` | `kotlinc` |
| `kubernetes` | disabled | `kubeconform`, `trivy_config`, `typos` | disabled |
| `markdown` | disabled | `lychee`, `markdown_check`, `typos` | disabled |
| `mdx` | `remark` | `lychee`, `typos` | disabled |
| `objc` | `clang_format` | `typos` | `clang_syntax` |
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

Families outside this table are opt-in.

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
| `clang_tidy` | Lint C, C++, CUDA, Objective-C | Objective-C: not on Linux or Windows |
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
| `swiftc` | Typecheck Swift (opt-in) | |

`clang_syntax` replays each compile action with `-fsyntax-only`, using the command line Bazel configured. Headers are checked only when the target compiles them (`--features=parse_headers`). Toolchains without a readable command line (clang-cl, params files) fail with an explicit error. Select it with `typecheck = ["clang_syntax"]` on the `cc` family. It is the default typecheck of the `objc` family. It has no fix support.

`nvcc_syntax` replays each `CudaCompile` action with `-c` changed to `-cuda`. Device compilation still runs for every `-gencode` architecture, so the check is heavy. Select it with `typecheck = ["nvcc_syntax"]` on the `cuda` family. Params files and header-only `cuda_library` targets fail with an explicit error.

`clang_tidy` on MSVC toolchains runs in clang-cl mode and names flags it cannot model: `/Yc`, `/Yu`, `/clr`, `/ZW`.

`clang_tidy` runs on the tier base of `config_tier` unless a `clang_tidy_config` is bound.

`standard` and `strict` clang-tidy tiers live in `core/quality/tiers/clang_tidy/` as `standard.clang-tidy` and `strict.clang-tidy`. Standard turns on `bugprone-*`, `performance-*` and the unused checks. Strict adds `modernize-*`, `readability-*`, `cppcoreguidelines-*`, `misc-*` and `objc-*`. clang-tidy cannot extend a file at another path, so a tier is used as a full copy: bind it with `clang_tidy_config`, and copy it to the root `.clang-tidy` for clangd. Neither file sets `WarningsAsErrors`.

`clippy` takes its tier from `core/quality/tiers/clippy/`. Standard runs `clippy::all` with warnings as errors. Strict adds `clippy::pedantic` and denies `unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented`, `exit`, `indexing_slicing`, `unwrap_in_result`, `panic_in_result_fn`, `get_unwrap`, `string_slice`, `fallible_impl_from`, `wildcard_enum_match_arm`, `let_underscore_must_use`, `undocumented_unsafe_blocks`, `allow_attributes_without_reason`, `allow_attributes`, `match_wildcard_for_single_variants`, `let_underscore_future`, `let_underscore_untyped`, `unchecked_time_subtraction`, `large_futures`, `try_err`, `infinite_loop`, `cast_possible_truncation`, `cast_possible_wrap`, `mem_forget`, `rc_mutex`, `multiple_unsafe_ops_per_block`, `string_add`, `create_dir`, `iter_over_hash_type`, `map_err_ignore`, `unused_result_ok`, and the rustc lints `unsafe_code`, `unsafe_op_in_unsafe_fn` and `unused_must_use`. `rust_test` targets and `testonly` crates skip `unwrap_used`, `expect_used`, `panic`, `unreachable`, `exit`, `indexing_slicing`, `unwrap_in_result`, `panic_in_result_fn`, `get_unwrap` and `string_slice`. Mark a needed exception with `#[expect(<lint>, reason = "...")]` on the item.

`zig_ast_check` in the strict tier also fails on these patterns. Comments and strings never match.

| Rule | Pattern |
| --- | --- |
| `catch_unreachable` | `catch unreachable` or `catch { unreachable; }` |
| `empty_catch` | `catch {}` |
| `debug_print` | `std.debug.print` outside a `test` block, also as `@import("std").debug.print` or through a `const` alias of `std` or `std.debug` |
| `todo_panic` | `@panic("TODO...")` |
| `unsafe_cast` | `@ptrCast`, `@alignCast` or `@intCast` without a `// SAFETY:` comment in the statement or in the comment lines right above it. A struct init or `switch` body counts as part of the statement that holds it |

`zig`, `zig_ast_check` and `zig_fmt` report every finding as an error.

`hadolint` in the strict tier also fails on these patterns, each as an error.

| Rule | Pattern |
| --- | --- |
| `guard/reasonless_ignore` | `# hadolint ignore=` or `# hadolint global ignore=` with no reason. Add one after the rule list: `# hadolint ignore=DL3008 # apt pins drift weekly` |
| `guard/unpinned_image` | `FROM` an image without an `@sha256:` digest. `scratch` and earlier build stages pass, and `ARG` defaults declared before the first `FROM` are expanded first |
| `guard/tls_off` | A `curl` or `wget` call in `RUN` that turns off certificate checks with curl's insecure option or wget's no-check-certificate option |

`kubeconform` runs with `-strict` in the strict tier, so duplicate keys fail; the tier comes from the build, not from `kubernetes_schemas`, which carries only the schemas and `skip_kinds`.

`perlcritic` takes its tier from `core/quality/tiers/perlcritic/`. Standard runs severity 3 policies. Strict runs severity 1 policies and requires `use strict` and `use warnings`, checked `open`, `close` and other builtin calls, three-argument `open` with lexical handles, and a policy name on every `## no critic`. It bans string `eval`, `return undef`, and `no strict` or `no warnings` without a category. It leaves out layout and style noise such as `ProhibitMagicNumbers`, `RequireTrailingCommas`, `ProhibitPostfixControls`, `RequireExtendedFormatting`, `RequirePodSections` and `RequireArgUnpacking`.

`shellcheck` reports every finding as a `warning`, style findings included, so they fail the default `--fail-on`. Both tiers set `external-sources=true`, so shellcheck follows `source` lines. Strict also turns on the optional checks `add-default-case`, `check-extra-masked-returns`, `check-set-e-suppressed`, `check-unassigned-uppercase` and `deprecate-which`.

`shfmt` runs in simplify mode, so it also simplifies code. It picks the shell dialect from the shebang or the file extension.

`vale` in the strict tier runs the `Vale`, `write-good` and `Dx` styles at `MinAlertLevel = warning`, with `write-good.Passive` and `write-good.E-Prime` off and spelling left to `typos`. `Dx` findings are errors: the markers `TODO`, `FIXME`, `TBD` and `XXX`; the placeholders `lorem ipsum`, `your-*-here`, `<placeholder>` and `as an AI`; and the filler words `it is worth noting`, `delve`, `seamless`, `robust`, `leverage`, `comprehensive` and `game-changer`. Other `vale` findings are warnings.

`typos` in the strict tier checks file names and contents in the `en-us` locale, includes hidden files, and skips lockfiles and generated files.

`hlint` takes its tier from `core/quality/tiers/hlint/`. Standard ignores the naming, section, lambda-case, tuple-section and bracket hints. Strict turns on the `extra`, `generalise` and `future` groups and bans the functions `head`, `tail`, `init`, `last`, `fromJust`, `!!`, `read`, `error`, `undefined`, `unsafePerformIO`, `unsafeCoerce`, `trace`, `traceShow`, `traceM`, `foldl1`, `foldr1`, `maximum`, `minimum` and `genericIndex`, the modules `Debug.Trace`, `System.IO.Unsafe` and `Unsafe.Coerce`, and every `OPTIONS_GHC` flag; set compiler flags in `ghcopts`. These bans and the hints `Use fromMaybe`, `Unused LANGUAGE pragma`, `Redundant bracket`, `Redundant $`, `Redundant id`, `Use null`, `Use isNothing` and `Use isJust` are errors.

`spotbugs` runs at default effort in standard and max effort in strict, and the tier's filter XML sets the confidence threshold. A bound `spotbugs_config` is used as an `-exclude` filter with `-low`, so it exposes every low-confidence finding.

`credo` takes its tier from `core/quality/tiers/credo/`. Standard runs the warning and refactor checks, including `IoInspect`, `Dbg`, `IoPuts`, `UnsafeToAtom`, `UnsafeExec`, `MixEnv` and `LeakyEnvironment`. Strict runs Credo in strict mode and adds the readability and design checks, including `Specs`, `ImplTrue`, `WithCustomTaggedTuple`, `SkipTestWithoutComment`, `TagTODO`, `TagFIXME` and `UnusedVariableNames`.

`elvis` takes its tier from `core/quality/tiers/elvis/`. Standard runs the `erl_files` ruleset, which already bans debug calls, `catch`, `throw`, `if`, `import`, nested `try`, receive without timeout and used ignored variables. Strict runs `erl_files_strict`, which adds `always_shortcircuit`, `no_common_caveats_call`, the function length and complexity limits, and the other stricter rules. Both tiers turn off the layout rules that `erlfmt` owns.

`dialyzer` runs the same in both tiers with `-Wunmatched_returns`, `-Wmissing_return`, `-Wextra_return`, `-Werror_handling` and `-Wunderspecs`. Its findings are errors.

With warnings as errors on, `erlc` compiles with `-Werror`, `+warn_untyped_record`, `+warn_unused_import` and `+warn_export_all`. Test apps, `testonly` bytecode and Common Test suites get `+nowarn_export_all` in its place. An `erlang_app` that lists `srcs`, as Gazelle writes them, compiles with the upstream rules_erlang defaults and gets none of these flags. Third-party Hex apps build with `warnings_as_errors = False`. `warnings_as_errors = False` on a target, or `--@rules_dx//config:warnings_as_errors=false`, turns all of them off.

`govet`, `errcheck` and `staticcheck` take their tier from `core/quality/tiers/`. Standard runs the `go vet` defaults, errcheck, and staticcheck's default checks. Strict adds the vet analyzers `forvar`, `nilness`, `shadow` and `unusedwrite`, plus `bodyclose`, `errorlint`, `exhaustive`, `noctx` and `sqlclosecheck`. It also makes errcheck report `_ = f()`, unchecked type assertions such as `v.(string)`, and errors from `Close` and `fmt.Fprint*`. staticcheck runs the same checks in both tiers and leaves out `SA9003`, `ST1000`, `ST1003`, `ST1016` and `ST1020` to `ST1023`. Its `SA` findings are errors; `S`, `ST` and `U1000` findings are warnings.

`fortitude` takes its tier from `core/quality/tiers/fortitude/`. Standard runs a fixed list of correctness, obsolescence and portability rules. Strict runs every stable rule with a 100-column line limit and leaves out `C021`, `C022`, `C031`, `C032`, `C183`, `MOD001`, `MOD002`, `S082`, `S102` to `S105`, `S211`, `S212`, `S221`, `S251` to `S255`, `S271`, `S901` and `S902`. Its `C`, `OB` and `PORT` findings are errors; the rest are warnings.

`ty` takes its tier from `core/quality/tiers/ty/`. Standard uses the ty default rule levels. Strict sets every rule to `error` except `division-by-zero`, so a blanket `# ty: ignore`, an unused suppression comment and `disjoint-cast` fail, and `error-on-warning` fails the run on any warning.

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
| `erlc` | Typecheck Erlang | `windows_x86_64`, `windows_arm64` |
| `elixirc` | Typecheck Elixir | `windows_x86_64`, `windows_arm64` |
| `ormolu` | Format Haskell | |
| `hlint` | Lint Haskell | |
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
| `taplo` | Format, lint and typecheck TOML (schema validation from `#:schema` or `.taplo.toml` rules) | |
| `yamlfmt`, `yamllint` | Format and lint YAML | |
| `xmllint` | Lint XML | |
| `prettier` | Format XML, Ansible, Kubernetes, Dockerfile, Gherkin, PHP, API specs | |
| `sqlfluff` | Format and lint SQL | |
| `buildifier` | Format and lint Starlark | |
| `cmakelint`, `gersemi` | Lint and format CMake | |
| `checkmake`, `mbake` | Lint and format Make | |
| `docstrfmt` | Format reStructuredText | |
| `hadolint` | Lint Dockerfiles | |
| `actionlint` | Lint GitHub workflows | |
| `zizmor` | Lint GitHub workflows | |
| `ansible_lint` | Lint Ansible | `windows_x86_64`, `windows_arm64` |
| `kubeconform` | Validate Kubernetes manifests | |
| `helm` | Lint Helm charts and validate rendered manifests | |
| `helmfmt` | Format Helm chart templates | |
| `terraform` | Format and typecheck Terraform | |
| `tflint` | Lint Terraform | |
| `bicep` | Format and lint Bicep | |
| `cfn_lint` | Lint CloudFormation | |
| `trivy_config` | Lint Dockerfiles, Kubernetes, Helm charts, Terraform, CloudFormation and Bicep (through `bicep build`) for misconfigurations at every severity under the strict tier, `HIGH` and `CRITICAL` under the standard tier | |
| `trivy_image_config` | Lint the user, healthcheck and secrets in the image config of an `oci_image_from_binary` image, offline | |
| `container_structure_test` | Typecheck the config, files and licenses of an `oci_image_from_binary` image against its `structure_tests` configs with the tar driver, without a container runtime | |
| `nixfmt` | Format Nix | `macos_arm64`, `windows_x86_64`, `windows_arm64` |
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
| `typos` | Spell check source, docs and text, and fix with `--apply`. File name typos are reported but `--apply` does not rename files. Add words to `typos.toml` at the repo root | |

### Schema languages

| Tool | Checks | Not on |
| --- | --- | --- |
| `buf` | Format, lint and typecheck (`buf build`) Protocol Buffers; breaking changes with `dx lint --breaking-against` | |
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

Select `flatc` in a `flatbuffers` family in `dx.toml`. The family is opt-in. See the [config reference](config.md#dxtoml).

```toml
[families.flatbuffers]
lint = ["flatc"]
```

```python
load("@rules_dx//flatbuffers:defs.bzl", "flatbuffers_library")

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
load("@rules_dx//flatbuffers:defs.bzl", "flatbuffers_conform_test")

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

A bound regal config may not set `capabilities.from.url`, use YAML aliases, merge keys or tags. `capabilities.from.file` must be a relative forward-slash path without `.` or `..` segments. Use `capabilities.from.engine` and `version` to pin capabilities.

A bound or root `buf.yaml` may not list `deps`, in v1 or v2, or in a v2 `modules` entry, and may not use YAML aliases, merge keys or tags. `dx` runs buf offline. Declare imported protos as `proto_library` deps instead.

trivy config and terraform typecheck read a Terraform file (`.tf` or `.tf.json`) only when every `module` it calls has a plain `./` or `../` string `source` that stays inside the target's files, and the same holds for every local module those call. Otherwise the file gets a `dx/terraform-module-source` error and the tool skips it (terraform typecheck skips that whole directory), because the tool would download the module and `dx` runs them offline. terraform typecheck also checks the `module` blocks in the `run` blocks of test files (`.tftest.hcl` or `.tftest.json`) in the directory and in its `tests` directory, and the Terraform files a dependency keeps in the same directory. A problem in a dependency's file lands on one of your files and names the dependency's file. Vendor the module into your repo and point `source` at that directory.

### Change a rule

Edit the tool's own root file. Where the Base column says `extends`, the file extends the dx tier base, so your edit is the only thing you keep. Where it says `override copy`, the root file is a copy of the base, and you edit the copy.

See [standard tiers](../../tiers-standard.md) and [strict tiers](../../tiers-strict.md) for what each tier turns on.

<!-- tool-config-table:begin (generated from core/quality/tools.bzl; run `bazel run //docs:update_tool_configs`) -->

| Tool | Level | Native file(s) | Base | Tier source | Shared with |
| --- | --- | --- | --- | --- | --- |
| `ansible_lint` | catalog | `*.ansible-lint`, `*.yml`, `*.yaml` | none | ansible-lint moderate and production profiles |  |
| `bicep` | curated | `bicepconfig.json` | none | dx |  |
| `biome` | curated | `biome.json` | none | dx |  |
| `buf` | catalog | `buf.yaml` | override copy | dx |  |
| `buildifier` | curated | `.buildifier.json` | override copy | dx |  |
| `cfn_lint` | curated | `*.cfnlintrc`, `*.yaml`, `*.yml` | none | dx |  |
| `checkmake` | catalog | `checkmake.ini` | override copy | dx |  |
| `clang_format` | curated | `.clang-format` | none | dx |  |
| `clang_tidy` | curated | `*.clang-tidy` | none | dx |  |
| `clippy` | curated |  | none | dx |  |
| `cmakelint` | catalog | `.cmakelintrc` | override copy | dx |  |
| `credo` | catalog | `*.exs` | none | dx |  |
| `csharpier` | catalog | `*.yaml` | none | dx |  |
| `dart` | curated | `analysis_options.yaml` | extends | dx |  |
| `deadnix` | curated | `*.txt` | none | dx |  |
| `detekt` | curated | `*.yml`, `*.yaml` | none | dx |  |
| `djlint` | catalog | `djlint.toml`, `*.djlintrc` | none | dx |  |
| `docstrfmt` | catalog | `docstrfmt.toml` | override copy | dx |  |
| `elvis` | catalog | `elvis.config`, `*.config` | none | dx |  |
| `errcheck` | curated | `*.txt` | none | dx |  |
| `eslint` | catalog | `eslint.config.mjs`, `eslint.config.js`, `eslint.config.cjs`, `eslint.config.ts`, `.eslintrc*` | extends | dx |  |
| `ffmt` | curated | `ffmt.toml` | none | dx |  |
| `fortitude` | curated | `*.toml` | none | dx |  |
| `fsharplint` | catalog | `*.json` | none | dx |  |
| `gersemi` | catalog | `.gersemirc` | override copy | dx |  |
| `govet` | curated | `*.json` | none | dx |  |
| `gplint` | catalog | `*.gplintrc`, `*.json` | none | dx |  |
| `graphql_eslint` | catalog | `*.json` | none | dx |  |
| `hadolint` | curated | `*.yaml`, `*.yml` | none | dx |  |
| `helmfmt` | catalog | `.helmfmt` | none | dx |  |
| `hlint` | catalog | `.hlint.yaml` | override copy | dx |  |
| `julialint` | curated | `*.toml` | none | dx |  |
| `kubeconform` | curated | `kubernetes.kubeconform.json` | none | dx |  |
| `latexindent` | catalog | `*.yaml`, `*.yml` | none | dx |  |
| `luals` | catalog | `*.json` | none | dx |  |
| `lychee` | curated | `lychee.toml` | override copy | dx |  |
| `markdown_check` | curated | `*.txt` | none | dx |  |
| `mbake` | catalog | `.bake.toml` | override copy | dx |  |
| `mix_format` | catalog | `.formatter.exs` | override copy | dx |  |
| `perlcritic` | catalog | `*.perlcriticrc`, `perlcriticrc` | none | dx |  |
| `perltidy` | catalog | `.perltidyrc` | override copy | dx |  |
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
| `roslyn` | catalog | `*.globalconfig` | none | dx |  |
| `rubocop` | catalog | `*.yml`, `*.yaml` | none | dx |  |
| `ruff` | curated | `ruff.toml`, `.ruff.toml` | extends | dx |  |
| `rustfmt` | curated | `rustfmt.toml`, `.rustfmt.toml` | none | dx |  |
| `scalafix` | catalog | `*.conf` | none | dx |  |
| `scalafmt` | curated | `.scalafmt.conf` | none | dx |  |
| `selene` | catalog | `*.toml` | none | dx |  |
| `shellcheck` | curated | `.shellcheckrc` | none | dx |  |
| `shfmt` | curated | `.editorconfig` | override copy | dx |  |
| `spotbugs` | curated | `*.xml` | none | dx |  |
| `sqlfluff` | catalog | `.sqlfluff` | override copy | dx |  |
| `staticcheck` | curated | `staticcheck.conf` | none | dx |  |
| `stylelint` | catalog | `.stylelintrc.json`, `.stylelintrc*`, `stylelint.config.*` | extends | dx |  |
| `stylua` | catalog | `*.toml` | none | dx |  |
| `swift_format` | catalog | `*.swift-format`, `*.json` | none | dx |  |
| `swift_lint` | catalog | `*.swift-format`, `*.json` | none | dx |  |
| `taplo` | curated | `.taplo.toml` | override copy | dx |  |
| `tflint` | catalog | `*.hcl` | none | dx |  |
| `trivy_config` | curated | `*.txt` | none | dx |  |
| `tsc` | curated | `tsconfig.json` | extends | dx |  |
| `ty` | curated | `*.toml` | none | dx |  |
| `typos` | curated | `typos.toml` | override copy | dx |  |
| `vale` | catalog | `.vale.ini`, `_vale.ini` | none | dx |  |
| `yamlfmt` | curated | `.yamlfmt` | override copy | dx |  |
| `yamllint` | curated | `.yamllint` | override copy | dx |  |
| `zanuda` | catalog | `*.txt` | none | dx |  |
| `zizmor` | curated | `*.txt` | none | dx |  |

<!-- tool-config-table:end -->

Formatters run on their tool defaults. dx ships no style for them, and `dx init` writes no formatter file.
