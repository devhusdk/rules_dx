"""Normalized codegen plan records."""

load("//core:defs.bzl", "DxSubjectInfo", "display_label")
load(
    "//core:plan_shard.bzl",
    "plan_shard_aspect_inputs",
    "plan_shard_conflict_error",
    "plan_shard_edge_targets",
    "plan_shard_emit",
    "plan_shard_fingerprint",
    "plan_shard_merge_records",
    "plan_shard_path_is_absolute",
    "plan_shard_record_error",
    "plan_shard_subject_files",
    "plan_shard_subject_records",
)

DxCodegenPlanInfo = provider(
    doc = "Normalized codegen plan records: direct plus transitive collection.",
    fields = {
        "direct": "List of codegen record structs contributed by this target.",
        "transitive": "Depset of codegen record structs from the closure.",
    },
)

DxCodegenPlanCollectedInfo = provider(
    doc = "Aspect-merged codegen plan records from the traversed closure.",
    fields = {
        "records": "Depset of merged codegen record structs.",
    },
)

DxCodegenSourcesInfo = provider(
    doc = "Workspace-relative sources and BUILD files the traversed closure reads.",
    fields = {
        "paths": "Depset of workspace-relative paths.",
    },
)

DX_CODEGEN_PLAN_OUTPUT_GROUP = "dx_codegen_plans"

CODEGEN_RUST_EDITION = "2021"

def codegen_rust_library_kwargs(name, generated, crate_name, runtime, deps, kwargs):
    """Arguments of the rust_library wrapping a generated crate."""
    args = {
        "crate_name": (crate_name or name).replace("-", "_"),
        "crate_root": None,
        "edition": CODEGEN_RUST_EDITION,
        "rustc_flags": ["--cap-lints=allow"],
    } | kwargs
    args["name"] = name
    args["srcs"] = [generated] + kwargs.get("srcs", [])
    args["deps"] = runtime + deps + kwargs.get("deps", [])
    return args

DX_CODEGEN_SOURCES_SUFFIX = ".dxcodegen.src"

DX_CODEGEN_SHARD_SUFFIX = ".dxcodegen.pb"

DX_CODEGEN_ADMITTED_PAIRS = (
    ("protobuf", "rust"),
    ("openapi", "typescript"),
    ("openapi", "go"),
    ("openapi", "python"),
    ("openapi", "rust"),
    ("flatbuffers", "cpp"),
    ("flatbuffers", "rust"),
    ("flatbuffers", "java"),
    ("capnp", "cpp"),
    ("capnp", "rust"),
    ("capnp", "java"),
    ("thrift", "cpp"),
    ("thrift", "rust"),
    ("thrift", "java"),
    ("avro", "cpp"),
    ("avro", "java"),
)

def codegen_path_error(path):
    """Validates one workspace-relative projection path."""
    if path == "":
        return "invalid codegen path '': must be a non-empty workspace-relative path"
    if plan_shard_path_is_absolute(path):
        return "invalid codegen path '" + path + "': must not be absolute"
    if "\\" in path:
        return "invalid codegen path '" + path + "': must not contain '\\'"
    for segment in path.split("/"):
        if segment == "." or segment == "..":
            return "invalid codegen path '" + path + "': must not contain '.' or '..' segments"
    return ""

def codegen_entry(logical_path, import_root, namespace = "", exec_path = "", replaces = ""):
    """Builds one normalized projection entry struct."""
    return struct(
        exec_path = exec_path,
        import_root = import_root,
        logical_path = logical_path,
        namespace = namespace,
        read_only = True,
        replaces = replaces,
    )

def codegen_record(producer, language, entries):
    """Builds one normalized contributor record struct."""
    return struct(
        entries = tuple(entries),
        language = language,
        producer = producer,
    )

def _codegen_entry_error(entry):
    error = codegen_path_error(entry.logical_path)
    if error != "":
        return error
    error = codegen_path_error(entry.import_root)
    if error != "":
        return error
    error = codegen_exec_error(entry.exec_path)
    if error != "":
        return error
    return codegen_replaces_error(entry.logical_path, entry.exec_path, entry.replaces)

def _codegen_owner_of(record):
    return (record.producer, record.language)

def _codegen_claim_key(entry):
    return entry.logical_path

def codegen_record_error(record):
    """Validates one contributor record."""
    second_error = ""
    if record.language == "":
        second_error = "language must be a non-empty file class"
    return plan_shard_record_error(
        record.producer,
        "codegen",
        second_error,
        record.entries,
        _codegen_entry_error,
        _codegen_claim_key,
        "logical path",
    )

def codegen_exec_error(path):
    """Validates one BEP-matching exec-path suffix."""
    if path == "":
        return ""
    if path.endswith(DX_CODEGEN_SHARD_SUFFIX):
        return "invalid codegen exec path '" + path + "': must not use the reserved shard suffix '" + DX_CODEGEN_SHARD_SUFFIX + "'"
    error = codegen_path_error(path)
    if error != "":
        return error.replace("invalid codegen path", "invalid codegen exec path", 1)
    return ""

