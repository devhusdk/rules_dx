"""The language list behind docs/support-matrix.md."""

load("//cuda/rules:defs.bzl", "CUDA_TARGET")
load("//d/rules:defs.bzl", "D_COMPATIBLE_WITH")
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
    "//elixir/rules:defs.bzl%ELIXIR_COMPATIBLE_WITH": _cells(OTP_CELLS, ELIXIR_CELLS),
    "//erlang/rules:defs.bzl%ERLANG_COMPATIBLE_WITH": _cells(OTP_CELLS, ERLANG_CELLS),
    "//fortran/rules:defs.bzl%FORTRAN_COMPATIBLE_WITH": _all_of(FORTRAN_COMPATIBLE_WITH),
    "//haskell/rules:defs.bzl%HASKELL_COMPATIBLE_WITH": _cells(GHC_CELLS, HASKELL_CELLS),
    "//julia/rules:defs.bzl%JULIA_COMPATIBLE_WITH": _os_cpu_pairs(JULIA_PLATFORMS),
    "//ocaml/rules:defs.bzl%OCAML_COMPATIBLE_WITH": _all_of(OCAML_COMPATIBLE_WITH),
    # PERL_COMPATIBLE_WITH is a select() on rules_perl_platform, which matches any of these.
    "//perl/rules:defs.bzl%PERL_COMPATIBLE_WITH": _os_cpu_pairs(RULES_PERL_PLATFORMS),
    "//php/rules:defs.bzl%PHP_COMPATIBLE_WITH": _all_of(PHP_COMPATIBLE_WITH),
}

def _hello(lang):
    return ["//" + lang + "/tests/fixtures/hello:hello_test"]

