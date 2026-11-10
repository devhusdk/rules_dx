"""Bootstrap environment tool registry."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label")
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

def _environment_config_impl(ctx):
    records = []
    runners = {}
    for tool in ctx.attr.tools:
        info = tool[EnvironmentInfo]
        records.extend(info.tools.to_list())
        runners.update(info.runners)
    error = env_collision_error(records)
    if error != "":
        fail("environment_config " + str(ctx.label) + ": " + error)
    names = {}
    for record in records:
        names[record.bin_name] = True
        for alias in record.aliases:
            names[alias] = True
    return [
        EnvironmentInfo(
            runners = runners,
            tools = depset(records),
        ),
        DefaultInfo(
            files = depset([]),
            runfiles = ctx.runfiles().merge_all([
                tool[DefaultInfo].default_runfiles
                for tool in ctx.attr.tools
            ]),
        ),
        DxSubjectInfo(fields = {
            "count": str(len(records)),
            "names": ",".join(sorted(names.keys())),
        }),
    ]

environment_config = rule(
    implementation = _environment_config_impl,
    doc = "Collects environment tools and rejects host-name collisions.",
    attrs = {
        "tools": attr.label_list(
            doc = "Environment tool targets collected into one config.",
            default = [],
            providers = [DefaultInfo, EnvironmentInfo],
        ),
    },
)

def _environment_tree_impl(ctx):
    info = ctx.attr.config[EnvironmentInfo]
    records = info.tools.to_list()
    is_windows = ctx.target_platform_has_constraint(
        ctx.attr._windows_os[platform_common.ConstraintValueInfo],
    )
    links = []
    host_names = {}
    for record in records:
        if record.owner not in info.runners:
            fail("environment_tree " + str(ctx.label) + ": no runner for " +
                 record.owner + ": every record needs its environment_tool executable")
        executable = info.runners[record.owner].executable
        for name in [record.bin_name] + list(record.aliases):
            host_name = env_host_filename(name, is_windows)
            host_names[host_name] = True
            link = ctx.actions.declare_file("bin/" + host_name)
            ctx.actions.symlink(
                output = link,
                target_file = executable,
                is_executable = True,
            )
            links.append(link)
    metadata = ctx.actions.declare_file(ctx.label.name + ".metadata.json")
    ctx.actions.write(metadata, env_tree_metadata(records, is_windows))
    runfiles = ctx.runfiles(files = links + [metadata]).merge(
        ctx.attr.config[DefaultInfo].default_runfiles,
    )
    return [
        DefaultInfo(files = depset(links + [metadata]), runfiles = runfiles),
        DxSubjectInfo(fields = {
            "count": str(len(records)),
            "host_names": ",".join(sorted(host_names.keys())),
            "platform": "windows" if is_windows else "posix",
        }),
    ]

environment_tree = rule(
    implementation = _environment_tree_impl,
    doc = "Builds the bin directory of tool links for one environment.",
    attrs = {
        "_windows_os": attr.label(
            default = "@platforms//os:windows",
        ),
        "config": attr.label(
            doc = "Environment config whose tools are staged into the tree.",
            mandatory = True,
            providers = [DefaultInfo, EnvironmentInfo],
        ),
    },
)

def _environment_metadata_impl(ctx):
    info = ctx.attr.config[EnvironmentInfo]
    metadata = ctx.actions.declare_file(ctx.label.name + ".metadata.json")
    ctx.actions.write(metadata, env_tree_metadata(info.tools.to_list(), ctx.attr.windows))
    return [DefaultInfo(files = depset([metadata]))]

environment_metadata = rule(
    implementation = _environment_metadata_impl,
    doc = "Writes the metadata JSON of one environment.",
    attrs = {
        "config": attr.label(
            doc = "Environment config whose tools are written as metadata.",
            mandatory = True,
            providers = [DefaultInfo, EnvironmentInfo],
        ),
        "windows": attr.bool(
            doc = "Whether to render host file names for Windows.",
            default = False,
        ),
    },
)
