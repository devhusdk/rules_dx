"""The api_compat and api_baseline attributes every library deploy rule shares."""

API_COMPAT_ATTRS = {
    "api_baseline": attr.string(
        doc = "Where the earlier version comes from: registry or git.",
        values = ["registry", "git"],
        default = "registry",
    ),
    "api_compat": attr.string(
        doc = "What a breaking public API change without a major bump does: off, warn or fail.",
        values = ["off", "warn", "fail"],
        default = "fail",
    ),
}

def api_compat_mode(value):
    """Returns the api_compat mode; True and False mean fail and off."""
    if value == True:
        return "fail"
    if value == False:
        return "off"
    return value

def api_compat_error(rule, api_compat, api_baseline):
    """Validates one api_compat and api_baseline pair."""
    if api_compat_mode(api_compat) not in ["off", "warn", "fail"]:
        return rule + ": invalid api_compat '" + str(api_compat) + "': want 'off', 'warn' or 'fail'"
    if api_baseline not in ["registry", "git"]:
        return rule + ": invalid api_baseline '" + str(api_baseline) + "': want 'registry' or 'git'"
    return ""