ROWS = [
    {"ci_tests": _hello("astro"), "name": "astro", "section": "languages", "starters": ["astro"], "title": "Astro"},
    {
        "ci_tests": _hello("cc"),
        "consumer_tests": ["//cc/tests/consumer:app_consumer_test", "//cc/tests/starter:starter_test"],
        "name": "cc",
        "section": "languages",
        "starters": ["c", "cpp"],
        "title": "C and C++",
    },
    {"ci_tests": _hello("csharp"), "name": "csharp", "section": "languages", "starters": ["csharp"], "title": "C#"},
    {
        "ci_tests": ["//cuda/tests:saxpy_device_code_test", "//cuda/tests:saxpy_gpu_test"],
        "consumer_tests": ["//cuda/tests/starter:starter_test"],
        "name": "cuda",
        "section": "languages",
        "starters": ["cuda"],
        "title": "CUDA",
    },
    {"ci_tests": ["//d/tests/fixtures/hello:greet_test"], "name": "d", "section": "languages", "title": "D"},
    {
        "ci_tests": ["//elixir/tests/runtime:probe_test"],
        "consumer_tests": ["//elixir/tests/starter:starter_test"],
        "name": "elixir",
        "section": "languages",
        "starters": ["elixir"],
        "title": "Elixir",
    },
    {
        "ci_tests": ["//erlang/tests/runtime:eunit"],
        "consumer_tests": ["//erlang/tests/starter:starter_test"],
        "name": "erlang",
        "section": "languages",
        "starters": ["erlang"],
        "title": "Erlang",
    },
    {"ci_tests": ["//fortran/tests/runtime:stats_test"], "name": "fortran", "section": "languages", "title": "Fortran"},
    {"ci_tests": _hello("fsharp"), "name": "fsharp", "section": "languages", "starters": ["fsharp"], "title": "F#"},
    {"ci_tests": _hello("go"), "name": "go", "section": "languages", "starters": ["go"], "title": "Go"},
    {"ci_tests": ["//haskell/tests/runtime:probe_test"], "consumer_tests": ["//haskell/tests/starter:starter_test"], "name": "haskell", "section": "languages", "starters": ["haskell"], "title": "Haskell"},
    {"ci_tests": _hello("java"), "name": "java", "section": "languages", "starters": ["java"], "title": "Java"},
    {"ci_tests": _hello("javascript"), "name": "javascript", "section": "languages", "starters": ["javascript"], "title": "JavaScript"},
    {"ci_tests": ["//julia/tests/fixtures/hello:greet_test"], "name": "julia", "section": "languages", "title": "Julia"},
    {"ci_tests": _hello("kotlin"), "name": "kotlin", "section": "languages", "starters": ["kotlin"], "title": "Kotlin"},
    {"ci_tests": ["//lua/tests/runtime:probe_test"], "name": "lua", "section": "languages", "title": "Lua"},
    {"ci_tests": ["//mdx/tests/app:site_static_test"], "name": "mdx", "section": "languages", "starters": ["mdx"], "title": "MDX"},
    {"ci_tests": ["//ocaml/tests/runtime:greet_test"], "name": "ocaml", "section": "languages", "starters": ["ocaml"], "title": "OCaml"},
    {"ci_tests": ["//perl/tests/runtime:probe_test"], "name": "perl", "section": "languages", "title": "Perl"},
    {"ci_tests": ["//php/tests/fixtures/hello:greeter_test"], "name": "php", "section": "languages", "title": "PHP"},
    {"ci_tests": _hello("powershell"), "name": "powershell", "section": "languages", "starters": ["powershell"], "title": "PowerShell"},
    {
        "ci_tests": ["//python/tests/fixtures/entries:helper_test"],
        "consumer_tests": ["//python/tests/consumer:service_consumer_test"],
        "name": "python",
        "section": "languages",
        "starters": ["python"],
        "title": "Python",
    },
    {"ci_tests": _hello("ruby"), "name": "ruby", "section": "languages", "starters": ["ruby"], "title": "Ruby"},
    {
        "ci_tests": ["//rust/tests/fixtures/consumer_wasm:consumer_wasm_test"],
        "name": "rust",
        "section": "languages",
        "starters": ["rust", "rust-android", "rust-web"],
        "title": "Rust",
    },
    {"ci_tests": _hello("scala"), "name": "scala", "section": "languages", "starters": ["scala"], "title": "Scala"},
    {"ci_tests": _hello("svelte"), "name": "svelte", "section": "languages", "starters": ["svelte"], "title": "Svelte"},
    {"ci_tests": _hello("typescript"), "name": "typescript", "section": "languages", "starters": ["typescript"], "title": "TypeScript"},
    {"ci_tests": _hello("vue"), "name": "vue", "section": "languages", "starters": ["vue"], "title": "Vue"},
    {"ci_tests": ["//zig/tests/fixtures/hello:greet_test"], "name": "zig", "section": "languages", "starters": ["zig"], "title": "Zig"},
    {
        "consumer_tests": ["//android/tests/starter:starter_test"],
        "families": [],
        "name": "android",
        "section": "targets",
        "title": "Android",
    },
    {
        "ci_tests": ["//appimage/tests:appimage_test"],
        "families": [],
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
    {"families": ["dockerfile"], "name": "dockerfile", "section": "targets", "title": "Dockerfile"},
    {"families": ["make"], "name": "make", "section": "targets", "title": "Make"},
    {"families": [], "name": "msix", "section": "targets", "title": "MSIX"},
    {
        "ci_tests": ["//qt/tests:qt_tasks_test"],
        "consumer_tests": ["//qt/tests:qt_app_consumer_test"],
        "families": ["qml"],
        "name": "qt",
        "section": "targets",
        "title": "Qt",
    },
    {"ci_tests": ["//wasi/testing:wasi_run_unit_test"], "families": [], "name": "wasi", "section": "targets", "title": "WASI"},
    {"name": "api_spec", "section": "formats", "title": "API specs"},
    {"name": "ansible", "section": "formats", "title": "Ansible"},
    {"name": "flatbuffers", "section": "formats", "title": "FlatBuffers"},
    {"name": "jupyter", "section": "formats", "title": "Jupyter"},
    {"name": "kubernetes", "section": "formats", "title": "Kubernetes"},
    {"name": "nix", "section": "formats", "title": "Nix"},
    {"name": "rego", "section": "formats", "title": "Rego"},
    {"name": "cue", "section": "formats", "title": "CUE"},
    {"name": "gherkin", "section": "formats", "title": "Gherkin"},
    {"name": "html_template", "section": "formats", "title": "HTML templates"},
    {"name": "jsonnet", "section": "formats", "title": "Jsonnet"},
    {"name": "pkl", "section": "formats", "title": "Pkl"},
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
    "build:*": 4435,
    "consumer:*": 4436,
    "format:*": 4439,
    "lint:*": 4438,
    "starter:*": 4437,
    "support:*": 4441,
    "typecheck:*": 4440,
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
    return sorted(files.keys())
