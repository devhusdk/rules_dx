"""JSON Schema declarations for the check_jsonschema lint tool."""

load("//quality:native_config.bzl", "DxNativeConfigInfo")

JsonSchemaInfo = provider(
    doc = "One checked-in JSON Schema, the files it validates, and its closure.",
    fields = {
        "closure": "depset[File]: the schema plus every schema it references.",
        "files": "list[str]: workspace-relative globs of the files it validates.",
        "schema": "File: the schema document.",
        "url": "str: the $schema URL this checked-in copy stands in for, or empty.",
    },
)

def _json_schema_impl(ctx):
    if not ctx.file.src.basename.endswith(".json"):
        fail("json_schema " + str(ctx.label) + ": src '" + ctx.file.src.basename + "' must be a .json file")
    for pattern in ctx.attr.files:
        if pattern == "" or pattern.startswith("/") or ".." in pattern.split("/"):
            fail("json_schema " + str(ctx.label) + ": files pattern '" + pattern + "' must be workspace-relative")
    if ctx.attr.url != "" and not (ctx.attr.url.startswith("https://") or ctx.attr.url.startswith("http://")):
        fail("json_schema " + str(ctx.label) + ": url '" + ctx.attr.url + "' must be an http(s) URL")
    closure = depset(
        [ctx.file.src],
        transitive = [dep[JsonSchemaInfo].closure for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = closure),
        JsonSchemaInfo(
            closure = closure,
            files = ctx.attr.files,
            schema = ctx.file.src,
            url = ctx.attr.url,
        ),
    ]

json_schema = rule(
    implementation = _json_schema_impl,
    doc = "Declares one checked-in JSON Schema and the files it validates.",
    attrs = {
        "deps": attr.label_list(
            providers = [JsonSchemaInfo],
            doc = "Schemas this one references through relative $ref values.",
        ),
        "files": attr.string_list(doc = "Workspace-relative globs of the files this schema validates."),
        "src": attr.label(
            allow_single_file = [".json"],
            mandatory = True,
            doc = "The schema document.",
        ),
        "url": attr.string(doc = "A $schema URL this checked-in copy stands in for; never fetched."),
    },
)

def _staged_path(file):
    short = file.short_path
    if short.startswith("../"):
        short = "external/" + short[3:]
    return short

def _json_schemas_impl(ctx):
    root = ctx.label.name + ".json_schemas"
    staged = {}
    links = []
    urls = {}
    entries = []
    for target in ctx.attr.schemas:
        info = target[JsonSchemaInfo]
        if len(info.files) == 0 and info.url == "":
            fail("json_schemas " + str(ctx.label) + ": " + str(target.label) + " needs files, url, or both")
        if info.url != "":
            if info.url in urls:
                fail("json_schemas " + str(ctx.label) + ": duplicate url '" + info.url + "'")
            urls[info.url] = True
        for f in info.closure.to_list():
            path = _staged_path(f)
            if path in staged:
                if staged[path] != f:
                    fail("json_schemas " + str(ctx.label) + ": duplicate staged path '" + path + "'")
                continue
            staged[path] = f
            link = ctx.actions.declare_file(root + "/" + path)
            ctx.actions.symlink(output = link, target_file = f)
            links.append(link)
        entry = {
            "files": sorted(info.files),
            "schema": root + "/" + _staged_path(info.schema),
        }
        if info.url != "":
            entry["url"] = info.url
        entries.append(entry)
    config = ctx.actions.declare_file(ctx.label.name + ".schemas.json")
    ctx.actions.write(config, json.encode({
        "schema_version": 1,
        "schemas": entries,
    }) + "\n")
    closure = depset([config] + links)
    return [
        DefaultInfo(files = closure),
        DxNativeConfigInfo(
            closure = closure,
            config = config,
            tool_id = "check_jsonschema",
        ),
    ]

json_schemas = rule(
    implementation = _json_schemas_impl,
    doc = "Stages offline JSON Schemas and the check_jsonschema settings for dx lint.",
    attrs = {
        "schemas": attr.label_list(
            providers = [JsonSchemaInfo],
            mandatory = True,
            doc = "The json_schema targets to validate with.",
        ),
    },
)
