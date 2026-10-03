"""Bazel-cached docs site execution (extract to render)."""

MDBOOK_VERSION = "0.4.43"

DOC_IR_SCHEMA_MAJOR = 1
DOC_IR_SCHEMA_MINOR = 1

_AWK_CHARS = (
    "BEGIN { NL = sprintf(\"%c\", 10); Q = sprintf(\"%c\", 34); " +
    "B2 = sprintf(\"%c%c\", 92, 92); BQ = sprintf(\"%c%c\", 92, 34); " +
    "RE_BS = sprintf(\"%c%c\", 92, 92); RE_Q = sprintf(\"%c\", 34); " +
    "print \"schema_major: \" smaj; print \"schema_minor: \" smin; " +
    "print \"language: \" Q lang Q; print \"package: \" Q pkg Q }"
)

_AWK_SYMBOLS = (
    "{ n = $$1; d = substr($$0, index($$0, \"|\") + 1); " +
    "gsub(RE_BS, B2, n); gsub(RE_BS, B2, d); gsub(RE_Q, BQ, n); gsub(RE_Q, BQ, d); " +
    "printf \"symbols {\" NL \"  id: \" Q prefix n Q NL " +
    "\"  doc_markdown: \" Q d Q NL \"}\" NL }"
)

_AWK_MISSING_SEPARATOR = "index($$0, \"|\") < 2 { print }"

SITE_CHECK = "site_check.sh"

def site_check_args(api, shards, prose, data = []):
    """Returns the site_check.sh arguments for one aggregate's inputs."""
    parts = ["--api " + api]
    for shard in shards:
        parts.append("--shard $(location " + shard + ")")
    for doc in prose:
        parts.append("--prose $(location " + doc + ")")
    for item in data:
        parts.append("--data $(location " + item + ")")
    return " ".join(parts)

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

def site_header_value_error(attr, value):
    """Validates one header value is emittable into quoted textproto."""
    if "\n" in value:
        return "docs_site: " + attr + " must be one line"
    if "\"" in value:
        return "docs_site: " + attr + " must not contain a double quote"
    if "\\" in value:
        return "docs_site: " + attr + " must not contain a backslash"
    return ""

def site_api_path(symbol_id):
    """Returns the workspace-relative API page for one symbol ID."""
    return "api/" + symbol_id.replace(":", "/") + ".md"

def site_url_for_symbol(symbol_id):
    """Returns the rendered URL for one symbol ID."""
    return "api/" + symbol_id.replace(":", "/") + ".html"

def site_shard_name(name):
    """Returns the generated IR shard output name (Bazel output only)."""
    return name + ".ir.textproto"

def site_summary_name(name):
    """Returns the mdBook SUMMARY output name for one aggregate."""
    return name + "_SUMMARY.md"

def site_api_name(name):
    """Returns the generated API pages output name for one aggregate."""
    return name + "_api.md"

def site_records_name(name):
    """Returns the search-records output name for one aggregate."""
    return name + "_search_records.json"

def site_html_name(name):
    """Returns the rendered site entry output name for one render."""
    return name + "_index.html"

def site_index_name(name):
    """Returns the single search-index output name for one render."""
    return name + "_searchindex.json"

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

