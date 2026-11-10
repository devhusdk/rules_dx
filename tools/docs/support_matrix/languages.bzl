"""The language list behind docs/support-matrix.md."""

load("//apple/private/ios:cell.bzl", APPLE_SUPPORT = "PLATFORM_SUPPORT")
load("//cc/private/toolchains:pins.bzl", "GCC_VERSION", "LLVM_CLANG_VERSION")
load("//cuda/private/toolchain:settings.bzl", CUDA_SUPPORT = "PLATFORM_SUPPORT")
load("//d/private:kwargs.bzl", D_SUPPORT = "PLATFORM_SUPPORT")
load("//dart/private:platforms.bzl", DART_SUPPORT = "PLATFORM_SUPPORT")
load("//elixir/private:kwargs.bzl", ELIXIR_SUPPORT = "PLATFORM_SUPPORT")
load("//erlang/private:kwargs.bzl", ERLANG_SUPPORT = "PLATFORM_SUPPORT")
load("//flutter/private:platforms.bzl", FLUTTER_SUPPORT = "PLATFORM_SUPPORT")
load("//fortran/private:kwargs.bzl", FORTRAN_SUPPORT = "PLATFORM_SUPPORT")
load("//haskell/private:kwargs.bzl", HASKELL_SUPPORT = "PLATFORM_SUPPORT")
load("//julia/private:platforms.bzl", JULIA_SUPPORT = "PLATFORM_SUPPORT")
load("//ocaml/private:kwargs.bzl", OCAML_SUPPORT = "PLATFORM_SUPPORT")
load("//perl/private:platforms.bzl", PERL_SUPPORT = "PLATFORM_SUPPORT")
load("//php/private:defs.bzl", PHP_SUPPORT = "PLATFORM_SUPPORT")
load("//qt/private/sdk:repos.bzl", QT_SUPPORT = "PLATFORM_SUPPORT")
load("//tools/platforms:defs.bzl", "WINDOWS_MSVC_HOST_CONSTRAINTS")
load("//tools/testing/hermetic:defs.bzl", HERMETIC_SUPPORT = "PLATFORM_SUPPORT")

PLATFORMS = [
    {"constraints": [], "cpu": "x86_64", "id": "linux_x86_64", "os": "linux"},
    {"constraints": [], "cpu": "arm64", "id": "linux_arm64", "os": "linux"},
    {"constraints": [], "cpu": "arm64", "id": "macos_arm64", "os": "macos"},
    {"constraints": WINDOWS_MSVC_HOST_CONSTRAINTS, "cpu": "x86_64", "id": "windows_x86_64", "os": "windows"},
    {"constraints": WINDOWS_MSVC_HOST_CONSTRAINTS, "cpu": "arm64", "id": "windows_arm64", "os": "windows"},
]

SECTIONS = ["languages", "targets", "compilers", "formats"]

PLATFORM_SUPPORT = [
    APPLE_SUPPORT,
    CUDA_SUPPORT,
    D_SUPPORT,
    DART_SUPPORT,
    ELIXIR_SUPPORT,
    ERLANG_SUPPORT,
    FLUTTER_SUPPORT,
    FORTRAN_SUPPORT,
    HASKELL_SUPPORT,
    HERMETIC_SUPPORT,
    JULIA_SUPPORT,
    OCAML_SUPPORT,
    PERL_SUPPORT,
    PHP_SUPPORT,
    QT_SUPPORT,
]

# The tool api_compat runs per publishing rule; "not available" where no maintained free API diff tool exists.
API_COMPAT_TOOLS = {
    "conan_deploy": "not available",
    "cpan_deploy": "not available",
    "crates_deploy": "cargo-semver-checks",
    "github_deploy": "apidiff",
    "hackage_deploy": "not available",
    "hex_deploy": "not available",
    "luarocks_deploy": "not available",
    "maven_deploy": "japicmp",
    "npm_deploy": "api-extractor",
    "nuget_deploy": "Microsoft.DotNet.ApiCompat",
    "pub_deploy": "dart_apitool",
    "pypi_deploy": "griffe",
    "rubygems_deploy": "not available",
}





def _hello(lang, repo = ""):
    return [repo + "//" + lang + "/tests/fixtures/hello:hello_test"]

def _starter_consumers(*starters):
    return ["//tools/testing/starters:" + starter for starter in starters]

