"""cmake_library ownership proofs."""

load("//core/quality:library_tests.bzl", "owns_class_test")

owns_cmake_class_test = owns_class_test("cmake", ["CMakeLists.txt", "helpers.cmake"])
