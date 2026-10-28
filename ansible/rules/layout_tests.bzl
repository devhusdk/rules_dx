"""Unit tests for the Ansible layout classifier."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":layout.bzl", "ansible_layout_class", "ansible_layout_kind")
load(":layout_corpus.bzl", "LAYOUT_CORPUS")

def layout_unit_tests(name):
    """Declares the layout classifier corpus test."""
    checks = []
    for row in LAYOUT_CORPUS:
        checks.append(expect_equal(row[0] + " kind", ansible_layout_kind(row[0]), row[1]))
        checks.append(expect_equal(row[0] + " class", ansible_layout_class(row[0], False), row[2]))
    checks.append(expect_equal("forced playbook", ansible_layout_class("site.yml", True), "ansible"))
    checks.append(expect_equal("unforced root file", ansible_layout_class("site.yml", False), "yaml"))
    starlark_test(name = name, mode = "unit", checks = checks)
