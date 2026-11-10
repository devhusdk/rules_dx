"""Test that every test of the calling package is in exactly one named suite."""

load("@bazel_skylib//rules:write_file.bzl", "write_file")

def _label(rule):
    return "//%s:%s" % (native.package_name(), rule)

def _normalize(label):
    if label.startswith(":"):
        return _label(label[1:])
    return label

def suite_membership_test(name, sh_test, views = []):
    """Declares a test over every rule declared before it in the package.

    Args:
      name: Name of the test.
      sh_test: The sh_test rule, loaded by the calling BUILD file.
      views: Suites that slice tests already owned by another suite. They do not count towards the one-suite rule.
    """
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
    manual = []
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
        elif "manual" in rule.get("tags", []):
            manual.append("%s %s" % (label, " ".join([_label(suite) for suite in everywhere])))

    write_file(
        name = name + "_problems",
        out = name + "_problems.txt",
        content = sorted(problems) + [""],
    )
    write_file(
        name = name + "_manual",
        out = name + "_manual.txt",
        content = sorted(manual) + [""],
    )
    sh_test(
        name = name,
        srcs = ["//core/quality:suite_membership_test.sh"],
        args = [
            "$(location :%s_problems)" % name,
            "$(location :%s_manual)" % name,
            "$(location //:CONTRIBUTING.md)",
        ],
        data = [
            ":%s_problems" % name,
            ":%s_manual" % name,
            "//:CONTRIBUTING.md",
        ],
    )
