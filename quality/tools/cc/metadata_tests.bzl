"""C and C++ tool acquisition inventory pins."""

load("//cc/tests/fixtures/cppcheck:pins.bzl", "CPPCHECK_VERSION")
load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load(":repos.bzl", "CC_TOOLS", "CC_TOOL_REPOS")

_VERSIONS = {
    "cc_cppcheck": CPPCHECK_VERSION,
}

def _cc_checks():
    checks = []
    derived = sorted(CC_TOOLS.keys())
    checks.append(expect_equal("cc tool repos match metadata", CC_TOOL_REPOS, derived))
    checks.append(expect_equal("cc tool repos sorted", CC_TOOL_REPOS, sorted(CC_TOOL_REPOS)))
    checks.append(expect_equal("cc versions covered", sorted(_VERSIONS.keys()), derived))
    for repo in derived:
        spec = CC_TOOLS[repo]
        checks.append(expect_true(repo + " cc prefix", repo.startswith("cc_")))
        checks.append(expect_equal(repo + " kind", spec["kind"], "archive"))
        checks.append(expect_equal(repo + " sha256 length", len(spec["sha256"]), 64))
        checks.append(expect_true(repo + " urls present", len(spec["urls"]) > 0))
        checks.append(expect_true(repo + " build file", "build_file" in spec))
        checks.append(expect_true(repo + " strip prefix", "strip_prefix" in spec))
        for url in spec["urls"]:
            checks.append(expect_true(repo + " https url", url.startswith("https://")))
            checks.append(expect_true(repo + " url version pinned", _VERSIONS[repo] in url))
        checks.append(expect_true(repo + " strip prefix version pinned", _VERSIONS[repo] in spec["strip_prefix"]))
    return checks

def cc_metadata_tests(name):
    """Declare the C and C++ acquisition inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _cc_checks(),
    )
