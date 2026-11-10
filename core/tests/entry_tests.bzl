"""Shared main inference tests."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")

def _checks():
    candidates = dx_main_candidates("x/tool", ".dart", ["bin"])
    return [
        expect_equal("candidates use the name stem then each dir", candidates, ["tool.dart", "bin/tool.dart"]),
        expect_equal("no dirs gives the stem only", dx_main_candidates("tool", ".pl"), ["tool.pl"]),
        expect_equal("explicit main wins", dx_infer_main("k", "tool", "m.dart", ["tool.dart"], candidates), "m.dart"),
        expect_equal("inferred by name", dx_infer_main("k", "tool", None, ["tool.dart", "x.dart"], candidates), "tool.dart"),
        expect_equal("inferred by dir", dx_infer_main("k", "tool", None, ["bin/tool.dart", "x.dart"], candidates), "bin/tool.dart"),
        expect_equal("inferred from the single src", dx_infer_main("k", "tool", None, ["other.dart"], candidates), "other.dart"),
        expect_equal("main is owned first and once", dx_entry_quality_srcs("a.pl", ["b.pl", "a.pl"]), ["a.pl", "b.pl"]),
        expect_equal("external main is skipped", dx_entry_quality_srcs("@gems//:rspec", ["a.rb"]), ["a.rb"]),
        expect_equal("label main is skipped", dx_entry_quality_srcs("//pkg:runner", None), []),
        expect_equal("select main is skipped", dx_entry_quality_srcs(select({"//conditions:default": "a.rb"}), ["b.rb"]), ["b.rb"]),
        expect_equal("extra comes last without repeats", dx_entry_quality_srcs("a.php", ["b.php"], ["b.php", "c.php"]), ["a.php", "b.php", "c.php"]),
        expect_equal("no main and no srcs is empty", dx_entry_quality_srcs(None, None), []),
    ]

def entry_tests(name):
    """Declares the shared main inference test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
