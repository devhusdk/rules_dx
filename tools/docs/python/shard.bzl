"""The Python API reference shard action shared by the rule and the docs aspect."""

load("@bazel_skylib//lib:paths.bzl", "paths")
load("//core:rlocation.bzl", "repo_relative_path")

def _source_row(file):
    """Returns the candidate row of one source for the griffe driver, or None for other files."""
    if file.extension not in ("py", "pyi"):
        return None
    return file.path + "\t" + file.short_path + "\t" + ("0" if file.is_source else "1")

def python_package_name_ok(package):
    """Returns whether the string can name a top-level Python package."""
    return bool(package) and package.replace("_", "a").isalnum() and not package[0].isdigit()

PythonDocsImportsInfo = provider(
    doc = "Repo-relative import roots of one Python library or its upstream.",
    fields = {"roots": "Import roots relative to the repository root; '' is the root itself."},
)

def _python_docs_imports_aspect_impl(_target, ctx):
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if type(upstream) == "list":
        upstream = upstream[0] if upstream else None
    if upstream and PythonDocsImportsInfo in upstream:
        return [upstream[PythonDocsImportsInfo]]
    roots = [paths.normalize(paths.join(ctx.label.package, imp)) for imp in getattr(ctx.rule.attr, "imports", [])]
    return [PythonDocsImportsInfo(roots = ["" if root == "." else root for root in roots])]

python_docs_imports_aspect = aspect(
    implementation = _python_docs_imports_aspect_impl,
    attr_aspects = ["upstream"],
    doc = "Collects the import roots of a Python library or its upstream.",
)

def python_owns_package(srcs, roots, package):
    """Returns whether one of srcs is a file of the package below the repository root or one of roots."""
    prefixes = [root + "/" for root in roots if root] + [""]
    for src in srcs:
        if src.extension not in ("py", "pyi"):
            continue
        rel = repo_relative_path(src.short_path)
        for prefix in prefixes:
            if not rel.startswith(prefix):
                continue
            inner = rel[len(prefix):]
            if inner.startswith(package + "/") or inner in (package + ".py", package + ".pyi"):
                return True
    return False

def python_shard_action(ctx, info, package, docstyle, stem, driver_tool, extract_tool, library):
    """Runs griffe and the extractor for one Python library."""
    if not python_package_name_ok(package):
        fail("python_doc_shard " + str(ctx.label) + ": '" + package + "' is not a Python package name; set package")
    sources = ctx.actions.declare_file(stem + ".sources.txt")
    rows = ctx.actions.args()
    rows.set_param_file_format("multiline")
    rows.add_all(info.transitive_sources, map_each = _source_row)
    ctx.actions.write(sources, rows)
    imports = ctx.actions.declare_file(stem + ".imports.txt")
    roots = ctx.actions.args()
    roots.set_param_file_format("multiline")
    roots.add_all(info.imports)
    ctx.actions.write(imports, roots)
    pyroot = ctx.actions.declare_directory(stem + ".pyroot")
    root = pyroot.path
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    json = ctx.actions.declare_file(stem + ".griffe.json")
    driver = ctx.actions.args()
    driver.add("--root", root)
    driver.add("--package", package)
    driver.add("--docstyle", docstyle)
    driver.add("--out", json)
    driver.add("--sources", sources)
    driver.add("--imports", imports)
    driver.add("--workspace", ctx.workspace_name)
    driver.add("--label", str(ctx.label))
    driver.add("--library", str(library))
    driver.add("--manifest", manifest)
    ctx.actions.run(
        executable = driver_tool,
        arguments = [driver],
        inputs = depset([sources, imports], transitive = [info.transitive_sources]),
        outputs = [json, manifest, pyroot],
        mnemonic = "DxDocsGriffe",
        progress_message = "Reading Python API docs for %{label}",
        use_default_shell_env = False,
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("python_extract")
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
