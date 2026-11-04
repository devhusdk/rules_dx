"""Tests for the @dx_gazelle language list and hub BUILD text."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "starlark_test")
load(":hub.bzl", "DISPATCH", "hub_languages", "render_hub_build")

def _names(labels):
    return [label.name for label in labels]

def _checks():
    python = Label("//gazelle/python")
    astro = Label("//gazelle/astro")
    return [
        expect_equal("a python-only input renders python and dispatch", _names(hub_languages([python])), ["python", "dispatch"]),
        expect_equal("duplicates collapse", _names(hub_languages([python, python, astro, python])), ["astro", "python", "dispatch"]),
        expect_equal("order is by name", _names(hub_languages([python, astro])), ["astro", "python", "dispatch"]),
        expect_equal("dispatch stays last when listed", _names(hub_languages([DISPATCH, astro])), ["astro", "dispatch"]),
        expect_contains("the build lists each language", render_hub_build([python]), '"%s",\n        "%s",\n    ],' % (python, DISPATCH)),
        expect_contains("the build names the binary", render_hub_build([python]), 'name = "gazelle"'),
    ]

def dx_gazelle_hub_tests(name):
    """Declares the @dx_gazelle hub test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
