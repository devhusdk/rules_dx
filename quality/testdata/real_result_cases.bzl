"""Cases where the real aspect and runner run on a fixture and the result file is checked."""

load(":real_results.bzl", "real_result_test")

_ZANUDA_POLICY = "//quality/testdata/zanuda_owned:policy"
_RUST_POLICY = "//quality/testdata/rust_owned:policy"

_DIALYZER = "//erlang/tools:dialyzer"

_NVCC_POLICY = "//quality/testdata/nvcc_syntax:policy"

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

REAL_RESULT_CASES = [
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
        "typecheck_subject": True,
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
        "typecheck_subject": True,
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
            typecheck_subject = case.get("typecheck_subject", False),
            linux_arm64 = case.get("linux_arm64", False),
            macos_arm64 = case.get("macos_arm64", False),
            windows_x86_64 = case.get("windows_x86_64", False),
            windows_arm64 = case.get("windows_arm64", False),
        )
