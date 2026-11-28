"""Plans the @dx_prebuilt repos and renders the hub BUILD file."""

load(":tools.bzl", "DEV_SOURCE_FALLBACK", "DX_SUBCOMMANDS", "JAR_TOOLS", "NATIVE_TOOLS", "PYTHON_BUNDLE_TOOLS", "RELEASE_CPU", "RELEASE_OS", "TOOLS", "asset_name", "deploy_jar", "split_key")

visibility(["//cli/..."])

KNOWN_OS = ["android", "freebsd", "ios", "linux", "macos", "openbsd", "windows"]

KNOWN_CPU = ["aarch64", "arm", "i386", "ppc64le", "riscv64", "s390x", "x86_32", "x86_64"]

SOURCE_LABEL = TOOLS["dx"].source

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

GO_TOOLS = {tool: _GO_PACKAGE + tool for tool in [
    "dx_errcheck",
    "dx_godoc_json",
    "dx_gofumpt",
    "dx_gotype",
    "dx_govet",
    "dx_modfmt",
    "dx_staticcheck",
]}

GUARD_VIEW = "@rules_dx//cli/prebuilt:guard_view_set"
GUARD_STUB = "@rules_dx//cli/prebuilt:guard_stub"

_HEX = "0123456789abcdef"

def valid_sha256(text):
    """Returns whether text is 64 lowercase hex digits."""
    return len(text) == 64 and all([c in _HEX for c in text.elems()])

def _override_key(tag):
    return "any" if tag.name in JAR_TOOLS else "{}_{}".format(tag.os, tag.cpu)

def plan_root(modules, ext = "dx_prebuilt", tools = ["dx"]):
    """Reads the module tags: returns struct(error, root, source, overrides); source lists the tools that skip the release pins."""
    rules_dx_is_root = False
    source = {}
    overrides = {}
    for mod in modules:
        if mod.name == "rules_dx" and mod.is_root:
            rules_dx_is_root = True
        if not mod.is_root:
            if mod.tags.asset or mod.tags.from_source:
                return struct(error = ext + ": only the root module may set asset or from_source tags; {} sets them".format(mod.name), root = False, source = [], overrides = {})
            continue
        for tag in mod.tags.from_source:
            listed = getattr(tag, "tools", [])
            unknown = [name for name in listed if name not in tools]
            if unknown:
                return struct(error = ext + ".from_source(tools = {}): each tool is one of {}".format(json.encode(unknown), json.encode(tools)), root = False, source = [], overrides = {})
            for name in listed or tools:
                source[name] = True
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
                return struct(error = error, root = False, source = [], overrides = {})
            overrides.setdefault(tag.name, {})[_override_key(tag)] = struct(cpu = tag.cpu, os = tag.os, sha256 = tag.sha256, url = tag.url)
    if rules_dx_is_root:
        return struct(error = "", root = True, source = sorted(tools), overrides = {})
    return struct(error = "", root = False, source = sorted(source), overrides = overrides)

def release_pins(plan, pins):
    """Returns the pins plan may use: none when rules_dx is root, else those of tools not in plan.source."""
    if plan.root:
        return {}
    return {tool: entry for tool, entry in pins.items() if tool not in plan.source}

def plan_hub(plan, version, pins, url_template):
    """Returns struct(downloads, jars, bundles, files): the @dx_prebuilt downloads to declare and the hub files."""
    pins = release_pins(plan, pins)
    assets = plan_assets("dx", version, pins, url_template, plan.overrides.get("dx", {}))
    go_assets = plan_assets("go", version, pins, url_template, plan.overrides.get("go", {}), exe = "dx-go", repo_prefix = "dx_go_prebuilt_")
    jars = plan_jars(version, pins, url_template, plan.overrides)
    native_tools = plan_native_tools(version, pins, url_template, plan.overrides)
    python_tools = plan_python_tools(version, pins, url_template, plan.overrides)
    build = alias_build(SOURCE_LABEL, guard = True) if plan.root else consumer_build(assets, go_assets)
    build = with_native_tools(build, native_tools, consumer = not plan.root, guard = plan.root)
    build = with_python_tools(build, python_tools, guard = plan.root)
    return struct(
        downloads = [a for a in assets + go_assets + [a for tool in sorted(native_tools) for a in native_tools[tool]] if a.repo],
        jars = [jar for jar in jars if jar.repo],
        bundles = [a for tool in sorted(python_tools) for a in python_tools[tool] if a.repo],
        files = {
            "BUILD.bazel": with_jars(build, jars, guard = plan.root),
            "downloads.bzl": downloads_bzl(native_tools),
            "dx/BUILD.bazel": dx_subcommand_build(consumer = not plan.root, guard = plan.root),
        },
    )

def _supported(assets):
    return ", ".join(["{} {}".format(a.os, a.cpu) for a in assets])

def _planned(os, cpu, exe, repo_prefix, asset, sha256, url, library = ""):
    key = "{}_{}".format(os, cpu)
    if library:
        exe = library
    elif os == "windows":
        exe += ".exe"
    return struct(
        asset = asset,
        cpu = cpu,
        exe = exe,
        executable = not library,
        key = key,
        os = os,
        repo = repo_prefix + key if sha256 else "",
        sha256 = sha256,
        url = url if sha256 else "",
    )

