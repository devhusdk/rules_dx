"""dockerfile_library ownership proofs."""

load("//core:owns_class_test.bzl", "owns_class_test")

owns_dockerfile_class_test = owns_class_test("dockerfile", ["app.Dockerfile"])
