# Support matrix

This page lists, for each language and target, the platforms it does not support, and its lint, format, and typecheck tools.

Everything builds and tests on every platform it supports. `none` means all five platforms are supported. Missing tests on a supported platform are gaps, tracked as issues and listed by `bazel run //tools/docs/support_matrix:gaps`.

`n/a` means the capability does not apply to that language or format.

The platforms are `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`.

Do not edit this page. Run `bazel run //tools/docs/support_matrix:update` to refresh it.

## Languages

| Language | Not supported on |
| --- | --- |
| Astro | none |
| C and C++ | none |
| C# | none |
| CUDA | `linux_arm64`, `windows_x86_64`: not tested yet; `macos_arm64`: NVIDIA ships no CUDA toolkit for macOS; `windows_arm64`: NVIDIA ships no CUDA toolkit for Windows arm64 |
| D | `windows_x86_64`, `windows_arm64`: not tested yet |
| Dart | `windows_arm64`: rules_dart 0.8.0 ships no Windows arm64 SDK |
| Elixir | `windows_x86_64`, `windows_arm64`: not tested yet |
| Erlang | `windows_x86_64`, `windows_arm64`: not tested yet |
| Fortran | `windows_x86_64`: not tested yet; `windows_arm64`: flang-releases v21.1.8 ships no Windows arm64 build |
| F# | none |
| Go | none |
| Haskell | `windows_x86_64`: not tested yet; `windows_arm64`: GHC 9.10.3 ships no Windows arm64 bindist |
| Java | none |
| JavaScript | none |
| Julia | `windows_arm64`: not tested yet |
| Kotlin | none |
| Lua | none |
| MDX | none |
| OCaml | `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`: not tested yet |
| Perl | `windows_arm64`: rules_perl 1.5.0 ships no Windows ARM64 perl build |
| PHP | `windows_x86_64`: not tested yet; `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build |
| PowerShell | `macos_arm64`, `windows_x86_64`, `windows_arm64`: not tested yet |
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
| Apple | `windows_x86_64`, `windows_arm64`: not tested yet |
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
| C and C++ | `clang_tidy`, `cppcheck` | `clang_format` | `clang_syntax` |
| C# | `roslyn` | `csharpier` | `csc` |
| CUDA | no | `clang_format` | `nvcc_syntax` |
| D | `dscanner` | `dfmt` | `ldc` |
| Dart | `dart` | `dart` | `dart` |
| Elixir | `credo` | `mix_format` | `dialyzer` |
| Erlang | `elvis` | `erlfmt` | `dialyzer` |
| Fortran | `fortitude` | no | `flang` |
| F# | `fsharplint` | `fantomas` | `fsc` |
| Go | `errcheck`, `govet`, `staticcheck` | `gofumpt` | `gotype` |
| Haskell | `hlint` | `ormolu` | no |
| Java | `checkstyle`, `pmd`, `spotbugs` | `google_java_format` | `javac` |
| JavaScript | `biome`, `eslint` | `biome`, `prettier` | no |
| Julia | no | `julia_format` | no |
| Kotlin | `ktlint` | `ktfmt` | no |
| Lua | no | `stylua` | `luals` |
| MDX | `eslint` | `remark` | no |
| OCaml | no | no | no |
| Perl | `perlcritic` | `perltidy` | n/a |
| PHP | `phpcs` | `prettier` | `phpstan` |
| PowerShell | `psscriptanalyzer` | `psscriptanalyzer` | n/a |
| Python | `flake8`, `pydoclint`, `pylint`, `ruff` | `ruff` | `ty` |
| Ruby | `rubocop` | `standardrb` | no |
| Rust | `clippy` | `rustfmt` | `rustc` |
| Scala | `scalafix` | `scalafmt` | no |
| Svelte | `eslint` | `prettier` | no |
| TypeScript | `biome` | `biome`, `prettier` | `tsc` |
| Vue | `eslint` | `prettier` | no |
| Zig | `zig_ast_check` | `zig_fmt` | no |
| Apple | `clang_tidy` | `clang_format` | `clang_syntax` |
| CMake | `cmakelint` | `gersemi` | n/a |
| Dockerfile | `hadolint` | `prettier` | n/a |
| Make | `checkmake` | `mbake` | n/a |
| Qt | `qmllint` | `qmlformat` | `qmllint` |
| Ansible | `ansible_lint` | `prettier` | n/a |
| API specs | `redocly` | `prettier` | n/a |
| AsciiDoc | `vale` | no | n/a |
| Cap'n Proto | `capnp` | n/a | `capnp` |
| CloudFormation | `cfn_lint` | no | n/a |
| CUE | n/a | `cue` | `cue` |
| FlatBuffers | `flatc` | n/a | `flatc` |
| Gherkin | `gplint` | `prettier` | n/a |
| Helm | `helm` | no | n/a |
| HTML templates | `djlint` | `djlint` | n/a |
| Jsonnet | `jsonnet_lint` | `jsonnetfmt` | n/a |
| Jupyter | `notebook_outputs`, `ruff` | `ruff` | `ty` |
| Kubernetes | `kubeconform` | `prettier` | n/a |
| LaTeX | `chktex` unsupported | `latexindent` | n/a |
| Nix | `deadnix`, `statix` | `nixfmt` | n/a |
| Pkl | no | `pkl` | no |
| Rego | `regal` | `opa` | `opa` |
| reStructuredText | `vale` | no | n/a |
| SQL | `sqlfluff` | `prettier`, `sqlfluff` | n/a |
| Terraform | `tflint` | `terraform` | no |
| Typst | no | `typstyle` | n/a |
| XML | no | `prettier` | n/a |
| YAML | `yamllint` | `yamlfmt` | n/a |

### Not applicable

Not applicable for `format`:

- `capnp`: schema IDL; no maintained .capnp formatter exists, capnp only validates
- `flatbuffers`: schema IDL; no maintained .fbs formatter exists, flatc only validates

Not applicable for `lint`:

- `cue`: cue vet is the only CUE analysis and it is already the typecheck capability; the pinned cue binary has no separate linter

Not applicable for `typecheck`:

- `ansible`: dynamically typed YAML playbooks; no type system beyond ansible-lint
- `api_spec`: declarative API description; no type system beyond schema validation
- `asciidoc`: markup; no static type system
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
