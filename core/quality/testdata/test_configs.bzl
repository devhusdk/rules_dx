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

def _config(policy, compiler = "llvm", profile = "strict", clippy_flags = [], tool_govet = "//go/private/tools/analysis/cmd/dx_govet"):
    """Returns the full settings of one configuration."""
    return {
        "//cc:compiler": compiler,
        "//config:config_profile": profile,
        "//config:tool_dialyzer": str(Label("//erlang/private/tools:dialyzer")),
        "//config:tool_govet": str(Label(tool_govet)),
        "//config:workspace": str(Label(policy)),
        "@rules_rust//rust/settings:clippy_flag": clippy_flags,
        "@rules_rust//rust/settings:clippy_output_diagnostics": True,
        "@rules_rust//rust/settings:extra_rustc_flag": [],
        "@rules_rust//rust/settings:extra_rustc_flags": [],
        "@rules_rust//rust/settings:rustc_output_diagnostics": True,
    }

_CC_SYNTAX = "//core/quality/testdata/clang_syntax:cc_syntax_policy"
_CC_TIDY = "//core/quality/testdata/clang_syntax:cc_tidy_policy"
_DOTNET = "//core/quality/testdata/dotnet_owned:policy"
_GO = "//core/quality/testdata/go_owned:policy"
_HASKELL = "//core/quality/testdata/haskell_owned:policy"
_OCAML = "//core/quality/testdata/ocaml_owned:policy"
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
    "consumer_shared": _config("//core/quality/tests/fixtures/consumer_policy:shared_policy"),
    "default": _config("//config:default_workspace_policy"),
    "detekt_standard": _config("//core/quality/testdata/jvmclean:detekt_standard_policy"),
    "detekt_strict": _config("//core/quality/testdata/jvmclean:detekt_strict_policy"),
    "dialyzer": _config("//core/quality/testdata/dialyzer:policy"),
    "dotnet_owned": _config(_DOTNET),
    "dotnet_owned_standard": _config(_DOTNET, profile = "standard"),
    "eslint_typed_standard": _config("//core/quality/testdata/eslint_typed_owned/standard:ts_policy"),
    "eslint_typed_strict": _config("//core/quality/testdata/eslint_typed_owned/strict:ts_policy"),
    "generated": _config("//core/quality/tests/fixtures/generated_policy:policy"),
    "go_owned": _config(_GO),
    "go_owned_govet_errcheck": _config(_GO, tool_govet = "//go/private/tools/analysis/cmd/dx_errcheck"),
    "go_owned_standard": _config(_GO, profile = "standard"),
    "haskell_owned": _config(_HASKELL),
    "haskell_owned_standard": _config(_HASKELL, profile = "standard"),
    "jvm_owned": _config("//core/quality/testdata/jvm_owned:policy"),
    "nvcc_syntax": _config("//core/quality/testdata/nvcc_syntax:policy"),
    "objc": _config("//core/quality/testdata/clang_syntax:objc_policy"),
    "ocaml_owned": _config(_OCAML),
    "ocaml_owned_standard": _config(_OCAML, profile = "standard"),
    "pmd_standard": _config("//core/quality/testdata/jvmclean:pmd_standard_policy"),
    "pmd_strict": _config("//core/quality/testdata/jvmclean:pmd_strict_policy"),
    "python_tiers_ruff": _config("//core/quality/testdata/python_tiers:ruff_policy"),
    "rust_owned": _config(_RUST),
    "rust_owned_clippy_standard": _config(_RUST, clippy_flags = _CLIPPY_STANDARD),
    "rust_owned_clippy_strict": _config(_RUST, clippy_flags = _CLIPPY_STRICT),
    "scalafix_aux": _config("//core/quality/testdata/scalafix_aux:policy"),
    "tier_clean": _config("//core/quality/testdata/tier_clean:policy"),
    "tier_clean_strict": _config("//core/quality/testdata/tier_clean:strict_policy"),
    "workspace": _config("//.dx:policy"),
    "zanuda_owned": _config("//core/quality/testdata/zanuda_owned:policy"),
}

TEST_CONFIG_SETTINGS = sorted(TEST_CONFIGS["default"].keys())
