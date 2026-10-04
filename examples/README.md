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
- [adopt-zig](adopt-zig/) Zig library adopted without upstream changes.
- [adopt-csharp](adopt-csharp/) foreign SDK-style tree adopted by the C# Gazelle extension.
- [adopt-fsharp](adopt-fsharp/) foreign SDK-style tree adopted by the F# Gazelle extension.
- [adopt-ruby](adopt-ruby/) foreign Bundler-layout tree adopted by the Ruby Gazelle extension.
- [adopt-powershell](adopt-powershell/) foreign PowerShell tree adopted by handwritten wrappers.
- [adopt-polyglot](adopt-polyglot/) foreign Python+Rust+JS/TS tree adopted package by package.

## Mix Frameworks

- [mixed](mixed/hello/) Vue, Svelte, Astro, and MDX containers over one shared JavaScript helper.
