"""Analysis tests proving the shared platform probe."""

load("//core:defs.bzl", "starlark_test")
load(":platform_subject.bzl", "platform_subject")

def platform_probe_tests(name):
    platform_subject(
        name = name + "_arm64_subject",
        platform = "//deploy:linux_arm64",
    )
    platform_subject(
        name = name + "_amd64_subject",
        platform = "//deploy:linux_amd64",
    )
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":" + name + "_arm64_subject", ":" + name + "_amd64_subject"],
        expected_observations = """subject //core/tests:{n}_amd64_subject
field cpu=x86_64
field os=linux
aspect_field aspect_seen=True
aspect_field field_count=2
aspect_field has_subject=True
aspect_field subject_label=//core/tests:{n}_amd64_subject
aspect_field transitive_count=0
subject //core/tests:{n}_arm64_subject
field cpu=aarch64
field os=linux
aspect_field aspect_seen=True
aspect_field field_count=2
aspect_field has_subject=True
aspect_field subject_label=//core/tests:{n}_arm64_subject
aspect_field transitive_count=0""".format(n = name),
    )
