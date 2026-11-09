"""Windows MSIX packaging for target binaries."""

load("@rules_dx//powershell/rules:defs.bzl", "powershell_binary")
load("@rules_dx//libs/starlark:packaging.bzl", "claim_dest", "data_path", "platform_arch")
load(":pins.bzl", "MAX_VERSION_TESTED", "MIN_VERSION")
load(":validate.bzl", "PLATFORM_VALUES", "msix_alias_errors", "msix_application_errors", "msix_identity_errors", "msix_named_version_errors", "msix_platform_errors", "msix_publisher_errors", "msix_version_errors")

MsixInfo = provider(
    doc = "Identity of one MSIX package.",
    fields = {
        "arch": "String: x64 or arm64.",
        "identity": "File: JSON with the packaged identities.",
        "name": "String: package identity name.",
        "package": "File: the .msix.",
        "publisher": "String: publisher distinguished name.",
        "version": "String: four-part package version.",
    },
)

TOOLCHAIN_TYPE = str(Label("//:toolchain_type"))

def compatible_with(constraints):
    """Returns the target constraints of a msix_package target, always including Windows."""
    return ["@platforms//os:windows"] + [c for c in constraints if c != "@platforms//os:windows"]

def publisher_cn(publisher):
    """Returns the unescaped CN value of a publisher distinguished name."""
    value = ""
    escaped = False
    quoted = False
    for char in publisher[len("CN="):].elems():
        if escaped:
            value += char
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == '"':
            quoted = not quoted
        elif not quoted and char in ",;+":
            break
        else:
            value += char
    return value.strip()

def _add(ctx, entries, seen, kind, dest, file):
    """Adds a packaged file once. Returns False for a repeat of the same file."""
    prior = claim_dest(seen, dest, file, fold_case = True)
    if prior != None and hasattr(prior, "path") and prior.path == file.path:
        return False
    if prior != None:
        fail("msix_package " + str(ctx.label) + ": " + getattr(prior, "short_path", prior) + " and " + file.short_path + " both map to " + dest)
    entries.append({"dest": dest, "kind": kind, "src": file.path})
    return True

