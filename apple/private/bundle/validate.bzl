_ID_CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"

_DIGITS = "0123456789"

def bundle_id_errors(kind, bundle_id):
    """Returns one error string when the bundle identifier leaves reverse-DNS."""
    if bundle_id == "":
        return [kind + ": bundle_id must not be empty"]
    parts = bundle_id.split(".")
    if len(parts) < 2:
        return [kind + ": bundle_id '" + bundle_id + "' is not reverse-DNS: want like 'com.example.app'"]
    for part in parts:
        if part == "":
            return [kind + ": bundle_id '" + bundle_id + "' is not reverse-DNS: want like 'com.example.app'"]
        for char in part.elems():
            if char not in _ID_CHARS:
                return [kind + ": bundle_id '" + bundle_id + "' uses '" + char + "': want letters, digits, '-', '_', '.'"]
    return []

def bundle_version_errors(kind, version):
    """Returns one error string when the bundle version leaves dotted digits."""
    if version == "":
        return [kind + ": bundle version must not be empty"]
    for part in version.split("."):
        if part == "":
            return [kind + ": bundle version '" + version + "' is not a dotted version"]
        for char in part.elems():
            if char not in _DIGITS:
                return [kind + ": bundle version '" + version + "' is not a dotted version"]
    return []
