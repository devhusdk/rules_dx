"""C/C++ foundation pins plus the hermetic toolchain contract."""

load(":versions.bzl", _LLVM_VERSION = "LLVM_VERSION", _RULES_CC_VERSION = "RULES_CC_VERSION")

LLVM_VERSION = _LLVM_VERSION
RULES_CC_VERSION = _RULES_CC_VERSION

# Upstream ships a full cc_toolchain for Linux, macOS, and Windows MinGW and
# MSVC. MinGW is the default because it needs no licence acceptance. The
# MSVC-ABI platform is opt-in and needs the two EULA flags below.
LLVM_TOOLCHAIN_REPO = "@llvm//toolchain:all"

LLVM_WINDOWS_PLATFORM = "@llvm//platforms:windows_x86_64"
LLVM_WINDOWS_ARM64_PLATFORM = "@llvm//platforms:windows_arm64"
LLVM_MACOS_PLATFORM = "@llvm//platforms:macos_arm64"
LLVM_LINUX_PLATFORM = "@llvm//platforms:linux_x86_64"

# The MSVC ABI path needs the Windows SDK and the MSVC runtime, which are
# fetchable under these two flags. Microsoft's compiler and STL are not
# redistributed, and are not needed: clang-cl emits the same ABI.
LLVM_MSDK_EULA_FLAGS = [
    "--repo_env=BAZEL_MSVC_RUNTIME_VISUAL_STUDIO_EULA=1",
    "--repo_env=BAZEL_WINDOWS_SDK_EULA=1",
]

LLVM_WINDOWS_ABI = "windows_arm64_msvc"

LLVM_KNOWN_LIMITS = [
    "The MSVC-ABI platform has no sanitizers, coverage, or FDO.",
    "Module maps and header parsing are unavailable on every platform.",
]
