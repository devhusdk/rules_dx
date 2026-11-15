"""@llvm cc toolchains registered behind //config:toolchains_on."""

load("@llvm//platforms:common.bzl", "MSVC_TARGET_BOOTSTRAP_SUPPORTED_EXECS", "MSVC_TARGET_STAGE0_SUPPORTED_EXECS", "SUPPORTED_EXECS", "SUPPORTED_TARGETS")

_BOOTSTRAP_STAGES = [
    ("stage3", "@llvm//toolchain:bootstrap_stage3_lto_and_fdo_applied"),
    ("stage2", "@llvm//toolchain:bootstrap_stage2_lto_and_fdo_instrumented"),
    ("stage1", "@llvm//toolchain:bootstrap_stage1_from_source"),
]

def _declare(name, exec_os, exec_cpu, target_os, target_cpu, target_settings, cc_toolchain):
    native.toolchain(
        name = name,
        exec_compatible_with = [
            "@platforms//cpu:" + exec_cpu,
            "@platforms//os:" + exec_os,
        ],
        target_compatible_with = [
            "@platforms//cpu:" + target_cpu,
            "@platforms//os:" + target_os,
        ],
        target_settings = [Label("//config:toolchains_on")] + target_settings,
        toolchain = cc_toolchain,
        toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
    )

def _declare_pair(prefix, exec_os, exec_cpu, target_os, target_cpu, stage_setting, cc_toolchain, msvc_execs):
    name = "{}{}_{}_to_{}_{}".format(prefix, exec_os, exec_cpu, target_os, target_cpu)
    settings = [stage_setting]
    if target_os == "windows":
        settings.append("@llvm//platforms/config:windows_{}_mingw_compatible".format(target_cpu))
    _declare(name, exec_os, exec_cpu, target_os, target_cpu, settings, cc_toolchain)
    if target_os == "windows" and (exec_os, exec_cpu) in msvc_execs:
        _declare(
            name + "_msvc",
            exec_os,
            exec_cpu,
            target_os,
            target_cpu,
            [stage_setting, "@llvm//platforms/config:windows_{}_msvc".format(target_cpu)],
            cc_toolchain,
        )

def llvm_toolchains():
    """Declares every toolchain @llvm//toolchain declares, each also requiring //config:toolchains_on."""
    for (exec_os, exec_cpu) in SUPPORTED_EXECS:
        for (target_os, target_cpu) in SUPPORTED_TARGETS:
            _declare_pair(
                "",
                exec_os,
                exec_cpu,
                target_os,
                target_cpu,
                "@llvm//toolchain:bootstrap_stage0_prebuilt_seed",
                "@llvm//toolchain:{}_{}_cc_toolchain".format(exec_os, exec_cpu),
                MSVC_TARGET_STAGE0_SUPPORTED_EXECS,
            )
    bootstrap_execs = [(os, cpu) for (os, cpu) in SUPPORTED_TARGETS if os != "none"]
    for (exec_os, exec_cpu) in bootstrap_execs:
        for (stage, stage_setting) in _BOOTSTRAP_STAGES:
            for (target_os, target_cpu) in SUPPORTED_TARGETS:
                _declare_pair(
                    stage + "_",
                    exec_os,
                    exec_cpu,
                    target_os,
                    target_cpu,
                    stage_setting,
                    "@llvm//toolchain:{}_{}_{}_cc_toolchain".format(stage, exec_os, exec_cpu),
                    MSVC_TARGET_BOOTSTRAP_SUPPORTED_EXECS,
                )
