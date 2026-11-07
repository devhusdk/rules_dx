# Support matrix

This page lists, for each language, target, and compiler, the platforms it does not support, and its lint, format, typecheck, and API doc tools.

Everything builds and tests on every platform it supports. `none` means all five platforms are supported.

`n/a` means the capability does not apply to that language or format.

The platforms are `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`.

Do not edit this page. Run `bazel run //tools/docs/support_matrix:update` to refresh it.

## Languages

| Language | Not supported on | Dependencies |
| --- | --- | --- |
| Astro | none | `npm` |
| Avro | none | none: schemas only, no packages |
| C and C++ | none | `conan` (pinned, no advisories); none for `vcpkg.json`: conan is the supported manager, there is no vcpkg set |
| C# | none | `nuget` |
| CUDA | `macos_arm64`: NVIDIA ships no CUDA toolkit for macOS; `windows_arm64`: not tested yet | none: the CUDA toolkit is a pinned download |
| D | none | `dub` (pinned, no advisories) |
| Dart | `windows_arm64`: rules_dart 0.8.1 ships no Windows arm64 SDK | `pub` |
| Elixir | none | `hex` |
| Erlang | none | `hex` from `mix.lock`; none for `rebar.lock`: `dx update` does not refresh it |
| Flutter | `windows_arm64`: rules_flutter 0.0.8 and Flutter 3.47.8 ship no Windows arm64 host SDK (flutter/private/toolchains_repo.bzl HOST_PLATFORMS) | `pub` |
| Fortran | `windows_arm64`: flang-releases v21.1.8 ships no Windows arm64 build | none: no package manager |
| F# | none | `nuget` |
| Go | none | `go` |
| Haskell | `windows_arm64`: GHC 9.10.3 ships no Windows arm64 bindist | `hackage` from `cabal.project.freeze`; none for `stack.yaml.lock`: only cabal has a set |
| Java | none | `maven` |
| JavaScript | none | `npm` |
| Julia | `windows_arm64`: julialang.org and rules_julia 0.8.0 ship no Windows arm64 Julia build | `julia` |
| Kotlin | none | `maven` |
| Lua | none | none: no LuaRocks support, rockspec packages are pinned with `http_archive` |
| MDX | none | `npm` |
| OCaml | `windows_x86_64`: conda-forge win-64 OCaml stops at 5.3.0, below the pinned 5.5.2; `windows_arm64`: conda-forge ships no win-arm64 OCaml package | `opam` |
| Perl | `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build | `cpan` |
| PHP | `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build | `packagist` |
| PowerShell | none | `powershell` (pinned, no advisories) |
| Protobuf | none | none: schemas only, no packages |
| Python | none | `uv`; none for `poetry.lock` and `Pipfile.lock`: uv is the supported manager |
| Ruby | none | `ruby` |
| Rust | none | `cargo` |
| Scala | none | `maven` |
| Svelte | none | `npm` |
| TypeScript | none | `npm` |
| Vue | none | `npm` |
| Zig | none | none for `build.zig.zon`: the zig family does not fetch `.dependencies` |

## Targets

| Target | Not supported on |
| --- | --- |
| Android | none |
| AppImage | `macos_arm64`, `windows_x86_64`, `windows_arm64`: AppImage is a Linux format |
| Apple | `windows_x86_64`, `windows_arm64`: Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS |
| CMake | none |
| Dockerfile | none |
| Make | none |
| MSIX | none |
| Qt | none |
| WASI | none |

## Compilers

Select one with `--@rules_dx//cc/toolchains:compiler=<value>`. Blocked on names a platform the compiler targets whose evidence is still waiting, with the reason.

| Compiler | Driver | Coverage | Not supported on | Blocked on |
| --- | --- | --- | --- | --- |
| GCC 16.2.0 (`gcc`) | gnu | gcov | `macos_arm64`, `windows_x86_64`, `windows_arm64`: The GCC backend builds only for Linux | none |
| LLVM 23.1.2 (`llvm`, default) | gnu | llvm | none | none |
| Microsoft cl.exe (`msvc`) | msvc | no | `linux_x86_64`, `linux_arm64`, `macos_arm64`: Microsoft cl.exe runs only on Windows | none |

