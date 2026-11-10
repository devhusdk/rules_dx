"""The Python API reference shard action shared by the rule and the docs aspect."""

load("//core:rlocation.bzl", "rlocation_path")
load("//tools/docs/ir:shard.bzl", "repo_relative", "srcs_manifest_line")


def _import_relative(ctx, file, imports):
    """Returns the path of one file relative to its longest matching import root."""
    path = rlocation_path(ctx, file)
    for root in imports:
        if path.startswith(root + "/"):
            return path[len(root) + 1:]
    return repo_relative(file.short_path)

def python_package_name_ok(package):
    """Returns whether the string can name a top-level Python package."""
    return bool(package) and package.replace("_", "a").isalnum() and not package[0].isdigit()

def python_shard_action(ctx, info, package, docstyle, stem, driver_tool, extract_tool):
    """Runs griffe and the extractor for one Python library.

    Args:
        ctx: The rule or aspect context.
        info: The PyInfo of the documented library.
        package: Top-level package to document.
        docstyle: Docstring convention griffe parses.
        stem: Output name prefix, relative to the target's package.
        driver_tool: The griffe dump executable.
        extract_tool: The DocIr extractor executable.

    Returns:
        A struct with the DocIr `shard`, the `json` and `manifest` files, or None when the library has no source for the package.
    """
    if not python_package_name_ok(package):
        fail("python_doc_shard " + str(ctx.label) + ": '" + package + "' is not a Python package name; set package")
    imports = sorted(info.imports.to_list(), key = lambda root: -len(root))
    originals = {}
    links = []
    lines = []
    for src in info.transitive_sources.to_list():
        if src.extension not in ("py", "pyi"):
            continue
        rel = _import_relative(ctx, src, imports)
        if not (rel.startswith(package + "/") or rel in (package + ".py", package + ".pyi")):
            continue
        if rel in originals:
            fail("python_doc_shard " + str(ctx.label) + ": " + originals[rel].path + " and " + src.path + " both stage as " + rel)
        originals[rel] = src
        link = ctx.actions.declare_file(stem + ".pyroot/" + rel)
        ctx.actions.symlink(output = link, target_file = src)
        links.append(link)
        lines.append(srcs_manifest_line(link.path, repo_relative(src.short_path), not src.is_source))
    if not links:
        return None
    prefix = ctx.bin_dir.path + "/" + (ctx.label.package + "/" if ctx.label.package else "")
    root = prefix + stem + ".pyroot"
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    json = ctx.actions.declare_file(stem + ".griffe.json")
    driver = ctx.actions.args()
    driver.add("--root", root)
    driver.add("--package", package)
    driver.add("--docstyle", docstyle)
    driver.add("--out", json)
    ctx.actions.run(
        executable = driver_tool,
        arguments = [driver],
        inputs = links,
        outputs = [json],
        mnemonic = "DxDocsGriffe",
        progress_message = "Reading Python API docs for %{label}",
        use_default_shell_env = False,
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", json)
    extract.add("--package", package)
    extract.add("--root", root)
    extract.add("--srcs", manifest)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [json, manifest],
        outputs = [shard],
        mnemonic = "DxDocsPythonExtract",
        progress_message = "Extracting Python API docs for %{label}",
        use_default_shell_env = False,
    )
    return struct(json = json, manifest = manifest, package = package, shard = shard)
