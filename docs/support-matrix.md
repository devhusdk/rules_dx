# Support matrix

This page lists, for each language, target, and compiler, the platforms it does not support, and its lint, format, typecheck, and API doc tools.

Everything builds and tests on every platform it supports. `none` means all five platforms are supported.

`n/a` means the capability does not apply to that language or format.

The platforms are `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`.

Do not edit this page. Run `bazel run //tools/docs/support_matrix:update` to refresh it.

## Languages

| Language | Not supported on |
| --- | --- |
| Astro | none |
| Avro | none |
| C and C++ | none |
| C# | none |
| CUDA | `macos_arm64`: NVIDIA ships no CUDA toolkit for macOS; `windows_x86_64`, `windows_arm64`: not tested yet |
| D | `windows_x86_64`, `windows_arm64`: not tested yet |
| Dart | `windows_arm64`: rules_dart 0.8.1 ships no Windows arm64 SDK |
| Elixir | none |
| Erlang | none |
| Flutter | `windows_arm64`: rules_flutter 0.0.8 and Flutter 3.47.8 ship no Windows arm64 host SDK (flutter/private/toolchains_repo.bzl HOST_PLATFORMS) |
| Fortran | `windows_arm64`: flang-releases v21.1.8 ships no Windows arm64 build |
| F# | none |
| Go | none |
| Haskell | `windows_arm64`: GHC 9.10.3 ships no Windows arm64 bindist |
| Java | none |
| JavaScript | none |
| Julia | `windows_arm64`: julialang.org and rules_julia 0.8.0 ship no Windows arm64 Julia build |
| Kotlin | none |
| Lua | none |
| MDX | none |
| OCaml | `windows_x86_64`: conda-forge win-64 OCaml stops at 5.3.0, below the pinned 5.5.2; `windows_arm64`: conda-forge ships no win-arm64 OCaml package |
| Perl | `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build |
| PHP | `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build |
| PowerShell | none |
| Protobuf | none |
| Python | none |
| Ruby | none |
| Rust | none |
| Scala | none |
| Svelte | none |
| TypeScript | none |
| Vue | none |
| Zig | none |

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

Select one with `--@rules_dx//cc/toolchains:compiler=<value>`.

| Compiler | Driver | Coverage | Not supported on |
| --- | --- | --- | --- |
| GCC 16.2.0 (`gcc`) | gnu | gcov | `macos_arm64`, `windows_x86_64`, `windows_arm64`: The GCC backend builds only for Linux |
| LLVM 23.1.2 (`llvm`, default) | gnu | llvm | none |
| Microsoft cl.exe (`msvc`) | msvc | no | `linux_x86_64`, `linux_arm64`, `macos_arm64`: Microsoft cl.exe runs only on Windows; `windows_arm64`: not tested yet |

## Quality

Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs, and the tool that builds its API docs. A tool runs on every platform its row supports unless a note says otherwise. Coverage names the collector that writes the LCOV for the row's tests, or `no` with the reason it has none.

