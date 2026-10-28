"""Windows MSIX packaging for target binaries."""

load("@rules_powershell//powershell:defs.bzl", "pwsh_binary")
load("//libs/starlark:packaging.bzl", "claim_dest", "data_path", "only_chars", "platform_arch")
load(":pins.bzl", "MAX_VERSION_TESTED", "MIN_VERSION")

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

TOOLCHAIN_TYPE = str(Label("//msix:toolchain_type"))

_LETTERS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
_DIGITS = "0123456789"

def named_version_errors(attr, version):
    """Returns why the value of a version attribute is not four numbers up to 65535."""
    parts = version.split(".")
    if len(parts) != 4:
        return [attr + " '" + version + "' must have four parts, as in 1.2.3.0"]
    for part in parts:
        if part == "" or not only_chars(part, _DIGITS) or len(part) > 5 or int(part) > 65535:
            return [attr + " '" + version + "' parts must be numbers from 0 to 65535"]
    return []

def version_errors(version):
    """Returns why a package version is not four numbers up to 65535."""
    return named_version_errors("version", version)

def compatible_with(constraints):
    """Returns the target constraints of a dx_msix target, always including Windows."""
    return ["@platforms//os:windows"] + [c for c in constraints if c != "@platforms//os:windows"]

def identity_errors(name):
    """Returns why a package identity name is invalid."""
    if len(name) < 3 or len(name) > 50 or not only_chars(name, _LETTERS + _DIGITS + ".-"):
        return ["identity_name '" + name + "' must be 3 to 50 letters, digits, dots or dashes"]
    return []

def application_errors(application_id):
    """Returns why an application id is invalid."""
    parts = application_id.split(".")
    for part in parts:
        if part == "" or part[0] not in _LETTERS.elems() or not only_chars(part, _LETTERS + _DIGITS):
            return ["application_id '" + application_id + "' must be dot-separated words that start with a letter"]
    if len(application_id) > 64:
        return ["application_id '" + application_id + "' must be at most 64 characters"]
    return []

def publisher_errors(publisher):
    """Returns why a publisher is not a distinguished name."""
    if not publisher.startswith("CN=") or len(publisher) > 8192:
        return ["publisher '" + publisher + "' must be a distinguished name that starts with CN="]
    return []

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

def alias_errors(alias):
    """Returns why an execution alias is invalid."""
    if alias == "":
        return []
    if not alias.endswith(".exe") or len(alias) < 5 or not only_chars(alias, _LETTERS + _DIGITS + ".-_"):
        return ["alias '" + alias + "' must be a file name that ends with .exe"]
    return []

def _add(ctx, entries, seen, kind, dest, file):
    """Adds a packaged file once. Returns False for a repeat of the same file."""
    prior = claim_dest(seen, dest, file, fold_case = True)
    if prior != None and hasattr(prior, "path") and prior.path == file.path:
        return False
    if prior != None:
        fail("dx_msix " + str(ctx.label) + ": " + getattr(prior, "short_path", prior) + " and " + file.short_path + " both map to " + dest)
    entries.append({"dest": dest, "kind": kind, "src": file.path})
    return True

def _msix_impl(ctx):
    cv = platform_common.ConstraintValueInfo
    arch = platform_arch(ctx.target_platform_has_constraint, ctx.attr._windows[cv], [
        ("arm64", ctx.attr._arm64[cv]),
        ("x64", ctx.attr._x86_64[cv]),
    ])
    if not arch:
        fail("dx_msix " + str(ctx.label) + ": target cell this platform is unsupported: only windows_x86_64 and windows_arm64 are pinned")
    errors = (
        identity_errors(ctx.attr.identity_name) +
        publisher_errors(ctx.attr.publisher) +
        version_errors(ctx.attr.version) +
        named_version_errors("min_version", ctx.attr.min_version) +
        named_version_errors("max_version_tested", ctx.attr.max_version_tested) +
        application_errors(ctx.attr.application_id) +
        alias_errors(ctx.attr.alias)
    )
    if errors:
        fail("dx_msix " + str(ctx.label) + ": " + "; ".join(errors))
    toolchain = ctx.toolchains[TOOLCHAIN_TYPE]
    if toolchain == None:
        fail("dx_msix " + str(ctx.label) + ": packaging needs a windows_x86_64 or windows_arm64 execution platform, and this build has none")
    if toolchain.error:
        fail(toolchain.error)
    exe = ctx.attr.binary[DefaultInfo].files_to_run.executable
    if exe == None or not exe.basename.lower().endswith(".exe"):
        fail("dx_msix " + str(ctx.label) + ": binary " + str(ctx.attr.binary.label) + " has no .exe executable")

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
            fail("dx_msix " + str(ctx.label) + ": libs entry " + str(target.label) + " has no .dll")
        for file in found:
            name = file.basename.lower()
            if name in dlls and dlls[name].path != file.path:
                fail("dx_msix " + str(ctx.label) + ": libs " + str(target.label) + " and " + owner[name] + " both provide " + file.basename)
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

_dx_msix = rule(
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
        "publisher": attr.string(doc = "Publisher distinguished name. Must match the signing certificate subject.", mandatory = True),
        "publisher_display_name": attr.string(doc = "Publisher display name. Defaults to the publisher CN."),
        "version": attr.string(doc = "Four-part package version.", mandatory = True),
        "_arm64": attr.label(default = "@platforms//cpu:arm64"),
        "_pack": attr.label(default = "//msix:pack.ps1", allow_single_file = True),
        "_pwsh": attr.label(default = "@rules_powershell//powershell:current_pwsh_toolchain", cfg = "exec"),
        "_windows": attr.label(default = "@platforms//os:windows"),
        "_x86_64": attr.label(default = "@platforms//cpu:x86_64"),
    },
    toolchains = [config_common.toolchain_type(TOOLCHAIN_TYPE, mandatory = False)],
)

def _windowsonly_chars(target_compatible_with):
    """Adds the Windows constraint to a target_compatible_with value."""
    if type(target_compatible_with) == "list":
        return compatible_with(target_compatible_with)
    return ["@platforms//os:windows"] + target_compatible_with

def dx_msix(name, target_compatible_with = [], **kwargs):
    """Packages one Windows target binary and its files as an unsigned MSIX."""
    _dx_msix(
        name = name,
        target_compatible_with = _windowsonly_chars(target_compatible_with),
        **kwargs
    )

def dx_msix_sign(name, package, target_compatible_with = [], **kwargs):
    """Declares a run target that signs a copy of one dx_msix package."""
    signtool = str(Label("//msix:signtool"))
    pwsh_binary(
        name = name,
        srcs = [Label("//msix:sign.ps1")],
        data = [package, signtool],
        env = {
            "DX_MSIX_PACKAGE": "$(rlocationpaths " + package + ")",
            "DX_MSIX_SIGNTOOL": "$(rlocationpaths " + signtool + ")",
        },
        deps = [Label("@rules_powershell//powershell/runfiles")],
        target_compatible_with = _windowsonly_chars(target_compatible_with),
        **kwargs
    )
