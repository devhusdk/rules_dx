"""Load-time checks for php, reported by a diff test."""

load("@bazel_skylib//rules:diff_test.bzl", "diff_test")
load("@bazel_skylib//rules:write_file.bzl", "write_file")

def expect_equal(name, actual, expected):
    """Returns a check that passes when actual equals expected."""
    return struct(name = name, passed = actual == expected, detail = "%r != %r" % (actual, expected))

def expect_true(name, actual):
    """Returns a check that passes when actual is True."""
    return struct(name = name, passed = actual == True, detail = "%r is not True" % (actual,))

def expect_contains(name, haystack, needle):
    """Returns a check that passes when haystack contains needle."""
    return struct(name = name, passed = needle in haystack, detail = "%r not in %r" % (needle, haystack))

def php_checks_test(name, checks, **kwargs):
    """Declares a test that passes when every check passed."""
    if not checks:
        fail("php_checks_test: checks must be non-empty")
    write_file(
        name = name + "_expected",
        out = name + "_expected.txt",
        content = ["ok %s" % check.name for check in checks] + [""],
    )
    write_file(
        name = name + "_actual",
        out = name + "_actual.txt",
        content = [("ok %s" % check.name) if check.passed else ("FAIL %s: %s" % (check.name, check.detail)) for check in checks] + [""],
    )
    diff_test(
        name = name,
        file1 = ":" + name + "_expected.txt",
        file2 = ":" + name + "_actual.txt",
        **kwargs
    )
