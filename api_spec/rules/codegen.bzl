"""OpenAPI TypeScript client generation (Redocly generate-client from the pinned CLI)."""

load("//generation:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers")
load("//libs/starlark:defs.bzl", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "ApiSpecInfo")

OPENAPI_GENERATED_SRCS_GROUP = "openapi_generated_srcs"

OPENAPI_TYPESCRIPT_FLAGS = [
    "--output-mode=single",
    "--runtime=inline",
    "--import-ext=js",
    "--generator=typescript",
    "--args-style=grouped",
    "--error-mode=throw",
    "--date-type=string",
]

_ENV = {
    "DO_NOT_TRACK": "1",
    "JS_BINARY__NO_CD_BINDIR": "1",
    "REDOCLY_SUPPRESS_UPDATE_NOTICE": "true",
    "REDOCLY_TELEMETRY": "off",
}

def _root(ctx, info):
    label = display_label(ctx.label)
    if ctx.file.root:
        for candidate in info.roots:
            if candidate.path == ctx.file.root.path:
                return candidate
        fail("openapi_typescript_client " + label + ": root " + display_label(ctx.attr.root.label) + " is not a root of " + display_label(ctx.attr.spec.label))
    if len(info.roots) != 1:
        fail("openapi_typescript_client " + label + ": " + display_label(ctx.attr.spec.label) + " has " + str(len(info.roots)) + " roots; set root")
    return info.roots[0]

def _client_impl(ctx):
    info = ctx.attr.spec[ApiSpecInfo]
    root = _root(ctx, info)
    out = ctx.actions.declare_file(ctx.attr.out if ctx.attr.out else ctx.label.name + ".ts")
    stamp = ctx.actions.declare_file(ctx.label.name + ".preflight")
    files = info.files.to_list()
    args = ctx.actions.args()
    args.add("--root", root.short_path)
    args.add("--stamp", stamp)
    for f in files:
        args.add("--doc", f.short_path + "=" + f.path)
    ctx.actions.run(
        executable = ctx.executable._preflight,
        inputs = info.files,
        outputs = [stamp],
        arguments = [args],
        mnemonic = "OpenapiPreflight",
        progress_message = "Checking " + root.short_path + " for code generation",
    )
    ctx.actions.run(
        executable = ctx.executable._redocly,
        inputs = depset([stamp, ctx.file._config], transitive = [info.files]),
        outputs = [out],
        arguments = ["generate-client", root.path, "--output=" + out.path, "--config=" + ctx.file._config.path] + OPENAPI_TYPESCRIPT_FLAGS,
        env = _ENV,
        execution_requirements = {"block-network": "1"},
        mnemonic = "OpenapiTypescriptClient",
        progress_message = "redocly generate-client " + root.short_path,
    )
    return [
        DefaultInfo(files = depset([out])),
        dx_quality_sources([], [], str(ctx.label)),
        OutputGroupInfo(**{OPENAPI_GENERATED_SRCS_GROUP: depset([out])}),
    ]

openapi_typescript_client = rule(
    implementation = _client_impl,
    doc = "Generates one self-contained TypeScript client from an OpenAPI root of an api_spec_library.",
    attrs = {
        "out": attr.string(doc = "Output file name; defaults to NAME.ts."),
        "root": attr.label(allow_single_file = [".yaml", ".yml", ".json"], doc = "The root document; required when the spec has more than one."),
        "spec": attr.label(providers = [ApiSpecInfo], mandatory = True, doc = "The api_spec_library to generate from."),
        "_config": attr.label(default = Label("//api_spec/codegen:redocly.yaml"), allow_single_file = True),
        "_preflight": attr.label(default = Label("//api_spec/codegen:preflight"), executable = True, cfg = "exec"),
        "_redocly": attr.label(default = Label("//quality/tools/javascript/bin:redocly"), executable = True, cfg = "exec"),
    },
)

def _shard_impl(ctx):
    client = ctx.attr.client
    if OutputGroupInfo not in client or OPENAPI_GENERATED_SRCS_GROUP not in client[OutputGroupInfo]:
        fail(
            "openapi_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(client.label) + " has no " + OPENAPI_GENERATED_SRCS_GROUP +
            " output group (want an openapi_typescript_client)",
        )
    return codegen_bound_shard_providers(
        ctx,
        "openapi_codegen_shard",
        "openapi",
        display_label(client.label),
        OPENAPI_GENERATED_SRCS_GROUP,
        client[OutputGroupInfo][OPENAPI_GENERATED_SRCS_GROUP].to_list(),
    )

openapi_codegen_shard = rule(
    implementation = _shard_impl,
    doc = "Declares codegen plan entries for the file an openapi_typescript_client generates. The file needs one entry with an EXEC_PATH suffix.",
    attrs = {
        "client": attr.label(doc = "openapi_typescript_client whose output the entries bind.", mandatory = True),
        "entries": attr.string_list(
            doc = "LOGICAL_PATH|IMPORT_ROOT|NAMESPACE|EXEC_PATH entries, one per generated file.",
            mandatory = True,
        ),
        "language": attr.string(doc = "typescript.", mandatory = True),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
