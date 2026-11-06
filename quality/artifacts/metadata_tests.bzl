"""Load tests pinning standalone-artifact metadata."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_match", "expect_true", "starlark_test")
load(":extension.bzl", "TOOL_ARTIFACTS", "TOOL_SOURCES")
load(":hub.bzl", "artifact_metadata_errors")
load(":repos.bzl", "DX_TOOL_HUB", "DX_TOOL_REPOS")

FROZEN_SCHEMA_KEYS = [
    "abi_floor",
    "archive",
    "checksum_source",
    "cpu",
    "delivery_class",
    "executable",
    "executable_sha256",
    "interpreter",
    "licenses",
    "linkage",
    "needed_shared_libraries",
    "os",
    "runtime_files",
    "sha256",
    "size",
    "tool",
    "upstream_version",
    "url",
]

_HEX = "0123456789abcdef"

def _is_sha256(value):
    if type(value) != "string" or len(value) != 64:
        return False
    for char in value.elems():
        if char not in _HEX:
            return False
    return True

def _artifact_checks(artifact):
    label = "%s %s_%s" % (artifact.get("tool", "?"), artifact.get("os", "?"), artifact.get("cpu", "?"))
    size = artifact.get("size", 0)
    return [
        expect_equal(label + " schema keys", sorted(artifact.keys()), FROZEN_SCHEMA_KEYS),
        expect_equal(label + " delivery class", artifact.get("delivery_class"), "standalone"),
        expect_true(label + " sha256 is hex", _is_sha256(artifact.get("sha256"))),
        expect_true(label + " executable sha256 is hex", _is_sha256(artifact.get("executable_sha256"))),
        expect_true(label + " size is positive", type(size) == "int" and size > 0),
        expect_match(label + " url carries version", str(artifact.get("url", "")), str(artifact.get("upstream_version", "?"))),
        expect_equal(label + " exe suffix", str(artifact.get("executable", "")).endswith(".exe"), artifact.get("os") == "windows"),
    ]

def _download_checks(label, version, urls, sha256):
    checks = [
        expect_true(label + " sha256 is hex", _is_sha256(sha256)),
        expect_true(label + " urls present", len(urls) > 0),
    ]
    for url in urls:
        checks.append(expect_true(label + " https url", url.startswith("https://")))
        checks.append(expect_true(label + " url carries version", version in url))
    return checks

def _source_checks(source):
    label = source["repo"]
    if source["kind"] == "duniverse":
        checks = []
        for package in source["packages"]:
            checks += _download_checks(label + " " + package["name"], package["version"], [package["url"]], package["sha256"])
        return checks
    return _download_checks(label, source["version"], source["urls"], source["sha256"])

def metadata_tests(name):
    """Declare the standalone-artifact metadata test."""
    checks = [expect_equal("released artifacts carry valid metadata", artifact_metadata_errors(TOOL_ARTIFACTS), [])]
    for artifact in TOOL_ARTIFACTS:
        checks += _artifact_checks(artifact)

    first = {}
    for source in TOOL_SOURCES:
        repo = source["repo"]
        if source["kind"] == "duniverse":
            lock = [source["packages"], source["patches"]]
            if repo in first:
                checks.append(expect_equal(repo + " is one lock for all its tools", first[repo], lock))
                continue
            first[repo] = lock
        checks += _source_checks(source)
    tools = [source["tool"] for source in TOOL_SOURCES]
    checks.append(expect_equal("source tools are unique", len(tools), len(depset(tools).to_list())))

    derived_repos = sorted(["dx_%s_%s_%s" % (artifact["tool"], artifact["os"], artifact["cpu"]) for artifact in TOOL_ARTIFACTS])
    checks.append(expect_equal("dx tool repos match metadata", DX_TOOL_REPOS, derived_repos))
    checks.append(expect_equal("dx tool repos sorted", DX_TOOL_REPOS, sorted(DX_TOOL_REPOS)))
    checks.append(expect_equal("dx tool hub", DX_TOOL_HUB, "dx_tools"))
    starlark_test(
        name = name,
        mode = "load",
        checks = checks,
    )
