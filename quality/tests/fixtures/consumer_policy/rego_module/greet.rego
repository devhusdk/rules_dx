package greet

import rego.v1

message(name) := sprintf("hello %s", [name])
