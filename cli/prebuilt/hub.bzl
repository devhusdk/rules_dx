"""Plans the @dx_prebuilt repos and renders the hub BUILD file."""

load(":tools.bzl", "JAR_TOOLS", "RELEASE_CPU", "RELEASE_OS", "TOOLS", "asset_name", "split_key")

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

def plan_root(modules, ext = "dx_prebuilt", tools = ["dx"]):
    """Reads the module tags: returns struct(error, source, overrides), overrides keyed by tool, then "<os>_<cpu>"."""
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
            elif tag.os not in KNOWN_OS or tag.cpu not in KNOWN_CPU:
                error = where + ": os must be one of {} and cpu one of {}".format(KNOWN_OS, KNOWN_CPU)
            elif not valid_sha256(tag.sha256):
                error = where + ": sha256 must be 64 lowercase hex digits"
            elif "{}_{}".format(tag.os, tag.cpu) in overrides.get(tag.name, {}):
                error = where + ": a second asset for the same os and cpu"
            if error:
                return struct(error = error, source = False, overrides = {})
            overrides.setdefault(tag.name, {})["{}_{}".format(tag.os, tag.cpu)] = struct(cpu = tag.cpu, os = tag.os, sha256 = tag.sha256, url = tag.url)
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
        key = "{}_{}".format(RELEASE_OS[os], RELEASE_CPU[cpu])
        os, cpu = split_key(fallback_key)
        fallback = planned["{}_{}".format(RELEASE_OS[os], RELEASE_CPU[cpu])]
        if not planned[key].repo and fallback.repo:
            # Same download, so the same repo: http_file runs once for both platforms.
            planned[key] = struct(
                asset = fallback.asset,
                cpu = planned[key].cpu,
                exe = planned[key].exe,
                key = key,
                os = planned[key].os,
                repo = fallback.repo,
                sha256 = fallback.sha256,
                url = fallback.url,
            )
    return [planned[key] for key in sorted(planned.keys())]

def plan_jars(version, pins, url_template):
    """Returns one struct per jar tool, sorted by tool: its source and, when pinned, a verified download."""
    planned = []
    for tool in JAR_TOOLS:
        asset = asset_name(tool, "any")
        sha256 = pins.get(tool, {}).get("any", "")
        planned.append(struct(
            asset = asset,
            main_class = TOOLS[tool].main_class,
            repo = "dx_prebuilt_" + tool if sha256 else "",
            sha256 = sha256,
            source = TOOLS[tool].source,
            tool = tool,
            url = url_template.format(version = version, asset = asset) if sha256 else "",
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

def alias_build(actual):
    """Returns the hub BUILD text that points dx at one label and the tools and launcher at their sources."""
    lines = []
    for name, label in [("dx", actual), ("launcher", LAUNCHER_SOURCE)] + sorted(SOURCE_TOOLS.items()):
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

def consumer_build(assets, launcher_assets = []):
    """Returns the hub BUILD text that selects a prebuilt dx and launcher by os and cpu.

    The launcher selects on the platform of the configuration that depends on it;
    a platform with no launcher download builds it from source.
    """
    if not [a for a in assets + launcher_assets if a.repo]:
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
    platforms = {a.key: a for a in launcher_assets}
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
