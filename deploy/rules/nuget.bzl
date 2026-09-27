"""Local-first NuGet publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")

NUGET_SCHEMA_VERSION = 1

_VALID_ID_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"

_VALID_VERSION_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-+"

NUGET_DEFAULT_SOURCE = "https://api.nuget.org/v3/index.json"

def nuget_id_charset():
    """Returns the launcher-safe package-id charset via registry query."""
    return _VALID_ID_CHARS

def nuget_version_charset():
    """Returns the launcher-safe package-version charset via registry query."""
    return _VALID_VERSION_CHARS

def nuget_schema_error():
    """Validates the versioned package id/version charset schema."""
    if NUGET_SCHEMA_VERSION != 1:
        return "nuget id: unsupported schema v" + str(NUGET_SCHEMA_VERSION) + " (want v1)"
    if type(_VALID_ID_CHARS) != "string" or _VALID_ID_CHARS == "":
        return "nuget id: want a non-empty charset (schema v1)"
    seen = {}
    for c in _VALID_ID_CHARS.elems():
        if c in seen:
            return "nuget id: duplicate charset char '" + c + "'"
        seen[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/", "+"]:
            return "nuget id: unsafe charset char '" + c + "' (must stay launcher-safe)"
    if type(_VALID_VERSION_CHARS) != "string" or _VALID_VERSION_CHARS == "":
        return "nuget version: want a non-empty charset (schema v1)"
    seen_version = {}
    for c in _VALID_VERSION_CHARS.elems():
        if c in seen_version:
            return "nuget version: duplicate charset char '" + c + "'"
        seen_version[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/"]:
            return "nuget version: unsafe charset char '" + c + "' (must stay launcher-safe)"
    return ""

def nuget_id_error(package_id):
    """Validates one package id value."""
    if type(package_id) != "string" or package_id == "":
        return ("nuget_deploy: invalid package id '" + str(package_id) +
                "': want a non-empty id (for example 'nuget_demo')")
    for c in package_id.elems():
        if c not in _VALID_ID_CHARS:
            return ("nuget_deploy: invalid package id '" + package_id +
                    "': want only [A-Za-z0-9._-] so the id embeds " +
                    "safely in the deploy launcher")
    return ""

def nuget_version_error(version):
    """Validates one package version value."""
    if type(version) != "string" or version == "":
        return ("nuget_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    for c in version.elems():
        if c not in _VALID_VERSION_CHARS:
            return ("nuget_deploy: invalid version '" + version +
                    "': want only [A-Za-z0-9._-+] so the version embeds " +
                    "safely in the deploy launcher")
    return ""

def nuget_nupkg_error(filename):
    """Validates one nupkg filename value."""
    if type(filename) != "string" or filename == "":
        return ("nuget_deploy: invalid nupkg '" + str(filename) +
                "': want a non-empty .nupkg filename")
    if not filename.endswith(".nupkg"):
        return ("nuget_deploy: invalid nupkg '" + filename +
                "': want a filename ending in .nupkg")
    for c in filename.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("nuget_deploy: invalid nupkg '" + filename +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the filename embeds safely in the deploy launcher")
    return ""

def nuget_source_error(source):
    """Validates one NuGet source URL value."""
    if type(source) != "string" or source == "":
        return ("nuget_deploy: invalid source '" + str(source) +
                "': want a non-empty https URL (for example '" +
                NUGET_DEFAULT_SOURCE + "')")
    if not source.startswith("https://"):
        return ("nuget_deploy: invalid source '" + source +
                "': want an https URL so pushes never go over plaintext")
    for c in source.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("nuget_deploy: invalid source '" + source +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the URL embeds safely in the deploy launcher")
    return ""

def _nuget_launcher_impl(ctx):
    """Expands the rust_binary launcher for one NuGet deployment."""
    nupkg_files = ctx.attr.nupkg[DefaultInfo].files.to_list()
    if len(nupkg_files) != 1:
        fail("nuget_deploy " + str(ctx.label) + ": nupkg " +
             str(ctx.attr.nupkg.label) + " provides " +
             str(len(nupkg_files)) + " files, want exactly one .nupkg")
    nupkg_file = nupkg_files[0]
    nupkg_error = nuget_nupkg_error(nupkg_file.basename)
    if nupkg_error != "":
        fail(nupkg_error + " (in " + str(ctx.label) + ")")
    nupkg_rloc = rlocation_path(ctx, nupkg_file)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const NUPKG_RLOC: &str = \"" + nupkg_rloc + "\";\n" +
                  "const PACKAGE_ID: &str = \"" + ctx.attr.package_id + "\";\n" +
                  "const PACKAGE_VERSION: &str = \"" + ctx.attr.version + "\";\n" +
                  "const PACKAGE_SOURCE: &str = \"" + ctx.attr.source + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::nuget_main(NUPKG_RLOC, PACKAGE_ID, PACKAGE_VERSION, PACKAGE_SOURCE, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_nuget_launcher = rule(
    implementation = _nuget_launcher_impl,
    attrs = {
        "nupkg": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "package_id": attr.string(
            mandatory = True,
        ),
        "source": attr.string(
            mandatory = True,
        ),
        "version": attr.string(
            mandatory = True,
        ),
    },
)

def nuget_deploy(name, nupkg, version = "0.0.0", source = NUGET_DEFAULT_SOURCE, profile = "release"):
    """Publishes one nupkg as a local-first NuGet deployment."""
    name_error = nuget_id_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    version_error = nuget_version_error(version)
    if version_error != "":
        fail(version_error + " (in " + native.package_name() + ":" + name + ")")
    source_error = nuget_source_error(source)
    if source_error != "":
        fail(source_error + " (in " + native.package_name() + ":" + name + ")")

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _nuget_launcher(
        name = launcher_target,
        nupkg = nupkg,
        package_id = name,
        source = source,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [nupkg],
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
