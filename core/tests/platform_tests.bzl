"""Analysis tests proving the shared platform probe."""

load("//core:defs.bzl", "starlark_test")
load(":platform_subject.bzl", "platform_subject", "platforms_subject")

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

def platforms_transition_tests(name):
    platforms_subject(
        name = name + "_split_subject",
        platforms = [str(Label("//deploy:linux_arm64")), str(Label("//deploy:linux_amd64"))],
    )
    platforms_subject(
        name = name + "_one_subject",
        platforms = [str(Label("//deploy:linux_arm64"))],
    )
    starlark_test(
        name = name,
        size = "small",
        mode = "analysis",
        subjects = [":" + name + "_split_subject", ":" + name + "_one_subject"],
        expected_observations = """subject //core/tests:{n}_one_subject
field configurations=linux_aarch64
aspect_field aspect_seen=True
aspect_field field_count=1
aspect_field has_subject=True
aspect_field subject_label=//core/tests:{n}_one_subject
aspect_field transitive_count=0
subject //core/tests:{n}_split_subject
field configurations=linux_aarch64,linux_x86_64
aspect_field aspect_seen=True
aspect_field field_count=1
aspect_field has_subject=True
aspect_field subject_label=//core/tests:{n}_split_subject
aspect_field transitive_count=0""".format(n = name),
    )
