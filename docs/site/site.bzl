"""Bazel-cached docs site execution (extract to render)."""

MDBOOK_VERSION = "0.4.43"

DOC_IR_SCHEMA_MAJOR = 1
DOC_IR_SCHEMA_MINOR = 1

_AWK_TEXTPROTO_CHARS = "BEGIN { BS = \"\\134\"; Q = \"\\042\" }"

_AWK_ESCAPE_FIELDS = "{ n = $$1; d = $$2; gsub(/\\\\/, BS BS, n); gsub(/\\\\/, BS BS, d); gsub(/\"/, BS Q, n); gsub(/\"/, BS Q, d); "

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
    if len(srcs) == 0:
        fail("docs_extract " + native.package_name() + ":" + name + ": need at least one src")
    shard = site_shard_name(name)
    header = "schema_major: " + str(DOC_IR_SCHEMA_MAJOR) + "\\nschema_minor: " + str(DOC_IR_SCHEMA_MINOR) + "\\nlanguage: \"" + language + "\"\\npackage: \"" + package + "\"\\n"
    native.genrule(
        name = name + "_shard",
        srcs = srcs,
        outs = [shard],
        cmd = "(printf '" + header + "'; LC_ALL=C sort $(SRCS) | awk -F'|' '" + _AWK_TEXTPROTO_CHARS + " " + _AWK_ESCAPE_FIELDS + "printf \"symbols {\\n  id: \\\"" + language + ":" + package + ":%s\\\"\\n  doc_markdown: \\\"%s\\\"\\n}\\n\", n, d }')" + " > $@",
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
    shard_locs = " ".join(["$(location " + s + ")" for s in shards])
    prose_locs = " ".join(["$(location " + p + ")" for p in prose])
    book_loc = "$(location " + book_toml + ")"
    native.genrule(
        name = name + "_aggregate",
        srcs = shards + prose + [book_toml],
        outs = [summary, api, records],
        cmd = "set -e; " +
              "summary=$(location :" + summary + "); api=$(location :" + api + "); records=$(location :" + records + "); " +
              "printf '# Summary\\n\\n- [Prose](prose.md)\\n- [API](api.md)\\n' > \"$$summary\"; " +
              "printf '# API Reference\\n\\n' > \"$$api\"; " +
              "LC_ALL=C grep -h '^  id: ' " + shard_locs + " | LC_ALL=C sort -u | sed 's/^  id: \"//;s/\"$$//;s/^/## /' >> \"$$api\"; " +
              "printf '[\\n' > \"$$records\"; " +
              "LC_ALL=C grep -h '^  id: ' " + shard_locs + " | LC_ALL=C sort -u | sed 's/^  id: \"//;s/\"$$//' | awk '{url=$$0; gsub(/:/, \"/\", url); printf \"  {\\\"body\\\": \\\"API docs for %s\\\", \\\"title\\\": \\\"%s\\\", \\\"url\\\": \\\"api/%s\\\"},\\n\", $$0, $$0, url}' | LC_ALL=C sort -u | sed '$$s/,$$//' >> \"$$records\"; " +
              "printf ']\\n' >> \"$$records\"; " +
              "grep -q '^title' " + book_loc + "; " +
              "grep -q '^# ' " + prose_locs + "; " +
              "for _sid in $$(LC_ALL=C grep -h '^  id: ' " + shard_locs + " | sed 's/^  id: \"//;s/\"$$//' | LC_ALL=C sort -u); do LC_ALL=C grep -qF \"$$_sid\" \"$$api\" || { echo \"docs_site: missing API page for $$_sid\" >&2; exit 1; }; done; " +
              "if LC_ALL=C grep -q '\\[[^]]*\\]()' " + prose_locs + "; then echo 'docs_site: empty link target' >&2; exit 1; fi; " +
              "anchors=$$( (LC_ALL=C grep -h '^#' " + prose_locs + " 2>/dev/null || true; LC_ALL=C grep -h '^## ' \"$$api\" 2>/dev/null || true) | sed 's/^#* *//' | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9 -]//g; s/^ *//; s/ *$$//; s/ /-/g; s/--*/-/g' | LC_ALL=C sort -u); " +
              "api_paths=$$(LC_ALL=C grep -h '^  id: ' " + shard_locs + " | sed 's/^  id: \"//;s/\"$$//' | sed 's/:/\\//g;s/^/api\\//;s/$$/.md/' | LC_ALL=C sort -u); " +
              "prose_bases=$$(for _f in " + prose_locs + "; do basename \"$$_f\"; done | LC_ALL=C sort -u); " +
              "targets=$$( (LC_ALL=C grep -h -o '\\[[^]]*\\]([^)]*)' " + prose_locs + " 2>/dev/null | sed -n 's/.*(\\([^)]*\\)).*/\\1/p' | sed 's/^ *//;s/ *$$//;s/^<//;s/>$$//;s/^\".*//;s/\".*$$//;s/^ *//;s/ *$$//' | cut -d' ' -f1 | LC_ALL=C sort -u || true; LC_ALL=C grep -h '^[ ]*\\[[^]]*\\]:' " + prose_locs + " 2>/dev/null | sed 's/^[^:]*:[[:space:]]*//;s/[[:space:]\".*].*//' | cut -d' ' -f1 | LC_ALL=C sort -u || true) | LC_ALL=C sort -u); " +
              "for _t in $$targets; do case \"$$_t\" in *\\://*|mailto:*) continue;; \\#*) _frag=$$(printf '%s' \"$$_t\" | cut -c2-); printf '%s\\n' \"$$anchors\" | LC_ALL=C grep -qxF \"$$_frag\" || { echo \"docs_site: dangling anchor '$$_t'\" >&2; exit 1; };; *) _base=$$(printf '%s' \"$$_t\" | cut -d'#' -f1); _frag=$$(printf '%s' \"$$_t\" | cut -s -d'#' -f2- || true); _found=0; for _p in api.md prose.md SUMMARY.md $$prose_bases $$api_paths; do if [ \"$$_base\" = \"$$_p\" ]; then _found=1; break; fi; done; if [ \"$$_found\" = \"0\" ]; then echo \"docs_site: dangling link '$$_t'\" >&2; exit 1; fi; if [ -n \"$$_frag\" ]; then _norm=$$(printf '%s' \"$$_frag\" | tr '[:upper:]' '[:lower:]'); printf '%s\\n' \"$$anchors\" | LC_ALL=C grep -qxF \"$$_norm\" || { echo \"docs_site: dangling fragment '$$_t'\" >&2; exit 1; }; fi;; esac; done",
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
    data_locs = " ".join(["$(location " + d + ")" for d in data])
    book_loc = "$(location " + book_toml + ")"
    native.genrule(
        name = name + "_aggregate",
        srcs = shards + prose + [book_toml] + data,
        outs = [summary, api, records],
        cmd = "set -e; " +
              "summary=$(location :" + summary + "); api=$(location :" + api + "); records=$(location :" + records + "); " +
              "grep -q '^title' " + book_loc + "; " +
              "for _f in " + prose_locs + "; do LC_ALL=C grep -q '^# ' \"$$_f\" || { echo \"docs_site: prose missing title '$$_f'\" >&2; exit 1; }; done; " +
              "{ printf '# Summary\\n\\n'; " +
              "for _f in $$(printf '%s\\n' " + prose_locs + " | LC_ALL=C sort -u); do _t=$$(LC_ALL=C grep -m1 '^# ' \"$$_f\" | sed 's/^# //'); printf '%s [%s](%s)\\n' '-' \"$$_t\" \"$$_f\"; done; " +
              "printf '%s\\n' '- [API](api.md)'; } > \"$$summary\"; " +
              "printf '# API Reference\\n\\n' > \"$$api\"; " +
              "LC_ALL=C grep -h '^  id: ' " + shard_locs + " | LC_ALL=C sort -u | sed 's/^  id: \"//;s/\"$$//;s/^/## /' >> \"$$api\"; " +
              "{ printf '[\\n'; _first=1; " +
              "for _f in $$(printf '%s\\n' " + prose_locs + " | LC_ALL=C sort -u); do _t=$$(LC_ALL=C grep -m1 '^# ' \"$$_f\" | sed 's/^# //'); if [ \"$$_first\" = 1 ]; then _first=0; else printf ',\\n'; fi; printf '  {\\\"body\\\": \\\"User guide %s\\\", \\\"title\\\": \\\"%s\\\", \\\"url\\\": \\\"%s.html\\\"}' \"$$_f\" \"$$_t\" \"$$_f\"; done; " +
              "for _sid in $$(LC_ALL=C grep -h '^  id: ' " + shard_locs + " | sed 's/^  id: \"//;s/\"$$//' | LC_ALL=C sort -u); do if [ \"$$_first\" = 1 ]; then _first=0; else printf ',\\n'; fi; _url=$$(printf '%s' \"$$_sid\" | sed 's/:/\\//g;s/^/api\\//'); printf '  {\\\"body\\\": \\\"API docs for %s\\\", \\\"title\\\": \\\"%s\\\", \\\"url\\\": \\\"%s\\\"}' \"$$_sid\" \"$$_sid\" \"$$_url\"; done; " +
              "printf '\\n]\\n'; } > \"$$records\"; " +
              "for _sid in $$(LC_ALL=C grep -h '^  id: ' " + shard_locs + " | sed 's/^  id: \"//;s/\"$$//' | LC_ALL=C sort -u); do LC_ALL=C grep -qF \"$$_sid\" \"$$api\" || { echo \"docs_site: missing API page for $$_sid\" >&2; exit 1; }; done; " +
              "if LC_ALL=C grep -q '\\[[^]]*\\]()' " + prose_locs + "; then echo 'docs_site: empty link target' >&2; exit 1; fi; " +
              "anchors=$$((LC_ALL=C grep -h '^#' " + prose_locs + " 2>/dev/null || true; LC_ALL=C grep -h '^## ' \"$$api\" 2>/dev/null || true) | sed 's/^#* *//' | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9 -]//g; s/^ *//; s/ *$$//; s/ /-/g; s/--*/-/g' | LC_ALL=C sort -u); " +
              "api_paths=$$(LC_ALL=C grep -h '^  id: ' " + shard_locs + " | sed 's/^  id: \"//;s/\"$$//' | sed 's/:/\\//g;s/^/api\\//;s/$$/.md/' | LC_ALL=C sort -u); " +
              "prose_bases=$$(for _f in " + prose_locs + "; do basename \"$$_f\"; done | LC_ALL=C sort -u); " +
              "data_bases=$$((for _f in " + data_locs + "; do basename \"$$_f\"; done | LC_ALL=C sort -u) 2>/dev/null || true); " +
              "targets=$$((LC_ALL=C grep -h -o '\\[[^]]*\\]([^)]*)' " + prose_locs + " 2>/dev/null | sed -n 's/.*(\\([^)]*\\)).*/\\1/p' | sed 's/^ *//;s/ *$$//;s/^<//;s/>$$//;s/^\".*//;s/\".*$$//;s/^ *//;s/ *$$//' | cut -d' ' -f1 | LC_ALL=C sort -u || true; LC_ALL=C grep -h '^[ ]*\\[[^]]*\\]:' " + prose_locs + " 2>/dev/null | sed 's/^[^:]*:[[:space:]]*//;s/[[:space:]\".*].*//' | cut -d' ' -f1 | LC_ALL=C sort -u || true) | LC_ALL=C sort -u); " +
              "for _t in $$targets; do case \"$$_t\" in *\\://*|mailto:*) continue;; \\#*) _frag=$$(printf '%s' \"$$_t\" | cut -c2-); printf '%s\\n' \"$$anchors\" | LC_ALL=C grep -qxF \"$$_frag\" || { echo \"docs_site: dangling anchor '$$_t'\" >&2; exit 1; };; *) _base=$$(printf '%s' \"$$_t\" | cut -d'#' -f1); _frag=$$(printf '%s' \"$$_t\" | cut -s -d'#' -f2- || true); _dir=0; case \"$$_base\" in */) _dir=1; _base=$$(printf '%s' \"$$_base\" | sed 's|/$$||');; esac; _n=\"$$_base\"; while case \"$$_n\" in ../*) true;; *) false;; esac; do _n=$$(printf '%s' \"$$_n\" | sed 's|^\\.\\./||'); done; _n=$$(printf '%s' \"$$_n\" | sed 's|^\\./||'); _found=0; " +
              "if [ \"$$_dir\" = 1 ]; then for _p in " + prose_locs + "; do case \"$$_p\" in \"$$_n/README.md\"|*/\"$$_n/README.md\") _found=1; break;; esac; done; " +
              "else for _p in " + prose_locs + " " + data_locs + " $$api_paths; do if [ \"$$_p\" = \"$$_n\" ]; then _found=1; break; fi; case \"$$_p\" in */\"$$_n\") _found=1; break;; esac; done; " +
              "if [ \"$$_found\" = 0 ]; then case \"$$_n\" in *\"/\"*) ;; *) for _b in $$prose_bases $$data_bases; do if [ \"$$_b\" = \"$$_n\" ]; then _found=1; break; fi; done;; esac; fi; " +
              "for _k in api.md SUMMARY.md; do if [ \"$$_n\" = \"$$_k\" ]; then _found=1; break; fi; done; fi; " +
              "if [ \"$$_found\" = 0 ]; then echo \"docs_site: dangling link '$$_t'\" >&2; exit 1; fi; " +
              "if [ -n \"$$_frag\" ]; then _norm=$$(printf '%s' \"$$_frag\" | tr '[:upper:]' '[:lower:]'); printf '%s\\n' \"$$anchors\" | LC_ALL=C grep -qxF \"$$_norm\" || { echo \"docs_site: dangling fragment '$$_t'\" >&2; exit 1; }; fi;; esac; done",
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
