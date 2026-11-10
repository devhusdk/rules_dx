"""The qt_sdk module extension."""

load("//core:repo.bzl", "hub_repo")
load(":pins.bzl", "DEFAULT_MODULES", "QT_VERSION")
load(":repos.bzl", "HUB_NAME", "cell_repository_name", "cell_spec", "consumer_cells", "effective_cells", "hub_build", "no_match_error", "pin_errors", "qt_cell_repo", "qualified_cells", "request_errors", "resolve_modules")

def _qt_sdk_impl(module_ctx):
    requests = []
    tags = []
    for module in module_ctx.modules:
        for tag in module.tags.cell:
            if not module.is_root:
                fail("qt_sdk: cell '" + tag.name + "' comes from module '" + module.name + "'; only the root module may declare cells")
            if not tag.spec.name.endswith(".json"):
                fail("qt_sdk: cell '" + tag.name + "' spec '" + str(tag.spec) + "' is not a .json file")
            tags.append(struct(
                constraints = [str(label) for label in tag.constraints],
                layout = tag.layout,
                name = tag.name,
                spec = module_ctx.read(module_ctx.path(tag.spec)),
            ))
        for tag in module.tags.configure:
            requests.append(struct(
                mirror = tag.mirror,
                modules = list(tag.modules),
                runtime_mirror = tag.runtime_mirror,
                version = tag.version,
            ))
    if len(requests) != 1:
        fail("qt_sdk: call qt_sdk.configure() exactly once")
    request = requests[0]
    failures = request_errors(request)
    if len(failures) > 0:
        fail("; ".join(failures))
    consumer, errors = consumer_cells(tags)
    table = effective_cells(consumer)
    errors = errors + pin_errors(table)
    if len(errors) > 0:
        fail("qt_sdk: " + "; ".join(errors))
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
            modules = resolve_modules(request.modules),
            runtime_mirror = request.runtime_mirror,
        )
    hub_repo(
        name = HUB_NAME,
        files = {"BUILD.bazel": hub_build(
            cells,
            str(Label("//qt/rules:defs.bzl")),
            {cell: table[cell].constraints for cell in cells},
            no_match_error(table),
        )},
    )

qt_sdk = module_extension(
    implementation = _qt_sdk_impl,
    doc = "Acquires the pinned Qt SDK and runtime closure on first use.",
    tag_classes = {
        "configure": tag_class(
            doc = "Requests Qt modules and optional mirrors.",
            attrs = {
                "mirror": attr.string(default = "", doc = "Https mirror of the Qt online repository."),
                "modules": attr.string_list(default = DEFAULT_MODULES, doc = "Qt modules to acquire."),
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
