"""Frozen v1 acceptance inventory: owner, executable evidence, applicability."""

load("//:v1_promises.bzl", ROOT = "V1_PROMISES")
load("//.github:v1_promises.bzl", GITHUB = "V1_PROMISES")
load("//astro/rules:v1_promises.bzl", ASTRO_RULES = "V1_PROMISES")
load("//cc/rules:v1_promises.bzl", CC_RULES = "V1_PROMISES")
load("//cli/cli:v1_promises.bzl", CLI_CLI = "V1_PROMISES")
load("//cli/process:v1_promises.bzl", CLI_PROCESS = "V1_PROMISES")
load("//csharp/rules:v1_promises.bzl", CSHARP_RULES = "V1_PROMISES")
load("//cue/rules:v1_promises.bzl", CUE_RULES = "V1_PROMISES")
load("//deploy/install:v1_promises.bzl", DEPLOY_INSTALL = "V1_PROMISES")
load("//deploy/offline:v1_promises.bzl", DEPLOY_OFFLINE = "V1_PROMISES")
load("//deploy/release:v1_promises.bzl", DEPLOY_RELEASE = "V1_PROMISES")
load("//deploy/rules:v1_promises.bzl", DEPLOY_RULES = "V1_PROMISES")
load("//elixir/rules:v1_promises.bzl", ELIXIR_RULES = "V1_PROMISES")
load("//erlang/rules:v1_promises.bzl", ERLANG_RULES = "V1_PROMISES")
load("//examples:v1_promises.bzl", EXAMPLES = "V1_PROMISES")
load("//fsharp/rules:v1_promises.bzl", FSHARP_RULES = "V1_PROMISES")
load("//gleam/rules:v1_promises.bzl", GLEAM_RULES = "V1_PROMISES")
load("//go/rules:v1_promises.bzl", GO_RULES = "V1_PROMISES")
load("//html_template/rules:v1_promises.bzl", HTML_TEMPLATE_RULES = "V1_PROMISES")
load("//java/rules:v1_promises.bzl", JAVA_RULES = "V1_PROMISES")
load("//javascript/rules:v1_promises.bzl", JAVASCRIPT_RULES = "V1_PROMISES")
load("//jsonnet/rules:v1_promises.bzl", JSONNET_RULES = "V1_PROMISES")
load("//kotlin/rules:v1_promises.bzl", KOTLIN_RULES = "V1_PROMISES")
load("//mdx/rules:v1_promises.bzl", MDX_RULES = "V1_PROMISES")
load("//modules:v1_promises.bzl", MODULES = "V1_PROMISES")
load("//powershell/rules:v1_promises.bzl", POWERSHELL_RULES = "V1_PROMISES")
load("//python/rules:v1_promises.bzl", PYTHON_RULES = "V1_PROMISES")
load("//qt/rules:v1_promises.bzl", QT_RULES = "V1_PROMISES")
load("//quality:v1_promises.bzl", QUALITY = "V1_PROMISES")
load("//quality/markdown:v1_promises.bzl", QUALITY_MARKDOWN = "V1_PROMISES")
load("//quality/tools/dotnet:v1_promises.bzl", QUALITY_TOOLS_DOTNET = "V1_PROMISES")
load("//quality/tools/dotnet/fsharplint:v1_promises.bzl", QUALITY_TOOLS_DOTNET_FSHARPLINT = "V1_PROMISES")
load("//quality/tools/javascript/bin:v1_promises.bzl", QUALITY_TOOLS_JAVASCRIPT_BIN = "V1_PROMISES")
load("//quality/tools/jvm:v1_promises.bzl", QUALITY_TOOLS_JVM = "V1_PROMISES")
load("//quality/tools/powershell:v1_promises.bzl", QUALITY_TOOLS_POWERSHELL = "V1_PROMISES")
load("//quality/tools/python:v1_promises.bzl", QUALITY_TOOLS_PYTHON = "V1_PROMISES")
load("//quality/tools/ruby:v1_promises.bzl", QUALITY_TOOLS_RUBY = "V1_PROMISES")
load("//ruby/rules:v1_promises.bzl", RUBY_RULES = "V1_PROMISES")
load("//rust/rules:v1_promises.bzl", RUST_RULES = "V1_PROMISES")
load("//rust/toolchains:v1_promises.bzl", RUST_TOOLCHAINS = "V1_PROMISES")
load("//scala/rules:v1_promises.bzl", SCALA_RULES = "V1_PROMISES")
load("//svelte/rules:v1_promises.bzl", SVELTE_RULES = "V1_PROMISES")
load("//terraform/rules:v1_promises.bzl", TERRAFORM_RULES = "V1_PROMISES")
load("//tools/docs:v1_promises.bzl", TOOLS_DOCS = "V1_PROMISES")
load("//typescript/rules:v1_promises.bzl", TYPESCRIPT_RULES = "V1_PROMISES")
load("//vue/rules:v1_promises.bzl", VUE_RULES = "V1_PROMISES")
load("//yaml/rules:v1_promises.bzl", YAML_RULES = "V1_PROMISES")
load("//zig/rules:v1_promises.bzl", ZIG_RULES = "V1_PROMISES")

