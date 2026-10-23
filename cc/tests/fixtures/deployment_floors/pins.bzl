"""Deployment plus execution floors pins."""

GLIBC_FLOOR = "2.28"
GLIBC_SYMBOL_FLOOR = "Upstream glibc 2.28 symbol floor"
GLIBC_CXX_LIB = "libc++"
GLIBC_LINK_MODEL = "ordinary dynamic glibc linkage"
GLIBC_NOTE = "Application glibc implementations come from deployment systems, not the link stubs"

MACOS_DEPLOYMENT_DEFAULT = "14.0"
MACOS_SDK_LIBCXX = "SDK libc++ headers"
MACOS_RUNTIME_LIB = "system dynamic libc++"
MACOS_SDK_NOTE = "SDK version is not deployment floor"
APPLE_SDK_IDENTITY = "MacOSX26.5"
APPLE_SDK_SOURCE = "hermetic-llvm v0.8.23 pinned extraction"
APPLE_SDK_HERMETIC_LLVM_COMMIT = "5a9583802e7dd3ac64225147189b3d508ff8b7d5"

WINDOWS_CRT_START = "/MD"
WINDOWS_STL = "Microsoft STL"
WINDOWS_RUNTIME_LIBS = ["UCRT", "VCRuntime"]
WINDOWS_SUPPORT_MSVC = "14.50.35717"
WINDOWS_SUPPORT_REDIST = "14.50.35710"
WINDOWS_SUPPORT_SDK_PACKAGE = "10.0.26100.7705"
WINDOWS_REJECTED_CRT = ["/MT", "debug CRT"]
WINDOWS_CRT_NOTE = "/MT plus debug CRT are not assumed interchangeable"

OLDEST_TARGET_NOTE = "oldest-target and current-host fixtures run separately"
OLDEST_TARGET_CASES = [
    "glibc 2.28 oldest-target symbols",
    "macOS 14.0 oldest-target deployment",
    "Windows /MD retail oldest-target CRT",
]
CURRENT_HOST_CASES = [
    "seed-host plus CI-runner execution floors",
    "matching native target execution, cross-building alone is insufficient",
]

LOADER_DEPS = [
    "compiler loader dependencies",
    "clangd loader dependencies",
    "bindgen loader dependencies",
]
LOADER_NOTE = "inspect compiler, clangd and bindgen loader dependencies"
LOADER_MINIMAL_ARCHIVE_NOTE = "the minimal compiler archive alone is not proof that a loadable libclang exists"

APPLE_FRAMEWORK_NOTE = "Check Apple's extracted SDK framework subset"
APPLE_GATES = [
    "oldest-OS execution",
    "framework completeness",
    "licensing",
]
APPLE_BEST_EFFORT_NOTE = "best-effort gaps never block required-host release"

REJECTED_ALTERNATIVES = [
    "unpinned floors",
    "merged oldest-target plus current-host proof",
    "SDK version as deployment floor",
]

DEPLOYMENT_FLOORS_FIXTURE_CORPUS = "//cc/tests/fixtures/deployment_floors:corpus_starlark"
DEPLOYMENT_FLOORS_LIVE_HELLO = "//cc/tests/fixtures/hello:hello"
DEPLOYMENT_FLOORS_LINUX_CORPUS = "//cc/tests/fixtures/linux_corpus:corpus"
