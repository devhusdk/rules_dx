"""Checks that the resolved C++ toolchain can build cgo."""

load("//cc/private/toolchains:bindings.bzl", "cc_compiler_identity")

def cgo_toolchain_error(compiler, target):
    """Returns why cgo cannot use this compiler, or "" when it can."""
    identity = cc_compiler_identity(compiler, target)
    if identity.driver == "msvc":
        reason = "cgo needs a GCC/Clang-style driver"
    elif identity.abi == "msvc":
        reason = "Go cgo on Windows needs the MinGW (GNU) ABI"
    else:
        return ""
    return (
        reason + ", but the C++ toolchain is " + compiler + " for " + target + ". " +
        "rules_go would build the package without cgo. " +
        "Select a gcc or clang toolchain with the GNU ABI, or set cgo = False."
    )