## Quality

Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs, and the tool that builds its API docs. A tool runs on every platform its row supports unless a note says otherwise. Coverage names the collector that writes the LCOV for the row's tests, or `no` with the reason it has none. Dead code names the tool and rule that report unused code, `no` when none runs yet, or `none` when the language has no such tool.

| Name | Lint | Format | Typecheck | API docs | Coverage | Dead code |
| --- | --- | --- | --- | --- | --- | --- |
| Astro | `eslint`, `typos` | `prettier` | `tsc` | `astro-compiler` | no: js_test has no dx coverage wiring | no |
| Avro | `avro_tools`, `typos` | n/a | `avro_tools` | no | n/a | no |
| C and C++ | `clang_tidy`, `cppcheck`, `typos` | `clang_format` | `clang_syntax` | no | `gcov`, `llvm-cov` | `clang_tidy misc-unused-parameters, misc-unused-using-decls` |
| C# | `roslyn`, `typos` | `csharpier` | `csc` | `csharp doc driver` | `coverlet` | `roslyn IDE0051, IDE0052, IDE0060, IDE0005` |
| CUDA | `clang_tidy`, `typos` | `clang_format` | `nvcc_syntax` | no | no: cuda_test has no dx coverage wiring | none |
| D | `typos` | no | `ldc` | no | `ldc --coverage` | none |
| Dart | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK), `typos` | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | no | `package:test` | `dart unused_element` |
| Elixir | `credo`, `typos` | `mix_format` | `dialyzer` (not on `windows_x86_64`: dialyzer on Windows is pending #6008; `windows_arm64`: dialyzer on Windows is pending #6321) | no | `cover` | no |
| Erlang | `elvis` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned), `typos` | `erlfmt` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `dialyzer`, `erlc` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | no | `cover` | `erlc warn_unused_vars, warn_unused_function` |
| Flutter | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK), `typos` | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | no | `flutter_tester` | no |
| Fortran | `fortitude`, `typos` | `ffmt` | `flang` | no | `kcov` (kcov runs on Linux only) | `fortitude C191 unreachable-statement` |
| F# | `fsharplint`, `typos` | `fantomas` | `fsc` | `fsharp doc driver` | `coverlet` | no |
| Go | `errcheck`, `govet`, `staticcheck`, `typos` | `gofumpt` | `gotype` | `go doc` | `go cover` | `staticcheck U1000` |
| Haskell | `hlint` (not on `linux_arm64`: hlint publishes no linux_arm64 build; `macos_arm64`: hlint v3.10 ships only x86_64-osx and no macOS fallback exists), `typos`, `weeder` (not on `linux_arm64`: weeder is built from Hackage and only the linux_x86_64 build is verified; `macos_arm64`: weeder is built from Hackage and only the linux_x86_64 build is verified; `windows_x86_64`: weeder is built from Hackage and only the linux_x86_64 build is verified; `windows_arm64`: weeder is built from Hackage and only the linux_x86_64 build is verified) | `ormolu` (not on `linux_arm64`: ormolu publishes no linux_arm64 build; `macos_arm64`: The aarch64-darwin zip needs sibling dylibs the standalone artifact model does not expose) | `ghc` | no | `hpc` | `ghc -Wunused-local-binds, -Wunused-matches (-Wall)` |
| Java | `checkstyle`, `pmd`, `spotbugs`, `typos` | `google_java_format` | `javac` | `javadoc` | `jacoco` | `pmd UnusedLocalVariable, UnusedPrivateField, UnusedPrivateMethod, UnusedFormalParameter` |
| JavaScript | `biome`, `eslint`, `typos` | `biome` | `tsc` | no | `jest` | `eslint no-unused-vars` |
| Julia | `julialint` (not on `windows_arm64`: Julia ships no windows_arm64 runtime), `typos` | `julia_format` (not on `windows_arm64`: Julia ships no windows_arm64 runtime) | `jet` | no | `CoverageTools` | none |
| Kotlin | `detekt`, `typos` | `ktfmt` | `kotlinc` | `dokka` | `jacoco` | `detekt` |
| Lua | `selene`, `typos` | `stylua` | `luals` | no | `luacov` | `selene unused_variable` |
| MDX | `eslint`, `lychee`, `typos` | `remark` | `tsc` | `mdx` | no: js_test has no dx coverage wiring | no |
| OCaml | `typos`, `zanuda` (not on `linux_arm64`: zanuda is built only for Linux x86_64; `macos_arm64`: zanuda is built only for Linux x86_64; `windows_x86_64`: zanuda is built only for Linux x86_64; `windows_arm64`: zanuda is built only for Linux x86_64) | `topiary` | `ocaml` | no | `bisect_ppx` | `ocamlc warnings 26, 27, 32 to 39` |
| Perl | `perlcritic` (not on `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build), `typos` | `perltidy` (not on `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build) | n/a | no | `DxLcov` | `perlcritic ProhibitUnusedVariables` |
| PHP | `phpcs` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build), `typos` | `prettier` | `phpstan` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build) | no | `pcov (Linux only)` | `phpcs Generic.CodeAnalysis.UnusedFunctionParameter` |
| PowerShell | `psscriptanalyzer` (not on `windows_x86_64`: No PSScriptAnalyzer artifact is pinned for Windows; `windows_arm64`: No PSScriptAnalyzer artifact is pinned for Windows), `typos` | `psscriptanalyzer` (not on `windows_x86_64`: No PSScriptAnalyzer artifact is pinned for Windows; `windows_arm64`: No PSScriptAnalyzer artifact is pinned for Windows) | n/a | `pwsh` | `pester` | `psscriptanalyzer PSUseDeclaredVarsMoreThanAssignments` |
| Protobuf | `buf`, `typos` | `buf` | no | no | n/a | no |
| Python | `pydoclint`, `ruff`, `typos` | `ruff` | `ty` | `griffe` | `coverage.py` | `ruff F401, F841` |
| Ruby | `rubocop` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter), `typos` | `rubocop` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter) | `sorbet` (not on `windows_x86_64`: Sorbet ships no Windows build; `windows_arm64`: Sorbet ships no Windows build) | `yard` | `simplecov` | `rubocop Lint/UselessAssignment` |
| Rust | `clippy`, `typos` | `rustfmt` | `rustc` | `rustdoc` | `llvm-cov` | `rustc dead_code` |
| Scala | `scalafix`, `typos` | `scalafmt` | `scalac` | `scaladoc` | `jacoco` | `scalac -Wunused` |
| Svelte | `eslint`, `typos` | `prettier` | `tsc` | `svelte2tsx` | no: js_test has no dx coverage wiring | no |
| TypeScript | `biome`, `eslint`, `typos` | `biome` | `tsc` | `typedoc` | `jest` | `eslint no-unused-vars` |
| Vue | `eslint`, `typos` | `prettier` | `tsc` | `vue-component-meta` | no: js_test has no dx coverage wiring | no |
| Zig | `typos`, `zig_ast_check` | `zig_fmt` | `zig` | no | `kcov` (kcov runs on Linux only) | `zig ast-check unused local, unused parameter` |
| Apple | `clang_tidy`, `typos` | `clang_format` | `clang_syntax` (not on `linux_x86_64`: objc_library builds only with an Apple toolchain; `linux_arm64`: objc_library builds only with an Apple toolchain; `windows_x86_64`: objc_library builds only with an Apple toolchain; `windows_arm64`: objc_library builds only with an Apple toolchain) | n/a | no: no dx coverage wiring in the Apple rules | n/a |
| CMake | `cmakelint`, `typos` | `gersemi` | n/a | n/a | n/a | n/a |
| Dockerfile | `hadolint`, `typos` | `prettier` | n/a | n/a | n/a | n/a |
| Make | `checkmake`, `typos` | `mbake` | n/a | n/a | n/a | n/a |
| Qt | `qmllint` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker), `typos` | `qmlformat` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | `qmllint` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | n/a | no: no dx coverage wiring in the Qt rules | n/a |
| Ansible | `ansible_lint` (not on `windows_x86_64`: ansible-core does not run on Windows; `windows_arm64`: ansible-core does not run on Windows), `typos` | `yamlfmt` | n/a | n/a | n/a | n/a |
| API specs | `redocly`, `typos` | `prettier` | n/a | n/a | n/a | n/a |
| AsciiDoc | `lychee`, `typos`, `vale` | no | n/a | n/a | n/a | n/a |
| Bicep | `bicep`, `trivy_config`, `typos` | `bicep` | n/a | n/a | n/a | n/a |
| Cap'n Proto | `capnp` (not on `linux_arm64`: The capnp compiler is built only for Linux x86_64; `macos_arm64`: The capnp compiler is built only for Linux x86_64; `windows_x86_64`: The capnp compiler is built only for Linux x86_64), `typos` | n/a | `capnp` (not on `linux_arm64`: The capnp compiler is built only for Linux x86_64; `macos_arm64`: The capnp compiler is built only for Linux x86_64; `windows_x86_64`: The capnp compiler is built only for Linux x86_64) | n/a | n/a | n/a |
| CloudFormation | `cfn_lint`, `trivy_config`, `typos` | no | n/a | n/a | n/a | n/a |
| CSS | `stylelint`, `typos` | `prettier` | no | n/a | n/a | n/a |
| CUE | n/a | `cue` | `cue` | n/a | n/a | n/a |
| FlatBuffers | `flatc`, `typos` | n/a | `flatc` | n/a | n/a | n/a |
| Gherkin | `gplint`, `typos` | `prettier` | n/a | n/a | n/a | n/a |
| GitHub workflows | `actionlint`, `zizmor` | `yamlfmt` | no | n/a | n/a | n/a |
| GraphQL | `graphql_eslint`, `typos` | `prettier` | no | n/a | n/a | n/a |
| Helm | `helm`, `trivy_config`, `typos` | no | n/a | n/a | n/a | n/a |
| HTML templates | `djlint`, `typos` | `djlint` | n/a | n/a | n/a | n/a |
| Jsonnet | `jsonnet_lint`, `typos` | `jsonnetfmt` | n/a | n/a | n/a | n/a |
| Jupyter | `notebook_outputs`, `ruff` | `ruff` | `ty` | n/a | n/a | n/a |
| Kubernetes | `kubeconform`, `trivy_config`, `typos` | `yamlfmt` | n/a | n/a | n/a | n/a |
| LaTeX | `chktex`, `typos` | `latexindent` | n/a | n/a | n/a | n/a |
| Markdown | `lychee`, `markdown_check`, `typos`, `vale` | no | no | n/a | n/a | n/a |
| Nix | `deadnix`, `typos` | `nixfmt` (not on `linux_arm64`: nixfmt ships only a Linux x86_64 binary; `macos_arm64`: nixfmt ships only a Linux x86_64 binary; `windows_x86_64`: nixfmt ships only a Linux x86_64 binary; `windows_arm64`: nixfmt ships only a Linux x86_64 binary) | n/a | n/a | n/a | `deadnix` |
| OCI images | no | no | no | n/a | n/a | n/a |
| Pkl | `pkl_lsp`, `typos` | `pkl` | `pkl` | n/a | n/a | n/a |
| Rego | `regal`, `typos` | `opa` | `opa` | n/a | n/a | n/a |
| reStructuredText | `lychee`, `typos`, `vale` | no | n/a | n/a | n/a | n/a |
| Shell | `shellcheck`, `typos` | `shfmt` | no | n/a | n/a | `shellcheck SC2034` |
| SQL | `sqlfluff`, `typos` | `sqlfluff` | n/a | n/a | n/a | n/a |
| Starlark | `buildifier`, `typos` | `buildifier` | no | n/a | n/a | `buildifier unused-variable` |
| Terraform | `tflint`, `trivy_config`, `typos` | `terraform` | `terraform` | n/a | n/a | n/a |
| Thrift | `thrift` (not on `linux_arm64`: The thrift compiler is built only for Linux x86_64; `macos_arm64`: The thrift compiler is built only for Linux x86_64; `windows_x86_64`: The thrift compiler is built only for Linux x86_64), `typos` | n/a | `thrift` (not on `linux_arm64`: The thrift compiler is built only for Linux x86_64; `macos_arm64`: The thrift compiler is built only for Linux x86_64; `windows_x86_64`: The thrift compiler is built only for Linux x86_64) | n/a | n/a | n/a |
| TOML | `taplo`, `typos` | `taplo` | no | n/a | n/a | n/a |
| Typst | `tinymist`, `typos` | `typstyle` | n/a | n/a | n/a | n/a |
| XML | `typos`, `xmllint` | `prettier` | n/a | n/a | n/a | n/a |
| YAML | `typos`, `yamllint` | `yamlfmt` | n/a | n/a | n/a | n/a |

