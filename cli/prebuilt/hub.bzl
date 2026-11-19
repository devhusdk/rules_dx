"""Plans the @dx_prebuilt repos and renders the hub BUILD file."""

load(":tools.bzl", "DEV_SOURCE_FALLBACK", "JAR_TOOLS", "NATIVE_TOOLS", "RELEASE_CPU", "RELEASE_OS", "TOOLS", "asset_name", "split_key")

visibility(["//cli/..."])

KNOWN_OS = ["android", "freebsd", "ios", "linux", "macos", "openbsd", "windows"]

KNOWN_CPU = ["aarch64", "arm", "i386", "ppc64le", "riscv64", "s390x", "x86_32", "x86_64"]

SOURCE_LABEL = TOOLS["dx"].source

LAUNCHER_SOURCE = TOOLS["launcher"].source

SOURCE_TOOLS = {
    "codegen_shard_writer": "@rules_dx//cli/codegen_shard:codegen_shard_writer",
    "collect_native": "@rules_dx//tools/coverage/collect:collect_native",
    "deploy_tool": "@rules_dx//deploy/private:deploy_tool",
    "env_shard_writer": "@rules_dx//core/env/private/tools/env_shard:env_shard_writer",
    "lsp_check": "@rules_dx//core/quality/lsp_check:lsp_check",
    "quality_markdown": "@rules_dx//core/quality/markdown:quality_markdown",
    "quality_notebook": "@rules_dx//core/quality/notebook:quality_notebook",
    "quality_runner": "@rules_dx//core/quality/runner:quality_runner",
}

_GO_PACKAGE = "@rules_dx//go/private/tools/analysis/dx_go:"

# The names the dx_go multicall runs as, each with its source build.
GO_TOOLS = {tool: _GO_PACKAGE + tool for tool in [
    "dx_errcheck",
    "dx_godoc_json",
    "dx_gofumpt",
    "dx_gotype",
    "dx_govet",
    "dx_modfmt",
    "dx_staticcheck",
]}

PYTHON_TOOLS = {
    "ansible_lint_tool": "@rules_dx//core/quality/tools/python:ansible_lint_tool",
    "cfn_lint": "@rules_dx//core/quality/tools/python:cfn_lint",
    "cmakelint": "@rules_dx//core/quality/tools/python:cmakelint",
    "djlint": "@rules_dx//core/quality/tools/python:djlint",
    "docstrfmt": "@rules_dx//core/quality/tools/python:docstrfmt",
    "mbake": "@rules_dx//core/quality/tools/python:mbake",
    "sqlfluff": "@rules_dx//core/quality/tools/python:sqlfluff",
    "yamllint": "@rules_dx//core/quality/tools/python:yamllint",
}

_HEX = "0123456789abcdef"

def valid_sha256(text):
    """Returns whether text is 64 lowercase hex digits."""
    return len(text) == 64 and all([c in _HEX for c in text.elems()])

def _override_key(tag):
    return "any" if tag.name in JAR_TOOLS else "{}_{}".format(tag.os, tag.cpu)

def plan_root(modules, ext = "dx_prebuilt", tools = ["dx"]):
    """Reads the module tags: returns struct(error, source, overrides), overrides keyed by tool, then "<os>_<cpu>" or "any" for a jar tool."""
    rules_dx_is_root = False
    from_source = False
    overrides = {}
    for mod in modules:
        if mod.name == "rules_dx" and mod.is_root:
            rules_dx_is_root = True
        if not mod.is_root:
            if mod.tags.asset or mod.tags.from_source:
                return struct(error = ext + ": only the root module may set asset or from_source tags; {} sets them".format(mod.name), source = False, overrides = {})
            continue
        if mod.tags.from_source:
            from_source = True
        for tag in mod.tags.asset:
            where = ext + ".asset(name = \"{}\", os = \"{}\", cpu = \"{}\")".format(tag.name, tag.os, tag.cpu)
            error = ""
            if tag.name not in tools:
                error = where + ": name is one of the tools {}".format(json.encode(tools))
            elif tag.name in JAR_TOOLS and (tag.os != "any" or tag.cpu != "any"):
                error = where + ": a jar tool takes os = \"any\" and cpu = \"any\""
            elif tag.name not in JAR_TOOLS and (tag.os not in KNOWN_OS or tag.cpu not in KNOWN_CPU):
                error = where + ": os must be one of {} and cpu one of {}".format(KNOWN_OS, KNOWN_CPU)
            elif not valid_sha256(tag.sha256):
                error = where + ": sha256 must be 64 lowercase hex digits"
            elif _override_key(tag) in overrides.get(tag.name, {}):
                error = where + ": a second asset for the same os and cpu"
            if error:
                return struct(error = error, source = False, overrides = {})
            overrides.setdefault(tag.name, {})[_override_key(tag)] = struct(cpu = tag.cpu, os = tag.os, sha256 = tag.sha256, url = tag.url)
    return struct(error = "", source = rules_dx_is_root or from_source, overrides = overrides)

