"""Test that every test of the calling package is in exactly one named suite."""

load("@bazel_skylib//rules:native_binary.bzl", "native_test")
load("@bazel_skylib//rules:write_file.bzl", "write_file")

def _label(rule):
    return "//%s:%s" % (native.package_name(), rule)

def _normalize(label):
    if label.startswith(":"):
        return _label(label[1:])
    return label

def suite_membership_test(name, views = []):
    """Declares a test over every rule declared before it in the package."""
    rules = native.existing_rules()
    suites = {n: r for n, r in rules.items() if r["kind"] == "test_suite"}
    owners = {}
    for suite, rule in suites.items():
        for member in rule.get("tests", []):
            member = _normalize(member)
            if member.split(":")[-1] in suites:
                continue
            owners.setdefault(member, []).append(suite)

    problems = []
    for rule_name, rule in rules.items():
        if rule_name == "all":
            problems.append("target named all")
        if not rule["kind"].endswith("_test") or rule["kind"] == "test_suite":
            continue
        label = _label(rule_name)
        everywhere = owners.get(label, [])
        found = [suite for suite in everywhere if suite not in views] or everywhere[:1]
        if not everywhere:
            problems.append("in no suite: " + label)
        elif len(found) > 1:
            problems.append("in several suites: %s -> %s" % (label, " ".join(sorted(found))))

    write_file(
        name = name + "_problems",
        out = name + "_problems.txt",
        content = sorted(problems) + [""],
    )
    native_test(
        name = name,
        src = "//core/quality:suite_membership_check",
        args = ["$(rootpath :%s_problems)" % name],
        data = [":%s_problems" % name],
        testonly = True,
    )
