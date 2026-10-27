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

_MAVEN_ARTIFACTS_BLOCK = """MAVEN_ARTIFACTS = [
    "junit:junit:4.13.2",
    "org.junit.jupiter:junit-jupiter-api:6.1.3",
    "org.junit.jupiter:junit-jupiter-engine:6.1.3",
    "org.junit.jupiter:junit-jupiter-params:6.1.3",
    "org.junit.platform:junit-platform-commons:6.1.3",
    "org.junit.platform:junit-platform-engine:6.1.3",
    "org.junit.platform:junit-platform-launcher:6.1.3",
    "org.junit.platform:junit-platform-console:6.1.3",
    "org.junit.vintage:junit-vintage-engine:6.1.3",
    "org.scalactic:scalactic_2.13:3.2.20",
    "org.scalatest:scalatest_2.13:3.2.20",
    "org.scalatest:scalatest-compatible:3.2.20",
    "org.scalatest:scalatest-core_2.13:3.2.20",
    "org.scalatest:scalatest-diagrams_2.13:3.2.20",
    "org.scalatest:scalatest-featurespec_2.13:3.2.20",
    "org.scalatest:scalatest-flatspec_2.13:3.2.20",
    "org.scalatest:scalatest-freespec_2.13:3.2.20",
    "org.scalatest:scalatest-funspec_2.13:3.2.20",
    "org.scalatest:scalatest-funsuite_2.13:3.2.20",
    "org.scalatest:scalatest-matchers-core_2.13:3.2.20",
    "org.scalatest:scalatest-mustmatchers_2.13:3.2.20",
    "org.scalatest:scalatest-propspec_2.13:3.2.20",
    "org.scalatest:scalatest-refspec_2.13:3.2.20",
    "org.scalatest:scalatest-shouldmatchers_2.13:3.2.20",
    "org.scalatest:scalatest-wordspec_2.13:3.2.20",
]"""

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
        file_blocks = {
            "//third_party/jvm:pins.bzl": [_MAVEN_ARTIFACTS_BLOCK],
        },
    )
