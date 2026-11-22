"""Table rows for the adapter_test.sh cases of the adb install, start and log runners."""

def adapter_case(name, mode, tool, apk = None, extra = [], adb_exit = None):
    """Returns one adapter_test.sh row against fake_adb.sh: its name, args, data and env."""
    files = [tool, ":fake_adb.sh"] + ([apk] if apk else [])
    return struct(
        name = name,
        args = [mode] + ["$(location %s)" % f for f in files] + extra,
        data = files,
        env = {"ADB_EXIT": adb_exit} if adb_exit else {},
    )