# consumer_tests: golden checks of dx init output and hermetic manifests, or tests that
# build a target in another module. CI selects the hermetic ones by the hermetic-consumer tag.
ROWS = [
    {"ci_tests": _hello("astro"), "consumer_tests": _starter_consumers("astro"), "name": "astro", "section": "languages", "title": "Astro"},
    {
        "ci_tests": _hello("cc"),
        "consumer_tests": _starter_consumers("c", "cpp"),
        "name": "cc",
        "section": "languages",
        "title": "C and C++",
    },
    {
        "ci_tests": [
            "//cc/tests/gcc_linux_arm64:native_arm64_test",
            "//cc/private/toolchains:gcc_arm64_asan_test",
            "//cc/private/toolchains:gcc_arm64_c_test",
            "//cc/private/toolchains:gcc_arm64_dynamic_libstdcxx_test",
            "//cc/private/toolchains:gcc_arm64_hello_c_test",
            "//cc/private/toolchains:gcc_arm64_hello_test",
            "//cc/private/toolchains:gcc_arm64_shared_test",
            "//cc/private/toolchains:gcc_arm64_ubsan_test",
            "//cc/private/toolchains:gcc_asan_test",
            "//cc/private/toolchains:gcc_c_test",
            "//cc/private/toolchains:gcc_hello_test",
            "//cc/private/toolchains:gcc_hello_c_test",
            "//cc/private/toolchains:gcc_hello_lib_test",
            "//cc/private/toolchains:gcc_shared_test",
            "//cc/private/toolchains:gcc_ubsan_test",
        ],
        "consumer_tests": ["//tools/testing/hermetic:c_fixture_gcc", "//tools/testing/hermetic:cpp_fixture_gcc"],
        "compiler": "gcc",
        "families": [],
        "name": "cc_gcc",
        "section": "compilers",
        "title": "GCC " + GCC_VERSION + " (`gcc`)",
    },
    {
        "ci_tests": _hello("cc"),
        "compiler": "llvm",
        "families": [],
        "name": "cc_llvm",
        "section": "compilers",
        "title": "LLVM " + LLVM_CLANG_VERSION + " (`llvm`, default)",
    },
    {
        "ci_tests": [
            "//cc/private/toolchains:msvc_arm64_hello_test",
            "//cc/private/toolchains:msvc_arm64_pe_machine_test",
            "//cc/private/toolchains:msvc_c_test",
            "//cc/private/toolchains:msvc_hello_c_test",
            "//cc/private/toolchains:msvc_hello_lib_test",
            "//cc/private/toolchains:msvc_hello_test",
            "//cc/private/toolchains:msvc_shared_dbg_test",
            "//cc/private/toolchains:msvc_shared_opt_test",
            "//cc/private/toolchains:msvc_shared_test",
            "//cc/private/toolchains:msvc_spaced_test",
            "//cc/private/toolchains:msvc_static_crt_test",
        ],
        "compiler": "msvc",
        "consumer_tests": ["//tools/testing/hermetic:c_fixture_msvc", "//tools/testing/hermetic:cpp_fixture_msvc"],
        "families": [],
        "name": "cc_msvc",
        "section": "compilers",
        "title": "Microsoft cl.exe (`msvc`)",
    },
    {"ci_tests": _hello("csharp"), "consumer_tests": _starter_consumers("csharp"), "name": "csharp", "section": "languages", "title": "C#"},
    {
        "ci_tests": ["//cuda/tests:saxpy_device_code_test", "//cuda/tests:saxpy_gpu_test"],
        "consumer_tests": _starter_consumers("cuda"),
        "name": "cuda",
        "section": "languages",
        "title": "CUDA",
    },
    {"ci_tests": ["//d/tests/fixtures/hello:greet_test", "//d/tests/fixtures/d-app/diagnostics:diagnostics_test", "//d/tests/fixtures/d-app/source:app_output_test", "//d/tests/fixtures/d-app/source/base:base_test", "//d/tests/fixtures/d-app/source/cli:cli_test"], "consumer_tests": _starter_consumers("d"), "name": "d", "section": "languages", "title": "D"},
    {"ci_tests": ["//dart/tests/runtime:greet_test"], "consumer_tests": ["//tools/testing/hermetic:dart"] + _starter_consumers("dart"), "name": "dart", "section": "languages", "title": "Dart"},
    {
        "ci_tests": ["//elixir/tests/runtime:probe_test"],
        "consumer_tests": _starter_consumers("elixir"),
        "name": "elixir",
        "section": "languages",
        "title": "Elixir",
    },
    {
        "ci_tests": ["//erlang/tests/runtime:eunit"],
        "consumer_tests": _starter_consumers("erlang"),
        "name": "erlang",
        "section": "languages",
        "title": "Erlang",
    },
    {
        "ci_tests": ["//flutter/tests/fixtures/flutter-app/app:app_test", "//flutter/tests/fixtures/flutter-app/web:browser_test"],
        "consumer_tests": ["//tools/testing/hermetic:flutter"],
        "families": ["dart"],
        "name": "flutter",
        "section": "languages",
        "title": "Flutter",
    },
    {"ci_tests": ["//fortran/tests/runtime:stats_test"], "consumer_tests": _starter_consumers("fortran"), "name": "fortran", "section": "languages", "title": "Fortran"},
    {"ci_tests": _hello("fsharp"), "consumer_tests": _starter_consumers("fsharp"), "name": "fsharp", "section": "languages", "title": "F#"},
    {"ci_tests": _hello("go"), "consumer_tests": _starter_consumers("go"), "name": "go", "section": "languages", "title": "Go"},
    {"ci_tests": ["//haskell/tests/runtime:probe_test"], "consumer_tests": _starter_consumers("haskell"), "name": "haskell", "section": "languages", "title": "Haskell"},
    {"ci_tests": _hello("java"), "consumer_tests": _starter_consumers("java"), "name": "java", "section": "languages", "title": "Java"},
    {"ci_tests": _hello("javascript"), "consumer_tests": _starter_consumers("javascript"), "name": "javascript", "section": "languages", "title": "JavaScript"},
    {"ci_tests": ["//julia/tests/fixtures/hello:greet_test"], "consumer_tests": _starter_consumers("julia"), "name": "julia", "section": "languages", "title": "Julia"},
    {"ci_tests": _hello("kotlin"), "name": "kotlin", "section": "languages", "title": "Kotlin"},
    {"ci_tests": ["//lua/tests/runtime:probe_test"], "consumer_tests": ["//tools/testing/hermetic:lua"] + _starter_consumers("lua"), "name": "lua", "section": "languages", "title": "Lua"},
    {"ci_tests": _hello("mdx") + ["//mdx/tests/app:site_static_test"], "consumer_tests": _starter_consumers("mdx"), "name": "mdx", "section": "languages", "title": "MDX"},
    {"ci_tests": ["//ocaml/tests/runtime:greet_test"], "consumer_tests": ["//tools/testing/hermetic:ocaml"] + _starter_consumers("ocaml"), "name": "ocaml", "section": "languages", "title": "OCaml"},
    {"ci_tests": ["//perl/tests/runtime:probe_test"], "consumer_tests": ["//tools/testing/hermetic:perl"] + _starter_consumers("perl"), "name": "perl", "section": "languages", "title": "Perl"},
    {"ci_tests": ["//php/tests/fixtures/hello:greeter_test"], "consumer_tests": _starter_consumers("php"), "name": "php", "section": "languages", "title": "PHP"},
    {"ci_tests": ["//powershell/tests/fixtures/hello:hello_test"], "consumer_tests": _starter_consumers("powershell"), "name": "powershell", "section": "languages", "title": "PowerShell"},
    {"ci_tests": ["//core/codegen:codegen_prost_fixture_argv_test"], "name": "protobuf", "section": "languages", "title": "Protobuf"},
    {
        "ci_tests": ["//python/tests/fixtures/entries:helper_test"],
        "consumer_tests": [
            "//python/tests/consumer:python_init_adopt_test",
        ] + _starter_consumers("python"),
        "name": "python",
        "section": "languages",
        "title": "Python",
    },
    {"ci_tests": _hello("ruby"), "consumer_tests": _starter_consumers("ruby"), "name": "ruby", "section": "languages", "title": "Ruby"},
    {
        "ci_tests": ["//rust/tests/fixtures/consumer_wasm:consumer_wasm_test"],
        "consumer_tests": ["//tools/testing/hermetic:rust_wasi", "//tools/testing/hermetic:cpp_rust"] + _starter_consumers("rust"),
        "name": "rust",
        "section": "languages",
        "title": "Rust",
    },
    {"ci_tests": _hello("scala"), "consumer_tests": ["//tools/testing/hermetic:scala_scalafix"], "name": "scala", "section": "languages", "title": "Scala"},
    {"ci_tests": _hello("svelte"), "consumer_tests": _starter_consumers("svelte"), "name": "svelte", "section": "languages", "title": "Svelte"},
    {"ci_tests": ["//swift/tests/fixtures/hello:hello_output_test"], "name": "swift", "section": "languages", "title": "Swift"},
    {"ci_tests": _hello("typescript"), "consumer_tests": _starter_consumers("typescript"), "name": "typescript", "section": "languages", "title": "TypeScript"},
    {"ci_tests": _hello("vue"), "consumer_tests": _starter_consumers("vue"), "name": "vue", "section": "languages", "title": "Vue"},
    {"ci_tests": ["//zig/tests/fixtures/hello:greet_test"], "consumer_tests": ["//tools/testing/hermetic:zig"] + _starter_consumers("zig"), "name": "zig", "section": "languages", "title": "Zig"},
    {
        "ci_tests": [
            "//android/tests/fixtures/rust_apk:check_unsigned_test",
            "//android/tests/fixtures/rust_apk:sign_test",
        ],
        "consumer_tests": _starter_consumers("rust-android"),
        "families": [],
        "name": "android",
        "section": "targets",
        "title": "Android",
    },
    {
        "ci_tests": ["//appimage/tests:appimage_test"],
        "families": [],
        "consumer_tests": ["//tools/testing/hermetic:appimage"],
        "name": "appimage",
        "section": "targets",
        "title": "AppImage",
    },
    {
        "ci_tests": [
            "//apple/private/provisioning:managed_without_xcode_config_test",
            "//apple/tests/fixtures/macos_ledger:ledger_test",
            "//apple/tests/fixtures/swift_app:ledger_app_test",
            "//apple/tests/fixtures/swift_app:ledger_device_ipa_test",
            "//swift/tests/fixtures/hello:xctest",
        ],
        "consumer_tests": [],
        "families": ["objc"],
        "name": "apple",
        "section": "targets",
        "title": "Apple",
        "workflows": ["evidence.yml"],
    },
    {
        "ci_tests": ["//dockerfile/tests:bad_name_fails", "//dockerfile/tests:owns_dockerfile_class_test"],
        "families": ["dockerfile"],
        "name": "dockerfile",
        "section": "targets",
        "title": "Dockerfile",
    },
    {
        "ci_tests": ["//make/tests:bad_name_fails", "//make/tests:owns_make_class_test"],
        "families": ["make"],
        "name": "make",
        "section": "targets",
        "title": "Make",
    },
    {
        "ci_tests": ["//cmake/tests:bad_name_fails", "//cmake/tests:owns_cmake_class_test"],
        "families": ["cmake"],
        "name": "cmake",
        "section": "targets",
        "title": "CMake",
    },
    {"ci_tests": ["//msix/tests:pack_test"], "families": [], "consumer_tests": ["//tools/testing/hermetic:msix"], "name": "msix", "section": "targets", "title": "MSIX"},
    {
        "ci_tests": ["//qt/tests:qt_test"],
        "consumer_tests": ["//tools/testing/hermetic:qt_tasks", "//tools/testing/hermetic:qt_rust"],
        "families": ["qml"],
        "name": "qt",
        "section": "targets",
        "title": "Qt",
    },
    {"ci_tests": ["//wasi/private/testing:wasi_run_unit_test", "//wasi/private/testing:wasi_guest_test", "//wasi/private/testing:wasi_guest_analysis_test"], "families": [], "consumer_tests": ["//tools/testing/hermetic:rust_wasi"], "name": "wasi", "section": "targets", "title": "WASI"},
    {"name": "css", "section": "formats", "title": "CSS"},
    {"name": "graphql", "section": "formats", "title": "GraphQL"},
    {"name": "markdown", "section": "formats", "title": "Markdown"},
    {"name": "shell", "section": "formats", "title": "Shell"},
    {"name": "starlark", "section": "formats", "title": "Starlark"},
    {"name": "toml", "section": "formats", "title": "TOML"},
    {"name": "api_spec", "section": "formats", "title": "API specs"},
    {"name": "ansible", "section": "formats", "title": "Ansible"},
    {"ci_tests": ["//avro/tests:cc_roundtrip_test", "//avro/tests:RoundtripTest"], "families": ["avro"], "name": "avro", "section": "languages", "title": "Avro"},
    {"families": ["capnp"], "name": "capnproto", "section": "formats", "title": "Cap'n Proto"},
    {"name": "bicep", "section": "formats", "title": "Bicep"},
    {"name": "cloudformation", "section": "formats", "title": "CloudFormation"},
    {"name": "flatbuffers", "section": "formats", "title": "FlatBuffers"},
    {"name": "jupyter", "section": "formats", "title": "Jupyter"},
    {"name": "helm", "section": "formats", "title": "Helm"},
    {"name": "kubernetes", "section": "formats", "title": "Kubernetes"},
    {"name": "nix", "section": "formats", "title": "Nix"},
    {"families": [], "name": "oci", "section": "formats", "title": "OCI images"},
    {"name": "rego", "section": "formats", "title": "Rego"},
    {"name": "cue", "section": "formats", "title": "CUE"},
    {"name": "gherkin", "section": "formats", "title": "Gherkin"},
    {"families": ["core"], "name": "github_workflow", "section": "formats", "title": "GitHub workflows"},
    {"name": "html_template", "section": "formats", "title": "HTML templates"},
    {"name": "jsonnet", "section": "formats", "title": "Jsonnet"},
    {"name": "latex", "section": "formats", "title": "LaTeX"},
    {"name": "pkl", "section": "formats", "title": "Pkl"},
    {"name": "rst", "section": "formats", "title": "reStructuredText"},
    {"name": "asciidoc", "section": "formats", "title": "AsciiDoc"},
    {"name": "sql", "section": "formats", "title": "SQL"},
    {"name": "terraform", "section": "formats", "title": "Terraform"},
    {"name": "thrift", "section": "formats", "title": "Thrift"},
    {"name": "typst", "section": "formats", "title": "Typst"},
    {"name": "xml", "section": "formats", "title": "XML"},
    {"name": "yaml", "section": "formats", "title": "YAML"},
]

