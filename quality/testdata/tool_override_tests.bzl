"""Analysis proofs for the declared ruff tool override."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//d/rules:defs.bzl", "D_COMPATIBLE_WITH")
load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load("//perl/rules:defs.bzl", "PERL_COMPATIBLE_WITH")
load("//quality:real_aspects.bzl", "real_format_aspect", "real_lint_aspect", "real_typecheck_aspect")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

_CANONICAL = "@@"  # buildifier: disable=canonical-repository

_TOOL_FLAG = "//config:tool_ruff"
_TOOL_SETTING = _CANONICAL + "//config:tool_ruff"
_ALTERNATE = _CANONICAL + "//quality/testdata:alternate_ruff"
_ALTERNATE_SHORT = "quality/testdata/alternate_ruff_bin"
_PAIR = _CANONICAL + "//quality/testdata:two_ruffs"
_EMPTY = _CANONICAL + "//quality/testdata:no_ruff"
_POLICY = _CANONICAL + "//quality:fixture_policy"
_QML_POLICY = _CANONICAL + "//quality/testdata:qml_policy"
_QML_LINT_POLICY = _CANONICAL + "//quality/testdata:qml_lint_policy"
_WORKSPACE_SETTING = _CANONICAL + "//config:workspace"
_ZIG_POLICY = _CANONICAL + "//quality/testdata:zig_policy"
_ZIG_FMT_SETTING = _CANONICAL + "//config:tool_zig_fmt"
_ZIG_FMT_TOOL = _CANONICAL + "//zig/tools:zig"
_D_POLICY = _CANONICAL + "//quality/testdata:d_policy"
_DFMT_SETTING = _CANONICAL + "//config:tool_dfmt"
_DFMT_TOOL = _CANONICAL + "//d/tools:dfmt"
_DSCANNER_SETTING = _CANONICAL + "//config:tool_dscanner"
_PERLCRITIC_SETTING = _CANONICAL + "//config:tool_perlcritic"
_PERLCRITIC_TOOL = _CANONICAL + "//perl/tools:perlcritic"
_PERL_LINT_POLICY = _CANONICAL + "//quality/testdata:perl_lint_policy"
_DSCANNER_TOOL = _CANONICAL + "//d/tools:dscanner"
_D_LINT_POLICY = _CANONICAL + "//quality/testdata:d_lint_policy"
_DART_POLICY = _CANONICAL + "//quality/testdata:dart_policy"
_DART_SETTING = _CANONICAL + "//config:tool_dart"
_DART_TOOL = _CANONICAL + "//dart/tools:dart"
_JULIA_POLICY = _CANONICAL + "//quality/testdata:julia_policy"
_JULIA_FORMAT_SETTING = _CANONICAL + "//config:tool_julia_format"
_JULIA_FORMAT_TOOL = _CANONICAL + "//julia/tools:julia_format"
_PERL_POLICY = _CANONICAL + "//quality/testdata:perl_policy"
_PERLTIDY_SETTING = _CANONICAL + "//config:tool_perltidy"
_PERLTIDY_TOOL = _CANONICAL + "//perl/tools:perltidy"
_ERLANG_POLICY = _CANONICAL + "//quality/testdata:erlang_policy"
_ERLFMT_SETTING = _CANONICAL + "//config:tool_erlfmt"
_ERLFMT_TOOL = _CANONICAL + "//erlang/tools:erlfmt"
_ERLANG_LINT_POLICY = _CANONICAL + "//quality/testdata:erlang_lint_policy"
_ELVIS_SETTING = _CANONICAL + "//config:tool_elvis"
_ELVIS_TOOL = _CANONICAL + "//erlang/tools:elvis"
_ERLANG_TYPECHECK_POLICY = _CANONICAL + "//quality/testdata:erlang_typecheck_policy"
_ELIXIR_TYPECHECK_POLICY = _CANONICAL + "//quality/testdata:elixir_typecheck_policy"
_DIALYZER_SETTING = _CANONICAL + "//config:tool_dialyzer"
_DIALYZER_TOOL = _CANONICAL + "//erlang/tools:dialyzer"
_PHP_LINT_POLICY = _CANONICAL + "//quality/testdata:php_lint_policy"
_PHP_TYPECHECK_POLICY = _CANONICAL + "//quality/testdata:php_typecheck_policy"
_PHPCS_SETTING = _CANONICAL + "//config:tool_phpcs"
_PHPCS_TOOL = _CANONICAL + "//php/tools:phpcs"
_PHPSTAN_SETTING = _CANONICAL + "//config:tool_phpstan"
_PHPSTAN_TOOL = _CANONICAL + "//php/tools:phpstan"
_ELIXIR_POLICY = _CANONICAL + "//quality/testdata:elixir_policy"
_MIX_FORMAT_SETTING = _CANONICAL + "//config:tool_mix_format"
_MIX_FORMAT_TOOL = _CANONICAL + "//elixir/tools:mix_format"
_ELIXIR_LINT_POLICY = _CANONICAL + "//quality/testdata:elixir_lint_policy"
_CREDO_SETTING = _CANONICAL + "//config:tool_credo"
_CREDO_TOOL = _CANONICAL + "//elixir/tools:credo"

def _tool_override_subject_impl(ctx):
    """Exposes the resolved ruff binary path beside the real aspect results."""
    fields = {
        "label": strip_canonical(str(ctx.attr.target.label)),
        "ruff": ctx.file._ruff.short_path,
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

tool_override_subject = rule(
    implementation = _tool_override_subject_impl,
    attrs = {
        "_ruff": attr.label(
            default = _TOOL_FLAG,
            allow_single_file = True,
        ),
        "target": attr.label(
            aspects = [real_lint_aspect, real_format_aspect],
            mandatory = True,
        ),
    },
)

def _qml_sources_impl(ctx):
    """Provides QML sources for the qmlformat flag proofs."""
    return [
        DefaultInfo(files = depset(ctx.files.srcs)),
        QualitySourcesInfo(direct_sources = {"qml": depset(ctx.files.srcs)}),
    ]

qml_sources = rule(
    implementation = _qml_sources_impl,
    attrs = {"srcs": attr.label_list(allow_files = [".qml"])},
)

def _qml_subject_impl(_ctx):
    """Applies the real format aspect to a QML target."""
    return [DefaultInfo(files = depset([]))]

qml_subject = rule(
    implementation = _qml_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = [real_format_aspect],
            mandatory = True,
        ),
    },
)

def _qml_lint_subject_impl(_ctx):
    """Applies the real lint aspect to a QML target."""
    return [DefaultInfo(files = depset([]))]

qml_lint_subject = rule(
    implementation = _qml_lint_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = [real_lint_aspect],
            mandatory = True,
        ),
    },
)

def _d_subject_impl(ctx):
    """Exposes the resolved dfmt binary and the format results beside the real aspect."""
    tool = ctx.attr._dfmt[DefaultInfo].files_to_run.executable
    wrapper = ctx.attr.target[QualityToolsInfo].tools["dfmt"].executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "dfmt": tool.short_path if tool else "",
        "results": ",".join([f.short_path for f in results]),
        "wrapper": wrapper.short_path if wrapper else "",
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

d_subject = rule(
    implementation = _d_subject_impl,
    attrs = {
        "_dfmt": attr.label(default = "//config:tool_dfmt"),
        "target": attr.label(
            aspects = [real_format_aspect],
            mandatory = True,
        ),
    },
)

def _perl_subject_impl(ctx):
    """Exposes the resolved perltidy binary and the format results beside the real aspect."""
    tool = ctx.attr._perltidy[DefaultInfo].files_to_run.executable
    wrapper = ctx.attr.target[QualityToolsInfo].tools["perltidy"].executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "perltidy": tool.short_path if tool else "",
        "results": ",".join([f.short_path for f in results]),
        "wrapper": wrapper.short_path if wrapper else "",
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

perl_subject = rule(
    implementation = _perl_subject_impl,
    attrs = {
        "_perltidy": attr.label(default = "//config:tool_perltidy"),
        "target": attr.label(
            aspects = [real_format_aspect],
            mandatory = True,
        ),
    },
)

def _zig_subject_impl(ctx):
    """Exposes the resolved zig_fmt binary and the format results beside the real aspect."""
    tool = ctx.attr._zig_fmt[DefaultInfo].files_to_run.executable
    wrapper = ctx.attr.target[QualityToolsInfo].tools["zig_fmt"].executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "results": ",".join([f.short_path for f in results]),
        "wrapper": wrapper.short_path if wrapper else "",
        "zig_fmt": tool.short_path if tool else "",
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

zig_subject = rule(
    implementation = _zig_subject_impl,
    attrs = {
        "_zig_fmt": attr.label(default = "//config:tool_zig_fmt"),
        "target": attr.label(
            aspects = [real_format_aspect],
            mandatory = True,
        ),
    },
)

def _dart_subject_impl(ctx):
    """Exposes the resolved dart binary and the format results beside the real aspect."""
    tool = ctx.attr._dart[DefaultInfo].files_to_run.executable
    wrapper = ctx.attr.target[QualityToolsInfo].tools["dart"].executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "results": ",".join([f.short_path for f in results]),
        "wrapper": wrapper.short_path if wrapper else "",
        "dart": tool.short_path if tool else "",
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

dart_subject = rule(
    implementation = _dart_subject_impl,
    attrs = {
        "_dart": attr.label(default = "//config:tool_dart"),
        "target": attr.label(
            aspects = [real_format_aspect],
            mandatory = True,
        ),
    },
)

def _julia_subject_impl(ctx):
    """Exposes the resolved julia_format binary and the format results beside the real aspect."""
    tool = ctx.attr._julia_format[DefaultInfo].files_to_run.executable
    wrapper = ctx.attr.target[QualityToolsInfo].tools["julia_format"].executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "results": ",".join([f.short_path for f in results]),
        "wrapper": wrapper.short_path if wrapper else "",
        "julia_format": tool.short_path if tool else "",
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

julia_subject = rule(
    implementation = _julia_subject_impl,
    attrs = {
        "_julia_format": attr.label(default = "//config:tool_julia_format"),
        "target": attr.label(
            aspects = [real_format_aspect],
            mandatory = True,
        ),
    },
)

def _erlang_subject_impl(ctx):
    """Exposes the resolved erlfmt binary and the format results beside the real aspect."""
    tool = ctx.attr._erlfmt[DefaultInfo].files_to_run.executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "erlfmt": tool.short_path if tool else "",
        "results": ",".join([f.short_path for f in results]),
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

erlang_subject = rule(
    implementation = _erlang_subject_impl,
    attrs = {
        "_erlfmt": attr.label(default = "//config:tool_erlfmt"),
        "target": attr.label(
            aspects = [real_format_aspect],
            mandatory = True,
        ),
    },
)

def _erlang_typecheck_subject_impl(ctx):
    """Exposes the dialyzer request files beside the real aspect."""
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = {"results": ",".join([f.short_path for f in results])})]

erlang_typecheck_subject = rule(
    implementation = _erlang_typecheck_subject_impl,
    attrs = {
        "_dialyzer": attr.label(default = "//config:tool_dialyzer"),
        "target": attr.label(
            aspects = [real_typecheck_aspect],
            mandatory = True,
        ),
    },
)

def _php_typecheck_subject_impl(ctx):
    """Exposes the phpstan request files beside the real aspect."""
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = {"results": ",".join([f.short_path for f in results])})]

php_typecheck_subject = rule(
    implementation = _php_typecheck_subject_impl,
    attrs = {
        "_phpstan": attr.label(default = "//config:tool_phpstan"),
        "target": attr.label(
            aspects = [real_typecheck_aspect],
            mandatory = True,
        ),
    },
)

def _php_lint_subject_impl(ctx):
    """Exposes the phpcs lint results beside the real aspect."""
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = {"results": ",".join([f.short_path for f in results])})]

php_lint_subject = rule(
    implementation = _php_lint_subject_impl,
    attrs = {
        "_phpcs": attr.label(default = "//config:tool_phpcs"),
        "target": attr.label(
            aspects = [real_lint_aspect],
            mandatory = True,
        ),
    },
)

def _erlang_lint_subject_impl(ctx):
    """Exposes the resolved elvis binary and the lint results beside the real aspect."""
    tool = ctx.attr._elvis[DefaultInfo].files_to_run.executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "elvis": tool.short_path if tool else "",
        "results": ",".join([f.short_path for f in results]),
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

erlang_lint_subject = rule(
    implementation = _erlang_lint_subject_impl,
    attrs = {
        "_elvis": attr.label(default = "//config:tool_elvis"),
        "target": attr.label(
            aspects = [real_lint_aspect],
            mandatory = True,
        ),
    },
)

def _elixir_subject_impl(ctx):
    """Exposes the resolved mix format launcher and the format results beside the real aspect."""
    tool = ctx.attr._mix_format[DefaultInfo].files_to_run.executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "mix_format": tool.short_path if tool else "",
        "results": ",".join([f.short_path for f in results]),
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

elixir_subject = rule(
    implementation = _elixir_subject_impl,
    attrs = {
        "_mix_format": attr.label(default = "//config:tool_mix_format"),
        "target": attr.label(
            aspects = [real_format_aspect],
            mandatory = True,
        ),
    },
)

def _subject_under_test(env):
    """Unwraps the possibly transitioned target under test."""
    dep = env.ctx.attr.target_under_test
    return dep[0] if type(dep) == "list" else dep

def _default_impl(ctx):
    """Asserts the managed ruff stays selected without the override flag."""
    env = analysistest.begin(ctx)
    ruff = _subject_under_test(env)[DxSubjectInfo].fields["ruff"]
    asserts.true(env, "alternate_ruff" not in ruff, "default ruff must stay managed, got " + ruff)
    asserts.true(env, ruff.split("/")[-1] in ["ruff", "ruff.exe"], "default ruff must be the managed binary, got " + ruff)
    return analysistest.end(env)

_tool_override_default_test = analysistest.make(_default_impl)

def _selected_impl(ctx):
    """Asserts the flag selects the declared alternate ruff binary."""
    env = analysistest.begin(ctx)
    ruff = _subject_under_test(env)[DxSubjectInfo].fields["ruff"]
    asserts.true(env, ruff == _ALTERNATE_SHORT, "want the declared alternate ruff, got " + ruff)
    return analysistest.end(env)

_tool_override_selected_test = analysistest.make(
    _selected_impl,
    config_settings = {_TOOL_SETTING: _ALTERNATE},
)

def _rejected_impl(ctx):
    """Asserts the expected override rejection diagnostic."""
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_tool_override_rejects_pair_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_TOOL_SETTING: _PAIR},
)

_tool_override_rejects_empty_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_TOOL_SETTING: _EMPTY},
)

_tool_override_rejects_policy_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_TOOL_SETTING: _POLICY},
)

_qmlformat_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _QML_POLICY},
)

_qmllint_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _QML_LINT_POLICY},
)

def _dfmt_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's dfmt."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["dfmt"])
    asserts.true(env, fields["wrapper"].endswith("/dfmt") or fields["wrapper"].endswith("/dfmt.exe"), "want dfmt, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the D target")
    return analysistest.end(env)

_dfmt_default_test = analysistest.make(
    _dfmt_default_impl,
    config_settings = {_WORKSPACE_SETTING: _D_POLICY},
)

def _dfmt_wired_impl(ctx):
    """Asserts the flag wires dfmt and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["dfmt"].endswith("/dfmt") or fields["dfmt"].endswith("/dfmt.exe"), "want dfmt, got " + fields["dfmt"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the D target")
    return analysistest.end(env)

_dfmt_set_test = analysistest.make(
    _dfmt_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _D_POLICY,
        _DFMT_SETTING: _DFMT_TOOL,
    },
)

