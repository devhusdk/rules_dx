"""Plans the @dx_prebuilt repos and renders the hub BUILD file."""

ASSET_PLATFORMS = {
    "dx-linux-arm64": ("linux", "aarch64"),
    "dx-linux-x86_64": ("linux", "x86_64"),
    "dx-macos-arm64": ("macos", "aarch64"),
    "dx-windows-arm64": ("windows", "aarch64"),
    "dx-windows-x86_64": ("windows", "x86_64"),
}

KNOWN_OS = ["android", "freebsd", "ios", "linux", "macos", "openbsd", "windows"]

KNOWN_CPU = ["aarch64", "arm", "i386", "ppc64le", "riscv64", "s390x", "x86_32", "x86_64"]

SOURCE_LABEL = "@rules_dx//cli/cli:dx"

# dx runs these tools when started under their names, so a prebuilt dx stands in for each.
SOURCE_TOOLS = {
    "quality_markdown": "@rules_dx//quality/markdown:quality_markdown",
    "quality_notebook": "@rules_dx//quality/notebook:quality_notebook",
    "quality_runner": "@rules_dx//quality/runner:quality_runner",
}

_HEX = "0123456789abcdef"

def valid_sha256(text):
    """Returns whether text is 64 lowercase hex digits."""
    return len(text) == 64 and all([c in _HEX for c in text.elems()])

def plan_root(modules):
    """Reads the module tags: returns struct(error, source, overrides).

    Args:
      modules: Structs with name, is_root and tags.asset / tags.from_source.

    Returns:
      A struct with the first error text (empty when fine), whether dx builds
      from source, and the root module's asset overrides.
    """
    rules_dx_is_root = False
    from_source = False
    overrides = {}
    for mod in modules:
        if mod.name == "rules_dx" and mod.is_root:
            rules_dx_is_root = True
        if not mod.is_root:
            if mod.tags.asset or mod.tags.from_source:
                return struct(error = "dx_prebuilt: only the root module may set asset or from_source tags; {} sets them".format(mod.name), source = False, overrides = {})
            continue
        if mod.tags.from_source:
            from_source = True
        for tag in mod.tags.asset:
            if not valid_sha256(tag.sha256):
                return struct(error = "dx_prebuilt.asset(name = \"{}\"): sha256 must be 64 lowercase hex digits".format(tag.name), source = False, overrides = {})
            overrides[tag.name] = struct(cpu = tag.cpu, os = tag.os, sha256 = tag.sha256, url = tag.url)
    return struct(error = "", source = rules_dx_is_root or from_source, overrides = overrides)

def repo_name(asset):
    """Returns the http_file repo name for a release asset."""
    return "dx_prebuilt_" + "".join([c if c.isalnum() else "_" for c in asset.removeprefix("dx-").elems()])

def _supported(assets):
    return ", ".join(["{} {}".format(a.os, a.cpu) for a in assets])

def plan_assets(version, pins, url_template, overrides):
    """Returns one struct per asset: its platform and, when pinned, a verified download.

    Args:
      version: The dx version the pins belong to.
      pins: Release asset name to sha256; empty means unpinned.
      url_template: Download URL with {version} and {asset}.
      overrides: Asset name to struct(url, sha256, os, cpu) from root module tags.

    Returns:
      A list of structs sorted by asset name.
    """
    names = sorted({name: None for name in list(pins.keys()) + list(overrides.keys())}.keys())
    planned = []
    seen = {}
    for name in names:
        override = overrides.get(name)
        os, cpu = ASSET_PLATFORMS.get(name, ("", ""))
        if override and override.os:
            os = override.os
        if override and override.cpu:
            cpu = override.cpu
        if not os or not cpu:
            fail("dx_prebuilt.asset(name = \"{}\") needs os and cpu: it is not a dx release asset".format(name))
        if os not in KNOWN_OS or cpu not in KNOWN_CPU:
            fail("dx_prebuilt.asset(name = \"{}\"): os must be one of {} and cpu one of {}".format(name, KNOWN_OS, KNOWN_CPU))
        key = "{}_{}".format(os, cpu)
        if key in seen:
            fail("dx_prebuilt: assets {} and {} both target {} {}".format(seen[key], name, os, cpu))
        seen[key] = name
        if override:
            url, sha256 = override.url, override.sha256
        else:
            url, sha256 = url_template.format(version = version, asset = name), pins.get(name, "")
        planned.append(struct(
            asset = name,
            cpu = cpu,
            exe = "dx.exe" if os == "windows" else "dx",
            key = key,
            os = os,
            repo = repo_name(name) if sha256 else "",
            sha256 = sha256,
            url = url if sha256 else "",
        ))
    return planned

