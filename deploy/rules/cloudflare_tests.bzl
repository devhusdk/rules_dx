"""Unit and analysis tests for the Cloudflare Pages publisher."""

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

EXPECTED_CLOUDFLARE_DEFAULT_OBSERVATIONS = """subject //deploy/rules:cloudflare_demo
file cloudflare_demo
field app=
field artifacts=//deploy/rules:cloudflare_demo_site
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:cloudflare_demo
aspect_field transitive_count=0"""

def cloudflare_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":cloudflare_demo"],
        expected_observations = EXPECTED_CLOUDFLARE_DEFAULT_OBSERVATIONS,
    )
