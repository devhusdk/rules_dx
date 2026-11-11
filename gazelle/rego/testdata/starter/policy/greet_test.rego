package greet_test

import rego.v1

test_hello if data.greet.hello("dx") == "Hello, dx"
