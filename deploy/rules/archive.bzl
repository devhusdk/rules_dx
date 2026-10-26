"""Credential-free release archives for dx deploy."""

load("//libs/starlark:defs.bzl", "display_label")
load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":resources.bzl", "resource_path_error")

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
        "app": attr.label(mandatory = True),
    },
)

def _archive_launcher_impl(ctx):
    """Expands the rust_binary launcher source from names alone."""
    repo = ctx.label.repo_name or ctx.workspace_name
    prefix = repo + "/" + (ctx.label.package + "/" if ctx.label.package else "")
    app_rloc = prefix + ctx.attr.app_name
    archive_rloc = prefix + ctx.attr.tarball
    checksum_rloc = prefix + ctx.attr.checksum

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const APP_RLOC: &str = \"" + app_rloc + "\";\n" +
                  "const TARBALL_RLOC: &str = \"" + archive_rloc + "\";\n" +
                  "const CHECKSUM_RLOC: &str = \"" + checksum_rloc + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::archive_main(APP_RLOC, TARBALL_RLOC, CHECKSUM_RLOC, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_archive_launcher = rule(
    implementation = _archive_launcher_impl,
    doc = "Writes the archive deploy launcher Rust source.",
    attrs = {
        "app_name": attr.string(mandatory = True),
        "checksum": attr.string(mandatory = True),
        "tarball": attr.string(mandatory = True),
    },
)

def archive_filenames(name):
    """Returns the deterministic (tarball, checksum) output names."""
    return (name + ".tar.gz", name + ".tar.gz.sha256")

def archive_deploy(name, app, profile = "release"):
    """Packages one executable as a deployable tarball and sha256 file."""
    (tarball, checksum) = archive_filenames(name)
    archive_target = name + "_archive"
    checksum_target = name + "_checksum"
    program_target = name + "_program"
    stage_target = name + "_stage"

    _archive_stage(
        name = stage_target,
        app = app,
    )

    native.genrule(
        name = archive_target,
        srcs = [":" + stage_target],
        outs = [tarball],
        tools = [_ARCHIVER],
        cmd = "$(location " + str(_ARCHIVER) + ") $(location :" + stage_target + ") $(OUTS)",
    )

    native.genrule(
        name = checksum_target,
        srcs = [":" + archive_target],
        outs = [checksum],
        tools = [_HASHER],
        cmd = "$(location " + _HASHER + ") $(location :" + archive_target + ") $(OUTS)",
    )

    app_name_target = name + "_app_name"
    _archive_app_name(
        name = app_name_target,
        app = app,
    )

    launcher_target = program_target + "_launcher"
    _archive_launcher(
        name = launcher_target,
        app_name = app_name_target + ".txt",
        checksum = checksum,
        tarball = tarball,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        deps = [Label("//deploy/rules:dx_deploy_tools")],
    )

    dx_deployment(
        name = name,
        app = app,
        data = [
            ":" + app_name_target,
            ":" + archive_target,
            ":" + checksum_target,
        ],
        deploy = ":" + program_target,
        profile = profile,
    )

CONSUMER_ARCHES = ["x86_64", "aarch64"]

CONSUMER_OSES = ["linux", "macos", "windows"]

CONSUMER_LINKAGES = ["static", "dynamic"]

def consumer_identity_error(arch, os, linkage, min_runtime):
    """Validates one consumer archive identity, returning an error or empty."""
    if arch not in CONSUMER_ARCHES:
        return ("consumer_archive: unsupported arch '" + str(arch) +
                "': want one of " + ", ".join(CONSUMER_ARCHES))
    if os not in CONSUMER_OSES:
        return ("consumer_archive: unsupported os '" + str(os) +
                "': want one of " + ", ".join(CONSUMER_OSES))
    if linkage not in CONSUMER_LINKAGES:
        return ("consumer_archive: unsupported linkage '" + str(linkage) +
                "': want one of " + ", ".join(CONSUMER_LINKAGES))
    if type(min_runtime) != "string" or min_runtime == "":
        return "consumer_archive: min_runtime must be a non-empty requirement"
    return ""

def consumer_identity_text(name, arch, os, linkage, min_runtime):
    """Renders the deterministic IDENTITY.txt body for one consumer archive."""
    return ("name: " + name + "\n" +
            "arch: " + arch + "\n" +
            "os: " + os + "\n" +
            "linkage: " + linkage + "\n" +
            "min_runtime: " + min_runtime + "\n")

_ConsumerStageInfo = provider(
    doc = "Staged consumer archive members.",
    fields = {
        "executable": "String: member name of the app executable.",
        "members": "Dict of String to File: archive member name to staged file.",
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
    error = consumer_identity_error(ctx.attr.arch, ctx.attr.os, ctx.attr.linkage, ctx.attr.min_runtime)
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
    identity_out = ctx.actions.declare_file(ctx.label.name + "/IDENTITY.txt")
    ctx.actions.write(
        output = identity_out,
        content = consumer_identity_text(
            ctx.attr.archive_name,
            ctx.attr.arch,
            ctx.attr.os,
            ctx.attr.linkage,
            ctx.attr.min_runtime,
        ),
    )
    by_member["IDENTITY.txt"] = identity_out
    return [
        DefaultInfo(files = depset(by_member.values())),
        _ConsumerStageInfo(executable = exe.basename, members = by_member),
    ]

_consumer_stage = rule(
    implementation = _consumer_stage_impl,
    doc = "Stages one consumer app with resources, NOTICE and IDENTITY.",
    attrs = {
        "app": attr.label(mandatory = True),
        "arch": attr.string(mandatory = True),
        "archive_name": attr.string(mandatory = True),
        "linkage": attr.string(mandatory = True),
        "min_runtime": attr.string(mandatory = True),
        "notice": attr.label(allow_files = True),
        "os": attr.string(mandatory = True),
        "resources": attr.label_list(allow_files = True),
    },
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
        "stage": attr.label(mandatory = True, providers = [_ConsumerStageInfo]),
        "tarball_name": attr.string(mandatory = True),
        "_archiver": attr.label(
            cfg = "exec",
            default = _ARCHIVER,
            executable = True,
        ),
    },
)

def consumer_archive(name, app, arch, os, min_runtime, linkage = "dynamic", resources = [], notice = None, tags = None):
    """Packages one consumer app with resources, NOTICE and IDENTITY as a tarball and sha256 file."""
    (tarball, checksum) = archive_filenames(name)
    stage_target = name + "_stage"
    archive_target = name + "_archive"
    checksum_target = name + "_checksum"

    _consumer_stage(
        name = stage_target,
        app = app,
        arch = arch,
        archive_name = name,
        linkage = linkage,
        min_runtime = min_runtime,
        notice = notice,
        os = os,
        resources = resources,
        tags = tags,
    )

    _consumer_tarball(
        name = archive_target,
        stage = ":" + stage_target,
        tags = tags,
        tarball_name = tarball,
    )

    native.genrule(
        name = checksum_target,
        srcs = [":" + archive_target],
        outs = [checksum],
        tags = tags,
        tools = [_HASHER],
        cmd = "$(location " + _HASHER + ") $(location :" + archive_target + ") $(OUTS)",
    )
