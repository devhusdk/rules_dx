"""Error text for the MSVC unsupported-platform guard."""

UNSUPPORTED = "--@rules_dx//cc:compiler=msvc has no MSVC toolchain for this exec and target platform; supported: a Windows x86_64 host targeting Windows x86_64 or arm64, and a Windows arm64 host targeting Windows arm64. Drop the flag to use LLVM."
