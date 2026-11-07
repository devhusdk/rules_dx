"""The policy macro generated .dx/config/BUILD.bazel files call."""

load("//quality/private:native_config_impl.bzl", "tier_base_dirs")
load("//quality:policy.bzl", "CAPABILITIES", "quality_family", "workspace_policy")

_TIERS = ["standard", "strict"]
_SPEC_KEYS = CAPABILITIES + ["disabled", "tier"]

def generated_families_error(name, families):
    """Returns an error for a family spec with an unknown key, or ""."""
    for family_id in sorted(families.keys()):
        for key in sorted(families[family_id].keys()):
            if key not in _SPEC_KEYS:
                return "dx_generated_policy (%s): family '%s' has unknown key '%s'; use one of %s" % (
                    name,
                    family_id,
                    key,
                    ", ".join(_SPEC_KEYS),
                )
    return ""

def dx_generated_policy(name, tier, families = None, defaults = True, disabled = [], configs = [], bases = {}, visibility = None):
    """Declares a workspace policy whose unbound tier-active tools run on their tier base.

    Args:
        name: name of the policy target.
        tier: tier of the workspace, `standard` or `strict`. Tools shared by several
            families, such as typos and prettier, always use it.
        families: dict from family ID to its spec: capability tool lists (`lint`, `typecheck`,
            `format`, `audit`), `disabled` capabilities and a `tier`. Each key replaces only
            that family of the curated defaults.
        defaults: whether the curated default families apply. False uses only `families`.
        disabled: curated family IDs to drop; needs `defaults`.
        configs: native config targets the repo binds; they win over a base.
        bases: tool ID to a base config target, replacing the tier base of `tier` for that
            tool. The generator sets it for a tool whose families use another tier.
        visibility: visibility of the policy target.
    """
    if tier not in _TIERS:
        fail("dx_generated_policy (%s): tier must be one of %s, got %r" % (name, ", ".join(_TIERS), tier))
    tier_bases = {
        tool: str(Label("//quality/tiers/%s:%s" % (tier_dir, tier)))
        for tool, tier_dir in tier_base_dirs().items()
    }
    for tool, label in bases.items():
        if tool not in tier_bases:
            fail("dx_generated_policy (%s): bases names tool '%s', which has no tier base" % (name, tool))
        tier_bases[tool] = label

    if not defaults and families == None:
        fail("dx_generated_policy (%s): defaults = False needs families" % name)
    if not defaults and disabled:
        fail("dx_generated_policy (%s): disabled needs defaults = True" % name)
    extends = Label("//quality:default_workspace_policy") if defaults else None
    family_labels = []
    if families != None:
        error = generated_families_error(name, families)
        if error:
            fail(error)
        for family_id in sorted(families.keys()):
            family_name = "%s_%s" % (name, family_id)
            quality_family(
                name = family_name,
                family_id = family_id,
                **families[family_id]
            )
            family_labels.append(":" + family_name)

    workspace_policy(
        name = name,
        bases = [tier_bases[tool] for tool in sorted(tier_bases.keys())],
        configs = configs,
        disabled = disabled,
        extends = extends,
        families = family_labels,
        visibility = visibility,
    )
