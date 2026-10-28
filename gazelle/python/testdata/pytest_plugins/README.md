# Pytest plugin declarations

Verifies module-level `pytest_plugins` entries in tests and in `conftest.py`
files at any level become deps, with resolve and ignore directives applied.
Function-local declarations and built-in pytest plugins add no deps.
