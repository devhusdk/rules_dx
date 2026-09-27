"""Tested-stack manifest generator."""

load("//modules:versions.bzl", _ASPECT_RULES_JEST_VERSION = "ASPECT_RULES_JEST_VERSION", _ASPECT_RULES_JS_VERSION = "ASPECT_RULES_JS_VERSION", _ASPECT_RULES_PY_VERSION = "ASPECT_RULES_PY_VERSION", _ASPECT_RULES_TS_VERSION = "ASPECT_RULES_TS_VERSION", _BAZEL_LIB_VERSION = "BAZEL_LIB_VERSION", _BAZEL_SKYLIB_VERSION = "BAZEL_SKYLIB_VERSION", _BAZEL_VERSION = "BAZEL_VERSION", _DOTNET_VERSION = "DOTNET_VERSION", _GAZELLE_VERSION = "GAZELLE_VERSION", _GOOGLETEST_VERSION = "GOOGLETEST_VERSION", _GO_SDK_VERSION = "GO_SDK_VERSION", _PLATFORMS_VERSION = "PLATFORMS_VERSION", _PNPM_VERSION = "PNPM_VERSION", _PYTHON_VERSION = "PYTHON_VERSION", _RULES_CC_VERSION = "RULES_CC_VERSION", _RULES_DOTNET_VERSION = "RULES_DOTNET_VERSION", _RULES_GO_VERSION = "RULES_GO_VERSION", _RULES_JAVA_VERSION = "RULES_JAVA_VERSION", _RULES_JVM_EXTERNAL_VERSION = "RULES_JVM_EXTERNAL_VERSION", _RULES_KOTLIN_VERSION = "RULES_KOTLIN_VERSION", _RULES_POWERSHELL_VERSION = "RULES_POWERSHELL_VERSION", _RULES_PROTO_VERSION = "RULES_PROTO_VERSION", _RULES_PYTHON_VERSION = "RULES_PYTHON_VERSION", _RULES_RUST_PROST_VERSION = "RULES_RUST_PROST_VERSION", _RULES_RUST_VERSION = "RULES_RUST_VERSION", _RULES_SCALA_VERSION = "RULES_SCALA_VERSION", _RULES_SHELL_VERSION = "RULES_SHELL_VERSION", _RUST_VERSION = "RUST_VERSION", _SCALA_VERSION = "SCALA_VERSION", _TYPESCRIPT_VERSION = "TYPESCRIPT_VERSION")

_TESTED_DEPS = {
    "rules_rust": _RULES_RUST_VERSION,
    "rules_cc": _RULES_CC_VERSION,
    "googletest": _GOOGLETEST_VERSION,
    "rules_python": _RULES_PYTHON_VERSION,
    "rules_go": _RULES_GO_VERSION,
    "gazelle": _GAZELLE_VERSION,
    "rules_shell": _RULES_SHELL_VERSION,
    "platforms": _PLATFORMS_VERSION,
    "bazel_skylib": _BAZEL_SKYLIB_VERSION,
    "rules_proto": _RULES_PROTO_VERSION,
    "rules_rust_prost": _RULES_RUST_PROST_VERSION,
    "rules_java": _RULES_JAVA_VERSION,
    "rules_kotlin": _RULES_KOTLIN_VERSION,
    "rules_scala": _RULES_SCALA_VERSION,
    "rules_dotnet": _RULES_DOTNET_VERSION,
    "bazel_lib": _BAZEL_LIB_VERSION,
    "rules_jvm_external": _RULES_JVM_EXTERNAL_VERSION,
    "aspect_rules_py": _ASPECT_RULES_PY_VERSION,
    "aspect_rules_js": _ASPECT_RULES_JS_VERSION,
    "aspect_rules_ts": _ASPECT_RULES_TS_VERSION,
    "aspect_rules_jest": _ASPECT_RULES_JEST_VERSION,
    "rules_powershell": _RULES_POWERSHELL_VERSION,
}

def _tested_stack_impl(ctx):
    deps = dict(ctx.attr.deps)
    for name, version in _TESTED_DEPS.items():
        deps.setdefault(name, version)
    manifest = {
        "bazel_version": ctx.attr.bazel_version,
        "dotnet_version": ctx.attr.dotnet_version,
        "generator": "libs/testing/tested_stack.bzl",
        "go_sdk_version": ctx.attr.go_sdk_version,
        "module_deps": deps,
        "platforms": sorted(ctx.attr.platforms),
        "pnpm_version": ctx.attr.pnpm_version,
        "python_version": ctx.attr.python_version,
        "rules_cc_version": ctx.attr.rules_cc_version,
        "rules_rust_version": ctx.attr.rules_rust_version,
        "rust_version": ctx.attr.rust_version,
        "scala_version": ctx.attr.scala_version,
        "schema_version": 2,
        "typescript_version": ctx.attr.typescript_version,
    }
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode_indent(manifest, indent = "  ") + "\n")
    return [DefaultInfo(files = depset([out]))]

tested_stack = rule(
    implementation = _tested_stack_impl,
    attrs = {
        "bazel_version": attr.string(
            default = _BAZEL_VERSION,
        ),
        "deps": attr.string_dict(
            default = {},
        ),
        "dotnet_version": attr.string(
            default = _DOTNET_VERSION,
        ),
        "go_sdk_version": attr.string(
            default = _GO_SDK_VERSION,
        ),
        "platforms": attr.string_list(),
        "pnpm_version": attr.string(
            default = _PNPM_VERSION,
        ),
        "python_version": attr.string(
            default = _PYTHON_VERSION,
        ),
        "rules_cc_version": attr.string(
            default = _RULES_CC_VERSION,
        ),
        "rules_rust_version": attr.string(
            default = _RULES_RUST_VERSION,
        ),
        "rust_version": attr.string(
            default = _RUST_VERSION,
        ),
        "scala_version": attr.string(
            default = _SCALA_VERSION,
        ),
        "typescript_version": attr.string(
            default = _TYPESCRIPT_VERSION,
        ),
    },
)
