"""Credential-free release archives for dx deploy."""

load("//core:defs.bzl", "display_label")
load("//core:platform.bzl", "PLATFORM_ATTRS", "platform_identity")
load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":native_runtime.bzl", "NativeRuntimeInfo")
load(":validate_archive.bzl", "consumer_identity_error")
load(":validate_resources.bzl", "resource_path_error")

_ARCHIVER = Label("//deploy/rules:archiver")
_HASHER = str(Label("//deploy/rules:hasher"))

def _archive_stage_impl(ctx):
    """Stages one executable as a single file preserving its basename."""
    exe = ctx.attr.app[DefaultInfo].files_to_run.executable
    if exe == None:
        fail("archive_deploy " + str(ctx.label) + ": app " +
             str(ctx.attr.app.label) + " has no executable")
    staged = ctx.actions.declare_file(ctx.label.name + "/" + exe.basename)
    ctx.actions.symlink(output = staged, target_file = exe)
    return [DefaultInfo(files = depset([staged]))]

_archive_stage = rule(
    implementation = _archive_stage_impl,
    doc = "Stages one executable as a single file with its basename.",
    attrs = {
        "app": attr.label(
            doc = "Executable target staged under its own basename.",
            mandatory = True,
        ),
    },
)

def _archive_app_name_impl(ctx):
    """Writes the app executable basename to a text file."""
    exe = ctx.attr.app[DefaultInfo].files_to_run.executable
    if exe == None:
        fail("archive_deploy " + str(ctx.label) + ": app " +
             str(ctx.attr.app.label) + " has no executable")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output = out, content = exe.basename)
    return [DefaultInfo(files = depset([out]))]

_archive_app_name = rule(
    implementation = _archive_app_name_impl,
    doc = "Writes the app executable basename to a text file.",
    attrs = {
        "app": attr.label(
            doc = "Executable target whose basename is written to a file.",
            mandatory = True,
        ),
    },
)

def _single_file(ctx, target):
    """Returns the one file a target provides."""
    files = target[DefaultInfo].files.to_list()
    if len(files) != 1:
        fail("archive_deploy " + str(ctx.label) + ": " + str(target.label) +
             " provides " + str(len(files)) + " files, want exactly one")
    return files[0]

def _archive_config_impl(ctx):
    """Writes the archive deploy config."""
    app = _single_file(ctx, ctx.attr.app_name)
    tarball = _single_file(ctx, ctx.attr.tarball)
    checksum = _single_file(ctx, ctx.attr.checksum)
    return deploy_config(
        ctx,
        "archive",
        {
            "app_rloc": rlocation_path(ctx, app),
            "checksum_rloc": rlocation_path(ctx, checksum),
            "tarball_rloc": rlocation_path(ctx, tarball),
        },
        ctx.runfiles(files = [app, tarball, checksum]),
    )

_archive_config = rule(
    implementation = _archive_config_impl,
    doc = "Writes the archive deploy config.",
    attrs = {
        "app_name": attr.label(mandatory = True, doc = "Text file with the app basename."),
        "checksum": attr.label(mandatory = True, doc = "Tarball sha256 file."),
        "tarball": attr.label(mandatory = True, doc = "Release tarball."),
    },
)

def archive_filenames(name, format = "tar.gz"):
    """Returns the deterministic (archive, checksum) output names."""
    return (name + "." + format, name + "." + format + ".sha256")

