# Examples

Start a new consumer, adopt an existing tree, or mix frameworks. Each
workspace below records exact commands and expected evidence in its own
`README.md`.

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
- [adopt-powershell](adopt-powershell/) foreign PowerShell tree adopted by handwritten wrappers.
- [adopt-polyglot](adopt-polyglot/) foreign Python+Rust+JS/TS+C++ tree adopted package by package.

## Mix Frameworks

- [mixed](mixed/hello/) Vue, Svelte, Astro, and MDX containers over one shared JavaScript helper.

## Share One Core

- [shared-rust](shared-rust/) one Rust task board driving native, browser, WASI, and Android outputs.

## Build an Application

- [api-spec](api-spec/) OpenAPI root with a local ref and a shared ref-only library, linted by Redocly.
- [appimage-app](appimage-app/) multi-package Linux C app with a shared library and a packaged resource, built as an AppImage and run without FUSE in tests.
- [c-app](c-app/) multi-package pure C stock CLI with a pinned `cJSON` dependency, a generated header and packaged data.
- [cpp-app](cpp-app/) multi-package C++17 menu report with templates, a pinned `fmt` dependency, a generated header and a runfiles resource.
- [elixir-app](elixir-app/) multi-package Elixir application with a generated unit table, a supervised ledger, a pinned `jason` Hex package and an escript.
- [erlang-app](erlang-app/) multi-package Erlang/OTP key-value store with a leex and yecc parser, a supervised application, a pinned `thoas` dependency and an escript.
- [gleam-app](gleam-app/) multi-package Gleam stock CLI for Erlang and JavaScript with a generated unit table, a typed ledger and a pinned `gleam_json` Hex package.
- [java-app](java-app/) multi-package Java invoice CLI with pinned `gson` and `guava` dependencies, a generated class and a packaged resource.
- [kotlin-app](kotlin-app/) multi-package Kotlin gradebook CLI with pinned `gson` and `guava` dependencies, a generated object and a packaged resource.
- [msix-app](msix-app/) Windows console app with a DLL and a packaged resource, built as an MSIX, installed and signed in tests.
- [node-app](node-app/) multi-package TypeScript and JavaScript registry CLI with a pinned `semver` dependency, a generated module and a packaged resource.
- [perl-app](perl-app/) multi-package Perl stock CLI with a typed ledger, a runfiles CSV table, a pinned `Text::CSV` CPAN dependency and a `select()` flag.
- [python-service](python-service/) multi-package Python CLI and local service with a pinned `click` dependency, a generated module and packaged data.
- [qt-app](qt-app/) headless Qt app with a QML module, a C++ QML type, a Qt resource and a QtTest suite.
- [qt-rust](qt-rust/) headless Qt/QML ledger backed by a Rust model through CXX-Qt bindings, with a pinned `adler2` dependency, a QtTest suite and a relocatable archive.
- [qt-tasks](qt-tasks/) multi-module Qt/QML task browser with C++ models, a Qt Quick board and Quick Test suite, a pinned `fmt` dependency, translations, JSON storage and relocatable archives.
- [zig-app](zig-app/) multi-package Zig manifest CLI with a pinned `xxHash` C dependency, a generated module and embedded data.
