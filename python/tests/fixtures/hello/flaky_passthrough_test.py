"""Subject of the flaky pass-through check; the target is incompatible and never runs."""


def test_stable_under_flaky() -> None:
    assert True
