"""CloudFormation template ownership for dx lint."""

load("//core:wrapper.bzl", "dx_quality_sources")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _cloudformation_library_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("cloudformation", ["yaml", "yml", "json", "template"])], str(ctx.label)),
    ]

cloudformation_library = rule(
    implementation = _cloudformation_library_impl,
    doc = "Owns the CloudFormation templates of one package for dx lint.",
    attrs = {
        "srcs": attr.label_list(
            allow_files = [".yaml", ".yml", ".json", ".template"],
            doc = "CloudFormation template files of this package; each needs a top-level Resources section.",
        ),
        "deps": attr.label_list(providers = [QualitySourcesInfo], doc = "Other template libraries this package depends on."),
    },
)
