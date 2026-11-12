# Support matrix

This page lists, for each language, target, and compiler, the platforms it does not support, and its lint, format, typecheck, and API doc tools.

`none` means all five platforms are supported. `macos_x86_64` is not supported.

`n/a` means the capability does not apply to that language or format.

The platforms are `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`.

Do not edit this page. Run `bazel run //tools/docs/support_matrix:update` to refresh it.

## Languages

Each suite column names the platforms the conformance test runs on, `n/a` with its reason, or `pending`.

| Language | Not supported on | Dependencies | Bench | Fuzz | build_test | pass_test | fail_test | coverage_test | cross_test | lock_test |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Astro | none | `npm` | n/a | n/a | n/a: astro has no binary rule | all | all | all | n/a: astro has no binary rule | n/a: astro has no binary rule |
| Avro | none | none: schemas only, no packages | n/a | n/a | pending | pending | pending | pending | pending | pending |
| C and C++ | none | none for `vcpkg.json`: there is no vcpkg set | `cc_bench` | `cc_fuzz_test` | all | all | all | all | `linux_arm64`, `linux_x86_64`, `windows_arm64`, `windows_x86_64`; pending on `macos_arm64` | pending |
| C# | none | `nuget` | `csharp_bench` | `csharp_fuzz_test` | all | all | all | all | pending | pending |
| CUDA | `macos_arm64`: NVIDIA ships no CUDA toolkit for macOS | none: the CUDA toolkit is a pinned download | n/a | n/a | `linux_arm64`, `linux_x86_64`, `windows_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `windows_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `windows_arm64`, `windows_x86_64` | pending | pending | pending |
| D | none | `dub` (pinned, no advisories) | `d_bench` | `d_fuzz_test` | all | all | all | all | pending | pending |
| Dart | `windows_arm64`: rules_dart 0.8.1 ships no Windows arm64 SDK | `pub` | `dart_bench` | n/a | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`; n/a: dart supports no other windows platform; pending on `macos_arm64` | pending |
| Elixir | none | `hex` | `elixir_bench` | n/a | all | all | all | all | pending | pending |
| Erlang | none | `hex` from `mix.lock` or an `erlang.rebar` `rebar.lock` | `erlang_bench` | n/a | all | all | all | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64`; pending on `windows_arm64` | pending | pending |
| Flutter | `windows_arm64`: rules_flutter 0.0.8 and Flutter 3.47.9 ship no Windows arm64 host SDK (flutter/private/toolchains_repo.bzl HOST_PLATFORMS) | `pub` | n/a | n/a | pending | pending | pending | pending | pending | pending |
| Fortran | `windows_x86_64`: not in lang.json platforms; `windows_arm64`: flang-releases v21.1.8 ships no Windows arm64 build | the `fpm.lock` tag of the `fpm` extension builds the locked packages; `dx security` and `dx license` read it, `dx update` runs the fpm lock step | n/a | n/a | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | pending | pending |
| F# | none | `nuget` | `fsharp_bench` | `fsharp_fuzz_test` | all | all | all | all | pending | pending |
| Go | none | `go`; cgo is not supported on `windows_x86_64` and `windows_arm64`: the hermetic Windows toolchain is MSVC ABI only, and cgo needs the MinGW (GNU) ABI | `go_bench` | `go_fuzz_test` | all | all | all | all | `linux_arm64`, `linux_x86_64`, `windows_arm64`, `windows_x86_64`; pending on `macos_arm64` | all |
| Haskell | `windows_arm64`: GHC 9.10.3 ships no Windows arm64 bindist | `hackage` from `cabal.project.freeze`; none for `stack.yaml.lock`, which `haskell_deps_resolve` reads to write it | `haskell_bench` | n/a | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | pending | pending |
| Java | none | `maven` | `java_bench` | `java_fuzz_test` | all | all | all | all | pending | all |
| JavaScript | none | `npm` | `javascript_bench` | `javascript_fuzz_test` | all | all | all | all | pending | pending |
| Julia | `windows_arm64`: julialang.org and rules_julia 0.8.0 ship no Windows arm64 Julia build | `julia` | `julia_bench` | n/a | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | pending | pending |
| Kotlin | none | `maven` | `kotlin_bench` | `kotlin_fuzz_test` | all | all | all | all | pending | all |
| Lua | none | `luarocks` (pinned, no advisories) | `lua_bench` | n/a | all | all | all | all | pending | pending |
| MDX | none | `npm` | n/a | n/a | n/a: mdx has no binary rule | all | all | all | n/a: mdx has no binary rule | n/a: mdx has no binary rule |
| OCaml | `windows_x86_64`: conda-forge win-64 OCaml stops at 5.3.0, below the pinned 5.5.2; `windows_arm64`: conda-forge ships no win-arm64 OCaml package | `opam` | `ocaml_bench` | `ocaml_fuzz_test` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | pending | pending |
| Perl | `windows_arm64`: Windows ARM64 perl is not qualified | `cpan` | `perl_bench` | n/a | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | pending | pending |
| PHP | `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build | `packagist` | `php_bench` | n/a | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `windows_x86_64`; pending on `macos_arm64` | pending | pending |
| PowerShell | none | `powershell` (pinned, no advisories) | n/a | n/a | all | all | all | all | pending | pending |
| Protobuf | none | none: schemas only, no packages | n/a | n/a | pending | pending | pending | pending | pending | pending |
| Python | none | `uv`; none for `poetry.lock` and `Pipfile.lock`: uv is the supported manager | `python_bench` | `python_fuzz_test` | all | all | all | all | pending | pending |
| Ruby | `windows_arm64`: not in lang.json platforms | `ruby` | `ruby_bench` | n/a | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_x86_64` | pending | pending |
| Rust | none | `cargo` | `rust_bench` | `rust_fuzz_test` | all | all | all | all | `linux_arm64`, `linux_x86_64`, `windows_arm64`, `windows_x86_64`; pending on `macos_arm64` | all |
| Scala | none | `maven` | `scala_bench` | `scala_fuzz_test` | all | all | all | all | pending | pending |
| Svelte | none | `npm` | n/a | n/a | n/a: svelte has no binary rule | all | all | all | n/a: svelte has no binary rule | n/a: svelte has no binary rule |
| Swift | `windows_x86_64`: rules_dx registers no Swift toolchain for Windows; `windows_arm64`: The Swift Windows arm64 toolchain is registered but unverified on Windows ARM | `swiftpm` | `swift_bench` | `swift_fuzz_test` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`, `macos_arm64` | `linux_arm64`, `linux_x86_64`; pending on `macos_arm64` | pending |
| TypeScript | none | `npm` | `typescript_bench` | `typescript_fuzz_test` | all | all | all | all | pending | pending |
| Vue | none | `npm` | n/a | n/a | n/a: vue has no binary rule | all | all | all | n/a: vue has no binary rule | n/a: vue has no binary rule |
| Zig | none | `zig_deps` (`build.zig.zon` `.dependencies`, pinned by sha256) | `zig_bench` | n/a | all | all | all | `linux_arm64`, `linux_x86_64`, `macos_arm64`, `windows_arm64`; pending on `windows_x86_64` | `linux_arm64`, `linux_x86_64`, `windows_arm64`, `windows_x86_64`; pending on `macos_arm64` | pending |

## Targets

| Target | Not supported on |
| --- | --- |
| Android | none |
| AppImage | `macos_arm64`, `windows_x86_64`, `windows_arm64`: AppImage is a Linux format |
| Apple | `linux_x86_64`, `linux_arm64`, `windows_x86_64`, `windows_arm64`: Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS |
| CMake | none |
| Dockerfile | none |
| Make | none |
| MSIX | none |
| Qt | none |
| WASI | none |

## Compilers

Select one with `--@rules_dx//cc:compiler=<value>`. Blocked on names a platform the compiler targets whose evidence is still waiting, with the reason.

