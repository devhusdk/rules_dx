"""Conformance suite: one language's build, test, coverage, lint, format and lock checks over a fixture."""

load("@bazel_lib//lib:transitions.bzl", "platform_transition_filegroup")
load("//core/modules:langs.bzl", "LANGS")
load("//core/quality/testdata:config_subject.bzl", "config_results")
load("//rust:defs.bzl", "rust_test")
load(":pending.bzl", "PENDING")
load(":quality.bzl", "QUALITY_SUBJECTS")

TESTS = ["build_test", "coverage_test", "cross_test", "fail_test", "format_error_test", "format_test", "lint_error_test", "lint_test", "lock_test", "pass_test", "runfiles_test"]

_QUALITY_TESTS = {
    "format": ["format_test", "format_error_test"],
    "lint": ["lint_test", "lint_error_test"],
}

_QUALITY_KEYS = {
    "format": ["clean", "format_dirty", "format_source"],
    "lint": ["clean", "lint_dirty", "lint_line", "lint_rule", "lint_source"],
}

_QUALITY_CONFIG = "default"

_QUALITY_CONFIGS = {
    "perl": "tool_overrides",
}

_CONFORMANCE_QUALITY = Label("//tools/testing/conformance:conformance_quality")

_FLAG = Label("//tools/testing/conformance:subject")
_PLATFORM_SETTINGS = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]

# The platform a cross_test builds the fixture binary for, per platform it runs on.
CROSS_TARGETS = {
    "linux_arm64": "linux_x86_64",
    "linux_x86_64": "linux_arm64",
    "macos_arm64": "linux_arm64",
    "windows_arm64": "windows_x86_64",
    "windows_x86_64": "windows_arm64",
}

def _cross_select(value):
    branches = {Label("//tools/testing/conformance:" + host): value(target) for host, target in CROSS_TARGETS.items()}
    branches["//conditions:default"] = value(CROSS_TARGETS["linux_x86_64"])
    return select(branches)

def _env(mode, subject, text, pattern):
    return {
        "DX_CONFORMANCE_MODE": mode,
        "DX_CONFORMANCE_PATTERN": pattern,
        "DX_CONFORMANCE_SUBJECT": "$(rlocationpaths %s)" % subject,
        "DX_CONFORMANCE_TEXT": text,
    }

def _subject_transition_impl(_settings, _attr):
    return {str(_FLAG): True}

subject_transition = transition(
    implementation = _subject_transition_impl,
    inputs = [],
    outputs = [str(_FLAG)],
)

# Mirrors the coverage:windows lines of .bazelrc.
WINDOWS_COVERAGE_OFF = ["-coverage", "-gcc_coverage_map_format"]

def coverage_subject_settings(features, windows, instrumentation_filter):
    """Returns the build settings of a coverage subject.

    Args:
      features: Value of --features before the transition.
      windows: Whether the subject targets Windows.
      instrumentation_filter: Value of --instrumentation_filter.

    Returns:
      Dict of the settings the coverage transition sets.
    """
    if windows:
        features = features + [f for f in WINDOWS_COVERAGE_OFF if f not in features]
    return {
        "//command_line_option:collect_code_coverage": True,
        "//command_line_option:features": features,
        "//command_line_option:instrument_test_targets": True,
        "//command_line_option:instrumentation_filter": instrumentation_filter,
        str(_FLAG): True,
    }

def _coverage_transition_impl(settings, attr):
    return coverage_subject_settings(settings["//command_line_option:features"], attr.windows, attr.instrumentation_filter)

_coverage_transition = transition(
    implementation = _coverage_transition_impl,
    inputs = ["//command_line_option:features"],
    outputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:features",
        "//command_line_option:instrument_test_targets",
        "//command_line_option:instrumentation_filter",
        str(_FLAG),
    ],
)

def _subject_impl(ctx):
    subject = ctx.attr.subject[0]
    executable = subject[DefaultInfo].files_to_run.executable
    environment = subject[RunEnvironmentInfo].environment if RunEnvironmentInfo in subject else {}
    env_file = ctx.actions.declare_file(ctx.label.name + ".env")
    ctx.actions.write(env_file, "".join(["%s=%s\n" % (k, environment[k]) for k in sorted(environment)]))
    instrumented = subject[InstrumentedFilesInfo].instrumented_files.to_list() if InstrumentedFilesInfo in subject else []
    manifest = ctx.actions.declare_file(ctx.label.name + ".coverage_manifest")
    ctx.actions.write(manifest, "".join([f.path + "\n" for f in instrumented]))
    metadata = subject[InstrumentedFilesInfo].metadata_files if InstrumentedFilesInfo in subject else depset()
    return [DefaultInfo(
        files = depset([executable, env_file, manifest]),
        runfiles = subject[DefaultInfo].default_runfiles.merge(ctx.runfiles(files = [env_file, manifest], transitive_files = metadata)),
    )]