def dfmt_flag_tests(name, subject):
    """Instantiates the dfmt flag proofs; D targets build on linux x86_64 only."""
    _dfmt_default_test(
        name = name + "_default_test",
        size = "small",
        target_compatible_with = D_COMPATIBLE_WITH,
        target_under_test = subject,
    )
    _dfmt_set_test(
        name = name + "_set_test",
        size = "small",
        target_compatible_with = D_COMPATIBLE_WITH,
        target_under_test = subject,
    )

def _d_lint_subject_impl(ctx):
    """Exposes the resolved dscanner binary and the lint results beside the real aspect."""
    tool = ctx.attr._dscanner[DefaultInfo].files_to_run.executable
    wrapper = ctx.attr.target[QualityToolsInfo].tools["dscanner"].executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "dscanner": tool.short_path if tool else "",
        "results": ",".join([f.short_path for f in results]),
        "wrapper": wrapper.short_path if wrapper else "",
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

d_lint_subject = rule(
    implementation = _d_lint_subject_impl,
    attrs = {
        "_dscanner": attr.label(default = "//config:tool_dscanner"),
        "target": attr.label(
            aspects = [real_lint_aspect],
            mandatory = True,
        ),
    },
)

def _dscanner_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's dscanner."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["dscanner"])
    asserts.true(env, fields["wrapper"].endswith("/dscanner") or fields["wrapper"].endswith("/dscanner.exe"), "want dscanner, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the D target")
    return analysistest.end(env)

