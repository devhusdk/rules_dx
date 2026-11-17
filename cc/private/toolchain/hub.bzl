"""The dx_cc_toolchains hub: every C and C++ toolchain, named in registration order."""

load("@apple_support//configs:platforms.bzl", "APPLE_PLATFORMS_CONSTRAINTS")
load("@llvm//platforms:common.bzl", "MSVC_TARGET_BOOTSTRAP_SUPPORTED_EXECS", "MSVC_TARGET_STAGE0_SUPPORTED_EXECS", "SUPPORTED_EXECS", "SUPPORTED_TARGETS", "WINDOWS_TARGETS")
load("//cc/private/toolchain/msvc:layout.bzl", "SUPPORTED_CELLS")
load("//cc/private/toolchain/msvc:repos.bzl", "cell_name")
load("//core:toolchain.bzl", "toolchain_target")

CC_TOOLCHAIN_TYPE = "@bazel_tools//tools/cpp:toolchain_type"

_GCC = str(Label("//cc/private/toolchain:gcc_selected"))
_MSVC = str(Label("//cc/private/toolchain:msvc_selected"))
_ON = str(Label("//config:toolchains_on"))
_STAGE0 = str(Label("@llvm//toolchain:bootstrap_stage0_prebuilt_seed"))

_BOOTSTRAP_STAGES = [
    ("stage3", str(Label("@llvm//toolchain:bootstrap_stage3_lto_and_fdo_applied"))),
    ("stage2", str(Label("@llvm//toolchain:bootstrap_stage2_lto_and_fdo_instrumented"))),
    ("stage1", str(Label("@llvm//toolchain:bootstrap_stage1_from_source"))),
]

_MSVC_CPU = {"arm64": "arm64", "x64": "x86_64"}

_MSVC_SUFFIX = {
    ("arm64", "arm64"): "arm64",
    ("x64", "arm64"): "x86_64_to_arm64",
    ("x64", "x64"): "x86_64",
}

def _platform(os, cpu):
    return ["@platforms//os:" + os, "@platforms//cpu:" + cpu]

def _entry(name, toolchain, exec_compatible_with = None, target_compatible_with = None, target_settings = None):
    return struct(
        name = name,
        exec_compatible_with = exec_compatible_with,
        target_compatible_with = target_compatible_with,
        target_settings = target_settings,
        toolchain = str(Label(toolchain)),
    )

def _gcc():
    pairs = [
        ("x86_64", "x86_64", "dx_gcc_linux_x86_64"),
        ("x86_64", "arm64", "dx_gcc_linux_aarch64_cross"),
        ("arm64", "arm64", "dx_gcc_linux_aarch64"),
    ]
    return [
        _entry(
            name = "gcc_linux_" + (exec_cpu if exec_cpu == target_cpu else exec_cpu + "_to_" + target_cpu),
            exec_compatible_with = _platform("linux", exec_cpu),
            target_compatible_with = _platform("linux", target_cpu),
            target_settings = [_GCC],
            toolchain = "@%s//:_cc_toolchain" % repo,
        )
        for exec_cpu, target_cpu, repo in pairs
    ] + [_entry(name = "gcc_unsupported", target_settings = [_GCC], toolchain = "//cc/private/toolchain:gcc_unsupported_impl")]

def _msvc():
    return [
        _entry(
            name = "msvc_windows_" + _MSVC_SUFFIX[cell],
            exec_compatible_with = _platform("windows", _MSVC_CPU[cell[0]]),
            target_compatible_with = _platform("windows", _MSVC_CPU[cell[1]]),
            target_settings = [_MSVC],
            toolchain = "@dx_msvc//:" + cell_name(cell[0], cell[1]),
        )
        for cell in SUPPORTED_CELLS
    ] + [_entry(name = "msvc_unsupported", target_settings = [_MSVC], toolchain = "//cc/private/toolchain/msvc:unsupported")]

def _fuzzer():
    return [
        _entry(
            name = "llvm_fuzzer_macos_" + cpu,
            exec_compatible_with = _platform("macos", cpu),
            target_compatible_with = _platform("macos", cpu),
            target_settings = [_ON, str(Label("@llvm//config:fuzzer_enabled")), _STAGE0],
            toolchain = "@llvm//toolchain:macos_%s_cc_toolchain" % llvm_cpu,
        )
        for cpu, llvm_cpu in [("arm64", "aarch64"), ("x86_64", "x86_64")]
    ]