def _subject_rule(subject_cfg, extra_attrs, doc):
    attrs = {
        "subject": attr.label(cfg = subject_cfg, executable = True, mandatory = True),
    }
    attrs.update(extra_attrs)
    return rule(implementation = _subject_impl, attrs = attrs, doc = doc)

dx_conformance_subject = _subject_rule(
    subject_transition,
    {},
    "Forwards the executable, runfiles and run environment of a subject built with the subject flag on.",
)

_coverage_subject = _subject_rule(
    _coverage_transition,
    {
        "instrumentation_filter": attr.string(mandatory = True),
        "windows": attr.bool(),
    },
    "Forwards a subject built with the subject flag and code coverage on.",
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

def _pending():
    result = {}
    for line in PENDING.splitlines():
        if not line.strip():
            continue
        fields = line.split(" ")
        if len(fields) != 3:
            fail("pending.bzl: want '<lang> <test> <platform>', got %r" % line)
        lang, test, platform = fields
        result.setdefault(lang, {}).setdefault(test, []).append(platform)
    return result

def _all_pending(lang):
    tests = _pending().get(lang, {})
    return all([test in tests and "*" in tests[test] for test in TESTS])

def pending_langs():
    """Returns the languages whose every suite test is pending on every platform."""
    return [lang for lang in LANGS if _all_pending(lang)]

def suite_langs():
    """Returns the languages that declare a conformance suite."""
    return [lang for lang in LANGS if not _all_pending(lang)]

def _skipped(lang, test, not_applicable):
    skipped = {}
    for key in not_applicable:
        parts = key.split(":")
        if len(parts) > 2 or parts[0] not in TESTS:
            fail("%s: unknown not_applicable key %r" % (lang, key))
        if len(parts) == 2 and parts[1] not in _PLATFORM_SETTINGS:
            fail("%s: unknown platform in not_applicable key %r" % (lang, key))
        if parts[0] == test:
            skipped[parts[1] if len(parts) == 2 else ""] = not_applicable[key]
    return skipped

def _compatible_with(lang, test, not_applicable):
    platforms = LANGS[lang]["platforms"]
    if not platforms:
        fail("%s: lang.json platforms is empty" % lang)
    skipped = _skipped(lang, test, not_applicable)
    if "" in skipped:
        return ["@platforms//:incompatible"]
    pending = _pending().get(lang, {}).get(test, [])
    if "*" in pending:
        return ["@platforms//:incompatible"]
    allowed = [p for p in platforms if p not in skipped and p not in pending]
    if not allowed:
        return ["@platforms//:incompatible"]
    conditions = {Label("//tools/testing/conformance:" + p): [] for p in allowed}
    conditions["//conditions:default"] = ["@platforms//:incompatible"]
    return select(conditions)

def _entries(lang, labels, not_applicable):
    entries = []
    pending = _pending().get(lang, {})
    for test in TESTS:
        skipped = _skipped(lang, test, not_applicable)
        # empty platforms lists one `*` row
        for platform in LANGS[lang]["platforms"] or ["*"]:
            reason = ""
            if "" in skipped:
                status = "n/a"
                reason = skipped[""]
            elif platform in skipped:
                status = "n/a"
                reason = skipped[platform]
            elif "*" in pending.get(test, []) or platform in pending.get(test, []):
                status = "pending"
                reason = "pending"
            else:
                status = "run"
            entries.append({
                "capability": test,
                "label": labels.get(test, ""),
                "lang": lang,
                "platform": platform,
                "reason": reason,
                "status": status,
            })
    return entries

ConformanceManifestInfo = provider(
    doc = "Conformance manifest entries of one language.",
    fields = {"entries": "List of entry dicts: lang, label, capability, platform, status, reason."},
)

def _manifest_impl(ctx):
    entries = json.decode(ctx.attr.entries)
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode({"entries": entries}))
    return [DefaultInfo(files = depset([out])), ConformanceManifestInfo(entries = entries)]

