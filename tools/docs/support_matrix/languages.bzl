"""The language list behind docs/support-matrix.md."""

load("//cc/toolchains:pins.bzl", "GCC_VERSION", "LLVM_CLANG_VERSION")
load("//d/rules:defs.bzl", "D_CELLS")
load("//d/toolchain:pins.bzl", "LDC_CELLS")
load("//dart/rules:platforms.bzl", "RULES_DART_PLATFORMS")
load("//elixir/rules:defs.bzl", "ELIXIR_CELLS")
load("//erlang/rules:defs.bzl", "ERLANG_CELLS")
load("//erlang/toolchain:pins.bzl", OTP_CELLS = "CELLS")
load("//flutter/rules:platforms.bzl", "FLUTTER_PLATFORMS")
load("//fortran/toolchain:pins.bzl", "FORTRAN_CELLS")
load("//haskell/rules:defs.bzl", "HASKELL_CELLS")
load("//haskell/toolchain:pins.bzl", GHC_CELLS = "CELLS")
load("//julia/rules:platforms.bzl", "JULIA_PLATFORMS")
load("//modules:cuda.bzl", "CUDA_CELLS", "cuda_qualified_cells")
load("//ocaml/toolchain:pins.bzl", "OCAML_CELLS")
load("//perl/rules:platforms.bzl", "RULES_PERL_PLATFORMS")
load("//php/toolchain:pins.bzl", "PHP_CELLS")
load("//qt/sdk:pins.bzl", QT_CELLS = "CELLS")
load("//qt/sdk:repos.bzl", "qualified_cells")
load("//tools/platforms:defs.bzl", "WINDOWS_MSVC_HOST_CONSTRAINTS")

PLATFORMS = [
    {"constraints": [], "cpu": "x86_64", "id": "linux_x86_64", "os": "linux"},
    {"constraints": [], "cpu": "arm64", "id": "linux_arm64", "os": "linux"},
    {"constraints": [], "cpu": "arm64", "id": "macos_arm64", "os": "macos"},
    {"constraints": WINDOWS_MSVC_HOST_CONSTRAINTS, "cpu": "x86_64", "id": "windows_x86_64", "os": "windows"},
    {"constraints": WINDOWS_MSVC_HOST_CONSTRAINTS, "cpu": "arm64", "id": "windows_arm64", "os": "windows"},
]

SECTIONS = ["languages", "targets", "compilers", "formats"]

def _os_cpu_pairs(platforms):
    return [["@platforms//os:" + os, "@platforms//cpu:" + cpu] for os, cpu in sorted(platforms.values())]

def _cells(pinned, cells):
    return [pinned[cell].exec_compatible_with for cell in cells]

COMPUTED_CONSTRAINTS = {
    "@rules_dx_apple//ios:cell.bzl%APPLE_HOSTS": [["@platforms//os:ios"], ["@platforms//os:macos"]],
    "@rules_dx_cuda//rules:defs.bzl%CUDA_TARGET": _cells(CUDA_CELLS, cuda_qualified_cells()),
    "//d/rules:defs.bzl%D_COMPATIBLE_WITH": _cells(LDC_CELLS, D_CELLS),
    "//dart/rules:defs.bzl%DART_COMPATIBLE_WITH": _os_cpu_pairs(RULES_DART_PLATFORMS),
    "//elixir/rules:defs.bzl%ELIXIR_COMPATIBLE_WITH": _cells(OTP_CELLS, ELIXIR_CELLS),
    "//erlang/rules:defs.bzl%ERLANG_COMPATIBLE_WITH": _cells(OTP_CELLS, ERLANG_CELLS),
    "//flutter/rules:defs.bzl%FLUTTER_COMPATIBLE_WITH": _os_cpu_pairs(FLUTTER_PLATFORMS),
    "//fortran/rules:defs.bzl%FORTRAN_COMPATIBLE_WITH": [FORTRAN_CELLS[cell].constraints for cell in sorted(FORTRAN_CELLS.keys())],
    "//haskell/rules:defs.bzl%HASKELL_COMPATIBLE_WITH": _cells(GHC_CELLS, HASKELL_CELLS),
    # Mirrors HERMETIC_COMPATIBLE_WITH in //tools/testing/hermetic:defs.bzl.
    "//tools/testing/hermetic:defs.bzl%HERMETIC_COMPATIBLE_WITH": [["@platforms//os:linux"], ["@platforms//os:macos"], ["@platforms//os:windows"]],
    "//julia/rules:defs.bzl%JULIA_COMPATIBLE_WITH": _os_cpu_pairs(JULIA_PLATFORMS),
    "//ocaml/rules:defs.bzl%OCAML_COMPATIBLE_WITH": _cells(OCAML_CELLS, sorted(OCAML_CELLS.keys())),
    # PERL_COMPATIBLE_WITH is a select() on rules_perl_platform, which matches any of these.
    "//perl/rules:defs.bzl%PERL_COMPATIBLE_WITH": _os_cpu_pairs(RULES_PERL_PLATFORMS),
    "//php/rules:defs.bzl%PHP_COMPATIBLE_WITH": _cells(PHP_CELLS, sorted(PHP_CELLS.keys())),
    "//qt/rules:defs.bzl%QT_COMPATIBLE_WITH": [QT_CELLS[c].constraints for c in qualified_cells()],
}

