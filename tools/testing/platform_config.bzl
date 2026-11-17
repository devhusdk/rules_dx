"""Analysis test that a rule's platform transition changes the configuration of its dependency."""

load("//core:defs.bzl", "DxSubjectInfo", "starlark_test")

_ConstraintsSeenInfo = provider(
    doc = "Constraint values the target platform of one configuration holds.",
    fields = {"names": "Names of the held constraint values, in attr order."},
)

def _constraint_probe_impl(ctx):
    names = [
        c.label.name
        for c in ctx.attr.constraints
        if ctx.target_platform_has_constraint(c[platform_common.ConstraintValueInfo])
    ]
    return [_ConstraintsSeenInfo(names = names)]

constraint_probe = rule(
    implementation = _constraint_probe_impl,
    doc = "Reports which of its constraint values the target platform holds.",
    attrs = {
        "constraints": attr.label_list(
            doc = "Constraint values to look for.",
            providers = [platform_common.ConstraintValueInfo],
        ),
    },
)

def _platform_config_subject_impl(ctx):
    seen = sorted([",".join(probe[_ConstraintsSeenInfo].names) for probe in ctx.attr.probe])
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {"configurations": ";".join(seen)}),
    ]

def platform_config_subject_rule(cfg, attrs = {}):
    """Returns a subject rule that builds its probe through cfg and reports each configuration."""
    return rule(
        implementation = _platform_config_subject_impl,
        doc = "Reports the constraint values of each configuration cfg builds the probe in.",
        attrs = attrs | {
            "probe": attr.label(cfg = cfg, mandatory = True, providers = [_ConstraintsSeenInfo]),
        },
    )

def platform_config_test(name, subject_rule, constraints, subjects):
    """Builds one subject per entry of subjects and checks the configurations each reports.

    subjects maps a subject suffix to (kwargs for subject_rule, expected configurations).
    """
    constraint_probe(
        name = name + "_probe",
        constraints = constraints,
        testonly = True,
    )
    blocks = []
    for suffix in sorted(subjects.keys()):
        kwargs, expected = subjects[suffix]
        subject = name + "_" + suffix
        subject_rule(
            name = subject,
            probe = ":" + name + "_probe",
            testonly = True,
            **kwargs
        )
        label = "//" + native.package_name() + ":" + subject
        blocks.append("""subject {l}
field configurations={e}
aspect_field aspect_seen=True
aspect_field field_count=1
aspect_field has_subject=True
aspect_field subject_label={l}
aspect_field transitive_count=0""".format(l = label, e = expected))
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":" + name + "_" + suffix for suffix in sorted(subjects.keys())],
        expected_observations = "\n".join(blocks),
    )