def _msix_impl(ctx):
    errors = msix_platform_errors(ctx.attr.platforms)
    if errors:
        fail("msix_package " + str(ctx.label) + ": " + errors[0])
    cv = platform_common.ConstraintValueInfo
    arch = platform_arch(ctx.target_platform_has_constraint, ctx.attr._windows[cv], [
        ("arm64", ctx.attr._arm64[cv]),
        ("x64", ctx.attr._x86_64[cv]),
    ])
    if not arch:
        fail("msix_package " + str(ctx.label) + ": target cell this platform is unsupported: only windows_x86_64 and windows_arm64 are pinned")
    errors = (
        msix_identity_errors(ctx.attr.identity_name) +
        msix_publisher_errors(ctx.attr.publisher) +
        msix_version_errors(ctx.attr.version) +
        msix_named_version_errors("min_version", ctx.attr.min_version) +
        msix_named_version_errors("max_version_tested", ctx.attr.max_version_tested) +
        msix_application_errors(ctx.attr.application_id) +
        msix_alias_errors(ctx.attr.alias)
    )
    if errors:
        fail("msix_package " + str(ctx.label) + ": " + "; ".join(errors))
    toolchain = ctx.toolchains[TOOLCHAIN_TYPE]
    if toolchain == None:
        fail("msix_package " + str(ctx.label) + ": packaging needs a windows_x86_64 or windows_arm64 execution platform, and this build has none")
    if toolchain.error:
        fail(toolchain.error)
    exe = ctx.attr.binary[DefaultInfo].files_to_run.executable
    if exe == None or not exe.basename.lower().endswith(".exe"):
        fail("msix_package " + str(ctx.label) + ": binary " + str(ctx.attr.binary.label) + " has no .exe executable")

    entries = []
    seen = {"appxmanifest.xml": "the generated manifest"}
    inputs = [exe, ctx.file.logo]
    _add(ctx, entries, seen, "exe", exe.basename, exe)
    _add(ctx, entries, seen, "logo", "Assets/" + ctx.file.logo.basename, ctx.file.logo)
    dlls = {}
    owner = {}
    for file in ctx.attr.binary[DefaultInfo].default_runfiles.files.to_list():
        name = file.basename.lower()
        if name.endswith(".dll") and (name not in dlls or file.dirname == exe.dirname):
            dlls[name] = file
            owner[name] = "runfiles " + (str(file.owner) if file.owner else file.short_path)
    for target in ctx.attr.libs:
        found = [f for f in target[DefaultInfo].files.to_list() if f.basename.lower().endswith(".dll")]
        if not found:
            fail("msix_package " + str(ctx.label) + ": libs entry " + str(target.label) + " has no .dll")
        for file in found:
            name = file.basename.lower()
            if name in dlls and dlls[name].path != file.path:
                fail("msix_package " + str(ctx.label) + ": libs " + str(target.label) + " and " + owner[name] + " both provide " + file.basename)
            dlls[name] = file
            owner[name] = "libs " + str(target.label)
    for name in sorted(dlls):
        if _add(ctx, entries, seen, "lib", dlls[name].basename, dlls[name]):
            inputs.append(dlls[name])
    for target in ctx.attr.data:
        for file in target[DefaultInfo].files.to_list():
            if _add(ctx, entries, seen, "data", "share/" + data_path(file, target.label), file):
                inputs.append(file)
    for target in ctx.attr.notice:
        for file in target[DefaultInfo].files.to_list():
            if _add(ctx, entries, seen, "notice", "licenses/" + file.basename, file):
                inputs.append(file)

    package = ctx.actions.declare_file(ctx.label.name + ".msix")
    identity = ctx.actions.declare_file(ctx.label.name + ".msix.identity.json")
    request = ctx.actions.declare_file(ctx.label.name + ".msix.request.json")
    ctx.actions.write(request, json.encode_indent({
        "alias": ctx.attr.alias,
        "application_id": ctx.attr.application_id,
        "arch": arch,
        "description": ctx.attr.description or ctx.attr.display_name or ctx.attr.identity_name,
        "display_name": ctx.attr.display_name or ctx.attr.identity_name,
        "executable": exe.basename,
        "files": entries,
        "identity": identity.path,
        "logo": "Assets/" + ctx.file.logo.basename,
        "makeappx": toolchain.makeappx.path,
        "makeappx_version": toolchain.version,
        "max_version_tested": ctx.attr.max_version_tested,
        "min_version": ctx.attr.min_version,
        "name": ctx.attr.identity_name,
        "package": package.path,
        "publisher": ctx.attr.publisher,
        "publisher_display_name": ctx.attr.publisher_display_name or publisher_cn(ctx.attr.publisher),
        "version": ctx.attr.version,
    }, indent = "  ") + "\n")

    pwsh = ctx.attr._pwsh[platform_common.ToolchainInfo]
    ctx.actions.run(
        executable = pwsh.pwsh,
        arguments = ["-NoLogo", "-NoProfile", "-NonInteractive", "-File", ctx.file._pack.path, request.path],
        inputs = depset(inputs + [request, ctx.file._pack, toolchain.makeappx], transitive = [toolchain.files]),
        tools = pwsh.all_files,
        toolchain = TOOLCHAIN_TYPE,
        outputs = [package, identity],
        mnemonic = "DxMsix",
        progress_message = "Packaging MSIX " + str(ctx.label),
        use_default_shell_env = False,
        env = {
            "POWERSHELL_TELEMETRY_OPTOUT": "1",
            "POWERSHELL_UPDATECHECK": "Off",
        },
    )
    return [
        DefaultInfo(files = depset([package, identity])),
        MsixInfo(
            arch = arch,
            identity = identity,
            name = ctx.attr.identity_name,
            package = package,
            publisher = ctx.attr.publisher,
            version = ctx.attr.version,
        ),
    ]

def _platforms_transition_impl(settings, attr):
    if msix_platform_errors(attr.platforms) or not attr.platforms:
        return {"//command_line_option:platforms": settings["//command_line_option:platforms"]}
    return {"//command_line_option:platforms": [str(Label("//platforms:" + PLATFORM_VALUES[attr.platforms[0]]))]}