def plan_assets(tool, version, pins, url_template, overrides, exe = "dx", repo_prefix = "dx_prebuilt_", library = ""):
    """Returns one struct per platform of tool, sorted by key: its platform and, when pinned, a verified download."""
    entry = TOOLS[tool]
    tool_pins = pins.get(tool, {})
    planned = {}
    for release_key in entry.platforms:
        os, cpu = split_key(release_key)
        os, cpu = RELEASE_OS[os], RELEASE_CPU[cpu]
        asset = asset_name(tool, release_key)
        sha256 = tool_pins.get(release_key, "")
        planned["{}_{}".format(os, cpu)] = _planned(os, cpu, exe, repo_prefix, asset, sha256, url_template.format(version = version, asset = asset), library)
    for key, override in overrides.items():
        planned[key] = _planned(override.os, override.cpu, exe, repo_prefix, "", override.sha256, override.url, library)
    for release_key, fallback_key in entry.fallbacks.items():
        os, cpu = split_key(release_key)
        os, cpu = RELEASE_OS[os], RELEASE_CPU[cpu]
        key = "{}_{}".format(os, cpu)
        own = planned.get(key) or _planned(os, cpu, exe, repo_prefix, "", "", "", library)
        fallback_os, fallback_cpu = split_key(fallback_key)
        fallback = planned["{}_{}".format(RELEASE_OS[fallback_os], RELEASE_CPU[fallback_cpu])]
        if not own.repo and fallback.repo:
            own = struct(
                asset = fallback.asset,
                cpu = own.cpu,
                exe = own.exe,
                executable = own.executable,
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

def _source_actual(source, guard):
    return _select([(GUARD_VIEW, GUARD_STUB)], source) if guard else '"{}"'.format(source)

def with_jars(build, jars, guard = False):
    """Returns the hub BUILD text build plus `<tool>_jvm`, `<tool>` and `<tool>_deploy.jar` per jar tool."""
    loads = []
    lines = []
    for jar in jars:
        if not jar.repo:
            lines += [
                "",
                "alias(",
                '    name = "{}",'.format(jar.tool),
                "    actual = " + _source_actual(jar.source, guard) + ",",
                '    visibility = ["//visibility:public"],',
                ")",
                "",
                "alias(",
                '    name = "{}_deploy.jar",'.format(jar.tool),
                "    actual = " + _source_actual("@rules_dx" + deploy_jar(jar.tool), guard) + ",",
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
            '    name = "{}_jvm",'.format(jar.tool),
            '    main_class = "{}",'.format(jar.main_class),
            '    runtime_deps = [":{}_jar"],'.format(jar.tool),
            ")",
            "",
            "alias(",
            '    name = "{}",'.format(jar.tool),
            '    actual = ":{}_jvm",'.format(jar.tool),
            '    visibility = ["//visibility:public"],',
            ")",
            "",
            "alias(",
            '    name = "{}_deploy.jar",'.format(jar.tool),
            '    actual = "@{}//file:{}",'.format(jar.repo, jar.asset),
            '    visibility = ["//visibility:public"],',
            ")",
        ]
    return "\n".join(loads + [build.rstrip("\n")] + lines + [""])

def plan_native_tools(version, pins, url_template, overrides):
    """Returns {tool: plan_assets of tool} for every native tool of the registry."""
    return {
        tool: plan_assets(tool, version, pins, url_template, overrides.get(tool, {}), exe = tool, repo_prefix = "dx_{}_prebuilt_".format(tool), library = TOOLS[tool].library)
        for tool in NATIVE_TOOLS
    }

_NATIVE_BINARY_LOAD = 'load("@bazel_skylib//rules:native_binary.bzl", "native_binary")'

def with_native_tools(build, native_tools, consumer = False, guard = False):
    """Returns the hub BUILD text build plus one target per native tool."""
    lines = []
    keys = {}
    binaries = False
    for tool, assets in sorted(native_tools.items()):
        pinned = [a for a in assets if a.repo]
        source = DEV_SOURCE_FALLBACK.get(tool, TOOLS[tool].source) if consumer else TOOLS[tool].source
        library = TOOLS[tool].library
        if not pinned:
            actual = _source_actual(source, guard)
        elif library:
            actual = _select([(":native_" + a.key, "@{}//file:{}".format(a.repo, a.exe)) for a in pinned], source)
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
            if library:
                continue
            binaries = True
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
    loads = [_NATIVE_BINARY_LOAD] if binaries and _NATIVE_BINARY_LOAD not in build else []
    return "\n".join(loads + [build.rstrip("\n")] + lines + [""])

def downloads_bzl(native_tools):
    """Returns the hub downloads.bzl text: DOWNLOADED maps each native tool with a download to its @platforms keys."""
    lines = [
        '"""The native tools @dx_prebuilt serves as downloads, by @platforms os_cpu key."""',
        "",
    ]
    downloaded = [(tool, sorted([a.key for a in assets if a.repo])) for tool, assets in sorted(native_tools.items())]
    entries = ["    \"{}\": {},".format(tool, keys) for tool, keys in downloaded if keys]
    if entries:
        lines += ["DOWNLOADED = {"] + entries + ["}"]
    else:
        lines.append("DOWNLOADED = {}")
    return "\n".join(lines + [""])

def alias_build(actual, guard = False):
    """Returns the hub BUILD text that points dx at one label and the tools at their sources; with guard, each selects GUARD_STUB under GUARD_VIEW."""
    lines = []
    for name, label in [("dx", actual)] + sorted(SOURCE_TOOLS.items()) + sorted(GO_TOOLS.items()):
        lines += [
            "alias(",
            '    name = "{}",'.format(name),
            "    actual = " + _source_actual(label, guard) + ",",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    return "\n".join(lines)

def dx_subcommand_build(consumer, guard = False):
    """Returns the hub dx/BUILD.bazel text: per DX_SUBCOMMANDS name, an alias of //:dx in a consumer, else of the subcommand's own binary."""
    lines = []
    for name, source in sorted(DX_SUBCOMMANDS.items()):
        lines += [
            "alias(",
            '    name = "{}",'.format(name),
            "    actual = " + ('"//:dx"' if consumer else _source_actual(source, guard)) + ",",
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    return "\n".join(lines)

def plan_python_tools(version, pins, url_template, overrides):
    """Returns {tool: plan_assets of tool} for every Python tool, one archive repo per pinned tool and platform."""
    return {
        tool: plan_assets(tool, version, pins, url_template, overrides.get(tool, {}), exe = tool, repo_prefix = "dx_{}_prebuilt_".format(tool))
        for tool in PYTHON_BUNDLE_TOOLS
    }

_BUNDLE_LOAD = 'load("@rules_dx//cli/prebuilt:bundle.bzl", "dx_bundle_tool", "dx_bundle_tree")'

def with_python_tools(build, python_tools, guard = False):
    """Returns the hub BUILD text build plus python_tool_<name> for each Python tool, each from its own archive."""
    lines = []
    keys = {}
    for name, assets in sorted(python_tools.items()):
        source = PYTHON_BUNDLE_TOOLS[name].source
        pinned = [a for a in assets if a.repo]
        actual = _select([(":python_" + a.key, ":python_tool_{}_{}".format(name, a.key)) for a in pinned], source) if pinned else _source_actual(source, guard)
        lines += [
            "",
            "alias(",
            '    name = "python_tool_{}",'.format(name),
            "    actual = " + actual + ",",
            '    visibility = ["//visibility:public"],',
            ")",
        ]
        for a in pinned:
            keys[a.key] = a
            lines += [
                "",
                "dx_bundle_tree(",
                '    name = "python_tools_{}_{}",'.format(name, a.key),
                '    srcs = ["@{}//:files"],'.format(a.repo),
                ")",
                "",
                "dx_bundle_tool(",
                '    name = "python_tool_{}_{}",'.format(name, a.key),
                '    launcher = "bin/{}{}",'.format(PYTHON_BUNDLE_TOOLS[name].launcher, ".cmd" if a.os == "windows" else ""),
                '    tree = ":python_tools_{}_{}",'.format(name, a.key),
                ")",
            ]
    for key, a in sorted(keys.items()):
        lines += [
            "",
            "config_setting(",
            '    name = "python_{}",'.format(key),
            "    constraint_values = [",
            '        "@platforms//os:{}",'.format(a.os),
            '        "@platforms//cpu:{}",'.format(a.cpu),
            "    ],",
            ")",
        ]
    loads = [_BUNDLE_LOAD] if keys else []
    return "\n".join(loads + [build.rstrip("\n")] + lines + [""])

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

def consumer_build(assets, go_assets = []):
    """Returns the hub BUILD text that selects a prebuilt dx and Go lint tools by os and cpu."""
    if not [a for a in assets + go_assets if a.repo]:
        return alias_build(SOURCE_LABEL)
    dx_pinned = [a for a in assets if a.repo]
    lines = [
        'load("@bazel_skylib//rules:native_binary.bzl", "native_binary")',
        'load("@rules_dx//cli/prebuilt:fail.bzl", "dx_unavailable")',
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
        "alias(",
        '    name = "dx",',
        "    actual = " + (_select([(":" + a.key, ":dx_" + a.key if a.repo else SOURCE_LABEL) for a in assets], ":unsupported") if dx_pinned else json.encode(SOURCE_LABEL)) + ",",
        ")",
    ]
    for tool, label in sorted(SOURCE_TOOLS.items()):
        lines += [
            "",
            "alias(",
            '    name = "{}",'.format(tool),
            "    actual = " + (_select([(":" + a.key, ":{}_{}".format(tool, a.key) if a.repo else label) for a in assets], ":unsupported") if dx_pinned else json.encode(label)) + ",",
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
    platforms = {a.key: a for a in go_assets}
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
