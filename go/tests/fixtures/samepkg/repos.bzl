"""An external Go repository whose package path matches the samepkg fixture."""

_BUILD_BAZEL = '''load("@rules_dx//go/rules:defs.bzl", "go_library")

go_library(
    name = "helper",
    srcs = ["helper.go"],
    importpath = "example.test/samepkg/helper",
    visibility = ["//visibility:public"],
)
'''

_HELPER_GO = """package helper

func Name() string {
	return "helper"
}
"""

def _go_samepkg_repo_impl(ctx):
    """Writes a Go library at the same package path as the samepkg fixture."""
    ctx.file("go/tests/fixtures/samepkg/BUILD.bazel", _BUILD_BAZEL)
    ctx.file("go/tests/fixtures/samepkg/helper.go", _HELPER_GO)
    ctx.file("BUILD.bazel", "")
    return ctx.repo_metadata(reproducible = True)

go_samepkg_repo = repository_rule(
    implementation = _go_samepkg_repo_impl,
)
