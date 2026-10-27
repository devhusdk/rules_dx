"""Local-first NuGet publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")
load(":validate.bzl", "EMBED_UNSAFE", "NAME_CHARS", "VERSION_CHARS", "has_any", "only_chars")

_VALID_ID_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

NUGET_DEFAULT_SOURCE = "https://api.nuget.org/v3/index.json"

def nuget_id_error(package_id):
    """Validates one package id value."""
    if type(package_id) != "string" or package_id == "":
        return ("nuget_deploy: invalid package id '" + str(package_id) +
                "': want a non-empty id (for example 'nuget_demo')")
    if not only_chars(package_id, _VALID_ID_CHARS):
        return ("nuget_deploy: invalid package id '" + package_id +
                "': want only [A-Za-z0-9._-] so the id embeds " +
                "safely in the deploy launcher")
    return ""

def nuget_version_error(version):
    """Validates one package version value."""
    if type(version) != "string" or version == "":
        return ("nuget_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
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
    if has_any(filename, EMBED_UNSAFE):
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
    if has_any(source, EMBED_UNSAFE):
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
                  "const TARGET_NAME: &str = \"" + ctx.label.name + "\";\n" +
                  "const PACKAGE_ID: &str = \"" + ctx.attr.package_id + "\";\n" +
                  "const PACKAGE_VERSION: &str = \"" + ctx.attr.version + "\";\n" +
                  "const PACKAGE_SOURCE: &str = \"" + ctx.attr.source + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::nuget_main(NUPKG_RLOC, TARGET_NAME, PACKAGE_ID, PACKAGE_VERSION, PACKAGE_SOURCE, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_nuget_launcher = rule(
    implementation = _nuget_launcher_impl,
    doc = "Writes the launcher Rust source for one NuGet deployment.",
    attrs = {
        "nupkg": attr.label(
            doc = "Single NuGet package file that is published.",
            allow_single_file = True,
            mandatory = True,
        ),
        "package_id": attr.string(doc = "NuGet package id embedded in the generated launcher."),
        "source": attr.string(
            doc = "HTTPS NuGet feed URL the launcher publishes to.",
            mandatory = True,
        ),
        "version": attr.string(doc = "NuGet package version embedded in the generated launcher."),
    },
)

def nuget_deploy(name, nupkg, package_id = "", version = "", source = NUGET_DEFAULT_SOURCE, profile = "release"):
    """Publishes one nupkg after checking its nuspec id and version."""
    name_error = nuget_id_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    if package_id != "":
        id_error = nuget_id_error(package_id)
        if id_error != "":
            fail(id_error + " (in " + native.package_name() + ":" + name + ")")
    if version != "":
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
        package_id = package_id,
        source = source,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [nupkg],
        deps = [Label("//deploy/rules:dx_deploy_tools")],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
