"""The language list behind docs/support-matrix.md."""

load("//cuda/rules:defs.bzl", "CUDA_TARGET")
load("//d/rules:defs.bzl", "D_COMPATIBLE_WITH")
load("//dart/rules:platforms.bzl", "RULES_DART_PLATFORMS")
load("//elixir/rules:defs.bzl", "ELIXIR_CELLS")
load("//erlang/rules:defs.bzl", "ERLANG_CELLS")
load("//erlang/toolchain:pins.bzl", OTP_CELLS = "CELLS")
load("//fortran/rules:defs.bzl", "FORTRAN_COMPATIBLE_WITH")
load("//haskell/rules:defs.bzl", "HASKELL_CELLS")
load("//haskell/toolchain:pins.bzl", GHC_CELLS = "CELLS")
load("//julia/rules:platforms.bzl", "JULIA_PLATFORMS")
load("//ocaml/rules:defs.bzl", "OCAML_COMPATIBLE_WITH")
load("//perl/rules:platforms.bzl", "RULES_PERL_PLATFORMS")
load("//php/rules:defs.bzl", "PHP_COMPATIBLE_WITH")

SUPPORT_MATRIX_SCHEMA_VERSION = 1

PLATFORMS = [
    {"cpu": "x86_64", "id": "linux_x86_64", "os": "linux"},
    {"cpu": "arm64", "id": "linux_arm64", "os": "linux"},
    {"cpu": "arm64", "id": "macos_arm64", "os": "macos"},
    {"cpu": "x86_64", "id": "windows_x86_64", "os": "windows"},
    {"cpu": "arm64", "id": "windows_arm64", "os": "windows"},
]

SECTIONS = ["languages", "targets", "formats"]

def _constraint(label):
    if type(label) == "Label" and label.repo_name in ("platforms", "platforms+"):
        return "@platforms//" + label.package + ":" + label.name
    return str(label)

def _all_of(labels):
    return [[_constraint(label) for label in labels]]

def _os_cpu_pairs(platforms):
    return [["@platforms//os:" + os, "@platforms//cpu:" + cpu] for os, cpu in sorted(platforms.values())]

def _cells(pinned, cells):
    return [pinned[cell].exec_compatible_with for cell in cells]

COMPUTED_CONSTRAINTS = {
    "//cuda/rules:defs.bzl%CUDA_TARGET": _all_of(CUDA_TARGET),
    "//d/rules:defs.bzl%D_COMPATIBLE_WITH": _all_of(D_COMPATIBLE_WITH),
    "//dart/rules:defs.bzl%DART_COMPATIBLE_WITH": _os_cpu_pairs(RULES_DART_PLATFORMS),
    "//elixir/rules:defs.bzl%ELIXIR_COMPATIBLE_WITH": _cells(OTP_CELLS, ELIXIR_CELLS),
    "//erlang/rules:defs.bzl%ERLANG_COMPATIBLE_WITH": _cells(OTP_CELLS, ERLANG_CELLS),
    "//fortran/rules:defs.bzl%FORTRAN_COMPATIBLE_WITH": _all_of(FORTRAN_COMPATIBLE_WITH),
    "//haskell/rules:defs.bzl%HASKELL_COMPATIBLE_WITH": _cells(GHC_CELLS, HASKELL_CELLS),
    # Mirrors HERMETIC_COMPATIBLE_WITH in //tools/testing/hermetic:defs.bzl.
    "//tools/testing/hermetic:defs.bzl%HERMETIC_COMPATIBLE_WITH": [["@platforms//os:linux"]],
    "//julia/rules:defs.bzl%JULIA_COMPATIBLE_WITH": _os_cpu_pairs(JULIA_PLATFORMS),
    "//ocaml/rules:defs.bzl%OCAML_COMPATIBLE_WITH": _all_of(OCAML_COMPATIBLE_WITH),
    # PERL_COMPATIBLE_WITH is a select() on rules_perl_platform, which matches any of these.
    "//perl/rules:defs.bzl%PERL_COMPATIBLE_WITH": _os_cpu_pairs(RULES_PERL_PLATFORMS),
    "//php/rules:defs.bzl%PHP_COMPATIBLE_WITH": _all_of(PHP_COMPATIBLE_WITH),
}