def docs_extract(name, language, package, srcs):
    """Runs one DocsExtract action emitting one cached IR shard."""
    unit_err = site_symbol_id_error(language, package, "unit")
    if unit_err != "":
        fail(unit_err + " (in " + native.package_name() + ":" + name + ")")
    for attr, value in [("language", language), ("package", package)]:
        header_err = site_header_value_error(attr, value)
        if header_err != "":
            fail(header_err + " (in " + native.package_name() + ":" + name + ")")
    if len(srcs) == 0:
        fail("docs_extract " + native.package_name() + ":" + name + ": need at least one src")
    shard = site_shard_name(name)
    prefix = language + ":" + package + ":"
    vars = (
        " -v smaj=" + str(DOC_IR_SCHEMA_MAJOR) +
        " -v smin=" + str(DOC_IR_SCHEMA_MINOR) +
        " -v lang=" + language +
        " -v pkg=" + package +
        " -v prefix=" + prefix
    )
    native.genrule(
        name = name + "_shard",
        srcs = srcs,
        outs = [shard],
        cmd = "set -e; " +
              "_lines=$$(for _f in $(SRCS); do awk 1 \"$$_f\"; done); " +
              "_bad=$$(echo \"$$_lines\" | LC_ALL=C awk '" + _AWK_MISSING_SEPARATOR + "'); " +
              "if [ -n \"$$_bad\" ]; then echo \"docs_site: symbol line needs name|doc_markdown: $$_bad\" >&2; exit 1; fi; " +
              "_dup=$$(echo \"$$_lines\" | LC_ALL=C cut -d'|' -f1 | LC_ALL=C sort | uniq -d); " +
              "if [ -n \"$$_dup\" ]; then echo \"docs_site: duplicate symbol name: $$_dup\" >&2; exit 1; fi; " +
              "echo \"$$_lines\" | LC_ALL=C sort -t'|' -k1,1 | " +
              "LC_ALL=C awk -F'|'" + vars + " '" + _AWK_CHARS + " " + _AWK_SYMBOLS + "' > \"$@\"; " +
              "if ! grep -q '^symbols {' \"$@\"; then echo \"docs_site: extract produced no symbols\" >&2; exit 1; fi",
    )
    native.filegroup(
        name = name,
        srcs = [":" + name + "_shard"],
    )

def docs_aggregate(name, shards, prose, book_toml):
    """Runs one DocsAggregate action emitting render inputs."""
    if len(shards) == 0:
        fail("docs_aggregate " + native.package_name() + ":" + name + ": need at least one shard")
    if len(prose) == 0:
        fail("docs_aggregate " + native.package_name() + ":" + name + ": need at least one prose file")
    summary = site_summary_name(name)
    api = site_api_name(name)
    records = site_records_name(name)
    api_loc = "$(location :" + api + ")"
    book_loc = "$(location " + book_toml + ")"
    shard_locs = " ".join(["$(location " + s + ")" for s in shards])
    native.genrule(
        name = name + "_aggregate",
        srcs = shards + prose + [book_toml, SITE_CHECK],
        outs = [summary, api, records],
        cmd = "set -e; " +
              "summary=$(location :" + summary + "); api=" + api_loc + "; records=$(location :" + records + "); " +
              "printf '# Summary\\n\\n- [Prose](prose.md)\\n- [API](api.md)\\n' > \"$$summary\"; " +
              "printf '# API Reference\\n\\n' > \"$$api\"; " +
              "LC_ALL=C grep -h '^  id: ' " + shard_locs + " | LC_ALL=C sort -u | sed 's/^  id: \"//;s/\"$$//;s/^/## /' >> \"$$api\"; " +
              "printf '[\\n' > \"$$records\"; " +
              "LC_ALL=C grep -h '^  id: ' " + shard_locs + " | LC_ALL=C sort -u | sed 's/^  id: \"//;s/\"$$//' | awk '{url=$$0; gsub(/:/, \"/\", url); printf \"  {\\\"body\\\": \\\"API docs for %s\\\", \\\"title\\\": \\\"%s\\\", \\\"url\\\": \\\"api/%s\\\"},\\n\", $$0, $$0, url}' | LC_ALL=C sort -u | sed '$$s/,$$//' >> \"$$records\"; " +
              "printf ']\\n' >> \"$$records\"; " +
              "bash $(location " + SITE_CHECK + ") --book " + book_loc + " " + site_check_args(api_loc, shards, prose),
    )
    native.filegroup(
        name = name,
        srcs = [":" + name + "_aggregate"],
    )

