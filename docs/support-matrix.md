# Support matrix

This page lists, for each language and target, the platforms it does not support, and its lint, format, and typecheck tools.

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
| CUDA | `linux_arm64`, `windows_arm64`, `windows_x86_64`: not tested yet; `macos_arm64`: NVIDIA ships no CUDA toolkit for macOS |
| Dart | `windows_arm64`: rules_dart 0.8.1 ships no Windows arm64 SDK |
| Elixir | `windows_x86_64`, `windows_arm64`: not tested yet |
| Erlang | `windows_x86_64`, `windows_arm64`: not tested yet |
| Fortran | `windows_arm64`: flang-releases v21.1.8 ships no Windows arm64 build |
| F# | none |
| Go | none |
| Haskell | `windows_x86_64`: not tested yet; `windows_arm64`: GHC 9.10.3 ships no Windows arm64 bindist |
| Java | none |
| JavaScript | none |
| Julia | `windows_arm64`: not tested yet |
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

## Quality

Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs. A tool runs on every platform its row supports unless a note says otherwise.

| Name | Lint | Format | Typecheck |
| --- | --- | --- | --- |
| Astro | `eslint` | `prettier` | no |
| C and C++ | `clang_tidy`, `cppcheck` | `clang_format` | `clang_syntax` (not on `linux_x86_64`: objc_library builds only with an Apple toolchain; `linux_arm64`: objc_library builds only with an Apple toolchain; `windows_x86_64`: objc_library builds only with an Apple toolchain; `windows_arm64`: objc_library builds only with an Apple toolchain) |
| C# | `roslyn` | `csharpier` | `csc` |
| CUDA | `clang_tidy` | `clang_format` | `nvcc_syntax` |
| D | `dscanner` | `dfmt` | `ldc` |
| Dart | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) |
| Elixir | `credo` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `mix_format` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `dialyzer` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) |
| Erlang | `elvis` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `erlfmt` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) | `dialyzer` (not on `windows_x86_64`: No OTP Windows cell is pinned; `windows_arm64`: No OTP Windows cell is pinned) |
| Fortran | `fortitude` | `ffmt` | `flang` |
| F# | `fsharplint` | `fantomas` | `fsc` |
| Go | `errcheck`, `govet`, `staticcheck` | `gofumpt` | `gotype` |
| Haskell | `hlint` (not on `linux_arm64`: hlint publishes no linux_arm64 build; `macos_arm64`: hlint v3.10 ships only x86_64-osx and no macOS fallback exists) | `ormolu` (not on `linux_arm64`: ormolu publishes no linux_arm64 build; `macos_arm64`: The aarch64-darwin zip needs sibling dylibs the standalone artifact model does not expose) | `ghc` |
| Java | `checkstyle`, `pmd`, `spotbugs` | `google_java_format` | `javac` |
| JavaScript | `biome`, `eslint` | `biome`, `prettier` | `tsc` |
| Julia | no | `julia_format` (not on `windows_arm64`: Julia ships no windows_arm64 runtime) | `jet` |
| Kotlin | `ktlint` | `ktfmt` | `kotlinc` |
| Lua | `selene` | `stylua` | `luals` |
| MDX | `eslint` | `remark` | `tsc` |
| OCaml | `zanuda` (not on `linux_arm64`: zanuda is built only for Linux x86_64; `macos_arm64`: zanuda is built only for Linux x86_64; `windows_x86_64`: zanuda is built only for Linux x86_64; `windows_arm64`: zanuda is built only for Linux x86_64) | `topiary` | `ocaml` |
| Perl | `perlcritic` (not on `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build) | `perltidy` (not on `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build) | n/a |
| PHP | `phpcs` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build) | `prettier` | `phpstan` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build) |
| PowerShell | `psscriptanalyzer` (not on `windows_x86_64`: No PSScriptAnalyzer artifact is pinned for Windows; `windows_arm64`: No PSScriptAnalyzer artifact is pinned for Windows) | `psscriptanalyzer` (not on `windows_x86_64`: No PSScriptAnalyzer artifact is pinned for Windows; `windows_arm64`: No PSScriptAnalyzer artifact is pinned for Windows) | n/a |
| Python | `flake8`, `pydoclint`, `pylint`, `ruff` | `ruff` | `ty` |
| Ruby | `rubocop` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter) | `standardrb` (not on `windows_x86_64`: Ruby portable builds ship no Windows interpreter; `windows_arm64`: Ruby portable builds ship no Windows interpreter) | `sorbet` (not on `windows_x86_64`: Sorbet ships no Windows build; `windows_arm64`: Sorbet ships no Windows build) |
| Rust | `clippy` | `rustfmt` | `rustc` |
| Scala | `scalafix` | `scalafmt` | `scalac` |
| Svelte | `eslint` | `prettier` | no |
| TypeScript | `biome` | `biome`, `prettier` | `tsc` |
| Vue | `eslint` | `prettier` | no |
| Zig | `zig_ast_check` | `zig_fmt` | `zig` |
| Apple | `clang_tidy` | `clang_format` | `clang_syntax` (not on `linux_x86_64`: objc_library builds only with an Apple toolchain; `linux_arm64`: objc_library builds only with an Apple toolchain; `windows_x86_64`: objc_library builds only with an Apple toolchain; `windows_arm64`: objc_library builds only with an Apple toolchain) |
| CMake | `cmakelint` | `gersemi` | n/a |
| Dockerfile | `hadolint` | `prettier` | n/a |
| Make | `checkmake` | `mbake` | n/a |
| Qt | `qmllint` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | `qmlformat` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) | `qmllint` (not on `windows_x86_64`: The Qt Windows x86_64 cell has a blocker; `windows_arm64`: The Qt Windows arm64 cell has a blocker) |
| Ansible | `ansible_lint` (not on `windows_x86_64`: ansible-core does not run on Windows; `windows_arm64`: ansible-core does not run on Windows) | `prettier` | n/a |
| API specs | `redocly` | `prettier` | n/a |
| AsciiDoc | `vale` | no | n/a |
| Avro | `avro_tools` | n/a | `avro_tools` |
| Bicep | `bicep` | `bicep` | n/a |
| Cap'n Proto | `capnp` (not on `linux_arm64`: The capnp compiler is built only for Linux x86_64; `macos_arm64`: The capnp compiler is built only for Linux x86_64; `windows_x86_64`: The capnp compiler is built only for Linux x86_64) | n/a | `capnp` (not on `linux_arm64`: The capnp compiler is built only for Linux x86_64; `macos_arm64`: The capnp compiler is built only for Linux x86_64; `windows_x86_64`: The capnp compiler is built only for Linux x86_64) |
| CloudFormation | `cfn_lint` | no | n/a |
| CUE | n/a | `cue` | `cue` |
| FlatBuffers | `flatc` | n/a | `flatc` |
| Gherkin | `gplint` | `prettier` | n/a |
| Helm | `helm` | no | n/a |
| HTML templates | `djlint` | `djlint` | n/a |
| Jsonnet | `jsonnet_lint` | `jsonnetfmt` | n/a |
| Jupyter | `notebook_outputs`, `ruff` | `ruff` | `ty` |
| Kubernetes | `kubeconform` | `prettier` | n/a |
| LaTeX | `chktex` | `latexindent` | n/a |
| Nix | `deadnix`, `statix` | `nixfmt` (not on `linux_arm64`: nixfmt ships only a Linux x86_64 binary; `macos_arm64`: nixfmt ships only a Linux x86_64 binary; `windows_x86_64`: nixfmt ships only a Linux x86_64 binary; `windows_arm64`: nixfmt ships only a Linux x86_64 binary) | n/a |
| Pkl | no | `pkl` | `pkl` |
| Rego | `regal` | `opa` | `opa` |
| reStructuredText | `vale` | no | n/a |
| SQL | `sqlfluff` | `prettier`, `sqlfluff` | n/a |
| Terraform | `tflint` | `terraform` | `terraform` |
| Thrift | `thrift` (not on `linux_arm64`: The thrift compiler is built only for Linux x86_64; `macos_arm64`: The thrift compiler is built only for Linux x86_64; `windows_x86_64`: The thrift compiler is built only for Linux x86_64) | n/a | `thrift` (not on `linux_arm64`: The thrift compiler is built only for Linux x86_64; `macos_arm64`: The thrift compiler is built only for Linux x86_64; `windows_x86_64`: The thrift compiler is built only for Linux x86_64) |
| Typst | no | `typstyle` | n/a |
| XML | `xmllint` | `prettier` | n/a |
| YAML | `yamllint` | `yamlfmt` | n/a |

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
