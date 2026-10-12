"""Frozen v1 acceptance inventory: owner, executable evidence, applicability."""

load("//:v1_promises.bzl", "V1_PROMISES_ROOT")
load("//.github:v1_promises.bzl", "V1_PROMISES_GITHUB")
load("//astro/rules:v1_promises.bzl", "V1_PROMISES_ASTRO_RULES")
load("//cc/rules:v1_promises.bzl", "V1_PROMISES_CC_RULES")
load("//cli/cli:v1_promises.bzl", "V1_PROMISES_CLI_CLI")
load("//cli/process:v1_promises.bzl", "V1_PROMISES_CLI_PROCESS")
load("//csharp/rules:v1_promises.bzl", "V1_PROMISES_CSHARP_RULES")
load("//deploy/install:v1_promises.bzl", "V1_PROMISES_DEPLOY_INSTALL")
load("//deploy/offline:v1_promises.bzl", "V1_PROMISES_DEPLOY_OFFLINE")
load("//deploy/release:v1_promises.bzl", "V1_PROMISES_DEPLOY_RELEASE")
load("//deploy/rules:v1_promises.bzl", "V1_PROMISES_DEPLOY_RULES")
load("//docs:v1_promises.bzl", "V1_PROMISES_DOCS")
load("//examples:v1_promises.bzl", "V1_PROMISES_EXAMPLES")
load("//fsharp/rules:v1_promises.bzl", "V1_PROMISES_FSHARP_RULES")
load("//go/rules:v1_promises.bzl", "V1_PROMISES_GO_RULES")
load("//java/rules:v1_promises.bzl", "V1_PROMISES_JAVA_RULES")
load("//javascript/rules:v1_promises.bzl", "V1_PROMISES_JAVASCRIPT_RULES")
load("//kotlin/rules:v1_promises.bzl", "V1_PROMISES_KOTLIN_RULES")
load("//mdx/rules:v1_promises.bzl", "V1_PROMISES_MDX_RULES")
load("//modules:v1_promises.bzl", "V1_PROMISES_MODULES")
load("//powershell/rules:v1_promises.bzl", "V1_PROMISES_POWERSHELL_RULES")
load("//python/rules:v1_promises.bzl", "V1_PROMISES_PYTHON_RULES")
load("//quality:v1_promises.bzl", "V1_PROMISES_QUALITY")
load("//quality/markdown:v1_promises.bzl", "V1_PROMISES_QUALITY_MARKDOWN")
load("//quality/tools/javascript/bin:v1_promises.bzl", "V1_PROMISES_QUALITY_TOOLS_JAVASCRIPT_BIN")
load("//quality/tools/jvm:v1_promises.bzl", "V1_PROMISES_QUALITY_TOOLS_JVM")
load("//quality/tools/python:v1_promises.bzl", "V1_PROMISES_QUALITY_TOOLS_PYTHON")
load("//ruby/rules:v1_promises.bzl", "V1_PROMISES_RUBY_RULES")
load("//rust/rules:v1_promises.bzl", "V1_PROMISES_RUST_RULES")
load("//rust/toolchains:v1_promises.bzl", "V1_PROMISES_RUST_TOOLCHAINS")
load("//scala/rules:v1_promises.bzl", "V1_PROMISES_SCALA_RULES")
load("//svelte/rules:v1_promises.bzl", "V1_PROMISES_SVELTE_RULES")
load("//typescript/rules:v1_promises.bzl", "V1_PROMISES_TYPESCRIPT_RULES")
load("//vue/rules:v1_promises.bzl", "V1_PROMISES_VUE_RULES")

FROZEN_BASE_REVISION = "4116a49925f7aa71c564485f066e6ccd3c46d8df"
FROZEN_ON = "2026-10-09"

PROMISE_COUNT = 354

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

PROMISES = sorted(
    V1_PROMISES_ROOT + V1_PROMISES_GITHUB + V1_PROMISES_ASTRO_RULES + V1_PROMISES_CC_RULES + V1_PROMISES_CLI_CLI + V1_PROMISES_CLI_PROCESS + V1_PROMISES_CSHARP_RULES + V1_PROMISES_DEPLOY_INSTALL + V1_PROMISES_DEPLOY_OFFLINE + V1_PROMISES_DEPLOY_RELEASE + V1_PROMISES_DEPLOY_RULES + V1_PROMISES_DOCS + V1_PROMISES_EXAMPLES + V1_PROMISES_FSHARP_RULES + V1_PROMISES_GO_RULES + V1_PROMISES_JAVA_RULES + V1_PROMISES_JAVASCRIPT_RULES + V1_PROMISES_KOTLIN_RULES + V1_PROMISES_MDX_RULES + V1_PROMISES_MODULES + V1_PROMISES_POWERSHELL_RULES + V1_PROMISES_PYTHON_RULES + V1_PROMISES_QUALITY_MARKDOWN + V1_PROMISES_QUALITY_TOOLS_JAVASCRIPT_BIN + V1_PROMISES_QUALITY_TOOLS_JVM + V1_PROMISES_QUALITY_TOOLS_PYTHON + V1_PROMISES_QUALITY + V1_PROMISES_RUBY_RULES + V1_PROMISES_RUST_RULES + V1_PROMISES_RUST_TOOLCHAINS + V1_PROMISES_SCALA_RULES + V1_PROMISES_SVELTE_RULES + V1_PROMISES_TYPESCRIPT_RULES + V1_PROMISES_VUE_RULES,
    key = lambda row: row["id"],
)

def _evidence_targets():
    seen = {}
    for row in PROMISES:
        for target in row["evidence"]:
            seen[target] = True
    return sorted(seen.keys())

V1_EVIDENCE = _evidence_targets()
