"""Ansible source ownership for dx quality."""

load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources")
load(":layout.bzl", "ansible_layout_class")

def _ansible_library_impl(ctx):
    forced = {f.short_path: True for f in ctx.files.playbooks}
    for f in ctx.files.playbooks:
        if f not in ctx.files.srcs:
            fail(str(ctx.label) + ": playbooks entry " + f.short_path + " is not in srcs")
    ansible = []
    yaml = []
    for f in ctx.files.srcs:
        if ansible_layout_class(f.short_path, f.short_path in forced) == "ansible":
            ansible.append(f)
        else:
            yaml.append(f)
    direct = {}
    if ansible:
        direct["ansible"] = depset(ansible)
    if yaml:
        direct["yaml"] = depset(yaml)
    check_direct_sources(direct, str(ctx.label))
    return [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        QualitySourcesInfo(direct_sources = direct),
    ]

ansible_library = rule(
    implementation = _ansible_library_impl,
    doc = "Owns the Ansible and plain YAML files of one package for dx quality, split by Ansible layout.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".yaml", ".yml"], doc = "YAML files of this package."),
        "playbooks": attr.label_list(allow_files = [".yaml", ".yml"], doc = "Subset of srcs forced to the ansible class."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "Libraries this package depends on."),
    },
)
