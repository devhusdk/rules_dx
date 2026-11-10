"""Analysis proofs for the declared ruff tool override."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:canonical.bzl", "strip_canonical")
load("//core:defs.bzl", "DxSubjectInfo")
load("//perl:defs.bzl", "PERL_COMPATIBLE_WITH")
load("//dart/private:dart_config.bzl", "DxDartConfigInfo")
load(
    "//core/quality/testdata:all_languages.bzl",
    "real_cc_format_aspect",
    "real_cc_lint_aspect",
    "real_dart_format_aspect",
    "real_elixir_format_aspect",
    "real_elixir_lint_aspect",
    "real_erlang_format_aspect",
    "real_erlang_lint_aspect",
    "real_erlang_typecheck_aspect",
    "real_julia_format_aspect",
    "real_julia_typecheck_aspect",
    "real_lua_typecheck_aspect",
    "real_php_lint_aspect",
    "real_perl_format_aspect",
    "real_perl_lint_aspect",
    "real_php_typecheck_aspect",
    "real_python_format_aspect",
    "real_python_lint_aspect",
    "real_ruby_typecheck_aspect",
    "real_typecheck_aspect",
    "real_zig_format_aspect",
    "real_zig_lint_aspect",
)
load("//core/quality/testdata:config_subject.bzl", "config_subject_rule")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")

_TOOL_FLAG = "//config:tool_ruff"
_ALTERNATE_SHORT = "core/quality/testdata/alternate_ruff_bin"
TOOL_OVERRIDE_CONFIGS = ["ruff_alternate", "ruff_empty", "ruff_pair", "ruff_policy", "tool_defaults", "tool_overrides"]

_override_subject = config_subject_rule([], [DxSubjectInfo])

def _subject(name, target, config, target_compatible_with = None):
    """Declares a subject that builds target under config and returns its label."""
    _override_subject(
        name = name,
        config = config,
        tags = ["manual"],
        target = target,
        target_compatible_with = target_compatible_with,
        testonly = True,
    )
    return ":" + name

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
            aspects = [real_python_lint_aspect, real_python_format_aspect],
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
            aspects = [real_cc_format_aspect],
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
            aspects = [real_cc_lint_aspect],
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
            aspects = [real_perl_format_aspect],
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
            aspects = [real_zig_format_aspect],
            mandatory = True,
        ),
    },
)

def _zig_lint_subject_impl(ctx):
    """Exposes the resolved zig_ast_check binary and the lint results beside the real aspect."""
    tool = ctx.attr._zig_ast_check[DefaultInfo].files_to_run.executable
    wrapper = ctx.attr.target[QualityToolsInfo].tools["zig_ast_check"].executable
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    fields = {
        "results": ",".join([f.short_path for f in results]),
        "wrapper": wrapper.short_path if wrapper else "",
        "zig_ast_check": tool.short_path if tool else "",
    }
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = fields)]

zig_lint_subject = rule(
    implementation = _zig_lint_subject_impl,
    attrs = {
        "_zig_ast_check": attr.label(default = "//config:tool_zig_ast_check"),
        "target": attr.label(
            aspects = [real_zig_lint_aspect],
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
            aspects = [real_dart_format_aspect],
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
            aspects = [real_julia_format_aspect],
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
            aspects = [real_erlang_format_aspect],
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
            aspects = [real_erlang_typecheck_aspect],
            mandatory = True,
        ),
    },
)

def _julia_typecheck_subject_impl(ctx):
    """Exposes the jet request files beside the real aspect."""
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = {"results": ",".join([f.short_path for f in results])})]

julia_typecheck_subject = rule(
    implementation = _julia_typecheck_subject_impl,
    attrs = {
        "_jet": attr.label(default = "//config:tool_jet"),
        "target": attr.label(
            aspects = [real_julia_typecheck_aspect],
            mandatory = True,
        ),
    },
)

def _lua_typecheck_subject_impl(ctx):
    """Exposes the luals request files beside the real aspect."""
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = {"results": ",".join([f.short_path for f in results])})]

lua_typecheck_subject = rule(
    implementation = _lua_typecheck_subject_impl,
    attrs = {
        "_luals": attr.label(default = "//config:tool_luals"),
        "target": attr.label(
            aspects = [real_lua_typecheck_aspect],
            mandatory = True,
        ),
    },
)

def _ruby_typecheck_subject_impl(ctx):
    """Exposes the sorbet request files beside the real aspect."""
    results = getattr(ctx.attr.target[OutputGroupInfo], "dx_results", depset()).to_list()
    return [DefaultInfo(files = depset([])), DxSubjectInfo(fields = {"results": ",".join([f.short_path for f in results])})]

ruby_typecheck_subject = rule(
    implementation = _ruby_typecheck_subject_impl,
    attrs = {
        "_sorbet": attr.label(default = "//config:tool_sorbet"),
        "target": attr.label(
            aspects = [real_ruby_typecheck_aspect],
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
            aspects = [real_php_typecheck_aspect],
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
            aspects = [real_php_lint_aspect],
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
            aspects = [real_erlang_lint_aspect],
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
            aspects = [real_elixir_format_aspect],
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

_tool_override_selected_test = analysistest.make(_selected_impl)

def _rejected_impl(ctx):
    """Asserts the expected override rejection diagnostic."""
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected_failure_substring)
    return analysistest.end(env)

_tool_override_rejects_pair_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
)

_tool_override_rejects_empty_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
)

_tool_override_rejects_policy_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
)

_qmlformat_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
)

_qmllint_unset_test = analysistest.make(
    _rejected_impl,
    expect_failure = True,
    attrs = {"expected_failure_substring": attr.string(mandatory = True)},
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
            aspects = [real_perl_lint_aspect],
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

_perlcritic_default_test = analysistest.make(_perlcritic_default_impl)

def _perlcritic_wired_impl(ctx):
    """Asserts the flag wires perlcritic and a lint stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["perlcritic"].endswith("/perlcritic") or fields["perlcritic"].endswith("/perlcritic.exe"), "want perlcritic, got " + fields["perlcritic"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the D target")
    return analysistest.end(env)

_perlcritic_set_test = analysistest.make(_perlcritic_wired_impl)

def perlcritic_flag_tests(name, subject):
    """Instantiates the perlcritic flag proofs; Perl targets build only on rules_perl platforms."""
    _perlcritic_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_compatible_with = PERL_COMPATIBLE_WITH,
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults", PERL_COMPATIBLE_WITH),
    )
    _perlcritic_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_compatible_with = PERL_COMPATIBLE_WITH,
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides", PERL_COMPATIBLE_WITH),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _perltidy_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's perltidy."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["perltidy"])
    asserts.true(env, fields["wrapper"].endswith("/Perl-Tidy_bin_perltidy") or fields["wrapper"].endswith("/Perl-Tidy_bin_perltidy.exe"), "want perltidy, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the Perl target")
    return analysistest.end(env)

