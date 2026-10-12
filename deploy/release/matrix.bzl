"""Full release matrix for dx standalone binaries."""

RELEASE_MATRIX = [
    ("dx-linux-x86_64", "linux", "x86_64", "qualified-seed-built-here", "Seed host; built + verified in publish dry-run"),
    ("dx-linux-arm64", "linux", "arm64", "qualified-host-evidence", "Required"),
    ("dx-macos-arm64", "macos", "arm64", "qualified-host-evidence", "Required"),
    ("dx-windows-x86_64", "windows", "x86_64", "qualified-host-evidence", "Required"),
    ("dx-windows-arm64", "windows", "arm64", "unqualified-missing-native-evidence", "Required"),
]

def release_matrix_names():
    """Returns the ordered release artifact names."""
    return [entry[0] for entry in RELEASE_MATRIX]

def release_matrix_status(name):
    """Returns the qualification status for one matrix cell."""
    for entry in RELEASE_MATRIX:
        if entry[0] == name:
            return entry[3]
    return ""

def release_matrix_error(name):
    """Validates one matrix cell name."""
    if release_matrix_status(name) == "":
        return ("release_matrix: unknown artifact '" + str(name) +
                "': want one of " + ", ".join(release_matrix_names()))
    return ""

def release_matrix_unqualified():
    """Returns the names of cells still awaiting qualification."""
    return [entry[0] for entry in RELEASE_MATRIX if not entry[3].startswith("qualified-")]
