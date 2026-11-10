"""Source fixtures for real aspects."""

load("//quality:sources.bzl", "KNOWN_SEMANTIC_FILE_CLASSES", "QualitySourcesInfo", "QualityToolsInfo", "check_direct_sources", "dockerfile_name_error", "named_source_class_error")

def markdown_sibling_error(what, siblings, markdown_srcs):
    """Returns the validation error for markdown_siblings, or "" when valid."""
    if len(siblings) > 0 and len(markdown_srcs) == 0:
        return ("real_source_target (" + what + "): markdown_siblings needs " +
                "markdown_srcs; siblings are link targets for checked markdown, " +
                "never sources")
    return ""

# File class to the extensions its `<class>_srcs` attribute accepts.
FIXTURE_EXTENSIONS = {
    "ansible": [".yaml", ".yml"],
    "asciidoc": [".adoc", ".asciidoc", ".asc"],
    "bicep": [".bicep", ".bicepparam"],
    "c": [".c", ".h"],
    "cloudformation": [".yaml", ".yml", ".json", ".template"],
    "cmake": True,
    "cpp": [".cc", ".cpp", ".cxx", ".hh", ".hpp", ".hxx"],
    "cue": [".cue"],
    "dockerfile": True,
    "go_module": [".mod"],
    "graphql": [".graphql", ".gql"],
    "haskell": [".hs"],
    "html": [".html", ".htm"],
    "html_template": [".html", ".htm", ".jinja", ".jinja2", ".j2", ".njk", ".hbs", ".tmpl", ".gohtml"],
    "java": [".java"],
    "javascript": [".js", ".mjs", ".cjs"],
    "json": [".json"],
    "json5": [".json5"],
    "jsonc": [".jsonc"],
    "jsonnet": [".jsonnet", ".libsonnet"],
    "jsx": [".jsx"],
    "jupyter": [".ipynb"],
    "kotlin": [".kt", ".kts"],
    "kubernetes": [".yaml", ".yml", ".json"],
    "latex": [".tex", ".sty", ".cls"],
    "lua": [".lua"],
    "make": True,
    "markdown": [".md"],
    "nix": [".nix"],
    "objc": [".m"],
    "objcpp": [".mm"],
    "pkl": [".pkl"],
    "python": [".py"],
    "python_stub": [".pyi"],
    "rego": [".rego"],
    "rst": [".rst", ".rest"],
    "rust": [".rs"],
    "scala": [".scala"],
    "shell": [".sh"],
    "starlark": [".bzl", ".bazel"],
    "terraform": [".tf", ".tfvars", ".hcl"],
    "text": [".txt"],
    "toml": [".toml"],
    "tsx": [".tsx"],
    "typescript": [".ts", ".mts", ".cts"],
    "typst": [".typ"],
    "yaml": [".yaml", ".yml"],
}

# File classes with no `<class>_srcs` attribute.
FIXTURE_EXEMPT_CLASSES = [
    "api_spec",
    "api_spec_ref",
    "astro",
    "avro",
    "capnp",
    "csharp",
    "css",
    "cuda",
    "d",
    "dart",
    "elixir",
    "erlang",
    "flatbuffers",
    "fortran",
    "fsharp",
    "gherkin",
    "github_workflow",
    "go",
    "helm",
    "julia",
    "less",
    "mdx",
    "oci_image",
    "ocaml",
    "perl",
    "php",
    "powershell",
    "protobuf",
    "qml",
    "ruby",
    "scss",
    "sql",
    "svelte",
    "swift",
    "thrift",
    "vue",
    "xml",
    "zig",
]

def fixture_class_error():
    """Returns the error for a taxonomy class with neither a source attribute nor an exemption, or "" when covered."""
    for class_id in KNOWN_SEMANTIC_FILE_CLASSES:
        if class_id not in FIXTURE_EXTENSIONS and class_id not in FIXTURE_EXEMPT_CLASSES:
            return "file class %s has no real_source_target attribute and is not exempt" % class_id
    for class_id in FIXTURE_EXTENSIONS:
        if class_id in FIXTURE_EXEMPT_CLASSES:
            return "file class %s is both a real_source_target attribute and exempt" % class_id
    for class_id in list(FIXTURE_EXTENSIONS) + FIXTURE_EXEMPT_CLASSES:
        if class_id not in KNOWN_SEMANTIC_FILE_CLASSES:
            return "file class %s is not in the taxonomy" % class_id
    return ""

