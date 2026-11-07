"""Writes the support matrix facts JSON from the language list and the quality registry."""

load("//cc/toolchains:bindings.bzl", "CC_COMPILERS", "cc_compiler_identity")
load("//cc/toolchains:pins.bzl", "CC_COMPILER_VERSIONS")
load("//quality:adapters.bzl", "NOT_APPLICABLE", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")
load("//quality:parity_tests.bzl", "PARITY_DEFERRED")
load("//quality:support.bzl", "SUPPORT_RECORDS")
load(":languages.bzl", "API_COMPAT_TOOLS", "COMPILER_BLOCKERS", "COMPILER_PLATFORM_PROBES", "COMPILER_PROBES", "COMPUTED_CONSTRAINTS", "COVERAGE_COLLECTORS", "COVERAGE_NOTES", "DEAD_CODE", "DEPENDENCIES", "DOC_TOOLS", "NOT_LANGUAGES", "NO_COVERAGE", "PENDING_COMPILERS", "PLATFORMS", "PLATFORM_NOT_APPLICABLE", "ROWS", "SECTIONS", "UNSUPPORTED_CONSTRAINTS")

_CAPABILITIES = ["lint", "format", "typecheck"]

_ROW_KEYS = ["ci_tests", "compiler", "consumer_tests", "families", "name", "section", "title", "workflows"]

_BUILD_KEYS = ["ci_tests", "consumer_tests", "workflows"]

def _doc(name):
    if name not in DOC_TOOLS:
        return None
    entry = DOC_TOOLS[name]
    return entry["tool"] if entry else ""

def _dead_code(row):
    if row["name"] in DEAD_CODE:
        return DEAD_CODE[row["name"]]
    if row["section"] == "languages":
        return []
    return None

def _coverage(name):
    if name in COVERAGE_COLLECTORS:
        return {"collectors": COVERAGE_COLLECTORS[name], "reason": COVERAGE_NOTES.get(name, "")}
    if name in NO_COVERAGE:
        return {"collectors": [], "reason": NO_COVERAGE[name]}
    return None

def _quality():
    families = {}
    for family in REAL_CLASS_TO_FAMILY.values():
        families[family] = {capability: {} for capability in _CAPABILITIES}
    for tool, capabilities in REAL_ADAPTERS.items():
        for capability, classes in capabilities.items():
            if capability not in _CAPABILITIES:
                continue
            for class_id in classes:
                families[REAL_CLASS_TO_FAMILY[class_id]][capability][tool] = True
    return {
        family: {capability: sorted(tools[capability].keys()) for capability in _CAPABILITIES}
        for family, tools in families.items()
    }

def _identity(row):
    if "compiler" not in row:
        return {}
    identity = cc_compiler_identity(CC_COMPILERS[row["compiler"]], "")
    return {
        "compiler": row["compiler"],
        "coverage": identity.coverage,
        "driver": identity.driver,
        "platform_probes": COMPILER_PLATFORM_PROBES.get(row["compiler"], {}),
        "probe": COMPILER_PROBES.get(row["compiler"], ""),
        "version": CC_COMPILER_VERSIONS.get(row["compiler"], ""),
    }

def tool_not_applicable(records):
    """Merges the unsupported platforms of the records into reasons per family, capability and tool."""
    merged = {}
    for record in records:
        for platform, reason in record.get("unsupported", {}).items():
            key = record["family"] + "/" + record["capability"] + "/" + record["tool"]
            by_tool = merged.setdefault(record["family"], {}).setdefault(record["capability"], {})
            reasons = by_tool.setdefault(record["tool"], {})
            if reasons.setdefault(platform, reason) != reason:
                fail("support matrix: " + key + " gives two reasons for " + platform)
    return merged

def compiler_blocker_problems(blockers, rows, platforms, not_applicable):
    """Returns why each blocker is invalid: it must name a compiler row, a known platform, a reason and a supported cell."""
    compiler_rows = {row["name"]: True for row in rows if row["section"] == "compilers"}
    platform_ids = {platform["id"]: True for platform in platforms}
    problems = []
    for name, by_platform in blockers.items():
        if name not in compiler_rows:
            problems.append(name + " is not a compiler row")
        for platform, reason in by_platform.items():
            if platform not in platform_ids or not reason:
                problems.append(name + " needs a known platform and a reason, got " + platform)
            elif platform in not_applicable.get(name, {}):
                problems.append(name + " on " + platform + " is both blocked and not applicable")
    return problems

def support_matrix_facts_dict():
    """Returns the validated facts as a dict."""
    quality = _quality()
    names = {}
    compiler_rows = {}
    for row in ROWS:
        for key in row:
            if key not in _ROW_KEYS:
                fail("support matrix: row " + row["name"] + " has unknown key " + key)
        if row["name"] in names:
            fail("support matrix: row " + row["name"] + " is listed twice")
        names[row["name"]] = True
        if row["name"] in NOT_LANGUAGES:
            fail("support matrix: " + row["name"] + " is both a row and not a language")
        if row["section"] not in SECTIONS:
            fail("support matrix: row " + row["name"] + " has unknown section " + row["section"])
        for family in row.get("families", [row["name"]]):
            if family not in quality:
                fail("support matrix: row " + row["name"] + " names unknown quality family " + family)
        if row["section"] == "formats":
            for key in _BUILD_KEYS:
                if row.get(key, []):
                    fail("support matrix: format row " + row["name"] + " sets " + key)
        if row["section"] == "compilers":
            for key in ["families", "workflows"]:
                if row.get(key, []):
                    fail("support matrix: compiler row " + row["name"] + " sets " + key)
            if row.get("compiler") not in CC_COMPILERS:
                fail("support matrix: compiler row " + row["name"] + " needs a compiler from CC_COMPILERS")
            compiler_rows.setdefault(row["compiler"], []).append(row["name"])
        elif "compiler" in row:
            fail("support matrix: row " + row["name"] + " sets compiler outside the compilers section")
    for compiler in CC_COMPILERS:
        rows = compiler_rows.get(compiler, [])
        pending = PENDING_COMPILERS.get(compiler, "")
        if rows and pending:
            fail("support matrix: compiler " + compiler + " has a row and is pending")
        if len(rows) > 1 or (not rows and not pending):
            fail("support matrix: compiler " + compiler + " needs exactly one row or a pending reason")
    for compiler in PENDING_COMPILERS:
        if compiler not in CC_COMPILERS:
            fail("support matrix: PENDING_COMPILERS names unknown compiler " + compiler)
    for compiler in COMPILER_PROBES:
        if compiler not in CC_COMPILERS:
            fail("support matrix: COMPILER_PROBES names unknown compiler " + compiler)
    row_names = {row["name"]: True for row in ROWS}
    for name in COVERAGE_COLLECTORS:
        if name not in row_names or name in NO_COVERAGE:
            fail("support matrix: COVERAGE_COLLECTORS names " + name + " which is unknown or also in NO_COVERAGE")
    for name in COVERAGE_NOTES:
        if name not in COVERAGE_COLLECTORS:
            fail("support matrix: COVERAGE_NOTES names " + name + " which has no coverage collector")
    for name in NO_COVERAGE:
        if name not in row_names:
            fail("support matrix: NO_COVERAGE names unknown row " + name)
    platform_ids = {platform["id"]: True for platform in PLATFORMS}
    tool_na = tool_not_applicable(SUPPORT_RECORDS)
    declared_tables = PLATFORM_NOT_APPLICABLE.items()
    for family, by_capability in tool_na.items():
        for capability, by_tool in by_capability.items():
            declared_tables += [(family + "/" + capability + "/" + tool, reasons) for tool, reasons in by_tool.items()]
    for name, declared in declared_tables:
        for platform, reason in declared.items():
            if platform not in platform_ids or not reason:
                fail("support matrix: " + name + " needs a known platform and a reason, got " + platform)
    for name in PLATFORM_NOT_APPLICABLE:
        if name not in row_names:
            fail("support matrix: PLATFORM_NOT_APPLICABLE names unknown row " + name)
    for compiler, by_platform in COMPILER_PLATFORM_PROBES.items():
        if compiler not in CC_COMPILERS:
            fail("support matrix: COMPILER_PLATFORM_PROBES names unknown compiler " + compiler)
        for platform in by_platform:
            if platform not in platform_ids:
                fail("support matrix: COMPILER_PLATFORM_PROBES names unknown platform " + platform + " for " + compiler)
            for name in compiler_rows.get(compiler, []):
                if platform in PLATFORM_NOT_APPLICABLE.get(name, {}):
                    fail("support matrix: COMPILER_PLATFORM_PROBES gives " + name + " a probe on " + platform + ", which does not apply")
    for problem in compiler_blocker_problems(COMPILER_BLOCKERS, ROWS, PLATFORMS, PLATFORM_NOT_APPLICABLE):
        fail("support matrix: COMPILER_BLOCKERS: " + problem)
    for name in UNSUPPORTED_CONSTRAINTS:
        if name not in PLATFORM_NOT_APPLICABLE:
            fail("support matrix: UNSUPPORTED_CONSTRAINTS names " + name + " which has no PLATFORM_NOT_APPLICABLE reason")
    support = []
    for record in SUPPORT_RECORDS:
        support.append({
            "capability": record["capability"],
            "family": record["family"],
            "platforms": record["platforms"],
            "tool": record["tool"],
        })
    deferred = {}
    for class_id in PARITY_DEFERRED:
        deferred[REAL_CLASS_TO_FAMILY.get(class_id, class_id)] = True
    return {
        "api_compat_tools": API_COMPAT_TOOLS,
        "compiler_blockers": COMPILER_BLOCKERS,
        "computed_constraints": COMPUTED_CONSTRAINTS,
        "dependencies": DEPENDENCIES,
        "deferred_families": sorted(deferred.keys()),
        "not_applicable": NOT_APPLICABLE,
        "not_languages": sorted(NOT_LANGUAGES),
        "pending_compilers": PENDING_COMPILERS,
        "platform_not_applicable": PLATFORM_NOT_APPLICABLE,
        "platforms": PLATFORMS,
        "quality": quality,
        "unsupported_constraints": UNSUPPORTED_CONSTRAINTS,
        "rows": [
            {
                "ci_tests": row.get("ci_tests", []),
                "consumer_tests": row.get("consumer_tests", []),
                "coverage": _coverage(row["name"]),
                "dead_code": _dead_code(row),
                "doc": _doc(row["name"]),
                "families": row.get("families", [row["name"]]),
                "identity": _identity(row),
                "name": row["name"],
                "section": row["section"],
                "title": row["title"],
                "workflows": row.get("workflows", []),
            }
            for row in sorted(ROWS, key = lambda row: row["name"])
        ],
        "sections": SECTIONS,
        "tool_not_applicable": tool_na,
        "support": sorted(support, key = lambda record: (record["family"], record["capability"], record["tool"])),
    }

def _support_matrix_facts_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(out, json.encode_indent(support_matrix_facts_dict(), indent = "  ") + "\n")
    return [DefaultInfo(files = depset([out]))]

support_matrix_facts = rule(
    implementation = _support_matrix_facts_impl,
    doc = "Writes the support matrix facts JSON.",
)
