"""Table-driven adapter_test.sh cases for the adb install, start and log runners."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")

def adapter_case(name, mode, tool, apk = None, extra = [], adb_exit = None):
    """Returns one adapter_test.sh row: the mode, the runner under test and what it gets."""
    return struct(name = name, mode = mode, tool = tool, apk = apk, extra = extra, adb_exit = adb_exit)

def adapter_tests(cases):
    """Declares one small adapter_test.sh test per row against fake_adb.sh."""
    for case in cases:
        files = [case.tool, ":fake_adb.sh"] + ([case.apk] if case.apk else [])
        sh_test(
            name = case.name,
            size = "small",
            srcs = ["adapter_test.sh"],
            args = [case.mode] + ["$(location %s)" % f for f in files] + case.extra,
            data = files,
            env = {"ADB_EXIT": case.adb_exit} if case.adb_exit else {},
        )
