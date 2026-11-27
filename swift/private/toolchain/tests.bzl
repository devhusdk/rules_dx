"""Tests for the Swift toolchains."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@dx_pins_data//:pins.bzl", "PINS")
load("@rules_swift//swift:providers.bzl", "SwiftToolchainInfo")
load("//core/private/testing:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//core:toolchain.bzl", "hub_checks")
load(":extensions.bzl", "swift_hub", "swift_toolchain_build")
load(":repos.bzl", "SWIFT_LINUX_CPUS", "swift_libs_build", "swift_linux_stub_build")

def _link_flags(cc_info):
    return [
        flag
        for linker_input in cc_info.linking_context.linker_inputs.to_list()
        for flag in linker_input.user_link_flags
    ]

def _cross_runtime_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[SwiftToolchainInfo]
    target_runtime = ctx.attr.target_root + "/usr/lib/swift"
    exec_runtime = ctx.attr.exec_root + "/usr/lib/swift"
    for name in ("dynamic_runtime_cc_info", "static_runtime_cc_info"):
        flags = _link_flags(getattr(info, name))
        asserts.true(env, [flag for flag in flags if target_runtime in flag], name + " has no target runtime flag: " + str(flags))
        asserts.equals(env, [], [flag for flag in flags if exec_runtime in flag], name + " links the runtime of the exec cpu")
    asserts.equals(env, ctx.attr.exec_root + "/usr", info.root_dir)
    return analysistest.end(env)

_cross_runtime_test = analysistest.make(
    _cross_runtime_impl,
    attrs = {
        "exec_root": attr.string(mandatory = True, doc = "The root of the Swift archive the tools run from."),
        "target_root": attr.string(mandatory = True, doc = "The root of the Swift archive of the target cpu."),
    },
)

def swift_cross_runtime_test(name, exec_cpu, target_cpu, **kwargs):
    """Asserts a cross toolchain links the target runtime and runs exec tools."""
    _cross_runtime_test(
        name = name,
        size = "small",
        exec_root = Label("@dx_swift_linux_%s//:files" % exec_cpu).workspace_root,
        target_root = Label("@dx_swift_linux_%s//:files" % target_cpu).workspace_root,
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + target_cpu],
        target_under_test = "@dx_swift_toolchain_impl//:swift_toolchain_%s_%s" % (exec_cpu, target_cpu),
        **kwargs
    )

def _tool_inputs_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [action for action in analysistest.target_actions(env) if action.mnemonic == "SwiftCompile"]
    asserts.true(env, len(actions) > 0, "no SwiftCompile action")
    for action in actions:
        inputs = [file.path for file in action.inputs.to_list()]
        for suffix in ctx.attr.expected:
            asserts.true(env, [path for path in inputs if path.endswith(ctx.attr.root + "/" + suffix)], "SwiftCompile does not take " + suffix + " as an input")
    return analysistest.end(env)

_tool_inputs_test = analysistest.make(
    _tool_inputs_impl,
    attrs = {
        "expected": attr.string_list(mandatory = True, doc = "Paths under the Swift archive root that every SwiftCompile action takes as inputs."),
        "root": attr.string(mandatory = True, doc = "The root of the Swift archive the tools run from."),
    },
)

def swift_tool_inputs_test(name, cpu, target_under_test, size = "small", **kwargs):
    """Asserts SwiftCompile takes the driver and tool libraries as inputs."""
    _tool_inputs_test(
        name = name,
        size = size,
        expected = [
            "usr/bin/swift-driver",
            "usr/bin/swift-frontend",
            "usr/lib/swift/host/compiler/lib_CompilerSwiftSyntax.so",
            "usr/lib/swift/linux/libswiftCore.so",
            "usr/lib/swift/linux/libuuid.so.1",
            "usr/lib/swift/linux/libsqlite3.so.0",
        ],
        root = Label("@dx_swift_linux_%s//:files" % cpu).workspace_root,
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + cpu],
        target_under_test = target_under_test,
        **kwargs
    )

_WINDOWS_CELLS = ["windows_arm64", "windows_x86_64"]

def _windows_pin_checks():
    pins = PINS["dx_swift_format_macos_arm64"]
    checks = []
    for name in ["seven_zip_windows", "swift_windows"]:
        cells = pins[name]["cells"]
        checks.extend([
            expect_equal(name + " cells", sorted(cells.keys()), _WINDOWS_CELLS),
            expect_true(name + " one url per cell", all([len(cell["urls"]) == 1 for cell in cells.values()])),
            expect_equal(name + " distinct digests", len({cell["sha256"]: True for cell in cells.values()}), len(cells)),
        ])
    checks.append(expect_true("installers carry the pinned version", all([
        "swift-{}-RELEASE".format(pins["swift_windows"]["version"]) in url
        for cell in pins["swift_windows"]["cells"].values()
        for url in cell["urls"]
    ])))
    return checks

def swift_windows_extract_test(name, size):
    """Asserts the Windows installer and extractor are pinned."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _windows_pin_checks(),
        file_checks = {
            "//swift/private/toolchain:extract_windows.ps1": "\n".join([
                "& $SevenZip x -y -bd",
                "Copy-Item -LiteralPath $dll.FullName -Destination (Join-Path $assemblyDir \"$assembly.dll\")",
                "Fail \"conflicting file $destination from $($msi.Name)\"",
                "\"windows_arm64\" = 0xAA64",
                "\"windows_x86_64\" = 0x8664",
            ]),
            "//swift/private/toolchain:repos.bzl": "\n".join([
                "repository_ctx.path(_SEVEN_ZIP + \"/full/7z.exe\")",
                "\"-Cell\",",
                "environment = _NO_HOST_TOOLS,",
                "\"PATH\": None,",
                "\"Path\": None,",
                "\"SystemRoot\": None,",
            ]),
            "//swift/private/toolchain:windows.bzl": "env = {\"PATH\": runtime, \"Path\": runtime},",
        },
    )

