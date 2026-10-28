"""Function-local declarations are not read by pytest."""


def helper():
    pytest_plugins = ["http_fixtures"]
    return pytest_plugins


def test_local():
    assert helper()