def alias_build(actual):
    """Returns the hub BUILD text that points dx at one label and the tools at their sources."""
    lines = []
    for name, label in [("dx", actual)] + sorted(SOURCE_TOOLS.items()):
        lines += [
            "alias(",
            '    name = "{}",'.format(name),
            '    actual = "{}",'.format(label),
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    return "\n".join(lines)

def unsupported_message(assets):
    """Returns the failure text for a platform with no dx asset."""
    return (
        "dx: no prebuilt binary for {os} {cpu}. Prebuilt dx exists for: " + _supported(assets) +
        ". In your root MODULE.bazel, add dx_prebuilt.asset(name, url, sha256, os, cpu) for a dx you built," +
        " or dx_prebuilt.from_source(), which requires Rust and C toolchains."
    )

def _select(entries, default):
    lines = ["select({"]
    for condition, value in entries:
        lines.append('        "{}": "{}",'.format(condition, value))
    lines.append('        "//conditions:default": "{}",'.format(default))
    lines.append("    })")
    return "\n".join(lines)

def consumer_build(assets):
    """Returns the hub BUILD text that selects a prebuilt dx by os and cpu.

    A platform whose asset has no pinned sha256 builds dx from source, as
    before prebuilt dx existed: nothing unverified is ever downloaded, and a
    release with no pins (every version until #1388) keeps working. With no
    pinned asset at all, every platform builds from source, exactly as
    before; unsupported platforms fail only once some asset is pinned.
    The quality tools follow dx: they run the same prebuilt file under
    their own names, and build from source where dx does.

    Args:
      assets: The structs plan_assets returns.

    Returns:
      The BUILD file content.
    """
    if not [a for a in assets if a.repo]:
        return alias_build(SOURCE_LABEL)
    lines = [
        'load("@bazel_skylib//rules:native_binary.bzl", "native_binary")',
        'load("@rules_dx//cli/prebuilt:fail.bzl", "dx_unavailable")',
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
        "alias(",
        '    name = "dx",',
        "    actual = " + _select([(":" + a.key, ":dx_" + a.key if a.repo else SOURCE_LABEL) for a in assets], ":unsupported") + ",",
        ")",
    ]
    for tool, label in sorted(SOURCE_TOOLS.items()):
        lines += [
            "",
            "alias(",
            '    name = "{}",'.format(tool),
            "    actual = " + _select([(":" + a.key, ":{}_{}".format(tool, a.key) if a.repo else label) for a in assets], ":unsupported") + ",",
            ")",
        ]
    for a in assets:
        lines += [
            "",
            "config_setting(",
            '    name = "{}",'.format(a.key),
            "    constraint_values = [",
            '        "@platforms//os:{}",'.format(a.os),
            '        "@platforms//cpu:{}",'.format(a.cpu),
            "    ],",
            ")",
        ]
        if a.repo:
            lines += [
                "",
                "native_binary(",
                '    name = "dx_{}",'.format(a.key),
                '    src = "@{}//file:{}",'.format(a.repo, a.exe),
                '    out = "{}/{}",'.format(a.key, a.exe),
                ")",
            ]
            for tool in sorted(SOURCE_TOOLS.keys()):
                lines += [
                    "",
                    "native_binary(",
                    '    name = "{}_{}",'.format(tool, a.key),
                    '    src = "@{}//file:{}",'.format(a.repo, a.exe),
                    '    out = "{}/{}{}",'.format(a.key, tool, ".exe" if a.os == "windows" else ""),
                    ")",
                ]
    lines += [
        "",
        "dx_unavailable(",
        '    name = "unsupported",',
        "    cpu = " + _select([("@platforms//cpu:" + c, c) for c in KNOWN_CPU], "unknown cpu") + ",",
        "    message = {},".format(json.encode(unsupported_message(assets))),
        "    os = " + _select([("@platforms//os:" + o, o) for o in KNOWN_OS], "unknown os") + ",",
        ")",
        "",
    ]
    return "\n".join(lines)
