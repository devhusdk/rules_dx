"""Default workspace policy generated from CURATED_DEFAULTS."""

load("//core/quality/private:native_config_impl.bzl", "CONFIG_REQUIRED_TOOLS", "tier_base_dirs")
load(":curated_defaults.bzl", "CURATED_DEFAULTS")
load(":policy.bzl", "CAPABILITIES", "quality_family", "workspace_policy")

visibility(["//config/...", "//core/quality/..."])

def _tier_bases(profile):
    dirs = tier_base_dirs()
    return [
        Label("//core/quality/tiers/%s:%s" % (dirs[tool], profile))
        for tool in sorted(dirs.keys())
        if tool not in CONFIG_REQUIRED_TOOLS
    ]

def default_workspace_policy(name):
    """Declares one default family per curated entry and the policy that lists them.

    Every tool with a tier base runs on the base of //config:config_profile unless it
    needs a config only the project can give.

    Args:
        name: name of the workspace policy target.
    """
    labels = []
    for family_id in sorted(CURATED_DEFAULTS.keys()):
        entry = CURATED_DEFAULTS[family_id]
        family_name = "default_" + family_id + "_family"
        selections = {c: entry[c] for c in CAPABILITIES if entry[c]}
        quality_family(
            name = family_name,
            disabled = [c for c in CAPABILITIES if not entry[c]],
            family_id = family_id,
            **selections
        )
        labels.append(":" + family_name)
    workspace_policy(
        name = name,
        bases = select({
            Label("//config:config_profile_strict"): _tier_bases("strict"),
            "//conditions:default": _tier_bases("standard"),
        }),
        families = labels,
    )
