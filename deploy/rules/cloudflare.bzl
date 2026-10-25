"""Explicit-apply Cloudflare Pages publisher for dx deploy."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "package_rlocation", "rlocation_path")

_VERIFY = Label("//deploy/rules:cloudflare_verify")
_HEX = "0123456789abcdef"
_PROJECT_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789-"
_BRANCH_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._/-"

def cloudflare_pages_intent(branch, production_branch):
    """Returns 'production' or 'preview' for a branch."""
    return "production" if branch == production_branch else "preview"

def _branch_error(attr, value):
    if type(value) != "string" or value == "" or len(value) > 100:
        return ("cloudflare_pages_deploy: invalid " + attr + " '" + str(value) +
                "': want 1-100 characters of [A-Za-z0-9._/-]")
    for c in value.elems():
        if c not in _BRANCH_CHARS:
            return ("cloudflare_pages_deploy: invalid " + attr + " '" + value +
                    "': want 1-100 characters of [A-Za-z0-9._/-]")
    if value.startswith("-") or value.startswith("/") or value.endswith("/") or ".." in value or "//" in value:
        return ("cloudflare_pages_deploy: invalid " + attr + " '" + value +
                "': no leading '-', no '..', no empty path parts")
    return ""

def cloudflare_pages_error(account_id, project, branch, production_branch):
    """Validates the account, project and branch values."""
    if type(account_id) != "string" or len(account_id) != 32:
        return ("cloudflare_pages_deploy: invalid account_id '" + str(account_id) +
                "': want 32 lowercase hex characters")
    for c in account_id.elems():
        if c not in _HEX:
            return ("cloudflare_pages_deploy: invalid account_id '" + account_id +
                    "': want 32 lowercase hex characters")
    bad_project = type(project) != "string" or project == "" or len(project) > 58 or project.startswith("-") or project.endswith("-")
    if not bad_project:
        for c in project.elems():
            if c not in _PROJECT_CHARS:
                bad_project = True
    if bad_project:
        return ("cloudflare_pages_deploy: invalid project '" + str(project) +
                "': want 1-58 characters of [a-z0-9-] that start and end with a letter or digit")
    branch_error = _branch_error("branch", branch)
    if branch_error != "":
        return branch_error
    return _branch_error("production_branch", production_branch)

def _cloudflare_site_ref_impl(ctx):
    """Writes the runfiles rlocation of one site directory."""
    site = ctx.attr.site[DefaultInfo].files.to_list()
    if len(site) != 1 or not site[0].is_directory:
        fail("cloudflare_pages_deploy " + str(ctx.label) + ": site " +
             str(ctx.attr.site.label) + " must provide exactly one directory " +
             "(for example a javascript_web_bundle)")
    ref = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(output = ref, content = rlocation_path(ctx, site[0]))
    return [DefaultInfo(files = depset([ref]))]

cloudflare_site_ref = rule(
    doc = "Writes the runfiles rlocation of one Pages site directory.",
    implementation = _cloudflare_site_ref_impl,
    attrs = {
        "site": attr.label(mandatory = True, allow_files = True, doc = "Static site directory."),
    },
)

def _cloudflare_launcher_impl(ctx):
    """Expands the rust_binary launcher for one Pages deployment."""
    wrangler = ctx.attr.wrangler[DefaultInfo].files_to_run.executable
    if wrangler == None:
        fail("cloudflare_pages_deploy " + str(ctx.label) + ": wrangler " +
             str(ctx.attr.wrangler.label) + " has no executable")
    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = """fn main() {{
    let launch = dx_deploy_tools::PagesLaunch {{
        deploy_name: {name},
        account_id: {account},
        project: {project},
        branch: {branch},
        production_branch: {production},
        site_ref_rloc: {site_ref},
        wrangler_rloc: {wrangler},
    }};
    std::process::exit(dx_deploy_tools::cloudflare_pages_main(&launch, &std::env::args().collect::<Vec<_>>()));
}}
""".format(
            name = json.encode(ctx.attr.deploy_name),
            account = json.encode(ctx.attr.account_id),
            project = json.encode(ctx.attr.project),
            branch = json.encode(ctx.attr.branch),
            production = json.encode(ctx.attr.production_branch),
            site_ref = json.encode(package_rlocation(ctx, ctx.attr.site_ref)),
            wrangler = json.encode(rlocation_path(ctx, wrangler)),
        ),
    )
    return [DefaultInfo(files = depset([launcher]))]

cloudflare_launcher = rule(
    doc = "Expands the Rust launcher source for one Pages deployment.",
    implementation = _cloudflare_launcher_impl,
    attrs = {
        "account_id": attr.string(mandatory = True, doc = "Cloudflare account id."),
        "branch": attr.string(mandatory = True, doc = "Branch to deploy."),
        "deploy_name": attr.string(mandatory = True, doc = "Deploy target name."),
        "production_branch": attr.string(mandatory = True, doc = "Project production branch."),
        "project": attr.string(mandatory = True, doc = "Pages project name."),
        "site_ref": attr.string(mandatory = True, doc = "Site ref file name in this package."),
        "wrangler": attr.label(mandatory = True, doc = "Pinned Wrangler executable."),
    },
)

def cloudflare_pages_deploy(name, account_id, project, branch, production_branch, site, wrangler = None, profile = "release", tags = []):
    """Publishes a static bundle directory to an existing Cloudflare Pages project."""
    wrangler = wrangler or str(Label("//quality/tools/javascript/bin:wrangler"))
    error = cloudflare_pages_error(account_id, project, branch, production_branch)
    if error != "":
        fail(error + " (in " + native.package_name() + ":" + name + ")")

    check_target = name + "_check"
    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    site_ref_target = name + "_site_ref"

    native.genrule(
        name = check_target,
        srcs = [site],
        outs = [name + ".pages.json"],
        tags = tags,
        tools = [_VERIFY],
        cmd = "$(location " + str(_VERIFY) + ") " + " ".join([
            shell.quote(account_id),
            shell.quote(project),
            shell.quote(branch),
            shell.quote(production_branch),
            "$(execpath " + site + ")",
            "$@",
        ]),
    )

    cloudflare_site_ref(
        name = site_ref_target,
        site = site,
        tags = tags,
    )

    cloudflare_launcher(
        name = launcher_target,
        account_id = account_id,
        branch = branch,
        deploy_name = name,
        production_branch = production_branch,
        project = project,
        site_ref = site_ref_target + ".txt",
        tags = tags,
        wrangler = wrangler,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [wrangler],
        tags = tags,
        deps = [Label("//deploy/rules:dx_deploy_tools")],
    )

    dx_deployment(
        name = name,
        artifacts = [site],
        data = [":" + check_target, ":" + site_ref_target],
        deploy = ":" + program_target,
        profile = profile,
        tags = tags,
    )