_dscanner_default_test = analysistest.make(
    _dscanner_default_impl,
    config_settings = {_WORKSPACE_SETTING: _D_LINT_POLICY},
)

def _dscanner_wired_impl(ctx):
    """Asserts the flag wires dscanner and a lint stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["dscanner"].endswith("/dscanner") or fields["dscanner"].endswith("/dscanner.exe"), "want dscanner, got " + fields["dscanner"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the D target")
    return analysistest.end(env)

_dscanner_set_test = analysistest.make(
    _dscanner_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _D_LINT_POLICY,
        _DSCANNER_SETTING: _DSCANNER_TOOL,
    },
)

def dscanner_flag_tests(name, subject):
    """Instantiates the dscanner flag proofs; D targets build on linux x86_64 only."""
    _dscanner_default_test(
        name = name + "_default_test",
        size = "small",
        target_compatible_with = D_COMPATIBLE_WITH,
        target_under_test = subject,
    )
    _dscanner_set_test(
        name = name + "_set_test",
        size = "small",
        target_compatible_with = D_COMPATIBLE_WITH,
        target_under_test = subject,
    )

def _perl_lint_subject_impl(ctx):
    """Exposes the resolved perlcritic binary and the lint results beside the real aspect."""
    tool = ctx.attr._perlcritic[DefaultInfo].files_to_run.executable
    wrapper = ctx.attr.target[QualityToolsInfo].tools["perlcritic"].executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "perlcritic": tool.short_path if tool else "",
        "results": ",".join([f.short_path for f in results]),
        "wrapper": wrapper.short_path if wrapper else "",
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

perl_lint_subject = rule(
    implementation = _perl_lint_subject_impl,
    attrs = {
        "_perlcritic": attr.label(default = "//config:tool_perlcritic"),
        "target": attr.label(
            aspects = [real_lint_aspect],
            mandatory = True,
        ),
    },
)

def _perlcritic_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's perlcritic."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["perlcritic"])
    asserts.true(env, fields["wrapper"].endswith("/perlcritic") or fields["wrapper"].endswith("/perlcritic.exe"), "want perlcritic, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the D target")
    return analysistest.end(env)

_perlcritic_default_test = analysistest.make(
    _perlcritic_default_impl,
    config_settings = {_WORKSPACE_SETTING: _PERL_LINT_POLICY},
)

def _perlcritic_wired_impl(ctx):
    """Asserts the flag wires perlcritic and a lint stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["perlcritic"].endswith("/perlcritic") or fields["perlcritic"].endswith("/perlcritic.exe"), "want perlcritic, got " + fields["perlcritic"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the D target")
    return analysistest.end(env)

_perlcritic_set_test = analysistest.make(
    _perlcritic_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _PERL_LINT_POLICY,
        _PERLCRITIC_SETTING: _PERLCRITIC_TOOL,
    },
)