FROZEN_BASE_REVISION = "4116a49925f7aa71c564485f066e6ccd3c46d8df"
FROZEN_ON = "2026-10-09"

PROMISE_COUNT = 401

PROMISE_FAMILIES = [
    "artifact",
    "aspect",
    "class",
    "command",
    "curated",
    "deferred",
    "dep",
    "deploy",
    "doc",
    "example",
    "exit",
    "family",
    "platform",
    "report",
    "tool",
    "version",
    "workflow",
]

SHARDS = {
    "//": ROOT,
    "//.github": GITHUB,
    "//astro/rules": ASTRO_RULES,
    "//cc/rules": CC_RULES,
    "//cli/cli": CLI_CLI,
    "//cli/process": CLI_PROCESS,
    "//csharp/rules": CSHARP_RULES,
    "//cue/rules": CUE_RULES,
    "//deploy/install": DEPLOY_INSTALL,
    "//deploy/offline": DEPLOY_OFFLINE,
    "//deploy/release": DEPLOY_RELEASE,
    "//deploy/rules": DEPLOY_RULES,
    "//elixir/rules": ELIXIR_RULES,
    "//erlang/rules": ERLANG_RULES,
    "//examples": EXAMPLES,
    "//fsharp/rules": FSHARP_RULES,
    "//gleam/rules": GLEAM_RULES,
    "//go/rules": GO_RULES,
    "//html_template/rules": HTML_TEMPLATE_RULES,
    "//java/rules": JAVA_RULES,
    "//javascript/rules": JAVASCRIPT_RULES,
    "//jsonnet/rules": JSONNET_RULES,
    "//kotlin/rules": KOTLIN_RULES,
    "//mdx/rules": MDX_RULES,
    "//modules": MODULES,
    "//powershell/rules": POWERSHELL_RULES,
    "//python/rules": PYTHON_RULES,
    "//qt/rules": QT_RULES,
    "//quality": QUALITY,
    "//quality/markdown": QUALITY_MARKDOWN,
    "//quality/tools/dotnet": QUALITY_TOOLS_DOTNET,
    "//quality/tools/dotnet/fsharplint": QUALITY_TOOLS_DOTNET_FSHARPLINT,
    "//quality/tools/javascript/bin": QUALITY_TOOLS_JAVASCRIPT_BIN,
    "//quality/tools/jvm": QUALITY_TOOLS_JVM,
    "//quality/tools/powershell": QUALITY_TOOLS_POWERSHELL,
    "//quality/tools/python": QUALITY_TOOLS_PYTHON,
    "//quality/tools/ruby": QUALITY_TOOLS_RUBY,
    "//ruby/rules": RUBY_RULES,
    "//rust/rules": RUST_RULES,
    "//rust/toolchains": RUST_TOOLCHAINS,
    "//scala/rules": SCALA_RULES,
    "//svelte/rules": SVELTE_RULES,
    "//terraform/rules": TERRAFORM_RULES,
    "//tools/docs": TOOLS_DOCS,
    "//typescript/rules": TYPESCRIPT_RULES,
    "//vue/rules": VUE_RULES,
    "//yaml/rules": YAML_RULES,
    "//zig/rules": ZIG_RULES,
}

def _id(promise):
    """Returns the promise ID that orders the inventory."""
    return promise["id"]

PROMISES = sorted([promise for rows in SHARDS.values() for promise in rows], key = _id)

V1_EVIDENCE = sorted({target: True for promise in PROMISES for target in promise["evidence"]}.keys())
