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
| C and C++ | none |
| C# | none |
| CUDA | `linux_arm64`, `windows_x86_64`, `windows_arm64`: not tested yet; `macos_arm64`: NVIDIA ships no CUDA toolkit for macOS |
| D | `windows_x86_64`, `windows_arm64`: not tested yet |
| Dart | `windows_arm64`: rules_dart 0.8.1 ships no Windows arm64 SDK |
| Elixir | none |
| Erlang | none |
| Fortran | `windows_arm64`: flang-releases v21.1.8 ships no Windows arm64 build |
| F# | none |
| Go | none |
| Haskell | `windows_x86_64`: not tested yet; `windows_arm64`: GHC 9.10.3 ships no Windows arm64 bindist |
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
| Qt | `windows_x86_64`, `windows_arm64`: not tested yet |
| WASI | none |

## Compilers

Select one with `--@rules_dx//cc/toolchains:compiler=<value>`.

| Compiler | Not supported on |
| --- | --- |
| GCC 16.2.0 (`gcc`) | `linux_arm64`: not tested yet; `macos_arm64`, `windows_x86_64`, `windows_arm64`: The GCC backend builds only for Linux |
| LLVM (`llvm`, default) | none |

## Quality

Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs, and the tool that builds its API docs. A tool runs on every platform its row supports unless a note says otherwise.

