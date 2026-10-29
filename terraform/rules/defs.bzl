"""Terraform source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "TerraformModuleInfo", "TerraformProviderInfo")

def collect_providers(label, own, deps):
    """Merges providers and fails when one source appears at two versions.

    Args:
      label: Label used in error messages.
      own: list of TerraformProviderInfo declared directly.
      deps: list of TerraformModuleInfo of dependencies.

    Returns:
      A TerraformModuleInfo.
    """
    transitive = depset(own, transitive = [d.transitive_providers for d in deps])
    seen = {}
    for p in transitive.to_list():
        if seen.get(p.source, p.version) != p.version:
            fail("%s: provider %s is required at versions %s and %s" % (label, p.source, seen[p.source], p.version))
        seen[p.source] = p.version
    return TerraformModuleInfo(transitive_providers = transitive)

def _terraform_module_impl(ctx):
    info = collect_providers(
        str(ctx.label),
        [p[TerraformProviderInfo] for p in ctx.attr.providers],
        [d[TerraformModuleInfo] for d in ctx.attr.deps if TerraformModuleInfo in d],
    )
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("terraform", ["tf", "tfvars", "hcl"])], str(ctx.label)),
        info,
    ]

terraform_module = rule(
    implementation = _terraform_module_impl,
    doc = "Owns the Terraform files of one module for dx format and lint.",
    attrs = {
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Terraform modules this module calls."),
        "providers": attr.label_list(
            cfg = "exec",
            providers = [TerraformProviderInfo],
            doc = "Providers of the module, from the dx_terraform extension hub.",
        ),
        "srcs": attr.label_list(allow_files = [".tf", ".tfvars", ".tftest.hcl", ".tfmock.hcl"], doc = "Terraform files of this module."),
    },
)