def _real_source_target_impl(ctx):
    direct_sources = {}
    all_files = []
    for class_id in FIXTURE_EXTENSIONS:
        files = getattr(ctx.files, class_id + "_srcs")
        for source in files:
            if class_id == "dockerfile":
                error = dockerfile_name_error(source.basename)
            elif class_id in ["make", "cmake"]:
                error = named_source_class_error(class_id, source.basename)
            elif class_id == "go_module" and source.basename != "go.mod":
                error = "'%s' is not a go_module file name; accepted names: go.mod" % source.basename
            else:
                error = ""
            if error != "":
                fail("real_source_target %s: %s" % (ctx.label, error))
        if len(files) > 0:
            direct_sources[class_id] = depset(files)
            all_files.extend(files)
    check_direct_sources(direct_sources, str(ctx.label))
    sibling_error = markdown_sibling_error(
        str(ctx.label),
        ctx.attr.markdown_siblings,
        ctx.files.markdown_srcs,
    )
    if sibling_error != "":
        fail(sibling_error)
    providers = [
        DefaultInfo(files = depset(all_files)),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]
    converters = {}
    if len(ctx.files.rst_srcs) > 0 and ctx.attr.rst2html:
        # Mirrors rst_library: the owner carries the Vale converter.
        converters["rst2html"] = ctx.attr.rst2html[DefaultInfo].files_to_run
    if len(ctx.files.asciidoc_srcs) > 0 and ctx.attr.asciidoctor:
        converters["asciidoctor"] = ctx.attr.asciidoctor[DefaultInfo].files_to_run
    if len(converters) > 0:
        providers.append(QualityToolsInfo(tools = converters))
    return providers

_real_source_target = rule(
    implementation = _real_source_target_impl,
    doc = "Carries source files of several classes for the quality aspects.",
    attrs = dict({
        class_id + "_srcs": attr.label_list(
            doc = "Files of the %s class checked by quality aspects." % class_id,
            allow_files = extensions,
            default = [],
        )
        for class_id, extensions in FIXTURE_EXTENSIONS.items()
    }, **{
        "markdown_siblings": attr.label_list(
            doc = "Link target files referenced by markdown sources, never checked as sources.",
            allow_files = True,
            default = [],
        ),
        "rst2html": attr.label(
            doc = "Converter that rst_srcs carry for Vale, as rst_library does.",
            cfg = "exec",
            executable = True,
        ),
        "asciidoctor": attr.label(
            doc = "Converter that asciidoc_srcs carry for Vale, as asciidoc_library does.",
            cfg = "exec",
            executable = True,
        ),
    }),
)

_CONVERTERS = [
    ("rst_srcs", "rst_converter", "rst2html", Label("//quality/tools/python:rst2html")),
    ("asciidoc_srcs", "asciidoc_converter", "asciidoctor", Label("//javascript/private/quality/bin:asciidoctor")),
]

def converter_attrs(kwargs):
    """Returns the converter labels a real_source_target with these arguments needs."""
    attrs = {}
    for srcs_attr, flag, attr, label in _CONVERTERS:
        srcs = kwargs.get(srcs_attr, [])
        has_srcs = type(srcs) == "select" or len(srcs) > 0
        if has_srcs and kwargs.get(flag, True) != False:
            attrs[attr] = label
    return attrs

def real_source_target(name, rst_converter = True, asciidoc_converter = True, **kwargs):
    """Sources by language for real quality aspects."""
    flags = dict(kwargs, rst_converter = rst_converter, asciidoc_converter = asciidoc_converter)
    _real_source_target(name = name, **dict(converter_attrs(flags), **kwargs))
