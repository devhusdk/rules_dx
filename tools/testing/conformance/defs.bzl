"""Conformance suite: one language's build, pass and fail checks over a fixture."""

load("//core/modules:langs.bzl", "LANGS")
load("//rust:defs.bzl", "rust_test")

_FLAG = Label("//tools/testing/conformance:subject")
_PLATFORM_SETTINGS = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]

def _subject_transition_impl(_settings, _attr):
    return {str(_FLAG): True}

subject_transition = transition(
    implementation = _subject_transition_impl,
    inputs = [],
    outputs = [str(_FLAG)],
)

def _subject_impl(ctx):
    subject = ctx.attr.subject[0]
    executable = subject[DefaultInfo].files_to_run.executable
    return [DefaultInfo(
        files = depset([executable]),
        runfiles = subject[DefaultInfo].default_runfiles,
    )]

dx_conformance_subject = rule(
    implementation = _subject_impl,
    attrs = {
        "subject": attr.label(cfg = subject_transition, executable = True, mandatory = True),
    },
    doc = "Forwards the executable and runfiles of a subject built with the subject flag on.",
)

def subject_compatible_with():
    """Returns the target_compatible_with of a subject: built only under the subject transition."""
    return select({
        Label("//tools/testing/conformance:subject_on"): [],
        "//conditions:default": ["@platforms//:incompatible"],
    })

def _platform_constraints(platform):
    return {
        "linux_arm64": ["@platforms//os:linux", "@platforms//cpu:arm64"],
        "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        "macos_arm64": ["@platforms//os:macos", "@platforms//cpu:arm64"],
        "windows_arm64": ["@platforms//os:windows", "@platforms//cpu:arm64"],
        "windows_x86_64": ["@platforms//os:windows", "@platforms//cpu:x86_64"],
    }[platform]

def _compatible_with(lang, test, not_applicable):
    platforms = LANGS[lang]["platforms"]
    if not platforms:
        fail("%s: lang.json platforms is empty" % lang)
    skipped = {}
    for key in not_applicable:
        parts = key.split(":")
        if len(parts) > 2 or parts[0] not in ["build_test", "pass_test", "fail_test"]:
            fail("%s: unknown not_applicable key %r" % (lang, key))
        if len(parts) == 2 and parts[1] not in _PLATFORM_SETTINGS:
            fail("%s: unknown platform in not_applicable key %r" % (lang, key))
        if parts[0] == test:
            skipped[parts[1] if len(parts) == 2 else ""] = True
    if "" in skipped:
        return ["@platforms//:incompatible"]
    allowed = [p for p in platforms if p not in skipped]
    if not allowed:
        return ["@platforms//:incompatible"]
    conditions = {Label("//tools/testing/conformance:" + p): [] for p in allowed}
    conditions["//conditions:default"] = ["@platforms//:incompatible"]
    return select(conditions)

def dx_conformance_suite(name, lang, binary, expected, passing_test, failing_test, failing_case, not_applicable = {}, size = "small", **kwargs):
    """Declares build_test, pass_test and fail_test for one language.

    Args:
      name: Prefix of the declared targets; empty for the bare test names.
      lang: Key of LANGS.
      binary: Fixture binary whose stdout is expected.
      expected: Stdout of binary.
      passing_test: Test target that passes; built under the subject transition.
      failing_test: Test target that fails with failing_case; built under the subject transition.
      failing_case: Case name the failing test must report.
      not_applicable: Maps a test, or test:platform, to the reason it does not apply.
      size: Size of the declared tests.
      **kwargs: Forwarded to the declared tests.
    """
    if lang not in LANGS:
        fail("unknown lang %r" % lang)
    prefix = name + "_" if name else ""
    dx_conformance_subject(
        name = prefix + "pass_subject",
        subject = passing_test,
        testonly = True,
        visibility = ["//visibility:private"],
    )
    dx_conformance_subject(
        name = prefix + "fail_subject",
        subject = failing_test,
        testonly = True,
        visibility = ["//visibility:private"],
    )
    runs = {
        "build_test": (binary, "build", expected),
        "pass_test": (":" + prefix + "pass_subject", "pass", ""),
        "fail_test": (":" + prefix + "fail_subject", "fail", failing_case),
    }
    for test, (subject, mode, text) in runs.items():
        rust_test(
            name = prefix + test,
            srcs = [Label("//tools/testing/conformance:runner_test.rs")],
            data = [subject],
            deps = [Label("//tools/testing:dx_testing")],
            env = {
                "DX_CONFORMANCE_MODE": mode,
                "DX_CONFORMANCE_SUBJECT": "$(rlocationpath %s)" % subject,
                "DX_CONFORMANCE_TEXT": text,
            },
            size = size,
            target_compatible_with = _compatible_with(lang, test, not_applicable),
            **kwargs
        )
