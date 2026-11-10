"""Pinned compiler releases for the C and C++ toolchains."""

# The clang release shipped by the pinned `llvm` module.
LLVM_CLANG_VERSION = "23.1.2"

GCC_VERSION = "16.2.0"

# The MSVC toolset, Windows SDK and redist versions that llvm's pinned payloads unpack to.
MSVC_TOOLSET_VERSION = "14.50.35717"

WINDOWS_SDK_INCLUDE_VERSION = "10.0.26100.0"

VC_REDIST_VERSION = "14.50.35710"

# The cl.exe build in the MSVC 14.50.35717 toolset, as the Rich header of its output records it.
MSVC_CL_BUILD = "35728"

CC_COMPILER_VERSIONS = {"gcc": GCC_VERSION, "llvm": LLVM_CLANG_VERSION, "msvc": MSVC_CL_BUILD}

# The glibc of the pinned sysroot.
GCC_GLIBC = "glibc_2.28"

GCC_LINUX_X86_64 = {
    "url": "https://github.com/f0rmiga/gcc-builds/releases/download/10082026/gcc-toolchain-16.2.0-x86_64.tar.xz",
    "sha256": "54af34c821e59b03ded8f82d3a1104426ec4baaf3b226233e1fd76ad5dcb78cf",
}

GCC_LINUX_X86_64_TO_AARCH64 = {
    "url": "https://github.com/f0rmiga/gcc-builds/releases/download/10082026/gcc-toolchain-16.2.0-aarch64.tar.xz",
    "sha256": "6407b35116f21c59e98ab5ad68ddd8ff5f3e0469723a5d465ec08ed615b11a55",
}

GCC_LINUX_AARCH64 = {
    "url": "https://github.com/f0rmiga/gcc-builds/releases/download/10082026/gcc-toolchain-16.2.0-aarch64-host-aarch64.tar.xz",
    "sha256": "82a482d269cf75334cb969ef93593655f7e726e855043214e38b3672b10f97de",
}
