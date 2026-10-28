"""CloudFormation template ownership for dx lint."""

load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources")

def _cloudformation_template_impl(ctx):
    direct_sources = {}
    if len(ctx.files.srcs) > 0:
        direct_sources["cloudformation"] = depset(ctx.files.srcs)
    check_direct_sources(direct_sources, str(ctx.label))
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]

cloudformation_template = rule(
    implementation = _cloudformation_template_impl,
    doc = "Owns the CloudFormation templates of one package for dx lint.",
    attrs = {
        "srcs": attr.label_list(
            allow_files = [".yaml", ".yml", ".json", ".template"],
            doc = "CloudFormation template files of this package; each needs a top-level Resources section.",
        ),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Other template libraries this package depends on."),
    },
)