def perlcritic_flag_tests(name, subject):
    """Instantiates the perlcritic flag proofs; Perl targets build only on rules_perl platforms."""
    _perlcritic_default_test(
        name = name + "_default_test",
        size = "small",
        target_compatible_with = PERL_COMPATIBLE_WITH,
        target_under_test = subject,
    )
    _perlcritic_set_test(
        name = name + "_set_test",
        size = "small",
        target_compatible_with = PERL_COMPATIBLE_WITH,
        target_under_test = subject,
    )

def _perltidy_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's perltidy."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["perltidy"])
    asserts.true(env, fields["wrapper"].endswith("/Perl-Tidy_bin_perltidy") or fields["wrapper"].endswith("/Perl-Tidy_bin_perltidy.exe"), "want perltidy, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the Perl target")
    return analysistest.end(env)

_perltidy_default_test = analysistest.make(
    _perltidy_default_impl,
    config_settings = {_WORKSPACE_SETTING: _PERL_POLICY},
)

def _perltidy_wired_impl(ctx):
    """Asserts the flag wires perltidy and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["perltidy"].endswith("/perltidy") or fields["perltidy"].endswith("/perltidy.exe") or fields["perltidy"].endswith("/Perl-Tidy_bin_perltidy"), "want perltidy, got " + fields["perltidy"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the Perl target")
    return analysistest.end(env)

_perltidy_set_test = analysistest.make(
    _perltidy_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _PERL_POLICY,
        _PERLTIDY_SETTING: _PERLTIDY_TOOL,
    },
)

def perltidy_flag_tests(name, subject):
    """Instantiates the perltidy flag proofs; Perl targets build only on rules_perl platforms."""
    _perltidy_default_test(
        name = name + "_default_test",
        size = "small",
        target_compatible_with = PERL_COMPATIBLE_WITH,
        target_under_test = subject,
    )
    _perltidy_set_test(
        name = name + "_set_test",
        size = "small",
        target_compatible_with = PERL_COMPATIBLE_WITH,
        target_under_test = subject,
    )

def _zig_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's compiler."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["zig_fmt"])
    asserts.true(env, fields["wrapper"].endswith("/zig") or fields["wrapper"].endswith("/zig.exe"), "want the zig compiler, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the zig target")
    return analysistest.end(env)

_zig_fmt_default_test = analysistest.make(
    _zig_default_impl,
    config_settings = {_WORKSPACE_SETTING: _ZIG_POLICY},
)

def _zig_wired_impl(ctx):
    """Asserts the flag wires the Zig compiler and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["zig_fmt"].endswith("/zig") or fields["zig_fmt"].endswith("/zig.exe"), "want the zig compiler, got " + fields["zig_fmt"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the zig target")
    return analysistest.end(env)

_zig_fmt_set_test = analysistest.make(
    _zig_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _ZIG_POLICY,
        _ZIG_FMT_SETTING: _ZIG_FMT_TOOL,
    },
)