_perltidy_default_test = analysistest.make(_perltidy_default_impl)

def _perltidy_wired_impl(ctx):
    """Asserts the flag wires perltidy and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["perltidy"].endswith("/perltidy") or fields["perltidy"].endswith("/perltidy.exe") or fields["perltidy"].endswith("/Perl-Tidy_bin_perltidy"), "want perltidy, got " + fields["perltidy"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the Perl target")
    return analysistest.end(env)

_perltidy_set_test = analysistest.make(_perltidy_wired_impl)

def perltidy_flag_tests(name, subject):
    """Instantiates the perltidy flag proofs; Perl targets build only on rules_perl platforms."""
    _perltidy_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_compatible_with = PERL_COMPATIBLE_WITH,
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults", PERL_COMPATIBLE_WITH),
    )
    _perltidy_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_compatible_with = PERL_COMPATIBLE_WITH,
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides", PERL_COMPATIBLE_WITH),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _zig_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's compiler."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["zig_fmt"])
    asserts.true(env, fields["wrapper"].endswith("/zig") or fields["wrapper"].endswith("/zig.exe"), "want the zig compiler, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the zig target")
    return analysistest.end(env)

_zig_fmt_default_test = analysistest.make(_zig_default_impl)

def _zig_wired_impl(ctx):
    """Asserts the flag wires the Zig compiler and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["zig_fmt"].endswith("/zig") or fields["zig_fmt"].endswith("/zig.exe"), "want the zig compiler, got " + fields["zig_fmt"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the zig target")
    return analysistest.end(env)

_zig_fmt_set_test = analysistest.make(_zig_wired_impl)

def zig_fmt_flag_tests(name, subject):
    """Instantiates the zig_fmt flag proofs."""
    _zig_fmt_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _zig_fmt_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _zig_ast_check_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's compiler."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["zig_ast_check"])
    asserts.true(env, fields["wrapper"].endswith("/zig") or fields["wrapper"].endswith("/zig.exe"), "want the zig compiler, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the zig target")
    return analysistest.end(env)

_zig_ast_check_default_test = analysistest.make(_zig_ast_check_default_impl)

def _zig_ast_check_wired_impl(ctx):
    """Asserts the flag wires the Zig compiler and a lint stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["zig_ast_check"].endswith("/zig") or fields["zig_ast_check"].endswith("/zig.exe"), "want the zig compiler, got " + fields["zig_ast_check"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the zig target")
    return analysistest.end(env)

_zig_ast_check_set_test = analysistest.make(_zig_ast_check_wired_impl)

def zig_ast_check_flag_tests(name, subject):
    """Instantiates the zig_ast_check flag proofs."""
    _zig_ast_check_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _zig_ast_check_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _dart_default_impl(ctx):
    """Asserts an unset flag falls back to the dart binary the wrapper carries."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["dart"])
    asserts.equals(env, "dart", fields["wrapper"].split("/")[-1])
    asserts.true(env, fields["results"] != "", "want a format stage result for the dart target")
    return analysistest.end(env)

_dart_format_default_test = analysistest.make(_dart_default_impl)

def _dart_wired_impl(ctx):
    """Asserts the flag wires the dart binary and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "dart", fields["dart"].split("/")[-1])
    asserts.true(env, fields["results"] != "", "want a format stage result for the dart target")
    return analysistest.end(env)

_dart_format_set_test = analysistest.make(_dart_wired_impl)

def dart_format_flag_tests(name, subject):
    """Instantiates the dart flag proofs."""
    _dart_format_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _dart_format_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _julia_default_impl(ctx):
    """Asserts an unset flag falls back to the wrapper's formatter."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.equals(env, "", fields["julia_format"])
    asserts.true(env, fields["wrapper"].split("/")[-1].startswith("julia_format"), "want the julia formatter, got " + fields["wrapper"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the julia target")
    return analysistest.end(env)

_julia_format_default_test = analysistest.make(_julia_default_impl)

def _julia_wired_impl(ctx):
    """Asserts the flag wires the Julia formatter and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["julia_format"].split("/")[-1].startswith("julia_format"), "want the julia formatter, got " + fields["julia_format"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the julia target")
    return analysistest.end(env)

_julia_format_set_test = analysistest.make(_julia_wired_impl)

def julia_format_flag_tests(name, subject):
    """Instantiates the julia_format flag proofs."""
    _julia_format_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _julia_format_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _erlfmt_wired_impl(ctx):
    """Asserts the flag wires the erlfmt escript and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["erlfmt"].endswith("/erlfmt"), "want the erlfmt escript, got " + fields["erlfmt"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the erlang target")
    return analysistest.end(env)

_erlfmt_default_test = analysistest.make(_erlfmt_wired_impl)

_erlfmt_set_test = analysistest.make(_erlfmt_wired_impl)

def erlfmt_flag_tests(name, subject):
    """Instantiates the erlfmt flag proofs."""
    _erlfmt_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _erlfmt_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

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
            aspects = [real_elixir_lint_aspect],
            mandatory = True,
        ),
    },
)

def _credo_wired_impl(ctx):
    """Asserts the flag wires the credo launcher and a lint stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["credo"].endswith("/credo"), "want the credo launcher, got " + fields["credo"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the elixir target")
    return analysistest.end(env)

_credo_default_test = analysistest.make(_credo_wired_impl)

_credo_set_test = analysistest.make(_credo_wired_impl)

def credo_flag_tests(name, subject):
    """Instantiates the credo flag proofs."""
    _credo_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _credo_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _elvis_wired_impl(ctx):
    """Asserts the flag wires the elvis escript and a lint stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["elvis"].endswith("/elvis"), "want the elvis escript, got " + fields["elvis"])
    asserts.true(env, fields["results"] != "", "want a lint stage result for the erlang target")
    return analysistest.end(env)