_STUB_FILEGROUPS = ["files", "swift_tool_inputs", "linux_dynamic_runtime", "linux_static_runtime"]

def _stub_paths(cpu):
    return [".swift-version"] + [
        "usr/bin/" + tool
        for tool in ["ld", "llvm-cov", "llvm-objdump", "llvm-profdata", "swift", "swift-autolink-extract", "swift-symbolgraph-extract", "swiftc"]
    ] + [
        "usr/lib/clang/21/lib/linux/libclang_rt.{}-{}.a".format(runtime, cpu)
        for runtime in ["asan", "asan_static", "fuzzer", "profile"]
    ]

def _linux_stub_checks():
    stub_libs = swift_libs_build(None)
    libs = swift_libs_build(Label("@dx_swift_sysroot_linux_x86_64//:files"))
    checks = [
        expect_true("libs stub has no sysroot repo", "sysroot_linux" not in stub_libs),
        expect_true("libs stub declares sysroot", "name = \"sysroot\"" in stub_libs),
        expect_true("libs declares sysroot", "name = \"sysroot\"" in libs),
        expect_true("libs sysroot points at the sysroot repo", "dx_swift_sysroot_linux_x86_64//:files" in libs),
    ]
    for cpu in SWIFT_LINUX_CPUS:
        stub = swift_linux_stub_build(cpu)
        checks.append(expect_true("stub {} has no sysroot repo".format(cpu), "sysroot_linux" not in stub))
        checks.append(expect_equal("stub {} missing paths".format(cpu), [p for p in _stub_paths(cpu) if repr(p) not in stub], []))
        checks.append(expect_equal("stub {} missing filegroups".format(cpu), [n for n in _STUB_FILEGROUPS if repr(n) not in stub], []))
    return checks

def swift_linux_repo_test(name, size):
    """Asserts the Linux repository declares every target the toolchain consumes."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = [expect_true("linux cpus", len(SWIFT_LINUX_CPUS) == 2)] + _linux_stub_checks(),
        file_checks = {
            "//swift/private/toolchain:repos.bzl": "\n".join([
                "name = \"files\",",
                "name = \"swift_tool_inputs\",",
                "name = \"linux_dynamic_runtime\",",
                "name = \"linux_static_runtime\",",
                "name = \"host_libs\",",
                "name = \"runtime_libs\",",
                "\"usr/bin/swift-driver\",",
                "\"usr/bin/swift-frontend\",",
                "\"libz.so.1\",",
            ]),
            "//swift/private/toolchain:toolchain.bzl": "\n".join([
                "//:swift_tool_inputs",
                "//:linux_dynamic_runtime",
                "//:linux_static_runtime",
                "//:host_libs",
                "//:sysroot",
            ]),
        },
    )

_HUB_ORDER = [
    "toolchain_aarch64_aarch64",
    "toolchain_aarch64_x86_64",
    "toolchain_windows_aarch64",
    "toolchain_windows_x86_64",
    "toolchain_x86_64_aarch64",
    "toolchain_x86_64_x86_64",
]

def _hub_checks():
    hub = swift_hub()
    toolchains = swift_toolchain_build()
    names = [line.split("\"")[1] for line in hub.splitlines() if line.startswith("    name = ")]
    declared = [line.split("\"")[1] for line in toolchains.splitlines() if line.startswith("    name = ")]
    return hub_checks(hub, "@rules_swift//toolchains:toolchain_type", len(_HUB_ORDER)) + [
        expect_equal("hub toolchain order", names, _HUB_ORDER),
        expect_true("hub names the generated swift toolchains", all([
            "@dx_swift_toolchain_impl//:swift_" + name + "\"" in hub
            for name in _HUB_ORDER
        ])),
        expect_equal("toolchain repository declares every hub toolchain", sorted(declared), sorted(["swift_" + name for name in _HUB_ORDER])),
    ]

def swift_toolchain_hub_test(name, size):
    """Asserts the hub declares every toolchain in order."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _hub_checks(),
    )
