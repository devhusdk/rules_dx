"""Builds deb, rpm and apk packages with nfpm and attaches them to a GitHub release."""

load("//libs/starlark:platform.bzl", "PLATFORM_ATTRS", "platform_identity")
load(":config.bzl", "deploy_attrs", "deploy_identity")
load(":github.bzl", "github_deploy")
load(":validate_linux_package.bzl", "LINUX_PACKAGE_ARCHES", "LINUX_PACKAGE_FORMATS", "linux_package_arch_error", "linux_package_dst_error", "linux_package_formats_error", "linux_package_name_error", "linux_package_version_error")

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

    id = platform_identity(ctx)
    arch_error = linux_package_arch_error(id.os, id.cpu)
    if arch_error != "":
        fail(arch_error + " (in " + str(ctx.label) + ")")
    arch = LINUX_PACKAGE_ARCHES[id.cpu]

    config = ctx.actions.declare_file(ctx.label.name + ".nfpm.yaml")
    ctx.actions.write(
        output = config,
        content = linux_package_config(
            ctx.attr.package,
            ctx.attr.version,
            arch,
            ctx.attr.maintainer,
            ctx.attr.description,
            contents,
        ) + "\n",
    )
    out = ctx.actions.declare_file(linux_package_filename(
        ctx.attr.package,
        ctx.attr.version,
        arch,
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
    } | PLATFORM_ATTRS,
)

def linux_package_deploy(
        name,
        binary,
        version,
        package = "",
        formats = ["deb", "rpm"],
        platform = None,
        maintainer = "unknown <unknown@example.com>",
        description = "",
        install_path = None,
        extra_files = {},
        release_tag = "",
        tag_from_head = False,
        notes = "commits",
        notes_file = None,
        build_mode = "release",
        sbom = None,
        tags = [],
        visibility = None,
        testonly = False,
        target_compatible_with = []):
    """Builds Linux packages from one binary and publishes them as a draft GitHub release."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    where = " (in " + native.package_name() + ":" + name + ")"
    version_error = linux_package_version_error(version)
    if version_error != "":
        fail(version_error + where)
    pkg, version = deploy_identity(name, package, version, linux_package_name_error, linux_package_version_error)
    formats_error = linux_package_formats_error(formats)
    if formats_error != "":
        fail(formats_error + where)

    contents = {binary: install_path or "/usr/bin/" + pkg}
    contents.update(extra_files)

    artifacts = []
    for fmt in formats:
        target = name + "_" + fmt
        linux_package(
            name = target,
            platform = platform,
            contents = contents,
            description = description or pkg,
            format = fmt,
            maintainer = maintainer,
            package = pkg,
            version = version,
            **attrs
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
        sbom = sbom,
        **attrs
    )
