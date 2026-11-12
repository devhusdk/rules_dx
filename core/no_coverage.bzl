"""Transition that builds one dependency edge without coverage instrumentation."""

_COVERAGE = "//command_line_option:collect_code_coverage"

def _no_coverage_transition_impl(_settings, _attr):
    return {_COVERAGE: False}

no_coverage_transition = transition(
    implementation = _no_coverage_transition_impl,
    inputs = [],
    outputs = [_COVERAGE],
)