_elvis_default_test = analysistest.make(_elvis_wired_impl)

_elvis_set_test = analysistest.make(_elvis_wired_impl)

def elvis_flag_tests(name, subject):
    """Instantiates the elvis flag proofs."""
    _elvis_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _elvis_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _mix_format_wired_impl(ctx):
    """Asserts the flag wires the mix format launcher and a format stage result."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["mix_format"].endswith("/mix_format"), "want the mix format launcher, got " + fields["mix_format"])
    asserts.true(env, fields["results"] != "", "want a format stage result for the elixir target")
    return analysistest.end(env)

_mix_format_default_test = analysistest.make(_mix_format_wired_impl)

_mix_format_set_test = analysistest.make(_mix_format_wired_impl)

def mix_format_flag_tests(name, subject):
    """Instantiates the mix format flag proofs."""
    _mix_format_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _mix_format_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def qmllint_flag_tests(name, subject):
    """Instantiates the missing qmllint flag proof."""
    _qmllint_unset_test(
        name = name + "_unset_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_unset_test_subject", subject, "tool_defaults"),
        expected_failure_substring = "--@rules_dx//config:tool_qmllint",
    )
    return [
        ":" + name + "_unset_test",
    ]

def qmlformat_flag_tests(name, subject):
    """Instantiates the missing qmlformat flag proof."""
    _qmlformat_unset_test(
        name = name + "_unset_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_unset_test_subject", subject, "tool_defaults"),
        expected_failure_substring = "--@rules_dx//config:tool_qmlformat",
    )
    return [
        ":" + name + "_unset_test",
    ]

def tool_override_tests(name, subject):
    """Instantiates the ruff override selection and rejection proofs."""
    _tool_override_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "default"),
    )
    _tool_override_selected_test(
        name = name + "_selected_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_selected_test_subject", subject, "ruff_alternate"),
    )
    _tool_override_rejects_pair_test(
        name = name + "_rejects_pair_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_rejects_pair_test_subject", subject, "ruff_pair"),
        expected_failure_substring = "two_ruffs' must produce a single file",
    )
    _tool_override_rejects_empty_test(
        name = name + "_rejects_empty_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_rejects_empty_test_subject", subject, "ruff_empty"),
        expected_failure_substring = "no_ruff' must produce a single file",
    )
    _tool_override_rejects_policy_test(
        name = name + "_rejects_policy_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_rejects_policy_test_subject", subject, "ruff_policy"),
        expected_failure_substring = "fixture_policy' must produce a single file",
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_selected_test",
        ":" + name + "_rejects_pair_test",
        ":" + name + "_rejects_empty_test",
        ":" + name + "_rejects_policy_test",
    ]

def _dialyzer_wired_impl(ctx):
    """Asserts the flag wires a typecheck stage result for the erlang target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a typecheck stage result for the erlang target")
    return analysistest.end(env)