def codegen_replaces_error(logical_path, exec_path, replaces):
    """Validates one replacement contract declaration."""
    if replaces == "":
        return ""
    error = codegen_path_error(replaces)
    if error != "":
        return error.replace("invalid codegen path", "invalid codegen replaces", 1)
    if replaces != logical_path:
        return "invalid codegen replaces '" + replaces + "': must equal logical path '" + logical_path + "'"
    if exec_path == "":
        return "invalid codegen replaces '" + replaces + "': needs a non-empty exec path binding the replacing artifact"
    return ""

def _codegen_entry_key(entry):
    return (entry.logical_path, entry.import_root, entry.namespace, entry.exec_path, entry.replaces)

def codegen_conflict_error(records):
    """Detects incompatible logical-path claims across records."""
    return plan_shard_conflict_error(
        records,
        _codegen_owner_of,
        _codegen_entry_key,
        _codegen_claim_key,
        "codegen path conflict",
        "logical path",
    )

def _codegen_encode_record(record):
    return {
        "entries": [
            {
                "exec_path": entry.exec_path,
                "import_root": entry.import_root,
                "logical_path": entry.logical_path,
                "namespace": entry.namespace,
                "read_only": entry.read_only,
                "replaces": entry.replaces,
            }
            for entry in record.entries
        ],
        "language": record.language,
        "producer": record.producer,
    }

def codegen_merge_records(records):
    """Merges records into deterministic normalized order."""
    return plan_shard_merge_records(
        records,
        _codegen_owner_of,
        _codegen_entry_key,
        codegen_record,
    )

def codegen_plan_fingerprint(records):
    """Renders the normalized complete-plan hash input."""
    return plan_shard_fingerprint(records, codegen_merge_records, _codegen_encode_record)

def codegen_pair_error(schema_kind, language):
    """Validates one generator/language pair against the admitted pairs."""
    if (schema_kind, language) in DX_CODEGEN_ADMITTED_PAIRS:
        return ""
    return (
        "unsupported codegen pair ('" + schema_kind + "', '" + language + "'): " +
        "admitted first-release pairs are " + str(DX_CODEGEN_ADMITTED_PAIRS)
    )

def _checked_record(producer, language, entry_structs):
    """Builds one record and fails on its schema error."""
    record = codegen_record(producer, language, entry_structs)
    record_error = codegen_record_error(record)
    if record_error != "":
        fail("codegen " + producer + ": " + record_error)
    return record

CODEGEN_SHARD_WRITER_LABEL = Label("@dx_prebuilt//:codegen_shard_writer")

CODEGEN_LIBRARY_ROOT = "gen"

def codegen_library_file(file, path):
    """Pairs a generated file with its path below the codegen_path directory."""
    return struct(file = file, path = path)

def _codegen_plan_outputs(ctx, producer, schema_kind, language, root, files, namespace):
    pair_error = codegen_pair_error(schema_kind, language)
    if pair_error != "":
        fail("codegen " + producer + ": " + pair_error)
    specs = []
    entries = []
    for f in files:
        logical = root + "/" + f.path
        entries.append(codegen_entry(logical, CODEGEN_LIBRARY_ROOT, namespace, f.file.short_path))
        specs.append("|".join([logical, CODEGEN_LIBRARY_ROOT, namespace, f.file.short_path]))
    record = _checked_record(producer, language, entries)
    out = plan_shard_emit(ctx, "codegen", producer, "--language", language, specs, DX_CODEGEN_SHARD_SUFFIX, "DxCodegenShard")
    return struct(record = record, files = [out] + [f.file for f in files])

_PROST_LIBRARY_KIND = "rust_prost_library"

def _prost_plan(target, ctx):
    if ctx.rule.kind != _PROST_LIBRARY_KIND or OutputGroupInfo not in target:
        return None
    groups = target[OutputGroupInfo]
    if "rust_generated_srcs" not in groups:
        return None
    generated = groups.rust_generated_srcs.to_list()
    if len(generated) == 0:
        return None
    files = [codegen_package_file(f) for f in generated]
    return _codegen_plan_outputs(ctx, display_label(target.label), "protobuf", "rust", CODEGEN_LIBRARY_ROOT, files, "")

_CODEGEN_ASPECT_ATTRS = ["deps", "proto", "generated"]

