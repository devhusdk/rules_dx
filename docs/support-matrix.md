# Support matrix

This page lists, for each language and target, the platforms CI tests it on, its `dx new` starter, and its lint, format, and typecheck tools.

An empty cell or `no` is a missing test, tracked as an issue.

`n/a` means the capability does not apply to that language or format.

The platforms are `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`.

Do not edit this page. Run `bazel run //tools/docs/support_matrix:update` to refresh it.

## Languages

"Builds and tests on" lists the platforms where CI runs the language's rule tests. "Consumer test on" lists the platforms where CI builds it from a module other than `rules_dx`.

| Language | Builds and tests on | Consumer test on | `dx new` starter |
| --- | --- | --- | --- |
| Astro | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `astro` |
| C and C++ | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `c`, `cpp` |
| C# | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `csharp` |
| CUDA | `linux_x86_64` | `linux_x86_64` | `cuda` |
| D | `linux_x86_64` | `linux_x86_64` | `d` |
| Dart | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | no |
| Elixir | `linux_x86_64` | `linux_x86_64` | `elixir` |
| Erlang | `linux_x86_64`, `linux_arm64`, `macos_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64` | `erlang` |
| Fortran | `linux_x86_64` | `linux_x86_64` | `fortran` |
| F# | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `fsharp` |
| Go | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `go` |
| Haskell | `linux_x86_64`, `linux_arm64`, `macos_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64` | `haskell` |
| Java | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `java` |
| JavaScript | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `javascript` |
| Julia | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `julia` |
| Kotlin | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `kotlin` |
| Lua | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `lua` |
| MDX | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `mdx` |
| OCaml | `linux_x86_64` | `linux_x86_64` | `ocaml` |
| Perl | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `perl` |
| PHP | `linux_x86_64` | `linux_x86_64` | `php` |
| PowerShell | `linux_x86_64`, `linux_arm64` | `linux_x86_64`, `linux_arm64` | `powershell` |
| Python | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `python` |
| Ruby | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `ruby` |
| Rust | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | `rust`, `rust-android`, `rust-web` |
| Scala | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | `scala` |
| Svelte | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `svelte` |
| TypeScript | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `typescript` |
| Vue | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `vue` |
| Zig | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `zig` |

## Targets

| Target | Builds and tests on | Consumer test on | `dx new` starter |
| --- | --- | --- | --- |
| Android | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | no |
| AppImage | `linux_x86_64`, `linux_arm64` | `linux_x86_64` | no |
| Apple | `linux_x86_64`, `macos_arm64` | `linux_x86_64`, `macos_arm64` | no |
| CMake | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | no |
| Dockerfile | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | no |
| Make | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | no |
| MSIX | `linux_x86_64`, `linux_arm64` | `linux_x86_64` | no |
| Qt | `linux_x86_64`, `linux_arm64` | `linux_x86_64`, `linux_arm64` | no |
| WASI | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | no |

## Quality

Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs. "on" lists the platforms a tool's support record covers.

| Name | Lint | Format | Typecheck |
| --- | --- | --- | --- |
| Astro | `eslint` | `prettier` | no |
| C and C++ | `clang_tidy`, `cppcheck` | `clang_format` | `clang_syntax` |
| C# | `roslyn` on `linux_x86_64` | `csharpier` on `linux_x86_64` | `csc` on `linux_x86_64` |
| CUDA | no | `clang_format` | no |
| D | `dscanner` | `dfmt` | no |
| Dart | `dart` | `dart` | `dart` |
| Elixir | `credo` | `mix_format` | `dialyzer` |
| Erlang | `elvis` | `erlfmt` | `dialyzer` |
| Fortran | `fortitude` on `linux_x86_64` | no | no |
| F# | `fsharplint` on `linux_x86_64` | `fantomas` on `linux_x86_64` | no |
| Go | `errcheck`, `govet`, `staticcheck` | `gofumpt` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Haskell | no | `ormolu` on `linux_x86_64`, `windows_x86_64`, `windows_arm64` | no |
| Java | `checkstyle`, `pmd`, `spotbugs` | `google_java_format` | no |
| JavaScript | `biome`, `eslint` | `biome`, `prettier` | no |
| Julia | no | `julia_format` | no |
| Kotlin | `ktlint` | `ktfmt` | no |
| Lua | no | `stylua` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| MDX | `eslint` | `remark` on `linux_x86_64` | no |
| OCaml | no | no | no |
| Perl | no | `perltidy` | n/a |
| PHP | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `phpstan` |
| PowerShell | `psscriptanalyzer` on `linux_x86_64` | `psscriptanalyzer` on `linux_x86_64` | n/a |
| Python | `flake8`, `pydoclint`, `pylint`, `ruff` | `ruff` | `ty` |
| Ruby | `rubocop` on `linux_x86_64` | `standardrb` on `linux_x86_64` | no |
| Rust | `clippy` | `rustfmt` | `rustc` |
| Scala | `scalafix` on `linux_x86_64` | `scalafmt` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Svelte | `eslint` | `prettier` | no |
| TypeScript | `biome` | `biome`, `prettier` | no |
| Vue | `eslint` | `prettier` | no |
| Zig | no | `zig_fmt` | no |
| Apple | no | `clang_format` | `clang_syntax` |
| CMake | no | `gersemi` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| Dockerfile | `hadolint` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| Make | `checkmake` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | n/a |
| Qt | `qmllint` | `qmlformat` | no |
| Ansible | `ansible_lint` on `linux_x86_64`, `linux_arm64`, `macos_arm64` | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| API specs | `redocly` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| CloudFormation | `cfn_lint` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | n/a |
| CUE | n/a | `cue` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `cue` on `linux_x86_64` |
| FlatBuffers | `flatc` on `linux_x86_64` | n/a | `flatc` on `linux_x86_64` |
| Gherkin | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| Helm | `helm` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | n/a |
| HTML templates | `djlint` on `linux_x86_64` | `djlint` on `linux_x86_64` | n/a |
| Jsonnet | `jsonnet_lint` on `linux_x86_64` | `jsonnetfmt` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | n/a |
| Jupyter | `notebook_outputs` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`, `ruff` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `ruff` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Kubernetes | `kubeconform` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| LaTeX | `chktex` unsupported | `latexindent` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| Nix | `deadnix` on `linux_x86_64`, `statix` on `linux_x86_64` | `nixfmt` on `linux_x86_64` | n/a |
| Pkl | no | `pkl` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Rego | `regal` on `linux_x86_64` | `opa` on `linux_x86_64` | `opa` on `linux_x86_64` |
| reStructuredText | `vale` | no | n/a |
| SQL | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | n/a |
| Terraform | `tflint` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `terraform` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Typst | no | `typstyle` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | n/a |
| XML | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| YAML | `yamllint` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `yamlfmt` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | n/a |

### Not applicable

Not applicable for `format`:

- `flatbuffers`: schema IDL; no maintained .fbs formatter exists, flatc only validates

Not applicable for `lint`:

- `cue`: cue vet is the only CUE analysis and it is already the typecheck capability; the pinned cue binary has no separate linter

Not applicable for `typecheck`:

- `ansible`: dynamically typed YAML playbooks; no type system beyond ansible-lint
- `api_spec`: declarative API description; no type system beyond schema validation
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
