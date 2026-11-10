"""Build settings of every quality test configuration."""

MAX_TEST_CONFIGS = 39

_CLIPPY_STANDARD = ["-Wclippy::all", "-Dwarnings"]

_CLIPPY_STRICT = [
    "-Wclippy::all",
    "-Wclippy::pedantic",
    "-Wclippy::dbg_macro",
    "-Wclippy::todo",
    "-Wclippy::unimplemented",
    "-Wclippy::allow_attributes_without_reason",
    "-Wclippy::allow_attributes",
    "-Wrust_2018_idioms",
    "-Wunused_qualifications",
    "-Dwarnings",
]

_TOOL_DEFAULTS = {
    "//config:tool_credo": "//elixir/private/tools:credo",
    "//config:tool_dart": "//config:no_dart",
    "//config:tool_dialyzer": "//erlang/private/tools:dialyzer",
    "//config:tool_elvis": "//erlang/private/tools:elvis",
    "//config:tool_erlfmt": "//erlang/private/tools:erlfmt",
    "//config:tool_govet": "//go/private/tools/analysis/cmd/dx_govet",
    "//config:tool_jet": "//julia/private/tools/jet:jet",
    "//config:tool_julia_format": "//config:no_julia_format",
    "//config:tool_luals": "//lua/private/tools:luals",
    "//config:tool_mix_format": "//elixir/private/tools:mix_format",
    "//config:tool_perlcritic": "//config:no_perlcritic",
    "//config:tool_perltidy": "//config:no_perltidy",
    "//config:tool_phpcs": "//php/private/tools:phpcs",
    "//config:tool_phpstan": "//php/private/tools:phpstan",
    "//config:tool_ruff": "@dx_tools//:ruff",
    "//config:tool_sorbet": "//ruby/private/tools:sorbet",
    "//config:tool_zig_ast_check": "//config:no_zig_ast_check",
    "//config:tool_zig_fmt": "//config:no_zig_fmt",
}

_TOOL_OVERRIDES = {
    "//config:tool_credo": "//elixir/private/tools:credo",
    "//config:tool_dart": "//dart/private/tools:dart",
    "//config:tool_dialyzer": "//erlang/private/tools:dialyzer",
    "//config:tool_elvis": "//erlang/private/tools:elvis",
    "//config:tool_erlfmt": "//erlang/private/tools:erlfmt",
    "//config:tool_govet": "//go/private/tools/analysis/cmd/dx_errcheck",
    "//config:tool_jet": "//julia/private/tools/jet:jet",
    "//config:tool_julia_format": "//julia/private/tools:julia_format",
    "//config:tool_luals": "//lua/private/tools:luals",
    "//config:tool_mix_format": "//elixir/private/tools:mix_format",
    "//config:tool_perlcritic": "//perl/private/tools:perlcritic",
    "//config:tool_perltidy": "//perl/private/tools:perltidy",
    "//config:tool_phpcs": "//php/private/tools:phpcs",
    "//config:tool_phpstan": "//php/private/tools:phpstan",
    "//config:tool_sorbet": "//ruby/private/tools:sorbet",
    "//config:tool_zig_ast_check": "//zig/private/tools:zig",
    "//config:tool_zig_fmt": "//zig/private/tools:zig",
}

def _config(policy, compiler = "llvm", profile = "strict", clippy_flags = [], tools = {}):
    """Returns the full settings of one configuration."""
    settings = {
        "//cc:compiler": compiler,
        "//config:config_profile": profile,
        "//config:workspace": str(Label(policy)),
        "@rules_rust//rust/settings:clippy_flag": clippy_flags,
        "@rules_rust//rust/settings:clippy_output_diagnostics": True,
        "@rules_rust//rust/settings:extra_rustc_flag": [],
        "@rules_rust//rust/settings:extra_rustc_flags": [],
        "@rules_rust//rust/settings:rustc_output_diagnostics": True,
    }
    for flag, tool in dict(_TOOL_DEFAULTS, **tools).items():
        settings[flag] = str(Label(tool))
    return settings

