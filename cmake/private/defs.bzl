"""CMake source ownership for dx quality."""

load("//core/quality:sources.bzl", "named_source_library")

cmake_library = named_source_library(
    "cmake",
    doc = "Owns the CMake files of one package for dx format (gersemi) and dx lint (cmakelint). It does not build CMake projects.",
    srcs_doc = "CMake files of this package: CMakeLists.txt or *.cmake.",
)
