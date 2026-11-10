"""Public C and C++ API."""

load(
    "//cc/private:defs.bzl",
    _CC_COVERAGE_INPUTS = "CC_COVERAGE_INPUTS",
    _CC_COVERAGE_TRANSITION = "CC_COVERAGE_TRANSITION",
    _CcCoverageProbeInfo = "CcCoverageProbeInfo",
    _cc_bench = "cc_bench",
    _cc_binary = "cc_binary",
    _cc_coverage_features = "cc_coverage_features",
    _cc_coverage_mismatch = "cc_coverage_mismatch",
    _cc_coverage_probe = "cc_coverage_probe",
    _cc_coverage_settings = "cc_coverage_settings",
    _cc_coverage_unsupported = "cc_coverage_unsupported",
    _cc_fuzz_test = "cc_fuzz_test",
    _cc_library = "cc_library",
    _cc_param_file_features = "cc_param_file_features",
    _cc_test = "cc_test",
    _objc_library = "objc_library",
)

CC_COVERAGE_INPUTS = _CC_COVERAGE_INPUTS
CC_COVERAGE_TRANSITION = _CC_COVERAGE_TRANSITION
CcCoverageProbeInfo = _CcCoverageProbeInfo
cc_bench = _cc_bench
cc_binary = _cc_binary
cc_coverage_features = _cc_coverage_features
cc_coverage_mismatch = _cc_coverage_mismatch
cc_coverage_probe = _cc_coverage_probe
cc_coverage_settings = _cc_coverage_settings
cc_coverage_unsupported = _cc_coverage_unsupported
cc_fuzz_test = _cc_fuzz_test
cc_library = _cc_library
cc_param_file_features = _cc_param_file_features
cc_test = _cc_test
objc_library = _objc_library