# What each language row shows in the Dependencies column: the dependency set names, or none with the reason. Lock files without a set are named so `dx security` warnings can point here.
DEPENDENCIES = {
    "astro": "`npm`",
    "avro": "none: schemas only, no packages",
    "cc": "`conan` (pinned, no advisories); none for `vcpkg.json`: conan is the supported manager, there is no vcpkg set",
    "csharp": "`nuget`",
    "cuda": "none: the CUDA toolkit is a pinned download",
    "d": "`dub` (pinned, no advisories)",
    "dart": "`pub`",
    "elixir": "`hex`",
    "erlang": "`hex` from `mix.lock`; none for `rebar.lock`: `dx update` does not refresh it",
    "flutter": "`pub`",
    "fortran": "none: no package manager",
    "fsharp": "`nuget`",
    "go": "`go`",
    "haskell": "`hackage` from `cabal.project.freeze`; none for `stack.yaml.lock`: only cabal has a set",
    "java": "`maven`",
    "javascript": "`npm`",
    "julia": "`julia`",
    "kotlin": "`maven`",
    "lua": "none: no LuaRocks support, rockspec packages are pinned with `http_archive`",
    "mdx": "`npm`",
    "ocaml": "`opam`",
    "perl": "`cpan`",
    "php": "`packagist`",
    "powershell": "`powershell` (pinned, no advisories)",
    "protobuf": "none: schemas only, no packages",
    "python": "`uv`; none for `poetry.lock` and `Pipfile.lock`: uv is the supported manager",
    "ruby": "`ruby`",
    "rust": "`cargo`",
    "scala": "`maven`",
    "svelte": "`npm`",
    "typescript": "`npm`",
    "vue": "`npm`",
    "zig": "none for `build.zig.zon`: the zig family does not fetch `.dependencies`",
}

# The API doc tool per language and the *_doc_shard macro or rule that runs it (docs_site for Rust, which takes the crate itself). None marks a gap.
DOC_TOOLS = {
    "astro": {"tool": "astro-compiler", "shard": "astro_doc_shard"},
    "avro": None,
    "cc": None,
    "csharp": {"tool": "csharp doc driver", "shard": "csharp_doc_shard"},
    "cuda": None,
    "d": None,
    "dart": None,
    "elixir": None,
    "erlang": None,
    "flutter": None,
    "fortran": None,
    "fsharp": {"tool": "fsharp doc driver", "shard": "fsharp_doc_shard"},
    "go": {"tool": "go doc", "shard": "go_doc_shard"},
    "haskell": None,
    "java": {"tool": "javadoc", "shard": "java_doc_shard"},
    "javascript": None,
    "julia": None,
    "kotlin": {"tool": "dokka", "shard": "kotlin_doc_shard"},
    "lua": None,
    "mdx": {"tool": "mdx", "shard": "mdx_doc_shard"},
    "ocaml": None,
    "perl": None,
    "php": None,
    "protobuf": None,
    "powershell": {"tool": "pwsh", "shard": "powershell_doc_shard"},
    "python": {"tool": "griffe", "shard": "python_doc_shard"},
    "ruby": {"tool": "yard", "shard": "ruby_doc_shard"},
    "rust": {"tool": "rustdoc", "shard": "docs_site"},
    "scala": {"tool": "scaladoc", "shard": "scala_doc_shard"},
    "svelte": {"tool": "svelte2tsx", "shard": "svelte_doc_shard"},
    "typescript": {"tool": "typedoc", "shard": "typescript_doc_shard"},
    "vue": {"tool": "vue-component-meta", "shard": "vue_doc_shard"},
    "zig": None,
}

