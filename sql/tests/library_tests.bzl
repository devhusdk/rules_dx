"""sql_library ownership proofs."""

load("//core:owns_class_test.bzl", "owns_class_test")

owns_sql_class_test = owns_class_test("sql", ["schema.sql"])