def zig_fmt_flag_tests(name, subject):
    """Instantiates the zig_fmt flag proofs."""
    _zig_fmt_default_test(
        name = name + "_default_test",
        size = "small",
        target_under_test = subject,
    )
    _zig_fmt_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

def _dart_default_impl(ctx):
    """Asserts an unset flag falls back to the dart binary the wrapper carries."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["dart"])
    asserts.equals(env, "dart", fields["wrapper"].split("/")[-1])
    asserts.true(env, fields["results"] != "", "want a format stage result for the dart target")
    return analysistest.end(env)

_dart_format_default_test = analysistest.make(
    _dart_default_impl,
    config_settings = {_WORKSPACE_SETTING: _DART_POLICY},
)

def _dart_wired_impl(ctx):
    """Asserts the flag wires the dart binary and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "dart", fields["dart"].split("/")[-1])
    asserts.true(env, fields["results"] != "", "want a format stage result for the dart target")
    return analysistest.end(env)

_dart_format_set_test = analysistest.make(
    _dart_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _DART_POLICY,
        _DART_SETTING: _DART_TOOL,
    },
)

def dart_format_flag_tests(name, subject):
    """Instantiates the dart flag proofs."""
    _dart_format_default_test(
        name = name + "_default_test",
        size = "small",
        target_under_test = subject,
    )
    _dart_format_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