def _dx_codegen_plan_aspect_impl(target, ctx):
    inputs = plan_shard_aspect_inputs(
        target,
        ctx,
        DxCodegenPlanInfo,
        DxCodegenPlanCollectedInfo,
        DX_CODEGEN_PLAN_OUTPUT_GROUP,
        _CODEGEN_ASPECT_ATTRS,
    )
    own = []
    if ctx.label.workspace_name == "":
        own.append(ctx.label.package + "/BUILD.bazel" if ctx.label.package else "BUILD.bazel")
        own.extend([f.path for f in getattr(ctx.rule.files, "srcs", []) if f.is_source and not f.path.startswith("external/")])
    transitive_paths = []
    for name in _CODEGEN_ASPECT_ATTRS:
        for dep in plan_shard_edge_targets(ctx.rule.attr, name):
            if DxCodegenSourcesInfo in dep:
                transitive_paths.append(dep[DxCodegenSourcesInfo].paths)
    paths = depset(own, transitive = transitive_paths)
    records = inputs.merged_records
    files = list(inputs.direct_files)
    prost = _prost_plan(target, ctx)
    if prost != None:
        records = depset([prost.record], transitive = [records])
        files.extend(prost.files)
    if prost != None or (DxCodegenPlanInfo in target and target[DxCodegenPlanInfo].direct):
        sources = ctx.actions.declare_file(ctx.label.name + DX_CODEGEN_SOURCES_SUFFIX)
        ctx.actions.write(sources, "".join([path + "\n" for path in sorted(paths.to_list())]))
        files.append(sources)
    return [
        DxCodegenPlanCollectedInfo(records = records),
        DxCodegenSourcesInfo(paths = paths),
        OutputGroupInfo(dx_codegen_plans = depset(files, transitive = inputs.transitive_files)),
    ]

dx_codegen_plan_aspect = aspect(
    implementation = _dx_codegen_plan_aspect_impl,
    doc = "Collects codegen plan shards through deps.",
    attr_aspects = _CODEGEN_ASPECT_ATTRS,
    attrs = {
        "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
    },
)

CODEGEN_LIBRARY_ATTRS = {
    "codegen_path": attr.string(doc = "Logical directory of the generated files in the dx sync projection, gen by default."),
    "_writer": attr.label(default = CODEGEN_SHARD_WRITER_LABEL, executable = True, cfg = "exec"),
}

def codegen_tree_file(file):
    """Pairs a file with its path below its generator's output directory."""
    pkg = file.owner.package
    rest = file.short_path[len(pkg) + 1:] if pkg else file.short_path
    return codegen_library_file(file, rest.partition("/")[2])

def codegen_package_file(file):
    """Pairs a file with its owner package and file name."""
    pkg = file.owner.package
    return codegen_library_file(file, (pkg + "/" if pkg else "") + file.basename)

def codegen_library_providers(ctx, schema_kind, language, generated, namespace = ""):
    """Builds the plan providers of a library rule from a File or a list of codegen_library_file."""
    producer = display_label(ctx.label)
    files = [codegen_library_file(generated, generated.basename)] if type(generated) == "File" else generated
    if len(files) == 0:
        return [DxCodegenPlanInfo(direct = [], transitive = depset()), OutputGroupInfo(dx_codegen_plans = depset())]
    plan = _codegen_plan_outputs(ctx, producer, schema_kind, language, ctx.attr.codegen_path or CODEGEN_LIBRARY_ROOT, files, namespace)
    return [
        DxCodegenPlanInfo(direct = [plan.record], transitive = depset([plan.record])),
        OutputGroupInfo(dx_codegen_plans = depset(plan.files)),
    ]

def codegen_srcjar(ctx, tree, srcjar_name, mnemonic):
    """Packs one generated tree artifact into a .srcjar and returns the File."""
    srcjar = ctx.actions.declare_file(srcjar_name)
    zip_args = ctx.actions.args()
    zip_args.add("c")
    zip_args.add(srcjar)
    zip_args.add_all([tree], expand_directories = True, map_each = _zip_entry)
    ctx.actions.run(
        executable = ctx.executable._zipper,
        toolchain = None,
        inputs = [tree],
        outputs = [srcjar],
        arguments = [zip_args],
        mnemonic = mnemonic,
        progress_message = "Packing " + srcjar.short_path,
    )
    return srcjar

def _zip_entry(f):
    return f.tree_relative_path + "=" + f.path

def _codegen_plan_subject_impl(ctx):
    target = ctx.attr.target
    records = plan_shard_subject_records(target, DxCodegenPlanInfo, DxCodegenPlanCollectedInfo)
    conflict = codegen_conflict_error(records)
    if conflict != "":
        fail("codegen_plan_subject on " + display_label(target.label) + ": " + conflict)
    shard_files = plan_shard_subject_files(target, DX_CODEGEN_PLAN_OUTPUT_GROUP)
    plan = {
        "files": ",".join([f.basename for f in shard_files]),
        "fingerprint": codegen_plan_fingerprint(records),
        "label": display_label(ctx.attr.target.label),
        "record_count": str(len(codegen_merge_records(records))),
    }
    return [
        DefaultInfo(files = depset([])),
        OutputGroupInfo(dx_codegen_plans = depset(shard_files)),
        DxSubjectInfo(fields = plan),
    ]

codegen_plan_subject = rule(
    implementation = _codegen_plan_subject_impl,
    doc = "Analysis subject reporting the codegen plan of one target.",
    attrs = {
        "target": attr.label(
            doc = "Target whose codegen plan this subject reports.",
            aspects = [dx_codegen_plan_aspect],
            mandatory = True,
        ),
    },
)
