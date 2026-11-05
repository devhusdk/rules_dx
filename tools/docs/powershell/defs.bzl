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

def powershell_shard_action(ctx, files, manifest, package, config, stem, pwsh, script, extract_tool):
    """Parses a PowerShell module and extracts its DocIr shard.

    Args:
        ctx: The rule or aspect context.
        files: The module's source files, `.ps1` and `.psm1`.
        manifest: The module's `.psd1` file, or None.
        package: Package segment of the symbol ids.
        config: Extra configuration identity recorded on every symbol.
        stem: Output name prefix.
        pwsh: The ToolchainInfo of the pinned pwsh runtime.
        script: The extractor script file.
        extract_tool: The DocIr extractor executable.

    Returns:
        A struct with the DocIr `shard`, the `api_json` and `srcs_manifest` files.
    """
    srcs_manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(
        output = srcs_manifest,
        content = "".join([f.path + "\t" + repo_relative(f.short_path) + "\n" for f in files]),
    )
    api_json = ctx.actions.declare_file(stem + ".api.json")
    arguments = [
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-File",
        script.path,
        "-Sources",
        srcs_manifest.path,
        "-Out",
        api_json.path,
    ]
    inputs = files + [srcs_manifest, script]
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

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    args = ctx.actions.args()
    args.add("--api", api_json)
    args.add("--out", shard)
    args.add("--package", package)
    args.add("--config", config)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [args],
        inputs = [api_json],
        outputs = [shard],
        mnemonic = "DxDocsPowershellExtract",
        progress_message = "Extracting PowerShell API docs for %{label}",
    )
    return struct(api_json = api_json, shard = shard, srcs_manifest = srcs_manifest)

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
    action = powershell_shard_action(
        ctx,
        sources,
        manifest,
        package,
        ctx.attr.config,
        ctx.label.name,
        ctx.attr._pwsh[platform_common.ToolchainInfo],
        ctx.file._script,
        ctx.executable._extract,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            api_json = depset([action.api_json]),
            srcs_manifest = depset([action.srcs_manifest]),
        ),
        DocsShardInfo(language = "powershell", package = package, shard = action.shard),
    ]

powershell_doc_shard = rule(
    doc = "Parses one PowerShell script module into a DocIr shard.",
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
