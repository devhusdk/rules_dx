"""OpenAPI client generation: TypeScript (Redocly generate-client), Go (oapi-codegen), Python (openapi-python-client) and Rust (progenitor)."""

load("//core/codegen:codegen.bzl", "CODEGEN_SHARD_WRITER_LABEL", "codegen_bound_shard_providers")
load("//core:defs.bzl", "display_label")
load("//core:resources.bzl", "dx_heavy_resources")
load("//core:wrapper.bzl", "dx_quality_sources")
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

def _preflight(ctx, info, root):
    stamp = ctx.actions.declare_file(ctx.label.name + ".preflight")
    args = ctx.actions.args()
    args.add("--root", root.short_path)
    args.add("--stamp", stamp)
    for f in info.files.to_list():
        args.add("--doc", f.short_path + "=" + f.path)
    ctx.actions.run(
        executable = ctx.executable._preflight,
        inputs = info.files,
        outputs = [stamp],
        arguments = [args],
        mnemonic = "OpenapiPreflight",
        progress_message = "Checking " + root.short_path + " for code generation",
    )
    return stamp

def _client_providers(ctx, out):
    return [
        DefaultInfo(files = depset([out])),
        dx_quality_sources([], [], str(ctx.label)),
        OutputGroupInfo(**{OPENAPI_GENERATED_SRCS_GROUP: depset([out])}),
    ]

def _client_impl(ctx):
    info = ctx.attr.spec[ApiSpecInfo]
    root = _root(ctx, info)
    out = ctx.actions.declare_file(ctx.attr.out if ctx.attr.out else ctx.label.name + ".ts")
    stamp = _preflight(ctx, info, root)
    ctx.actions.run(
        executable = ctx.executable._redocly,
        inputs = depset([stamp, ctx.file._config], transitive = [info.files]),
        outputs = [out],
        arguments = ["generate-client", root.path, "--output=" + out.path, "--config=" + ctx.file._config.path] + OPENAPI_TYPESCRIPT_FLAGS,
        env = _ENV,
        execution_requirements = {"block-network": "1"},
        mnemonic = "OpenapiTypescriptClient",
        resource_set = dx_heavy_resources,
        progress_message = "redocly generate-client " + root.short_path,
    )
    return _client_providers(ctx, out)

openapi_typescript_client = rule(
    implementation = _client_impl,
    doc = "Generates one self-contained TypeScript client from an OpenAPI root of an api_spec_library.",
    attrs = {
        "out": attr.string(doc = "Output file name; defaults to NAME.ts."),
        "root": attr.label(allow_single_file = [".yaml", ".yml", ".json"], doc = "The root document; required when the spec has more than one."),
        "spec": attr.label(providers = [ApiSpecInfo], mandatory = True, doc = "The api_spec_library to generate from."),
        "_config": attr.label(default = Label("//api_spec/private/codegen:redocly.yaml"), allow_single_file = True),
        "_preflight": attr.label(default = Label("//api_spec/private/codegen:preflight"), executable = True, cfg = "exec"),
        "_redocly": attr.label(default = Label("//javascript/private/quality/bin:redocly"), executable = True, cfg = "exec"),
    },
)

def _go_client_impl(ctx):
    info = ctx.attr.spec[ApiSpecInfo]
    root = _root(ctx, info)
    out = ctx.actions.declare_file(ctx.attr.out if ctx.attr.out else ctx.label.name + ".go")
    stamp = _preflight(ctx, info, root)
    sdk = ctx.toolchains["@rules_go//go:toolchain"].sdk
    # run_shell: The shell turns the Go path into an absolute PATH entry and sets HOME to the execroot.
    ctx.actions.run_shell(
        tools = [ctx.executable._oapi_codegen],
        inputs = depset([stamp, sdk.go], transitive = [info.files, sdk.srcs, sdk.libs, sdk.tools, sdk.headers]),
        outputs = [out],
        env = {"GOCACHE": "/tmp/dx_oapi_gocache", "GOFLAGS": "-mod=mod", "GOPROXY": "off"},
        command = 'PATH="$PWD/$(dirname "$1"):$PATH" HOME="$PWD" exec "$2" -package "$3" -generate types,client -o "$4" "$5"',
        arguments = [sdk.go.path, ctx.executable._oapi_codegen.path, ctx.attr.package, out.path, root.path],
        execution_requirements = {"block-network": "1"},
        mnemonic = "OpenapiGoClient",
        progress_message = "oapi-codegen " + root.short_path,
    )
    return _client_providers(ctx, out)

openapi_go_client = rule(
    implementation = _go_client_impl,
    doc = "Generates one Go client file (types and client) with oapi-codegen from an OpenAPI root of an api_spec_library.",
    attrs = {
        "out": attr.string(doc = "Output file name; defaults to NAME.go."),
        "package": attr.string(doc = "Go package name of the generated file.", mandatory = True),
        "root": attr.label(allow_single_file = [".yaml", ".yml", ".json"], doc = "The root document; required when the spec has more than one."),
        "spec": attr.label(providers = [ApiSpecInfo], mandatory = True, doc = "The api_spec_library to generate from."),
        "_oapi_codegen": attr.label(default = Label("@com_github_oapi_codegen_oapi_codegen_v2//cmd/oapi-codegen"), executable = True, cfg = "exec"),
        "_preflight": attr.label(default = Label("//api_spec/private/codegen:preflight"), executable = True, cfg = "exec"),
    },
    toolchains = ["@rules_go//go:toolchain"],
)

