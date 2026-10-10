"""Selection tests for standalone tool artifact repositories."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "expect_contains", "expect_equal", "expect_false", "starlark_test")
load("//quality/artifacts:extension.bzl", "TOOL_ARTIFACTS")
load("//quality/artifacts:hub.bzl", "PLATFORM_CONSTRAINTS", "TOOL_PLATFORMS", "artifact_map_errors", "artifact_metadata_errors", "branch_labels", "decode_artifacts", "encode_artifacts", "hub_build", "no_match_error", "ordered_platforms")

_CONSTRAINT_LABELS = {
    "cpu_arm64": "@platforms//cpu:arm64",
    "cpu_x86_64": "@platforms//cpu:x86_64",
    "os_linux": "@platforms//os:linux",
    "os_macos": "@platforms//os:macos",
    "os_windows": "@platforms//os:windows",
}

_FIXTURE_TOOL = "taplo"

_FIXTURE_REPOS = {
    "linux_x86_64": "dx_hub_fixture_taplo_linux_x86_64",
    "linux_arm64": "dx_hub_fixture_taplo_linux_arm64",
    "macos_arm64": "dx_hub_fixture_taplo_macos_arm64",
    "windows_x86_64": "dx_hub_fixture_taplo_windows_x86_64",
}

_SPARSE = {
    _FIXTURE_TOOL: {
        platform: _FIXTURE_REPOS[platform]
        for platform in ["linux_x86_64", "macos_arm64", "windows_x86_64"]
    },
}

_COMPLETE = {_FIXTURE_TOOL: dict(_FIXTURE_REPOS)}

def _artifact_repo(artifact):
    """Returns the tool repository name one artifact is fetched into."""
    return "dx_%s_%s_%s" % (artifact["tool"], artifact["os"], artifact["cpu"])

def _released_map():
    """Returns the released tool-to-platform repository map."""
    by_tool = {}
    for artifact in TOOL_ARTIFACTS:
        by_tool.setdefault(artifact["tool"], {})[artifact["os"] + "_" + artifact["cpu"]] = _artifact_repo(artifact)
    return by_tool

def _platform_branches(tool):
    return {
        ":" + platform: "@dx_%s_%s//:tool" % (tool, platform)
        for platform in TOOL_PLATFORMS
    }

def _unit_checks():
    released = _released_map()
    rendered = hub_build(released, _CONSTRAINT_LABELS)
    sparse_rendered = hub_build(_SPARSE, _CONSTRAINT_LABELS)
    sample = dict(TOOL_ARTIFACTS[0])
    sample_tool = sample["tool"]
    sample_platform = sample["os"] + "_" + sample["cpu"]
    checks = [
        expect_equal("the platform table is the supported set", TOOL_PLATFORMS, ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"]),
        expect_equal("the encoded map keys every platform explicitly", encode_artifacts(_SPARSE), '{"taplo":{"linux_x86_64":"dx_hub_fixture_taplo_linux_x86_64","macos_arm64":"dx_hub_fixture_taplo_macos_arm64","windows_x86_64":"dx_hub_fixture_taplo_windows_x86_64"}}'),
        expect_equal("a sparse map keeps the platforms it declares", ordered_platforms(_SPARSE[_FIXTURE_TOOL]), ["linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("a sparse map names the repository of every platform it declares", branch_labels(_SPARSE[_FIXTURE_TOOL]), {
            ":linux_x86_64": "@dx_hub_fixture_taplo_linux_x86_64//:tool",
            ":macos_arm64": "@dx_hub_fixture_taplo_macos_arm64//:tool",
            ":windows_x86_64": "@dx_hub_fixture_taplo_windows_x86_64//:tool",
        }),
        expect_false("a sparse map selects no repository for the platform it lacks", '":linux_arm64": "@' in sparse_rendered),
        expect_equal("a sparse map diagnostic names the tool and the platforms it has", no_match_error(_SPARSE[_FIXTURE_TOOL], _FIXTURE_TOOL), "rules_dx: no taplo artifact for this execution platform; want one of linux_x86_64, macos_arm64, windows_x86_64."),
        expect_equal("a sparse map is a valid repository map", artifact_map_errors(_SPARSE), []),
        expect_equal("a complete map keeps every supported platform", ordered_platforms(_COMPLETE[_FIXTURE_TOOL]), TOOL_PLATFORMS),
        expect_equal("a complete map names the repository of every platform", branch_labels(_COMPLETE[_FIXTURE_TOOL]), {
            ":linux_x86_64": "@dx_hub_fixture_taplo_linux_x86_64//:tool",
            ":linux_arm64": "@dx_hub_fixture_taplo_linux_arm64//:tool",
            ":macos_arm64": "@dx_hub_fixture_taplo_macos_arm64//:tool",
            ":windows_x86_64": "@dx_hub_fixture_taplo_windows_x86_64//:tool",
        }),
        expect_equal("the released artifacts carry valid metadata", artifact_metadata_errors(TOOL_ARTIFACTS), []),
        expect_equal("the released tool map is valid", artifact_map_errors(released), []),
        expect_equal("a decoded map equals the encoded map", decode_artifacts(encode_artifacts(released)).artifacts, released),
        expect_equal("a decoded map has no diagnostic", decode_artifacts(encode_artifacts(released)).error, ""),
        expect_equal("a JSON array is not a tool map", decode_artifacts('["taplo"]').error, "artifacts must be a JSON object keyed by tool, got list"),
        expect_equal("one repository cannot serve two platforms", artifact_map_errors({"taplo": {"linux_x86_64": "dx_a", "macos_arm64": "dx_a"}}), ["repository 'dx_a' is selected by both 'taplo/linux_x86_64' and 'taplo/macos_arm64'"]),
        expect_equal("an unsupported platform key is rejected", artifact_map_errors({"taplo": {"solaris_x86_64": "dx_a"}}), ["tool 'taplo' names unsupported platform 'solaris_x86_64'; want one of linux_x86_64, linux_arm64, macos_arm64, windows_x86_64"]),
        expect_equal("a platform without a repository is rejected", artifact_map_errors({"taplo": {"macos_arm64": ""}}), ["tool 'taplo' platform 'macos_arm64' has no repository name"]),
        expect_equal("a tool without a platform map is rejected", artifact_map_errors({"taplo": ["dx_a"]}), ["tool 'taplo' maps platforms to a list, want an object keyed by platform"]),
        expect_equal("a tool without an artifact is rejected", artifact_map_errors({"taplo": {}}), ["tool 'taplo' has no artifact for any platform"]),
        expect_equal("an artifact without metadata is rejected", artifact_metadata_errors([dict(sample, executable_sha256 = "")]), ["artifact '" + sample_tool + "' is missing executable_sha256"]),
        expect_equal("an artifact without an archive format is rejected", artifact_metadata_errors([dict(sample, archive = {})]), ["artifact '" + sample_tool + "' is missing archive.format"]),
        expect_equal("an artifact on an unsupported platform is rejected", artifact_metadata_errors([dict(sample, os = "solaris")]), ["artifact '" + sample_tool + "' names unsupported platform 'solaris_" + sample["cpu"] + "'; want one of linux_x86_64, linux_arm64, macos_arm64, windows_x86_64"]),
        expect_equal("two artifacts for one platform are rejected", artifact_metadata_errors([sample, dict(sample, url = "https://example.invalid/other")]), ["artifact '" + sample_tool + "/" + sample_platform + "' is declared twice: '" + sample["url"] + "' and 'https://example.invalid/other'"]),
    ]
    for platform in TOOL_PLATFORMS:
        checks.append(expect_contains("the rendered hub declares the " + platform + " config_setting", rendered, '    name = "%s",' % platform))
    for tool in sorted(released.keys()):
        checks.append(expect_equal(tool + " keeps one branch per supported platform", ordered_platforms(released[tool]), TOOL_PLATFORMS))
        checks.append(expect_equal(tool + " keeps the repository of every platform", branch_labels(released[tool]), _platform_branches(tool)))
        checks.append(expect_contains(tool + " renders its linux_x86_64 branch", rendered, '":linux_x86_64": "@dx_%s_linux_x86_64//:tool",' % tool))
    return checks

def _platform_transition_impl(_settings, attr):
    """Selects the fixture platform named by another attribute."""
    return {"//command_line_option:platforms": str(attr.platform_target)}

_platform_transition = transition(
    implementation = _platform_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _dep_files(dep):
    """Returns the files behind one transitioned label attribute."""
    targets = dep if type(dep) == "list" else [dep]
    files = []
    for target in targets:
        files.extend(target[DefaultInfo].files.to_list())
    return files

def _hub_branch_probe_impl(ctx):
    """Fails unless the configured platform selects the expected tool repository."""
    got = sorted([f.short_path for f in _dep_files(ctx.attr.branch)])
    want = sorted([f.short_path for f in _dep_files(ctx.attr.expected)])
    if got != want:
        fail("hub selection: " + ctx.attr.platform + " selected " + str(got) + ", want " + str(want))
    return [DxSubjectInfo(fields = {
        "branch": ",".join(got),
        "platform": ctx.attr.platform,
    })]

hub_branch_probe = rule(
    implementation = _hub_branch_probe_impl,
    attrs = {
        "branch": attr.label(cfg = _platform_transition, mandatory = True),
        "expected": attr.label(mandatory = True),
        "platform": attr.string(mandatory = True),
        "platform_target": attr.label(mandatory = True),
    },
)

def _selection_fixture(fixture, tool_map):
    native.alias(
        name = fixture + "_" + _FIXTURE_TOOL,
        actual = select(
            branch_labels(tool_map[_FIXTURE_TOOL]),
            no_match_error = no_match_error(tool_map[_FIXTURE_TOOL], _FIXTURE_TOOL),
        ),
        tags = ["manual"],
        visibility = ["//visibility:public"],
    )

def _hub_exec_middle_impl(ctx):
    """Exposes the execution-resolved tool files behind a target transition."""
    files = _dep_files(ctx.attr.tool)
    return [
        DefaultInfo(files = depset(files)),
        DxSubjectInfo(fields = {
            "exec": ",".join(sorted([f.short_path for f in files])),
            "role": "execution",
        }),
    ]

hub_exec_middle = rule(
    doc = "Resolves one tool in the execution configuration behind a target transition.",
    implementation = _hub_exec_middle_impl,
    attrs = {
        "tool": attr.label(
            cfg = "exec",
            doc = "Tool alias resolved for the execution platform.",
            mandatory = True,
        ),
    },
)

def _hub_exec_probe_impl(ctx):
    """Fails unless the execution tool ignores the Windows target transition."""
    target = sorted([f.short_path for f in _dep_files(ctx.attr.target)])
    expected = sorted([f.short_path for f in _dep_files(ctx.attr.expected)])
    if target != expected:
        fail("hub execution selection: " + ctx.attr.platform + " target selected " + str(target) + ", want " + str(expected))
    middle = sorted([f.short_path for f in _dep_files(ctx.attr.middle)])
    control = sorted([f.short_path for f in _dep_files(ctx.attr.control)])
    if middle != control:
        fail("hub execution selection: execution tool followed the " + ctx.attr.platform + " target (" + str(middle) + "), want execution selection " + str(control))
    return [DxSubjectInfo(fields = {
        "exec": ",".join(middle),
        "platform": ctx.attr.platform,
        "target": ",".join(target),
    })]

hub_exec_probe = rule(
    doc = "Checks execution tool selection stays off the user target platform.",
    implementation = _hub_exec_probe_impl,
    attrs = {
        "control": attr.label(
            cfg = "exec",
            doc = "Tool alias resolved for the execution platform without a transition.",
            mandatory = True,
        ),
        "expected": attr.label(
            doc = "Tool repository files the transitioned target must select.",
            mandatory = True,
        ),
        "middle": attr.label(
            cfg = _platform_transition,
            doc = "Middle rule resolving the tool for the execution platform.",
            mandatory = True,
        ),
        "platform": attr.string(
            doc = "Transitioned target platform key under test.",
            mandatory = True,
        ),
        "platform_target": attr.label(
            doc = "Platform the target transition selects.",
            mandatory = True,
        ),
        "target": attr.label(
            cfg = _platform_transition,
            doc = "Tool alias resolved for the transitioned target platform.",
            mandatory = True,
        ),
    },
)

def hub_selection_tests(name):
    """Declares the standalone tool artifact selection tests."""
    for platform in TOOL_PLATFORMS:
        constraint_values = [_CONSTRAINT_LABELS[attr] for attr in PLATFORM_CONSTRAINTS[platform]]
        native.platform(
            name = "platform_" + platform,
            constraint_values = constraint_values,
            visibility = ["//visibility:public"],
        )
        native.config_setting(
            name = platform,
            constraint_values = constraint_values,
            visibility = ["//visibility:public"],
        )
    subjects = []
    for fixture, tool_map in [("sparse", _SPARSE), ("complete", _COMPLETE)]:
        _selection_fixture(fixture, tool_map)
        for platform in ordered_platforms(tool_map[_FIXTURE_TOOL]):
            probe = fixture + "_" + _FIXTURE_TOOL + "_" + platform
            hub_branch_probe(
                name = probe,
                branch = ":" + fixture + "_" + _FIXTURE_TOOL,
                expected = "@" + tool_map[_FIXTURE_TOOL][platform] + "//:tool",
                platform = platform,
                platform_target = ":platform_" + platform,
            )
            subjects.append(":" + probe)
    hub_exec_middle(
        name = "exec_middle",
        tool = ":complete_" + _FIXTURE_TOOL,
    )
    hub_exec_probe(
        name = "complete_" + _FIXTURE_TOOL + "_exec_windows",
        control = ":complete_" + _FIXTURE_TOOL,
        expected = "@" + _COMPLETE[_FIXTURE_TOOL]["windows_x86_64"] + "//:tool",
        middle = ":exec_middle",
        platform = "windows_x86_64",
        platform_target = ":platform_windows_x86_64",
        target = ":complete_" + _FIXTURE_TOOL,
    )
    subjects.append(":complete_" + _FIXTURE_TOOL + "_exec_windows")
    starlark_test(
        name = name + "_analysis",
        mode = "analysis",
        subjects = subjects,
    )
    starlark_test(
        name = name + "_unit",
        mode = "unit",
        checks = _unit_checks(),
    )