_dialyzer_default_test = analysistest.make(_dialyzer_wired_impl)

_dialyzer_set_test = analysistest.make(_dialyzer_wired_impl)

def dialyzer_flag_tests(name, subject):
    """Instantiates the dialyzer flag proofs."""
    _dialyzer_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _dialyzer_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

_dialyzer_elixir_default_test = analysistest.make(_dialyzer_wired_impl)

_dialyzer_elixir_set_test = analysistest.make(_dialyzer_wired_impl)

def dialyzer_elixir_flag_tests(name, subject):
    """Instantiates the dialyzer flag proofs for an elixir_app."""
    _dialyzer_elixir_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _dialyzer_elixir_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _phpcs_wired_impl(ctx):
    """Asserts the flag wires a lint stage result for the php target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a lint stage result for the php target")
    return analysistest.end(env)

_phpcs_default_test = analysistest.make(_phpcs_wired_impl)

_phpcs_set_test = analysistest.make(_phpcs_wired_impl)

def phpcs_flag_tests(name, subject):
    """Instantiates the phpcs flag proofs."""
    _phpcs_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _phpcs_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _phpstan_wired_impl(ctx):
    """Asserts the flag wires a typecheck stage result for the php target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a typecheck stage result for the php target")
    return analysistest.end(env)

_phpstan_default_test = analysistest.make(_phpstan_wired_impl)

_phpstan_set_test = analysistest.make(_phpstan_wired_impl)

def phpstan_flag_tests(name, subject):
    """Instantiates the phpstan flag proofs."""
    _phpstan_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _phpstan_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _luals_wired_impl(ctx):
    """Asserts the flag wires a typecheck stage result for the lua target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a typecheck stage result for the lua target")
    return analysistest.end(env)

_luals_default_test = analysistest.make(_luals_wired_impl)

_luals_set_test = analysistest.make(_luals_wired_impl)

def luals_flag_tests(name, subject):
    """Instantiates the luals flag proofs."""
    _luals_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _luals_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _jet_wired_impl(ctx):
    """Asserts the flag wires a typecheck stage result for the julia target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a typecheck stage result for the julia target")
    return analysistest.end(env)

