"""Unit and analysis tests for the Cloudflare Pages publisher."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":cloudflare.bzl", "cloudflare_pages_error", "cloudflare_pages_intent")

_ACCOUNT = "0123456789abcdef0123456789abcdef"

def _site_dir_impl(ctx):
    out = ctx.actions.declare_directory(ctx.label.name)
    commands = ["mkdir -p " + out.path]
    for src in ctx.files.srcs:
        rel = src.short_path.removeprefix(ctx.attr.strip + "/")
        commands.append("mkdir -p $(dirname {out}/{rel}) && cp {src} {out}/{rel}".format(out = out.path, rel = rel, src = src.path))
    ctx.actions.run_shell(
        outputs = [out],
        inputs = ctx.files.srcs,
        command = "\n".join(commands),
    )
    return [DefaultInfo(files = depset([out]))]

site_dir = rule(
    implementation = _site_dir_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "strip": attr.string(doc = "Package-relative prefix removed from each source."),
    },
)

def cloudflare_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "cloudflare_pages_error accepts a valid target",
                cloudflare_pages_error(_ACCOUNT, "my-site", "feature/x", "main"),
                "",
            ),
            expect_equal(
                "cloudflare_pages_error rejects a short account id",
                cloudflare_pages_error("abc", "my-site", "main", "main"),
                "cloudflare_pages_deploy: invalid account_id 'abc': want 32 lowercase hex characters",
            ),
            expect_equal(
                "cloudflare_pages_error rejects an uppercase project",
                cloudflare_pages_error(_ACCOUNT, "My-Site", "main", "main"),
                "cloudflare_pages_deploy: invalid project 'My-Site': want 1-58 characters of [a-z0-9-] that start and end with a letter or digit",
            ),
            expect_equal(
                "cloudflare_pages_error rejects an empty branch",
                cloudflare_pages_error(_ACCOUNT, "my-site", "", "main"),
                "cloudflare_pages_deploy: invalid branch '': want 1-100 characters of [A-Za-z0-9._/-]",
            ),
            expect_equal(
                "cloudflare_pages_error rejects a flag-like branch",
                cloudflare_pages_error(_ACCOUNT, "my-site", "--evil", "main"),
                "cloudflare_pages_deploy: invalid branch '--evil': no leading '-', no '..', no empty path parts",
            ),
            expect_equal(
                "cloudflare_pages_error rejects a shell-unsafe production branch",
                cloudflare_pages_error(_ACCOUNT, "my-site", "main", "m;x"),
                "cloudflare_pages_deploy: invalid production_branch 'm;x': want 1-100 characters of [A-Za-z0-9._/-]",
            ),
            expect_equal(
                "cloudflare_pages_intent names the production branch",
                cloudflare_pages_intent("main", "main"),
                "production",
            ),
            expect_equal(
                "cloudflare_pages_intent names any other branch preview",
                cloudflare_pages_intent("dev", "main"),
                "preview",
            ),
        ],
    )

_ClosureInfo = provider(
    doc = "Labels one target reaches through its attributes.",
    fields = {"labels": "Depset of reached labels."},
)

def _closure_aspect_impl(target, ctx):
    """Collects the labels a target reaches."""
    reached = []
    for attr_name in dir(ctx.rule.attr):
        value = getattr(ctx.rule.attr, attr_name)
        if type(value) == "dict":
            value = value.keys() + value.values()
        elif type(value) != "list":
            value = [value]
        for dep in value:
            if type(dep) == "Target" and _ClosureInfo in dep:
                reached.append(dep[_ClosureInfo].labels)
    return [_ClosureInfo(labels = depset([target.label], transitive = reached))]

_closure_aspect = aspect(
    implementation = _closure_aspect_impl,
    attr_aspects = ["*"],
)

def _program_closure_test_impl(ctx):
    env = analysistest.begin(ctx)
    labels = analysistest.target_under_test(env)[_ClosureInfo].labels.to_list()
    asserts.true(
        env,
        Label("//deploy/rules:dx_deploy_tools") in labels,
        "deploy tool closure misses //deploy/rules:dx_deploy_tools",
    )
    asserts.false(
        env,
        Label("//deploy/rules:cloudflare_demo_site") in labels,
        "deploy tool closure reaches //deploy/rules:cloudflare_demo_site",
    )
    return analysistest.end(env)

_program_closure_test = analysistest.make(
    _program_closure_test_impl,
    extra_target_under_test_aspects = [_closure_aspect],
)

def _write_content(env):
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite"]
    asserts.equals(env, 1, len(writes), "want one FileWrite action")
    return writes[0].content if writes else ""

def _config_content_test_impl(ctx):
    env = analysistest.begin(ctx)
    content = _write_content(env)
    asserts.true(
        env,
        "\"site_rloc\":\"_main/deploy/rules/cloudflare_demo_site\"" in content,
        "config must name the site rlocation: " + content,
    )
    asserts.true(env, "\"kind\":\"cloudflare_pages\"" in content, "config must name its kind: " + content)
    return analysistest.end(env)

_config_content_test = analysistest.make(_config_content_test_impl)

def cloudflare_program_closure_tests(name):
    """Checks that the deploy tool never builds the site and the config names it."""
    _program_closure_test(
        name = name + "_deps",
        size = "small",
        target_under_test = ":deploy_tool",
    )
    _config_content_test(
        name = name + "_config",
        size = "small",
        target_under_test = ":cloudflare_demo_config",
    )
    native.test_suite(
        name = name,
        tests = [name + "_deps", name + "_config"],
    )
