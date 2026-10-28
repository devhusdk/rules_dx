# Support matrix

This page lists, for each language and target, the platforms CI tests it on, its `dx new` starter, and its lint, format, and typecheck tools.

An empty cell or `no` is a missing test, tracked as an issue.

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
| D | `linux_x86_64` | no | no |
| Elixir | `linux_x86_64` | `linux_x86_64` | `elixir` |
| Erlang | `linux_x86_64`, `linux_arm64`, `macos_arm64` | `linux_x86_64` | `erlang` |
| Fortran | `linux_x86_64` | no | no |
| F# | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `fsharp` |
| Go | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `go` |
| Haskell | `linux_x86_64`, `linux_arm64` | `linux_x86_64` | `haskell` |
| Java | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `java` |
| JavaScript | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `javascript` |
| Julia | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | no |
| Kotlin | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `kotlin` |
| Lua | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | no |
| MDX | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `mdx` |
| OCaml | `linux_x86_64` | no | no |
| Perl | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | no |
| PHP | `linux_x86_64` | no | no |
| PowerShell | `linux_x86_64`, `linux_arm64` | no | `powershell` |
| Python | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | `python` |
| Ruby | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `ruby` |
| Rust | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `rust`, `rust-android`, `rust-web` |
| Scala | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `scala` |
| Svelte | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `svelte` |
| TypeScript | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `typescript` |
| Vue | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `vue` |
| Zig | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | `zig` |

## Targets

| Target | Builds and tests on | Consumer test on | `dx new` starter |
| --- | --- | --- | --- |
| Android | no | `linux_x86_64` | no |
| AppImage | `linux_x86_64`, `linux_arm64` | no | no |
| Apple | `linux_x86_64`, `macos_arm64` | `linux_x86_64`, `macos_arm64` | no |
| Dockerfile | no | no | no |
| MSIX | no | no | no |
| Kubernetes | `kubeconform` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | no |
| Qt | `linux_x86_64`, `linux_arm64` | `linux_x86_64`, `linux_arm64` | no |
| WASI | `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64` | no | no |

## Quality

Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs. "on" lists the platforms a tool's support record covers.

| Name | Lint | Format | Typecheck |
| --- | --- | --- | --- |
| Astro | `eslint` | `prettier` | no |
| C and C++ | `clang_tidy`, `cppcheck` | `clang_format` | no |
| C# | `roslyn` on `linux_x86_64` | `csharpier` on `linux_x86_64` | no |
| CUDA | no | `clang_format` | no |
| D | no | no | no |
| Elixir | no | `mix_format` | no |
| Erlang | no | `erlfmt` | no |
| Fortran | no | no | no |
| F# | `fsharplint` on `linux_x86_64` | `fantomas` on `linux_x86_64` | no |
| Go | `errcheck`, `govet`, `staticcheck` | `gofumpt` on `linux_x86_64` | no |
| Haskell | no | `ormolu` on `linux_x86_64` | no |
| Java | `checkstyle`, `pmd`, `spotbugs` | `google_java_format` | no |
| JavaScript | `biome`, `eslint` | `biome`, `prettier` | no |
| Julia | no | no | no |
| Kotlin | `ktlint` | `ktfmt` | no |
| Lua | no | `stylua` on `linux_x86_64` | no |
| MDX | `eslint` | `remark` on `linux_x86_64` | no |
| OCaml | no | no | no |
| Perl | no | no | no |
| PHP | no | no | no |
| PowerShell | `psscriptanalyzer` on `linux_x86_64` | `psscriptanalyzer` on `linux_x86_64` | no |
| Python | `flake8`, `pydoclint`, `pylint`, `ruff` | `ruff` | `ty` |
| Ruby | `rubocop` on `linux_x86_64` | `standardrb` on `linux_x86_64` | no |
| Rust | `clippy` | `rustfmt` | `rustc` |
| Scala | `scalafix` on `linux_x86_64` | `scalafmt` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| Svelte | `eslint` | `prettier` | no |
| TypeScript | `biome` | `biome`, `prettier` | no |
| Vue | `eslint` | `prettier` | no |
| Zig | no | `zig_fmt` | no |
| Apple | no | `clang_format` | no |
| Dockerfile | `hadolint` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no | no |
| Qt | `qmllint` | `qmlformat` | no |
| API specs | `redocly` on `linux_x86_64` | no | no |
| CUE | no | `cue` on `linux_x86_64` | no |
| Gherkin | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| HTML templates | `djlint` on `linux_x86_64` | `djlint` on `linux_x86_64` | no |
| Jsonnet | no | `jsonnetfmt` on `linux_x86_64` | no |
| Pkl | no | `pkl` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| SQL | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| Terraform | no | `terraform` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| XML | no | `prettier` on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` | no |
| YAML | `yamllint` on `linux_x86_64` | `yamlfmt` on `linux_x86_64` | no |
