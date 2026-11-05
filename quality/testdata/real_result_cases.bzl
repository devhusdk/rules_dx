"""Cases where the real aspect and runner run on a fixture and the result file is checked."""

load(":real_results.bzl", "real_result_test")

_ZANUDA_POLICY = "//quality/testdata/zanuda_owned:policy"
_RUST_POLICY = "//quality/testdata/rust_owned:policy"

_DIALYZER = "//erlang/tools:dialyzer"

_GO_POLICY = "//quality/testdata/go_owned:policy"

_GO_TYPECHECK_POLICY = "//quality/testdata/go_owned:typecheck_policy"

_GO_STATICCHECK_POLICY = "//quality/testdata/go_owned:staticcheck_policy"

_GO_LINT_ALL_POLICY = "//quality/testdata/go_owned:lint_all_policy"

_GO_GOVET_POLICY = "//quality/testdata/go_owned:govet_policy"

_NVCC_POLICY = "//quality/testdata/nvcc_syntax:policy"

_CC_POLICY = "//quality/testdata/clang_syntax:cc_syntax_policy"
_OBJC_POLICY = "//quality/testdata/clang_syntax:objc_policy"
_CC_TIDY_POLICY = "//quality/testdata/clang_syntax:cc_tidy_policy"
_MACOS_ONLY = ["@platforms//os:macos"]

def _gcc_case(tool, capability, name, target, contains, absent, policy = _CC_POLICY):
    return {
        "name": "real_result_" + tool + "_cc_gcc_" + name,
        "target": "//quality/testdata/clang_syntax:" + target,
        "policy": policy,
        "tool": tool,
        "capability": capability,
        "class": "c",
        "compiler": "gcc",
        "contains": contains,
        "absent": absent,
    }

def _cc_case(tool, capability, name, target, contains, absent, policy = _CC_POLICY):
    return {
        "name": "real_result_" + tool + "_cc_" + name,
        "target": "//quality/testdata/clang_syntax:" + target,
        "policy": policy,
        "tool": tool,
        "capability": capability,
        "class": "c",
        "contains": contains,
        "absent": absent,
        "linux_arm64": True,
        "macos_arm64": True,
        "windows_x86_64": True,
        "windows_arm64": True,
    }

def _objc_case(name, contains, absent):
    return {
        "name": "real_result_clang_syntax_objc_" + name,
        "target": "//quality/testdata/clang_syntax:" + name + "_objc",
        "policy": _OBJC_POLICY,
        "tool": "clang_syntax",
        "capability": "typecheck",
        "class": "objc",
        "contains": contains,
        "absent": absent,
        "target_compatible_with": _MACOS_ONLY,
        "macos_arm64": True,
    }

def _nvcc_case(name, contains, absent):
    return {
        "name": "real_result_nvcc_syntax_" + name,
        "target": "//quality/testdata/nvcc_syntax:" + name,
        "policy": _NVCC_POLICY,
        "tool": "nvcc_syntax",
        "capability": "typecheck",
        "class": "cuda",
        "contains": contains,
        "absent": absent,
        "linux_arm64": True,
        "windows_x86_64": True,
    }

def _go_case(tool, capability, kind, target, contains, absent, policy = _GO_POLICY, **extra):
    return extra | {
        "name": "real_result_" + tool + "_" + kind,
        "target": target,
        "policy": policy,
        "tool": tool,
        "capability": capability,
        "class": "go",
        "contains": contains,
        "absent": absent,
        "linux_arm64": True,
        "macos_arm64": True,
        "windows_x86_64": True,
        "windows_arm64": True,
    }

