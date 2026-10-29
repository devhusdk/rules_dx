"""Normalized codegen plan records."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label")
load(
    "//libs/starlark:plan_shard.bzl",
    "plan_shard_aspect_inputs",
    "plan_shard_conflict_error",
    "plan_shard_emit",
    "plan_shard_exec_matches",
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

DX_CODEGEN_PLAN_OUTPUT_GROUP = "dx_codegen_plans"

DX_CODEGEN_SHARD_SUFFIX = ".dxcodegen.pb"

DX_CODEGEN_ADMITTED_PAIRS = (
    ("protobuf", "rust"),
    ("openapi", "typescript"),
    ("flatbuffers", "cpp"),
    ("flatbuffers", "rust"),
    ("flatbuffers", "java"),
    ("capnp", "cpp"),
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

def _parse_entry_spec(spec, label_text):
    """Parses one LOGICAL|ROOT|NAMESPACE[|EXEC[|REPLACES]] entry spec."""
    parts = spec.split("|")
    if len(parts) == 3:
        return codegen_entry(parts[0], parts[1], parts[2])
    if len(parts) == 4:
        return codegen_entry(parts[0], parts[1], parts[2], parts[3])
    if len(parts) == 5:
        return codegen_entry(parts[0], parts[1], parts[2], parts[3], parts[4])
    fail(
        "dx_codegen_shard " + label_text +
        ": bad entry " + repr(spec) +
        ": want LOGICAL_PATH|IMPORT_ROOT|NAMESPACE[|EXEC_PATH[|REPLACES]]",
    )

def _checked_record(producer, language, entry_structs):
    """Builds one record and fails on its schema error."""
    record = codegen_record(producer, language, entry_structs)
    record_error = codegen_record_error(record)
    if record_error != "":
        fail("dx_codegen_shard " + producer + ": " + record_error)
    return record

def _dx_codegen_shard_impl(ctx):
    producer = display_label(ctx.label)
    pair_error = codegen_pair_error(ctx.attr.schema_kind, ctx.attr.language)
    if pair_error != "":
        fail("dx_codegen_shard " + producer + ": " + pair_error)
    entries = [_parse_entry_spec(spec, producer) for spec in ctx.attr.entries]
    record = _checked_record(producer, ctx.attr.language, entries)
    out = plan_shard_emit(ctx, "codegen", producer, "--language", ctx.attr.language, ctx.attr.entries, DX_CODEGEN_SHARD_SUFFIX, "DxCodegenShard")
    return [
        DefaultInfo(files = depset([out])),
        DxCodegenPlanInfo(direct = [record], transitive = depset([record])),
        OutputGroupInfo(dx_codegen_plans = depset([out])),
    ]

dx_codegen_shard = rule(
    implementation = _dx_codegen_shard_impl,
    doc = "Declares codegen plan entries for dx codegen.",
    attrs = {
        "deps": attr.label_list(
            doc = "Other codegen shard targets whose plans are collected.",
            default = [],
        ),
        "entries": attr.string_list(
            doc = "Pipe separated codegen entries of logical path, root and namespace.",
            mandatory = True,
        ),
        "language": attr.string(
            doc = "Target language of the generated code.",
            mandatory = True,
        ),
        "schema_kind": attr.string(
            doc = "Schema kind of the inputs, protobuf by default.",
            default = "protobuf",
        ),
        "_writer": attr.label(
            default = "//generation/codegen_shard:codegen_shard_writer",
            executable = True,
            cfg = "exec",
        ),
    },
)

_CODEGEN_ASPECT_ATTRS = ["deps", "proto", "proto_rs"]

def _dx_codegen_plan_aspect_impl(target, ctx):
    inputs = plan_shard_aspect_inputs(
        target,
        ctx,
        DxCodegenPlanInfo,
        DxCodegenPlanCollectedInfo,
        DX_CODEGEN_PLAN_OUTPUT_GROUP,
        _CODEGEN_ASPECT_ATTRS,
    )
    conflict = codegen_conflict_error(inputs.merged_records.to_list())
    if conflict != "":
        fail("dx_codegen_plan_aspect on " + display_label(target.label) + ": " + conflict)
    return [
        DxCodegenPlanCollectedInfo(records = inputs.merged_records),
        OutputGroupInfo(dx_codegen_plans = depset(inputs.direct_files, transitive = inputs.transitive_files)),
    ]

dx_codegen_plan_aspect = aspect(
    implementation = _dx_codegen_plan_aspect_impl,
    doc = "Collects codegen plan shards through deps.",
    attr_aspects = _CODEGEN_ASPECT_ATTRS,
)

def codegen_bind_generated(rule_kind, producer, upstream_label, group, generated, entries):
    """Fails unless every generated artifact is claimed once by an EXEC_PATH entry.

    Shared by the codegen shard rules that bind plan entries to generated files.
    """
    for entry in entries:
        if entry.exec_path == "":
            fail(
                rule_kind + " " + producer + ": entry '" +
                entry.logical_path + "' needs an EXEC_PATH suffix binding it to one " + group + " artifact (logical-only entries carry no backing artifact)",
            )
    for entry in entries:
        matches = [f for f in generated if plan_shard_exec_matches(f.path, entry.exec_path)]
        if len(matches) == 0:
            fail(
                rule_kind + " " + producer + ": entry '" +
                entry.logical_path + "' with EXEC_PATH '" + entry.exec_path +
                "' matches no " + group + " artifact from " +
                upstream_label,
            )
        if len(matches) > 1:
            fail(
                rule_kind + " " + producer + ": entry '" +
                entry.logical_path + "' with EXEC_PATH '" + entry.exec_path +
                "' is ambiguous: matches " + str(len(matches)) +
                " " + group + " artifacts, want exactly one",
            )
    for f in generated:
        claimants = [entry for entry in entries if plan_shard_exec_matches(f.path, entry.exec_path)]
        if len(claimants) == 0:
            fail(
                rule_kind + " " + producer + ": unreported " + group + " artifact '" +
                f.path + "' from " + upstream_label +
                ": every generated artifact needs one claiming entry",
            )
        if len(claimants) > 1:
            fail(
                rule_kind + " " + producer + ": duplicate claim on " + group + " artifact '" +
                f.path + "': claimed by " + str(len(claimants)) + " entries, want exactly one",
            )

def codegen_bound_shard_providers(ctx, rule_kind, schema_kind, upstream_label, group, generated):
    """Builds the providers of a shard rule whose entries bind to generated files.

    The rule needs `entries`, `language` and `_writer` attributes.
    The generated files ride in the shard's plan output group so they are built
    with the plan.
    """
    producer = display_label(ctx.label)
    pair_error = codegen_pair_error(schema_kind, ctx.attr.language)
    if pair_error != "":
        fail(rule_kind + " " + producer + ": " + pair_error)
    if len(generated) == 0:
        fail(rule_kind + " " + producer + ": upstream " + upstream_label + " emitted no generated sources")
    entries = [_parse_entry_spec(spec, producer) for spec in ctx.attr.entries]
    codegen_bind_generated(rule_kind, producer, upstream_label, group, generated, entries)
    record = _checked_record(producer, ctx.attr.language, entries)
    out = plan_shard_emit(ctx, "codegen", producer, "--language", ctx.attr.language, ctx.attr.entries, DX_CODEGEN_SHARD_SUFFIX, "DxCodegenShard")
    return [
        DefaultInfo(files = depset([out])),
        DxCodegenPlanInfo(direct = [record], transitive = depset([record])),
        OutputGroupInfo(dx_codegen_plans = depset([out] + generated)),
    ]

CODEGEN_SHARD_WRITER_LABEL = Label("//generation/codegen_shard:codegen_shard_writer")

def _prost_codegen_shard_impl(ctx):
    producer = display_label(ctx.label)
    pair_error = codegen_pair_error(ctx.attr.schema_kind, ctx.attr.language)
    if pair_error != "":
        fail("prost_codegen_shard " + producer + ": " + pair_error)
    upstream = ctx.attr.proto_rs
    if OutputGroupInfo not in upstream:
        fail(
            "prost_codegen_shard " + producer + ": upstream " +
            display_label(upstream.label) + " carries no OutputGroupInfo",
        )
    groups = upstream[OutputGroupInfo]
    if "rust_generated_srcs" not in groups:
        fail(
            "prost_codegen_shard " + producer + ": upstream " +
            display_label(upstream.label) +
            " has no rust_generated_srcs output group (not a rust_prost_library)",
        )
    generated = groups["rust_generated_srcs"].to_list()
    if len(generated) == 0:
        fail(
            "prost_codegen_shard " + producer + ": upstream " +
            display_label(upstream.label) + " emitted no generated Rust sources",
        )
    entries = [_parse_entry_spec(spec, producer) for spec in ctx.attr.entries]
    codegen_bind_generated("prost_codegen_shard", producer, display_label(upstream.label), "rust_generated_srcs", generated, entries)
    record = _checked_record(producer, ctx.attr.language, entries)
    out = plan_shard_emit(ctx, "codegen", producer, "--language", ctx.attr.language, ctx.attr.entries, DX_CODEGEN_SHARD_SUFFIX, "DxCodegenShard")
    return [
        DefaultInfo(files = depset([out])),
        DxCodegenPlanInfo(direct = [record], transitive = depset([record])),
        OutputGroupInfo(dx_codegen_plans = depset([out] + generated)),
    ]

prost_codegen_shard = rule(
    implementation = _prost_codegen_shard_impl,
    doc = "Declares codegen plan entries for the sources prost generates.",
    attrs = {
        "entries": attr.string_list(
            doc = "Codegen entry specs for the sources prost generates.",
            mandatory = True,
        ),
        "language": attr.string(
            doc = "Target language of the generated code, rust by default.",
            default = "rust",
        ),
        "proto_rs": attr.label(
            doc = "Target providing the prost generated Rust sources.",
            mandatory = True,
        ),
        "schema_kind": attr.string(
            doc = "Schema kind of the inputs, protobuf by default.",
            default = "protobuf",
        ),
        "_writer": attr.label(
            default = "//generation/codegen_shard:codegen_shard_writer",
            executable = True,
            cfg = "exec",
        ),
    },
)

def _codegen_plan_subject_impl(ctx):
    target = ctx.attr.target
    records = plan_shard_subject_records(target, DxCodegenPlanInfo, DxCodegenPlanCollectedInfo)
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
