"""Unit and analysis tests for platform names, platform_select, the hub and platform_transition."""

load("//core:failure_test.bzl", "failure_test_rule")
load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")
load("//platforms:defs.bzl", "PLATFORM_NAMES", "platform_select")
load("//platforms/private:extension.bzl", "registration_error")
load("//platforms/private:hub.bzl", "hub_entries", "render_hub")
load("//platforms/private:names.bzl", "full_name", "host_name", "matches", "platforms_error")
load("//platforms/private:select.bzl", "os_label", "resolve")
load("//platforms/private:target.bzl", "current_error", "min_os_error", "move_to", "transition_error")
load(":subject.bzl", "moved_subject", "platform_subject")

_FAMILY = "@rules_dx//platforms:"

_unknown_current_test = failure_test_rule(config_settings = {str(Label("//platforms:current")): "android"})

_SIM_TAG = {
    "constraint_values": ["@platforms//os:ios", "@platforms//cpu:arm64", "@apple_support//constraints:simulator"],
    "hosts": ["macos_arm64"],
    "module": "rules_dx_apple",
    "parent": "@apple_support//platforms:ios_sim_arm64",
}

def names_unit_tests(name, size):
    """Checks host names, entry matching and platforms list errors."""
    starlark_test(
        name = name,
        size = size,
        mode = "unit",
        checks = [
            expect_equal("linux amd64 host", host_name("Linux", "amd64"), "linux_x86_64"),
            expect_equal("linux aarch64 host", host_name("linux", "aarch64"), "linux_arm64"),
            expect_equal("macos arm host", host_name("Mac OS X", "aarch64"), "macos_arm64"),
            expect_equal("windows host", host_name("Windows 11", "amd64"), "windows_x86_64"),
            expect_equal("unknown host", host_name("freebsd", "amd64"), ""),
            expect_equal("full name matches itself", matches("ios_arm64", "ios_arm64"), True),
            expect_equal("os name matches its cpus", matches("android", "android_x86_64"), True),
            expect_equal("os name skips other os", matches("linux", "macos_arm64"), False),
            expect_equal("full name skips its sibling", matches("ios_arm64", "ios_sim_arm64"), False),
            expect_equal("full name is itself", full_name("ios_sim_arm64", []), "ios_sim_arm64"),
            expect_equal("os name takes its default", [full_name(os, []) for os in ["android", "ios", "linux", "macos", "web", "windows"]], [
                "android_arm64",
                "ios_arm64",
                "linux_x86_64",
                "macos_arm64",
                "web_wasm32",
                "windows_x86_64",
            ]),
            expect_equal("os name takes its listed name", full_name("ios", ["ios", "macos_arm64", "ios_sim_arm64"]), "ios_sim_arm64"),
            expect_equal("valid native list", platforms_error(["ios_arm64", "ios_sim_arm64"], True), ""),
            expect_equal("os name on a jvm rule", platforms_error(["android"], False), ""),
            expect_equal(
                "os name on a native rule",
                platforms_error(["android"], True),
                "android needs a cpu: android_arm64 or android_x86_64",
            ),
            expect_equal("duplicate entry", platforms_error(["linux_arm64", "linux_arm64"], False), "platforms lists linux_arm64 twice"),
            expect_equal(
                "unknown entry",
                platforms_error(["linux_amd64"], False).startswith("unknown platform linux_amd64: use one of android, android_arm64,"),
                True,
            ),
        ],
    )