def _julia_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's formatter."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["julia_format"])
    asserts.true(env, fields["wrapper"].split("/")[-1].startswith("julia_format"), "want the julia formatter, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the julia target")
    return analysistest.end(env)

_julia_format_default_test = analysistest.make(
    _julia_default_impl,
    config_settings = {_WORKSPACE_SETTING: _JULIA_POLICY},
)

def _julia_wired_impl(ctx):
    """Asserts the flag wires the Julia formatter and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["julia_format"].split("/")[-1].startswith("julia_format"), "want the julia formatter, got " + fields["julia_format"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the julia target")
    return analysistest.end(env)

_julia_format_set_test = analysistest.make(
    _julia_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _JULIA_POLICY,
        _JULIA_FORMAT_SETTING: _JULIA_FORMAT_TOOL,
    },
)

def julia_format_flag_tests(name, subject):
    """Instantiates the julia_format flag proofs."""
    _julia_format_default_test(
        name = name + "_default_test",
        size = "small",
        target_under_test = subject,
    )
    _julia_format_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

_erlfmt_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _ERLANG_POLICY},
)

def _erlfmt_wired_impl(ctx):
    """Asserts the flag wires the erlfmt escript and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["erlfmt"].endswith("/erlfmt"), "want the erlfmt escript, got " + fields["erlfmt"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the erlang target")
    return analysistest.end(env)

_erlfmt_set_test = analysistest.make(
    _erlfmt_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _ERLANG_POLICY,
        _ERLFMT_SETTING: _ERLFMT_TOOL,
    },
)

