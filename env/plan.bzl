"""Normalized environment plan records."""

load("@rules_rust//rust:defs.bzl", _rust_common = "rust_common")
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

DxEnvPlanInfo = provider(
    doc = "Normalized environment plan records: direct plus transitive collection.",
    fields = {
        "direct": "List of environment record structs contributed by this target.",
        "transitive": "Depset of environment record structs from the closure.",
    },
)

DxEnvPlanCollectedInfo = provider(
    doc = "Aspect-merged environment plan records from the traversed closure.",
    fields = {
        "records": "Depset of merged environment record structs.",
    },
)

DX_ENV_PLAN_OUTPUT_GROUP = "dx_env_plans"

DX_ENV_SHARD_SUFFIX = ".dxenv.pb"

DX_ENV_ADMITTED_INTEGRATIONS = (
    "rust",
)

def env_plan_key_error(key):
    """Validates one identity-dimension key."""
    if key == "":
        return "invalid env plan key '': must be a non-empty single token"
    if "/" in key or "\\" in key:
        return "invalid env plan key '" + key + "': must not contain '/' or '\\'"
    if "|" in key:
        return "invalid env plan key '" + key + "': must not contain '|'"
    return ""

def env_plan_value_error(value):
    """Validates one identity-input value."""
    if value == "":
        return "invalid env plan value '': must be a non-empty identity input"
    if "|" in value:
        return "invalid env plan value '" + value + "': must not contain '|'"
    return ""

def env_plan_exec_error(path):
    """Validates one BEP-matching exec-path suffix."""
    if path == "":
        return ""
    if path.endswith(DX_ENV_SHARD_SUFFIX):
        return "invalid env plan exec path '" + path + "': must not use the reserved shard suffix '" + DX_ENV_SHARD_SUFFIX + "'"
    if plan_shard_path_is_absolute(path):
        return "invalid env plan exec path '" + path + "': must not be absolute"
    if "\\" in path:
        return "invalid env plan exec path '" + path + "': must not contain '\\'"
    for segment in path.split("/"):
        if segment == "." or segment == "..":
            return "invalid env plan exec path '" + path + "': must not contain '.' or '..' segments"
    return ""

def env_plan_entry(key, value, exec_path = ""):
    """Builds one normalized environment identity entry struct."""
    return struct(
        exec_path = exec_path,
        key = key,
        value = value,
    )

def env_plan_record(producer, integration, entries):
    """Builds one normalized contributor record struct."""
    return struct(
        entries = tuple(entries),
        integration = integration,
        producer = producer,
    )

def _env_plan_entry_error(entry):
    error = env_plan_key_error(entry.key)
    if error != "":
        return error
    error = env_plan_value_error(entry.value)
    if error != "":
        return error
    return env_plan_exec_error(entry.exec_path)

def _env_plan_owner_of(record):
    return (record.producer, record.integration)

def _env_plan_claim_key(entry):
    return entry.key

def env_plan_record_error(record):
    """Validates one contributor record."""
    second_error = ""
    if record.integration == "":
        second_error = "integration must be a non-empty language class"
    return plan_shard_record_error(
        record.producer,
        "env plan",
        second_error,
        record.entries,
        _env_plan_entry_error,
        _env_plan_claim_key,
        "key",
    )

def _env_plan_entry_key(entry):
    return (entry.key, entry.value, entry.exec_path)

def env_plan_conflict_error(records):
    """Detects incompatible identity-key claims across records."""
    return plan_shard_conflict_error(
        records,
        _env_plan_owner_of,
        _env_plan_entry_key,
        _env_plan_claim_key,
        "env plan conflict",
        "identity key",
    )

def _env_plan_encode_record(record):
    return {
        "entries": [
            {
                "exec_path": entry.exec_path,
                "key": entry.key,
                "value": entry.value,
            }
            for entry in record.entries
        ],
        "integration": record.integration,
        "producer": record.producer,
    }