| Name | Lint | Format | Typecheck | API docs | Coverage |
| --- | --- | --- | --- | --- | --- |
| Astro | `eslint`, `typos` | `prettier` | `tsc` | `astro-compiler` | no: js_test has no dx coverage wiring |
| Avro | `avro_tools`, `typos` | n/a | `avro_tools` | no | n/a |
| C and C++ | `clang_tidy`, `cppcheck`, `typos` | `clang_format` | `clang_syntax` | no | `gcov`, `llvm-cov` |
| C# | `roslyn`, `typos` | `csharpier` | `csc` | `csharp doc driver` | `coverlet` |
| CUDA | `clang_tidy`, `typos` | `clang_format` | `nvcc_syntax` | no | no: cuda_test has no dx coverage wiring |
| D | `dscanner`, `typos` | `dfmt` | `ldc` | no | `ldc --coverage` |
| Dart | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK), `typos` | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | no | `package:test` |
| Elixir | `credo`, `typos`, `xref` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `mix_format` | `dialyzer` (not on `windows_x86_64`: dialyzer on Windows is pending #6008; `windows_arm64`: dialyzer on Windows is pending #6321) | no | `cover` |
| Erlang | `elvis` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned), `typos`, `xref` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `erlfmt` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `dialyzer` | no | `cover` |
| Flutter | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK), `typos` | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | no | `flutter_tester` |
| Fortran | `fortitude`, `typos` | `ffmt` | `flang` | no | no: no collector yet; tests carry dx-no-coverage-collector (#2308) |
| F# | `fsharplint`, `typos` | `fantomas` | `fsc` | `fsharp doc driver` | `coverlet` |
| Go | `errcheck`, `govet`, `staticcheck`, `typos` | `gofumpt` | `gotype` | `go doc` | `go cover` |
| Haskell | `hlint` (not on `linux_arm64`: hlint publishes no linux_arm64 build; `macos_arm64`: hlint v3.10 ships only x86_64-osx and no macOS fallback exists), `typos` | `ormolu` (not on `linux_arm64`: ormolu publishes no linux_arm64 build; `macos_arm64`: The aarch64-darwin zip needs sibling dylibs the standalone artifact model does not expose) | `ghc` | no | `hpc` |
| Java | `checkstyle`, `pmd`, `spotbugs`, `typos` | `google_java_format` | `javac` | `javadoc` | `jacoco` |
| JavaScript | `biome`, `eslint`, `knip`, `typos` | `biome`, `prettier` | `tsc` | no | `jest` |
| Julia | `julialint` (not on `windows_arm64`: Julia ships no windows_arm64 runtime), `typos` | `julia_format` (not on `windows_arm64`: Julia ships no windows_arm64 runtime) | `jet` | no | `CoverageTools` |
| Kotlin | `detekt`, `ktlint`, `typos` | `ktfmt` | `kotlinc` | `dokka` | `jacoco` |
| Lua | `selene`, `typos` | `stylua` | `luals` | no | `luacov` |
| MDX | `eslint`, `lychee`, `typos` | `remark` | `tsc` | `mdx` | no: js_test has no dx coverage wiring |
| OCaml | `typos`, `zanuda` (not on `linux_arm64`: zanuda is built only for Linux x86_64; `macos_arm64`: zanuda is built only for Linux x86_64; `windows_x86_64`: zanuda is built only for Linux x86_64; `windows_arm64`: zanuda is built only for Linux x86_64) | `topiary` | `ocaml` | no | no: no collector yet; tests carry dx-no-coverage-collector (#2308) |
| Perl | `perlcritic` (not on `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build), `typos` | `perltidy` (not on `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build) | n/a | no | no: no collector yet; tests carry dx-no-coverage-collector (#2308) |
| PHP | `phpcs` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build), `typos` | `prettier` | `phpstan` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build) | no | `pcov (Linux only)` |
| PowerShell | `psscriptanalyzer` (not on `windows_x86_64`: No PSScriptAnalyzer artifact is pinned for Windows; `windows_arm64`: No PSScriptAnalyzer artifact is pinned for Windows), `typos` | `psscriptanalyzer` (not on `windows_x86_64`: No PSScriptAnalyzer artifact is pinned for Windows; `windows_arm64`: No PSScriptAnalyzer artifact is pinned for Windows) | n/a | `pwsh` | `pester` |
| Protobuf | `buf`, `typos` | `buf` | no | no | n/a |
| Python | `flake8`, `pydoclint`, `pylint`, `ruff`, `typos`, `vulture` | `ruff` | `ty` | `griffe` | `coverage.py` |
| Ruby | `debride` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter), `rubocop` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter), `typos` | `standardrb` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter) | `sorbet` (not on `windows_x86_64`: Sorbet ships no Windows build; `windows_arm64`: Sorbet ships no Windows build) | `yard` | `simplecov` |
| Rust | `clippy`, `typos` | `rustfmt` | `rustc` | `rustdoc` | `llvm-cov` |
| Scala | `scalafix`, `typos` | `scalafmt` | `scalac` | `scaladoc` | `jacoco` |
| Svelte | `eslint`, `typos` | `prettier` | `tsc` | `svelte2tsx` | no: js_test has no dx coverage wiring |
| TypeScript | `biome`, `knip`, `typos` | `biome`, `prettier` | `tsc` | `typedoc` | `jest` |
| Vue | `eslint`, `typos` | `prettier` | `tsc` | `vue-component-meta` | no: js_test has no dx coverage wiring |
| Zig | `typos`, `zig_ast_check` | `zig_fmt` | `zig` | no | `kcov` (kcov runs on Linux only) |
| Apple | `clang_tidy`, `typos` | `clang_format` | `clang_syntax` (not on `linux_x86_64`: objc_library builds only with an Apple toolchain; `linux_arm64`: objc_library builds only with an Apple toolchain; `windows_x86_64`: objc_library builds only with an Apple toolchain; `windows_arm64`: objc_library builds only with an Apple toolchain) | n/a | no: no dx coverage wiring in the Apple rules |
| CMake | `cmakelint`, `typos` | `gersemi` | n/a | n/a | n/a |
| Dockerfile | `hadolint`, `typos` | `prettier` | n/a | n/a | n/a |
| Make | `checkmake`, `typos` | `mbake` | n/a | n/a | n/a |
| Qt | `qmllint` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker), `typos` | `qmlformat` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | `qmllint` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | n/a | no: no dx coverage wiring in the Qt rules |
| Ansible | `ansible_lint` (not on `windows_x86_64`: ansible-core does not run on Windows; `windows_arm64`: ansible-core does not run on Windows), `typos` | `prettier` | n/a | n/a | n/a |
| API specs | `redocly`, `typos` | `prettier` | n/a | n/a | n/a |
| AsciiDoc | `lychee`, `typos`, `vale` | no | n/a | n/a | n/a |
| Bicep | `bicep`, `typos` | `bicep` | n/a | n/a | n/a |
| Cap'n Proto | `capnp` (not on `linux_arm64`: The capnp compiler is built only for Linux x86_64; `macos_arm64`: The capnp compiler is built only for Linux x86_64; `windows_x86_64`: The capnp compiler is built only for Linux x86_64), `typos` | n/a | `capnp` (not on `linux_arm64`: The capnp compiler is built only for Linux x86_64; `macos_arm64`: The capnp compiler is built only for Linux x86_64; `windows_x86_64`: The capnp compiler is built only for Linux x86_64) | n/a | n/a |
| CloudFormation | `cfn_lint`, `typos` | no | n/a | n/a | n/a |
| CSS | `stylelint`, `typos` | `prettier` | no | n/a | n/a |
| CUE | n/a | `cue` | `cue` | n/a | n/a |
| FlatBuffers | `flatc`, `typos` | n/a | `flatc` | n/a | n/a |
| Gherkin | `gplint`, `typos` | `prettier` | n/a | n/a | n/a |
| GitHub workflows | no | `yamlfmt` | no | n/a | n/a |
| GraphQL | `graphql_eslint`, `typos` | `prettier` | no | n/a | n/a |
| Helm | `helm`, `typos` | no | n/a | n/a | n/a |
| HTML templates | `djlint`, `typos` | `djlint` | n/a | n/a | n/a |
| Jsonnet | `jsonnet_lint`, `typos` | `jsonnetfmt` | n/a | n/a | n/a |
| Jupyter | `notebook_outputs`, `ruff` | `ruff` | `ty` | n/a | n/a |
| Kubernetes | `kubeconform`, `typos` | `prettier` | n/a | n/a | n/a |
| LaTeX | `chktex`, `typos` | `latexindent` | n/a | n/a | n/a |
| Markdown | `lychee`, `markdown_check`, `typos`, `vale` | no | no | n/a | n/a |
| Nix | `deadnix`, `statix`, `typos` | `nixfmt` (not on `linux_arm64`: nixfmt ships only a Linux x86_64 binary; `macos_arm64`: nixfmt ships only a Linux x86_64 binary; `windows_x86_64`: nixfmt ships only a Linux x86_64 binary; `windows_arm64`: nixfmt ships only a Linux x86_64 binary) | n/a | n/a | n/a |
| OCI images | no | no | no | n/a | n/a |
| Pkl | `pkl_lsp` (not on `windows_x86_64`: pkl-lsp exits at start under the hermetic runner (#7292); `windows_arm64`: pkl-lsp exits at start under the hermetic runner (#7292)), `typos` | `pkl` | `pkl` | n/a | n/a |
| Rego | `regal`, `typos` | `opa` | `opa` | n/a | n/a |
| reStructuredText | `lychee`, `typos`, `vale` | no | n/a | n/a | n/a |
| Shell | `shellcheck`, `typos` | `shfmt` | no | n/a | n/a |
| SQL | `sqlfluff`, `typos` | `prettier`, `sqlfluff` | n/a | n/a | n/a |
| Starlark | `buildifier`, `typos` | `buildifier` | no | n/a | n/a |
| Terraform | `tflint`, `typos` | `terraform` | `terraform` | n/a | n/a |
| Thrift | `thrift` (not on `linux_arm64`: The thrift compiler is built only for Linux x86_64; `macos_arm64`: The thrift compiler is built only for Linux x86_64; `windows_x86_64`: The thrift compiler is built only for Linux x86_64), `typos` | n/a | `thrift` (not on `linux_arm64`: The thrift compiler is built only for Linux x86_64; `macos_arm64`: The thrift compiler is built only for Linux x86_64; `windows_x86_64`: The thrift compiler is built only for Linux x86_64) | n/a | n/a |
| TOML | `taplo`, `typos` | `taplo` | no | n/a | n/a |
| Typst | `tinymist`, `typos` | `typstyle` | n/a | n/a | n/a |
| XML | `typos`, `xmllint` | `prettier` | n/a | n/a | n/a |
| YAML | `typos`, `yamllint` | `yamlfmt` | n/a | n/a | n/a |

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