def select_unit_tests(name, size):
    """Checks which platform_select key wins for each name."""
    resolved = resolve({"apple": "a", "default": "d", "ios": "i", "ios_sim_arm64": "s", "windows": "w"})
    wasm = resolve({"wasm": "wasm", "web": "web"})
    web_names = resolve({"web": "os", "web_wasm32": "full"})
    starlark_test(
        name = name,
        size = size,
        mode = "unit",
        checks = [
            expect_equal("full name keys use the hub", resolved.names, {"ios_sim_arm64": "s"}),
            expect_equal("os beats group", resolved.oses["ios"], "i"),
            expect_equal("group covers macos", resolved.oses["macos"], "a"),
            expect_equal("os key", resolved.oses["windows"], "w"),
            expect_equal("default is left to select", "linux" in resolved.oses, False),
            expect_equal("web os beats wasm group", wasm.oses["web"], "web"),
            expect_equal("wasm group covers wasip1", wasm.oses["wasip1"], "wasm"),
            expect_equal("full names that cover an os drop its arm", [web_names.names, web_names.oses], [{"web_wasm32": "full"}, {}]),
            expect_equal("posix group", sorted(resolve({"posix": 1}).oses.keys()), ["android", "ios", "linux", "macos"]),
            expect_equal("os arm label", str(os_label("linux")).endswith("//platforms:os_linux"), True),
        ],
    )

def hub_unit_tests(name, size):
    """Checks the hub entries for core, stub, registered and wrong-host names, and tag errors."""
    stubs = hub_entries({}, "linux_x86_64")
    real = hub_entries({"ios_sim_arm64": _SIM_TAG}, "macos_arm64")
    wrong_host = hub_entries({"ios_sim_arm64": _SIM_TAG}, "linux_x86_64")
    no_parent = hub_entries({"android_arm64": _SIM_TAG | {"parent": "", "constraint_values": ["@platforms//os:android", "@platforms//cpu:arm64"], "hosts": ["linux_x86_64"]}}, "linux_x86_64")
    starlark_test(
        name = name,
        size = size,
        mode = "unit",
        checks = [
            expect_equal("core name", stubs["linux_arm64"].platform, ["@platforms//os:linux", "@platforms//cpu:arm64"]),
            expect_equal("core name has no parent", stubs["wasip1_wasm32"].parent, ""),
            expect_equal("stub apple name", stubs["ios_arm64"].platform, ["@platforms//os:ios", "@platforms//cpu:arm64", _FAMILY + "needs_rules_dx_apple"]),
            expect_equal("stub android name", stubs["android_x86_64"].platform[-1], _FAMILY + "needs_rules_dx_android"),
            expect_equal("registered on a listed host", real["ios_sim_arm64"].platform, [_FAMILY + "family_ok"]),
            expect_equal("registered parent", real["ios_sim_arm64"].parent, "@apple_support//platforms:ios_sim_arm64"),
            expect_equal("registered config", real["ios_sim_arm64"].config, _SIM_TAG["constraint_values"]),
            expect_equal("registered on another host", wrong_host["ios_sim_arm64"].platform, [_FAMILY + "needs_macos_host"]),
            expect_equal("other apple names stay stubs", wrong_host["ios_arm64"].platform[-1], _FAMILY + "needs_rules_dx_apple"),
            expect_equal(
                "registered without parent",
                no_parent["android_arm64"].platform,
                ["@platforms//os:android", "@platforms//cpu:arm64", _FAMILY + "family_ok"],
            ),
            expect_equal("one platform and config per name", render_hub({}, "linux_x86_64").count("\nplatform("), 13),
            expect_equal("render names the parent", "parents = [\"@apple_support//platforms:ios_sim_arm64\"]" in render_hub({"ios_sim_arm64": _SIM_TAG}, "macos_arm64"), True),
            expect_equal("tag for a family name", registration_error("ios_arm64", ["macos_arm64"], ["@platforms//os:ios"], "rules_dx_apple", {}), ""),
            expect_equal(
                "tag for a core name",
                registration_error("linux_x86_64", [], ["@platforms//os:ios"], "m", {}),
                "dx.platform: linux_x86_64 is a core platform; only family platforms are registered",
            ),
            expect_equal("tag for an unknown name", registration_error("ios", [], ["@platforms//os:ios"], "m", {}), "dx.platform: unknown platform ios"),
            expect_equal(
                "tag with a family host",
                registration_error("ios_arm64", ["ios_sim_arm64"], ["@platforms//os:ios"], "m", {}),
                "dx.platform: ios_arm64 lists host ios_sim_arm64, which is not a core platform name",
            ),
            expect_equal(
                "tag without constraint_values",
                registration_error("ios_arm64", [], [], "m", {}),
                "dx.platform: ios_arm64 needs constraint_values, which platform_select matches on",
            ),
            expect_equal(
                "tag registered twice",
                registration_error("ios_sim_arm64", [], ["@platforms//os:ios"], "other", {"ios_sim_arm64": _SIM_TAG}),
                "dx.platform: ios_sim_arm64 is registered by both rules_dx_apple and other",
            ),
        ],
    )

