"""rules_shell sh_test behind a forwarder whose coverage goes through the dx native collector."""

load("@rules_shell//shell:sh_test.bzl", _sh_test = "sh_test")
load(":native_coverage.bzl", "dx_native_coverage_attrs", "dx_native_coverage_toolchains", "dx_native_test_coverage")
load(":wrapper.bzl", "dx_data_coverage_aspect", "dx_executable_forward_rule", "dx_forward_spec", "dx_language_spec", "dx_lcov_merger_attr", "dx_wrap_test")

visibility(["//..."])

_SHELL = dx_language_spec(what = "sh_test", quality_specs = [], allow_files = [".sh"])

_sh_forward_test = dx_executable_forward_rule(
    _SHELL,
    "test",
    dx_forward_spec(
        extra_attrs = dx_lcov_merger_attr() | dx_native_coverage_attrs(),
        upstream_aspects = [dx_data_coverage_aspect],
        coverage_providers = dx_native_test_coverage,
        toolchains = dx_native_coverage_toolchains(),
    ),
)

def sh_test(name, srcs, visibility = None, **kwargs):
    """A rules_shell sh_test whose coverage the dx collector gathers, with the LLVM tools it needs."""
    dx_wrap_test(name, _sh_test, _sh_forward_test, srcs, visibility = visibility, quality_srcs = [], upstream_env = True, **kwargs)
