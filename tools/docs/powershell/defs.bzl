"""PowerShell API reference shards parsed by the pinned pwsh runtime."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")

_SOURCE_EXTENSIONS = [".ps1", ".psm1"]

def _where(ctx):
    return "powershell_doc_shard " + ctx.label.package + ":" + ctx.label.name

def _manifest(ctx, files):
    if ctx.file.manifest:
        return ctx.file.manifest
    manifests = [f for f in files if f.extension == "psd1"]
    if len(manifests) > 1:
        fail(_where(ctx) + ": srcs has more than one .psd1; set manifest")
    return manifests[0] if manifests else None

def _powershell_doc_shard_impl(ctx):
    files = sorted(
        depset(transitive = [src[DefaultInfo].files for src in ctx.attr.srcs]).to_list(),
        key = lambda f: repo_relative(f.short_path),
    )
    sources = [f for f in files if ("." + f.extension) in _SOURCE_EXTENSIONS]
    if not sources:
        fail(_where(ctx) + ": srcs has no .ps1 or .psm1 file")
    manifest = _manifest(ctx, files)
    package = ctx.attr.package
    if not package and manifest:
        package = manifest.basename[:-len(".psd1")]
    if not package:
        fail(_where(ctx) + ": package is required")

    srcs_manifest = ctx.actions.declare_file(ctx.label.name + ".srcs.tsv")
    ctx.actions.write(
        output = srcs_manifest,
        content = "".join([f.path + "\t" + repo_relative(f.short_path) + "\n" for f in sources]),
    )
    api_json = ctx.actions.declare_file(ctx.label.name + ".api.json")
    pwsh = ctx.attr._pwsh[platform_common.ToolchainInfo]
    arguments = [
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-File",
        ctx.file._script.path,
        "-Sources",
        srcs_manifest.path,
        "-Out",
        api_json.path,
    ]
    inputs = sources + [srcs_manifest, ctx.file._script]
    if manifest:
        arguments.extend(["-Manifest", manifest.path, "-ManifestRel", repo_relative(manifest.short_path)])
        inputs.append(manifest)
    ctx.actions.run(
        executable = pwsh.pwsh,
        arguments = arguments,
        inputs = depset(inputs),
        tools = pwsh.all_files,
        outputs = [api_json],
        mnemonic = "DxDocsPowershellApi",
        progress_message = "Parsing PowerShell API of %{label}",
        use_default_shell_env = False,
        env = {
            "POWERSHELL_TELEMETRY_OPTOUT": "1",
            "POWERSHELL_UPDATECHECK": "Off",
        },
    )

    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    args = ctx.actions.args()
    args.add("--api", api_json)
    args.add("--out", shard)
    args.add("--package", package)
    args.add("--config", ctx.attr.config)
    ctx.actions.run(
        executable = ctx.executable._extract,
        arguments = [args],
        inputs = [api_json],
        outputs = [shard],
        mnemonic = "DxDocsPowershellExtract",
        progress_message = "Extracting PowerShell API docs for %{label}",
    )
    return [
        DefaultInfo(files = depset([shard])),
        OutputGroupInfo(
            api_json = depset([api_json]),
            srcs_manifest = depset([srcs_manifest]),
        ),
        DocsShardInfo(language = "powershell", package = package, shard = shard),
    ]

powershell_doc_shard = rule(
    doc = """Parses one PowerShell script module into a DocIr shard.

The sources are parsed by the pwsh runtime's own parser; they are never imported,
dot-sourced or run.""",
    implementation = _powershell_doc_shard_impl,
    attrs = {
        "config": attr.string(doc = "Extra configuration identity recorded on every symbol."),
        "manifest": attr.label(
            allow_single_file = [".psd1"],
            doc = "The module manifest. Defaults to the one .psd1 in srcs, if any.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids. Defaults to the manifest file name."),
        "srcs": attr.label_list(
            allow_files = [".ps1", ".psm1", ".psd1"],
            allow_empty = False,
            mandatory = True,
            doc = "The module's .psm1 and every .ps1 it dot-sources, or pwsh_library targets.",
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/powershell/extract:powershell_extract"),
            executable = True,
        ),
        "_pwsh": attr.label(
            cfg = "exec",
            default = Label("@rules_powershell//powershell:current_pwsh_toolchain"),
        ),
        "_script": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/powershell:extract.ps1"),
        ),
    },
    provides = [DocsShardInfo],
)