### Not applicable

Not applicable for `format`:

- `avro`: schema files; avro-tools has no formatter, it only validates
- `capnp`: schema IDL; no maintained .capnp formatter exists, capnp only validates
- `flatbuffers`: schema IDL; no maintained .fbs formatter exists, flatc only validates
- `thrift`: schema IDL; the thrift compiler has no formatter, it only validates

Not applicable for `lint`:

- `cue`: cue vet is the only CUE analysis and it is already the typecheck capability; the pinned cue binary has no separate linter

Not applicable for `typecheck`:

- `ansible`: dynamically typed YAML playbooks; no type system beyond ansible-lint
- `api_spec`: declarative API description; no type system beyond schema validation
- `asciidoc`: markup; no static type system
- `bicep`: declarative templates; bicep lint already reports compile errors and analyzer rules, so there is no separate type check
- `cloudformation`: declarative templates; cfn-lint schema validation is lint, not type checking
- `cmake`: no static type system; build language with untyped strings
- `dockerfile`: no static type system; hadolint covers lint
- `gherkin`: natural-language scenarios; no type system
- `helm`: chart lint plus schema validation of rendered manifests
- `html_template`: markup templates; no static type system
- `jsonnet`: dynamically typed; no maintained type checker (the official linter only flags unused variables)
- `kubernetes`: declarative manifests; schema validation is not type checking
- `latex`: typesetting macro language; no static type system
- `make`: no static type system; build language with untyped strings
- `nix`: dynamically typed expression language; no maintained type checker
- `perl`: dynamically typed; no maintained type checker
- `powershell`: no type checker; PSScriptAnalyzer is a linter only
- `rst`: markup; no static type system
- `sql`: type checking needs a database schema and dialect; standalone .sql files carry neither
- `typst`: dynamically typed markup; no standalone type checker
- `xml`: data format; schema validation is not type checking
- `yaml`: data format; schema validation is not type checking

## API compatibility check

The tool each publishing rule runs for `api_compat`, or `not available` when no maintained free tool exists.

| Rule | Tool |
| --- | --- |
| `crates_deploy` | `cargo-semver-checks` |
| `github_deploy` | `apidiff` |
| `hackage_deploy` | not available |
| `hex_deploy` | not available |
| `luarocks_deploy` | not available |
| `maven_deploy` | `japicmp` |
| `npm_deploy` | `api-extractor` |
| `nuget_deploy` | `Microsoft.DotNet.ApiCompat` |
| `pub_deploy` | `dart_apitool` |
| `pypi_deploy` | `griffe` |
| `rubygems_deploy` | not available |
