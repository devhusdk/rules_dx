"""Package-boundary visibility contract (single source)."""

VIS_PUBLIC = ["//visibility:public"]

VIS_PRIVATE = ["//visibility:private"]

VIS_INTERNAL = ["//:__subpackages__"]

VIS_CLI = [
    "//cli:__pkg__",
    "//cli:__subpackages__",
]

VIS_CLI_ENV_QUALITY = VIS_CLI + [
    "//env:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_GENERATION_QUALITY = VIS_CLI + [
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_DEPLOY_QUALITY = VIS_CLI + [
    "//deploy:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_DEPLOY_GENERATION_QUALITY = VIS_CLI + [
    "//deploy:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_TOOLS = VIS_CLI + [
    "//tools:__subpackages__",
]

VIS_CLI_ENV_GENERATION_QUALITY = VIS_CLI + [
    "//env:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_WIDE = VIS_CLI + [
    "//docs:__subpackages__",
    "//env:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

# The platform facts every package reads, so tools and deploy resolve them too.
VIS_CLI_PLATFORM = VIS_CLI_WIDE + [
    "//deploy:__subpackages__",
    "//tools:__subpackages__",
]

VIS_CLI_DEPLOY = VIS_CLI + [
    "//deploy:__subpackages__",
]

VIS_CLI_DOCS_GENERATION_QUALITY = VIS_CLI + [
    "//docs:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_ENV_SHARD = VIS_CLI + [
    "//env:__pkg__",
    "//env:__subpackages__",
]

VIS_CLI_GENERATION = VIS_CLI + [
    "//generation:__pkg__",
]

VIS_QUALITY = [
    "//quality:__pkg__",
    "//quality:__subpackages__",
]

VIS_QUALITY_ENV = [
    "//env:__pkg__",
] + VIS_QUALITY

VIS_QUALITY_CLI = VIS_CLI + VIS_QUALITY

PUBLIC_PACKAGES = [
    "astro/rules",
    "cc/rules",
    "config",
    "csharp/rules",
    "deploy/offline",
    "deploy/rules",
    "env",
    "fsharp/rules",
    "generation",
    "go/rules",
    "java/rules",
    "javascript/rules",
    "kotlin/rules",
    "mdx/rules",
    "modules",
    "powershell/rules",
    "python/rules",
    "quality",
    "ruby/rules",
    "rust/rules",
    "scala/rules",
    "svelte/rules",
    "typescript/rules",
    "vue/rules",
]

PRIVATE_ENV_PACKAGES = [
    "astro/env",
    "cc/env",
    "csharp/env",
    "fsharp/env",
    "go/env",
    "java/env",
    "javascript/env",
    "kotlin/env",
    "mdx/env",
    "powershell/env",
    "python/env",
    "ruby/env",
    "rust/env",
    "scala/env",
    "svelte/env",
    "typescript/env",
    "vue/env",
]

PRIVATE_GAZELLE_PACKAGES = [
    "gazelle/astro",
    "gazelle/cc",
    "gazelle/csharp",
    "gazelle/dispatch",
    "gazelle/fsharp",
    "gazelle/go",
    "gazelle/java",
    "gazelle/javascript",
    "gazelle/kotlin",
    "gazelle/mdx",
    "gazelle/python",
    "gazelle/ruby",
    "gazelle/rust",
    "gazelle/scala",
    "gazelle/svelte",
    "gazelle/typescript",
    "gazelle/vue",
]

DX_FACADE_PACKAGE = "dx"

SCOPED_PACKAGES = {
    "cli/adopt": "VIS_CLI",
    "cli/apply": "VIS_CLI",
    "cli/atomic_fs": "VIS_CLI_ENV_QUALITY",
    "cli/audit": "VIS_CLI",
    "cli/bep": "VIS_CLI",
    "cli/bump": "VIS_CLI",
    "cli/clean": "VIS_CLI",
    "cli/cli": "VIS_CLI",
    "cli/codegen": "VIS_CLI",
    "cli/diff": "VIS_CLI",
    "cli/digest": "VIS_CLI_DEPLOY_GENERATION_QUALITY",
    "cli/env": "VIS_CLI",
    "cli/env_plan": "VIS_CLI",
    "cli/fingerprint": "VIS_CLI_DEPLOY_QUALITY",
    "cli/identity": "VIS_CLI",
    "cli/lcov": "VIS_CLI_TOOLS",
    "cli/output": "VIS_CLI_ENV_GENERATION_QUALITY",
    "cli/path": "VIS_CLI_WIDE",
    "cli/process": "VIS_CLI_DEPLOY",
    "cli/proto_validate": "VIS_CLI_WIDE",
    "cli/roots": "VIS_CLI",
    "cli/schema": "VIS_CLI_DOCS_GENERATION_QUALITY",
    "cli/setup": "VIS_CLI",
    "cli/test_scratch": "VIS_CLI",
    "cli/update": "VIS_CLI",
    "env/env_shard": "VIS_ENV_SHARD",
    "generation/codegen_shard": "VIS_CLI_GENERATION",
    "generation/result": "VIS_CLI",
    "libs/gomod": "VIS_CLI_TOOLS",
    "quality/adapter": "VIS_QUALITY",
    "quality/artifacts": "VIS_QUALITY",
    "quality/evaluator": "VIS_QUALITY",
    "quality/markdown": "VIS_QUALITY_ENV",
    "quality/matrix": "VIS_QUALITY",
    "quality/result": "VIS_QUALITY_CLI",
    "quality/runner": "VIS_QUALITY",
    "quality/testdata": "VIS_QUALITY",
}

EXPLICIT_PUBLIC_TARGETS = {
    "": ["dx"],
    "cli/cli": ["dx", "man_pages"],
    "cli/env": ["env"],
    "deploy/rules": ["archiver", "dx_deploy_tools", "hasher"],
    "dx": ["codegen", "config", "env", "generate", "generate_check"],
}

EXPLICIT_PUBLIC_EXPORTS = [
    "cli/cli",
    "cli/env",
]

SCOPED_TARGET_GRANTS = [
    ["quality/artifacts", "exports_files", "//tools/ci:__pkg__"],
    ["quality/testdata", "exports_files", "//tools/ci:__pkg__"],
]

LAYER_FORBIDDEN_DEPS = [
    ["cli/", "//dx:", ["//dx:codegen", "//dx:env"]],
    ["cli/", "//docs:", []],
    ["cli/", "//tools/", []],
    ["dx/", "//tools/", []],
    ["env/", "//dx:", []],
    ["gazelle/", "//dx:", []],
    ["generation/", "//dx:", []],
    ["quality/", "//dx:", []],
    ["tools/", "//dx:", []],
]

def is_public_package(pkg):
    """Reports whether a package directory carries the public default."""
    return pkg in PUBLIC_PACKAGES

def is_private_package(pkg):
    """Reports whether a package directory carries the private default."""
    return pkg in PRIVATE_ENV_PACKAGES or pkg in PRIVATE_GAZELLE_PACKAGES or pkg == DX_FACADE_PACKAGE

def scoped_constant(pkg):
    """Returns the scope constant a scoped package must load ("" when none)."""
    return SCOPED_PACKAGES.get(pkg, "")
