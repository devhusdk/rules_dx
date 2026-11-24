"""Analysis tests that the Dart web binaries own their sources for quality."""

load("@rules_dx//core:owns_class_test.bzl", "owns_class_test")

web_owns_main_test = owns_class_test("dart", ["main.dart"])