| Compiler | Driver | Coverage | Not supported on | Blocked on |
| --- | --- | --- | --- | --- |
| GCC 16.2.0 (`gcc`) | gnu | gcov | `macos_arm64`, `windows_x86_64`, `windows_arm64`: The GCC backend builds only for Linux | none |
| LLVM 23.1.2 (`llvm`, default) | gnu | llvm | none | none |
| Microsoft cl.exe (`msvc`) | msvc | no | `linux_x86_64`, `linux_arm64`, `macos_arm64`: Microsoft cl.exe runs only on Windows | none |

## Quality

Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs, and the tool that builds its API docs. A tool runs on every platform its row supports unless a note says otherwise. Dead code names the tool and rule that report unused code, `no` when none runs yet, or `none` when the language has no such tool.

| Name | Lint | Format | Typecheck | API docs | Dead code |
| --- | --- | --- | --- | --- | --- |
| Astro | `eslint`, `typos` | `prettier` | `tsc` | `astro-compiler` | `eslint @typescript-eslint/no-unused-vars` |
| Avro | `avro_tools`, `typos` | `prettier` | `avro_tools` | `avro-tools` | none |
| C and C++ | `clang_tidy`, `typos` | `clang_format` | `clang_syntax` | `doxygen` | `clang_tidy misc-unused-parameters, misc-unused-using-decls` |
| C# | `roslyn`, `typos` | `csharpier` | `csc` | `csharp doc driver` | `roslyn IDE0051, IDE0052, IDE0060, IDE0005` |
| CUDA | `clang_tidy`, `typos` | `clang_format` | `nvcc_syntax` | `doxygen` | none |
| D | `typos` | no | `ldc` | `ldc ddoc` | none |
| Dart | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK), `typos` | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart doc` | `dart unused_element` |
| Elixir | `credo`, `typos` | `mix_format` | `dialyzer`, `elixirc` (not on `windows_x86_64`: The elixirc replay needs a POSIX shell; `windows_arm64`: The elixirc replay needs a POSIX shell) | `Code.fetch_docs` | `elixirc unused variable, unused function` |
| Erlang | `elvis`, `typos` | `erlfmt` | `dialyzer`, `erlc` | `edoc` | `erlc warn_unused_vars, warn_unused_function` |
| Flutter | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK), `typos` | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart` (not on `windows_arm64`: rules_dart ships no windows_arm64 SDK) | `dart doc` | `dart unused_element` |
| Fortran | `fortitude`, `typos` | `ffmt` | `flang` | `ford` | `fortitude C191 unreachable-statement` |
| F# | `fsharplint`, `typos` | `fantomas` | `fsc` | `fsharp doc driver` | `fsc FS1182 (--warnon:1182)` |
| Go | `errcheck`, `govet`, `staticcheck`, `typos` | `gofumpt` | `gotype` | `go doc` | `staticcheck U1000` |
| Haskell | `hlint`, `typos`, `weeder` (not on `windows_x86_64`: weeder.exe crashes with an access violation (#9971); `windows_arm64`: GHC 9.10.3 ships no Windows arm64 bindist) | `ormolu` | `ghc` | `haddock` | `ghc -Wunused-local-binds, -Wunused-matches (-Wall)` |
| Java | `pmd`, `spotbugs`, `typos` | `google_java_format` | `javac` | `javadoc` | `pmd UnusedLocalVariable, UnusedPrivateField, UnusedPrivateMethod, UnusedFormalParameter` |
| JavaScript | `biome`, `eslint`, `typos` | `biome` | `tsc` | `typedoc` | `eslint no-unused-vars` |
| Julia | `julialint` (not on `windows_arm64`: Julia ships no windows_arm64 runtime), `typos` | `julia_format` (not on `windows_arm64`: Julia ships no windows_arm64 runtime) | `jet` | `documenter` | none |
| Kotlin | `detekt`, `typos` | `ktfmt` | `kotlinc` | `dokka` | `detekt` |
| Lua | `selene` (not on `windows_arm64`: the dx_quality_crates hub has no windows_arm64 cell), `typos` | `stylua` | `luals` | `ldoc` | `selene unused_variable` |
| MDX | `eslint`, `lychee`, `typos` | `remark` | `tsc` | `mdx` | `eslint no-unused-vars` |
| OCaml | `typos`, `zanuda` (not on `windows_x86_64`: zanuda has no Windows build; `windows_arm64`: zanuda has no Windows build) | `topiary` | `ocaml` | `odoc` | `ocamlc warnings 26, 27, 32 to 39` |
| Perl | `perlcritic` (not on `windows_arm64`: Windows ARM64 perl is not qualified), `typos` | `perltidy` (not on `windows_arm64`: Windows ARM64 perl is not qualified) | n/a | `pod (perl)` | `perlcritic ProhibitUnusedVariables` |
| PHP | `phpcs` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build), `typos` | `prettier` | `phpstan` (not on `windows_arm64`: static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build) | `phpdocumentor` | `phpcs Generic.CodeAnalysis.UnusedFunctionParameter` |
| PowerShell | `psscriptanalyzer`, `typos` | `psscriptanalyzer` | n/a | `pwsh` | `psscriptanalyzer PSUseDeclaredVarsMoreThanAssignments` |
| Protobuf | `buf`, `typos` | `buf` | `buf` | `protoc` | none |
| Python | `pydoclint`, `ruff`, `typos` | `ruff` | `ty` | `griffe` | `ruff F401, F841` |
| Ruby | `rubocop` (not on `windows_arm64`: RubyInstaller arm64 Ruby is not pinned or qualified), `typos` | `rubocop` (not on `windows_arm64`: RubyInstaller arm64 Ruby is not pinned or qualified) | `sorbet` (not on `windows_x86_64`: Sorbet ships no Windows build; `windows_arm64`: Sorbet ships no Windows build) | `yard` | `rubocop Lint/UselessAssignment` |
| Rust | `clippy`, `typos` | `rustfmt` | `rustc` | `rustdoc` | `rustc dead_code` |
| Scala | `scalafix`, `typos` | `scalafmt` | `scalac` | `scaladoc` | `scalac -Wunused` |
| Svelte | `eslint`, `typos` | `prettier` | `tsc` | `svelte2tsx` | `eslint @typescript-eslint/no-unused-vars` |
| Swift | `swift_lint` (not on `windows_x86_64`: swift_lint runs the swift-format pinned only for Linux and macOS arm64; `windows_arm64`: swift_lint runs the swift-format pinned only for Linux and macOS arm64), `typos` | `swift_format` (not on `windows_x86_64`: swift-format is pinned only for Linux and macOS arm64; `windows_arm64`: swift-format is pinned only for Linux and macOS arm64) | `swiftc` | `swift-symbolgraph-extract` | no |
| TypeScript | `biome`, `eslint`, `typos` | `biome` | `tsc` | `typedoc` | `eslint no-unused-vars` |
| Vue | `eslint`, `typos` | `prettier` | `tsc` | `vue-component-meta` | `eslint @typescript-eslint/no-unused-vars` |
| Zig | `typos`, `zig_ast_check` | `zig_fmt` | `zig` | `zig std.zig.Ast` | `zig ast-check unused local, unused parameter` |
| Apple | `clang_tidy`, `typos` | `clang_format` | `clang_syntax` (not on `linux_x86_64`: objc_library builds only with an Apple toolchain; `linux_arm64`: objc_library builds only with an Apple toolchain; `windows_x86_64`: objc_library builds only with an Apple toolchain; `windows_arm64`: objc_library builds only with an Apple toolchain) | n/a | n/a |
| CMake | `cmakelint`, `typos` | `gersemi` | n/a | n/a | n/a |
| Dockerfile | `hadolint`, `typos` | `prettier` | n/a | n/a | n/a |
| Make | `checkmake`, `typos` | `mbake` | n/a | n/a | n/a |
| Qt | `qmllint`, `typos` | `qmlformat` | `qmllint` | n/a | n/a |
| Ansible | `ansible_lint` (not on `windows_x86_64`: ansible-core does not run on Windows; `windows_arm64`: ansible-core does not run on Windows), `typos` | `yamlfmt` | n/a | n/a | n/a |
| API specs | `redocly`, `typos` | `prettier` | n/a | n/a | n/a |
| AsciiDoc | `lychee`, `typos`, `vale` | n/a | n/a | n/a | n/a |
| Bicep | `bicep`, `trivy_config`, `typos` | `bicep` | n/a | n/a | n/a |
| Cap'n Proto | `capnp`, `typos` | n/a | `capnp` | n/a | n/a |
| CloudFormation | `cfn_lint`, `trivy_config`, `typos` | `prettier` | n/a | n/a | n/a |
| CSS | `stylelint`, `typos` | `prettier` | n/a | n/a | n/a |
| CUE | n/a | `cue` | `cue` | n/a | n/a |
| FlatBuffers | `flatc`, `typos` | n/a | `flatc` | n/a | n/a |
| Gherkin | `gplint`, `typos` | `prettier` | n/a | n/a | n/a |
| GitHub workflows | `actionlint`, `zizmor` | `yamlfmt` | n/a | n/a | n/a |
| GraphQL | `graphql_eslint`, `typos` | `prettier` | `graphql_eslint` | n/a | n/a |
| Helm | `helm`, `trivy_config`, `typos` | `helmfmt`, `prettier` | n/a | n/a | n/a |
| HTML templates | `djlint`, `typos` | `djlint` | n/a | n/a | n/a |
| Jsonnet | `jsonnet_lint`, `typos` | `jsonnetfmt` | n/a | n/a | n/a |
| Jupyter | `notebook_outputs`, `ruff` | `ruff` | `ty` | n/a | n/a |
| Kubernetes | `kubeconform`, `trivy_config`, `typos` | `yamlfmt` | n/a | n/a | n/a |
| LaTeX | `chktex`, `typos` | `latexindent` | n/a | n/a | n/a |
| Markdown | `lychee`, `markdown_check`, `typos`, `vale` | `prettier` | n/a | n/a | n/a |
| Nix | `deadnix` (not on `windows_arm64`: the dx_quality_crates hub has no windows_arm64 cell), `typos` | `nixfmt` (not on `macos_arm64`: nixfmt builds from source with the managed GHC, which has no hermetic macOS cabal path yet; `windows_x86_64`: nixfmt depends on the unix package and does not build on Windows; `windows_arm64`: nixfmt depends on the unix package and does not build on Windows) | n/a | n/a | `deadnix` |
| OCI images | `trivy_image_config` | n/a | `container_structure_test` | n/a | n/a |
| Pkl | `pkl_lsp`, `typos` | `pkl` | `pkl` | n/a | n/a |
| Rego | `regal`, `typos` | `opa` | `opa` | n/a | n/a |
| reStructuredText | `lychee`, `typos`, `vale` | `docstrfmt` | n/a | n/a | n/a |
| Shell | `shellcheck`, `typos` | `shfmt` | n/a | n/a | `shellcheck SC2034` |
| SQL | `sqlfluff`, `typos` | `sqlfluff` | n/a | n/a | n/a |
| Starlark | `buildifier`, `typos` | `buildifier` | n/a | n/a | `buildifier unused-variable` |
| Terraform | `tflint`, `trivy_config`, `typos` | `terraform` | `terraform` | n/a | n/a |
| Thrift | `thrift`, `typos` | n/a | `thrift` | n/a | n/a |
| TOML | `taplo`, `typos` | `taplo` | `taplo` | n/a | n/a |
| Typst | `tinymist`, `typos` | `typstyle` | n/a | n/a | n/a |
| XML | `typos`, `xmllint` | `prettier` | n/a | n/a | n/a |
| YAML | `typos`, `yamllint` | `yamlfmt` | n/a | n/a | n/a |

### Not applicable

Not applicable for `format`:

- `asciidoc`: markup; no maintained AsciiDoc formatter exists, asciidoctor only converts
- `capnp`: schema IDL; no maintained .capnp formatter exists, capnp only validates
- `flatbuffers`: schema IDL; no maintained .fbs formatter exists, flatc only validates
- `oci_image`: image config; an image is a tar of layers, not text
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
- `core`: workflow YAML; actionlint already checks expressions in lint
- `css`: no static type system; stylelint covers lint
- `dockerfile`: no static type system; hadolint covers lint
- `gherkin`: natural-language scenarios; no type system
- `helm`: chart lint plus schema validation of rendered manifests
- `html_template`: markup templates; no static type system
- `jsonnet`: dynamically typed; no maintained type checker (the official linter only flags unused variables)
- `kubernetes`: declarative manifests; schema validation is not type checking
- `latex`: typesetting macro language; no static type system
- `make`: no static type system; build language with untyped strings
- `markdown`: markup; no static type system
- `nix`: dynamically typed expression language; no maintained type checker
- `perl`: dynamically typed; no maintained type checker
- `powershell`: no type checker; PSScriptAnalyzer is a linter only
- `rst`: markup; no static type system
- `shell`: no static type system; shellcheck covers lint
- `sql`: type checking needs a database schema and dialect; standalone .sql files carry neither
- `starlark`: dynamically typed; no maintained standalone type checker, buildifier covers lint
- `typst`: dynamically typed markup; no standalone type checker
- `xml`: data format; schema validation is not type checking
- `yaml`: data format; schema validation is not type checking

## API compatibility check

The tool each publishing rule runs for `api_compat`, or `not available` when no maintained free tool exists.

| Rule | Tool |
| --- | --- |
| `conan_deploy` | not available |
| `cpan_deploy` | not available |
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