def env_plan_merge_records(records):
    """Merges records into deterministic normalized order."""
    return plan_shard_merge_records(
        records,
        _env_plan_owner_of,
        _env_plan_entry_key,
        env_plan_record,
    )

def env_plan_fingerprint(records):
    """Renders the normalized complete-plan hash input."""
    return plan_shard_fingerprint(records, env_plan_merge_records, _env_plan_encode_record)

def env_plan_integration_error(integration):
    """Validates one language integration against the admitted set."""
    if integration in DX_ENV_ADMITTED_INTEGRATIONS:
        return ""
    return (
        "unsupported env plan integration '" + integration + "': " +
        "admitted first-release integrations are " + str(DX_ENV_ADMITTED_INTEGRATIONS)
    )

def _parse_entry_spec(spec, label_text):
    """Parses one KEY|VALUE[|EXEC] entry spec."""
    parts = spec.split("|")
    if len(parts) == 2:
        return env_plan_entry(parts[0], parts[1])
    if len(parts) == 3:
        return env_plan_entry(parts[0], parts[1], parts[2])
    fail(
        "dx_env_shard " + label_text +
        ": bad entry " + repr(spec) +
        ": want KEY|VALUE[|EXEC_PATH]",
    )

def _emit_shard(ctx, producer, integration, entry_structs):
    """Validates one record and emits its binary shard via the writer."""
    record = env_plan_record(producer, integration, entry_structs)
    record_error = env_plan_record_error(record)
    if record_error != "":
        fail("dx_env_shard " + producer + ": " + record_error)
    out = ctx.actions.declare_file(ctx.label.name + DX_ENV_SHARD_SUFFIX)
    args = ctx.actions.args()
    args.add("--producer", producer)
    args.add("--integration", integration)
    for entry in entry_structs:
        if entry.exec_path == "":
            args.add(
                "--entry",
                entry.key + "|" + entry.value,
            )
        else:
            args.add(
                "--entry",
                entry.key + "|" + entry.value + "|" + entry.exec_path,
            )
    args.add("--output", out.path)
    ctx.actions.run(
        executable = ctx.executable._writer,
        arguments = [args],
        outputs = [out],
        mnemonic = "DxEnvShard",
        progress_message = "Dx env shard %{label}",
    )
    return out, record

def _exec_matches(file_path, exec_path):
    """Reports whether a Bazel file path satisfies an exec-path suffix."""
    return plan_shard_exec_matches(file_path, exec_path)

def _dx_env_shard_impl(ctx):
    producer = display_label(ctx.label)
    integration_error = env_plan_integration_error(ctx.attr.integration)
    if integration_error != "":
        fail("dx_env_shard " + producer + ": " + integration_error)
    entries = [_parse_entry_spec(spec, producer) for spec in ctx.attr.entries]
    out, record = _emit_shard(ctx, producer, ctx.attr.integration, entries)
    return [
        DefaultInfo(files = depset([out])),
        DxEnvPlanInfo(direct = [record], transitive = depset([record])),
        OutputGroupInfo(dx_env_plans = depset([out])),
    ]

dx_env_shard = rule(
    implementation = _dx_env_shard_impl,
    attrs = {
        "deps": attr.label_list(
            default = [],
        ),
        "entries": attr.string_list(
            mandatory = True,
        ),
        "integration": attr.string(
            default = "rust",
        ),
        "_writer": attr.label(
            default = "//env/env_shard:env_shard_writer",
            executable = True,
            cfg = "exec",
        ),
    },
)

_ENV_PLAN_ASPECT_ATTRS = ["deps", "target"]

def _dx_env_plan_aspect_impl(target, ctx):
    inputs = plan_shard_aspect_inputs(
        target,
        ctx,
        DxEnvPlanInfo,
        DxEnvPlanCollectedInfo,
        DX_ENV_PLAN_OUTPUT_GROUP,
        _ENV_PLAN_ASPECT_ATTRS,
    )
    conflict = env_plan_conflict_error(inputs.merged_records.to_list())
    if conflict != "":
        fail("dx_env_plan_aspect on " + display_label(target.label) + ": " + conflict)
    return [
        DxEnvPlanCollectedInfo(records = inputs.merged_records),
        OutputGroupInfo(dx_env_plans = depset(inputs.direct_files, transitive = inputs.transitive_files)),
    ]

