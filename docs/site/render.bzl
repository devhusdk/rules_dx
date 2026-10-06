"""Real upstream mdBook rendering of the documentation site."""

_MDBOOK = "//docs/site/mdbook:mdbook"
_SITE_BUILD = "//docs/site/build:site_build"

def mdbook_index(title, intro):
    """Returns the generated landing chapter of one book."""
    return struct(kind = "index", route = "README.md", title = title, intro = intro)

def mdbook_section(title):
    """Returns one sidebar section title of a book."""
    return struct(kind = "section", title = title)

def mdbook_page(title, route, source):
    """Returns one chapter with the Markdown file it renders."""
    return struct(kind = "page", route = route, source = source, title = title)

def mdbook_chapters_error(chapters):
    """Returns one error string per chapter table mdBook cannot render."""
    errors = []
    if len(chapters) == 0:
        return ["docs_site: the book has no chapters"]
    routes = {}
    sections = 0
    for index in range(len(chapters)):
        chapter = chapters[index]
        if chapter.kind == "section":
            sections += 1
            if chapter.title.strip() == "":
                errors.append("docs_site: a sidebar section has no title")
            if index == 0 or chapters[index - 1].kind == "section":
                errors.append("docs_site: sidebar section '" + chapter.title + "' holds no page")
            continue
        if index == 0 and chapter.kind != "index":
            errors.append("docs_site: the book must open with the generated index chapter")
        elif index != 0 and chapter.kind == "index":
            errors.append("docs_site: the book has more than one index chapter")
        error = mdbook_route_error(chapter.route)
        if error != "":
            errors.append(error)
        elif chapter.route in routes:
            errors.append("docs_site: route '" + chapter.route + "' is declared twice, by " +
                          routes[chapter.route] + " and " + chapter.title)
        else:
            routes[chapter.route] = chapter.title
        if chapter.kind == "page" and chapter.source.strip() == "":
            errors.append("docs_site: route '" + chapter.route + "' names no source page")
        if chapter.title.strip() == "":
            errors.append("docs_site: route '" + chapter.route + "' has no title")
    if sections == 0:
        errors.append("docs_site: the book has no sidebar sections")
    return errors

def mdbook_route_error(route):
    """Validates one source route, returning "" when mdBook can render it."""
    if route == "":
        return "docs_site: a source route is empty"
    if not route.endswith(".md"):
        return "docs_site: route '" + route + "' is not a Markdown page"
    if route.startswith("/") or "\\" in route:
        return "docs_site: route '" + route + "' is not a relative clean path"
    for part in route.split("/"):
        if part == "" or part == "." or part == "..":
            return "docs_site: route '" + route + "' has an empty or relative segment"
    return ""

def mdbook_page_list(chapters):
    """Returns the chapter routes and their source labels, in book order."""
    routes = []
    pages = []
    for chapter in chapters:
        if chapter.kind == "section":
            continue
        routes.append(chapter.route)
        pages.append(None if chapter.kind == "index" else chapter.source)
    return routes, pages

def mdbook_summary_text(chapters):
    """Returns the mdBook SUMMARY text one curated chapter list renders."""
    lines = ["# Summary", ""]
    for chapter in chapters:
        if chapter.kind == "section":
            lines.append("# " + chapter.title)
        else:
            lines.append("- [" + chapter.title + "](" + chapter.route + ")")
    return "\n".join(lines) + "\n"

def mdbook_index_text(chapters):
    """Returns the generated landing page that opens one book."""
    landing = chapters[0]
    lines = ["# " + landing.title, ""]
    if landing.intro.strip() != "":
        lines.append(landing.intro)
        lines.append("")
    pending = ""
    for chapter in chapters:
        if chapter.kind == "section":
            pending = chapter.title
        elif pending != "":
            lines.append("- [" + pending + "](" + chapter.route + ")")
            pending = ""
    return "\n".join(lines) + "\n"

def _mdbook_text_impl(ctx):
    ctx.actions.write(ctx.outputs.out, ctx.attr.text)

mdbook_text = rule(
    implementation = _mdbook_text_impl,
    attrs = {
        "out": attr.output(mandatory = True),
        "text": attr.string(mandatory = True),
    },
)

def _mdbook_site_impl(ctx):
    manifest = ctx.actions.declare_file(ctx.label.name + "_pages.tsv")
    ctx.actions.write(
        output = manifest,
        content = "".join([
            route + "\t" + page.path + "\n"
            for route, page in zip(ctx.attr.routes, ctx.files.pages)
        ]),
    )
    tree = ctx.actions.declare_directory(ctx.label.name)
    ctx.actions.run(
        executable = ctx.executable.site_build,
        arguments = [
            "--mdbook",
            ctx.file.mdbook.path,
            "--book",
            ctx.file.book_toml.path,
            "--summary",
            ctx.file.summary.path,
            "--manifest",
            manifest.path,
            "--out",
            tree.path,
        ],
        inputs = [ctx.file.book_toml, ctx.file.mdbook, ctx.file.summary, manifest] + ctx.files.pages,
        outputs = [tree],
        mnemonic = "DxMdBookSite",
        progress_message = "Rendering %{label} with pinned mdBook",
    )
    return [DefaultInfo(files = depset([tree]))]

mdbook_site = rule(
    implementation = _mdbook_site_impl,
    attrs = {
        "book_toml": attr.label(allow_single_file = True, mandatory = True),
        "mdbook": attr.label(
            allow_single_file = True,
            cfg = "exec",
            default = Label(_MDBOOK),
            executable = True,
        ),
        "pages": attr.label_list(allow_files = True, mandatory = True),
        "routes": attr.string_list(mandatory = True),
        "site_build": attr.label(
            cfg = "exec",
            default = Label(_SITE_BUILD),
            executable = True,
        ),
        "summary": attr.label(allow_single_file = True, mandatory = True),
    },
)

def mdbook_book(name, book_toml, chapters):
    """Emits the generated book inputs and renders one complete site tree."""
    mdbook_inputs(name, chapters)
    mdbook_site(
        name = name,
        book_toml = book_toml,
        pages = _book_pages(name, chapters),
        routes = mdbook_page_list(chapters)[0],
        summary = ":" + name + "_SUMMARY.md",
    )

def mdbook_check(name, chapters):
    """Validates one curated chapter list without rendering the site."""
    mdbook_inputs(name, chapters)
    native.filegroup(
        name = name,
        srcs = [":" + name + "_SUMMARY.md", ":" + name + "_index.md"],
    )

def mdbook_inputs(name, chapters):
    """Emits the SUMMARY and the landing page one curated chapter list renders."""
    errors = mdbook_chapters_error(chapters)
    if len(errors) > 0:
        fail("; ".join(errors) + " (in " + native.package_name() + ":" + name + ")")
    mdbook_text(
        name = name + "_SUMMARY",
        out = name + "_SUMMARY.md",
        text = mdbook_summary_text(chapters),
    )
    mdbook_text(
        name = name + "_index",
        out = name + "_index.md",
        text = mdbook_index_text(chapters),
    )

def _book_pages(name, chapters):
    """Returns the source labels of one book's chapters, in book order."""
    pages = [":" + name + "_index.md"]
    for chapter in chapters:
        if chapter.kind == "page":
            pages.append(chapter.source)
    return pages
