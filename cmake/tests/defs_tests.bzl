"""cmake_library ownership proofs."""

load("//core:owns_class_test.bzl", "owns_class_test")

owns_cmake_class_test = owns_class_test("cmake", ["CMakeLists.txt", "helpers.cmake"])