_CC_SYNTAX = "//core/quality/testdata/clang_syntax:cc_syntax_policy"
_CC_TIDY = "//core/quality/testdata/clang_syntax:cc_tidy_policy"
_DOTNET = "//core/quality/testdata/dotnet_owned:policy"
_GO = "//core/quality/testdata/go_owned:policy"
_HASKELL = "//core/quality/testdata/haskell_owned:policy"
_OCAML = "//core/quality/testdata/ocaml_owned:policy"
_SHARED = "//core/quality/tests/fixtures/consumer_policy:shared_policy"
_TOOL_OVERRIDE_POLICY = "//core/quality/testdata:tool_override_policy"
_RUST = "//core/quality/testdata/rust_owned:policy"

TEST_CONFIGS = {
    "cc_syntax": _config(_CC_SYNTAX),
    "cc_syntax_gcc": _config(_CC_SYNTAX, compiler = "gcc"),
    "cc_syntax_msvc": _config(_CC_SYNTAX, compiler = "msvc"),
    "cc_tidy": _config(_CC_TIDY),
    "cc_tidy_gcc": _config(_CC_TIDY, compiler = "gcc"),
    "cc_tidy_msvc": _config(_CC_TIDY, compiler = "msvc"),
    "consumer_cc_clang_tidy": _config("//core/quality/tests/fixtures/consumer_policy:cc_clang_tidy_policy"),
    "consumer_format_off": _config("//core/quality/tests/fixtures/consumer_policy:format_off_policy"),
    "shared": _config(_SHARED),
    "shared_standard": _config(_SHARED, profile = "standard"),
    "default": _config("//config:default_workspace_policy"),
    "dialyzer": _config("//core/quality/testdata/dialyzer:policy"),
    "dotnet_owned": _config(_DOTNET),
    "dotnet_owned_standard": _config(_DOTNET, profile = "standard"),
    "eslint_typed_standard": _config("//core/quality/testdata/eslint_typed_owned/standard:ts_policy"),
    "eslint_typed_strict": _config("//core/quality/testdata/eslint_typed_owned/strict:ts_policy"),
    "generated": _config("//core/quality/tests/fixtures/generated_policy:policy"),
    "go_owned": _config(_GO),
    "go_owned_standard": _config(_GO, profile = "standard"),
    "haskell_owned": _config(_HASKELL),
    "haskell_owned_standard": _config(_HASKELL, profile = "standard"),
    "jvm_owned": _config("//core/quality/testdata/jvm_owned:policy"),
    "nvcc_syntax": _config("//core/quality/testdata/nvcc_syntax:policy"),
    "objc": _config("//core/quality/testdata/clang_syntax:objc_policy"),
    "ocaml_owned": _config(_OCAML),
    "ocaml_owned_standard": _config(_OCAML, profile = "standard"),
    "python_tiers_ruff": _config("//core/quality/testdata/python_tiers:ruff_policy"),
    "ruff_alternate": _config("//config:default_workspace_policy", tools = {"//config:tool_ruff": "//core/quality/testdata:alternate_ruff"}),
    "ruff_empty": _config("//config:default_workspace_policy", tools = {"//config:tool_ruff": "//core/quality/testdata:no_ruff"}),
    "ruff_pair": _config("//config:default_workspace_policy", tools = {"//config:tool_ruff": "//core/quality/testdata:two_ruffs"}),
    "ruff_policy": _config("//config:default_workspace_policy", tools = {"//config:tool_ruff": "//core/quality:fixture_policy"}),
    "rust_owned": _config(_RUST),
    "rust_owned_clippy_standard": _config(_RUST, clippy_flags = _CLIPPY_STANDARD),
    "rust_owned_clippy_strict": _config(_RUST, clippy_flags = _CLIPPY_STRICT),
    "scalafix_aux": _config("//core/quality/testdata/scalafix_aux:policy"),
    "tool_defaults": _config(_TOOL_OVERRIDE_POLICY),
    "tool_overrides": _config(_TOOL_OVERRIDE_POLICY, tools = _TOOL_OVERRIDES),
    "workspace": _config("//.dx:policy"),
    "zanuda_owned": _config("//core/quality/testdata/zanuda_owned:policy"),
}

TEST_CONFIG_SETTINGS = sorted(TEST_CONFIGS["default"].keys())
