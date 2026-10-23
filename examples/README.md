# Examples

Start a new consumer, adopt an existing tree, or mix frameworks. Each
workspace below records exact commands and expected evidence in its own
`README.md`.

## Start From CI Templates

- [consumer-ci](consumer-ci/) starter caller for the reusable consumer workflow.
- [docs-ci](docs-ci/) starter caller for the reusable docs workflow.

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

- [c-app](c-app/) multi-package pure C stock CLI with a pinned `cJSON` dependency, a generated header and packaged data.
- [cpp-app](cpp-app/) multi-package C++17 menu report with templates, a pinned `fmt` dependency, a generated header and a runfiles resource.
- [java-app](java-app/) multi-package Java invoice CLI with pinned `gson` and `guava` dependencies, a generated class and a packaged resource.
- [kotlin-app](kotlin-app/) multi-package Kotlin gradebook CLI with pinned `gson` and `guava` dependencies, a generated object and a packaged resource.
- [node-app](node-app/) multi-package TypeScript and JavaScript registry CLI with a pinned `semver` dependency, a generated module and a packaged resource.
- [python-service](python-service/) multi-package Python CLI and local service with a pinned `click` dependency, a generated module and packaged data.
- [qt-app](qt-app/) headless Qt app with a QML module, a C++ QML type, a Qt resource and a QtTest suite.
- [qt-tasks](qt-tasks/) multi-module Qt/QML task browser with C++ models, a pinned `fmt` dependency, translations, JSON storage and a relocatable archive.
