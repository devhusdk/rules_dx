XCODE_LINEAGE = "16."

SDK_FLOORS = {
    "iphoneos": "17.0",
    "iphonesimulator": "17.0",
    "macosx": "14.0",
}

_DIGITS = "0123456789"

def _numbers(version):
    numbers = []
    for part in version.split("."):
        if part == "":
            return []
        for char in part.elems():
            if char not in _DIGITS:
                return []
        numbers.append(int(part))
    return numbers

def provisioning_version_below(version, minimum):
    """Returns whether one dotted version sorts below another."""
    left = _numbers(version)
    right = _numbers(minimum)
    if len(left) == 0 or len(right) == 0:
        return False
    width = max(len(left), len(right))
    for index in range(width):
        first = left[index] if index < len(left) else 0
        second = right[index] if index < len(right) else 0
        if first != second:
            return first < second
    return False
