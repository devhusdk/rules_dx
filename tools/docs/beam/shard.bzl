"""Shared actions of the Elixir and Erlang API doc shards: the pinned runtime reads the docs, beam_extract writes DocIr."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("//core:rlocation.bzl", "repo_relative_path")
load("//core/coreutils:defs.bzl", "coreutils")
load("//tools/docs/ir:shard.bzl", "srcs_manifest_line")

OTP_TOOLCHAIN = "@rules_erlang//tools:toolchain_type"
ELIXIR_TOOLCHAIN = "@rules_elixir//:toolchain_type"

ESCRIPT_TOOL = Label("//erlang/private/tools:escript_bin")
ELIXIR_DUMP_TOOL = Label("//tools/docs/elixir:elixir_dump")

def beam_sources(app, extensions):
    """Returns the workspace sources of an app with one of the extensions, sorted by path."""
    files = [f for f in app[ErlangAppInfo].srcs if f.extension in extensions and f.owner != None and f.owner.repo_name == ""]
    return sorted(files, key = lambda f: f.path)

def beam_dirs(app):
    """Returns the directories and files of the compiled dependencies of an app."""
    files = []
    for dep in app[ErlangAppInfo].deps:
        files.extend(dep[ErlangAppInfo].beam)
    return files

def _manifest(ctx, stem, files):
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted([srcs_manifest_line(f.path, repo_relative_path(f.short_path), not f.is_source) for f in files])))
    return manifest

def beam_shard_action(ctx, language, srcs, dep_beams, package, stem, runner, extract_tool, driver = None):
    """Runs the pinned runtime over the sources and the extractor over its JSON."""
    manifest = _manifest(ctx, stem, srcs)
    json = ctx.actions.declare_file(stem + ".dump.json")
    applets = coreutils(ctx)
    args = ctx.actions.args()
    inputs = srcs + dep_beams + [manifest]
    outputs = [json]
    if language == "elixir":
        scratch = ctx.actions.declare_directory(stem + ".beam_scratch")
        outputs.append(scratch)
        for f in dep_beams:
            args.add("--pa", f.path if f.is_directory else f.dirname)
        args.add("--srcs", manifest)
        args.add("--scratch", scratch.path + "/beams")
        args.add("--out", json)
    else:
        inputs.append(driver)
        args.add(driver)
        args.add(manifest)
        args.add(json)
    ctx.actions.run(
        executable = runner,
        arguments = [args],
        inputs = depset(inputs, transitive = [applets.files]),
        outputs = outputs,
        env = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "PATH": applets.path},
        mnemonic = "DxDocs" + language.capitalize(),
        progress_message = "Reading " + language.capitalize() + " API docs for %{label}",
        use_default_shell_env = False,
        toolchain = None,
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", json)
    extract.add("--package", package)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [json],
        outputs = [shard],
        mnemonic = "DxDocsBeamExtract",
        toolchain = None,
        progress_message = "Extracting " + language.capitalize() + " API docs for %{label}",
    )
    return struct(shard = shard, json = json, manifest = manifest)