| Name | Lint | Format | Typecheck | API docs |
| --- | --- | --- | --- | --- |
| Astro | `eslint` | `prettier` | no | `astro-compiler` |
| C and C++ | `clang_tidy`, `cppcheck` | `clang_format` | `clang_syntax` (not on `linux_x86_64`: objc_library builds only with an Apple toolchain; `linux_arm64`: objc_library builds only with an Apple toolchain; `windows_x86_64`: objc_library builds only with an Apple toolchain; `windows_arm64`: objc_library builds only with an Apple toolchain) | no |
| C# | `roslyn` | `csharpier` | `csc` | `csharp doc driver` |
| CUDA | `clang_tidy` | `clang_format` | `nvcc_syntax` | no |
| D | `dscanner` | `dfmt` | `ldc` | no |
| Dart | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | no |
| Elixir | `credo` | `mix_format` | `dialyzer` (not on `windows_x86_64`: dialyzer on Windows is pending #6008; `windows_arm64`: dialyzer on Windows is pending #6321) | no |
| Erlang | `elvis` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `erlfmt` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `dialyzer` (not on `windows_x86_64`: dialyzer on Windows is pending #6008; `windows_arm64`: dialyzer on Windows is pending #6321) | no |
| Fortran | `fortitude` | `ffmt` | `flang` | no |
| F# | `fsharplint` | `fantomas` | `fsc` | `fsharp doc driver` |
| Go | `errcheck`, `govet`, `staticcheck` | `gofumpt` | `gotype` | `go doc` |
| Haskell | `hlint` (not on `linux_arm64`: hlint publishes no linux_arm64 build; `macos_arm64`: hlint v3.10 ships only x86_64-osx and no macOS fallback exists) | `ormolu` (not on `linux_arm64`: ormolu publishes no linux_arm64 build; `macos_arm64`: The aarch64-darwin zip needs sibling dylibs the standalone artifact model does not expose) | `ghc` | no |
| Java | `checkstyle`, `pmd`, `spotbugs` | `google_java_format` | `javac` | `javadoc` |
| JavaScript | `biome`, `eslint` | `biome`, `prettier` | `tsc` | no |
| Julia | no | `julia_format` (not on `windows_arm64`: Julia ships no windows_arm64 runtime) | `jet` | no |
| Kotlin | `ktlint` | `ktfmt` | `kotlinc` | `dokka` |
| Lua | `selene` | `stylua` | `luals` | no |
| MDX | `eslint` | `remark` | `tsc` | `mdx` |
| OCaml | `zanuda` (not on `linux_arm64`: zanuda is built only for Linux x86_64; `macos_arm64`: zanuda is built only for Linux x86_64; `windows_x86_64`: zanuda is built only for Linux x86_64; `windows_arm64`: zanuda is built only for Linux x86_64) | `topiary` | `ocaml` | no |
| Perl | `perlcritic` (not on `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build) | `perltidy` (not on `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build) | n/a | no |
| PHP | `phpcs` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build) | `prettier` | `phpstan` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build) | no |
| PowerShell | `psscriptanalyzer` (not on `windows_x86_64`: No PSScriptAnalyzer artifact is pinned for Windows; `windows_arm64`: No PSScriptAnalyzer artifact is pinned for Windows) | `psscriptanalyzer` (not on `windows_x86_64`: No PSScriptAnalyzer artifact is pinned for Windows; `windows_arm64`: No PSScriptAnalyzer artifact is pinned for Windows) | n/a | `pwsh` |
| Python | `flake8`, `pydoclint`, `pylint`, `ruff` | `ruff` | `ty` | `griffe` |
| Ruby | `rubocop` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter) | `standardrb` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter) | `sorbet` (not on `windows_x86_64`: Sorbet ships no Windows build; `windows_arm64`: Sorbet ships no Windows build) | `yard` |
| Rust | `clippy` | `rustfmt` | `rustc` | `rustdoc` |
| Scala | `scalafix` | `scalafmt` | `scalac` | `scaladoc` |
| Svelte | `eslint` | `prettier` | `tsc` | `svelte2tsx` |
| TypeScript | `biome` | `biome`, `prettier` | `tsc` | `typedoc` |
| Vue | `eslint` | `prettier` | `tsc` | `vue-component-meta` |
| Zig | `zig_ast_check` | `zig_fmt` | `zig` | no |
| Apple | `clang_tidy` | `clang_format` | `clang_syntax` (not on `linux_x86_64`: objc_library builds only with an Apple toolchain; `linux_arm64`: objc_library builds only with an Apple toolchain; `windows_x86_64`: objc_library builds only with an Apple toolchain; `windows_arm64`: objc_library builds only with an Apple toolchain) | n/a |
| CMake | `cmakelint` | `gersemi` | n/a | n/a |
| Dockerfile | `hadolint` | `prettier` | n/a | n/a |
| Make | `checkmake` | `mbake` | n/a | n/a |
| Qt | `qmllint` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | `qmlformat` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | `qmllint` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | n/a |
| Ansible | `ansible_lint` (not on `windows_x86_64`: ansible-core does not run on Windows; `windows_arm64`: ansible-core does not run on Windows) | `prettier` | n/a | n/a |
| API specs | `redocly` | `prettier` | n/a | n/a |
| AsciiDoc | `vale` | no | n/a | n/a |
| Avro | `avro_tools` | n/a | `avro_tools` | n/a |
| Bicep | `bicep` | `bicep` | n/a | n/a |
| Cap'n Proto | `capnp` (not on `linux_arm64`: The capnp compiler is built only for Linux x86_64; `macos_arm64`: The capnp compiler is built only for Linux x86_64; `windows_x86_64`: The capnp compiler is built only for Linux x86_64) | n/a | `capnp` (not on `linux_arm64`: The capnp compiler is built only for Linux x86_64; `macos_arm64`: The capnp compiler is built only for Linux x86_64; `windows_x86_64`: The capnp compiler is built only for Linux x86_64) | n/a |
| CloudFormation | `cfn_lint` | no | n/a | n/a |
| CUE | n/a | `cue` | `cue` | n/a |
| FlatBuffers | `flatc` | n/a | `flatc` | n/a |
| Gherkin | `gplint` | `prettier` | n/a | n/a |
| Helm | `helm` | no | n/a | n/a |
| HTML templates | `djlint` | `djlint` | n/a | n/a |
| Jsonnet | `jsonnet_lint` | `jsonnetfmt` | n/a | n/a |
| Jupyter | `notebook_outputs`, `ruff` | `ruff` | `ty` | n/a |
| Kubernetes | `kubeconform` | `prettier` | n/a | n/a |
| LaTeX | `chktex` | `latexindent` | n/a | n/a |
| Nix | `deadnix`, `statix` | `nixfmt` (not on `linux_arm64`: nixfmt ships only a Linux x86_64 binary; `macos_arm64`: nixfmt ships only a Linux x86_64 binary; `windows_x86_64`: nixfmt ships only a Linux x86_64 binary; `windows_arm64`: nixfmt ships only a Linux x86_64 binary) | n/a | n/a |
| Pkl | no | `pkl` | `pkl` | n/a |
| Rego | `regal` | `opa` | `opa` | n/a |
| reStructuredText | `vale` | no | n/a | n/a |
| SQL | `sqlfluff` | `prettier`, `sqlfluff` | n/a | n/a |
| Terraform | `tflint` | `terraform` | `terraform` | n/a |
| Thrift | `thrift` (not on `linux_arm64`: The thrift compiler is built only for Linux x86_64; `macos_arm64`: The thrift compiler is built only for Linux x86_64; `windows_x86_64`: The thrift compiler is built only for Linux x86_64) | n/a | `thrift` (not on `linux_arm64`: The thrift compiler is built only for Linux x86_64; `macos_arm64`: The thrift compiler is built only for Linux x86_64; `windows_x86_64`: The thrift compiler is built only for Linux x86_64) | n/a |
| Typst | `tinymist` | `typstyle` | n/a | n/a |
| XML | `xmllint` | `prettier` | n/a | n/a |
| YAML | `yamllint` | `yamlfmt` | n/a | n/a |

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
