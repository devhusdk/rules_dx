package app_test

import data.app
import data.rego.tests.fixtures

test_value if app.value == 42

test_data if fixtures.limit == 1