def docs_render(name, summary, api, records, book_toml):
    """Runs one DocsRender action emitting the complete static site."""
    html = site_html_name(name)
    index = site_index_name(name)
    native.genrule(
        name = name + "_render",
        srcs = [summary, api, records, book_toml],
        outs = [html, index],
        cmd = "set -e; " +
              "html=$(location :" + html + "); index=$(location :" + index + "); " +
              "title=$$(grep '^title' $(location " + book_toml + ") | cut -d '\"' -f 2); " +
              "{ printf '<!doctype html>\\n<html lang=\"en\">\\n<head><meta charset=\"utf-8\"><title>%s</title></head>\\n<body>\\n<!-- rendered by mdBook " + MDBOOK_VERSION + " fixture -->\\n' \"$$title\"; " +
              "printf '<h1>%s</h1>\\n' \"$$title\"; cat $(location " + summary + "); printf '\\n'; cat $(location " + api + "); printf '\\n</body>\\n</html>\\n'; } > \"$$html\"; " +
              "{ printf '{\\n  \"book\": \"%s\",\\n  \"docs\": ' \"$$title\"; cat $(location " + records + "); printf '\\n}\\n'; } > \"$$index\"",
    )
    native.filegroup(
        name = name,
        srcs = [":" + name + "_render"],
    )

def docs_site(name, language, package, srcs, prose, book_toml):
    """Chains extract, aggregate, and render for one (language, package) demo."""
    docs_extract(
        name = name + "_extract",
        language = language,
        package = package,
        srcs = srcs,
    )
    docs_aggregate(
        name = name + "_aggregate",
        shards = [":" + name + "_extract"],
        prose = prose,
        book_toml = book_toml,
    )
    docs_render(
        name = name,
        summary = ":" + name + "_aggregate_SUMMARY.md",
        api = ":" + name + "_aggregate_api.md",
        records = ":" + name + "_aggregate_search_records.json",
        book_toml = book_toml,
    )

def docs_user_aggregate(name, shards, prose, book_toml, data = []):
    """Runs one DocsAggregate action for the user guides plus API reference."""
    if len(shards) == 0:
        fail("docs_user_aggregate " + native.package_name() + ":" + name + ": need at least one shard")
    if len(prose) == 0:
        fail("docs_user_aggregate " + native.package_name() + ":" + name + ": need at least one prose file")
    summary = site_summary_name(name)
    api = site_api_name(name)
    records = site_records_name(name)
    shard_locs = " ".join(["$(location " + s + ")" for s in shards])
    prose_locs = " ".join(["$(location " + p + ")" for p in prose])
    api_loc = "$(location :" + api + ")"
    book_loc = "$(location " + book_toml + ")"
    native.genrule(
        name = name + "_aggregate",
        srcs = shards + prose + [book_toml] + data + [SITE_CHECK],
        outs = [summary, api, records],
        cmd = "set -e; " +
              "summary=$(location :" + summary + "); api=$(location :" + api + "); records=$(location :" + records + "); " +
              "{ printf '# Summary\\n\\n'; " +
              "for _f in $$(printf '%s\\n' " + prose_locs + " | LC_ALL=C sort -u); do _t=$$(LC_ALL=C grep -m1 '^# ' \"$$_f\" | sed 's/^# //'); printf '%s [%s](%s)\\n' '-' \"$$_t\" \"$$_f\"; done; " +
              "printf '%s\\n' '- [API](api.md)'; } > \"$$summary\"; " +
              "printf '# API Reference\\n\\n' > \"$$api\"; " +
              "LC_ALL=C grep -h '^  id: ' " + shard_locs + " | LC_ALL=C sort -u | sed 's/^  id: \"//;s/\"$$//;s/^/## /' >> \"$$api\"; " +
              "{ printf '[\\n'; _first=1; " +
              "for _f in $$(printf '%s\\n' " + prose_locs + " | LC_ALL=C sort -u); do _t=$$(LC_ALL=C grep -m1 '^# ' \"$$_f\" | sed 's/^# //'); if [ \"$$_first\" = 1 ]; then _first=0; else printf ',\\n'; fi; printf '  {\\\"body\\\": \\\"User guide %s\\\", \\\"title\\\": \\\"%s\\\", \\\"url\\\": \\\"%s.html\\\"}' \"$$_f\" \"$$_t\" \"$$_f\"; done; " +
              "for _sid in $$(LC_ALL=C grep -h '^  id: ' " + shard_locs + " | sed 's/^  id: \"//;s/\"$$//' | LC_ALL=C sort -u); do if [ \"$$_first\" = 1 ]; then _first=0; else printf ',\\n'; fi; _url=$$(printf '%s' \"$$_sid\" | sed 's/:/\\//g;s/^/api\\//'); printf '  {\\\"body\\\": \\\"API docs for %s\\\", \\\"title\\\": \\\"%s\\\", \\\"url\\\": \\\"%s\\\"}' \"$$_sid\" \"$$_sid\" \"$$_url\"; done; " +
              "printf '\\n]\\n'; } > \"$$records\"; " +
              "bash $(location " + SITE_CHECK + ") --book " + book_loc + " " + site_check_args(api_loc, shards, prose, data),
    )
    native.filegroup(
        name = name,
        srcs = [":" + name + "_aggregate"],
    )

