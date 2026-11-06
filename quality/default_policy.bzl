"""Default workspace policy generated from CURATED_DEFAULTS."""

load(":curated_defaults.bzl", "CURATED_DEFAULTS")
load(":policy.bzl", "CAPABILITIES", "quality_family", "workspace_policy")

visibility(["//quality/..."])

def default_workspace_policy(name):
    """Declares one default family per curated entry and the policy that lists them.

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
        families = labels,
    )
