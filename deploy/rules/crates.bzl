"""Local-first crates.io publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")
load(":validate.bzl", "ALNUM", "EMBED_UNSAFE", "VERSION_CHARS", "has_any", "only_chars")

_VALID_NAME_CHARS = ALNUM + "_-"

_VALID_VERSION_CHARS = VERSION_CHARS

def crates_name_error(crate_name):
    """Validates one crate name value."""
    if type(crate_name) != "string" or crate_name == "":
        return ("crates_deploy: invalid crate name '" + str(crate_name) +
                "': want a non-empty name (for example 'crates_demo')")
    if not only_chars(crate_name, _VALID_NAME_CHARS):
        return ("crates_deploy: invalid crate name '" + crate_name +
                "': want only [A-Za-z0-9_-] so the name embeds " +
                "safely in the deploy launcher")
    return ""

def crates_version_error(version):
    """Validates one crate version value."""
    if type(version) != "string" or version == "":
        return ("crates_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("crates_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+] so the version embeds " +
                "safely in the deploy launcher")
    return ""

def crates_file_error(filename):
    """Validates one crate source filename value."""
    if type(filename) != "string" or filename == "":
        return ("crates_deploy: invalid crate file '" + str(filename) +
                "': want a non-empty filename")
    if has_any(filename, EMBED_UNSAFE):
        return ("crates_deploy: invalid crate file '" + filename +
                "': want no quotes, backslashes, spaces, or newlines " +
                "so the filename embeds safely in the deploy launcher")
    return ""

def _crates_launcher_impl(ctx):
    """Expands the rust_binary launcher for one crates.io deployment."""
    crate_files = ctx.attr.crate[DefaultInfo].files.to_list()
    if len(crate_files) == 0:
        fail("crates_deploy " + str(ctx.label) + ": crate " +
             str(ctx.attr.crate.label) + " provides no files, want at least one source file")
    rlocs = []
    for f in crate_files:
        file_error = crates_file_error(f.basename)
        if file_error != "":
            fail(file_error + " (in " + str(ctx.label) + ")")
        rlocs.append(rlocation_path(ctx, f))

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const CRATE_RLOCS: &str = \"" + ";".join(rlocs) + "\";\n" +
                  "const CRATE_NAME: &str = \"" + ctx.attr.crate_name + "\";\n" +
                  "const CRATE_VERSION: &str = \"" + ctx.attr.version + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::crates_main(CRATE_RLOCS, CRATE_NAME, CRATE_VERSION, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_crates_launcher = rule(
    implementation = _crates_launcher_impl,
    doc = "Writes the launcher Rust source for one crates.io deployment.",
    attrs = {
        "crate": attr.label(
            allow_files = True,
            mandatory = True,
        ),
        "crate_name": attr.string(
            mandatory = True,
        ),
        "version": attr.string(
            mandatory = True,
        ),
    },
)

def crates_deploy(name, crate, version = "0.0.0", profile = "release"):
    """Publishes staged crate sources as a local-first crates.io deployment."""
    name_error = crates_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    version_error = crates_version_error(version)
    if version_error != "":
        fail(version_error + " (in " + native.package_name() + ":" + name + ")")

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _crates_launcher(
        name = launcher_target,
        crate = crate,
        crate_name = name,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [crate],
        deps = [Label("//deploy/rules:dx_deploy_tools")],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
