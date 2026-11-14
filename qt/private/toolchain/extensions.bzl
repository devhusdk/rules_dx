"""The qt module extension."""

load(":pins.bzl", "MODULES", "QT_VERSION")
load(":repos.bzl", "HUB_NAME", "default_request", "cell_repository_name", "cell_spec", "consumer_cells", "effective_cells", "hub_build", "no_match_error", "pin_errors", "qt_cell_repo", "qualified_cells", "request_errors", "resolve_modules")

def _qt_hub_impl(rctx):
    rctx.file("REPO.bazel", "")
    for path, content in rctx.attr.files.items():
        rctx.file(path, content)
    return rctx.repo_metadata(reproducible = True)

_qt_hub = repository_rule(
    implementation = _qt_hub_impl,
    doc = "Writes the rendered Qt hub files into a repository that downloads nothing.",
    attrs = {
        "files": attr.string_dict(doc = "Repository path to file content."),
    },
)

def _merge(requests):
    """Returns the one request the configure tags of every module add up to."""
    request = default_request()
    fields = {"mirror": "", "runtime_mirror": "", "version": request.version}
    for tag in requests:
        for field in fields:
            value = getattr(tag, field)
            if value == "" or value == getattr(request, field):
                continue
            if fields[field] not in ["", getattr(request, field), value]:
                fail("qt: configure tags set " + field + " to both '" + fields[field] + "' and '" + value + "'")
            fields[field] = value
    return struct(
        mirror = fields["mirror"],
        runtime_mirror = fields["runtime_mirror"],
        version = fields["version"],
    )

def _qt_impl(module_ctx):
    requests = []
    tags = []
    for module in module_ctx.modules:
        for tag in module.tags.cell:
            if not module.is_root:
                fail("qt: cell '" + tag.name + "' comes from module '" + module.name + "'; only the root module may declare cells")
            if not tag.spec.name.endswith(".json"):
                fail("qt: cell '" + tag.name + "' spec '" + str(tag.spec) + "' is not a .json file")
            tags.append(struct(
                constraints = [str(label) for label in tag.constraints],
                layout = tag.layout,
                name = tag.name,
                spec = module_ctx.read(module_ctx.path(tag.spec)),
            ))
        if len(module.tags.configure) > 0 and not module.is_root:
            fail("qt: configure comes from module '" + module.name + "'; only the root module may call configure")
        if len(module.tags.configure) > 1:
            fail("qt: call qt.configure() at most once")
        for tag in module.tags.configure:
            requests.append(struct(
                mirror = tag.mirror,
                runtime_mirror = tag.runtime_mirror,
                version = tag.version,
            ))
    request = _merge(requests)
    failures = request_errors(request)
    if len(failures) > 0:
        fail("; ".join(failures))
    consumer, errors = consumer_cells(tags)
    table = effective_cells(consumer)
    errors = errors + pin_errors(table)
    if len(errors) > 0:
        fail("qt: " + "; ".join(errors))
    layouts = {tag.name: tag.layout for tag in tags}
    cells = {}
    for cell in qualified_cells(table):
        cells[cell] = cell_repository_name(cell)
        entry = ""
        if cell in consumer:
            entry = json.encode({
                "constraints": consumer[cell].constraints,
                "layout": layouts[cell],
                "spec": cell_spec(consumer[cell]),
            })
        qt_cell_repo(
            name = cells[cell],
            cell = cell,
            entry = entry,
            mirror = request.mirror,
            modules = resolve_modules(MODULES.keys()),
            runtime_mirror = request.runtime_mirror,
        )
    _qt_hub(
        name = HUB_NAME,
        files = {"BUILD.bazel": hub_build(
            cells,
            str(Label("//qt/private:defs.bzl")),
            {cell: table[cell].constraints for cell in cells},
            no_match_error(table),
        )},
    )
    return module_ctx.extension_metadata(reproducible = True)

qt = module_extension(
    implementation = _qt_impl,
    doc = "Acquires the pinned Qt SDK and runtime closure on first use.",
    tag_classes = {
        "configure": tag_class(
            doc = "Requests optional mirrors.",
            attrs = {
                "mirror": attr.string(default = "", doc = "Https mirror of the Qt online repository."),
                "runtime_mirror": attr.string(default = "", doc = "Https mirror of vault.almalinux.org."),
                "version": attr.string(default = QT_VERSION, doc = "Qt version; only the pinned one is accepted."),
            },
        ),
        "cell": tag_class(
            doc = "Declares a Qt cell from a JSON spec; a pinned cell of the same name is replaced.",
            attrs = {
                "constraints": attr.label_list(doc = "Platform constraints the cell serves."),
                "layout": attr.string(default = "linux", doc = "File layout of the cell's Qt archives."),
                "name": attr.string(mandatory = True, doc = "Cell name; the cell is acquired into @dx_qt_<name>."),
                "spec": attr.label(mandatory = True, doc = "JSON file with the cell's archives, directory, release and runtime closure."),
            },
        ),
    },
)
