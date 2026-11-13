"""The rules_dx python extension: one interpreter toolchain hub with no version target setting."""

_TOOLCHAINS_ON = str(Label("//config:toolchains_on"))
_LIBC_FLAG = "@rules_py//uv/private/constraints/platform:platform_libc"

_PLATFORMS = {
    "aarch64-apple-darwin": struct(os = "macos", cpu = "aarch64", libc = None),
    "aarch64-pc-windows-msvc": struct(os = "windows", cpu = "aarch64", libc = None),
    "aarch64-unknown-linux-gnu": struct(os = "linux", cpu = "aarch64", libc = "glibc"),
    "x86_64-pc-windows-msvc": struct(os = "windows", cpu = "x86_64", libc = None),
    "x86_64-unknown-linux-gnu": struct(os = "linux", cpu = "x86_64", libc = "glibc"),
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

def _hub_impl(rctx):
    version = rctx.attr.python_version
    lines = [
        'package(default_visibility = ["//visibility:public"])',
        "",
        "config_setting(",
        '    name = "libc_glibc",',
        '    flag_values = {"%s": "glibc"},' % _LIBC_FLAG,
        ")",
    ]
    for triple, platform in sorted(_PLATFORMS.items()):
        repo = _repo_name(version, triple)
        constraints = [
            "@platforms//os:" + platform.os,
            "@platforms//cpu:" + platform.cpu,
        ]
        settings = [_TOOLCHAINS_ON] + ([":libc_glibc"] if platform.libc else [])
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
    rctx.file("BUILD.bazel", "\n".join(lines) + "\n")
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    attrs = {"python_version": attr.string(mandatory = True, doc = "Pinned Python version the hub registers toolchains for.")},
)

def _impl(module_ctx):
    pins = json.decode(module_ctx.read(Label("//python:pins.json")))
    version = pins["default_version"]
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