_manifest = rule(
    implementation = _manifest_impl,
    attrs = {"entries": attr.string(mandatory = True)},
)

def _aggregate_impl(ctx):
    entries = []
    for dep in ctx.attr.manifests:
        entries.extend(dep[ConformanceManifestInfo].entries)
    for lang in ctx.attr.pending_langs:
        entries.extend(_entries(lang, {}, {}))
    entries = sorted(entries, key = lambda e: (e["lang"], e["capability"], e["platform"]))
    platforms = {lang: LANGS[lang]["platforms"] for lang in LANGS}
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode({"entries": entries, "platforms": platforms}))
    return [DefaultInfo(files = depset([out])), ConformanceManifestInfo(entries = entries)]

dx_conformance_manifest = rule(
    implementation = _aggregate_impl,
    attrs = {
        "manifests": attr.label_list(providers = [ConformanceManifestInfo]),
        "pending_langs": attr.string_list(),
    },
    doc = "Writes one JSON with the entries of every language and the platforms of each.",
)

def dx_coverage_test(name, subject, covered_source, size = "small", **kwargs):
    """Declares a test that runs subject under code coverage and requires covered_source to be fully executed.

    Args:
      name: Name of the test.
      subject: Test target to run; built under the coverage transition.
      covered_source: File name whose lines the subject must cover.
      size: Size of the test.
      **kwargs: Forwarded to the test.
    """
    _coverage_subject(
        name = name + "_subject",
        subject = subject,
        instrumentation_filter = "^//%s[/:]" % native.package_name(),
        windows = select({
            "@platforms//os:windows": True,
            "//conditions:default": False,
        }),
        testonly = True,
        visibility = ["//visibility:private"],
    )
    rust_test(
        name = name,
        srcs = [Label("//tools/testing/conformance:runner_test.rs")],
        data = [":" + name + "_subject"],
        deps = [_CONFORMANCE_QUALITY, Label("//tools/testing:dx_testing")],
        env = {
            "DX_CONFORMANCE_MODE": "coverage",
            "DX_CONFORMANCE_SUBJECT": "$(rlocationpaths :%s_subject)" % name,
            "DX_CONFORMANCE_TEXT": covered_source,
        },
        size = size,
        **kwargs
    )

def _quality_kinds(lang, quality, not_applicable):
    kinds = []
    for kind in sorted(_QUALITY_TESTS):
        runs = [_compatible_with(lang, test, not_applicable) != ["@platforms//:incompatible"] for test in _QUALITY_TESTS[kind]]
        if not any(runs):
            continue
        if kind not in QUALITY_SUBJECTS.get(lang, {}):
            fail("%s: %s tests need a lang.json quality.aspects %s entry, or a pending or not_applicable entry" % (lang, kind, kind))
        for key in _QUALITY_KEYS[kind]:
            if key not in quality:
                fail("%s: quality needs %r" % (lang, key))
        kinds.append(kind)
    return kinds

def _quality_tests(prefix, lang, quality, not_applicable, size, kwargs):
    labels = []
    for kind in _quality_kinds(lang, quality, not_applicable):
        clean, error = _QUALITY_TESTS[kind]
        source = quality[kind + "_source"]
        text = "%s %d" % (quality["lint_rule"], quality["lint_line"]) if kind == "lint" else ""
        for test, fixture, mode in [(clean, quality["clean"], kind), (error, quality[kind + "_dirty"], kind + "_error")]:
            QUALITY_SUBJECTS[lang][kind](
                name = prefix + test + "_subject",
                config = _QUALITY_CONFIGS.get(lang, _QUALITY_CONFIG),
                target = fixture,
                testonly = True,
                target_compatible_with = _compatible_with(lang, test, not_applicable),
                visibility = ["//visibility:private"],
            )
            config_results(
                name = prefix + test + "_results",
                config = _QUALITY_CONFIGS.get(lang, _QUALITY_CONFIG),
                subject = ":" + prefix + test + "_subject",
                testonly = True,
                target_compatible_with = _compatible_with(lang, test, not_applicable),
                visibility = ["//visibility:private"],
            )
            rust_test(
                name = prefix + test,
                srcs = [Label("//tools/testing/conformance:runner_test.rs")],
                data = [":" + prefix + test + "_results", source],
                deps = [_CONFORMANCE_QUALITY, Label("//tools/testing:dx_testing")],
                env = {
                    "DX_CONFORMANCE_MODE": mode,
                    "DX_CONFORMANCE_SOURCE": "$(rlocationpath %s)" % source,
                    "DX_CONFORMANCE_SUBJECT": "$(rlocationpaths :%s_results)" % (prefix + test),
                    "DX_CONFORMANCE_TEXT": text,
                },
                size = size,
                target_compatible_with = _compatible_with(lang, test, not_applicable),
                **kwargs
            )
            labels.append(test)
    return labels