def archive_deploy(name, app, build_mode = "release", size_budget = None, sbom = None, tags = None, visibility = None, testonly = False, target_compatible_with = None):
    """Packages one executable as a deployable tarball and sha256 file, optionally failing above size_budget such as '25MB'."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    (tarball, checksum) = archive_filenames(name)
    archive_target = name + "_archive"
    checksum_target = name + "_checksum"
    stage_target = name + "_stage"

    _archive_stage(
        name = stage_target,
        app = app,
        **attrs
    )

    native.genrule(
        name = archive_target,
        srcs = [":" + stage_target],
        outs = [tarball],
        tools = [_ARCHIVER],
        cmd = "$(location " + str(_ARCHIVER) + ") $(location :" + stage_target + ") $(OUTS)",
        **attrs
    )

    native.genrule(
        name = checksum_target,
        srcs = [":" + archive_target],
        outs = [checksum],
        tools = [_HASHER],
        cmd = "$(location " + _HASHER + ") $(location :" + archive_target + ") $(OUTS)",
        **attrs
    )

    app_name_target = name + "_app_name"
    _archive_app_name(
        name = app_name_target,
        app = app,
        **attrs
    )

    config_target = name + "_config"
    _archive_config(
        name = config_target,
        app_name = ":" + app_name_target,
        checksum = ":" + checksum_target,
        tarball = ":" + archive_target,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        app = app,
        data = [
            ":" + app_name_target,
            ":" + archive_target,
            ":" + checksum_target,
        ],
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [":" + archive_target]) | attrs)
    )

def consumer_identity_text(name, arch, os, linkage, min_runtime, runtimes = []):
    """Renders the deterministic IDENTITY.txt body for one consumer archive."""
    return ("name: " + name + "\n" +
            "arch: " + arch + "\n" +
            "os: " + os + "\n" +
            "linkage: " + linkage + "\n" +
            "min_runtime: " + min_runtime + "\n" +
            "".join(["runtime: " + record + "\n" for record in sorted(runtimes)]))

def native_library_name(basename):
    """Returns True when basename is a native library: a .dll, .dylib, .so or .so.<n> file."""
    if basename.endswith((".dll", ".dylib", ".so")):
        return True
    (stem, sep, version) = basename.rpartition(".so.")
    return stem != "" and sep != "" and version.replace(".", "").isdigit()

_ConsumerStageInfo = provider(
    doc = "Staged consumer archive members.",
    fields = {
        "executable": "String: member name of the app executable.",
        "members": "Dict of String to File: archive member name to staged file.",
        "runtime": "List of String: member names of the shipped runtime files.",
    },
)

def consumer_resource_member(repo, package, name, short_path):
    """Maps one staged resource file to its archive member path, or empty when unmappable."""
    prefix = ("../" + repo + "/" if repo else "") + (package + "/" if package else "") + name + "/"
    if not short_path.startswith(prefix):
        return ""
    rest = short_path[len(prefix):]
    if resource_path_error(rest) != "":
        return ""
    return "resources/" + rest

def _consumer_stage_impl(ctx):
    """Stages one consumer app plus resources, NOTICE and IDENTITY for archiving."""
    id = platform_identity(ctx)
    error = consumer_identity_error(id.cpu, id.os, ctx.attr.linkage, ctx.attr.min_runtime, ctx.attr.format)
    if error != "":
        fail(error + " (in " + str(ctx.label) + ")")
    exe = ctx.attr.app[DefaultInfo].files_to_run.executable
    if exe == None:
        fail("consumer_archive " + str(ctx.label) + ": app " +
             str(ctx.attr.app.label) + " has no executable")
    if ctx.attr.notice == None:
        fail("consumer_archive " + str(ctx.label) + ": need a license-words file for notice")
    notice_files = ctx.attr.notice[DefaultInfo].files.to_list()
    if len(notice_files) == 0:
        fail("consumer_archive " + str(ctx.label) + ": " +
             display_label(ctx.attr.notice.label) + " provides no files")
    if len(notice_files) != 1:
        fail("consumer_archive " + str(ctx.label) + ": " +
             display_label(ctx.attr.notice.label) + " must provide exactly one file")
    by_member = {}
    app_out = ctx.actions.declare_file(ctx.label.name + "/" + exe.basename)
    ctx.actions.symlink(output = app_out, target_file = exe)
    by_member[exe.basename] = app_out
    reserved = [exe.basename, "LICENSE", "NOTICE", "IDENTITY.txt"]
    runtime = []
    runtime_records = []
    provides = []
    for target in ctx.attr.runtime:
        members = {}
        if NativeRuntimeInfo in target:
            info = target[NativeRuntimeInfo]
            if info.redistribution == "build_only":
                fail("consumer_archive " + str(ctx.label) + ": " +
                     display_label(target.label) + " is a compiler/SDK build-only input and cannot ship")
            runtime_records.append(" ".join([info.name, info.version, info.redistribution]))
            provides.extend(info.provides)
            if info.redistribution == "os_supplied":
                continue
            members["notices/" + info.name] = info.notice
            files = info.files
        else:
            files = target[DefaultInfo].files.to_list()
            if len(files) == 0:
                fail("consumer_archive " + str(ctx.label) + ": " +
                     display_label(target.label) + " provides no files")
            for src in files:
                if native_library_name(src.basename):
                    fail("consumer_archive " + str(ctx.label) + ": native library '" +
                         src.basename + "' from " + display_label(target.label) +
                         " needs native_runtime metadata (version, source, notice)")
        for src in files:
            if src.basename in members:
                fail("consumer_archive " + str(ctx.label) + ": duplicate member '" +
                     src.basename + "' from runtime " + display_label(target.label))
            members[src.basename] = src
        for member, src in members.items():
            if member in reserved or member in by_member:
                fail("consumer_archive " + str(ctx.label) + ": duplicate member '" +
                     member + "' from runtime " + display_label(target.label))
            out = ctx.actions.declare_file(ctx.label.name + "/" + member)
            ctx.actions.symlink(output = out, target_file = src)
            by_member[member] = out
            runtime.append(member)
    for target in ctx.attr.resources:
        files = target[DefaultInfo].files.to_list()
        if len(files) == 0:
            fail("consumer_archive " + str(ctx.label) + ": " +
                 display_label(target.label) + " provides no files")
        for src in files:
            member = consumer_resource_member(
                target.label.repo_name,
                target.label.package,
                target.label.name,
                src.short_path,
            )
            if member == "":
                fail("consumer_archive " + str(ctx.label) + ": cannot map " +
                     src.short_path + " from " + display_label(target.label))
            if member in by_member:
                fail("consumer_archive " + str(ctx.label) + ": duplicate member '" +
                     member + "' from " + display_label(target.label))
            out = ctx.actions.declare_file(ctx.label.name + "/" + member)
            ctx.actions.symlink(output = out, target_file = src)
            by_member[member] = out
    notice_out = ctx.actions.declare_file(ctx.label.name + "/NOTICE")
    ctx.actions.symlink(output = notice_out, target_file = notice_files[0])
    by_member["NOTICE"] = notice_out
    if ctx.attr.license_file != None:
        license_files = ctx.attr.license_file[DefaultInfo].files.to_list()
        if len(license_files) != 1:
            fail("consumer_archive " + str(ctx.label) + ": " +
                 display_label(ctx.attr.license_file.label) + " must provide exactly one file")
        license_out = ctx.actions.declare_file(ctx.label.name + "/LICENSE")
        ctx.actions.symlink(output = license_out, target_file = license_files[0])
        by_member["LICENSE"] = license_out
    identity_out = ctx.actions.declare_file(ctx.label.name + "/IDENTITY.txt")
    ctx.actions.write(
        output = identity_out,
        content = consumer_identity_text(
            ctx.attr.archive_name,
            id.cpu,
            id.os,
            ctx.attr.linkage,
            ctx.attr.min_runtime,
            runtime_records,
        ),
    )
    by_member["IDENTITY.txt"] = identity_out
    groups = {}
    if ctx.attr.check_imports:
        checked = [exe.basename] + [member for member in runtime if native_library_name(member)]
        stamp = ctx.actions.declare_file(ctx.label.name + "/imports.ok")
        args = ctx.actions.args()
        args.use_param_file("@%s", use_always = True)
        args.set_param_file_format("multiline")
        args.add("--imports")
        args.add(stamp.path)
        for member in checked:
            args.add("--check", member)
        for name in provides:
            args.add("--provides", name)
        for member, out in by_member.items():
            args.add(member + "=" + out.path)
        ctx.actions.run(
            executable = ctx.executable._archiver,
            arguments = [args],
            inputs = depset(by_member.values()),
            outputs = [stamp],
            mnemonic = "DxConsumerImports",
            progress_message = "Dx check imports %{label}",
        )
        groups["_validation"] = depset([stamp])
    return [
        DefaultInfo(files = depset(by_member.values())),
        OutputGroupInfo(**groups),
        _ConsumerStageInfo(executable = exe.basename, members = by_member, runtime = runtime),
    ]

_consumer_stage = rule(
    implementation = _consumer_stage_impl,
    doc = "Stages one consumer app with resources, NOTICE and IDENTITY.",
    attrs = {
        "app": attr.label(
            doc = "Consumer app executable target packaged into the archive.",
            mandatory = True,
        ),
        "archive_name": attr.string(
            doc = "Archive name recorded in IDENTITY.txt.",
            mandatory = True,
        ),
        "check_imports": attr.bool(
            doc = "Fail the build when the app or a runtime member imports a library that is neither shipped, a system import nor provided by an os_supplied runtime.",
            default = True,
        ),
        "format": attr.string(doc = "Archive format, tar.gz or zip.", default = "tar.gz"),
        "license_file": attr.label(
            doc = "Optional license file packaged as LICENSE, exactly one file.",
            allow_files = True,
        ),
        "linkage": attr.string(
            doc = "Linkage mode, either static or dynamic, recorded in IDENTITY.txt.",
            mandatory = True,
        ),
        "min_runtime": attr.string(
            doc = "Minimum runtime version recorded in IDENTITY.txt.",
            mandatory = True,
        ),
        "notice": attr.label(
            doc = "License file packaged as NOTICE, exactly one file.",
            allow_files = True,
        ),
        "resources": attr.label_list(
            doc = "Resource targets packaged under derived member paths.",
            allow_files = True,
        ),
        "runtime": attr.label_list(
            doc = "Runtime library targets packaged beside the app executable.",
            allow_files = True,
        ),
        "_archiver": attr.label(
            cfg = "exec",
            default = _ARCHIVER,
            executable = True,
        ),
    } | PLATFORM_ATTRS,
)

def _consumer_tarball_impl(ctx):
    """Archives one staged consumer tree with explicit member names."""
    stage = ctx.attr.stage[_ConsumerStageInfo]
    by_member = stage.members
    for member in by_member:
        if resource_path_error(member) != "":
            fail("consumer_archive " + str(ctx.label) + ": invalid member '" + member + "'")
    out = ctx.actions.declare_file(ctx.attr.tarball_name)
    args = ctx.actions.args()
    args.use_param_file("@%s", use_always = True)
    args.set_param_file_format("multiline")
    args.add("--members")
    args.add(out.path)
    args.add("--executable", stage.executable)
    for member in sorted(by_member.keys()):
        args.add(member + "=" + by_member[member].path)
    ctx.actions.run(
        executable = ctx.executable._archiver,
        arguments = [args],
        inputs = depset(by_member.values()),
        outputs = [out],
        mnemonic = "DxConsumerArchive",
        progress_message = "Dx archive consumer %{label}",
    )
    return [DefaultInfo(files = depset([out]))]

_consumer_tarball = rule(
    implementation = _consumer_tarball_impl,
    doc = "Archives one staged consumer tree as a tarball.",
    attrs = {
        "stage": attr.label(
            doc = "Consumer stage target whose members are archived.",
            mandatory = True,
            providers = [_ConsumerStageInfo],
        ),
        "tarball_name": attr.string(
            doc = "Output filename of the archive tarball.",
            mandatory = True,
        ),
        "_archiver": attr.label(
            cfg = "exec",
            default = _ARCHIVER,
            executable = True,
        ),
    },
)

def consumer_archive(name, app, min_runtime, linkage = "dynamic", resources = [], runtime = [], notice = None, license_file = None, check_imports = True, format = "tar.gz", platform = None, tags = None, target_compatible_with = None, visibility = None, testonly = None):
    """Packages one consumer app with runtime, resources, NOTICE and IDENTITY as a tar.gz or zip and sha256 file. `name` is the archive; `name_checksum` is its sha256 file. `name_stage` and `name_archive` are helpers."""
    (tarball, checksum) = archive_filenames(name, format)
    stage_target = name + "_stage"
    archive_target = name + "_archive"
    checksum_target = name + "_checksum"

    _consumer_stage(
        name = stage_target,
        app = app,
        archive_name = name,
        check_imports = check_imports,
        format = format,
        linkage = linkage,
        min_runtime = min_runtime,
        license_file = license_file,
        notice = notice,
        platform = platform,
        resources = resources,
        runtime = runtime,
        tags = tags,
        target_compatible_with = target_compatible_with,
        testonly = testonly,
    )

    _consumer_tarball(
        name = archive_target,
        stage = ":" + stage_target,
        tags = tags,
        tarball_name = tarball,
        target_compatible_with = target_compatible_with,
        testonly = testonly,
        visibility = visibility,
    )

    native.alias(
        name = name,
        actual = ":" + archive_target,
        tags = tags,
        target_compatible_with = target_compatible_with,
        testonly = testonly,
        visibility = visibility,
    )

    native.genrule(
        name = checksum_target,
        srcs = [":" + archive_target],
        outs = [checksum],
        tags = tags,
        target_compatible_with = target_compatible_with,
        testonly = testonly,
        tools = [_HASHER],
        visibility = visibility,
        cmd = "$(location " + _HASHER + ") $(location :" + archive_target + ") $(OUTS)",
    )
