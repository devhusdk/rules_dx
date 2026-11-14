"""The policy macro generated .dx/config/BUILD.bazel files call."""

load("//core/quality/private:native_config_impl.bzl", "tier_base_dirs")
load("//core/quality:policy.bzl", "CAPABILITIES", "quality_family", "workspace_policy")

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
    """Declares a workspace policy whose unbound tier-active tools run on their tier base."""
    if tier not in _TIERS:
        fail("dx_generated_policy (%s): tier must be one of %s, got %r" % (name, ", ".join(_TIERS), tier))
    tier_bases = {
        tool: str(Label("//core/quality/tiers/%s:%s" % (tier_dir, tier)))
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
    extends = Label("//config:default_workspace_policy") if defaults else None
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