def dx_conformance_suite(name, lang, binary, expected, passing_test, failing_test, failing_case, covered_source, failing_pattern = "--- FAIL: {case}", lock = {}, runfiles = {}, quality = {}, not_applicable = {}, size = "small", **kwargs):
    """Declares build_test, coverage_test, cross_test, pass_test, fail_test, lock_test, runfiles_test, lint_test, lint_error_test, format_test and format_error_test for one language.

    Args:
      name: Prefix of the declared targets; empty for the bare test names.
      lang: Key of LANGS.
      binary: Fixture binary whose stdout is expected.
      expected: Stdout of binary.
      passing_test: Test target that passes; built under the subject transition.
      failing_test: Test target that fails with failing_case; built under the subject transition.
      failing_case: Case name the failing test must report.
      failing_pattern: Line the failing test must print, with one `{case}` placeholder for failing_case.
      covered_source: File name whose lines passing_test must cover; empty only when coverage_test is not applicable.
      lock: Inputs of lock_test: binary, expected, ecosystem, package, manifest, lockfile, stale. Empty only when lock_test is pending or not applicable.
      runfiles: Inputs of runfiles_test: binary, data (file name the binary carries as data), expected. Empty only when runfiles_test is pending or not applicable.
      quality: Inputs of the lint and format tests: clean, the fixture library with no finding; lint_dirty, lint_source, lint_rule and lint_line, the library, file, rule and line of one lint finding; format_dirty and format_source, the library and its unformatted file. Keys of a kind whose tests are all pending or not applicable may be absent.
      not_applicable: Maps a test, or test:platform, to the reason it does not apply.
      size: Size of the declared tests.
      **kwargs: Forwarded to the declared tests.
    """
    if lang not in LANGS:
        fail("unknown lang %r" % lang)
    if _all_pending(lang):
        fail("%s: every test is pending on every platform; remove the `*` lines" % lang)
    if not covered_source and "coverage_test" not in not_applicable:
        fail("%s: covered_source is required unless coverage_test is not applicable" % lang)
    if lock:
        for key in ["binary", "expected", "ecosystem", "package", "manifest", "lockfile", "stale"]:
            if key not in lock:
                fail("%s: lock needs %r" % (lang, key))
        if _compatible_with(lang, "lock_test", not_applicable) == ["@platforms//:incompatible"]:
            fail("%s: lock is set but lock_test is pending or not applicable" % lang)
    elif _compatible_with(lang, "lock_test", not_applicable) != ["@platforms//:incompatible"]:
        fail("%s: lock_test needs lock, or a pending or not_applicable entry" % lang)
    if runfiles:
        for key in ["binary", "data", "expected"]:
            if key not in runfiles:
                fail("%s: runfiles needs %r" % (lang, key))
        if _compatible_with(lang, "runfiles_test", not_applicable) == ["@platforms//:incompatible"]:
            fail("%s: runfiles is set but runfiles_test is pending or not applicable" % lang)
    elif _compatible_with(lang, "runfiles_test", not_applicable) != ["@platforms//:incompatible"]:
        fail("%s: runfiles_test needs runfiles, or a pending or not_applicable entry" % lang)
    prefix = name + "_" if name else ""
    quality_labels = _quality_tests(prefix, lang, quality, not_applicable, size, kwargs)
    dx_conformance_subject(
        name = prefix + "pass_subject",
        subject = passing_test,
        testonly = True,
        target_compatible_with = _compatible_with(lang, "pass_test", not_applicable),
        visibility = ["//visibility:private"],
    )
    dx_conformance_subject(
        name = prefix + "fail_subject",
        subject = failing_test,
        testonly = True,
        target_compatible_with = _compatible_with(lang, "fail_test", not_applicable),
        visibility = ["//visibility:private"],
    )
    _coverage_subject(
        name = prefix + "coverage_subject",
        subject = passing_test,
        instrumentation_filter = "^//%s[/:]" % native.package_name(),
        windows = select({
            "@platforms//os:windows": True,
            "//conditions:default": False,
        }),
        testonly = True,
        target_compatible_with = _compatible_with(lang, "coverage_test", not_applicable),
        visibility = ["//visibility:private"],
    )
    platform_transition_filegroup(
        name = prefix + "cross_binary",
        srcs = [binary],
        target_platform = _cross_select(lambda target: Label("//tools/platforms:" + target)),
        testonly = True,
        target_compatible_with = _compatible_with(lang, "cross_test", not_applicable),
        visibility = ["//visibility:private"],
    )
    runs = {
        "build_test": (binary, "build", expected),
        "coverage_test": (":" + prefix + "coverage_subject", "coverage", covered_source),
        "cross_test": (":" + prefix + "cross_binary", "cross", None),
        "pass_test": (":" + prefix + "pass_subject", "pass", ""),
        "fail_test": (":" + prefix + "fail_subject", "fail", failing_case),
    }
    _manifest(
        name = prefix + "manifest",
        entries = json.encode(_entries(lang, {t: "//%s:%s%s" % (native.package_name(), prefix, t) for t in TESTS if (lock or t != "lock_test") and (runfiles or t != "runfiles_test") and (t in quality_labels or t not in _QUALITY_TESTS["lint"] + _QUALITY_TESTS["format"])}, not_applicable)),
        visibility = ["//:__subpackages__"],
    )
    for test, (subject, mode, text) in runs.items():
        if test == "lock_test":
            continue
        rust_test(
            name = prefix + test,
            srcs = [Label("//tools/testing/conformance:runner_test.rs")],
            data = [subject],
            deps = [_CONFORMANCE_QUALITY, Label("//tools/testing:dx_testing")],
            env = _cross_select(lambda target: _env(mode, subject, target, failing_pattern)) if text == None else _env(mode, subject, text, failing_pattern),
            size = size,
            target_compatible_with = _compatible_with(lang, test, not_applicable),
            **kwargs
        )

    if lock:
        rust_test(
            name = prefix + "lock_test",
            srcs = [Label("//tools/testing/conformance:runner_test.rs")],
            data = [lock["binary"], lock["lockfile"], lock["manifest"], lock["stale"], Label("//cli/depcheck")],
            deps = [_CONFORMANCE_QUALITY, Label("//tools/testing:dx_testing")],
            env = {
                "DX_CONFORMANCE_DEPCHECK": "$(rlocationpath %s)" % Label("//cli/depcheck"),
                "DX_CONFORMANCE_ECOSYSTEM": lock["ecosystem"],
                "DX_CONFORMANCE_LOCK": "$(rlocationpath %s)" % lock["lockfile"],
                "DX_CONFORMANCE_MANIFEST": "$(rlocationpath %s)" % lock["manifest"],
                "DX_CONFORMANCE_MODE": "lock",
                "DX_CONFORMANCE_PACKAGE": lock["package"],
                "DX_CONFORMANCE_STALE": "$(rlocationpath %s)" % lock["stale"],
                "DX_CONFORMANCE_SUBJECT": "$(rlocationpaths %s)" % lock["binary"],
                "DX_CONFORMANCE_TEXT": lock["expected"],
            },
            size = size,
            target_compatible_with = _compatible_with(lang, "lock_test", not_applicable),
            **kwargs
        )

    if runfiles:
        rust_test(
            name = prefix + "runfiles_test",
            srcs = [Label("//tools/testing/conformance:runner_test.rs")],
            data = [runfiles["binary"]],
            deps = [_CONFORMANCE_QUALITY, Label("//tools/testing:dx_testing")],
            env = {
                "DX_CONFORMANCE_ARGS": "_main/%s/%s" % (native.package_name(), runfiles["data"]),
                "DX_CONFORMANCE_MODE": "runfiles",
                "DX_CONFORMANCE_SUBJECT": "$(rlocationpaths %s)" % runfiles["binary"],
                "DX_CONFORMANCE_TEXT": runfiles["expected"],
            },
            size = size,
            target_compatible_with = _compatible_with(lang, "runfiles_test", not_applicable),
            **kwargs
        )
