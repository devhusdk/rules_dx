"""Standalone tool acquisition."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive", "http_file")
load("//libs/starlark:repo.bzl", "hub_repo")
load("//quality/artifacts:acquire.bzl", "ACQUIRE_ATTRS", "acquire_tool")
load("//quality/artifacts:actionlint.bzl", _actionlint = "ARTIFACTS")
load("//quality/artifacts:avro_tools.bzl", _avro_tools = "SOURCE")
load("//quality/artifacts:bicep.bzl", _bicep = "ARTIFACTS")
load("//quality/artifacts:biome.bzl", _biome = "ARTIFACTS")
load("//quality/artifacts:buf.bzl", _buf = "ARTIFACTS")
load("//quality/artifacts:buildifier.bzl", _buildifier = "ARTIFACTS")
load("//quality/artifacts:checkmake.bzl", _checkmake = "ARTIFACTS")
load("//quality/artifacts:checkstyle.bzl", _checkstyle = "SOURCE")
load("//quality/artifacts:chktex.bzl", _chktex = "ARTIFACTS")
load("//quality/artifacts:cppcheck.bzl", _cppcheck = "SOURCE")
load("//quality/artifacts:cue.bzl", _cue = "ARTIFACTS")
load("//quality/artifacts:ffmt.bzl", _ffmt = "ARTIFACTS")
load("//quality/artifacts:fortitude.bzl", _fortitude = "ARTIFACTS")
load("//quality/artifacts:gersemi.bzl", _gersemi = "ARTIFACTS")
load("//quality/artifacts:gitleaks.bzl", _gitleaks = "ARTIFACTS")
load("//quality/artifacts:google_java_format.bzl", _google_java_format = "SOURCE")
load("//quality/artifacts:hadolint.bzl", _hadolint = "ARTIFACTS")
load("//quality/artifacts:helm.bzl", _helm = "ARTIFACTS")
load("//quality/artifacts:hlint.bzl", _hlint = "ARTIFACTS")
load("//quality/artifacts:hub.bzl", "artifact_map_errors", "artifact_metadata_errors", "hub_build")
load("//quality/artifacts:jsonnet_lint.bzl", _jsonnet_lint = "ARTIFACTS")
load("//quality/artifacts:jsonnetfmt.bzl", _jsonnetfmt = "ARTIFACTS")
load("//quality/artifacts:keep_sorted.bzl", _keep_sorted = "ARTIFACTS")
load("//quality/artifacts:ktfmt.bzl", _ktfmt = "SOURCE")
load("//quality/artifacts:detekt.bzl", _detekt = "SOURCE")
load("//quality/artifacts:ktlint.bzl", _ktlint = "SOURCE")
load("//quality/artifacts:kubeconform.bzl", _kubeconform = "ARTIFACTS")
load("//quality/artifacts:latexindent.bzl", _latexindent = "ARTIFACTS")
load("//quality/artifacts:lychee.bzl", _lychee = "ARTIFACTS")
load("//quality/artifacts:mdbook.bzl", _mdbook = "ARTIFACTS")
load("//quality/artifacts:nixfmt.bzl", _nixfmt = "ARTIFACTS")
load("//quality/artifacts:opa.bzl", _opa = "ARTIFACTS")
load("//quality/artifacts:ormolu.bzl", _ormolu = "ARTIFACTS")
load("//quality/artifacts:pkl.bzl", _pkl = "ARTIFACTS")
load("//quality/artifacts:pkl_lsp.bzl", _pkl_lsp = "SOURCE")
load("//quality/artifacts:platforms.bzl", "artifact_platform_key", "platform_repeat_message")
load("//quality/artifacts:pmd.bzl", _pmd = "SOURCE")
load("//quality/artifacts:regal.bzl", _regal = "ARTIFACTS")
load("//quality/artifacts:ruff.bzl", _ruff = "ARTIFACTS")
load("//quality/artifacts:scalafmt.bzl", _scalafmt = "ARTIFACTS")
load("//quality/artifacts:shellcheck.bzl", _shellcheck = "ARTIFACTS")
load("//quality/artifacts:shfmt.bzl", _shfmt = "ARTIFACTS")
load("//quality/artifacts:spotbugs.bzl", _spotbugs = "SOURCE")
load("//quality/artifacts:stylua.bzl", _stylua = "ARTIFACTS")
load("//quality/artifacts:taplo.bzl", _taplo = "ARTIFACTS")
load("//quality/artifacts:terraform.bzl", _terraform = "ARTIFACTS")
load("//quality/artifacts:tflint.bzl", _tflint = "ARTIFACTS")
load("//quality/artifacts:tinymist.bzl", _tinymist = "ARTIFACTS")
load("//quality/artifacts:trivy.bzl", _trivy = "ARTIFACTS")
load("//quality/artifacts:ty.bzl", _ty = "ARTIFACTS")
load("//quality/artifacts:typos.bzl", _typos = "ARTIFACTS")
load("//quality/artifacts:typstyle.bzl", _typstyle = "ARTIFACTS")
load("//quality/artifacts:uv.bzl", _uv = "ARTIFACTS")
load("//quality/artifacts:vale.bzl", _vale = "ARTIFACTS")
load("//quality/artifacts:wasmtime.bzl", _wasmtime = "ARTIFACTS")
load("//quality/artifacts:yamlfmt.bzl", _yamlfmt = "ARTIFACTS")
load("//quality/artifacts:zanuda.bzl", _zanuda = "SOURCE")
load("//quality/tools/ocaml:repos.bzl", "dx_ocaml_duniverse")

_ARTIFACTS = [
    artifact
    for tool in [
        _actionlint,
        _bicep,
        _biome,
        _buf,
        _buildifier,
        _checkmake,
        _chktex,
        _cue,
        _ffmt,
        _fortitude,
        _gersemi,
        _gitleaks,
        _hadolint,
        _helm,
        _hlint,
        _jsonnet_lint,
        _jsonnetfmt,
        _keep_sorted,
        _kubeconform,
        _latexindent,
        _lychee,
        _mdbook,
        _nixfmt,
        _opa,
        _ormolu,
        _pkl,
        _regal,
        _ruff,
        _scalafmt,
        _shellcheck,
        _shfmt,
        _stylua,
        _taplo,
        _terraform,
        _tflint,
        _tinymist,
        _trivy,
        _ty,
        _typos,
        _typstyle,
        _uv,
        _vale,
        _wasmtime,
        _yamlfmt,
    ]
    for artifact in tool.values()
]

TOOL_SOURCES = [
    _avro_tools,
    _checkstyle,
    _cppcheck,
    _detekt,
    _google_java_format,
    _ktfmt,
    _ktlint,
    _pkl_lsp,
    _pmd,
    _spotbugs,
    _zanuda,
]

TOOL_ARTIFACTS = _ARTIFACTS

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
    errors = artifact_metadata_errors(_ARTIFACTS)
    if len(errors) > 0:
        fail("dx_tools: " + "; ".join(errors))
    by_tool = {}
    for artifact in _ARTIFACTS:
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
    for source in TOOL_SOURCES:
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
