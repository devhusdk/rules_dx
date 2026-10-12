"""Prebuilt interop pins."""

WINDOWS_CRT_START = "/MD"
WINDOWS_STL = "Microsoft STL"
WINDOWS_RUNTIME_LIBS = ["UCRT", "VCRuntime"]
WINDOWS_REDIST_NOTE = "Align Rust CRT mode, iterator-debug settings, system libraries and redistributable deployment"

WINDOWS_COMBOS = [
    "clang-cl + Microsoft STL + /MD + lld-link + static .lib",
    "clang-cl + Microsoft STL + /MD + lld-link + import .lib + DLL",
    "cl.exe compat + Microsoft STL + /MD + link + static .lib (diagnosis only, never a second default)",
]

WINDOWS_COMPAT_LIMITS = "https://learn.microsoft.com/en-us/cpp/porting/binary-compat-2015-2017?view=msvc-170"
WINDOWS_CLANG_COMPAT = "https://clang.llvm.org/docs/MSVCCompatibility.html"
WINDOWS_REJECTED_OBJECTS = ["/GL", "/LTCG", "arbitrary CRT combinations", "vendor libraries"]

WINDOWS_MSVC_PLATFORM = "@llvm//platforms:windows_x86_64_msvc"
WINDOWS_MSVC_TARGET = "x86_64-pc-windows-msvc"
WINDOWS_MSVC_DRIVER = "clang-cl with -fuse-ld=lld"
WINDOWS_MSVC_SETUP = "--repo_env=BAZEL_MSVC_RUNTIME_VISUAL_STUDIO_EULA=1 --repo_env=BAZEL_WINDOWS_SDK_EULA=1"
WINDOWS_MSVC_EXECUTION = "native Windows AMD64 runs the cross-built boundary binary with exit 0"

LINUX_DEFAULT_CXX_LIB = "libc++"
LINUX_ALT_CXX_LIB = "explicit dynamic libstdc++ (Linux glibc only)"
LINUX_ALT_SCOPE = "Upstream supports it only on Linux glibc"
LINUX_GCC_ABI_NOTE = "Patched Rust runtime selection and actual GCC ABI/library fixtures must pass"

LINK_MODEL = "ordinary object linking without cross-language LTO"
ABI_PROOF = [
    "shared-runtime deployment",
    "exceptions across the boundary",
    "RTTI across the boundary",
    "allocation ownership (new pairs with delete on the owning side)",
    "ABI boundaries",
]

CXX_IDENTITY_VERSION = "1.0.200"
CXXBRIDGE_CMD_LABEL = "@crates//:cxxbridge-cmd"
MIXED_NOTE = "mixed Rust/C/C++ host-to-target plus target execution qualify separately"

REJECTED_ALTERNATIVES = [
    "single-combo proof",
    "compiler-target availability as proof",
    "LLVM ancestry as proof",
]

PREBUILT_INTEROP_FIXTURE_LIB = "//cc/tests/fixtures/prebuilt_interop:interop"
PREBUILT_INTEROP_FIXTURE_TEST = "//cc/tests/fixtures/prebuilt_interop:interop_test"
PREBUILT_INTEROP_BOUNDARY_BIN = "//cc/tests/fixtures/prebuilt_interop:interop_check"
PREBUILT_INTEROP_MIXED_CXX = "//rust/tests/fixtures/cxx_identity:bridge"
PREBUILT_INTEROP_MIXED_RUST = "//rust/tests/fixtures/cxx_identity:cxx_identity"
PREBUILT_INTEROP_STABLE_CC = "//cc/tests/fixtures/hello:hello"
PREBUILT_INTEROP_STABLE_RUST = "//rust/tests/fixtures/hello:hello"
PREBUILT_INTEROP_FIXTURE_CORPUS = "//cc/tests/fixtures/prebuilt_interop:corpus_starlark"
