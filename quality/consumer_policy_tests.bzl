"""Analysis proofs for the consumer-selected workspace policy."""

load(
    "//quality:real_aspects.bzl",
    "real_format_aspect",
    "real_js_format_aspect",
    "real_js_lint_aspect",
    "real_jvm_format_aspect",
    "real_jvm_lint_aspect",
    "real_lint_aspect",
    "real_python_lint_aspect",
    "real_rust_format_aspect",
    "real_rust_lint_aspect",
    "real_shell_lint_aspect",
)

def _policy_transition_impl(_settings, attr):
    """Selects the workspace policy named by another attribute of the same rule."""
    return {"//config:workspace": str(attr.policy_target)}

_policy_transition = transition(
    implementation = _policy_transition_impl,
    inputs = [],
    outputs = ["//config:workspace"],
)

_ASPECTS = [
    real_format_aspect,
    real_js_format_aspect,
    real_js_lint_aspect,
    real_jvm_format_aspect,
    real_jvm_lint_aspect,
    real_lint_aspect,
    real_python_lint_aspect,
    real_rust_format_aspect,
    real_rust_lint_aspect,
    real_shell_lint_aspect,
]

def _dx_result_names(target):
    """Returns the sorted real-aspect result names published for one target."""
    if OutputGroupInfo not in target:
        return []
    groups = target[OutputGroupInfo]
    if "dx_results" not in groups:
        return []
    return sorted([f.basename for f in groups["dx_results"].to_list()])

def _consumer_policy_impl(ctx):
    dep = ctx.attr.subject
    subject = dep[0] if type(dep) == "list" else dep
    results = _dx_result_names(subject)
    if results != ctx.attr.expected_dx_results:
        fail("consumer policy " + str(ctx.attr.policy_target) + " publishes " +
             str(results) + " real result(s) for " + str(subject.label) +
             ", want " + str(ctx.attr.expected_dx_results))
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        output = out,
        content = "#!/bin/sh\nexit 0\n",
        is_executable = True,
    )
    return [DefaultInfo(executable = out)]

_consumer_policy_test = rule(
    implementation = _consumer_policy_impl,
    test = True,
    attrs = {
        "expected_dx_results": attr.string_list(
            mandatory = True,
        ),
        "policy_target": attr.label(
            mandatory = True,
        ),
        "subject": attr.label(
            aspects = _ASPECTS,
            cfg = _policy_transition,
            mandatory = True,
        ),
    },
)

def consumer_policy_tests(name, cases, **kwargs):
    """Instantiates one proof per consumer policy selection."""
    kwargs.setdefault("size", "small")
    for case in cases:
        _consumer_policy_test(
            name = name + "_" + case["case"] + "_test",
            expected_dx_results = case["expected_dx_results"],
            policy_target = case["policy"],
            subject = case["subject"],
            **kwargs
        )
