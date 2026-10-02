"""Local-first PyPI publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")

PYPI_SCHEMA_VERSION = 1

_VALID_NAME_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"

PYPI_DEFAULT_REPOSITORY_URL = "https://upload.pypi.org/legacy/"

def pypi_schema_error():
    """Validates the versioned distribution-name charset schema."""
    if PYPI_SCHEMA_VERSION != 1:
        return "pypi name: unsupported schema v" + str(PYPI_SCHEMA_VERSION) + " (want v1)"
    if type(_VALID_NAME_CHARS) != "string" or _VALID_NAME_CHARS == "":
        return "pypi name: want a non-empty charset (schema v1)"
    seen = {}
    for c in _VALID_NAME_CHARS.elems():
        if c in seen:
            return "pypi name: duplicate charset char '" + c + "'"
        seen[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/", "+"]:
            return "pypi name: unsafe charset char '" + c + "' (must stay launcher-safe)"
    return ""

def pypi_name_error(dist_name):
    """Validates one distribution name value."""
    if type(dist_name) != "string" or dist_name == "":
        return ("pypi_deploy: invalid distribution name '" + str(dist_name) +
                "': want a non-empty name (for example 'pypi_demo')")
    for c in dist_name.elems():
        if c not in _VALID_NAME_CHARS:
            return ("pypi_deploy: invalid distribution name '" + dist_name +
                    "': want only [A-Za-z0-9._-] so the name embeds " +
                    "safely in the deploy launcher")
    return ""

def pypi_repository_error(repository_url):
    """Validates one PyPI repository URL value."""
    if type(repository_url) != "string" or repository_url == "":
        return ("pypi_deploy: invalid repository_url '" + str(repository_url) +
                "': want a non-empty https URL (for example '" +
                PYPI_DEFAULT_REPOSITORY_URL + "')")
    if not repository_url.startswith("https://"):
        return ("pypi_deploy: invalid repository_url '" + repository_url +
                "': want an https URL so uploads never go over plaintext")
    for c in repository_url.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("pypi_deploy: invalid repository_url '" + repository_url +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the URL embeds safely in the deploy launcher")
    return ""

def pypi_wheel_error(filename):
    """Validates one wheel filename value."""
    if type(filename) != "string" or filename == "":
        return ("pypi_deploy: invalid wheel '" + str(filename) +
                "': want a non-empty .whl filename")
    if not filename.endswith(".whl"):
        return ("pypi_deploy: invalid wheel '" + filename +
                "': want a filename ending in .whl")
    for c in filename.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("pypi_deploy: invalid wheel '" + filename +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the filename embeds safely in the deploy launcher")
    return ""

def _pypi_launcher_impl(ctx):
    """Expands the rust_binary launcher for one PyPI deployment."""
    wheel_files = ctx.attr.wheel[DefaultInfo].files.to_list()
    if len(wheel_files) != 1:
        fail("pypi_deploy " + str(ctx.label) + ": wheel " +
             str(ctx.attr.wheel.label) + " provides " +
             str(len(wheel_files)) + " files, want exactly one .whl")
    wheel_file = wheel_files[0]
    wheel_error = pypi_wheel_error(wheel_file.basename)
    if wheel_error != "":
        fail(wheel_error + " (in " + str(ctx.label) + ")")
    wheel_rloc = rlocation_path(ctx, wheel_file)

    sdist_rloc = ""
    if ctx.attr.sdist:
        sdist_files = ctx.attr.sdist[DefaultInfo].files.to_list()
        if len(sdist_files) != 1:
            fail("pypi_deploy " + str(ctx.label) + ": sdist " +
                 str(ctx.attr.sdist.label) + " provides " +
                 str(len(sdist_files)) + " files, want exactly one .tar.gz")
        sdist_file = sdist_files[0]
        if not sdist_file.basename.endswith(".tar.gz"):
            fail("pypi_deploy " + str(ctx.label) + ": sdist '" +
                 sdist_file.basename + "' must end in .tar.gz")
        sdist_rloc = rlocation_path(ctx, sdist_file)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const WHEEL_RLOC: &str = \"" + wheel_rloc + "\";\n" +
                  "const SDIST_RLOC: &str = \"" + sdist_rloc + "\";\n" +
                  "const DIST_NAME: &str = \"" + ctx.attr.dist_name + "\";\n" +
                  "const REPOSITORY_URL: &str = \"" + ctx.attr.repository_url + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::pypi_main(WHEEL_RLOC, SDIST_RLOC, DIST_NAME, REPOSITORY_URL, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_pypi_launcher = rule(
    implementation = _pypi_launcher_impl,
    attrs = {
        "dist_name": attr.string(
            mandatory = True,
        ),
        "repository_url": attr.string(
            mandatory = True,
        ),
        "sdist": attr.label(
            allow_single_file = True,
        ),
        "wheel": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
    },
)

def pypi_deploy(name, wheel, sdist = None, repository_url = PYPI_DEFAULT_REPOSITORY_URL, profile = "release"):
    """Publishes one wheel plus an optional sdist as a local-first PyPI deployment."""
    name_error = pypi_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    repository_error = pypi_repository_error(repository_url)
    if repository_error != "":
        fail(repository_error + " (in " + native.package_name() + ":" + name + ")")

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _pypi_launcher(
        name = launcher_target,
        dist_name = name,
        repository_url = repository_url,
        sdist = sdist,
        wheel = wheel,
    )

    data = [wheel]
    if sdist != None:
        data.append(sdist)
    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = data,
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
