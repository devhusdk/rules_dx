_HOST_RESERVED_STEMS = (
    "aux",
    "com1",
    "com2",
    "com3",
    "com4",
    "com5",
    "com6",
    "com7",
    "com8",
    "com9",
    "con",
    "lpt1",
    "lpt2",
    "lpt3",
    "lpt4",
    "lpt5",
    "lpt6",
    "lpt7",
    "lpt8",
    "lpt9",
    "nul",
    "prn",
)

_EXECUTABLE_SUFFIXES = (".bat", ".cmd", ".com", ".exe")

_RESERVED_HOST_NAMES = (".rules_dx_managed",)

def env_name_error(name):
    """Validates one host command name."""
    if name == "":
        return "invalid host name '': must be a non-empty single path component"
    if name == "." or name == "..":
        return "invalid host name '" + name + "': must not be '.' or '..'"
    if "/" in name or "\\" in name:
        return "invalid host name '" + name + "': must not contain '/' or '\\'"
    lower = name.lower()
    if lower in _RESERVED_HOST_NAMES:
        return "invalid host name '" + name + "': is reserved for the provenance marker"
    for suffix in _EXECUTABLE_SUFFIXES:
        if lower.endswith(suffix):
            return "invalid host name '" + name + "': must not carry an executable suffix (found '" + suffix + "')"
    stem = lower.split(".")[0]
    if stem in _HOST_RESERVED_STEMS:
        return "invalid host name '" + name + "': stem '" + stem + "' is reserved on Windows"
    return ""

def env_tool_error(bin_name, aliases):
    """Validates one tool record, whose host names must be unique without regard to case."""
    primary_error = env_name_error(bin_name)
    if primary_error != "":
        return "invalid bin_name: " + primary_error
    seen = {bin_name.lower(): True}
    for alias in aliases:
        alias_error = env_name_error(alias)
        if alias_error != "":
            return "invalid alias '" + alias + "': " + alias_error
        if alias.lower() in seen:
            return "duplicate host name '" + alias + "'"
        seen[alias.lower()] = True
    return ""

def env_collision_error(records):
    """Detects host-name collisions across tool records."""
    owners_by_key = {}
    for record in records:
        # Records are checked by env_tool_error; owners_by_key dedupes per owner.
        for name in [record.bin_name] + list(record.aliases):
            key = name.lower()
            if key not in owners_by_key:
                owners_by_key[key] = {}
            owners_by_key[key][record.owner] = True
    collisions = []
    for key in sorted(owners_by_key.keys()):
        owners = sorted(owners_by_key[key].keys())
        if len(owners) > 1:
            collisions.append(
                "host name '" + key + "' claimed by " + ", ".join(owners),
            )
    if not collisions:
        return ""
    return "host-name collision: " + "; ".join(collisions)
