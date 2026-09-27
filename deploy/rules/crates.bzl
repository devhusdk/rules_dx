"""Local-first crates.io publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")

CRATES_SCHEMA_VERSION = 1

_VALID_NAME_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-"

_VALID_VERSION_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-+"

def crates_name_charset():
    """Returns the launcher-safe crate-name charset via registry query."""
    return _VALID_NAME_CHARS

def crates_version_charset():
    """Returns the launcher-safe crate-version charset via registry query."""
    return _VALID_VERSION_CHARS

def crates_schema_error():
    """Validates the versioned crate name/version charset schema."""
    if CRATES_SCHEMA_VERSION != 1:
        return "crates name: unsupported schema v" + str(CRATES_SCHEMA_VERSION) + " (want v1)"
    if type(_VALID_NAME_CHARS) != "string" or _VALID_NAME_CHARS == "":
        return "crates name: want a non-empty charset (schema v1)"
    seen = {}
    for c in _VALID_NAME_CHARS.elems():
        if c in seen:
            return "crates name: duplicate charset char '" + c + "'"
        seen[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/", "+", "."]:
            return "crates name: unsafe charset char '" + c + "' (must stay launcher-safe)"
    if type(_VALID_VERSION_CHARS) != "string" or _VALID_VERSION_CHARS == "":
        return "crates version: want a non-empty charset (schema v1)"
    seen_version = {}
    for c in _VALID_VERSION_CHARS.elems():
        if c in seen_version:
            return "crates version: duplicate charset char '" + c + "'"
        seen_version[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/"]:
            return "crates version: unsafe charset char '" + c + "' (must stay launcher-safe)"
    return ""

def crates_name_error(crate_name):
    """Validates one crate name value."""
    if type(crate_name) != "string" or crate_name == "":
        return ("crates_deploy: invalid crate name '" + str(crate_name) +
                "': want a non-empty name (for example 'crates_demo')")
    for c in crate_name.elems():
        if c not in _VALID_NAME_CHARS:
            return ("crates_deploy: invalid crate name '" + crate_name +
                    "': want only [A-Za-z0-9_-] so the name embeds " +
                    "safely in the deploy launcher")
    return ""

def crates_version_error(version):
    """Validates one crate version value."""
    if type(version) != "string" or version == "":
        return ("crates_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    for c in version.elems():
        if c not in _VALID_VERSION_CHARS:
            return ("crates_deploy: invalid version '" + version +
                    "': want only [A-Za-z0-9._-+] so the version embeds " +
                    "safely in the deploy launcher")
    return ""

def crates_file_error(filename):
    """Validates one crate source filename value."""
    if type(filename) != "string" or filename == "":
        return ("crates_deploy: invalid crate file '" + str(filename) +
                "': want a non-empty filename")
    for c in filename.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("crates_deploy: invalid crate file '" + filename +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the filename embeds safely in the deploy launcher")
    return ""

def crates_allow_dirty_error(allow_dirty):
    """Validates the clean-tree gate."""
    if allow_dirty != False:
        return ("crates_deploy: allow_dirty=True requires explicit owner " +
                "approval; keep a clean tree and publish from committed " +
                "sources instead")
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
                  "const ALLOW_DIRTY: bool = " + ("true" if ctx.attr.allow_dirty else "false") + ";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::crates_main(CRATE_RLOCS, CRATE_NAME, CRATE_VERSION, ALLOW_DIRTY, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_crates_launcher = rule(
    implementation = _crates_launcher_impl,
    attrs = {
        "allow_dirty": attr.bool(),
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

def crates_deploy(name, crate, version = "0.0.0", allow_dirty = False, profile = "release"):
    """Publishes staged crate sources as a local-first crates.io deployment."""
    name_error = crates_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    version_error = crates_version_error(version)
    if version_error != "":
        fail(version_error + " (in " + native.package_name() + ":" + name + ")")
    dirty_error = crates_allow_dirty_error(allow_dirty)
    if dirty_error != "":
        fail(dirty_error + " (in " + native.package_name() + ":" + name + ")")

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _crates_launcher(
        name = launcher_target,
        allow_dirty = allow_dirty,
        crate = crate,
        crate_name = name,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [crate],
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
