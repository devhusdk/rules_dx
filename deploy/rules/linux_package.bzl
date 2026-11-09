"""Builds deb, rpm and apk packages with nfpm and attaches them to a GitHub release."""

load(":github.bzl", "github_deploy")
load(":validate.bzl", "NAME_CHARS", "only_chars")

LINUX_PACKAGE_FORMATS = ["deb", "rpm", "apk"]

def linux_package_name_error(package):
    """Validates one package name value."""
    if type(package) != "string" or package == "":
        return ("linux_package_deploy: invalid package name '" + str(package) +
                "': want a non-empty name (for example 'demo')")
    if not only_chars(package, NAME_CHARS):
        return ("linux_package_deploy: invalid package name '" + package +
                "': want only [A-Za-z0-9._-]")
    return ""

def linux_package_formats_error(formats):
    """Validates the list of package formats."""
    if type(formats) != "list" or len(formats) == 0:
        return "linux_package_deploy: want at least one format of " + str(LINUX_PACKAGE_FORMATS)
    seen = []
    for fmt in formats:
        if fmt not in LINUX_PACKAGE_FORMATS:
            return ("linux_package_deploy: invalid format '" + str(fmt) +
                    "': want one of " + str(LINUX_PACKAGE_FORMATS))
        if fmt in seen:
            return "linux_package_deploy: duplicate format '" + fmt + "'"
        seen.append(fmt)
    return ""

def linux_package_dst_error(dst):
    """Validates one install path inside the package."""
    if type(dst) != "string" or not dst.startswith("/") or dst.endswith("/"):
        return ("linux_package_deploy: invalid install path '" + str(dst) +
                "': want an absolute file path (for example '/usr/bin/demo')")
    return ""

def linux_package_filename(package, version, arch, fmt):
    """Returns the file name of one built package."""
    return package + "_" + version + "_" + arch + "." + fmt

def linux_package_config(package, version, arch, maintainer, description, contents):
    """Renders the nfpm config; JSON is valid YAML."""
    return json.encode({
        "arch": arch,
        "contents": contents,
        "description": description,
        "maintainer": maintainer,
        "name": package,
        "platform": "linux",
        "version": version,
    })

def _linux_package_impl(ctx):
    """Runs nfpm once for one package format."""
    contents = []
    inputs = []
    for target, dst in ctx.attr.contents.items():
        dst_error = linux_package_dst_error(dst)
        if dst_error != "":
            fail(dst_error + " (in " + str(ctx.label) + ")")
        info = target[DefaultInfo]
        executable = info.files_to_run.executable
        if executable != None:
            src = executable
        else:
            files = info.files.to_list()
            if len(files) != 1:
                fail("linux_package_deploy " + str(ctx.label) + ": " +
                     str(target.label) + " provides " + str(len(files)) +
                     " files, want exactly one")
            src = files[0]
        inputs.append(src)
        mode = 493 if executable != None else 420
        contents.append({
            "dst": dst,
            "file_info": {"mode": mode},
            "src": src.path,
            "type": "file",
        })

    config = ctx.actions.declare_file(ctx.label.name + ".nfpm.yaml")
    ctx.actions.write(
        output = config,
        content = linux_package_config(
            ctx.attr.package,
            ctx.attr.version,
            ctx.attr.arch,
            ctx.attr.maintainer,
            ctx.attr.description,
            contents,
        ) + "\n",
    )
    out = ctx.actions.declare_file(linux_package_filename(
        ctx.attr.package,
        ctx.attr.version,
        ctx.attr.arch,
        ctx.attr.format,
    ))
    nfpm = ctx.executable._nfpm
    ctx.actions.run(
        executable = nfpm,
        arguments = ["package", "--config", config.path, "--packager", ctx.attr.format, "--target", out.path],
        inputs = inputs + [config],
        outputs = [out],
        env = {"SOURCE_DATE_EPOCH": "315532800"},
        mnemonic = "NfpmPackage",
        progress_message = "Building " + out.basename,
    )
    return [DefaultInfo(files = depset([out]))]

linux_package = rule(
    doc = "Builds one deb, rpm or apk package from files with nfpm.",
    implementation = _linux_package_impl,
    attrs = {
        "arch": attr.string(mandatory = True, doc = "Package architecture in Go naming, such as amd64 or arm64."),
        "contents": attr.label_keyed_string_dict(
            allow_files = True,
            mandatory = True,
            doc = "Files to install, mapped to their absolute install path.",
        ),
        "description": attr.string(mandatory = True, doc = "Package description."),
        "format": attr.string(mandatory = True, values = LINUX_PACKAGE_FORMATS, doc = "Package format."),
        "maintainer": attr.string(mandatory = True, doc = "Maintainer, as 'Name <email>'."),
        "package": attr.string(mandatory = True, doc = "Package name."),
        "version": attr.string(mandatory = True, doc = "Package version."),
        "_nfpm": attr.label(
            default = Label("//deploy/tools:nfpm"),
            cfg = "exec",
            executable = True,
            allow_single_file = True,
        ),
    },
)

def linux_package_deploy(
        name,
        binary,
        version = "0.0.0",
        formats = ["deb", "rpm"],
        arch = "amd64",
        maintainer = "unknown <unknown@example.com>",
        description = "",
        install_path = None,
        extra_files = {},
        release_tag = "",
        tag_from_head = False,
        notes = "commits",
        notes_file = None,
        build_mode = "release",
        tags = [],
        sbom = None,
        visibility = None,
        testonly = None):
    """Builds Linux packages from one binary and publishes them as a draft GitHub release."""
    where = " (in " + native.package_name() + ":" + name + ")"
    name_error = linux_package_name_error(name)
    if name_error != "":
        fail(name_error + where)
    formats_error = linux_package_formats_error(formats)
    if formats_error != "":
        fail(formats_error + where)

    contents = {binary: install_path or "/usr/bin/" + name}
    contents.update(extra_files)

    artifacts = []
    for fmt in formats:
        target = name + "_" + fmt
        linux_package(
            name = target,
            arch = arch,
            contents = contents,
            description = description or name,
            format = fmt,
            maintainer = maintainer,
            package = name,
            version = version,
            tags = tags,
            testonly = testonly,
            visibility = visibility,
        )
        artifacts.append(":" + target)

    github_deploy(
        name = name,
        artifacts = artifacts,
        release_tag = release_tag,
        tag_from_head = tag_from_head,
        notes = notes,
        notes_file = notes_file,
        build_mode = build_mode,
        tags = tags,
        sbom = sbom,
        testonly = testonly,
        visibility = visibility,
    )