REAL_RESULT_CASES = [
    _cc_case("clang_syntax", "typecheck", "clean", "clean", ["quality/testdata/clang_syntax/clean.c"], ["undeclared_name", "Fatal error"]),
    _cc_case("clang_syntax", "typecheck", "dirty", "dirty", ["undeclared identifier 'undeclared_name'"], []),
    _cc_case("clang_tidy", "lint", "clean", "clean", ["quality/testdata/clang_syntax/clean.c"], ["readability-else-after-return", "Error"], _CC_TIDY_POLICY),
    _cc_case("clang_tidy", "lint", "dirty", "tidy_dirty", ["readability-else-after-return"], [], _CC_TIDY_POLICY),
    _gcc_case("clang_syntax", "typecheck", "clean", "clean", ["quality/testdata/clang_syntax/clean.c"], ["undeclared_name", "Fatal error"]),
    _gcc_case("clang_syntax", "typecheck", "dirty", "dirty", ["'undeclared_name' undeclared"], []),
    _gcc_case("clang_tidy", "lint", "clean", "clean", ["quality/testdata/clang_syntax/clean.c"], ["readability-else-after-return", "Error"], _CC_TIDY_POLICY),
    _objc_case("clean", ["clean.m"], ["undeclared_name", "Fatal error"]),
    _objc_case("dirty", ["undeclared identifier 'undeclared_name'"], []),
    {
        "name": "real_result_zanuda_clean",
        "target": "//quality/testdata/zanuda_owned:clean",
        "policy": _ZANUDA_POLICY,
        "tool": "zanuda",
        "capability": "lint",
        "class": "ocaml",
        "contains": ["quality/testdata/zanuda_owned/clean.ml"],
        "absent": ["if_bool", "list_length_comparisons", "Fatal error", "no such file"],
    },
    {
        "name": "real_result_zanuda_dirty",
        "target": "//quality/testdata/zanuda_owned:dirty",
        "policy": _ZANUDA_POLICY,
        "tool": "zanuda",
        "capability": "lint",
        "class": "ocaml",
        "contains": ["quality/testdata/zanuda_owned/dirty.ml", "if_bool", "list_length_comparisons"],
        "absent": [],
    },
    _go_case("errcheck", "lint", "clean", "//go/tests/fixtures/errcheck:clean", ["errcheck"], ["unchecked error"]),
    _go_case("errcheck", "lint", "dirty", "//go/tests/fixtures/errcheck:dirty", ["unchecked error"], []),
    _go_case("govet", "lint", "clean", "//go/tests/fixtures/govet:clean", ["govet"], ["non-constant format string"]),
    _go_case("govet", "lint", "dirty", "//go/tests/fixtures/govet:dirty", ["non-constant format string"], []),
    _go_case("gotype", "typecheck", "clean", "//go/tests/fixtures/gotype:clean", ["gotype"], ["cannot use"], _GO_TYPECHECK_POLICY),
    _go_case("gotype", "typecheck", "dirty", "//go/tests/fixtures/gotype:broken", ["cannot use"], [], _GO_TYPECHECK_POLICY),
    _go_case("staticcheck", "lint", "clean", "//go/tests/fixtures/staticcheck:clean", ["staticcheck"], ["SA4017"], _GO_STATICCHECK_POLICY),
    _go_case("staticcheck", "lint", "dirty", "//go/tests/fixtures/staticcheck:dirty", ["SA4017", "double", "U1000", "unusedHelper"], [], _GO_STATICCHECK_POLICY),
    _go_case("golint", "lint", "clean", "//go/tests/fixtures/golint:clean", ["errcheck", "govet", "staticcheck"], ["unchecked error", "non-constant format string", "SA4017"], _GO_LINT_ALL_POLICY),
    _go_case("golint", "lint", "dirty", "//go/tests/fixtures/golint:app", ["unchecked error", "Unmarshal", "non-constant format string", "app.go", "SA4017", "tagged_on.go"], ["tagged_off.go", "gen.go"], _GO_LINT_ALL_POLICY),
    _go_case("govet", "lint", "override", "//go/tests/fixtures/errcheck:dirty", ["unchecked error"], [], _GO_GOVET_POLICY, tool_govet = "//go/tools/analysis/cmd/dx_errcheck"),
    _nvcc_case("clean", ["nvcc_syntax/clean.cu"], ["__host__", "undefined", "declared but never"]),
    _nvcc_case("dirty", ["nvcc_syntax/dirty.cu", "__host__ function"], []),
    _nvcc_case("two_archs", ["nvcc_syntax/two_archs.cu", "177-D"], []),
    {
        "name": "real_result_dialyzer_erlang_clean",
        "target": "//erlang/tests/runtime:erlang_app",
        "policy": "//quality/testdata/dialyzer:erlang_policy",
        "tool": "dialyzer",
        "capability": "typecheck",
        "class": "erlang",
        "tool_dialyzer": _DIALYZER,
        "windows_x86_64": True,
        "windows_arm64": True,
        "linux_arm64": True,
        "macos_arm64": True,
        "contains": [],
        "absent": ["invalid_contract"],
    },
    {
        "name": "real_result_dialyzer_erlang_dirty",
        "target": "//quality/testdata/dialyzer:erlang_dirty",
        "policy": "//quality/testdata/dialyzer:erlang_policy",
        "tool": "dialyzer",
        "capability": "typecheck",
        "class": "erlang",
        "tool_dialyzer": _DIALYZER,
        "windows_x86_64": True,
        "windows_arm64": True,
        "linux_arm64": True,
        "macos_arm64": True,
        "contains": ["dialyzer/", "invalid_contract"],
        "absent": [],
    },
    {
        "name": "real_result_dialyzer_elixir_clean",
        "target": "//elixir/tests/runtime:erlang_app",
        "policy": "//quality/testdata/dialyzer:elixir_policy",
        "tool": "dialyzer",
        "capability": "typecheck",
        "class": "elixir",
        "tool_dialyzer": _DIALYZER,
        "linux_arm64": True,
        "macos_arm64": True,
        "contains": [],
        "absent": ["invalid_contract"],
    },
    {
        "name": "real_result_dialyzer_elixir_dirty",
        "target": "//quality/testdata/dialyzer:elixir_dirty",
        "policy": "//quality/testdata/dialyzer:elixir_policy",
        "tool": "dialyzer",
        "capability": "typecheck",
        "class": "elixir",
        "tool_dialyzer": _DIALYZER,
        "linux_arm64": True,
        "macos_arm64": True,
        "contains": ["dialyzer/", "invalid_contract"],
        "absent": [],
    },
    {
        "name": "real_result_clippy_clean",
        "target": "//quality/testdata/rust_owned:clean",
        "policy": _RUST_POLICY,
        "tool": "clippy",
        "capability": "lint",
        "class": "rust",
        "contains": ["quality/testdata/rust_owned/clean.rs"],
        "absent": ["len_zero", "Fatal error"],
        "linux_arm64": True,
        "macos_arm64": True,
        "windows_x86_64": True,
        "windows_arm64": True,
    },
    {
        "name": "real_result_clippy_dirty",
        "target": "//quality/testdata/rust_owned:dirty",
        "policy": _RUST_POLICY,
        "tool": "clippy",
        "capability": "lint",
        "class": "rust",
        "contains": ["quality/testdata/rust_owned/dirty.rs", "len_zero"],
        "absent": [],
        "linux_arm64": True,
        "macos_arm64": True,
        "windows_x86_64": True,
        "windows_arm64": True,
    },
    {
        "name": "real_result_rustc_clean",
        "target": "//quality/testdata/rust_owned:clean",
        "policy": _RUST_POLICY,
        "tool": "rustc",
        "capability": "typecheck",
        "class": "rust",
        "contains": ["quality/testdata/rust_owned/clean.rs"],
        "absent": ["unused variable", "Fatal error"],
        "linux_arm64": True,
        "macos_arm64": True,
        "windows_x86_64": True,
        "windows_arm64": True,
    },
    {
        "name": "real_result_rustc_dirty",
        "target": "//quality/testdata/rust_owned:dirty",
        "policy": _RUST_POLICY,
        "tool": "rustc",
        "capability": "typecheck",
        "class": "rust",
        "contains": ["quality/testdata/rust_owned/dirty.rs", "unused variable"],
        "absent": [],
        "linux_arm64": True,
        "macos_arm64": True,
        "windows_x86_64": True,
        "windows_arm64": True,
    },
]

def real_result_evidence(case):
    """Returns a matrix-shaped evidence case for one real-result case."""
    evidence = {
        "capability": case["capability"],
        "tool_names": [case["tool"]],
        "stages": [case["tool"] + ";" + case["class"] + ";" + case["name"]],
    }
    for flag in ["linux_arm64", "macos_arm64", "windows_x86_64", "windows_arm64"]:
        if flag in case:
            evidence[flag] = case[flag]
    return evidence

def real_result_cases():
    """Instantiates every real-result case."""
    for case in REAL_RESULT_CASES:
        real_result_test(
            name = case["name"],
            target = case["target"],
            policy = case["policy"],
            contains = case["contains"],
            absent = case["absent"],
            tool_dialyzer = case.get("tool_dialyzer"),
            tool_govet = case.get("tool_govet"),
            compiler = case.get("compiler", ""),
            typecheck_subject = case.get("typecheck_subject", False),
            target_compatible_with = case.get("target_compatible_with"),
            linux_arm64 = case.get("linux_arm64", False),
            macos_arm64 = case.get("macos_arm64", False),
            windows_x86_64 = case.get("windows_x86_64", False),
            windows_arm64 = case.get("windows_arm64", False),
        )
