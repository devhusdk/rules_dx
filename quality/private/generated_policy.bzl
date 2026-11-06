"""The policy macro generated .dx/config/BUILD.bazel files call."""

load("//quality:native_config.bzl", "tier_active_dirs")
load("//quality:policy.bzl", "workspace_policy")

_PROFILES = ["standard", "strict"]

def dx_generated_policy(name, profile, families, configs = [], bases = {}):
    """Declares a workspace policy whose unbound tier-active tools run on their tier base.

    Args:
        name: name of the policy target.
        profile: tier profile of the workspace, `standard` or `strict`. Tools shared by several
            families, such as typos and prettier, always use it.
        families: quality_family targets.
        configs: native config targets the repo binds; they win over a base.
        bases: tool ID to a base config target, replacing the tier base of `profile` for that
            tool. The generator sets it for a tool whose families use another profile.
    """
    if profile not in _PROFILES:
        fail("dx_generated_policy (%s): profile must be one of %s, got %r" % (name, ", ".join(_PROFILES), profile))
    tier_bases = {
        tool: str(Label("//quality/tiers/%s:%s" % (tier_dir, profile)))
        for tool, tier_dir in tier_active_dirs().items()
    }
    for tool, label in bases.items():
        if tool not in tier_bases:
            fail("dx_generated_policy (%s): bases names tool '%s', which is not tier-active" % (name, tool))
        tier_bases[tool] = label
    workspace_policy(
        name = name,
        bases = [tier_bases[tool] for tool in sorted(tier_bases.keys())],
        configs = configs,
        families = families,
    )
