"""Docs site rules that render DocIr shards and prose with mdBook."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load(":render.bzl", "DocsApiInfo", "mdbook_site", "mdbook_text")

def site_symbol_id(language, package, qualified):
    """Returns the stable symbol ID language:package:qualified."""
    return language + ":" + package + ":" + qualified

def site_symbol_id_error(language, package, qualified):
    """Validates one symbol identity, returning "" when valid."""
    if language == "":
        return "docs_site: language is required"
    if package == "":
        return "docs_site: package is required"
    if qualified == "":
        return "docs_site: qualified name is required"
    return ""

def site_api_path(symbol_id):
    """Returns the workspace-relative API page for one symbol ID."""
    return "api/" + symbol_id.replace(":", "/") + ".md"

def site_url_for_symbol(symbol_id):
    """Returns the rendered URL for one symbol ID."""
    return "api/" + symbol_id.replace(":", "/") + ".html"

def site_summary_name(name):
    """Returns the mdBook SUMMARY output name for one aggregate."""
    return name + "_SUMMARY.md"

def site_api_name(name):
    """Returns the generated API pages output name for one aggregate."""
    return name + "_api.md"

def site_records_name(name):
    """Returns the search-records output name for one aggregate."""
    return name + "_search_records.json"

def site_prose_error(path):
    """Validates one prose input is mdBook-compatible Markdown."""
    if path.endswith(".md"):
        return ""
    return "docs_site: prose inputs must be Markdown, got '" + path + "'"

def site_is_external_link(target):
    """Returns True when a Markdown link target is remote and never fetched."""
    if "://" in target:
        return True
    if target.startswith("mailto:"):
        return True
    return False

def site_link_target_error(target, known_pages, known_api_paths):
    """Validates one internal link target, returning "" when valid."""
    if target == "":
        return "docs_site: empty link target"
    if site_is_external_link(target):
        return ""
    if target.startswith("#"):
        if len(target) > 1:
            return ""
        return "docs_site: empty link target"
    parts = target.split("#")
    base = parts[0]
    if base == "":
        return "docs_site: empty link target"
    if base in known_pages:
        return ""
    if base in known_api_paths:
        return ""
    if base.startswith("api/"):
        return "docs_site: dangling API link '" + target + "'"
    if base.endswith(".md"):
        return "docs_site: dangling prose link '" + target + "'"
    return "docs_site: unknown link target '" + target + "'"

def site_search_record(url, title, body):
    """Returns one search-index record with sorted keys."""
    return "{\"body\": \"" + body + "\", \"title\": \"" + title + "\", \"url\": \"" + url + "\"}"

def site_is_known_guide(name):
    """Returns True for the three frozen release-blocking guides."""
    return name in ["quickstart", "tutorial", "migration"]

def site_guide_step_error(step):
    """Validates one guide-step line, returning "" when executable."""
    if step == "" or step.startswith("#"):
        return ""
    if "TODO" in step or "FIXME" in step or "UNEXECUTED" in step or "TBD" in step or "SKIP" in step:
        return "docs_site: unexecuted guide step '" + step + "'"
    return ""

def _docs_api_impl(ctx):
    name = ctx.label.name
    tree = ctx.actions.declare_directory(name)
    summary = ctx.actions.declare_file(site_summary_name(name))
    api = ctx.actions.declare_file(site_api_name(name))
    records = ctx.actions.declare_file(site_records_name(name))
    shards = [shard[DocsShardInfo].shard for shard in ctx.attr.shards]
    args = ctx.actions.args()
    args.add_all(shards, before_each = "--shard")
    if ctx.attr.source_url:
        args.add("--source-url", ctx.attr.source_url)
    args.add("--summary", summary)
    args.add("--api", api)
    args.add("--api-tree", tree.path)
    args.add("--records", records)
    ctx.actions.run(
        executable = ctx.executable._aggregate,
        arguments = [args],
        inputs = shards,
        outputs = [tree, summary, api, records],
        mnemonic = "DxDocsAggregate",
        progress_message = "Rendering the API reference of %{label}",
    )
    checked = ctx.actions.declare_file(name + ".checked")
    check = ctx.actions.args()
    check.add(ctx.executable._check)
    check.add(checked)
    check.add("--book", ctx.file.book_toml)
    check.add("--api", api)
    check.add("--records", records)
    check.add_all(shards, before_each = "--shard")
    check.add_all(ctx.files.prose, before_each = "--prose")
    ctx.actions.run_shell(
        command = "check=$1; out=$2; shift 2; \"$check\" \"$@\" && : > \"$out\"",
        arguments = [check],
        inputs = shards + ctx.files.prose + [ctx.file.book_toml, api, records],
        tools = [ctx.executable._check],
        outputs = [checked],
        mnemonic = "DxDocsCheck",
        progress_message = "Checking the links of %{label}",
    )
    return [
        DefaultInfo(files = depset([tree, summary, api, records])),
        DocsApiInfo(api = api, records = records, summary = summary, tree = tree),
        OutputGroupInfo(
            _validation = depset([checked]),
            api = depset([api]),
            records = depset([records]),
            summary = depset([summary]),
        ),
    ]

docs_api = rule(
    doc = "Renders DocIr shards into API pages and checks the site's links.",
    implementation = _docs_api_impl,
    attrs = {
        "book_toml": attr.label(doc = "The book's book.toml.", allow_single_file = True, mandatory = True),
        "prose": attr.label_list(doc = "The prose pages beside the API reference.", allow_files = [".md"], mandatory = True),
        "shards": attr.label_list(doc = "The DocIr shards to render.", mandatory = True, providers = [DocsShardInfo]),
        "source_url": attr.string(doc = "URL prefix that source paths link under."),
        "_aggregate": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/site/aggregate:site_aggregate"),
            executable = True,
        ),
        "_check": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/site/check:site_check"),
            executable = True,
        ),
    },
    provides = [DocsApiInfo],
)

def docs_site(name, shards, prose, book_toml, prose_route = "prose.md", source_url = ""):
    """Renders one prose page and the API reference of some shards."""
    docs_api(
        name = name + "_docs",
        book_toml = book_toml,
        prose = prose,
        shards = shards,
        source_url = source_url,
    )
    mdbook_text(
        name = name + "_SUMMARY",
        out = name + "_SUMMARY.md",
        text = "# Summary\n\n- [Prose](" + prose_route + ")\n",
    )
    mdbook_site(
        name = name,
        api = ":" + name + "_docs",
        book_toml = book_toml,
        pages = prose,
        routes = [prose_route],
        summary = ":" + name + "_SUMMARY.md",
    )