def _python_client_impl(ctx):
    info = ctx.attr.spec[ApiSpecInfo]
    root = _root(ctx, info)
    out = ctx.actions.declare_directory(ctx.attr.out if ctx.attr.out else ctx.label.name)
    stamp = _preflight(ctx, info, root)
    ctx.actions.run(
        executable = ctx.executable._generator,
        inputs = depset([stamp, ctx.file._config], transitive = [info.files]),
        outputs = [out],
        arguments = [
            "generate",
            "--path=" + root.path,
            "--output-path=" + out.path,
            "--config=" + ctx.file._config.path,
            "--meta=none",
            "--overwrite",
        ],
        env = {"HOME": "/nonexistent"},
        execution_requirements = {"block-network": "1"},
        mnemonic = "OpenapiPythonClient",
        progress_message = "openapi-python-client " + root.short_path,
    )
    return _client_providers(ctx, out)

openapi_python_client = rule(
    implementation = _python_client_impl,
    doc = "Generates one Python client package directory with openapi-python-client from an OpenAPI root of an api_spec_library.",
    attrs = {
        "out": attr.string(doc = "Output directory name; defaults to NAME."),
        "root": attr.label(allow_single_file = [".yaml", ".yml", ".json"], doc = "The root document; required when the spec has more than one."),
        "spec": attr.label(providers = [ApiSpecInfo], mandatory = True, doc = "The api_spec_library to generate from."),
        "_config": attr.label(default = Label("//api_spec/private/codegen:openapi_python_client.yaml"), allow_single_file = True),
        "_generator": attr.label(default = Label("//core/quality/tools/python:openapi_python_client"), executable = True, cfg = "exec"),
        "_preflight": attr.label(default = Label("//api_spec/private/codegen:preflight"), executable = True, cfg = "exec"),
    },
)

def _rust_client_impl(ctx):
    info = ctx.attr.spec[ApiSpecInfo]
    root = _root(ctx, info)
    out = ctx.actions.declare_file(ctx.attr.out if ctx.attr.out else ctx.label.name + ".rs")
    stamp = _preflight(ctx, info, root)
    ctx.actions.run(
        executable = ctx.executable._generator,
        inputs = depset([stamp], transitive = [info.files]),
        outputs = [out],
        arguments = [root.path, out.path],
        execution_requirements = {"block-network": "1"},
        mnemonic = "OpenapiRustClient",
        progress_message = "progenitor " + root.short_path,
    )
    return _client_providers(ctx, out)

openapi_rust_client = rule(
    implementation = _rust_client_impl,
    doc = "Generates one Rust client file with progenitor from an OpenAPI root of an api_spec_library. The file needs progenitor-client, reqwest, serde and serde_json at compile time.",
    attrs = {
        "out": attr.string(doc = "Output file name; defaults to NAME.rs."),
        "root": attr.label(allow_single_file = [".yaml", ".yml", ".json"], doc = "The root document; required when the spec has more than one."),
        "spec": attr.label(providers = [ApiSpecInfo], mandatory = True, doc = "The api_spec_library to generate from."),
        "_generator": attr.label(default = Label("//rust/private/quality/tools:progenitor"), executable = True, cfg = "exec"),
        "_preflight": attr.label(default = Label("//api_spec/private/codegen:preflight"), executable = True, cfg = "exec"),
    },
)

def _shard_impl(ctx):
    generated = ctx.attr.generated
    if OutputGroupInfo not in generated or OPENAPI_GENERATED_SRCS_GROUP not in generated[OutputGroupInfo]:
        fail(
            "openapi_codegen_shard " + display_label(ctx.label) + ": " +
            display_label(generated.label) + " has no " + OPENAPI_GENERATED_SRCS_GROUP +
            " output group (want an openapi_typescript_client, openapi_go_client, openapi_python_client or openapi_rust_client)",
        )
    return codegen_bound_shard_providers(
        ctx,
        "openapi_codegen_shard",
        "openapi",
        display_label(generated.label),
        OPENAPI_GENERATED_SRCS_GROUP,
        generated[OutputGroupInfo][OPENAPI_GENERATED_SRCS_GROUP].to_list(),
    )

openapi_codegen_shard = rule(
    implementation = _shard_impl,
    doc = "Declares codegen plan entries for the file an openapi client rule generates. The file needs one entry with an EXEC_PATH suffix.",
    attrs = {
        "generated": attr.label(doc = "openapi_typescript_client, openapi_go_client, openapi_python_client or openapi_rust_client whose output the entries bind.", mandatory = True),
        "entries": attr.string_list(
            doc = "LOGICAL_PATH|IMPORT_ROOT|NAMESPACE|EXEC_PATH entries, one per generated file.",
            mandatory = True,
        ),
        "language": attr.string(doc = "typescript, go, python or rust.", mandatory = True),
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)
