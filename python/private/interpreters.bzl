"""The rules_dx python extension: one interpreter toolchain hub with no version target setting."""

load("//core:root_only.bzl", "warn_dropped_tags")

_TOOLCHAINS_ON = str(Label("//config:toolchains_on"))

_EXEC_TOOLS_ENABLED = str(Label("@rules_python//python/config_settings:is_exec_tools_toolchain_enabled"))

_PLATFORMS = {
    "aarch64-apple-darwin": struct(os = "macos", cpu = "aarch64"),
    "aarch64-pc-windows-msvc": struct(os = "windows", cpu = "aarch64"),
    "aarch64-unknown-linux-gnu": struct(os = "linux", cpu = "aarch64"),
    "x86_64-pc-windows-msvc": struct(os = "windows", cpu = "x86_64"),
    "x86_64-unknown-linux-gnu": struct(os = "linux", cpu = "x86_64"),
}

def _repo_name(version, triple):
    return "python_{}_{}".format(version.replace(".", "_"), triple.replace("-", "_"))

_TOOLCHAIN = """
toolchain(
    name = "{name}",
    {constraint_attr} = {constraints},
    target_settings = {settings},
    toolchain = "@{repo}//:{target}",
    toolchain_type = "{toolchain_type}",
)"""

_WINDOWS_TOOLCHAIN = """
toolchain(
    name = "{name}",
    {constraint_attr} = ["@platforms//os:windows", "@platforms//cpu:{cpu}"],
    target_settings = {settings},
    toolchain = "{toolchain}",
    toolchain_type = "{toolchain_type}",
)"""

_WINDOWS_RUNTIMES = {
    "aarch64": "@python_3_12_aarch64-pc-windows-msvc",
    "x86_64": "@python_3_12_x86_64-pc-windows-msvc",
}

def _hub_impl(rctx):
    version = rctx.attr.python_version
    lines = [
        'package(default_visibility = ["//visibility:public"])',
    ]
    for triple, platform in sorted(_PLATFORMS.items()):
        repo = _repo_name(version, triple)
        constraints = [
            "@platforms//os:" + platform.os,
            "@platforms//cpu:" + platform.cpu,
        ]
        settings = [_TOOLCHAINS_ON]
        for suffix, target, toolchain_type, target_side in [
            ("", "runtime", "@bazel_tools//tools/python:toolchain_type", True),
            ("_py_cc", "py_cc_toolchain", "@rules_python//python/cc:toolchain_type", True),
            ("_exec_tools", "runtime", "@rules_py//py/private/toolchain:exec_tools_toolchain_type", False),
        ]:
            constraint_attr = "target_compatible_with" if target_side else "exec_compatible_with"
            lines.append(_TOOLCHAIN.format(
                name = repo + suffix,
                constraint_attr = constraint_attr,
                constraints = repr(constraints),
                settings = repr(settings if target_side else [_TOOLCHAINS_ON]),
                repo = repo,
                target = target,
                toolchain_type = toolchain_type,
            ))
    for cpu, repo in sorted(_WINDOWS_RUNTIMES.items()):
        for kind, target, toolchain_type, target_side, settings in [
            ("runtime", "python_runtimes", "@rules_python//python:toolchain_type", True, [_TOOLCHAINS_ON]),
            ("py_cc", "py_cc_toolchain", "@rules_python//python/cc:toolchain_type", True, [_TOOLCHAINS_ON]),
            ("exec_tools", "py_exec_tools_toolchain", "@rules_python//python:exec_tools_toolchain_type", False, [_EXEC_TOOLS_ENABLED, _TOOLCHAINS_ON]),
        ]:
            lines.append(_WINDOWS_TOOLCHAIN.format(
                name = "cpython_windows_{}_{}".format(cpu, kind),
                constraint_attr = "target_compatible_with" if target_side else "exec_compatible_with",
                cpu = cpu,
                settings = repr(settings),
                toolchain = str(Label("{}//:{}".format(repo, target))),
                toolchain_type = str(Label(toolchain_type)),
            ))
    rctx.file("BUILD.bazel", "\n".join(lines) + "\n")
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    attrs = {"python_version": attr.string(mandatory = True, doc = "Pinned Python version the hub registers toolchains for.")},
)

def _impl(module_ctx):
    pins = json.decode(module_ctx.read(Label("//python:pins.json")))
    version = pins["default_version"]
    warn_dropped_tags("python", module_ctx.modules, ["toolchain"])
    for mod in module_ctx.modules:
        if not mod.is_root:
            continue
        for tag in mod.tags.toolchain:
            version = tag.python_version
    if version not in pins["versions"]:
        fail("python_version {} is not pinned; pinned: {}".format(version, ", ".join(pins["versions"])))
    _hub(name = "dx_python_toolchains", python_version = version)
    return module_ctx.extension_metadata(reproducible = True)

python = module_extension(
    implementation = _impl,
    tag_classes = {
        "toolchain": tag_class(attrs = {"python_version": attr.string(mandatory = True, doc = "Pinned Python version of the default toolchain.")}),
    },
)
