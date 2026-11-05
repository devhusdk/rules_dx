"""Authoritative C++ toolchain binding and compiler identity."""

CC_TOOLCHAIN_TYPE = "@bazel_tools//tools/cpp:toolchain_type"

def cc_toolchain_info(ctx):
    """Return the resolved C++ toolchain for the target platform."""
    info = ctx.toolchains[CC_TOOLCHAIN_TYPE]
    if info == None:
        return None
    return info.cc

def cc_toolchain_toolchains():
    """Toolchain types a probe rule must declare for cc_toolchain_info."""
    return [CC_TOOLCHAIN_TYPE]

CC_COMPILERS = {
    "gcc": "gcc",
    "llvm": "clang",
    "msvc": "msvc-cl",
}

_FAMILY = {
    "clang": "clang",
    "clang-cl": "clang-cl",
    "gcc": "gcc",
    "mingw-gcc": "gcc",
    "msvc-cl": "msvc",
}
_DRIVER = {
    "clang": "gnu",
    "clang-cl": "msvc",
    "gcc": "gnu",
    "msvc": "msvc",
}
_COVERAGE = {
    "clang": "llvm",
    "clang-cl": "llvm",
    "gcc": "gcov",
}
_SYNTAX_ONLY = {
    "gnu": "-fsyntax-only",
    "msvc": "/Zs",
}

def cc_compiler_identity(compiler, target):
    """Return family, driver, ABI and capabilities for a compiler string."""
    family = _FAMILY.get(compiler, "unknown")
    driver = _DRIVER.get(family, "unknown")
    if driver == "msvc" or target.endswith("-msvc"):
        abi = "msvc"
    elif family == "unknown":
        abi = "unknown"
    else:
        abi = "itanium"
    return struct(
        family = family,
        driver = driver,
        abi = abi,
        coverage = _COVERAGE.get(family, ""),
        syntax_only = _SYNTAX_ONLY.get(driver, ""),
        target = target,
    )

def cc_toolchain_identity(ctx):
    """Return the compiler identity of the resolved C++ toolchain."""
    cc_info = cc_toolchain_info(ctx)
    if cc_info == None:
        return cc_compiler_identity("", "")
    return cc_compiler_identity(cc_info.compiler, cc_info.target_gnu_system_name)
