"""Typed native-configuration targets for adapters."""

load(
    "//core/quality/private:native_config_impl.bzl",
    _DxNativeConfigInfo = "DxNativeConfigInfo",
    "native_config_error",
)

DxNativeConfigInfo = _DxNativeConfigInfo

def native_config_impl(ctx):
    tool_id = ctx.attr._tool_id
    config = ctx.file.src
    config_path = config.path if config else ""
    config_is_source = config.is_source if config else False
    data = [struct(path = f.path, is_source = f.is_source) for f in ctx.files.data]
    err = native_config_error(tool_id, config_path, config_is_source, data, ctx.attr.allow_generated)
    if err != "":
        fail(err + " (in " + str(ctx.label) + ")")
    closure = depset([config] + ctx.files.data)
    return [
        DefaultInfo(files = closure),
        DxNativeConfigInfo(
            closure = closure,
            config = config,
            tool_id = tool_id,
        ),
    ]

def _make_native_config_rule(tool_id):
    return rule(
        implementation = native_config_impl,
        doc = "Native config file for " + tool_id + ".",
        attrs = {
            "allow_generated": attr.bool(
                doc = "Accepts generated config and data files, not only checked-in sources.",
                default = False,
            ),
            "data": attr.label_list(
                doc = "Extra files the tool reads alongside its config.",
                allow_files = True,
                default = [],
            ),
            "src": attr.label(
                doc = "Config file the tool reads, normally a checked-in source.",
                allow_single_file = True,
                mandatory = True,
            ),
            "_tool_id": attr.string(
                default = tool_id,
            ),
        },
    )

buildifier_config = _make_native_config_rule("buildifier")

stylua_config = _make_native_config_rule("stylua")

selene_config = _make_native_config_rule("selene")

taplo_config = _make_native_config_rule("taplo")

latexindent_config = _make_native_config_rule("latexindent")

vale_config = _make_native_config_rule("vale")

rustfmt_config = _make_native_config_rule("rustfmt")

ruff_config = _make_native_config_rule("ruff")

ty_config = _make_native_config_rule("ty")

biome_config = _make_native_config_rule("biome")

eslint_config = _make_native_config_rule("eslint")

scalafmt_config = _make_native_config_rule("scalafmt")

scalafix_config = _make_native_config_rule("scalafix")

sqlfluff_config = _make_native_config_rule("sqlfluff")

csharpier_config = _make_native_config_rule("csharpier")

helmfmt_config = _make_native_config_rule("helmfmt")

clang_format_config = _make_native_config_rule("clang_format")

clang_tidy_config = _make_native_config_rule("clang_tidy")

roslyn_config = _make_native_config_rule("roslyn")

staticcheck_config = _make_native_config_rule("staticcheck")

errcheck_config = _make_native_config_rule("errcheck")

govet_config = _make_native_config_rule("govet")

fsharplint_config = _make_native_config_rule("fsharplint")


pmd_config = _make_native_config_rule("pmd")

spotbugs_config = _make_native_config_rule("spotbugs")

buf_config = _make_native_config_rule("buf")

qmlformat_config = _make_native_config_rule("qmlformat")

qmllint_config = _make_native_config_rule("qmllint")

prettier_config = _make_native_config_rule("prettier")

stylelint_config = _make_native_config_rule("stylelint")

gplint_config = _make_native_config_rule("gplint")

graphql_eslint_config = _make_native_config_rule("graphql_eslint")

redocly_config = _make_native_config_rule("redocly")

markdown_check_config = _make_native_config_rule("markdown_check")
remark_config = _make_native_config_rule("remark")

psscriptanalyzer_config = _make_native_config_rule("psscriptanalyzer")

djlint_config = _make_native_config_rule("djlint")

yamlfmt_config = _make_native_config_rule("yamlfmt")

shellcheck_config = _make_native_config_rule("shellcheck")

yamllint_config = _make_native_config_rule("yamllint")

ansible_lint_config = _make_native_config_rule("ansible_lint")

bicep_config = _make_native_config_rule("bicep")

cfn_lint_config = _make_native_config_rule("cfn_lint")

hadolint_config = _make_native_config_rule("hadolint")
hlint_config = _make_native_config_rule("hlint")
regal_config = _make_native_config_rule("regal")
checkmake_config = _make_native_config_rule("checkmake")
gersemi_config = _make_native_config_rule("gersemi")
cmakelint_config = _make_native_config_rule("cmakelint")
ffmt_config = _make_native_config_rule("ffmt")
docstrfmt_config = _make_native_config_rule("docstrfmt")
mbake_config = _make_native_config_rule("mbake")
tflint_config = _make_native_config_rule("tflint")
trivy_config_config = _make_native_config_rule("trivy_config")
zizmor_config = _make_native_config_rule("zizmor")
fortitude_config = _make_native_config_rule("fortitude")
swift_format_config = _make_native_config_rule("swift_format")
swift_lint_config = _make_native_config_rule("swift_lint")
credo_config = _make_native_config_rule("credo")
detekt_config = _make_native_config_rule("detekt")

elvis_config = _make_native_config_rule("elvis")

luals_config = _make_native_config_rule("luals")

perlcritic_config = _make_native_config_rule("perlcritic")

perltidy_config = _make_native_config_rule("perltidy")

shfmt_config = _make_native_config_rule("shfmt")

mix_format_config = _make_native_config_rule("mix_format")

lychee_config = _make_native_config_rule("lychee")

typos_config = _make_native_config_rule("typos")

pydoclint_config = _make_native_config_rule("pydoclint")

phpcs_config = _make_native_config_rule("phpcs")

phpstan_config = _make_native_config_rule("phpstan")

rubocop_config = _make_native_config_rule("rubocop")