dx_env_plan_aspect = aspect(
    implementation = _dx_env_plan_aspect_impl,
    attr_aspects = _ENV_PLAN_ASPECT_ATTRS,
)

def _rust_env_shard_impl(ctx):
    producer = display_label(ctx.label)
    integration_error = env_plan_integration_error(ctx.attr.integration)
    if integration_error != "":
        fail("rust_env_shard " + producer + ": " + integration_error)
    upstream = ctx.attr.target
    if _rust_common.crate_info in upstream:
        crate = upstream[_rust_common.crate_info]
    elif _rust_common.test_crate_info in upstream:
        crate = upstream[_rust_common.test_crate_info].crate
    else:
        fail(
            "rust_env_shard " + producer + ": upstream " +
            display_label(upstream.label) +
            " carries neither CrateInfo nor TestCrateInfo (not a rust_* wrapper)",
        )
    candidates_by_path = {}
    for f in crate.srcs.to_list() + [crate.root]:
        candidates_by_path[f.path] = f
    candidates = sorted(candidates_by_path.values(), key = lambda f: f.path)
    entries = [_parse_entry_spec(spec, producer) for spec in ctx.attr.entries]
    for entry in entries:
        if entry.exec_path == "":
            continue
        matches = [f for f in candidates if _exec_matches(f.path, entry.exec_path)]
        if len(matches) == 0:
            fail(
                "rust_env_shard " + producer + ": entry '" +
                entry.key + "' with EXEC_PATH '" + entry.exec_path +
                "' matches no crate source from " +
                display_label(upstream.label),
            )
        if len(matches) > 1:
            fail(
                "rust_env_shard " + producer + ": entry '" +
                entry.key + "' with EXEC_PATH '" + entry.exec_path +
                "' is ambiguous: matches " + str(len(matches)) +
                " crate sources, want exactly one",
            )

    bound = []
    for entry in entries:
        if entry.exec_path == "":
            continue
        for f in candidates:
            if _exec_matches(f.path, entry.exec_path):
                bound.append(f)
                break
    out, record = _emit_shard(ctx, producer, ctx.attr.integration, entries)
    return [
        DefaultInfo(files = depset([out])),
        DxEnvPlanInfo(direct = [record], transitive = depset([record])),
        OutputGroupInfo(dx_env_plans = depset([out] + bound)),
    ]

rust_env_shard = rule(
    implementation = _rust_env_shard_impl,
    attrs = {
        "entries": attr.string_list(
            mandatory = True,
        ),
        "integration": attr.string(
            default = "rust",
        ),
        "target": attr.label(
            mandatory = True,
        ),
        "_writer": attr.label(
            default = "//env/env_shard:env_shard_writer",
            executable = True,
            cfg = "exec",
        ),
    },
)

def _env_plan_subject_impl(ctx):
    target = ctx.attr.target
    records = plan_shard_subject_records(target, DxEnvPlanInfo, DxEnvPlanCollectedInfo)
    shard_files = plan_shard_subject_files(target, DX_ENV_PLAN_OUTPUT_GROUP)
    plan = {
        "files": ",".join([f.basename for f in shard_files]),
        "fingerprint": env_plan_fingerprint(records),
        "label": display_label(ctx.attr.target.label),
        "record_count": str(len(env_plan_merge_records(records))),
    }
    return [
        DefaultInfo(files = depset([])),
        OutputGroupInfo(dx_env_plans = depset(shard_files)),
        DxSubjectInfo(fields = plan),
    ]

env_plan_subject = rule(
    implementation = _env_plan_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = [dx_env_plan_aspect],
            mandatory = True,
        ),
    },
)
