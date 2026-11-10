"""Bootstrap environment tool registry."""

load("//core:defs.bzl", "DxSubjectInfo", "display_label")
load(":validate.bzl", "env_collision_error", "env_tool_error")

EnvironmentInfo = provider(
    doc = "Transitively composed bootstrap environment tool records.",
    fields = {
        "runners": "Dict of owner label string to the tool executable's files-to-run object.",
        "tools": "Depset of tool-record structs (bin_name, aliases tuple, owner).",
    },
)

def env_host_filename(name, is_windows):
    """Maps one logical tool name to its host-native filename."""
    if is_windows:
        return name + ".exe"
    return name

def env_tool_record(owner, bin_name, aliases):
    """Builds one tool-record struct for collision analysis."""
    return struct(
        aliases = tuple(aliases),
        bin_name = bin_name,
        owner = owner,
    )

def env_tree_metadata(records, is_windows):
    """Renders the staged tree management metadata as a JSON string."""
    tools = []
    for record in sorted(records, key = lambda r: (r.bin_name, r.owner)):
        names = [record.bin_name] + list(record.aliases)
        tools.append({
            "aliases": list(record.aliases),
            "bin_name": record.bin_name,
            "host_names": [env_host_filename(name, is_windows) for name in names],
            "owner": record.owner,
        })
    return json.encode({
        "tools": tools,
    })

def _environment_tool_impl(ctx):
    error = env_tool_error(ctx.attr.bin_name, ctx.attr.aliases)
    if error != "":
        fail("environment_tool " + str(ctx.label) + ": " + error)

    owner = display_label(ctx.label)
    return [
        EnvironmentInfo(
            runners = {owner: ctx.attr.executable.files_to_run},
            tools = depset([env_tool_record(owner, ctx.attr.bin_name, ctx.attr.aliases)]),
        ),
        DefaultInfo(
            runfiles = ctx.attr.executable[DefaultInfo].default_runfiles,
        ),
    ]

environment_tool = rule(
    implementation = _environment_tool_impl,
    doc = "One tool that dx sync puts on PATH under its names.",
    attrs = {
        "aliases": attr.string_list(
            doc = "Extra command names exposed on PATH for the same tool.",
            default = [],
        ),
        "bin_name": attr.string(
            doc = "Command name the tool is exposed under on PATH.",
            mandatory = True,
        ),
        "executable": attr.label(
            doc = "Executable target or file linked into the environment under its names.",
            allow_files = True,
            cfg = "exec",
            executable = True,
            mandatory = True,
        ),
    },
)

EnvironmentInfo = _EnvironmentInfo
env_host_filename = _env_host_filename
env_tool_record = _env_tool_record
env_tree_metadata = _env_tree_metadata
environment_config = _environment_config
environment_metadata = _environment_metadata
environment_tool = _environment_tool
environment_tree = _environment_tree