def docs_user_render(name, summary, api, records, book_toml, prose):
    """Runs one DocsRender action emitting the user site with guide bodies."""
    html = site_html_name(name)
    index = site_index_name(name)
    prose_locs = " ".join(["$(location " + p + ")" for p in prose])
    native.genrule(
        name = name + "_render",
        srcs = [summary, api, records, book_toml] + prose,
        outs = [html, index],
        cmd = "set -e; " +
              "html=$(location :" + html + "); index=$(location :" + index + "); " +
              "title=$$(grep '^title' $(location " + book_toml + ") | cut -d '\"' -f 2); " +
              "{ printf '<!doctype html>\\n<html lang=\"en\">\\n<head><meta charset=\"utf-8\"><title>%s</title></head>\\n<body>\\n<!-- rendered by mdBook " + MDBOOK_VERSION + " fixture -->\\n' \"$$title\"; " +
              "printf '<h1>%s</h1>\\n' \"$$title\"; cat $(location " + summary + "); printf '\\n'; " +
              "for _f in $$(printf '%s\\n' " + prose_locs + " | LC_ALL=C sort -u); do printf '<hr>\\n<!-- %s -->\\n' \"$$_f\"; sed 's/&/\\&amp;/g; s/</\\&lt;/g; s/>/\\&gt;/g' \"$$_f\"; printf '\\n'; done; " +
              "sed 's/&/\\&amp;/g; s/</\\&lt;/g; s/>/\\&gt;/g' $(location " + api + "); printf '\\n</body>\\n</html>\\n'; } > \"$$html\"; " +
              "{ printf '{\\n  \"book\": \"%s\",\\n  \"docs\": ' \"$$title\"; cat $(location " + records + "); printf '\\n}\\n'; } > \"$$index\"",
    )
    native.filegroup(
        name = name,
        srcs = [":" + name + "_render"],
    )

def docs_user_site(name, language, package, srcs, prose, book_toml, data = []):
    """Chains extract, user aggregate, and user render for the user site."""
    docs_extract(
        name = name + "_extract",
        language = language,
        package = package,
        srcs = srcs,
    )
    docs_user_aggregate(
        name = name + "_aggregate",
        shards = [":" + name + "_extract"],
        prose = prose,
        book_toml = book_toml,
        data = data,
    )
    docs_user_render(
        name = name,
        summary = ":" + name + "_aggregate_SUMMARY.md",
        api = ":" + name + "_aggregate_api.md",
        records = ":" + name + "_aggregate_search_records.json",
        book_toml = book_toml,
        prose = prose,
    )