def _supported(assets):
    return ", ".join(["{} {}".format(a.os, a.cpu) for a in assets])

def _planned(os, cpu, exe, repo_prefix, asset, sha256, url):
    key = "{}_{}".format(os, cpu)
    return struct(
        asset = asset,
        cpu = cpu,
        exe = exe + ".exe" if os == "windows" else exe,
        key = key,
        os = os,
        repo = repo_prefix + key if sha256 else "",
        sha256 = sha256,
        url = url if sha256 else "",
    )

def plan_assets(tool, version, pins, url_template, overrides, exe = "dx", repo_prefix = "dx_prebuilt_"):
    """Returns one struct per platform of tool, sorted by key: its platform and, when pinned, a verified download.

    pins is pins.bzl's PINS; overrides comes from plan_root. A platform with no pin
    takes its registry fallback's download, and failing that builds from source.
    """
    entry = TOOLS[tool]
    tool_pins = pins.get(tool, {})
    planned = {}
    for release_key in entry.platforms:
        os, cpu = split_key(release_key)
        os, cpu = RELEASE_OS[os], RELEASE_CPU[cpu]
        asset = asset_name(tool, release_key)
        sha256 = tool_pins.get(release_key, "")
        planned["{}_{}".format(os, cpu)] = _planned(os, cpu, exe, repo_prefix, asset, sha256, url_template.format(version = version, asset = asset))
    for key, override in overrides.items():
        planned[key] = _planned(override.os, override.cpu, exe, repo_prefix, "", override.sha256, override.url)
    for release_key, fallback_key in entry.fallbacks.items():
        os, cpu = split_key(release_key)
        os, cpu = RELEASE_OS[os], RELEASE_CPU[cpu]
        key = "{}_{}".format(os, cpu)
        own = planned.get(key) or _planned(os, cpu, exe, repo_prefix, "", "", "")
        fallback_os, fallback_cpu = split_key(fallback_key)
        fallback = planned["{}_{}".format(RELEASE_OS[fallback_os], RELEASE_CPU[fallback_cpu])]
        if not own.repo and fallback.repo:
            # Same download, so the same repo: http_file runs once for both platforms.
            own = struct(
                asset = fallback.asset,
                cpu = own.cpu,
                exe = own.exe,
                key = key,
                os = own.os,
                repo = fallback.repo,
                sha256 = fallback.sha256,
                url = fallback.url,
            )
        if own.repo or key in planned:
            planned[key] = own
    return [planned[key] for key in sorted(planned.keys())]

def plan_jars(version, pins, url_template, overrides = {}):
    """Returns one struct per jar tool, sorted by tool: its source and, when pinned or overridden, a verified download."""
    planned = []
    for tool in JAR_TOOLS:
        asset = asset_name(tool, "any")
        sha256 = pins.get(tool, {}).get("any", "")
        url = url_template.format(version = version, asset = asset)
        override = overrides.get(tool, {}).get("any")
        if override:
            sha256, url = override.sha256, override.url
        planned.append(struct(
            asset = asset,
            main_class = TOOLS[tool].main_class,
            repo = "dx_prebuilt_" + tool if sha256 else "",
            sha256 = sha256,
            source = TOOLS[tool].source,
            tool = tool,
            url = url if sha256 else "",
        ))
    return planned

def with_jars(build, jars):
    """Returns the hub BUILD text build plus one target per jar tool: the pinned jar on the JVM, else its source."""
    loads = []
    lines = []
    for jar in jars:
        if not jar.repo:
            lines += [
                "",
                "alias(",
                '    name = "{}",'.format(jar.tool),
                '    actual = "{}",'.format(jar.source),
                '    visibility = ["//visibility:public"],',
                ")",
            ]
            continue
        loads = ['load("@rules_java//java:defs.bzl", "java_binary", "java_import")']
        lines += [
            "",
            "java_import(",
            '    name = "{}_jar",'.format(jar.tool),
            '    jars = ["@{}//file:{}"],'.format(jar.repo, jar.asset),
            ")",
            "",
            "java_binary(",
            '    name = "{}",'.format(jar.tool),
            '    main_class = "{}",'.format(jar.main_class),
            '    visibility = ["//visibility:public"],',
            '    runtime_deps = [":{}_jar"],'.format(jar.tool),
            ")",
        ]
    return "\n".join(loads + [build.rstrip("\n")] + lines + [""])

def plan_native_tools(version, pins, url_template, overrides):
    """Returns {tool: plan_assets of tool} for every native tool of the registry."""
    return {
        tool: plan_assets(tool, version, pins, url_template, overrides.get(tool, {}), exe = tool, repo_prefix = "dx_{}_prebuilt_".format(tool))
        for tool in NATIVE_TOOLS
    }

