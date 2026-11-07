"""Renders docs/tiers-standard.md and docs/tiers-strict.md from quality/tiers."""

load("@bazel_lib//lib:write_source_files.bzl", "write_source_files")
load("@bazel_skylib//rules:write_file.bzl", "write_file")
load("//quality:tools.bzl", "TOOLS")

def _native(config):
    if config.files:
        return ", ".join(config.files)
    return ", ".join([("*" + suffix) if suffix.startswith(".") else suffix for suffix in config.suffixes])

def tier_manifest():
    """Returns one tab separated line per tool that has tier files."""
    lines = []
    for tool in sorted(TOOLS, key = lambda tool: tool.id):
        config = tool.config
        if not config.tier_active or not config.ext:
            continue
        base = "quality/tiers/%s" % (config.tier_dir or tool.id)
        lines.append("\t".join([
            tool.id,
            config.tier_source,
            _native(config),
            "%s/standard.%s" % (base, config.ext),
            "%s/strict.%s" % (base, config.ext),
        ]))
    return lines

def tier_docs(name, standard_page, strict_page):
    """Declares the update target and currency test for the two tier pages."""
    write_file(
        name = name + "_manifest",
        out = name + "_manifest.tsv",
        content = tier_manifest() + [""],
    )
    native.genrule(
        name = name + "_pages",
        srcs = [":" + name + "_manifest", "//quality/tiers:all"],
        outs = [name + "_standard.md", name + "_strict.md"],
        cmd = "$(location //tools/docs/tiers:render) $(location :%s_manifest) $(location %s_standard.md) $(location %s_strict.md)" % (name, name, name),
        tools = ["//tools/docs/tiers:render"],
    )
    write_source_files(
        name = name,
        files = {
            standard_page: ":" + name + "_standard.md",
            strict_page: ":" + name + "_strict.md",
        },
    )
