"""Pinned compiler releases for the C and C++ toolchains; the GCC archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

# The clang release shipped by the pinned `llvm` module.
LLVM_CLANG_VERSION = "23.1.2"

_GCC = DATA["dx_cc_gcc"]

GCC_VERSION = _GCC["version"]

# The MSVC toolset, Windows SDK and redist versions that llvm's pinned payloads unpack to.
MSVC_TOOLSET_VERSION = "14.50.35717"

WINDOWS_SDK_INCLUDE_VERSION = "10.0.26100.0"

VC_REDIST_VERSION = "14.50.35710"

# The cl.exe build in the MSVC 14.50.35717 toolset, as the Rich header of its output records it.
MSVC_CL_BUILD = "35728"

CC_COMPILER_VERSIONS = {"gcc": GCC_VERSION, "llvm": LLVM_CLANG_VERSION, "msvc": MSVC_CL_BUILD}

# The glibc of the pinned sysroot.
GCC_GLIBC = "glibc_2.28"

def _archive(name):
    archive = _GCC["archives"][name]
    return {"sha256": archive["sha256"], "url": archive["urls"][0]}

GCC_LINUX_X86_64 = _archive("gcc_linux_x86_64")

GCC_LINUX_X86_64_TO_AARCH64 = _archive("gcc_linux_x86_64_to_aarch64")

GCC_LINUX_AARCH64 = _archive("gcc_linux_aarch64")
