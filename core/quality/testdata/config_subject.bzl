"""Applies one TEST_CONFIGS entry to the subjects of a quality test."""

load(":test_configs.bzl", "TEST_CONFIGS", "TEST_CONFIG_SETTINGS")

def _config_transition_impl(_settings, attr):
    """Writes the settings of the configuration named by the config attribute."""
    return TEST_CONFIGS[attr.config]

config_transition = transition(
    implementation = _config_transition_impl,
    inputs = [],
    outputs = TEST_CONFIG_SETTINGS,
)

def config_attr():
    """Returns the attribute that names one TEST_CONFIGS entry."""
    return attr.string(
        doc = "Name of one TEST_CONFIGS entry.",
        mandatory = True,
        values = sorted(TEST_CONFIGS.keys()),
    )

def _config_results_impl(ctx):
    files = depset(transitive = [subject[OutputGroupInfo].dx_results for subject in ctx.attr.subject])
    if not ctx.attr.allow_empty and not files.to_list():
        fail("%s: no result files" % ctx.label)
    return [DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = files))]

config_results = rule(
    implementation = _config_results_impl,
    doc = "Exposes the real aspect results of a subject under one configuration.",
    attrs = {
        "allow_empty": attr.bool(doc = "Accepts a subject without result files."),
        "config": config_attr(),
        "subject": attr.label(doc = "Subject that carries the results.", mandatory = True, cfg = config_transition),
    },
)

def config_subject_rule(aspects = [], providers = []):
    """Returns a rule that builds its target under one TEST_CONFIGS entry and forwards the listed providers.

    Args:
        aspects: aspects applied to the target under the entry.
        providers: providers of the target, with its aspects, that the subject forwards.

    Returns:
        A rule with the config and target attributes.
    """

    def _impl(ctx):
        target = ctx.attr.target[0]
        forwarded = [target[provider] for provider in providers if provider in target]
        return [DefaultInfo(files = target[DefaultInfo].files)] + forwarded

    return rule(
        implementation = _impl,
        doc = "Builds the target under one TEST_CONFIGS entry.",
        attrs = {
            "config": config_attr(),
            "target": attr.label(doc = "Target built under the entry.", mandatory = True, cfg = config_transition, aspects = aspects),
        },
    )

config_subject = config_subject_rule()
