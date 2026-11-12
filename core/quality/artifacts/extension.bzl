"""Standalone tool acquisition."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive", "http_file")
load("//core:repo.bzl", "hub_repo")
load("//core/quality/artifacts:acquire.bzl", "ACQUIRE_ATTRS", "acquire_tool")
load("//core/quality/artifacts:hub.bzl", "artifact_map_errors", "artifact_metadata_errors", "hub_build")
load("//core/quality/artifacts:platforms.bzl", "artifact_platform_key", "platform_repeat_message")
load("//core/quality/artifacts:ocaml_duniverse.bzl", DUNIVERSE_PACKAGES = "PACKAGES", DUNIVERSE_PATCHES = "PATCHES", DUNIVERSE_REPO = "REPO")
load("//ocaml/private/quality:repos.bzl", "dx_ocaml_duniverse")

_LOCK = Label("//core/quality/artifacts:lock.json")
_PINS = Label("//core/quality/artifacts:pins.json")

def _read_artifacts(ctx):
    lock = json.decode(ctx.read(_LOCK))
    return [artifact for tool in sorted(lock.keys()) for artifact in lock[tool].values()]

def _read_sources(ctx):
    sources = json.decode(ctx.read(_PINS))["sources"]
    result = []
    for tool in sorted(sources.keys()):
        source = json.decode(json.encode(sources[tool]).replace("{version}", sources[tool]["version"]))
        if source["kind"] == "duniverse":
            source["repo"] = DUNIVERSE_REPO
            source["packages"] = DUNIVERSE_PACKAGES
            source["patches"] = DUNIVERSE_PATCHES
        result.append(source)
    return result

def _repo_name(artifact):
    return "dx_%s_%s" % (artifact["tool"], artifact_platform_key(artifact))

def _standalone_tool_repo_impl(ctx):
    acquire_tool(ctx)
    return ctx.repo_metadata(reproducible = True)

_standalone_tool_repo = repository_rule(
    implementation = _standalone_tool_repo_impl,
    attrs = ACQUIRE_ATTRS,
)

def _source_repo(source):
    kind = source["kind"]
    if kind == "archive":
        http_archive(
            name = source["repo"],
            build_file = Label(source["build_file"]),
            sha256 = source["sha256"],
            strip_prefix = source["strip_prefix"],
            urls = source["urls"],
        )
    elif kind == "file":
        http_file(
            name = source["repo"],
            downloaded_file_path = source["downloaded_file_path"],
            sha256 = source["sha256"],
            urls = source["urls"],
        )
    elif kind == "duniverse":
        dx_ocaml_duniverse(
            name = source["repo"],
            packages = json.encode([
                {"dir": p["dir"], "name": p["name"], "sha256": p["sha256"], "url": p["url"]}
                for p in source["packages"]
            ]),
            patches = [Label(patch) for patch in source["patches"]],
        )
    else:
        fail("dx_tools: unsupported source kind for '%s': %s" % (source["repo"], kind))

def _dx_tools_impl(ctx):
    os_linux, os_macos, os_windows, cpu_x86_64, cpu_arm64 = None, None, None, None, None
    declarer = None
    for module in ctx.modules:
        for tag in module.tags.platform:
            if os_linux != None:
                fail(platform_repeat_message(declarer, "%s %s" % (module.name, module.version)))
            declarer = "%s %s" % (module.name, module.version)
            os_linux = str(tag.os_linux)
            os_macos = str(tag.os_macos)
            os_windows = str(tag.os_windows)
            cpu_x86_64 = str(tag.cpu_x86_64)
            cpu_arm64 = str(tag.cpu_arm64)
    if os_linux == None:
        fail("dx_tools.platform(os_linux, os_macos, os_windows, cpu_x86_64, cpu_arm64) is required in MODULE.bazel")
    artifacts = _read_artifacts(ctx)
    errors = artifact_metadata_errors(artifacts)
    if len(errors) > 0:
        fail("dx_tools: " + "; ".join(errors))
    by_tool = {}
    for artifact in artifacts:
        name = _repo_name(artifact)
        _standalone_tool_repo(
            name = name,
            url = artifact["url"],
            sha256 = artifact["sha256"],
            asset = artifact["url"].split("/")[-1],
            archive_format = artifact["archive"]["format"],
            executable = artifact["executable"],
            executable_sha256 = artifact["executable_sha256"],
            runtime_files = json.encode(artifact["runtime_files"]),
        )
        platform = artifact_platform_key(artifact)
        by_tool.setdefault(artifact["tool"], {})[platform] = name
    errors = artifact_map_errors(by_tool)
    if len(errors) > 0:
        fail("dx_tools hub: " + "; ".join(errors))
    hub_repo(
        name = "dx_tools",
        files = {"BUILD.bazel": hub_build(by_tool, {
            "cpu_arm64": cpu_arm64,
            "cpu_x86_64": cpu_x86_64,
            "os_linux": os_linux,
            "os_macos": os_macos,
            "os_windows": os_windows,
        })},
    )
    declared = {}
    for source in _read_sources(ctx):
        if source["repo"] not in declared:
            declared[source["repo"]] = True
            _source_repo(source)
    return ctx.extension_metadata(reproducible = True)

_platform = tag_class(attrs = {
    "cpu_arm64": attr.label(
        doc = "Constraint value selecting arm64 CPUs for dx tools.",
        mandatory = True,
    ),
    "cpu_x86_64": attr.label(
        doc = "Constraint value selecting x86_64 CPUs for dx tools.",
        mandatory = True,
    ),
    "os_linux": attr.label(
        doc = "Constraint value selecting Linux for dx tools.",
        mandatory = True,
    ),
    "os_macos": attr.label(
        doc = "Constraint value selecting macOS for dx tools.",
        mandatory = True,
    ),
    "os_windows": attr.label(
        doc = "Constraint value selecting Windows for dx tools.",
        mandatory = True,
    ),
})

dx_tools = module_extension(
    implementation = _dx_tools_impl,
    tag_classes = {"platform": _platform},
)