def erlfmt_flag_tests(name, subject):
    """Instantiates the erlfmt flag proofs."""
    _erlfmt_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_erlfmt",
    )
    _erlfmt_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

def _elixir_lint_subject_impl(ctx):
    """Exposes the resolved credo launcher and the lint results beside the real aspect."""
    tool = ctx.attr._credo[DefaultInfo].files_to_run.executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "credo": tool.short_path if tool else "",
        "results": ",".join([f.short_path for f in results]),
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

elixir_lint_subject = rule(
    implementation = _elixir_lint_subject_impl,
    attrs = {
        "_credo": attr.label(default = "//config:tool_credo"),
        "target": attr.label(
            aspects = [real_lint_aspect],
            mandatory = True,
        ),
    },
)

_credo_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _ELIXIR_LINT_POLICY},
)

def _credo_wired_impl(ctx):
    """Asserts the flag wires the credo launcher and a lint stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["credo"].endswith("/credo"), "want the credo launcher, got " + fields["credo"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the elixir target")
    return analysistest.end(env)

_credo_set_test = analysistest.make(
    _credo_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _ELIXIR_LINT_POLICY,
        _CREDO_SETTING: _CREDO_TOOL,
    },
)

def credo_flag_tests(name, subject):
    """Instantiates the credo flag proofs."""
    _credo_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_credo",
    )
    _credo_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

_elvis_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _ERLANG_LINT_POLICY},
)

def _elvis_wired_impl(ctx):
    """Asserts the flag wires the elvis escript and a lint stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["elvis"].endswith("/elvis"), "want the elvis escript, got " + fields["elvis"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the erlang target")
    return analysistest.end(env)

_elvis_set_test = analysistest.make(
    _elvis_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _ERLANG_LINT_POLICY,
        _ELVIS_SETTING: _ELVIS_TOOL,
    },
)

def elvis_flag_tests(name, subject):
    """Instantiates the elvis flag proofs."""
    _elvis_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_elvis",
    )
    _elvis_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

_mix_format_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _ELIXIR_POLICY},
)