_jet_default_test = analysistest.make(_jet_wired_impl)

_jet_set_test = analysistest.make(_jet_wired_impl)

def jet_flag_tests(name, subject):
    """Instantiates the jet flag proofs."""
    _jet_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _jet_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _sorbet_wired_impl(ctx):
    """Asserts the flag wires a typecheck stage result for the ruby target."""
    env = analysistest.begin(ctx)
    fields = _subject_under_test(env)[DxSubjectInfo].fields
    asserts.true(env, fields["results"] != "", "want a typecheck stage result for the ruby target")
    return analysistest.end(env)

_sorbet_default_test = analysistest.make(_sorbet_wired_impl)

_sorbet_set_test = analysistest.make(_sorbet_wired_impl)

def sorbet_flag_tests(name, subject):
    """Instantiates the sorbet flag proofs."""
    _sorbet_default_test(
        name = name + "_default_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_default_test_subject", subject, "tool_defaults"),
    )
    _sorbet_set_test(
        name = name + "_set_test",
        size = "small",
        tags = ["manual"],
        target_under_test = _subject(name + "_set_test_subject", subject, "tool_overrides"),
    )
    return [
        ":" + name + "_default_test",
        ":" + name + "_set_test",
    ]

def _dart_config_deps_impl(ctx):
    """Asserts a dart_config carries the lints package and its recommended.yaml."""
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[DxDartConfigInfo]
    names = [pkg.package_name for pkg in info.packages.to_list()]
    asserts.true(env, "lints" in names, "want lints in the config packages, got %s" % names)
    paths = [f.path for f in info.resources.to_list()]
    asserts.true(env, [p for p in paths if p.endswith("lints/lib/recommended.yaml")], "want recommended.yaml in the config resources, got %s" % paths)
    return analysistest.end(env)

dart_config_deps_test = analysistest.make(_dart_config_deps_impl)

def tool_override_suite(name):
    """Instantiates every tool override proof and a test_suite named name over them."""
    tests = []
    for macro, test_name, subject in [
        (tool_override_tests, "tool_override", ":tool_override_default_subject"),
        (qmlformat_flag_tests, "qmlformat_flag", ":qml_subject"),
        (erlfmt_flag_tests, "erlfmt_flag", ":erlang_subject"),
        (elvis_flag_tests, "elvis_flag", ":erlang_lint_subject"),
        (dialyzer_flag_tests, "dialyzer_flag", ":erlang_typecheck_subject"),
        (dialyzer_elixir_flag_tests, "dialyzer_elixir_flag", ":elixir_typecheck_subject"),
        (phpcs_flag_tests, "phpcs_flag", ":php_lint_subject"),
        (luals_flag_tests, "luals_flag", ":lua_typecheck_subject"),
        (sorbet_flag_tests, "sorbet_flag", ":ruby_typecheck_subject"),
        (phpstan_flag_tests, "phpstan_flag", ":php_typecheck_subject"),
        (mix_format_flag_tests, "mix_format_flag", ":elixir_subject"),
        (credo_flag_tests, "credo_flag", ":elixir_lint_subject"),
        (perltidy_flag_tests, "perltidy_flag", ":perl_subject"),
        (perlcritic_flag_tests, "perlcritic_flag", ":perl_lint_subject"),
        (zig_fmt_flag_tests, "zig_fmt_flag", ":zig_subject"),
        (zig_ast_check_flag_tests, "zig_ast_check_flag", ":zig_lint_subject"),
        (dart_format_flag_tests, "dart_format_flag", ":dart_subject"),
        (julia_format_flag_tests, "julia_format_flag", ":julia_subject"),
        (jet_flag_tests, "jet_flag", ":julia_typecheck_subject"),
        (qmllint_flag_tests, "qmllint_flag", ":qml_lint_subject"),
    ]:
        tests += macro(name = test_name, subject = subject)
    dart_config_deps_test(
        name = "dart_config_deps",
        size = "small",
        tags = ["manual"],
        target_under_test = ":dart_lints_config",
    )
    tests.append(":dart_config_deps")
    native.test_suite(name = name, tests = tests, tags = ["manual"])
