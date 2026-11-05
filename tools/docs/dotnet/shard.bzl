"""Shared C# and F# API shard action, used by the doc shard rules and the docs aspect."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyCompileInfo", "DotnetAssemblyRuntimeInfo", "DotnetTargetingPackInfo")
load("//fsharp/rules:symbols.bzl", "fsharp_compile_defines")
load("//libs/starlark:resources.bzl", "dx_heavy_resources")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

DotnetDocsInfo = provider(
    doc = "The compile facts of one rules_dotnet library that its API docs are read from.",
    fields = {
        "allow_unsafe_blocks": "Whether the library compiles with unsafe blocks.",
        "assembly": "Assembly name of the library.",
        "defines": "Conditional compilation symbols of the compile.",
        "dep_refs": "Reference assemblies of the direct dependencies outside the framework.",
        "framework_refs": "Reference assemblies of the targeting pack.",
        "langversion": "Language version of the compile.",
        "language": "Source language of the library, csharp or fsharp.",
        "nullable": "Nullable context of the compile, or None for F#.",
        "own_ref": "Reference assembly of the library itself.",
        "ref_count": "Number of reference assemblies the library produced.",
        "srcs": "Source files of the library in its language.",
        "tfm": "Target framework moniker of the compile.",
        "xml_docs": "XML documentation file of an F# library, or None for C#.",
    },
)

def _one(value):
    return value[0] if type(value) == "list" else value

def _collect_impl(target, ctx):
    attr = ctx.rule.attr
    if hasattr(attr, "upstream"):
        upstream = _one(attr.upstream)
        return [upstream[DotnetDocsInfo]] if DotnetDocsInfo in upstream else []
    if DotnetAssemblyCompileInfo not in target or not hasattr(attr, "defines") or not hasattr(attr, "_targeting_pack"):
        return []
    language = "csharp" if hasattr(attr, "allow_unsafe_blocks") else "fsharp"
    compile_info = target[DotnetAssemblyCompileInfo]
    xml_docs = None
    if language == "fsharp":
        xmls = target[DotnetAssemblyRuntimeInfo].xml_docs
        xml_docs = xmls[0] if xmls else None
    framework = {}
    for pack in _one(attr._targeting_pack)[DotnetTargetingPackInfo].assembly_compile_infos:
        for ref in pack.refs:
            framework[ref.basename] = ref
    deps = {}
    for dep in attr.deps:
        info = dep[DotnetAssemblyCompileInfo]
        for ref in info.refs + info.transitive_refs.to_list():
            if ref.basename not in framework:
                deps[ref.path] = ref
    tfm = attr._target_framework[BuildSettingInfo].value
    return [DotnetDocsInfo(
        allow_unsafe_blocks = attr.allow_unsafe_blocks if language == "csharp" else None,
        assembly = compile_info.name,
        defines = fsharp_compile_defines(tfm, attr.target_frameworks, attr.defines),
        dep_refs = sorted(deps.values(), key = lambda f: f.path),
        framework_refs = sorted(framework.values(), key = lambda f: f.basename),
        langversion = attr.langversion,
        language = language,
        nullable = attr.nullable if language == "csharp" else None,
        own_ref = compile_info.refs[0] if compile_info.refs else None,
        ref_count = len(compile_info.refs),
        srcs = [src for src in ctx.rule.files.srcs if src.extension in (["cs"] if language == "csharp" else ["fs", "fsi"])],
        tfm = tfm,
        xml_docs = xml_docs,
    )]

dotnet_docs_collect_aspect = aspect(
    implementation = _collect_impl,
    attr_aspects = ["upstream"],
    doc = "Reads the compile facts of a rules_dotnet C# or F# library behind a rules_dx wrapper.",
)

def _context_json(info, language, references):
    if language == "fsharp":
        return {
            "assembly": info.assembly,
            "defines": sorted(info.defines),
            "dep_refs": [ref.path for ref in references],
            "dll": info.own_ref.path,
            "srcs": sorted([src.path for src in info.srcs]),
            "xml": info.xml_docs.path,
        }
    return {
        "allow_unsafe_blocks": info.allow_unsafe_blocks,
        "assembly": info.assembly,
        "defines": sorted(info.defines),
        "framework_refs": [ref.path for ref in info.framework_refs],
        "own_ref": info.own_ref.path,
        "references": [ref.path for ref in references],
        "srcs": sorted([src.path for src in info.srcs]),
    }

def dotnet_docs_problem(info, language):
    """Returns why a library cannot get a shard in the given language, or None."""
    label = {"csharp": "C#", "fsharp": "F#"}[language]
    if info.language != language:
        return "{}_doc_shard needs a {}_library".format(language, language)
    if language == "csharp" and info.nullable != "disable":
        return "csharp_doc_shard reads docs with docfx, which ignores nullable = \"{}\"; set nullable = \"disable\" and enable nullable with #nullable enable in the source files".format(info.nullable)
    if language == "fsharp" and not info.xml_docs:
        return "fsharp_doc_shard reads the XML documentation file; set generate_documentation_file = True"
    if not info.srcs:
        return "the library has no {} sources".format(label)
    if info.ref_count != 1:
        return "a doc shard needs exactly one reference assembly"
    return None

def dotnet_shard_action(ctx, info, package, stem, driver, extract):
    """Declares the actions that turn one library into a DocIr shard.

    Returns a struct with shard, api_json and manifest.
    """
    language = info.language
    label = {"csharp": "C#", "fsharp": "F#"}[language]
    references = [ref for ref in info.dep_refs]
    lines = [srcs_manifest_line(src.path, repo_relative(src.short_path), not src.is_source) for src in info.srcs]
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    context = ctx.actions.declare_file(stem + ".context.json")
    ctx.actions.write(context, json.encode(_context_json(info, language, references)))
    api_json = ctx.actions.declare_file("{}.{}.json".format(stem, language))
    scratch = ctx.actions.declare_directory(stem + ".scratch")
    if language == "csharp":
        inputs = [context] + info.srcs + info.framework_refs + references + [info.own_ref]
        env = {
            "DOTNET_CLI_TELEMETRY_OPTOUT": "1",
            "DOTNET_NOLOGO": "1",
        }
        config = ";".join([
            "tfm=" + info.tfm,
            "defines=" + ",".join(sorted(info.defines)),
            "unsafe=" + str(info.allow_unsafe_blocks).lower(),
            "langversion=" + (info.langversion or "default"),
        ])
    else:
        inputs = [context] + info.srcs + references + [info.own_ref, info.xml_docs]
        env = {
            "DOTNET_CLI_TELEMETRY_OPTOUT": "1",
            "DOTNET_NOLOGO": "1",
            "DOTNET_SKIP_FIRST_TIME_EXPERIENCE": "1",
        }
        config = ";".join([
            "tfm=" + info.tfm,
            "defines=" + ",".join(sorted(info.defines)),
            "langversion=" + (info.langversion or "default"),
        ])
    ctx.actions.run(
        executable = driver,
        arguments = [ctx.actions.args().add("--context", context).add("--scratch", scratch.path).add("--out", api_json)],
        inputs = inputs,
        outputs = [api_json, scratch],
        env = env,
        mnemonic = "DxDocs" + language.capitalize() + "Api",
        resource_set = dx_heavy_resources,
        progress_message = "Reading {} API docs for %{{label}}".format(label),
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    ctx.actions.run(
        executable = extract,
        arguments = [
            ctx.actions.args()
                .add("--api", api_json)
                .add("--language", language)
                .add("--package", package)
                .add("--srcs-file", manifest)
                .add("--config", config)
                .add("--out", shard),
        ],
        inputs = [api_json, manifest],
        outputs = [shard],
        mnemonic = "DxDocs" + language.capitalize() + "Extract",
        progress_message = "Extracting {} API docs for %{{label}}".format(label),
    )
    return struct(shard = shard, api_json = api_json, manifest = manifest)
