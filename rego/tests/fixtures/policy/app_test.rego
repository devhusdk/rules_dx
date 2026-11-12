package app_test

import data.app
import data.rego.tests.fixtures.policy

test_value if app.value == 42

test_data if policy.limit == 1
