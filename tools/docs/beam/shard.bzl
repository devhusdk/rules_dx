"""Shared actions of the Elixir and Erlang API doc shards: the pinned runtime reads the docs, beam_extract writes DocIr."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("//core/coreutils:defs.bzl", "COREUTILS_TOOLCHAIN_TYPE", "coreutils")
load("//tools/docs/ir:shard.bzl", "repo_relative", "srcs_manifest_line")

OTP_TOOLCHAIN = "@rules_erlang//tools:toolchain_type"
ELIXIR_TOOLCHAIN = "@rules_elixir//:toolchain_type"

_ELIXIR_COMMAND = """set -eu
PATH="$1"
export PATH
shift
otp="$(cd "$(dirname "$1")" && pwd)"
elixir="$(cd "$(dirname "$2")" && pwd)"
shift 2
mkdir -p "$1"
export PATH="$otp/bin:$elixir/bin:$PATH" HOME="$PWD/$1" ELIXIR_ERL_OPTIONS="-elixir ansi_enabled false" LANG=C.UTF-8 LC_ALL=C.UTF-8
shift
exec "$elixir/bin/elixir" "$@"
"""

_ERLANG_COMMAND = """set -eu
PATH="$1"
export PATH
shift
otp="$(cd "$(dirname "$1")" && pwd)"
shift
mkdir -p "$1"
export PATH="$otp/bin:$PATH" HOME="$PWD/$1" LANG=C.UTF-8 LC_ALL=C.UTF-8
shift
exec "$otp/bin/escript" "$@"
"""

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
    ctx.actions.write(manifest, "".join(sorted([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in files])))
    return manifest

def beam_shard_action(ctx, language, srcs, dep_beams, package, stem, driver, extract_tool):
    """Runs the pinned runtime over the sources and the extractor over its JSON.

    Args:
      ctx: The rule or aspect context carrying the OTP toolchain, and the Elixir one for Elixir.
      language: "elixir" or "erlang".
      srcs: The .ex or .erl files the shard documents.
      dep_beams: Compiled dependency files and ebin directories the Elixir compiler loads.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      driver: The elixir_dump.exs or edoc_dump.escript file.
      extract_tool: The beam_extract executable.

    Returns:
      A struct with the DocIr `shard`, the dump `json` and the `manifest`.
    """
    otp = ctx.toolchains[OTP_TOOLCHAIN].otpinfo
    manifest = _manifest(ctx, stem, srcs)
    json = ctx.actions.declare_file(stem + ".dump.json")
    scratch = ctx.actions.declare_directory(stem + ".beam_scratch")
    applets = coreutils(ctx)
    args = ctx.actions.args()
    args.add(applets.path)
    args.add(otp.version_file)
    transitive = [otp.tree, applets.files]
    if language == "elixir":
        elixir = ctx.toolchains[ELIXIR_TOOLCHAIN].elixirinfo
        args.add(elixir.version_file)
        args.add(scratch.path)
        for f in dep_beams:
            args.add("-pa", f.path if f.is_directory else f.dirname)
        args.add(driver)
        args.add("--srcs", manifest)
        args.add("--scratch", scratch.path + "/beams")
        args.add("--out", json)
        transitive.append(elixir.tree)
        command = _ELIXIR_COMMAND
    else:
        args.add(scratch.path)
        args.add(driver)
        args.add(manifest)
        args.add(json)
        command = _ERLANG_COMMAND
    ctx.actions.run_shell(
        command = command,
        arguments = [args],
        inputs = depset(srcs + dep_beams + [manifest, driver, otp.version_file], transitive = transitive),
        outputs = [json, scratch],
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