def _mix_format_wired_impl(ctx):
    """Asserts the flag wires the mix format launcher and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["mix_format"].endswith("/mix_format"), "want the mix format launcher, got " + fields["mix_format"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the elixir target")
    return analysistest.end(env)

_mix_format_set_test = analysistest.make(
    _mix_format_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _ELIXIR_POLICY,
        _MIX_FORMAT_SETTING: _MIX_FORMAT_TOOL,
    },
)

def mix_format_flag_tests(name, subject):
    """Instantiates the mix format flag proofs."""
    _mix_format_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_mix_format",
    )
    _mix_format_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

def qmllint_flag_tests(name, subject):
    """Instantiates the missing qmllint flag proof."""
    _qmllint_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_qmllint",
    )

def qmlformat_flag_tests(name, subject):
    """Instantiates the missing qmlformat flag proof."""
    _qmlformat_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_qmlformat",
    )

def tool_override_tests(name, subject):
    """Instantiates the ruff override selection and rejection proofs."""
    _tool_override_default_test(
        name = name + "_default_test",
        size = "small",
        target_under_test = subject,
    )
    _tool_override_selected_test(
        name = name + "_selected_test",
        size = "small",
        target_under_test = subject,
    )
    _tool_override_rejects_pair_test(
        name = name + "_rejects_pair_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "two_ruffs' must produce a single file",
    )
    _tool_override_rejects_empty_test(
        name = name + "_rejects_empty_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "no_ruff' must produce a single file",
    )
    _tool_override_rejects_policy_test(
        name = name + "_rejects_policy_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "fixture_policy' must produce a single file",
    )

_dialyzer_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _ERLANG_TYPECHECK_POLICY},
)

def _dialyzer_wired_impl(ctx):
    """Asserts the flag wires a typecheck stage result for the erlang target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a typecheck stage result for the erlang target")
    return analysistest.end(env)

_dialyzer_set_test = analysistest.make(
    _dialyzer_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _ERLANG_TYPECHECK_POLICY,
        _DIALYZER_SETTING: _DIALYZER_TOOL,
    },
)

def dialyzer_flag_tests(name, subject):
    """Instantiates the dialyzer flag proofs."""
    _dialyzer_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_dialyzer",
    )
    _dialyzer_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

_dialyzer_elixir_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _ELIXIR_TYPECHECK_POLICY},
)

_dialyzer_elixir_set_test = analysistest.make(
    _dialyzer_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _ELIXIR_TYPECHECK_POLICY,
        _DIALYZER_SETTING: _DIALYZER_TOOL,
    },
)

def dialyzer_elixir_flag_tests(name, subject):
    """Instantiates the dialyzer flag proofs for an elixir_app."""
    _dialyzer_elixir_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_dialyzer",
    )
    _dialyzer_elixir_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

_phpcs_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _PHP_LINT_POLICY},
)

def _phpcs_wired_impl(ctx):
    """Asserts the flag wires a lint stage result for the php target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a lint stage result for the php target")
    return analysistest.end(env)

_phpcs_set_test = analysistest.make(
    _phpcs_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _PHP_LINT_POLICY,
        _PHPCS_SETTING: _PHPCS_TOOL,
    },
)

def phpcs_flag_tests(name, subject):
    """Instantiates the phpcs flag proofs."""
    _phpcs_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_phpcs",
    )
    _phpcs_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )

_phpstan_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
    config_settings = {_WORKSPACE_SETTING: _PHP_TYPECHECK_POLICY},
)

def _phpstan_wired_impl(ctx):
    """Asserts the flag wires a typecheck stage result for the php target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a typecheck stage result for the php target")
    return analysistest.end(env)

_phpstan_set_test = analysistest.make(
    _phpstan_wired_impl,
    config_settings = {
        _WORKSPACE_SETTING: _PHP_TYPECHECK_POLICY,
        _PHPSTAN_SETTING: _PHPSTAN_TOOL,
    },
)

def phpstan_flag_tests(name, subject):
    """Instantiates the phpstan flag proofs."""
    _phpstan_unset_test(
        name = name + "_unset_test",
        size = "small",
        target_under_test = subject,
        expected_failure_substring = "--@rules_dx//config:tool_phpstan",
    )
    _phpstan_set_test(
        name = name + "_set_test",
        size = "small",
        target_under_test = subject,
    )
