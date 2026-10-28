"""Root conftest that loads shared fixtures and a built-in plugin."""

pytest_plugins = ["database_fixtures", "pytester"]
