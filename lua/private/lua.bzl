"""Core Lua rules: modules, entry files and the shared launcher. Loads nothing from dx."""

load(":providers.bzl", "LUA_ENTRY_SUFFIX", "LuaInfo")

LUA_TOOLCHAIN_TYPE = Label("//lua/toolchain:toolchain_type")

def repo_relative_path(short_path):
    """Returns a File.short_path relative to the root of its own repository."""
    if short_path.startswith("../"):
        rest = short_path[3:]
        slash = rest.find("/")
        return rest[slash + 1:] if slash >= 0 else ""
    return short_path

def canonical_rlocation(short_path, workspace_name):
    """Returns the canonical runfiles key of one File.short_path."""
    if short_path.startswith("../"):
        return short_path[3:]
    return workspace_name + "/" + short_path

def _join(parts):
    return "/".join([p for p in parts if p])

def _strip_slashes(path):
    path = path.strip()
    for _ in range(len(path)):
        if not path.endswith("/"):
            break
        path = path[:-1]
    return path

def import_root(package, strip_import_prefix):
    """Returns the repo-relative directory module names start from."""
    if not strip_import_prefix:
        return ""
    if strip_import_prefix.startswith("/"):
        return _strip_slashes(strip_import_prefix[1:])
    return _strip_slashes(_join([package, strip_import_prefix]))

def module_name(path, package = "", strip_import_prefix = "", import_prefix = ""):
    """Returns the require name of one repo-relative .lua path, or None."""
    if not path.endswith(".lua"):
        return None
    root = import_root(package, strip_import_prefix)
    rest = path
    if root:
        if not path.startswith(root + "/"):
            return None
        rest = path[len(root) + 1:]
    segments = rest[:-len(".lua")].split("/")
    prefix = [s for s in import_prefix.replace(".", "/").split("/") if s]
    if segments[-1] == "init" and (len(segments) > 1 or prefix):
        segments = segments[:-1]
    segments = prefix + segments
    for segment in segments:
        if not segment or "." in segment:
            return None
    return ".".join(segments)

def entry_lines(main_rlocation, source_repo, modules):
    """Returns the text of one .luaentry file; modules is a list of (name, rlocation) pairs."""
    lines = [main_rlocation, source_repo]
    for name, rlocation in sorted(modules):
        lines.append(name + "\t" + rlocation)
    return "\n".join(lines) + "\n"

def lua_executable_name(name, launcher_basename):
    """Returns the executable file name of one Lua target, keeping the launcher's .exe."""
    if launcher_basename.endswith(".exe"):
        return name + ".exe"
    return name

def _module_map(entries, what):
    by_name = {}
    for name, file in entries:
        seen = by_name.get(name)
        if seen == None:
            by_name[name] = file
        elif seen != file:
            fail("%s: module '%s' is provided by both %s and %s" % (what, name, seen.owner, file.owner))
    return by_name

def _own_modules(ctx, srcs):
    out = []
    for src in srcs:
        name = module_name(
            repo_relative_path(src.short_path),
            ctx.label.package,
            ctx.attr.strip_import_prefix,
            ctx.attr.import_prefix,
        )
        if name == None:
            fail("%s: cannot name a module for %s under strip_import_prefix '%s'" % (
                ctx.label,
                src.short_path,
                ctx.attr.strip_import_prefix,
            ))
        out.append((name, src))
    return out

def _validation(ctx, srcs):
    luac = ctx.toolchains[LUA_TOOLCHAIN_TYPE].luac
    outputs = []
    for src in srcs:
        out = ctx.actions.declare_file("_luac/%s/%s.luac" % (ctx.label.name, src.short_path.replace("../", "_ext/")))
        args = ctx.actions.args()
        args.add("-s")
        args.add("-o", out)
        args.add(src)
        ctx.actions.run(
            executable = luac,
            arguments = [args],
            inputs = [src],
            outputs = [out],
            mnemonic = "LuacCheck",
            progress_message = "Checking Lua syntax of %{input}",
        )
        outputs.append(out)
    return outputs

def _collect(ctx, srcs):
    deps = ctx.attr.deps
    own = _own_modules(ctx, srcs)
    modules = depset(own, transitive = [d[LuaInfo].modules for d in deps])
    _module_map(modules.to_list(), str(ctx.label))
    runfiles = ctx.runfiles(files = srcs + ctx.files.data)
    for target in ctx.attr.data:
        runfiles = runfiles.merge(target[DefaultInfo].default_runfiles)
    runfiles = runfiles.merge_all([d[LuaInfo].runfiles for d in deps])
    return modules, runfiles

