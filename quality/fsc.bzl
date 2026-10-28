"""Replays the F# compilation of a target and records the fsc diagnostics."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_dotnet//dotnet/private:common.bzl", "collect_compile_info", "framework_preprocessor_symbols", "is_core_framework", "is_standard_framework")
load("//libs/starlark:resources.bzl", "dx_quality_heavy_resources")
load("//quality:roslyn.bzl", "tfm_sort_key")

_TOOLCHAIN = "@rules_dotnet//dotnet:toolchain_type"

_FSHARP_KINDS = ["fsharp_library", "fsharp_binary", "fsharp_test"]

FscInfo = provider(
    doc = "fsc diagnostics from compiling one F# target, one per target framework.",
    fields = {
        "logs": "list of File: fsc output per target framework.",
        "statuses": "list of File: fsc exit code per target framework.",
        "tfms": "list of string: target framework per entry, in ascending version order.",
    },
)

FscUnsupportedInfo = provider(
    doc = "Marks an F# target on a platform where the fsc typecheck is unavailable.",
    fields = {"reason": "string: why fsc cannot run for this target."},
)

def _first(value):
    return value[0] if type(value) == "list" else value

def _merged_fsc(infos):
    by_tfm = {}
    for info in infos:
        for i in range(len(info.tfms)):
            if info.tfms[i] not in by_tfm:
                by_tfm[info.tfms[i]] = (info.logs[i], info.statuses[i])
    tfms = sorted(by_tfm.keys(), key = tfm_sort_key)
    return FscInfo(
        logs = [by_tfm[tfm][0] for tfm in tfms],
        statuses = [by_tfm[tfm][1] for tfm in tfms],
        tfms = tfms,
    )

def _targetprofile(tfm):
    # Mirrors rules_dotnet 0.22.2 fsharp_assembly.bzl _format_targetprofile.
    if is_standard_framework(tfm):
        return "--targetprofile:netstandard"
    if is_core_framework(tfm):
        return "--targetprofile:netcore"
    return "--targetprofile:mscorlib"

def _fsc_impl(target, ctx):
    attrs = ctx.rule.attr
    if ctx.rule.kind not in _FSHARP_KINDS:
        if not hasattr(attrs, "upstream"):
            return []
        upstream = _first(attrs.upstream)
        if FscInfo not in upstream:
            return []
        variants = getattr(attrs, "dx_tfm_variants", None) or []
        if type(variants) != "list":
            variants = [variants]
        infos = [upstream[FscInfo]] + [variant[FscInfo] for variant in variants if FscInfo in variant]
        return [_merged_fsc(infos)]

    toolchain = ctx.toolchains[_TOOLCHAIN]
    if toolchain == None:
        fail("fsc: " + str(target.label) + " needs a resolved rules_dotnet toolchain")
    if ctx.target_platform_has_constraint(attrs._windows_constraint[platform_common.ConstraintValueInfo]):
        # Do not fail here: this aspect runs on every F# target the typecheck
        # visits, whatever the policy says. The pipeline fails only if fsc is resolved.
        return [FscUnsupportedInfo(reason = "the fsc typecheck is linux_x86_64 only and " + str(target.label) + " is built for a Windows target platform")]
    tfm = attrs._target_framework[BuildSettingInfo].value
    fsc = toolchain.fsharp_compiler.files_to_run.executable
    runtime = toolchain.runtime.files_to_run.executable
    wrapper = ctx.rule.executable._compiler_wrapper_sh

    assembly_name = getattr(attrs, "out", "") or target.label.name
    (refs, _, _, _, _, _, compile_data, framework_files, _) = collect_compile_info(
        assembly_name,
        attrs.deps,
        _first(attrs._targeting_pack),
        getattr(attrs, "exports", []),
        toolchain.strict_deps[BuildSettingInfo].value,
    )

    log = ctx.actions.declare_file("dx_fsc/%s/%s.log" % (target.label.name, tfm))
    status = ctx.actions.declare_file("dx_fsc/%s/%s.exit" % (target.label.name, tfm))
    out_tree = ctx.actions.declare_directory("dx_fsc/%s/%s.out" % (target.label.name, tfm))

    # The arguments mirror rules_dotnet 0.22.2 fsharp/actions/fsharp_assembly.bzl
    # _compile, so the diagnostics match the real build. Strong-name keys,
    # resources and the generated InternalsVisibleTo file do not change them.
    debug = ctx.var["COMPILATION_MODE"] in ["dbg", "fastbuild"]
    args = ctx.actions.args()
    args.add("--noframework")
    args.add("--utf8output")
    args.add("--deterministic+")
    args.add("--nowin32manifest")
    args.add("--nocopyfsharpcore")
    args.add("--simpleresolution")
    args.add(_targetprofile(tfm))
    args.add("--nologo")
    args.add("--warn:%s" % attrs.warning_level)
    if len(attrs.nowarn) > 0:
        args.add("--nowarn:" + ",".join(attrs.nowarn))
    args.add("--target:" + ("library" if ctx.rule.kind == "fsharp_library" else "exe"))
    langversion = attrs.langversion if attrs.langversion != "" else toolchain.dotnetinfo.fsharp_default_version
    if langversion:
        args.add("--langversion:" + langversion)
    if debug:
        args.add_all(["--debug+", "--optimize-", "--define:TRACE", "--define:DEBUG", "--tailcalls-"])
    else:
        args.add_all(["--debug-", "--optimize+", "--define:TRACE", "--define:RELEASE"])
    args.add("--debug:portable")
    args.add("%s/%s.dll" % (out_tree.path, assembly_name), format = "--out:%s")
    args.add_all(depset(framework_files, transitive = [refs]), format_each = "-r:%s")
    args.add_all(ctx.rule.files.srcs)
    args.add_all(framework_preprocessor_symbols(tfm) + list(attrs.defines), format_each = "-d:%s")
    args.add_all(attrs.compiler_options)
    args.add_all(["--warnaserror-", "--flaterrors", "--consolecolors-"])
    args.set_param_file_format("multiline")
    args.use_param_file("@%s", use_always = True)

    inputs = depset(
        direct = ctx.rule.files.srcs + framework_files + [wrapper, runtime, fsc],
        transitive = [
            refs,
            compile_data,
            toolchain.runtime.default_runfiles.files,
            toolchain.fsharp_compiler.default_runfiles.files,
        ],
    )
    ctx.actions.run_shell(
        mnemonic = "FSharpFsc",
        progress_message = "fsc diagnostics %{label}",
        command = 'log="$1"; status="$2"; shift 2; "$@" > "$log" 2>&1; echo "$?" > "$status"',
        arguments = [log.path, status.path, wrapper.path, runtime.path, fsc.path, args],
        inputs = inputs,
        outputs = [log, status, out_tree],
        env = {"DOTNET_CLI_HOME": runtime.dirname},
        resource_set = dx_quality_heavy_resources,
    )
    return [FscInfo(logs = [log], statuses = [status], tfms = [tfm])]

fsc_aspect = aspect(
    implementation = _fsc_impl,
    doc = "Replays the F# compilation of a target and records the fsc diagnostics.",
    attr_aspects = ["upstream", "dx_tfm_variants"],
    toolchains = [config_common.toolchain_type(_TOOLCHAIN, mandatory = False)],
)