def transition_unit_tests(name, size):
    """Checks when platform_transition moves a target and which min_os keys it takes."""
    starlark_test(
        name = name,
        size = size,
        mode = "unit",
        checks = [
            expect_equal("no platforms stays", move_to([], "ios_arm64"), ""),
            expect_equal("listed current stays", move_to(["ios_arm64", "ios_sim_arm64"], "ios_sim_arm64"), ""),
            expect_equal("os entry covers current", move_to(["android"], "android_x86_64"), ""),
            expect_equal("os-only entry moves to its first name", move_to(["android"], "linux_x86_64"), "android_arm64"),
            expect_equal("os-only entry moves from an unknown host", move_to(["android"], ""), "android_arm64"),
            expect_equal("os-only entry takes its listed name", move_to(["android", "android_x86_64"], "linux_x86_64"), "android_x86_64"),
            expect_equal("os-only web entry", move_to(["web", "linux"], "windows_x86_64"), "web_wasm32"),
            expect_equal("empty current is valid", current_error(""), ""),
            expect_equal("full current is valid", current_error("ios_sim_arm64"), ""),
            expect_equal(
                "os-only current",
                current_error("android"),
                "--@rules_dx//platforms:current=android is not a platform name: use one of " + ", ".join(PLATFORM_NAMES),
            ),
            expect_equal("unknown current name does not match", matches("linux", "freebsd_x86_64"), False),
            expect_equal("os-only linux entry moves to x86_64 from macos", move_to(["linux"], "macos_arm64"), "linux_x86_64"),
            expect_equal("os-only windows entry moves to x86_64 from linux", move_to(["windows"], "linux_x86_64"), "windows_x86_64"),
            expect_equal(
                "unknown entry fails the transition with the list error",
                transition_error("", ["linux_amd64"], {}).startswith("unknown platform linux_amd64: use one of android, android_arm64,"),
                True,
            ),
            expect_equal("duplicate entry fails the transition", transition_error("", ["web", "web"], {}), "platforms lists web twice"),
            expect_equal("valid transition inputs", transition_error("", ["android", "linux_arm64"], {"android": "29"}), ""),
            expect_equal("unlisted current moves to the first", move_to(["ios_arm64", "ios_sim_arm64"], "macos_arm64"), "ios_arm64"),
            expect_equal("valid min_os", min_os_error({"android": "29", "ios": "17.0"}), ""),
            expect_equal("unknown min_os", min_os_error({"tvos": "1"}), "min_os: unknown OS tvos: use one of android, ios, macos, windows"),
        ],
    )

_SELECT = {
    "wasm": "wasm",
    "web": "web",
    "web_wasm32": "web_wasm32",
    "default": "host",
}

