"""JVM tool acquisition inventory pins."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//modules:java-scala-kotlin.bzl", "JVM_TOOL_VERSIONS")
load(":repos.bzl", "JVM_TOOLS", "JVM_TOOL_REPOS")

_VERSION_KEY = {
    "jvm_checkstyle": "checkstyle",
    "jvm_google_java_format": "google-java-format",
    "jvm_ktfmt": "ktfmt",
    "jvm_ktlint": "ktlint",
    "jvm_pmd_dist": "pmd",
    "jvm_spotbugs_dist": "spotbugs",
}

def _jvm_checks():
    checks = []
    derived = sorted(JVM_TOOLS.keys())
    checks.append(expect_equal("jvm tool repos match metadata", JVM_TOOL_REPOS, derived))
    checks.append(expect_equal("jvm tool repos sorted", JVM_TOOL_REPOS, sorted(JVM_TOOL_REPOS)))
    checks.append(expect_equal(
        "jvm versions covered",
        sorted(_VERSION_KEY.values() + ["scalafix"]),
        sorted(JVM_TOOL_VERSIONS.keys()),
    ))
    for repo in derived:
        spec = JVM_TOOLS[repo]
        checks.append(expect_true(repo + " jvm prefix", repo.startswith("jvm_")))
        checks.append(expect_true(
            repo + " single artifact",
            "_linux_" not in repo and "_macos_" not in repo and "_windows_" not in repo,
        ))
        checks.append(expect_true(repo + " kind", spec["kind"] in ["file", "archive"]))
        checks.append(expect_equal(repo + " sha256 length", len(spec["sha256"]), 64))
        checks.append(expect_true(repo + " urls present", len(spec["urls"]) > 0))
        for url in spec["urls"]:
            checks.append(expect_true(repo + " https url", url.startswith("https://")))
        if spec["kind"] == "file":
            checks.append(expect_true(repo + " downloaded file", "downloaded_file_path" in spec))
        else:
            checks.append(expect_true(repo + " build file", "build_file" in spec))
            checks.append(expect_true(repo + " strip prefix", "strip_prefix" in spec))
        version = JVM_TOOL_VERSIONS[_VERSION_KEY[repo]]
        haystack = " ".join(spec["urls"]) + " " + spec.get("strip_prefix", "")
        checks.append(expect_true(repo + " version pinned", version in haystack))
    return checks

def jvm_metadata_tests(name):
    """Declare the JVM acquisition inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _jvm_checks(),
    )
