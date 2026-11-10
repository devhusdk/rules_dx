"""Retry-semantics fixture (item 3): never fails.

`flaky = True` must reach the private upstream test target through the
wrapper's kwargs without changing this test's outcome: a stable test
never exercises a retry, so this stays green while proving the
attribute flows end to end.
"""


def test_stable_under_flaky() -> None:
    assert True
