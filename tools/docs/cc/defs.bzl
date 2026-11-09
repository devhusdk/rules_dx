"""C, C++ and CUDA API reference shards parsed from public headers by the pinned Doxygen."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//tools/docs/cc:repos.bzl", "UNSUPPORTED_REASON")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

_EXECUTABLES = ["doxygen", "doxygen.exe"]

# Settings that keep the run reproducible and the XML limited to the listed headers.
_DOXYFILE = [
    "ALPHABETICAL_INDEX = NO",
    "CLASS_GRAPH = NO",
    "CASE_SENSE_NAMES = YES",
    "ENABLE_PREPROCESSING = YES",
    "EXTENSION_MAPPING = cu=C++ cuh=C++ h=C++ inl=C++",
    "EXTRACT_ALL = YES",
    "EXTRACT_ANON_NSPACES = NO",
    "EXTRACT_PRIVATE = NO",
    "EXTRACT_STATIC = YES",
    "GENERATE_HTML = NO",
    "GENERATE_LATEX = NO",
    "GENERATE_XML = YES",
    "HAVE_DOT = NO",
    "MACRO_EXPANSION = NO",
    "QUIET = YES",
    "SEARCH_INCLUDES = NO",
    "SKIP_FUNCTION_MACROS = NO",
    "WARNINGS = NO",
    "WARN_IF_UNDOCUMENTED = NO",
    "XML_OUTPUT = xml",
    "XML_PROGRAMLISTING = NO",
]

_PREDEFINED = {
    "cc": ["__cplusplus"],
    "cuda": ["__cplusplus", "__CUDACC__", "__host__=", "__device__=", "__global__=", "__forceinline__=", "__managed__=", "__constant__=", "__shared__="],
}

def _where(ctx):
    return ctx.attr._language + "_doc_shard " + str(ctx.label)

def _quote(value):
    return "\"" + value.replace("\"", "\\\"") + "\""

def _doxygen_executable(files):
    for f in files:
        if f.basename in _EXECUTABLES:
            return f
    return None

def doxygen_available(files):
    """Returns whether the pinned Doxygen has a build for this host.

    Args:
      files: The files of the @dx_doxygen tool filegroup.

    Returns:
      True when the filegroup holds a Doxygen executable.
    """
    return _doxygen_executable(files) != None

def cc_shard_action(ctx, language, headers, package, stem, doxygen_files, extract_tool):
    """Runs Doxygen over the public headers of one library, then the extractor.

    Args:
      ctx: The rule or aspect context.
      language: `cc` or `cuda`, the first segment of every symbol id.
      headers: The public header files the library lists.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      doxygen_files: The files of the pinned Doxygen.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the Doxygen `xml` tree and the `manifest`.
    """
    headers = sorted({f.path: f for f in headers}.values(), key = lambda f: f.path)
    if not headers:
        fail(_where(ctx) + ": lib lists no public headers")
    doxygen = _doxygen_executable(doxygen_files)
    if doxygen == None:
        fail(_where(ctx) + ": " + UNSUPPORTED_REASON)
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in headers])))

    tree = ctx.actions.declare_directory(stem + ".doxygen")
    config = ctx.actions.declare_file(stem + ".Doxyfile")
    lines = list(_DOXYFILE) + [
        "OUTPUT_DIRECTORY = " + _quote(tree.path),
        "INPUT = " + " ".join([_quote(f.path) for f in headers]),
        "PREDEFINED = " + " ".join(_PREDEFINED[language]),
        "PROJECT_NAME = " + _quote(package),
    ]
    ctx.actions.write(config, "\n".join(lines) + "\n")
    ctx.actions.run(
        executable = doxygen,
        arguments = [config.path],
        inputs = depset(headers + [config], transitive = [depset(doxygen_files)]),
        outputs = [tree],
        mnemonic = "DxDocsDoxygen",
        progress_message = "Reading " + language + " API docs for %{label}",
        use_default_shell_env = False,
        env = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8"},
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--xml", tree.path + "/xml")
    extract.add("--srcs", manifest)
    extract.add("--package", package)
    extract.add("--language", language)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [tree, manifest],
        outputs = [shard],
        mnemonic = "DxDocsCcExtract",
        progress_message = "Extracting " + language + " API docs for %{label}",
    )
    return struct(shard = shard, xml = tree, manifest = manifest)

def _shard_impl(ctx):
    package = ctx.attr.package or ctx.attr.lib.label.name
    headers = ctx.attr.lib[CcInfo].compilation_context.direct_public_headers
    action = cc_shard_action(ctx, ctx.attr._language, headers, package, ctx.label.name, ctx.files._doxygen, ctx.executable._extract)
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            doxygen_xml = depset([action.xml]),
            srcs_manifest = depset([action.manifest]),
        ),
        DocsShardInfo(language = ctx.attr._language, package = package, shard = action.shard),
    ]

def _cc_shard_rule(language, what):
    return rule(
        doc = "Converts one " + what + " library's public headers into a DocIr shard with Doxygen.",
        implementation = _shard_impl,
        attrs = {
            "lib": attr.label(
                mandatory = True,
                providers = [CcInfo],
                doc = "The documented library; its public hdrs are documented.",
            ),
            "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of lib."),
            "_doxygen": attr.label(
                cfg = "exec",
                default = Label("@dx_doxygen//:tool"),
            ),
            "_extract": attr.label(
                cfg = "exec",
                default = Label("//tools/docs/cc/extract:cc_extract"),
                executable = True,
            ),
            "_language": attr.string(default = language),
        },
        provides = [DocsShardInfo],
    )

docs_cc_shard = _cc_shard_rule("cc", "C or C++")

docs_cuda_shard = _cc_shard_rule("cuda", "CUDA")

def cc_doc_shard(name, lib, package = "", **kwargs):
    """Declares the DocIr shard of one C or C++ library.

    Args:
      name: Name of the shard target.
      lib: The documented cc_library; its public hdrs are documented.
      package: Package segment of every symbol id; defaults to the name of lib.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_cc_shard(name = name, lib = lib, package = package, **kwargs)

def cuda_doc_shard(name, lib, package = "", **kwargs):
    """Declares the DocIr shard of one CUDA library.

    Args:
      name: Name of the shard target.
      lib: The documented cuda_library; its public hdrs are documented.
      package: Package segment of every symbol id; defaults to the name of lib.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_cuda_shard(name = name, lib = lib, package = package, **kwargs)
