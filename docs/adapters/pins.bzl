"""Docs adapter pins plus mappings."""

load("//modules:versions.bzl", _DOTNET_VERSION = "DOTNET_VERSION", _GO_SDK_VERSION = "GO_SDK_VERSION")

RUST_RUSTDOC = "nightly-2026-09-01 with rustdoc JSON format_version 30 via -Z unstable-options --output-format json"
PYTHON_GRIFFE = "griffe==2.2.0 with griffe dump --full plus JSON schema"
TYPESCRIPT_TYPEDOC = "typedoc@0.28.20 with --json --emit none plus schemaVersion"
JAVA_JDK = "JDK 25 with custom Javadoc Doclet plus owned JSON contract"
KOTLIN_DOKKA = "Kotlin 2.2.20 plus Dokka 2.2.0 with custom plugin owned JSON"
GO_XTOOLS = "Go SDK " + _GO_SDK_VERSION + " plus golang.org/x/tools v0.36.0 via go/packages"
CPP_DOXYGEN = "Doxygen 1.18.0 XML generator with Clang-assisted comparison"
CSHARP_ROSLYN = ".NET SDK " + _DOTNET_VERSION + " with assembly plus /doc XML join plus owned inheritdoc resolver"
FSHARP_SERVICE = ".NET SDK " + _DOTNET_VERSION + " plus FSharp.Compiler.Service 43.9.200 with XML join"
VUE_DOCGEN = "vue-docgen-api@4.79.2 with parseMulti plus arrays-only props/events/slots/methods"
SVELTE_SVELD = "sveld@0.37.3 with compiler plus svelte2tsx comparison"
SCALA_TASTY = "Scala 3.3.6 with TASTy Inspector spike plus Scaladoc bridge comparison"
ASTROMDX_PROSE = "None; prose-only with no API surface"