# The tool api_compat runs per publishing rule; "not available" where no maintained free API diff tool exists.
API_COMPAT_TOOLS = {
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

# The tool and rule that report unused code per row. A language row missing here has no dead-code tool yet; "none" marks a language where none exists.
DEAD_CODE = {
    "astro": ["eslint @typescript-eslint/no-unused-vars"],
    "cc": ["clang_tidy misc-unused-parameters, misc-unused-using-decls"],
    "csharp": ["roslyn IDE0051, IDE0052, IDE0060, IDE0005"],
    "cuda": ["none"],
    "d": ["none"],
    "dart": ["dart unused_element"],
    "elixir": ["elixirc unused variable, unused function"],
    "erlang": ["erlc warn_unused_vars, warn_unused_function"],
    "fortran": ["fortitude C191 unreachable-statement"],
    "fsharp": ["fsc FS1182 (--warnon:1182)"],
    "go": ["staticcheck U1000"],
    "haskell": ["ghc -Wunused-local-binds, -Wunused-matches (-Wall)"],
    "java": ["pmd UnusedLocalVariable, UnusedPrivateField, UnusedPrivateMethod, UnusedFormalParameter"],
    "javascript": ["eslint no-unused-vars"],
    "julia": ["none"],
    "kotlin": ["detekt"],
    "lua": ["selene unused_variable"],
    "nix": ["deadnix"],
    "ocaml": ["ocamlc warnings 26, 27, 32 to 39"],
    "perl": ["perlcritic ProhibitUnusedVariables"],
    "php": ["phpcs Generic.CodeAnalysis.UnusedFunctionParameter"],
    "powershell": ["psscriptanalyzer PSUseDeclaredVarsMoreThanAssignments"],
    "python": ["ruff F401, F841"],
    "ruby": ["rubocop Lint/UselessAssignment"],
    "rust": ["rustc dead_code"],
    "scala": ["scalac -Wunused"],
    "shell": ["shellcheck SC2034"],
    "starlark": ["buildifier unused-variable"],
    "svelte": ["eslint @typescript-eslint/no-unused-vars"],
    "typescript": ["eslint no-unused-vars"],
    "vue": ["eslint @typescript-eslint/no-unused-vars"],
    "zig": ["zig ast-check unused local, unused parameter"],
}

# The coverage collector each row's tests write LCOV with. A row in neither dict has no coverage to report.
COVERAGE_COLLECTORS = {
    "cc": ["gcov", "llvm-cov"],
    "csharp": ["coverlet"],
    "d": ["ldc --coverage"],
    "dart": ["package:test"],
    "elixir": ["cover"],
    "erlang": ["cover"],
    "flutter": ["flutter_tester"],
    "fortran": ["kcov"],
    "fsharp": ["coverlet"],
    "go": ["go cover"],
    "haskell": ["hpc"],
    "java": ["jacoco"],
    "javascript": ["jest"],
    "julia": ["CoverageTools"],
    "kotlin": ["jacoco"],
    "lua": ["luacov"],
    "ocaml": ["bisect_ppx"],
    "perl": ["DxLcov"],
    "php": ["pcov (Linux only)"],
    "powershell": ["pester"],
    "python": ["coverage.py"],
    "ruby": ["simplecov"],
    "rust": ["llvm-cov"],
    "scala": ["jacoco"],
    "typescript": ["jest"],
    "zig": ["kcov"],
}

# A note on where a row's coverage collector runs.
COVERAGE_NOTES = {
    "fortran": "kcov runs on Linux only",
    "zig": "kcov runs on Linux only",
}

# The reason a row's tests have no coverage collector.
NO_COVERAGE = {
    "android": "no dx coverage wiring in the Android rules",
    "apple": "no dx coverage wiring in the Apple rules",
    "astro": "js_test has no dx coverage wiring",
    "cuda": "cuda_test has no dx coverage wiring",
    "mdx": "js_test has no dx coverage wiring",
    "qt": "no dx coverage wiring in the Qt rules",
    "svelte": "js_test has no dx coverage wiring",
    "vue": "js_test has no dx coverage wiring",
    "wasi": "the WASI guest test has no coverage collector",
}

def _hello(lang, repo = ""):
    return [repo + "//" + lang + "/tests/fixtures/hello:hello_test"]

def _starter_consumers(*starters):
    return ["//tools/testing/starters:" + starter for starter in starters]

# consumer_tests: golden checks of dx init output and hermetic manifests, or tests that
# build a target in another module. CI selects the hermetic ones by the hermetic-consumer tag.
ROWS = [
    {"ci_tests": _hello("astro", "@rules_dx_js"), "consumer_tests": _starter_consumers("astro"), "name": "astro", "section": "languages", "title": "Astro"},
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
            "//cc/toolchains:gcc_arm64_asan_test",
            "//cc/toolchains:gcc_arm64_c_test",
            "//cc/toolchains:gcc_arm64_dynamic_libstdcxx_test",
            "//cc/toolchains:gcc_arm64_hello_c_test",
            "//cc/toolchains:gcc_arm64_hello_test",
            "//cc/toolchains:gcc_arm64_shared_test",
            "//cc/toolchains:gcc_arm64_ubsan_test",
            "//cc/toolchains:gcc_asan_test",
            "//cc/toolchains:gcc_c_test",
            "//cc/toolchains:gcc_hello_test",
            "//cc/toolchains:gcc_hello_c_test",
            "//cc/toolchains:gcc_hello_lib_test",
            "//cc/toolchains:gcc_shared_test",
            "//cc/toolchains:gcc_ubsan_test",
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
            "//cc/toolchains:msvc_arm64_hello_test",
            "//cc/toolchains:msvc_arm64_pe_machine_test",
            "//cc/toolchains:msvc_c_test",
            "//cc/toolchains:msvc_hello_c_test",
            "//cc/toolchains:msvc_hello_lib_test",
            "//cc/toolchains:msvc_hello_test",
            "//cc/toolchains:msvc_shared_dbg_test",
            "//cc/toolchains:msvc_shared_opt_test",
            "//cc/toolchains:msvc_shared_test",
            "//cc/toolchains:msvc_spaced_test",
            "//cc/toolchains:msvc_static_crt_test",
        ],
        "compiler": "msvc",
        "consumer_tests": ["//tools/testing/hermetic:c_fixture_msvc", "//tools/testing/hermetic:cpp_fixture_msvc"],
        "families": [],
        "name": "cc_msvc",
        "section": "compilers",
        "title": "Microsoft cl.exe (`msvc`)",
    },
    {"ci_tests": _hello("csharp", "@rules_dx_dotnet"), "consumer_tests": _starter_consumers("csharp"), "name": "csharp", "section": "languages", "title": "C#"},
    {
        "ci_tests": ["@rules_dx_cuda//tests:saxpy_device_code_test", "@rules_dx_cuda//tests:saxpy_gpu_test"],
        "consumer_tests": _starter_consumers("cuda"),
        "name": "cuda",
        "section": "languages",
        "title": "CUDA",
    },
    {"ci_tests": ["//d/tests/fixtures/hello:greet_test", "//d/tests/fixtures/d-app/diagnostics:diagnostics_test", "//d/tests/fixtures/d-app/source:app_output_test", "//d/tests/fixtures/d-app/source/base:base_test", "//d/tests/fixtures/d-app/source/cli:cli_test"], "consumer_tests": _starter_consumers("d"), "name": "d", "section": "languages", "title": "D"},
    {"ci_tests": ["//dart/tests/runtime:greet_test"], "consumer_tests": ["//tools/testing/hermetic:dart"] + _starter_consumers("dart"), "name": "dart", "section": "languages", "title": "Dart"},
    {
        "ci_tests": ["@rules_dx_beam//elixir/tests/runtime:probe_test"],
        "consumer_tests": _starter_consumers("elixir"),
        "name": "elixir",
        "section": "languages",
        "title": "Elixir",
    },
    {
        "ci_tests": ["@rules_dx_beam//erlang/tests/runtime:eunit"],
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
    {"ci_tests": _hello("fsharp", "@rules_dx_dotnet"), "consumer_tests": _starter_consumers("fsharp"), "name": "fsharp", "section": "languages", "title": "F#"},
    {"ci_tests": _hello("go"), "consumer_tests": _starter_consumers("go"), "name": "go", "section": "languages", "title": "Go"},
    {"ci_tests": ["//haskell/tests/runtime:probe_test"], "consumer_tests": _starter_consumers("haskell"), "name": "haskell", "section": "languages", "title": "Haskell"},
    {"ci_tests": _hello("java", "@rules_dx_jvm"), "consumer_tests": _starter_consumers("java"), "name": "java", "section": "languages", "title": "Java"},
    {"ci_tests": _hello("javascript", "@rules_dx_js"), "consumer_tests": _starter_consumers("javascript"), "name": "javascript", "section": "languages", "title": "JavaScript"},
    {"ci_tests": ["//julia/tests/fixtures/hello:greet_test"], "consumer_tests": _starter_consumers("julia"), "name": "julia", "section": "languages", "title": "Julia"},
    {"ci_tests": _hello("kotlin", "@rules_dx_jvm"), "name": "kotlin", "section": "languages", "title": "Kotlin"},
    {"ci_tests": ["//lua/tests/runtime:probe_test"], "consumer_tests": ["//tools/testing/hermetic:lua"] + _starter_consumers("lua"), "name": "lua", "section": "languages", "title": "Lua"},
    {"ci_tests": ["@rules_dx_js//mdx/tests/app:site_static_test"], "consumer_tests": _starter_consumers("mdx"), "name": "mdx", "section": "languages", "title": "MDX"},
    {"ci_tests": ["//ocaml/tests/runtime:greet_test"], "consumer_tests": ["//tools/testing/hermetic:ocaml"] + _starter_consumers("ocaml"), "name": "ocaml", "section": "languages", "title": "OCaml"},
    {"ci_tests": ["//perl/tests/runtime:probe_test"], "consumer_tests": ["//tools/testing/hermetic:perl"] + _starter_consumers("perl"), "name": "perl", "section": "languages", "title": "Perl"},
    {"ci_tests": ["//php/tests/fixtures/hello:greeter_test"], "consumer_tests": _starter_consumers("php"), "name": "php", "section": "languages", "title": "PHP"},
    {"ci_tests": ["@rules_dx_powershell//tests/fixtures/hello:hello_test"], "consumer_tests": _starter_consumers("powershell"), "name": "powershell", "section": "languages", "title": "PowerShell"},
    {"ci_tests": ["//generation:codegen_prost_fixture_argv_test"], "name": "protobuf", "section": "languages", "title": "Protobuf"},
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
    {"ci_tests": _hello("scala", "@rules_dx_jvm"), "consumer_tests": ["//tools/testing/hermetic:scala_scalafix"], "name": "scala", "section": "languages", "title": "Scala"},
    {"ci_tests": _hello("svelte", "@rules_dx_js"), "consumer_tests": _starter_consumers("svelte"), "name": "svelte", "section": "languages", "title": "Svelte"},
    {"ci_tests": _hello("typescript", "@rules_dx_js"), "consumer_tests": _starter_consumers("typescript"), "name": "typescript", "section": "languages", "title": "TypeScript"},
    {"ci_tests": _hello("vue", "@rules_dx_js"), "consumer_tests": _starter_consumers("vue"), "name": "vue", "section": "languages", "title": "Vue"},
    {"ci_tests": ["//zig/tests/fixtures/hello:greet_test"], "consumer_tests": ["//tools/testing/hermetic:zig"] + _starter_consumers("zig"), "name": "zig", "section": "languages", "title": "Zig"},
    {
        "ci_tests": [
            "@rules_dx_android//tests/fixtures/rust_apk:check_unsigned_test",
            "@rules_dx_android//tests/fixtures/rust_apk:sign_test",
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
            "@rules_dx_apple//provisioning:managed_without_xcode_config_test",
            "@rules_dx_apple//tests/fixtures/macos_ledger:ledger_test",
            "@rules_dx_apple//tests/fixtures/swift_app:ledger_app_test",
            "@rules_dx_apple//tests/fixtures/swift_app:ledger_device_ipa_test",
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
    {"ci_tests": ["@rules_dx_msix//tests:pack_test"], "families": [], "consumer_tests": ["//tools/testing/hermetic:msix"], "name": "msix", "section": "targets", "title": "MSIX"},
    {
        "ci_tests": ["//qt/tests:qt_test"],
        "consumer_tests": ["//tools/testing/hermetic:qt_tasks", "//tools/testing/hermetic:qt_rust"],
        "families": ["qml"],
        "name": "qt",
        "section": "targets",
        "title": "Qt",
    },
    {"ci_tests": ["//wasi/testing:wasi_run_unit_test", "//wasi/testing:wasi_guest_test", "//wasi/testing:wasi_guest_analysis_test"], "families": [], "consumer_tests": ["//tools/testing/hermetic:rust_wasi"], "name": "wasi", "section": "targets", "title": "WASI"},
    {"name": "css", "section": "formats", "title": "CSS"},
    {"name": "graphql", "section": "formats", "title": "GraphQL"},
    {"name": "markdown", "section": "formats", "title": "Markdown"},
    {"name": "shell", "section": "formats", "title": "Shell"},
    {"name": "starlark", "section": "formats", "title": "Starlark"},
    {"name": "toml", "section": "formats", "title": "TOML"},
    {"name": "api_spec", "section": "formats", "title": "API specs"},
    {"name": "ansible", "section": "formats", "title": "Ansible"},
    {"ci_tests": ["@rules_dx_generation//avro/tests:cc_roundtrip_test", "@rules_dx_generation//avro/tests:RoundtripTest"], "families": ["avro"], "name": "avro", "section": "languages", "title": "Avro"},
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

NOT_LANGUAGES = [
    "containers",
    "swift",
    "bcr_test_module",
    "ci",
    "cli",
    "config",
    "containers",
    "deploy",
    "docs",
    "dotnet",
    "dx",
    "env",
    "examples",
    "families",
    "gazelle",
    "generation",
    "libs",
    "modules",
    "patches",
    "quality",
    "third_party",
    "tools",
]

PENDING_COMPILERS = {}

# A compiler row and platform that the row targets but whose evidence is still waiting on something, with the reason.
COMPILER_BLOCKERS = {}

# The identity test of each compiler that has one.
COMPILER_PROBES = {"gcc": "//cc/toolchains:identity_gcc", "llvm": "//cc/toolchains:identity", "msvc": "//cc/toolchains:identity_msvc"}

# The identity test of a compiler on a platform where its COMPILER_PROBES test does not run.
COMPILER_PLATFORM_PROBES = {
    "gcc": {"linux_arm64": "//cc/toolchains:identity_gcc_arm64_native"},
    "msvc": {"windows_arm64": "//cc/toolchains:identity_msvc_arm64_native"},
}

# The unsupported constraint_value of each row that declares one, written to cli/cli/src/exec/unsupported.json.
UNSUPPORTED_CONSTRAINTS = {
    "dart": "//dart/rules:unsupported",
    "flutter": "//flutter/rules:unsupported",
    "haskell": "//haskell/toolchain:unsupported",
    "julia": "//julia/rules:unsupported",
    "php": "//php/toolchain:unsupported",
}

# A platform a row does not support, with the reason.
PLATFORM_NOT_APPLICABLE = {
    "appimage": {
        "macos_arm64": "AppImage is a Linux format",
        "windows_x86_64": "AppImage is a Linux format",
        "windows_arm64": "AppImage is a Linux format",
    },
    "apple": {
        "windows_arm64": "Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS",
        "windows_x86_64": "Apple toolchains (Xcode, codesign, iOS SDKs) run only on macOS",
    },
    "cc_gcc": {
        "macos_arm64": "The GCC backend builds only for Linux",
        "windows_arm64": "The GCC backend builds only for Linux",
        "windows_x86_64": "The GCC backend builds only for Linux",
    },
    "cc_msvc": {
        "linux_arm64": "Microsoft cl.exe runs only on Windows",
        "linux_x86_64": "Microsoft cl.exe runs only on Windows",
        "macos_arm64": "Microsoft cl.exe runs only on Windows",
    },
    "cuda": {
        "macos_arm64": "NVIDIA ships no CUDA toolkit for macOS",
    },
    "dart": {
        "windows_arm64": "rules_dart 0.8.1 ships no Windows arm64 SDK",
    },
    "flutter": {
        "windows_arm64": "rules_flutter 0.0.8 and Flutter 3.47.8 ship no Windows arm64 host SDK (flutter/private/toolchains_repo.bzl HOST_PLATFORMS)",
    },
    "fortran": {
        "windows_arm64": "flang-releases v21.1.8 ships no Windows arm64 build",
    },
    "haskell": {
        "windows_arm64": "GHC 9.10.3 ships no Windows arm64 bindist",
    },
    "julia": {
        "windows_arm64": "julialang.org and rules_julia 0.8.0 ship no Windows arm64 Julia build",
    },
    "ocaml": {
        "windows_arm64": "conda-forge ships no win-arm64 OCaml package",
        "windows_x86_64": "conda-forge win-64 OCaml stops at 5.3.0, below the pinned 5.5.2",
    },
    "perl": {
        "windows_arm64": "rules_perl 1.5.0 ships no Windows ARM64 perl build",
    },
    "php": {
        "windows_arm64": "static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build",
    },
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
    files["//flutter/rules:defs.bzl"] = True
    files["//tools/testing:nested_bazel.bzl"] = True
    files["//tools/testing/hermetic:defs.bzl"] = True
    return sorted(files.keys())
