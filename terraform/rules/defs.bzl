"""Terraform source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")

def _terraform_module_impl(ctx):
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("terraform", ["tf", "tfvars", "hcl"])], str(ctx.label)),
    ]

terraform_module = rule(
    implementation = _terraform_module_impl,
    doc = "Owns the Terraform files of one module for dx format and lint.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".tf", ".tfvars", ".tftest.hcl", ".tfmock.hcl"], doc = "Terraform files of this module."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Terraform modules this module calls."),
    },
)