_platforms_transition = transition(
    implementation = _platforms_transition_impl,
    inputs = ["//command_line_option:platforms"],
    outputs = ["//command_line_option:platforms"],
)

_msix_package = rule(
    implementation = _msix_impl,
    doc = "Packages one Windows target binary and its files as an unsigned MSIX.",
    attrs = {
        "alias": attr.string(doc = "Command-line execution alias, such as app.exe. Empty adds none."),
        "application_id": attr.string(doc = "Application id in the manifest.", default = "App"),
        "binary": attr.label(doc = "The .exe to package.", mandatory = True, executable = True, cfg = "target"),
        "data": attr.label_list(doc = "Resources placed under share/.", allow_files = True),
        "description": attr.string(doc = "Application description. Defaults to the display name."),
        "display_name": attr.string(doc = "Display name. Defaults to the identity name."),
        "identity_name": attr.string(doc = "Package identity name.", mandatory = True),
        "libs": attr.label_list(doc = "Extra DLLs placed next to the executable.", allow_files = True),
        "logo": attr.label(doc = "The PNG logo.", mandatory = True, allow_single_file = [".png"]),
        "max_version_tested": attr.string(doc = "Highest Windows version tested.", default = MAX_VERSION_TESTED),
        "min_version": attr.string(doc = "Lowest Windows version supported.", default = MIN_VERSION),
        "notice": attr.label_list(doc = "License and NOTICE files placed under licenses/.", allow_files = True, mandatory = True, allow_empty = False),
        "platforms": attr.string_list(doc = "One of arm64 or x86_64 to build the package for that architecture. Empty uses the current target platform. Do not set it with select()."),
        "publisher": attr.string(doc = "Publisher distinguished name. Must match the signing certificate subject.", mandatory = True),
        "publisher_display_name": attr.string(doc = "Publisher display name. Defaults to the publisher CN."),
        "version": attr.string(doc = "Four-part package version.", mandatory = True),
        "_arm64": attr.label(default = "@platforms//cpu:arm64"),
        "_pack": attr.label(default = "//:pack.ps1", allow_single_file = True),
        "_pwsh": attr.label(default = "@rules_powershell//powershell:current_pwsh_toolchain", cfg = "exec"),
        "_windows": attr.label(default = "@platforms//os:windows"),
        "_x86_64": attr.label(default = "@platforms//cpu:x86_64"),
    },
    toolchains = [config_common.toolchain_type(TOOLCHAIN_TYPE, mandatory = False)],
    cfg = _platforms_transition,
)

def _windowsonly_chars(target_compatible_with):
    """Adds the Windows constraint to a target_compatible_with value."""
    if type(target_compatible_with) == "list":
        return compatible_with(target_compatible_with)
    return ["@platforms//os:windows"] + target_compatible_with

def msix_package(name, target_compatible_with = [], **kwargs):
    """Packages one Windows target binary and its files as an unsigned MSIX."""
    _msix_package(
        name = name,
        target_compatible_with = _windowsonly_chars(target_compatible_with),
        **kwargs
    )

def msix_sign(name, package, target_compatible_with = [], **kwargs):
    """Declares a run target that signs a copy of one MSIX package."""
    package = str(package)
    signtool = str(Label("//:signtool"))
    data = list(kwargs.pop("data", []))
    env = dict(kwargs.pop("env", {}))
    deps = list(kwargs.pop("deps", []))
    srcs = list(kwargs.pop("srcs", []))
    env["DX_MSIX_PACKAGE"] = "$(rlocationpaths " + package + ")"
    env["DX_MSIX_SIGNTOOL"] = "$(rlocationpaths " + signtool + ")"
    powershell_binary(
        name = name,
        srcs = [Label("//:sign.ps1")] + srcs,
        data = [package, signtool] + data,
        env = env,
        deps = [Label("@rules_powershell//powershell/runfiles")] + deps,
        target_compatible_with = _windowsonly_chars(target_compatible_with),
        **kwargs
    )
