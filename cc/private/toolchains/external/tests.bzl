"""Analysis tests for an ordinary rules_cc toolchain supplied by the user."""

load("//core:defs.bzl", "starlark_test")

def _expected(subject, compiler, target, family, driver, abi, coverage, syntax_only):
    return """subject //cc/private/toolchains/external:{subject}
field cc.abi={abi}
field cc.builtin_includes=
field cc.compiler={compiler}
field cc.coverage={coverage}
field cc.driver={driver}
field cc.family={family}
field cc.libc=fake
field cc.syntax_only={syntax_only}
field cc.sysroot=None
field cc.target={target}
aspect_field aspect_seen=True
aspect_field field_count=10
aspect_field has_subject=True
aspect_field subject_label=//cc/private/toolchains/external:{subject}
aspect_field transitive_count=0""".format(
        abi = abi,
        subject = subject,
        compiler = compiler,
        coverage = coverage,
        driver = driver,
        family = family,
        syntax_only = syntax_only,
        target = target,
    )

def external_toolchain_tests(name):
    """Pins how --extra_toolchains cc toolchains are reported.

    Args:
      name: prefix of the test targets.
    """
    for subject, compiler, family, driver, abi, coverage, syntax_only in [
        ("gcc_subject", "gcc", "gcc", "gnu", "itanium", "gcov", "-fsyntax-only"),
        ("gcc_flag_subject", "gcc", "gcc", "gnu", "itanium", "gcov", "-fsyntax-only"),
        ("icx_subject", "icx", "unknown", "unknown", "unknown", "", ""),
    ]:
        starlark_test(
            name = name + "_" + subject,
            mode = "analysis",
            subjects = [":" + subject],
            expected_observations = _expected(subject, compiler, "fake-%s-linux-gnu" % compiler, family, driver, abi, coverage, syntax_only),
        )
