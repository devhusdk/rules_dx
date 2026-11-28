"""Tests for the gazelle launcher descriptor of //dx:generate."""

load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")
load("//core/private/testing:providers.bzl", "DxSubjectInfo")
load(":generate.bzl", "GENERATE_EXCLUDES", "gazelle_launch_descriptor")

visibility("private")

def _checks():
    return [
        expect_equal(
            "a mode",
            gazelle_launch_descriptor("dx_gazelle/gazelle", "diff", GENERATE_EXCLUDES),
            "\n".join([
                "program=dx_gazelle/gazelle",
                "verb=update",
                "verb=fix",
                "arg=-bzlmod",
                "default_arg=-mode",
                "default_arg=diff",
                "default_arg=-exclude=.dx",
                "default_arg=-exclude=**/node_modules",
            ]) + "\n",
        ),
        expect_equal(
            "no mode",
            gazelle_launch_descriptor("g", None, ["-x"]),
            "program=g\nverb=update\nverb=fix\narg=-bzlmod\ndefault_arg=-x\n",
        ),
        expect_equal(
            "an empty mode",
            gazelle_launch_descriptor("g", "", []),
            "program=g\nverb=update\nverb=fix\narg=-bzlmod\n",
        ),
    ]

def _generate_runfiles_subject_impl(ctx):
    files = ctx.attr.run[DefaultInfo].default_runfiles.files.to_list()
    go_deps = [f for f in files if "+go_deps+" in f.owner.repo_name]
    go_sdk = [f for f in files if "go_sdk" in f.owner.repo_name]
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "go_deps_files": str(len(go_deps)),
            "go_sdk_files": str(len(go_sdk)),
        }),
    ]

_generate_runfiles_subject = rule(
    implementation = _generate_runfiles_subject_impl,
    doc = "Reports the go_deps and Go SDK files in the runfiles of a generate target.",
    attrs = {
        "run": attr.label(mandatory = True, doc = "The generate target."),
    },
)

def dx_generate_tests(name, size = "small"):
    """Declares the //dx:generate descriptor and runfiles tests."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
    _generate_runfiles_subject(
        name = name + "_runfiles_subject",
        run = ":generate_check",
        testonly = True,
    )
    starlark_test(
        name = name + "_runfiles",
        size = size,
        mode = "analysis",
        subjects = [":" + name + "_runfiles_subject"],
        expected_observations = "\n".join([
            "subject //dx:" + name + "_runfiles_subject",
            "field go_deps_files=0",
            "field go_sdk_files=0",
            "aspect_field aspect_seen=True",
            "aspect_field field_count=2",
            "aspect_field has_subject=True",
            "aspect_field subject_label=//dx:" + name + "_runfiles_subject",
            "aspect_field transitive_count=0",
        ]),
    )
