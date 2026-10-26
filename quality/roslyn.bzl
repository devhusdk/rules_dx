"""Compiles a C# target with the SDK analyzers and records the SARIF log."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_dotnet//dotnet/private:common.bzl", "collect_compile_info", "framework_preprocessor_symbols")

_TOOLCHAIN = "@rules_dotnet//dotnet:toolchain_type"

RoslynInfo = provider(
    doc = "SARIF log from compiling one C# target with the SDK analyzers.",
    fields = {
        "sarif": "File: SARIF 2.1 log.",
        "status": "File: csc exit code.",
        "tfm": "string: target framework of the analyzed compilation.",
    },
)

def _is_csharp_compilation(attrs):
    return hasattr(attrs, "additionalfiles") and hasattr(attrs, "analyzer_configs") and hasattr(attrs, "_targeting_pack")

def _first(value):
    return value[0] if type(value) == "list" else value

def _analysis_level(csc, tfm):
    marker = "/sdk/"
    sdk_version = csc.path.partition(marker)[2].partition("/")[0]
    sdk_major = sdk_version.partition(".")[0]
    if tfm.startswith("net") and tfm.endswith(".0") and tfm[3:-2].isdigit() and int(tfm[3:-2]) >= 5:
        return tfm[3:-2]
    return sdk_major

def _roslyn_impl(target, ctx):
    attrs = ctx.rule.attr
    if not _is_csharp_compilation(attrs):
        if hasattr(attrs, "upstream"):
            upstream = _first(attrs.upstream)
            if RoslynInfo in upstream:
                return [upstream[RoslynInfo]]
        return []
    if getattr(attrs, "is_analyzer", False) or getattr(attrs, "is_language_specific_analyzer", False):
        return []

    toolchain = ctx.toolchains[_TOOLCHAIN]
    if toolchain == None:
        fail("roslyn: " + str(target.label) + " needs a resolved rules_dotnet toolchain")
    tfm = attrs._target_framework[BuildSettingInfo].value
    csc = toolchain.csharp_compiler.files_to_run.executable
    runtime = toolchain.runtime.files_to_run.executable
    wrapper = ctx.rule.executable._compiler_wrapper_sh
    if ctx.target_platform_has_constraint(attrs._windows_constraint[platform_common.ConstraintValueInfo]):
        wrapper = ctx.rule.executable._compiler_wrapper_bat

    assembly_name = getattr(attrs, "out", "") or target.label.name
    exports = getattr(attrs, "exports", [])
    (refs, _, analyzers, analyzers_csharp, _, _, compile_data, framework_files, _) = collect_compile_info(
        assembly_name,
        attrs.deps,
        _first(attrs._targeting_pack),
        exports,
        toolchain.strict_deps[BuildSettingInfo].value,
    )

    sdk_analyzers = csc.path.replace("/Roslyn/bincore/csc.dll", "/Sdks/Microsoft.NET.Sdk/analyzers")
    level = _analysis_level(csc, tfm)

    sarif = ctx.actions.declare_file("dx_roslyn/%s/%s.sarif" % (target.label.name, tfm))
    status = ctx.actions.declare_file("dx_roslyn/%s/%s.exit" % (target.label.name, tfm))
    out_tree = ctx.actions.declare_directory("dx_roslyn/%s/%s.out" % (target.label.name, tfm))

    args = ctx.actions.args()
    args.add("/nologo")
    args.add("/nostdlib+")
    args.add("/utf8output")
    args.add("/deterministic+")
    args.add("/target:library" if ctx.rule.kind == "csharp_library" else "/target:exe")
    args.add("/unsafe+" if attrs.allow_unsafe_blocks else "/unsafe-")
    args.add("/nullable:" + attrs.nullable)
    args.add("/warn:" + str(attrs.warning_level))
    if len(attrs.nowarn) > 0:
        args.add("/nowarn:" + ",".join(attrs.nowarn))
    args.add("/langversion:" + (attrs.langversion if attrs.langversion != "" else toolchain.dotnetinfo.csharp_default_version))
    configuration = "DEBUG" if ctx.var["COMPILATION_MODE"] in ["dbg", "fastbuild"] else "RELEASE"
    args.add_all(["TRACE", configuration] + framework_preprocessor_symbols(tfm) + list(attrs.defines), format_each = "/d:%s")
    args.add_all(depset(framework_files, transitive = [refs]), format_each = "/r:%s")
    args.add_all(analyzers, format_each = "/analyzer:%s")
    args.add_all(analyzers_csharp, format_each = "/analyzer:%s")
    args.add(sdk_analyzers + "/Microsoft.CodeAnalysis.NetAnalyzers.dll", format = "/analyzer:%s")
    args.add(sdk_analyzers + "/Microsoft.CodeAnalysis.CSharp.NetAnalyzers.dll", format = "/analyzer:%s")
    args.add(sdk_analyzers + "/build/config/analysislevel_" + level + "_default.globalconfig", format = "/analyzerconfig:%s")
    args.add_all(ctx.rule.files.analyzer_configs, format_each = "/analyzerconfig:%s")
    args.add_all(ctx.rule.files.additionalfiles, format_each = "/additionalfile:%s")
    args.add("/warnaserror-")
    args.add_all(attrs.compiler_options)
    args.add("%s/%s.dll" % (out_tree.path, assembly_name), format = "/out:%s")
    args.add(sarif, format = "/errorlog:%s,version=2.1")
    args.add_all(ctx.rule.files.srcs)
    args.set_param_file_format("multiline")
    args.use_param_file("@%s", use_always = True)

    inputs = depset(
        direct = ctx.rule.files.srcs + ctx.rule.files.additionalfiles + ctx.rule.files.analyzer_configs + framework_files + [wrapper, runtime, csc],
        transitive = [
            refs,
            analyzers,
            analyzers_csharp,
            compile_data,
            toolchain.runtime.default_runfiles.files,
            toolchain.csharp_compiler.default_runfiles.files,
        ],
    )
    env = {"DOTNET_CLI_HOME": runtime.dirname}
    if ctx.target_platform_has_constraint(attrs._windows_constraint[platform_common.ConstraintValueInfo]):
        ctx.actions.run(
            mnemonic = "CSharpRoslyn",
            progress_message = "Roslyn analyzers %{label}",
            executable = ctx.file._status_launcher,
            arguments = [status.path, wrapper.path, runtime.path, csc.path, args],
            inputs = inputs,
            outputs = [sarif, status, out_tree],
            env = env,
        )
    else:
        ctx.actions.run_shell(
            mnemonic = "CSharpRoslyn",
            progress_message = "Roslyn analyzers %{label}",
            command = 'status="$1"; shift; "$@"; echo "$?" > "$status"',
            arguments = [status.path, wrapper.path, runtime.path, csc.path, args],
            inputs = inputs,
            outputs = [sarif, status, out_tree],
            env = env,
        )
    return [RoslynInfo(sarif = sarif, status = status, tfm = tfm)]

roslyn_aspect = aspect(
    implementation = _roslyn_impl,
    doc = "Compiles a C# target with the SDK analyzers and records the SARIF log.",
    attr_aspects = ["upstream"],
    attrs = {
        "_status_launcher": attr.label(
            allow_single_file = True,
            default = "//quality:roslyn_status.bat",
            doc = "Windows launcher that records the csc exit code.",
        ),
    },
    toolchains = [config_common.toolchain_type(_TOOLCHAIN, mandatory = False)],
)
