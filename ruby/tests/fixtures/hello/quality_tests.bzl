"""Analysis tests that Ruby targets own their main for quality."""

load("@rules_dx//core:owns_class_test.bzl", "owns_class_test")

main_outside_srcs_test = owns_class_test("ruby", ["hello.rb", "main_outside_srcs.rb"], sort = True)
rspec_owns_spec_test = owns_class_test("ruby", ["greeter_spec.rb"], sort = True)
