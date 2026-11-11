"""Conformance suite: one language's build, pass and fail checks over a fixture."""

load("//core/modules:langs.bzl", "LANGS")
load("//rust:defs.bzl", "rust_test")
load(":pending.bzl", "PENDING")

TESTS = ["build_test", "fail_test", "pass_test"]

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
        # until #11196: empty platforms lists one `*` row
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
    return [DefaultInfo(files = depset([out]))]

dx_conformance_manifest = rule(
    implementation = _aggregate_impl,
    attrs = {
        "manifests": attr.label_list(providers = [ConformanceManifestInfo]),
        "pending_langs": attr.string_list(),
    },
    doc = "Writes one JSON with the entries of every language and the platforms of each.",
)

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
    if _all_pending(lang):
        fail("%s: every test is pending on every platform; remove the `*` lines" % lang)
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
    _manifest(
        name = prefix + "manifest",
        entries = json.encode(_entries(lang, {t: "//%s:%s%s" % (native.package_name(), prefix, t) for t in TESTS}, not_applicable)),
        visibility = ["//:__subpackages__"],
    )
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