def transition_analysis_tests(name, size):
    """Moves subjects with platform_transition and reads the flags and a platform_select value."""
    moved_subject(name = name + "_stay", value = platform_select(_SELECT))
    moved_subject(name = name + "_os_only", platforms = ["linux", "macos", "windows"], value = platform_select(_SELECT))
    moved_subject(name = name + "_wasi", platforms = ["wasip1_wasm32"], value = platform_select(_SELECT))
    moved_subject(name = name + "_web", platforms = ["web_wasm32"], value = platform_select(_SELECT))
    moved_subject(name = name + "_min_os", min_os = {"android": "29"}, value = platform_select(_SELECT))
    moved_subject(name = name + "_android", platforms = ["android"], value = platform_select(_SELECT))
    moved_subject(name = name + "_web_os", platforms = ["web"], value = platform_select(_SELECT))

    # The host is linux, macos or windows, so an OS key matches it whatever its CPU.
    moved_subject(name = name + "_os_key", value = platform_select({"linux": "os", "macos": "os", "windows": "os"}))
    subjects = ["android", "min_os", "os_key", "os_only", "stay", "wasi", "web", "web_os"]
    observations = []
    for subject, fields in [
        ("android", {"current": "android_arm64", "min_os_android": "", "value": "host"}),
        ("min_os", {"current": "", "min_os_android": "29", "value": "host"}),
        ("os_key", {"current": "", "min_os_android": "", "value": "os"}),
        ("os_only", {"current": "", "min_os_android": "", "value": "host"}),
        ("stay", {"current": "", "min_os_android": "", "value": "host"}),
        ("wasi", {"current": "wasip1_wasm32", "min_os_android": "", "value": "wasm"}),
        ("web", {"current": "web_wasm32", "min_os_android": "", "value": "web_wasm32"}),
        ("web_os", {"current": "web_wasm32", "min_os_android": "", "value": "web_wasm32"}),
    ]:
        label = "//platforms/tests:{}_{}".format(name, subject)
        observations.append("subject " + label)
        for key in ["current", "min_os_android", "runner", "value"]:
            observations.append("field {}={}".format(key, fields.get(key, "None")))
        observations += [
            "aspect_field aspect_seen=True",
            "aspect_field field_count=4",
            "aspect_field has_subject=True",
            "aspect_field subject_label=" + label,
            "aspect_field transitive_count=0",
        ]
    starlark_test(
        name = name,
        size = size,
        mode = "analysis",
        subjects = [":{}_{}".format(name, subject) for subject in subjects],
        expected_observations = "\n".join(observations),
    )

def unknown_current_tests(name, size):
    """Checks that the current flag names the valid platforms when it holds an unknown name."""
    _unknown_current_test(
        name = name,
        size = size,
        target_under_test = "//platforms:current",
        expected_failure_substring = "invalid value 'android'. Allowed values are [\"\", \"android_arm64\", \"android_x86_64\",",
    )

def select_analysis_tests(name, size):
    """Resolves platform_select on a linux platform rules_dx has no name for and on a named one."""
    riscv = str(Label(":linux_riscv64"))
    linux = str(Label("//platforms:linux_x86_64"))
    cases = [
        ("riscv_no_default", riscv, {"linux": "linux", "windows": "windows"}, "linux"),
        ("riscv_default", riscv, {"default": "default", "linux": "linux"}, "linux"),
        ("riscv_group", riscv, {"posix": "posix", "wasm": "wasm"}, "posix"),
        ("full_name_beats_os", linux, {"default": "default", "linux": "linux", "linux_x86_64": "x86_64"}, "x86_64"),
        ("os_beats_sibling", linux, {"linux": "linux", "linux_arm64": "arm64"}, "linux"),
    ]
    observations = []
    for case, platform, branches, value in sorted(cases):
        platform_subject(name = "{}_{}".format(name, case), platform = platform, value = platform_select(branches))
        label = "//platforms/tests:{}_{}".format(name, case)
        observations += [
            "subject " + label,
            "field value=" + value,
            "aspect_field aspect_seen=True",
            "aspect_field field_count=1",
            "aspect_field has_subject=True",
            "aspect_field subject_label=" + label,
            "aspect_field transitive_count=0",
        ]
    starlark_test(
        name = name,
        size = size,
        mode = "analysis",
        subjects = [":{}_{}".format(name, case[0]) for case in cases],
        expected_observations = "\n".join(observations),
    )
