"""Normalized codegen plan records."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label")
load(
    "//libs/starlark:plan_shard.bzl",
    "plan_shard_aspect_inputs",
    "plan_shard_conflict_error",
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

CODEGEN_SCHEMA_VERSION = 1

DX_CODEGEN_ADMITTED_PAIRS = (
    ("protobuf", "rust"),
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

def codegen_merge_schema_error(records, merged):
    """Validates merged is the normalized form of records."""
    if type(merged) != "list":
        return "codegen merge: want a list, got " + type(merged)
    owners = []
    for record in merged:
        err = codegen_record_error(record)
        if err != "":
            return "codegen merge: " + err
        owners.append((record.producer, record.language))
    if owners != sorted(owners):
        return "codegen merge: owners must sort by (producer, language): " + str(owners)
    seen_owners = {}
    for owner in owners:
        if owner in seen_owners:
            return "codegen merge: duplicate owner " + str(owner)
        seen_owners[owner] = True
    for record in merged:
        keys = [_codegen_entry_key(e) for e in record.entries]
        if keys != sorted(keys):
            return "codegen merge: entries for " + record.producer + " must sort by (logical, root, namespace, exec, replaces)"
        seen_keys = {}
        for key in keys:
            if key in seen_keys:
                return "codegen merge: duplicate entry " + str(key) + " for " + record.producer
            seen_keys[key] = True
    input_by_owner = {}
    for record in records:
        owner = (record.producer, record.language)
        owned = input_by_owner.setdefault(owner, {})
        for entry in record.entries:
            owned[_codegen_entry_key(entry)] = True
    merged_by_owner = {}
    for record in merged:
        owner = (record.producer, record.language)
        owned = merged_by_owner.setdefault(owner, {})
        for entry in record.entries:
            owned[_codegen_entry_key(entry)] = True
    if sorted(input_by_owner.keys()) != sorted(merged_by_owner.keys()):
        return "codegen merge: owners must match input owners: " + str(sorted(merged_by_owner.keys())) + " vs " + str(sorted(input_by_owner.keys()))
    for owner in input_by_owner.keys():
        if sorted(input_by_owner[owner].keys()) != sorted(merged_by_owner[owner].keys()):
            return "codegen merge: entries for " + str(owner) + " must match deduped input entries"
    return ""

def codegen_plan_fingerprint(records):
    """Renders the normalized complete-plan hash input."""
    return plan_shard_fingerprint(records, codegen_merge_records, _codegen_encode_record)

def codegen_fingerprint_schema_error(fingerprint):
    """Validates a plan fingerprint JSON shape."""
    decoded = json.decode(fingerprint)
    if type(decoded) != "list" or len(decoded) == 0:
        return "codegen fingerprint: want a non-empty list"
    owners = []
    for item in decoded:
        if type(item) != "dict":
            return "codegen fingerprint: want objects, got " + type(item)
        for key in ("producer", "language", "entries"):
            if key not in item:
                return "codegen fingerprint: missing key '" + key + "'"
        producer = item["producer"]
        language = item["language"]
        entries = item["entries"]
        if type(producer) != "string" or producer == "":
            return "codegen fingerprint: producer must be non-empty"
        if not (producer.startswith("//") or producer.startswith("@")):
            return "codegen fingerprint: producer '" + producer + "' must be a label"
        if type(language) != "string" or language == "":
            return "codegen fingerprint: language must be non-empty"
        if type(entries) != "list" or len(entries) == 0:
            return "codegen fingerprint: entries must be non-empty for " + producer
        owners.append((producer, language))
        keys = []
        for entry in entries:
            if type(entry) != "dict":
                return "codegen fingerprint: want entry objects for " + producer
            for ekey in ("exec_path", "import_root", "logical_path", "namespace", "read_only", "replaces"):
                if ekey not in entry:
                    return "codegen fingerprint: missing entry key '" + ekey + "' for " + producer
            if entry["read_only"] != True:
                return "codegen fingerprint: read_only must stay true for " + producer
            err = codegen_path_error(entry["logical_path"])
            if err != "":
                return "codegen fingerprint: " + err
            err = codegen_path_error(entry["import_root"])
            if err != "":
                return "codegen fingerprint: " + err
            err = codegen_exec_error(entry["exec_path"])
            if err != "":
                return "codegen fingerprint: " + err
            err = codegen_replaces_error(entry["logical_path"], entry["exec_path"], entry["replaces"])
            if err != "":
                return "codegen fingerprint: " + err
            keys.append((entry["logical_path"], entry["import_root"], entry["namespace"], entry["exec_path"], entry["replaces"]))
        if keys != sorted(keys):
            return "codegen fingerprint: entries for " + producer + " must sort by (logical, root, namespace, exec, replaces)"
    if owners != sorted(owners):
        return "codegen fingerprint: records must sort by (producer, language): " + str(owners)
    return ""

def codegen_admitted_pairs():
    """Returns the admitted generator/language pairs via registry query."""
    return DX_CODEGEN_ADMITTED_PAIRS

def codegen_schema_error():
    """Validates the versioned codegen-pair schema."""
    if CODEGEN_SCHEMA_VERSION != 1:
        return "codegen: unsupported schema v" + str(CODEGEN_SCHEMA_VERSION) + " (want v1)"
    if type(DX_CODEGEN_ADMITTED_PAIRS) != "tuple" or len(DX_CODEGEN_ADMITTED_PAIRS) == 0:
        return "codegen: want a non-empty admitted-pair tuple (schema v1)"
    seen = {}
    for pair in DX_CODEGEN_ADMITTED_PAIRS:
        if type(pair) != "tuple" or len(pair) != 2:
            return "codegen: bad admitted pair '" + str(pair) + "': want (schema_kind, language)"
        for token in pair:
            if type(token) != "string" or token == "":
                return "codegen: bad admitted pair '" + str(pair) + "': tokens must be non-empty strings"
        if pair in seen:
            return "codegen: duplicate admitted pair '" + str(pair) + "'"
        seen[pair] = True
    return ""

def codegen_pair_error(schema_kind, language):
    """Validates one generator/language pair against the frozen."""
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

def _emit_shard(ctx, producer, language, entry_structs):
    """Validates one record and emits its binary shard via the writer."""
    record = codegen_record(producer, language, entry_structs)
    record_error = codegen_record_error(record)
    if record_error != "":
        fail("dx_codegen_shard " + producer + ": " + record_error)
    out = ctx.actions.declare_file(ctx.label.name + DX_CODEGEN_SHARD_SUFFIX)
    args = ctx.actions.args()
    args.add("--producer", producer)
    args.add("--language", language)
    for entry in entry_structs:
        if entry.exec_path == "":
            args.add(
                "--entry",
                entry.logical_path + "|" + entry.import_root + "|" + entry.namespace,
            )
        elif entry.replaces == "":
            args.add(
                "--entry",
                entry.logical_path + "|" + entry.import_root + "|" + entry.namespace + "|" + entry.exec_path,
            )
        else:
            args.add(
                "--entry",
                entry.logical_path + "|" + entry.import_root + "|" + entry.namespace + "|" + entry.exec_path + "|" + entry.replaces,
            )
    args.add("--output", out.path)
    ctx.actions.run(
        executable = ctx.executable._writer,
        arguments = [args],
        outputs = [out],
        mnemonic = "DxCodegenShard",
        progress_message = "Dx codegen shard %{label}",
    )
    return out, record

def _exec_matches(file_path, exec_path):
    """Reports whether a Bazel file path satisfies an exec-path suffix."""
    return plan_shard_exec_matches(file_path, exec_path)

def _dx_codegen_shard_impl(ctx):
    producer = display_label(ctx.label)
    pair_error = codegen_pair_error(ctx.attr.schema_kind, ctx.attr.language)
    if pair_error != "":
        fail("dx_codegen_shard " + producer + ": " + pair_error)
    entries = [_parse_entry_spec(spec, producer) for spec in ctx.attr.entries]
    out, record = _emit_shard(ctx, producer, ctx.attr.language, entries)
    return [
        DefaultInfo(files = depset([out])),
        DxCodegenPlanInfo(direct = [record], transitive = depset([record])),
        OutputGroupInfo(dx_codegen_plans = depset([out])),
    ]

dx_codegen_shard = rule(
    implementation = _dx_codegen_shard_impl,
    attrs = {
        "deps": attr.label_list(
            default = [],
        ),
        "entries": attr.string_list(
            mandatory = True,
        ),
        "language": attr.string(
            mandatory = True,
        ),
        "schema_kind": attr.string(
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
    attr_aspects = _CODEGEN_ASPECT_ATTRS,
)

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
    for entry in entries:
        if entry.exec_path == "":
            fail(
                "prost_codegen_shard " + producer + ": entry '" +
                entry.logical_path + "' needs an EXEC_PATH suffix binding it to one rust_generated_srcs artifact (logical-only entries carry no backing artifact)",
            )
    for entry in entries:
        matches = [f for f in generated if _exec_matches(f.path, entry.exec_path)]
        if len(matches) == 0:
            fail(
                "prost_codegen_shard " + producer + ": entry '" +
                entry.logical_path + "' with EXEC_PATH '" + entry.exec_path +
                "' matches no rust_generated_srcs artifact from " +
                display_label(upstream.label),
            )
        if len(matches) > 1:
            fail(
                "prost_codegen_shard " + producer + ": entry '" +
                entry.logical_path + "' with EXEC_PATH '" + entry.exec_path +
                "' is ambiguous: matches " + str(len(matches)) +
                " rust_generated_srcs artifacts, want exactly one",
            )
    for f in generated:
        claimants = [entry for entry in entries if _exec_matches(f.path, entry.exec_path)]
        if len(claimants) == 0:
            fail(
                "prost_codegen_shard " + producer + ": unreported rust_generated_srcs artifact '" +
                f.path + "' from " + display_label(upstream.label) +
                ": every generated artifact needs one claiming entry",
            )
        if len(claimants) > 1:
            fail(
                "prost_codegen_shard " + producer + ": duplicate claim on rust_generated_srcs artifact '" +
                f.path + "': claimed by " + str(len(claimants)) + " entries, want exactly one",
            )
    out, record = _emit_shard(ctx, producer, ctx.attr.language, entries)
    return [
        DefaultInfo(files = depset([out])),
        DxCodegenPlanInfo(direct = [record], transitive = depset([record])),
        OutputGroupInfo(dx_codegen_plans = depset([out] + generated)),
    ]

prost_codegen_shard = rule(
    implementation = _prost_codegen_shard_impl,
    attrs = {
        "entries": attr.string_list(
            mandatory = True,
        ),
        "language": attr.string(
            default = "rust",
        ),
        "proto_rs": attr.label(
            mandatory = True,
        ),
        "schema_kind": attr.string(
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
    attrs = {
        "target": attr.label(
            aspects = [dx_codegen_plan_aspect],
            mandatory = True,
        ),
    },
)