def _apple():
    return sorted([
        _entry(
            name = "xcode_" + arch + "_" + cpu,
            exec_compatible_with = _platform("macos", cpu),
            target_compatible_with = [str(c) for c in APPLE_PLATFORMS_CONSTRAINTS[arch]],
            target_settings = [_ON],
            toolchain = "@apple_support//toolchain",
        )
        for arch in APPLE_PLATFORMS_CONSTRAINTS.keys()
        for cpu in ["arm64", "x86_64"]
    ], key = lambda e: e.name)

def _clang_cl():
    return sorted([
        _entry(
            name = "clang_cl_%s_%s_to_%s_%s" % (exec_os, exec_cpu, target_os, target_cpu),
            exec_compatible_with = ["@platforms//cpu:" + exec_cpu, "@platforms//os:" + exec_os],
            target_compatible_with = ["@platforms//cpu:" + target_cpu, "@platforms//os:" + target_os],
            target_settings = [_ON, _STAGE0, str(Label("@llvm//platforms/config:windows_%s_msvc" % target_cpu))],
            toolchain = "//cc/private/toolchain/clang_cl:%s_%s_cc_toolchain" % (exec_os, exec_cpu),
        )
        for (exec_os, exec_cpu) in MSVC_TARGET_STAGE0_SUPPORTED_EXECS
        for (target_os, target_cpu) in WINDOWS_TARGETS
    ], key = lambda e: e.name)

def _llvm_pair(prefix, exec_os, exec_cpu, target_os, target_cpu, stage_setting, cc_toolchain, msvc_execs):
    name = "llvm_{}{}_{}_to_{}_{}".format(prefix, exec_os, exec_cpu, target_os, target_cpu)
    settings = [_ON, stage_setting]
    if target_os == "windows":
        settings.append(str(Label("@llvm//platforms/config:windows_{}_mingw_compatible".format(target_cpu))))
    constraints = {
        "exec_compatible_with": ["@platforms//cpu:" + exec_cpu, "@platforms//os:" + exec_os],
        "target_compatible_with": ["@platforms//cpu:" + target_cpu, "@platforms//os:" + target_os],
    }
    entries = [_entry(name = name, target_settings = settings, toolchain = cc_toolchain, **constraints)]
    if target_os == "windows" and (exec_os, exec_cpu) in msvc_execs:
        entries.append(_entry(
            name = name + "_msvc",
            target_settings = [_ON, stage_setting, str(Label("@llvm//platforms/config:windows_{}_msvc".format(target_cpu)))],
            toolchain = cc_toolchain,
            **constraints
        ))
    return entries

def _llvm():
    entries = []
    for (exec_os, exec_cpu) in SUPPORTED_EXECS:
        for (target_os, target_cpu) in SUPPORTED_TARGETS:
            entries.extend(_llvm_pair(
                "",
                exec_os,
                exec_cpu,
                target_os,
                target_cpu,
                _STAGE0,
                "@llvm//toolchain:{}_{}_cc_toolchain".format(exec_os, exec_cpu),
                MSVC_TARGET_STAGE0_SUPPORTED_EXECS,
            ))
    for (exec_os, exec_cpu) in [(os, cpu) for (os, cpu) in SUPPORTED_TARGETS if os != "none"]:
        for (stage, stage_setting) in _BOOTSTRAP_STAGES:
            for (target_os, target_cpu) in SUPPORTED_TARGETS:
                entries.extend(_llvm_pair(
                    stage + "_",
                    exec_os,
                    exec_cpu,
                    target_os,
                    target_cpu,
                    stage_setting,
                    "@llvm//toolchain:{}_{}_{}_cc_toolchain".format(stage, exec_os, exec_cpu),
                    MSVC_TARGET_BOOTSTRAP_SUPPORTED_EXECS,
                ))
    return sorted(entries, key = lambda e: e.name)

def cc_hub_entries():
    """Returns the C and C++ toolchains in the order they must be registered."""
    return _gcc() + _msvc() + _fuzzer() + _apple() + _clang_cl() + _llvm()

def cc_hub_names():
    """Returns the hub target names; sorting them yields the registration order."""
    entries = cc_hub_entries()
    width = len(str(len(entries)))
    return [("0" * width + str(i))[-width:] + "_" + e.name for i, e in enumerate(entries)]

def cc_hub():
    """Returns the BUILD file of the dx_cc_toolchains hub."""
    return "\n".join([
        toolchain_target(
            name = name,
            exec_compatible_with = e.exec_compatible_with,
            target_compatible_with = e.target_compatible_with,
            target_settings = e.target_settings,
            toolchain = e.toolchain,
            toolchain_type = CC_TOOLCHAIN_TYPE,
            visibility = ["//visibility:public"],
        )
        for name, e in zip(cc_hub_names(), cc_hub_entries())
    ])
