"""Windows MSVC boundary selection behind explicit platform transitions."""

load("//cc/toolchains:bindings.bzl", "CC_TOOLCHAIN_TYPE")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")

MSVC_BOUNDARY = ":interop_check"

EXPECTED_MSVC_OBSERVATIONS = """subject //cc/tests/fixtures/prebuilt_interop:msvc_windows_arm64
field boundary=cc/tests/fixtures/prebuilt_interop/interop_check.exe
field cc.compiler=clang-cl
field cc.target=
field platform=windows_arm64
field target.cpu=arm64
aspect_field aspect_seen=True
aspect_field field_count=5
aspect_field has_subject=True
aspect_field subject_label=//cc/tests/fixtures/prebuilt_interop:msvc_windows_arm64
aspect_field transitive_count=0
subject //cc/tests/fixtures/prebuilt_interop:msvc_windows_x86_64
field boundary=cc/tests/fixtures/prebuilt_interop/interop_check.exe
field cc.compiler=clang-cl
field cc.target=x86_64-pc-windows-msvc
field platform=windows_x86_64
field target.cpu=x86_64
aspect_field aspect_seen=True
aspect_field field_count=5
aspect_field has_subject=True
aspect_field subject_label=//cc/tests/fixtures/prebuilt_interop:msvc_windows_x86_64
aspect_field transitive_count=0"""

_MSVC_CASES = [
    ("windows_x86_64", "@llvm//platforms:windows_x86_64_msvc", "x86_64"),
    ("windows_arm64", "@llvm//platforms:windows_aarch64_msvc", "arm64"),
]

def _msvc_transition_impl(_settings, attr):
    """Moves one dependency onto the named Windows MSVC platform."""
    return {"//command_line_option:platforms": str(attr.platform_target)}

_msvc_transition = transition(
    implementation = _msvc_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _msvc_subject_impl(ctx):
    """Records the resolved MSVC toolchain triple for one boundary binary."""
    info = ctx.toolchains[CC_TOOLCHAIN_TYPE]
    cc_info = info.cc if hasattr(info, "cc") else info
    if cc_info.compiler != "clang-cl":
        fail("msvc abi: " + ctx.attr.platform + " selected compiler '" + cc_info.compiler + "', want 'clang-cl'")
    if not ctx.target_platform_has_constraint(ctx.attr._os_windows[platform_common.ConstraintValueInfo]):
        fail("msvc abi: " + ctx.attr.platform + " missed the windows os constraint")
    cpu = ctx.attr._cpu_arm64 if ctx.attr.expected_cpu == "arm64" else ctx.attr._cpu_x86_64
    cpu_constraint = "@platforms//cpu:" + ctx.attr.expected_cpu
    if not ctx.target_platform_has_constraint(cpu[platform_common.ConstraintValueInfo]):
        fail("msvc abi: " + ctx.attr.platform + " missed the cpu constraint '" + cpu_constraint + "'")
    files = ctx.attr.boundary[DefaultInfo].files.to_list()
    if len(files) == 0:
        fail("msvc abi: " + ctx.attr.platform + " produced no boundary binary")
    return [
        DefaultInfo(files = depset(files)),
        DxSubjectInfo(fields = {
            "boundary": ",".join(sorted([f.short_path for f in files])),
            "cc.compiler": cc_info.compiler,
            "cc.target": cc_info.target_gnu_system_name,
            "platform": ctx.attr.platform,
            "target.cpu": ctx.attr.expected_cpu,
        }),
    ]

msvc_subject = rule(
    doc = "Resolves one boundary binary with the MSVC toolchain.",
    implementation = _msvc_subject_impl,
    toolchains = [CC_TOOLCHAIN_TYPE],
    attrs = {
        "boundary": attr.label(doc = "Boundary binary resolved with the MSVC toolchain.", mandatory = True),
        "expected_cpu": attr.string(doc = "CPU constraint the MSVC platform must carry.", mandatory = True),
        "platform": attr.string(doc = "Canonical Windows platform key under test.", mandatory = True),
        "_cpu_arm64": attr.label(doc = "Constraint marking ARM64 CPUs.", default = "@platforms//cpu:arm64"),
        "_cpu_x86_64": attr.label(doc = "Constraint marking x86_64 CPUs.", default = "@platforms//cpu:x86_64"),
        "_os_windows": attr.label(doc = "Constraint marking the Windows OS.", default = "@platforms//os:windows"),
    },
)

def _msvc_probe_impl(ctx):
    """Forwards one transitioned MSVC subject to the analysis test."""
    subject = ctx.attr.subject
    if type(subject) == "list":
        subject = subject[0]
    return [subject[DxSubjectInfo]]

msvc_probe = rule(
    doc = "Exposes one transitioned MSVC subject without rebuilding the boundary.",
    implementation = _msvc_probe_impl,
    attrs = {
        "platform_target": attr.label(doc = "Upstream MSVC platform the subject transition selects.", mandatory = True),
        "subject": attr.label(cfg = _msvc_transition, doc = "Subject resolved on the MSVC platform.", mandatory = True),
    },
)

def msvc_boundary_tests(name):
    """Declares manual MSVC boundary selection tests for both Windows CPUs."""
    subjects = []
    for key, platform_label, cpu in _MSVC_CASES:
        subject = name + "_subject_" + key
        msvc_subject(
            name = subject,
            boundary = MSVC_BOUNDARY,
            expected_cpu = cpu,
            platform = key,
            tags = ["manual"],
        )
        probe = name + "_" + key
        msvc_probe(
            name = probe,
            platform_target = platform_label,
            subject = ":" + subject,
            tags = ["manual"],
        )
        subjects.append(":" + probe)
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = subjects,
        expected_observations = EXPECTED_MSVC_OBSERVATIONS,
        tags = ["manual"],
    )