def _hello(lang):
    return ["//" + lang + "/tests/fixtures/hello:hello_test"]

def _starter_consumers(*starters):
    return ["//tools/testing/starters:" + starter for starter in starters]

# consumer_tests rule: the test must build and test a target of the row's rules in a
# module other than rules_dx. Hermetic qualifications qualify; they are manual, and CI
# selects them by the hermetic-consumer tag, which the reader resolves per job.
ROWS = [
    {"ci_tests": _hello("astro"), "consumer_tests": _starter_consumers("astro"), "name": "astro", "section": "languages", "title": "Astro"},
    {
        "ci_tests": _hello("cc"),
        "consumer_tests": ["//cc/tests/consumer:app_consumer_test", "//cc/tests/starter:starter_test"] + _starter_consumers("c", "cpp"),
        "name": "cc",
        "section": "languages",
        "title": "C and C++",
    },
    {"ci_tests": _hello("csharp"), "name": "csharp", "section": "languages", "title": "C#"},
    {
        "ci_tests": ["//cuda/tests:saxpy_device_code_test", "//cuda/tests:saxpy_gpu_test"],
        "consumer_tests": ["//cuda/tests/starter:starter_test"],
        "name": "cuda",
        "section": "languages",
        "title": "CUDA",
    },
    {"ci_tests": ["//d/tests/fixtures/hello:greet_test"], "consumer_tests": _starter_consumers("d"), "name": "d", "section": "languages", "title": "D"},
    {"ci_tests": ["//dart/tests/runtime:greet_test"], "name": "dart", "section": "languages", "title": "Dart"},
    {
        "ci_tests": ["//elixir/tests/runtime:probe_test"],
        "consumer_tests": ["//elixir/tests/starter:starter_test"] + _starter_consumers("elixir"),
        "name": "elixir",
        "section": "languages",
        "title": "Elixir",
    },
    {
        "ci_tests": ["//erlang/tests/runtime:eunit"],
        "consumer_tests": ["//erlang/tests/starter:starter_test"] + _starter_consumers("erlang"),
        "name": "erlang",
        "section": "languages",
        "title": "Erlang",
    },
    {"ci_tests": ["//fortran/tests/runtime:stats_test"], "consumer_tests": _starter_consumers("fortran"), "name": "fortran", "section": "languages", "title": "Fortran"},
    {"ci_tests": _hello("fsharp"), "name": "fsharp", "section": "languages", "title": "F#"},
    {"ci_tests": _hello("go"), "name": "go", "section": "languages", "title": "Go"},
    {"ci_tests": ["//haskell/tests/runtime:probe_test"], "consumer_tests": ["//haskell/tests/starter:starter_test"] + _starter_consumers("haskell"), "name": "haskell", "section": "languages", "title": "Haskell"},
    {"ci_tests": _hello("java"), "name": "java", "section": "languages", "title": "Java"},
    {"ci_tests": _hello("javascript"), "name": "javascript", "section": "languages", "title": "JavaScript"},
    {"ci_tests": ["//julia/tests/fixtures/hello:greet_test"], "consumer_tests": ["//julia/tests/starter:starter_test"] + _starter_consumers("julia"), "name": "julia", "section": "languages", "title": "Julia"},
    {"ci_tests": _hello("kotlin"), "name": "kotlin", "section": "languages", "title": "Kotlin"},
    {"ci_tests": ["//lua/tests/runtime:probe_test"], "consumer_tests": ["//tools/testing/hermetic:lua"] + _starter_consumers("lua"), "name": "lua", "section": "languages", "title": "Lua"},
    {"ci_tests": ["//mdx/tests/app:site_static_test"], "consumer_tests": _starter_consumers("mdx"), "name": "mdx", "section": "languages", "title": "MDX"},
    {"ci_tests": ["//ocaml/tests/runtime:greet_test"], "consumer_tests": ["//tools/testing/hermetic:ocaml"] + _starter_consumers("ocaml"), "name": "ocaml", "section": "languages", "title": "OCaml"},
    {"ci_tests": ["//perl/tests/runtime:probe_test"], "consumer_tests": ["//tools/testing/hermetic:perl"] + _starter_consumers("perl"), "name": "perl", "section": "languages", "title": "Perl"},
    {"ci_tests": ["//php/tests/fixtures/hello:greeter_test"], "consumer_tests": _starter_consumers("php"), "name": "php", "section": "languages", "title": "PHP"},
    {"ci_tests": _hello("powershell"), "consumer_tests": _starter_consumers("powershell"), "name": "powershell", "section": "languages", "title": "PowerShell"},
    {
        "ci_tests": ["//python/tests/fixtures/entries:helper_test"],
        "consumer_tests": [
            "//python/tests/consumer:service_consumer_test",
            "//python/tests/consumer:docs_consumer_test",
        ],
        "name": "python",
        "section": "languages",
        "title": "Python",
    },
    {"ci_tests": _hello("ruby"), "consumer_tests": _starter_consumers("ruby"), "name": "ruby", "section": "languages", "title": "Ruby"},
    {
        "ci_tests": ["//rust/tests/fixtures/consumer_wasm:consumer_wasm_test"],
        "consumer_tests": ["//tools/testing/hermetic:rust_wasi", "//tools/testing/hermetic:cpp_rust"],
        "name": "rust",
        "section": "languages",
        "title": "Rust",
    },
    {"ci_tests": _hello("scala"), "consumer_tests": ["//tools/testing/hermetic:scala_scalafix"], "name": "scala", "section": "languages", "title": "Scala"},
    {"ci_tests": _hello("svelte"), "consumer_tests": _starter_consumers("svelte"), "name": "svelte", "section": "languages", "title": "Svelte"},
    {"ci_tests": _hello("typescript"), "name": "typescript", "section": "languages", "title": "TypeScript"},
    {"ci_tests": _hello("vue"), "consumer_tests": _starter_consumers("vue"), "name": "vue", "section": "languages", "title": "Vue"},
    {"ci_tests": ["//zig/tests/fixtures/hello:greet_test"], "consumer_tests": ["//tools/testing/hermetic:zig"] + _starter_consumers("zig"), "name": "zig", "section": "languages", "title": "Zig"},
    {
        "ci_tests": [
            "//android/tests/fixtures/rust_apk:check_unsigned_test",
            "//android/tests/fixtures/rust_apk:sign_test",
        ],
        "consumer_tests": ["//android/tests/starter:starter_test"],
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
            "//apple/tests:linux_consumer_test",
            "//apple/tests:macos_consumer_test",
            "//apple/tests:macos_app_consumer_test",
            "//apple/tests:macos_swift_consumer_test",
        ],
        "consumer_tests": [
            "//apple/tests:linux_consumer_test",
            "//apple/tests:macos_consumer_test",
            "//apple/tests:macos_app_consumer_test",
            "//apple/tests:macos_swift_consumer_test",
        ],
        "families": ["objc"],
        "name": "apple",
        "section": "targets",
        "title": "Apple",
        "workflows": ["apple.yml"],
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
        "consumer_tests": ["//quality/tests:cmake_consumer_test"],
        "families": ["cmake"],
        "name": "cmake",
        "section": "targets",
        "title": "CMake",
    },
    {"ci_tests": ["//msix/tests:pack_test"], "families": [], "consumer_tests": ["//tools/testing/hermetic:msix"], "name": "msix", "section": "targets", "title": "MSIX"},
    {
        "ci_tests": ["//qt/tests:qt_tasks_test"],
        "consumer_tests": ["//qt/tests:qt_app_consumer_test"],
        "families": ["qml"],
        "name": "qt",
        "section": "targets",
        "title": "Qt",
    },
    {"ci_tests": ["//wasi/testing:wasi_run_unit_test"], "families": [], "consumer_tests": ["//tools/testing/hermetic:rust_wasi"], "name": "wasi", "section": "targets", "title": "WASI"},
    {"name": "api_spec", "section": "formats", "title": "API specs"},
    {"name": "ansible", "section": "formats", "title": "Ansible"},
    {"name": "cloudformation", "section": "formats", "title": "CloudFormation"},
    {"name": "flatbuffers", "section": "formats", "title": "FlatBuffers"},
    {"name": "jupyter", "section": "formats", "title": "Jupyter"},
    {"name": "helm", "section": "formats", "title": "Helm"},
    {"name": "kubernetes", "section": "formats", "title": "Kubernetes"},
    {"name": "nix", "section": "formats", "title": "Nix"},
    {"name": "rego", "section": "formats", "title": "Rego"},
    {"name": "cue", "section": "formats", "title": "CUE"},
    {"name": "gherkin", "section": "formats", "title": "Gherkin"},
    {"name": "html_template", "section": "formats", "title": "HTML templates"},
    {"name": "jsonnet", "section": "formats", "title": "Jsonnet"},
    {"name": "latex", "section": "formats", "title": "LaTeX"},
    {"name": "pkl", "section": "formats", "title": "Pkl"},
    {"name": "rst", "section": "formats", "title": "reStructuredText"},
    {"name": "asciidoc", "section": "formats", "title": "AsciiDoc"},
    {"name": "sql", "section": "formats", "title": "SQL"},
    {"name": "terraform", "section": "formats", "title": "Terraform"},
    {"name": "typst", "section": "formats", "title": "Typst"},
    {"name": "xml", "section": "formats", "title": "XML"},
    {"name": "yaml", "section": "formats", "title": "YAML"},
]