_NATIVE_BINARY_LOAD = 'load("@bazel_skylib//rules:native_binary.bzl", "native_binary")'

def with_native_tools(build, native_tools, consumer = False):
    """Returns the hub BUILD text build plus one target per native tool.

    A tool selects its download on the platform it runs on and builds from source on
    any platform with no download. In a consumer hub a tool in DEV_SOURCE_FALLBACK
    serves its fallback label there instead of its source build.
    """
    lines = []
    keys = {}
    for tool, assets in sorted(native_tools.items()):
        pinned = [a for a in assets if a.repo]
        source = DEV_SOURCE_FALLBACK.get(tool, TOOLS[tool].source) if consumer else TOOLS[tool].source
        if not pinned:
            actual = '"{}"'.format(source)
        else:
            actual = _select([(":native_" + a.key, ":{}_{}".format(tool, a.key)) for a in pinned], source)
        lines += [
            "",
            "alias(",
            '    name = "{}",'.format(tool),
            "    actual = " + actual + ",",
            '    visibility = ["//visibility:public"],',
            ")",
        ]
        for a in pinned:
            keys[a.key] = a
            lines += [
                "",
                "native_binary(",
                '    name = "{}_{}",'.format(tool, a.key),
                '    src = "@{}//file:{}",'.format(a.repo, a.exe),
                '    out = "{}/{}",'.format(a.key, a.exe),
                ")",
            ]
    for key, a in sorted(keys.items()):
        lines += [
            "",
            "config_setting(",
            '    name = "native_{}",'.format(key),
            "    constraint_values = [",
            '        "@platforms//os:{}",'.format(a.os),
            '        "@platforms//cpu:{}",'.format(a.cpu),
            "    ],",
            ")",
        ]
    loads = [_NATIVE_BINARY_LOAD] if keys and _NATIVE_BINARY_LOAD not in build else []
    return "\n".join(loads + [build.rstrip("\n")] + lines + [""])

def alias_build(actual):
    """Returns the hub BUILD text that points dx at one label and the tools and launcher at their sources."""
    lines = []
    for name, label in [("dx", actual), ("launcher", LAUNCHER_SOURCE)] + sorted(SOURCE_TOOLS.items()) + sorted(GO_TOOLS.items()):
        lines += [
            "alias(",
            '    name = "{}",'.format(name),
            '    actual = "{}",'.format(label),
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    lines += _python_tool_aliases()
    return "\n".join(lines)

def _python_tool_aliases():
    lines = []
    for name, label in sorted(PYTHON_TOOLS.items()):
        lines += [
            "alias(",
            '    name = "python_tool_{}",'.format(name),
            '    actual = "{}",'.format(label),
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    return lines

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

def consumer_build(assets, launcher_assets = [], go_assets = []):
    """Returns the hub BUILD text that selects a prebuilt dx, launcher and Go lint tools by os and cpu.

    The launcher and Go tools select on the platform of the configuration that
    depends on them; a platform with no download builds them from source.
    """
    if not [a for a in assets + launcher_assets + go_assets if a.repo]:
        return alias_build(SOURCE_LABEL)
    python_tools = _python_tool_aliases()
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
        "",
        "alias(",
        '    name = "launcher",',
        "    actual = " + _select([(":" + a.key, ":launcher_" + a.key) for a in launcher_assets if a.repo], LAUNCHER_SOURCE) + ",",
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
    for tool, label in sorted(GO_TOOLS.items()):
        lines += [
            "",
            "alias(",
            '    name = "{}",'.format(tool),
            "    actual = " + _select([(":" + a.key, ":{}_{}".format(tool, a.key)) for a in go_assets if a.repo], label) + ",",
            ")",
        ]
    platforms = {a.key: a for a in launcher_assets + go_assets}
    platforms.update({a.key: a for a in assets})
    for key, a in sorted(platforms.items()):
        lines += [
            "",
            "config_setting(",
            '    name = "{}",'.format(key),
            "    constraint_values = [",
            '        "@platforms//os:{}",'.format(a.os),
            '        "@platforms//cpu:{}",'.format(a.cpu),
            "    ],",
            ")",
        ]
    for a in launcher_assets:
        if a.repo:
            lines += [
                "",
                "native_binary(",
                '    name = "launcher_{}",'.format(a.key),
                '    src = "@{}//file:{}",'.format(a.repo, a.exe),
                '    out = "{}/{}",'.format(a.key, a.exe),
                ")",
            ]
    for a in go_assets:
        if a.repo:
            for tool in sorted(GO_TOOLS.keys()):
                lines += [
                    "",
                    "native_binary(",
                    '    name = "{}_{}",'.format(tool, a.key),
                    '    src = "@{}//file:{}",'.format(a.repo, a.exe),
                    '    out = "{}/{}{}",'.format(a.key, tool, ".exe" if a.os == "windows" else ""),
                    ")",
                ]
    for a in assets:
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
    lines += [""] + python_tools[:-1]
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
