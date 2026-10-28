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
| Astro | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `astro` |
| C and C++ | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `windows_x86_64` | `c`, `cpp` |
| C# | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `csharp` |
| CUDA | `linux_x86_64` | `linux_x86_64` | `cuda` |
| D | `linux_x86_64` | no | `d` |
| Dart | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | no |
| Elixir | `linux_x86_64` | `linux_x86_64` | `elixir` |
| Erlang | `linux_x86_64`, `linux_arm64`, `macos_arm64` | `linux_x86_64` | `erlang` |
| Fortran | `linux_x86_64` | no | `fortran` |
| F# | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `fsharp` |
| Go | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `go` |
| Haskell | `linux_x86_64`, `linux_arm64`, `macos_arm64` | `linux_x86_64` | `haskell` |
| Java | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `java` |
| JavaScript | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `javascript` |
| Julia | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `linux_x86_64` | `julia` |
| Kotlin | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `kotlin` |
| Lua | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | `lua` |
| MDX | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `mdx` |
| OCaml | `linux_x86_64` | `linux_x86_64` | `ocaml` |
| Perl | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `linux_x86_64` | `perl` |
| PHP | `linux_x86_64` | no | `php` |
| PowerShell | `linux_x86_64`, `linux_arm64` | no | `powershell` |
| Python | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `python` |
| Ruby | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `ruby` |
| Rust | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | `rust`, `rust-android`, `rust-web` |
| Scala | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | `scala` |
| Svelte | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `svelte` |
| TypeScript | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `typescript` |
| Vue | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `vue` |
| Zig | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | `zig` |

## Targets

| Target | Builds and tests on | Consumer test on | `dx new` starter |
| --- | --- | --- | --- |
| Android | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64` | no |
| AppImage | `linux_x86_64`, `linux_arm64` | `linux_x86_64` | no |
| Apple | `linux_x86_64`, `macos_arm64` | `linux_x86_64`, `macos_arm64` | no |
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
| C and C++ | `clang_tidy`, `cppcheck` | `clang_format` | no |
| C# | `roslyn` on `linux_x86_64` | `csharpier` on `linux_x86_64` | no |
| CUDA | no | `clang_format` | no |
| D | `dscanner` | `dfmt` | no |
| Dart | no | no | no |
| Elixir | `credo` | `mix_format` | no |
| Erlang | `elvis` | `erlfmt` | `dialyzer` |
| Fortran | no | no | no |
| F# | `fsharplint` on `linux_x86_64` | `fantomas` on `linux_x86_64` | no |
| Go | `errcheck`, `govet`, `staticcheck` | `gofumpt` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Haskell | no | `ormolu` on `linux_x86_64` | no |
| Java | `checkstyle`, `pmd`, `spotbugs` | `google_java_format` | no |
| JavaScript | `biome`, `eslint` | `biome`, `prettier` | no |
| Julia | no | `julia_format` | no |
| Kotlin | `ktlint` | `ktfmt` | no |
| Lua | no | `stylua` on `linux_x86_64` | no |
| MDX | `eslint` | `remark` on `linux_x86_64` | no |
| OCaml | no | no | no |
| Perl | no | `perltidy` | n/a |
| PHP | no | no | no |
| PowerShell | `psscriptanalyzer` on `linux_x86_64` | `psscriptanalyzer` on `linux_x86_64` | n/a |
| Python | `flake8`, `pydoclint`, `pylint`, `ruff` | `ruff` | `ty` |
| Ruby | `rubocop` on `linux_x86_64` | `standardrb` on `linux_x86_64` | no |
| Rust | `clippy` | `rustfmt` | `rustc` |
| Scala | `scalafix` on `linux_x86_64` | `scalafmt` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| Svelte | `eslint` | `prettier` | no |
| TypeScript | `biome` | `biome`, `prettier` | no |
| Vue | `eslint` | `prettier` | no |
| Zig | no | `zig_fmt` | no |
| Apple | no | `clang_format` | no |
| Dockerfile | `hadolint` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | n/a |
| Make | `checkmake` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | n/a |
| Qt | `qmllint` | `qmlformat` | no |
| Ansible | `ansible_lint` on `linux_x86_64`, `linux_arm64`, `macos_arm64` | no | no |
| API specs | `redocly` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| CUE | no | `cue` on `linux_x86_64` | no |
| FlatBuffers | `flatc` on `linux_x86_64` | no | no |
| Gherkin | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| HTML templates | `djlint` on `linux_x86_64` | `djlint` on `linux_x86_64` | n/a |
| Jsonnet | `jsonnet_lint` on `linux_x86_64` | `jsonnetfmt` on `linux_x86_64` | no |
| Jupyter | `notebook_outputs` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`, `ruff` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | `ruff` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Kubernetes | `kubeconform` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | no |
| LaTeX | `chktex` unsupported | `latexindent` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| Nix | `deadnix` on `linux_x86_64`, `statix` on `linux_x86_64` | `nixfmt` on `linux_x86_64` | no |
| Pkl | no | `pkl` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| Rego | `regal` on `linux_x86_64` | `opa` on `linux_x86_64` | `opa` on `linux_x86_64` |
| SQL | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Terraform | `tflint` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `terraform` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no |
| Typst | no | `typstyle` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| XML | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | n/a |
| YAML | `yamllint` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `yamlfmt` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | n/a |

### Not applicable

Not applicable for `typecheck`:

- `api_spec`: declarative API description; no type system beyond schema validation
- `dockerfile`: no static type system; hadolint covers lint
- `gherkin`: natural-language scenarios; no type system
- `html_template`: markup templates; no static type system
- `make`: no static type system; build language with untyped strings
- `perl`: dynamically typed; no maintained type checker
- `powershell`: no type checker; PSScriptAnalyzer is a linter only
- `typst`: dynamically typed markup; no standalone type checker
- `xml`: data format; schema validation is not type checking
- `yaml`: data format; schema validation is not type checking