NOT_LANGUAGES = [
    "cli",
    "config",
    "deploy",
    "docs",
    "dx",
    "env",
    "examples",
    "gazelle",
    "generation",
    "libs",
    "modules",
    "patches",
    "quality",
    "third_party",
    "tools",
]

GAP_ISSUES = {
    "build:appimage:*": 4539,
    "build:apple:*": 4540,
    "build:cuda:*": 4541,
    "build:d:*": 4542,
    "build:dart:*": 4543,
    "build:elixir:*": 4544,
    "build:erlang:*": 4545,
    "build:fortran:*": 4546,
    "build:haskell:*": 4547,
    "build:julia:*": 4548,
    "build:msix:*": 4549,
    "build:ocaml:*": 4550,
    "build:perl:*": 4551,
    "build:php:*": 4552,
    "build:powershell:*": 4553,
    "build:qt:*": 4554,
    "consumer:*": 4436,
    "format:cloudformation": 4439,
    "format:fortran": 4685,
    "format:helm": 4439,
    "format:ocaml": 4686,
    "format:rst": 4439,
    "lint:apple": 4616,
    "lint:cuda": 4616,
    "lint:julia": 4623,
    "lint:lua": 4612,
    "lint:ocaml": 4621,
    "lint:pkl": 4623,
    "lint:typst": 4623,
    "lint:xml": 4618,
    "support:*": 4441,
    "typecheck:astro": 1767,
    "typecheck:d": 4530,
    "typecheck:fortran": 4530,
    "typecheck:fsharp": 4529,
    "typecheck:haskell": 4530,
    "typecheck:javascript": 4524,
    "typecheck:julia": 4532,
    "typecheck:kotlin": 4528,
    "typecheck:mdx": 1768,
    "typecheck:ocaml": 4530,
    "typecheck:pkl": 4533,
    "typecheck:ruby": 4532,
    "typecheck:scala": 4528,
    "typecheck:svelte": 1766,
    "typecheck:terraform": 4533,
    "typecheck:vue": 4525,
    "typecheck:zig": 4530,
}

GAP_ENFORCEMENT = True

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
    files["//tools/testing:nested_bazel.bzl"] = True
    files["//tools/testing/hermetic:defs.bzl"] = True
    return sorted(files.keys())
