"""sql_library ownership proofs."""

load("//core/quality:library_tests.bzl", "owns_class_test")

owns_sql_class_test = owns_class_test("sql", ["schema.sql"])