# One entry per row that carries per-language columns. Every row in the languages section must give deps, doc, and either coverage or no_coverage; facts.bzl fails on a gap instead of printing n/a.
NO_DOC = struct(tool = "", shard = "")

def _language(name, bench = None, fuzz = None, deps = None, doc = None, dead_code = None, coverage = None, coverage_note = "", no_coverage = None, unsupported = None, not_applicable = None):
    return struct(
        name = name,
        bench = bench,
        fuzz = fuzz,
        deps = deps,
        doc = doc,
        dead_code = dead_code,
        coverage = coverage,
        coverage_note = coverage_note,
        no_coverage = no_coverage,
        unsupported = unsupported,
        not_applicable = not_applicable,
    )

LANGUAGES = [
    _language(
        "android",
        no_coverage = "no dx coverage wiring in the Android rules",
    ),
    _language(
        "appimage",
        not_applicable = {
            "macos_arm64": "AppImage is a Linux format",
            "windows_x86_64": "AppImage is a Linux format",
            "windows_arm64": "AppImage is a Linux format"
        },
    ),
    _language(
        "apple",
        coverage = ["llvm-cov"],
        not_applicable = {
            "windows_arm64": "Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS",
            "windows_x86_64": "Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS"
        },
    ),
    _language(
        "astro",
        deps = "`npm`",
        doc = struct(tool = "astro-compiler", shard = "astro_doc_shard"),
        dead_code = ["eslint @typescript-eslint/no-unused-vars"],
        coverage = ["jest"],
    ),
    _language(
        "avro",
        deps = "none: schemas only, no packages",
        doc = struct(tool = "avro-tools", shard = "avro_doc_shard"),
        dead_code = ["none"],
    ),
    _language(
        "cc",
        bench = "cc_bench",
        fuzz = "cc_fuzz_test",
        deps = "`conan` (pinned, no advisories); none for `vcpkg.json`: conan is the supported manager, there is no vcpkg set",
        doc = struct(tool = "doxygen", shard = "cc_doc_shard"),
        dead_code = ["clang_tidy misc-unused-parameters, misc-unused-using-decls"],
        coverage = ["gcov", "llvm-cov"],
    ),
    _language(
        "cc_gcc",
        not_applicable = {
            "macos_arm64": "The GCC backend builds only for Linux",
            "windows_arm64": "The GCC backend builds only for Linux",
            "windows_x86_64": "The GCC backend builds only for Linux"
        },
    ),
    _language(
        "cc_msvc",
        not_applicable = {
            "linux_arm64": "Microsoft cl.exe runs only on Windows",
            "linux_x86_64": "Microsoft cl.exe runs only on Windows",
            "macos_arm64": "Microsoft cl.exe runs only on Windows"
        },
    ),
    _language(
        "csharp",
        bench = "csharp_bench",
        fuzz = "csharp_fuzz_test",
        deps = "`nuget`",
        doc = struct(tool = "csharp doc driver", shard = "csharp_doc_shard"),
        dead_code = ["roslyn IDE0051, IDE0052, IDE0060, IDE0005"],
        coverage = ["coverlet"],
    ),
    _language(
        "cuda",
        deps = "none: the CUDA toolkit is a pinned download",
        doc = struct(tool = "doxygen", shard = "cuda_doc_shard"),
        dead_code = ["none"],
        no_coverage = "cuda_test has no dx coverage wiring",
        not_applicable = {
            "macos_arm64": "NVIDIA ships no CUDA toolkit for macOS"
        },
    ),
    _language(
        "d",
        bench = "d_bench",
        fuzz = "d_fuzz_test",
        deps = "`dub` (pinned, no advisories)",
        doc = struct(tool = "ldc ddoc", shard = "d_doc_shard"),
        dead_code = ["none"],
        coverage = ["ldc --coverage"],
    ),
    _language(
        "dart",
        bench = "dart_bench",
        deps = "`pub`",
        doc = struct(tool = "dart doc", shard = "dart_doc_shard"),
        dead_code = ["dart unused_element"],
        coverage = ["package:test"],
        unsupported = "//dart/private:unsupported",
        not_applicable = {
            "windows_arm64": "rules_dart 0.8.1 ships no Windows arm64 SDK"
        },
    ),
    _language(
        "elixir",
        bench = "elixir_bench",
        deps = "`hex`",
        doc = NO_DOC,
        dead_code = ["elixirc unused variable, unused function"],
        coverage = ["cover"],
    ),
    _language(
        "erlang",
        bench = "erlang_bench",
        deps = "`hex` from `mix.lock` or an `erlang.rebar` `rebar.lock`",
        doc = NO_DOC,
        dead_code = ["erlc warn_unused_vars, warn_unused_function"],
        coverage = ["cover"],
    ),
    _language(
        "flutter",
        deps = "`pub`",
        doc = struct(tool = "dart doc", shard = "dart_doc_shard"),
        dead_code = ["dart unused_element"],
        coverage = ["flutter_tester"],
        unsupported = "//flutter/private:unsupported",
        not_applicable = {
            "windows_arm64": "rules_flutter 0.0.8 and Flutter 3.47.8 ship no Windows arm64 host SDK (flutter/private/toolchains_repo.bzl HOST_PLATFORMS)"
        },
    ),
    _language(
        "fortran",
        deps = "`fortran.fpm` builds pinned archives from `fpm.lock`; `dx security` and `dx license` read it, `dx update` re-hashes the archives",
        doc = struct(tool = "ford", shard = "fortran_doc_shard"),
        dead_code = ["fortitude C191 unreachable-statement"],
        coverage = ["kcov"],
        coverage_note = "kcov runs on Linux only",
        not_applicable = {
            "windows_arm64": "flang-releases v21.1.8 ships no Windows arm64 build"
        },
    ),
    _language(
        "fsharp",
        bench = "fsharp_bench",
        fuzz = "fsharp_fuzz_test",
        deps = "`nuget`",
        doc = struct(tool = "fsharp doc driver", shard = "fsharp_doc_shard"),
        dead_code = ["fsc FS1182 (--warnon:1182)"],
        coverage = ["coverlet"],
    ),
    _language(
        "go",
        bench = "go_bench",
        fuzz = "go_fuzz_test",
        deps = "`go`",
        doc = struct(tool = "go doc", shard = "go_doc_shard"),
        dead_code = ["staticcheck U1000"],
        coverage = ["go cover"],
    ),
    _language(
        "haskell",
        bench = "haskell_bench",
        deps = "`hackage` from `cabal.project.freeze`; none for `stack.yaml.lock`, which `haskell_deps_resolve` reads to write it",
        doc = struct(tool = "haddock", shard = "haskell_doc_shard"),
        dead_code = ["ghc -Wunused-local-binds, -Wunused-matches (-Wall)"],
        coverage = ["hpc"],
        unsupported = "//haskell/private/toolchain:unsupported",
        not_applicable = {
            "windows_arm64": "GHC 9.10.3 ships no Windows arm64 bindist"
        },
    ),
    _language(
        "java",
        bench = "java_bench",
        fuzz = "java_fuzz_test",
        deps = "`maven`",
        doc = struct(tool = "javadoc", shard = "java_doc_shard"),
        dead_code = ["pmd UnusedLocalVariable, UnusedPrivateField, UnusedPrivateMethod, UnusedFormalParameter"],
        coverage = ["jacoco"],
    ),
    _language(
        "javascript",
        bench = "javascript_bench",
        fuzz = "javascript_fuzz_test",
        deps = "`npm`",
        doc = struct(tool = "typedoc", shard = "javascript_doc_shard"),
        dead_code = ["eslint no-unused-vars"],
        coverage = ["jest"],
    ),
    _language(
        "julia",
        deps = "`julia`",
        doc = struct(tool = "documenter", shard = "julia_doc_shard"),
        dead_code = ["none"],
        coverage = ["CoverageTools"],
        unsupported = "//julia/private:unsupported",
        not_applicable = {
            "windows_arm64": "julialang.org and rules_julia 0.8.0 ship no Windows arm64 Julia build"
        },
    ),
    _language(
        "kotlin",
        bench = "kotlin_bench",
        fuzz = "kotlin_fuzz_test",
        deps = "`maven`",
        doc = struct(tool = "dokka", shard = "kotlin_doc_shard"),
        dead_code = ["detekt"],
        coverage = ["jacoco"],
    ),
    _language(
        "lua",
        bench = "lua_bench",
        deps = "`luarocks` (pinned, no advisories)",
        doc = struct(tool = "ldoc", shard = "lua_doc_shard"),
        dead_code = ["selene unused_variable"],
        coverage = ["luacov"],
    ),
    _language(
        "mdx",
        deps = "`npm`",
        doc = struct(tool = "mdx", shard = "mdx_doc_shard"),
        dead_code = ["eslint no-unused-vars"],
        coverage = ["jest"],
    ),
    _language(
        "nix",
        dead_code = ["deadnix"],
    ),
    _language(
        "ocaml",
        bench = "ocaml_bench",
        fuzz = "ocaml_fuzz_test",
        deps = "`opam`",
        doc = struct(tool = "odoc", shard = "ocaml_doc_shard"),
        dead_code = ["ocamlc warnings 26, 27, 32 to 39"],
        coverage = ["bisect_ppx"],
        not_applicable = {
            "windows_arm64": "conda-forge ships no win-arm64 OCaml package",
            "windows_x86_64": "conda-forge win-64 OCaml stops at 5.3.0, below the pinned 5.5.2"
        },
    ),
    _language(
        "perl",
        bench = "perl_bench",
        deps = "`cpan`",
        doc = struct(tool = "pod (perl)", shard = "perl_doc_shard"),
        dead_code = ["perlcritic ProhibitUnusedVariables"],
        coverage = ["DxLcov"],
        not_applicable = {
            "windows_arm64": "rules_perl 1.5.0 ships no Windows ARM64 perl build"
        },
    ),
    _language(
        "php",
        bench = "php_bench",
        deps = "`packagist`",
        doc = struct(tool = "phpdocumentor", shard = "php_doc_shard"),
        dead_code = ["phpcs Generic.CodeAnalysis.UnusedFunctionParameter"],
        coverage = ["pcov (Linux only)"],
        unsupported = "//php/private/toolchain:unsupported",
        not_applicable = {
            "windows_arm64": "static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build"
        },
    ),
    _language(
        "powershell",
        deps = "`powershell` (pinned, no advisories)",
        doc = struct(tool = "pwsh", shard = "powershell_doc_shard"),
        dead_code = ["psscriptanalyzer PSUseDeclaredVarsMoreThanAssignments"],
        coverage = ["pester"],
    ),
    _language(
        "protobuf",
        deps = "none: schemas only, no packages",
        doc = struct(tool = "protoc", shard = "protobuf_doc_shard"),
        dead_code = ["none"],
    ),
    _language(
        "python",
        bench = "python_bench",
        fuzz = "python_fuzz_test",
        deps = "`uv`; none for `poetry.lock` and `Pipfile.lock`: uv is the supported manager",
        doc = struct(tool = "griffe", shard = "python_doc_shard"),
        dead_code = ["ruff F401, F841"],
        coverage = ["coverage.py"],
    ),
    _language(
        "qt",
        coverage = ["llvm-cov"],
    ),
    _language(
        "ruby",
        bench = "ruby_bench",
        deps = "`ruby`",
        doc = struct(tool = "yard", shard = "ruby_doc_shard"),
        dead_code = ["rubocop Lint/UselessAssignment"],
        coverage = ["simplecov"],
    ),
    _language(
        "rust",
        bench = "rust_bench",
        fuzz = "rust_fuzz_test",
        deps = "`cargo`",
        doc = struct(tool = "rustdoc", shard = "rust_docs_site"),
        dead_code = ["rustc dead_code"],
        coverage = ["llvm-cov"],
    ),
    _language(
        "scala",
        bench = "scala_bench",
        fuzz = "scala_fuzz_test",
        deps = "`maven`",
        doc = struct(tool = "scaladoc", shard = "scala_doc_shard"),
        dead_code = ["scalac -Wunused"],
        coverage = ["jacoco"],
    ),
    _language(
        "shell",
        dead_code = ["shellcheck SC2034"],
    ),
    _language(
        "starlark",
        dead_code = ["buildifier unused-variable"],
    ),
    _language(
        "svelte",
        deps = "`npm`",
        doc = struct(tool = "svelte2tsx", shard = "svelte_doc_shard"),
        dead_code = ["eslint @typescript-eslint/no-unused-vars"],
        coverage = ["jest"],
    ),
    _language(
        "swift",
        bench = "swift_bench",
        fuzz = "swift_fuzz_test",
        deps = "`swiftpm`",
        doc = struct(tool = "swift-symbolgraph-extract", shard = "swift_doc_shard"),
        coverage = ["llvm-cov"],
        not_applicable = {
            "windows_arm64": "The Swift Windows arm64 toolchain is registered but unverified on Windows ARM",
            "windows_x86_64": "rules_dx registers no Swift toolchain for Windows"
        },
    ),
    _language(
        "typescript",
        bench = "typescript_bench",
        fuzz = "typescript_fuzz_test",
        deps = "`npm`",
        doc = struct(tool = "typedoc", shard = "typescript_doc_shard"),
        dead_code = ["eslint no-unused-vars"],
        coverage = ["jest"],
    ),
    _language(
        "vue",
        deps = "`npm`",
        doc = struct(tool = "vue-component-meta", shard = "vue_doc_shard"),
        dead_code = ["eslint @typescript-eslint/no-unused-vars"],
        coverage = ["jest"],
    ),
    _language(
        "wasi",
        no_coverage = "the WASI guest test has no coverage collector",
    ),
    _language(
        "zig",
        bench = "zig_bench",
        deps = "`zig_deps` (`build.zig.zon` `.dependencies`, pinned by sha256)",
        doc = struct(tool = "zig std.zig.Ast", shard = "zig_doc_shard"),
        dead_code = ["zig ast-check unused local, unused parameter"],
        coverage = ["kcov"],
        coverage_note = "kcov runs on Linux only",
    ),
]