def lua_library_impl(ctx):
    """Returns the providers of one lua_library."""
    srcs = ctx.files.srcs
    modules, runfiles = _collect(ctx, srcs)
    return [
        DefaultInfo(files = depset(srcs), runfiles = runfiles),
        LuaInfo(modules = modules, runfiles = runfiles),
        OutputGroupInfo(_validation = depset(_validation(ctx, srcs))),
        coverage_common.instrumented_files_info(
            ctx,
            source_attributes = ["srcs"],
            dependency_attributes = ["deps", "data"],
            extensions = ["lua"],
        ),
    ]

def lua_executable_impl(ctx):
    """Returns the providers of one lua_binary or lua_test."""
    main = ctx.file.main
    srcs = ctx.files.srcs
    own_srcs = [main] + [s for s in srcs if s != main]
    modules, runfiles = _collect(ctx, [s for s in own_srcs if s != main])
    module_map = _module_map(modules.to_list(), str(ctx.label))

    toolchain = ctx.toolchains[LUA_TOOLCHAIN_TYPE]
    launcher = toolchain.launcher
    executable = ctx.actions.declare_file(lua_executable_name(ctx.label.name, launcher.basename))
    ctx.actions.symlink(output = executable, target_file = launcher, is_executable = True)
    entry = ctx.actions.declare_file(ctx.label.name + LUA_ENTRY_SUFFIX)
    ctx.actions.write(entry, entry_lines(
        canonical_rlocation(main.short_path, ctx.workspace_name),
        ctx.label.repo_name,
        [(name, canonical_rlocation(f.short_path, ctx.workspace_name)) for name, f in module_map.items()],
    ))

    runfiles = ctx.runfiles(files = [executable, entry, main]).merge_all([
        runfiles,
        toolchain.launcher_runfiles,
    ])
    env = {
        key: ctx.expand_location(value, ctx.attr.data)
        for key, value in ctx.attr.env.items()
    }
    return [
        DefaultInfo(executable = executable, files = depset([executable, entry]), runfiles = runfiles),
        RunEnvironmentInfo(environment = env),
        OutputGroupInfo(_validation = depset(_validation(ctx, own_srcs))),
        coverage_common.instrumented_files_info(
            ctx,
            source_attributes = ["srcs", "main"],
            dependency_attributes = ["deps", "data"],
            extensions = ["lua"],
        ),
    ]

_COMMON_ATTRS = {
    "data": attr.label_list(
        allow_files = True,
        doc = "Files the Lua code reads at run time; find them with require('runfiles').rlocation.",
    ),
    "deps": attr.label_list(
        providers = [LuaInfo],
        doc = "lua_library targets whose modules this target requires.",
    ),
    "srcs": attr.label_list(
        allow_files = [".lua"],
        doc = "Lua modules of this target. Each file becomes one module named after its path.",
    ),
}

LUA_LIBRARY_ATTRS = _COMMON_ATTRS | {
    "import_prefix": attr.string(
        doc = "Module prefix added after strip_import_prefix, for example 'vendor.inspect'.",
    ),
    "strip_import_prefix": attr.string(
        doc = "Directory removed from module paths. '/x' starts at the repository root, 'x' at the package.",
    ),
}

LUA_EXECUTABLE_ATTRS = LUA_LIBRARY_ATTRS | {
    "env": attr.string_dict(
        doc = "Environment variables set when the target runs or is tested; $(location) is expanded.",
    ),
    "main": attr.label(
        mandatory = True,
        allow_single_file = [".lua"],
        doc = "The .lua chunk that runs first. It receives the command-line arguments in arg and in ....",
    ),
}

LUA_TEST_ATTRS = LUA_EXECUTABLE_ATTRS | {
    "_lcov_merger": attr.label(
        default = configuration_field(fragment = "coverage", name = "output_generator"),
        executable = True,
        cfg = "exec",
        doc = "Coverage output generator that bazel coverage runs after the test.",
    ),
}

LUA_TOOLCHAINS = [LUA_TOOLCHAIN_TYPE]

lua_library = rule(
    implementation = lua_library_impl,
    doc = "Lua modules other Lua targets can require.",
    attrs = LUA_LIBRARY_ATTRS,
    provides = [LuaInfo],
    toolchains = LUA_TOOLCHAINS,
)

lua_binary = rule(
    implementation = lua_executable_impl,
    doc = "A Lua program that bazel run starts through the shared launcher.",
    attrs = LUA_EXECUTABLE_ATTRS,
    executable = True,
    toolchains = LUA_TOOLCHAINS,
)

lua_test = rule(
    implementation = lua_executable_impl,
    doc = "A Lua test; it passes when main returns or calls os.exit(0).",
    attrs = LUA_TEST_ATTRS,
    test = True,
    toolchains = LUA_TOOLCHAINS,
)
