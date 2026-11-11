package greet_test

import rego.v1
import data.greet

test_hello if greet.hello("dx") == "Hello, dx"
