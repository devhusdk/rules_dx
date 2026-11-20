"""Tests for the gazelle launcher descriptor of //dx:generate."""

load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")
load(":generate.bzl", "GENERATE_EXCLUDES", "gazelle_launch_descriptor")

visibility("private")

def _checks():
    return [
        expect_equal(
            "a config and a mode",
            gazelle_launch_descriptor("dx_gazelle/gazelle", "cfg/WORKSPACE", "go_sdk/ROOT", "diff", GENERATE_EXCLUDES),
            "\n".join([
                "program=dx_gazelle/gazelle",
                "verb=update",
                "verb=fix",
                "arg=-bzlmod",
                "arg=-repo_config",
                "path=cfg/WORKSPACE",
                "default_arg=-mode",
                "default_arg=diff",
                "default_arg=-exclude=.dx",
                "default_arg=-exclude=**/node_modules",
                "env_dir=GOROOT\tgo_sdk/ROOT",
                "env=GOTOOLCHAIN\tlocal",
            ]) + "\n",
        ),
        expect_equal(
            "no config and no mode",
            gazelle_launch_descriptor("dx_gazelle/gazelle", None, "go_sdk/ROOT", "", ["-x"]),
            "\n".join([
                "program=dx_gazelle/gazelle",
                "verb=update",
                "verb=fix",
                "default_arg=-x",
                "env_dir=GOROOT\tgo_sdk/ROOT",
                "env=GOTOOLCHAIN\tlocal",
            ]) + "\n",
        ),
        expect_equal(
            "a config and no mode",
            gazelle_launch_descriptor("g", "c", "r", None, []),
            "program=g\nverb=update\nverb=fix\narg=-bzlmod\narg=-repo_config\npath=c\nenv_dir=GOROOT\tr\nenv=GOTOOLCHAIN\tlocal\n",
        ),
        expect_equal(
            "a mode and no config",
            gazelle_launch_descriptor("g", None, "r", "fix", []),
            "program=g\nverb=update\nverb=fix\ndefault_arg=-mode\ndefault_arg=fix\nenv_dir=GOROOT\tr\nenv=GOTOOLCHAIN\tlocal\n",
        ),
    ]

def dx_generate_tests(name):
    """Declares the //dx:generate descriptor test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
