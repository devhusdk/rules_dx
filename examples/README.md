# Examples

Start a new consumer, adopt an existing tree, or mix frameworks. Each
workspace below records exact commands and expected evidence in its own
`README.md`.

For the rule and aspect attributes these examples use, see the
[Starlark rule reference](https://devhusdk.github.io/rules_dx/rules/).

## Start From CI Templates

- [consumer-ci](consumer-ci/) starter caller for the reusable consumer workflow.
- [docs-ci](docs-ci/) starter caller for the reusable docs workflow.
- [gitlab-ci](gitlab-ci/) starter pipeline for GitLab CI.

## Adopt An Existing Tree

- [adopt-rust](adopt-rust/) foreign Cargo workspace adopted by `dx generate`.
- [adopt-python](adopt-python/) foreign Python tree adopted by the Python Gazelle extension.
- [adopt-js-ts](adopt-js-ts/) foreign JS/TS tree adopted by the JavaScript/TypeScript Gazelle extensions.
- [adopt-go](adopt-go/) foreign Go module adopted by the Go Gazelle extension.
- [adopt-cpp](adopt-cpp/) foreign C++ tree adopted by the C++ Gazelle extension.
- [adopt-java](adopt-java/) foreign Maven-layout tree adopted by the Java Gazelle extension.
- [adopt-kotlin](adopt-kotlin/) foreign Maven-layout tree adopted by the Kotlin Gazelle extension.
- [adopt-scala](adopt-scala/) foreign sbt-layout tree adopted by the Scala Gazelle extension.
- [adopt-csharp](adopt-csharp/) foreign SDK-style tree adopted by the C# Gazelle extension.
- [adopt-fsharp](adopt-fsharp/) foreign SDK-style tree adopted by the F# Gazelle extension.
- [adopt-ruby](adopt-ruby/) foreign Bundler-layout tree adopted by the Ruby Gazelle extension.
- [adopt-powershell](adopt-powershell/) foreign PowerShell tree adopted by the PowerShell Gazelle extension.
- [adopt-polyglot](adopt-polyglot/) foreign Python+Rust+JS/TS+C++ tree adopted package by package.
- [adopt-android](adopt-android/) Gradle-shaped Rust, C++ and Java tree packaged as an APK.

## Mix Frameworks

- [mixed](mixed/hello/) Vue, Svelte, Astro, and MDX containers over one shared JavaScript helper.

## Share One Core

- [shared-rust](shared-rust/) one Rust task board driving native, browser, WASI, and Android outputs.

## Build an Application

- [api-spec](api-spec/) OpenAPI root with a local ref and a shared ref-only library, linted by Redocly.
- [appimage-app](appimage-app/) multi-package Linux C app with a shared library and a packaged resource, built as an AppImage and run without FUSE in tests.
- [c-app](c-app/) multi-package pure C stock CLI with a pinned `cJSON` dependency, a generated header and packaged data.
- [cpp-app](cpp-app/) multi-package C++17 menu report with templates, a pinned `fmt` dependency, a generated header and a runfiles resource.
- [cuda-app](cuda-app/) multi-package CUDA app with three kernels behind a C ABI header, a C++ binary with a CPU fallback and checks on the compiled device code.
- [elixir-app](elixir-app/) multi-package Elixir application with a generated unit table, a supervised ledger, a pinned `jason` Hex package and an escript.
- [erlang-app](erlang-app/) multi-package Erlang/OTP key-value store with a leex and yecc parser, a supervised application, a pinned `thoas` dependency, a C NIF and an escript.
- [haskell-app](haskell-app/) Haskell stock tool with a pinned Hackage `split` dependency, a generated module, a C foreign call and a string flag.
- [java-app](java-app/) multi-package Java invoice CLI with pinned `gson` and `guava` dependencies, a generated class and a packaged resource.
- [julia-app](julia-app/) multi-package Julia report CLI with a pinned `OrderedCollections` package and a `Bzip2_jll` binary dependency, a generated data file, runfiles lookup and a `select()` on a digits flag.
- [kotlin-app](kotlin-app/) multi-package Kotlin gradebook CLI with pinned `gson` and `guava` dependencies, a generated object and a packaged resource.
- [lua-app](lua-app/) multi-package Lua unit-stock CLI on Lua 5.4 built from source, with a pinned `inspect.lua` dependency, runfiles data and StyLua checks.
- [msix-app](msix-app/) Windows console app with a DLL and a packaged resource, built as an MSIX, installed and signed in tests.
- [node-app](node-app/) multi-package TypeScript and JavaScript registry CLI with a pinned `semver` dependency, a generated module and a packaged resource.
- [ocaml-app](ocaml-app/) multi-package OCaml stock CLI with a pinned `Astring` dependency, an ocamllex lexer, a C stub in the native cell and a `select()` on the emitter flag.
- [perl-app](perl-app/) multi-package Perl stock CLI with a typed ledger, a runfiles CSV table, a pinned `Text::CSV` CPAN dependency and a `select()` flag.
- [python-service](python-service/) multi-package Python CLI and local service with a pinned `click` dependency, a generated module and packaged data.
- [qt-app](qt-app/) headless Qt app with a QML module, a C++ QML type, a Qt resource and a QtTest suite.
- [qt-rust](qt-rust/) headless Qt/QML ledger backed by a Rust model through CXX-Qt bindings, with a pinned `adler2` dependency, a QtTest suite and a relocatable archive.
- [qt-tasks](qt-tasks/) multi-module Qt/QML task browser with C++ models, a Qt Quick board and Quick Test suite, a pinned `fmt` dependency, translations, JSON storage and relocatable archives.
- [zig-app](zig-app/) multi-package Zig manifest CLI with a pinned `xxHash` C dependency, a generated module and embedded data.