def _view(field):
    return {lang.name: getattr(lang, field) for lang in LANGUAGES if getattr(lang, field) != None}

BENCH = _view("bench")
FUZZ = _view("fuzz")
DEPENDENCIES = _view("deps")
DEAD_CODE = _view("dead_code")
COVERAGE_COLLECTORS = _view("coverage")
NO_COVERAGE = _view("no_coverage")
UNSUPPORTED_CONSTRAINTS = _view("unsupported")
PLATFORM_NOT_APPLICABLE = _view("not_applicable")
COVERAGE_NOTES = {lang.name: lang.coverage_note for lang in LANGUAGES if lang.coverage_note}
DOC_TOOLS = {
    lang.name: (None if lang.doc == NO_DOC else {"tool": lang.doc.tool, "shard": lang.doc.shard})
    for lang in LANGUAGES
    if lang.doc != None
}

NOT_LANGUAGES = [
    "containers",
    "bcr_test_module",
    "ci",
    "cli",
    "config",
    "containers",
    "core",
    "deploy",
    "docs",
    "dotnet",
    "dx",
    "env",
    "examples",
    "families",
    "gazelle",
    "patches",
    "quality",
    "third_party",
    "tools",
]

PENDING_COMPILERS = {}

# A compiler row and platform that the row targets but whose evidence is still waiting on something, with the reason.
COMPILER_BLOCKERS = {}

# The identity test of each compiler that has one.
COMPILER_PROBES = {"gcc": "//cc/private/toolchains:identity_gcc", "llvm": "//cc/private/toolchains:identity", "msvc": "//cc/private/toolchains:identity_msvc"}

# The identity test of a compiler on a platform where its COMPILER_PROBES test does not run.
COMPILER_PLATFORM_PROBES = {
    "gcc": {"linux_arm64": "//cc/private/toolchains:identity_gcc_arm64_native"},
    "msvc": {"windows_arm64": "//cc/private/toolchains:identity_msvc_arm64_native"},
}



def _build_file(label):
    return label.split(":")[0] + ":BUILD.bazel"

def support_matrix_inputs():
    """Returns the BUILD and .bzl files the reader opens."""
    files = {}
    for row in ROWS:
        for label in row.get("ci_tests", []) + row.get("consumer_tests", []):
            files[_build_file(label)] = True
        for workflow in row.get("workflows", []):
            files["//.github:workflows/" + workflow] = True
    files["//flutter:defs.bzl"] = True
    files["//tools/testing:nested_bazel.bzl"] = True
    files["//tools/testing/hermetic:defs.bzl"] = True
    return sorted(files.keys())
