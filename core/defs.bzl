"""Public rules_dx rules that belong to no single language: source libraries, native tool configs, the generated policy and docs sites."""

load(
    "//core/quality:native_config.bzl",
    _DxNativeConfigInfo = "DxNativeConfigInfo",
    _ansible_lint_config = "ansible_lint_config",
    _bicep_config = "bicep_config",
    _biome_config = "biome_config",
    _buf_config = "buf_config",
    _buildifier_config = "buildifier_config",
    _cfn_lint_config = "cfn_lint_config",
    _checkmake_config = "checkmake_config",
    _clang_format_config = "clang_format_config",
    _clang_tidy_config = "clang_tidy_config",
    _cmakelint_config = "cmakelint_config",
    _credo_config = "credo_config",
    _csharpier_config = "csharpier_config",
    _dart_config = "dart_config",
    _detekt_config = "detekt_config",
    _djlint_config = "djlint_config",
    _docstrfmt_config = "docstrfmt_config",
    _elvis_config = "elvis_config",
    _errcheck_config = "errcheck_config",
    _eslint_config = "eslint_config",
    _ffmt_config = "ffmt_config",
    _fortitude_config = "fortitude_config",
    _fsharplint_config = "fsharplint_config",
    _gersemi_config = "gersemi_config",
    _govet_config = "govet_config",
    _gplint_config = "gplint_config",
    _graphql_eslint_config = "graphql_eslint_config",
    _hadolint_config = "hadolint_config",
    _helmfmt_config = "helmfmt_config",
    _hlint_config = "hlint_config",
    _julialint_config = "julialint_config",
    _latexindent_config = "latexindent_config",
    _luals_config = "luals_config",
    _lychee_config = "lychee_config",
    _markdown_check_config = "markdown_check_config",
    _mbake_config = "mbake_config",
    _mix_format_config = "mix_format_config",
    _perlcritic_config = "perlcritic_config",
    _perltidy_config = "perltidy_config",
    _phpcs_config = "phpcs_config",
    _phpstan_config = "phpstan_config",
    _pmd_config = "pmd_config",
    _prettier_config = "prettier_config",
    _psscriptanalyzer_config = "psscriptanalyzer_config",
    _pydoclint_config = "pydoclint_config",
    _qmlformat_config = "qmlformat_config",
    _qmllint_config = "qmllint_config",
    _redocly_config = "redocly_config",
    _regal_config = "regal_config",
    _remark_config = "remark_config",
    _roslyn_config = "roslyn_config",
    _rubocop_config = "rubocop_config",
    _ruff_config = "ruff_config",
    _rustfmt_config = "rustfmt_config",
    _scalafix_config = "scalafix_config",
    _scalafmt_config = "scalafmt_config",
    _selene_config = "selene_config",
    _shellcheck_config = "shellcheck_config",
    _shfmt_config = "shfmt_config",
    _spotbugs_config = "spotbugs_config",
    _sqlfluff_config = "sqlfluff_config",
    _staticcheck_config = "staticcheck_config",
    _stylelint_config = "stylelint_config",
    _stylua_config = "stylua_config",
    _swift_format_config = "swift_format_config",
    _swift_lint_config = "swift_lint_config",
    _taplo_config = "taplo_config",
    _tflint_config = "tflint_config",
    _trivy_config_config = "trivy_config_config",
    _ty_config = "ty_config",
    _typos_config = "typos_config",
    _vale_config = "vale_config",
    _yamlfmt_config = "yamlfmt_config",
    _yamllint_config = "yamllint_config",
    _zizmor_config = "zizmor_config",
)
load(
    "//core/quality:source_libraries.bzl",
    _cue_library = "cue_library",
    _gherkin_library = "gherkin_library",
    _html_template_library = "html_template_library",
    _jsonnet_library = "jsonnet_library",
    _jupyter_library = "jupyter_library",
    _latex_library = "latex_library",
    _nix_library = "nix_library",
    _pkl_library = "pkl_library",
    _typst_library = "typst_library",
    _xml_library = "xml_library",
    _yaml_library = "yaml_library",
)
load(
    "//core/quality/private:generated_policy.bzl",
    _dx_generated_policy = "dx_generated_policy",
)
load(
    "//tools/docs/lua:defs.bzl",
    _lua_doc_shard = "lua_doc_shard",
)
load(
    "//tools/docs/php:defs.bzl",
    _php_doc_shard = "php_doc_shard",
)
load(
    "//tools/docs/site:site.bzl",
    _docs_site = "docs_site",
)

DxNativeConfigInfo = _DxNativeConfigInfo
ansible_lint_config = _ansible_lint_config
bicep_config = _bicep_config
biome_config = _biome_config
buf_config = _buf_config
buildifier_config = _buildifier_config
cfn_lint_config = _cfn_lint_config
checkmake_config = _checkmake_config
clang_format_config = _clang_format_config
clang_tidy_config = _clang_tidy_config
cmakelint_config = _cmakelint_config
credo_config = _credo_config
csharpier_config = _csharpier_config
cue_library = _cue_library
dart_config = _dart_config
detekt_config = _detekt_config
djlint_config = _djlint_config
docs_site = _docs_site
docstrfmt_config = _docstrfmt_config
dx_generated_policy = _dx_generated_policy
elvis_config = _elvis_config
errcheck_config = _errcheck_config
eslint_config = _eslint_config
ffmt_config = _ffmt_config
fortitude_config = _fortitude_config
fsharplint_config = _fsharplint_config
gersemi_config = _gersemi_config
gherkin_library = _gherkin_library
govet_config = _govet_config
gplint_config = _gplint_config
graphql_eslint_config = _graphql_eslint_config
hadolint_config = _hadolint_config
helmfmt_config = _helmfmt_config
hlint_config = _hlint_config
html_template_library = _html_template_library
jsonnet_library = _jsonnet_library
julialint_config = _julialint_config
jupyter_library = _jupyter_library
latex_library = _latex_library
latexindent_config = _latexindent_config
lua_doc_shard = _lua_doc_shard
luals_config = _luals_config
lychee_config = _lychee_config
markdown_check_config = _markdown_check_config
mbake_config = _mbake_config
mix_format_config = _mix_format_config
nix_library = _nix_library
perlcritic_config = _perlcritic_config
perltidy_config = _perltidy_config
php_doc_shard = _php_doc_shard
phpcs_config = _phpcs_config
phpstan_config = _phpstan_config
pkl_library = _pkl_library
pmd_config = _pmd_config
prettier_config = _prettier_config
psscriptanalyzer_config = _psscriptanalyzer_config
pydoclint_config = _pydoclint_config
qmlformat_config = _qmlformat_config
qmllint_config = _qmllint_config
redocly_config = _redocly_config
regal_config = _regal_config
remark_config = _remark_config
roslyn_config = _roslyn_config
rubocop_config = _rubocop_config
ruff_config = _ruff_config
rustfmt_config = _rustfmt_config
scalafix_config = _scalafix_config
scalafmt_config = _scalafmt_config
selene_config = _selene_config
shellcheck_config = _shellcheck_config
shfmt_config = _shfmt_config
spotbugs_config = _spotbugs_config
sqlfluff_config = _sqlfluff_config
staticcheck_config = _staticcheck_config
stylelint_config = _stylelint_config
stylua_config = _stylua_config
swift_format_config = _swift_format_config
swift_lint_config = _swift_lint_config
taplo_config = _taplo_config
tflint_config = _tflint_config
trivy_config_config = _trivy_config_config
ty_config = _ty_config
typos_config = _typos_config
typst_library = _typst_library
vale_config = _vale_config
xml_library = _xml_library
yaml_library = _yaml_library
yamlfmt_config = _yamlfmt_config
yamllint_config = _yamllint_config
zizmor_config = _zizmor_config
