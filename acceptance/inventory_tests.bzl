"""Executable checks over the frozen v1 acceptance inventory."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//quality:parity_tests.bzl", "PARITY_DEFERRED", "deferred_classes")
load("//quality:real_aspects.bzl", "real_wired_tools")
load("//quality:registry.bzl", "registry_classes", "registry_curated_families", "registry_families", "registry_tools")
load("//quality:wrapper_owners.bzl", "wrapper_owner")
load(":v1_inventory.bzl", "FROZEN_BASE_REVISION", "FROZEN_ON", "PROMISES", "PROMISE_COUNT", "PROMISE_FAMILIES", "SHARDS", "V1_EVIDENCE")

STATES = ["evidenced", "gapped"]

def _ids():
    """Returns the promise IDs in inventory order."""
    return [promise["id"] for promise in PROMISES]

def _ids_for(family):
    """Returns the promise IDs of one family."""
    return [promise["id"] for promise in PROMISES if promise["id"].startswith(family + ".")]

def _row(id):
    """Returns the promise with this ID, or None."""
    for promise in PROMISES:
        if promise["id"] == id:
            return promise
    return None

def _suffix(id):
    """Returns the part of a promise ID after its family."""
    return id.split(".", 1)[1]

def _missing(prefix, names):
    """Returns the registered names with no promise row."""
    return [name for name in names if _row(prefix + "." + name) == None]

def _extra(prefix, names):
    """Returns the promise rows naming no registered name."""
    return [id for id in _ids_for(prefix) if _suffix(id) not in names]

def _unwired_tool_misstates():
    """Returns the tool promises whose state disagrees with the wired-tool table."""
    return [id for id in _ids_for("tool") if (_row(id)["state"] == "gapped") != (_suffix(id) not in real_wired_tools())]

def _bad_labels(texts):
    """Returns the sorted texts that are neither package nor external labels."""
    return sorted([text for text in texts if not (text.startswith("//") or text.startswith("@"))])

def _noncanonical_test_labels(texts):
    """Returns the sorted texts that are not canonical package test labels."""
    return sorted([text for text in texts if not text.startswith("//") or ":" not in text])

def _label_owners():
    """Returns the owners that name the owning package rather than a registry value."""
    return [promise["owner"] for promise in PROMISES if not promise["id"].startswith("deferred.") and not promise["id"].startswith("family.")]

_UNMOVED = "_UNMOVED"

_ROW_CHECKS = [
    ("names no owner", lambda promise: promise["owner"] == ""),
    ("names no executable evidence", lambda promise: len(promise["evidence"]) == 0),
    ("records an unknown state", lambda promise: promise["state"] not in STATES),
    ("records a non-bool native applicability", lambda promise: type(promise["native"]) != "bool"),
]

def _family(id):
    """Returns the family part of a promise ID."""
    return id.split(".", 1)[0]

def promise_home(owner):
    """Returns the package whose shard holds a promise with this owner."""
    if not owner.startswith("//"):
        return "//quality"
    package = owner[2:].split(":")[0]
    if package == "docs" or package.startswith("docs/"):
        return "//tools/docs"
    return "//" + package

def problems(shards, count, families):
    """Returns the sorted problems of a sharded promise inventory."""
    found = {}
    rows = [promise for key in shards for promise in shards[key]]
    ids = [promise["id"] for promise in rows]
    if len(rows) != count:
        found["count: %d promise rows, want %d" % (len(rows), count)] = True
    seen = {}
    for id in ids:
        if id in seen:
            found["%s: duplicate promise ID" % id] = True
        seen[id] = True
        parts = id.split(".", 1)
        if len(parts) < 2 or parts[1] == "":
            found["%s: names no family member" % id] = True
        elif parts[0] not in families:
            found["%s: family %s is not declared" % (id, parts[0])] = True
    used = {_family(id): True for id in ids}
    for family in families:
        if family not in used:
            found["family %s: no promise row" % family] = True
    for key in shards:
        shard = [promise["id"] for promise in shards[key]]
        if len(shard) == 0:
            found["shard %s: no promise row" % key] = True
        if shard != sorted(shard):
            found["shard %s: rows are not sorted by ID" % key] = True
        for promise in shards[key]:
            home = promise_home(promise["owner"])
            if key != _UNMOVED and home != key:
                found["%s: owner %s belongs in shard %s, not %s" % (promise["id"], promise["owner"], home, key)] = True
            for message, failing in _ROW_CHECKS:
                if failing(promise):
                    found["%s: %s" % (promise["id"], message)] = True
    return sorted(found.keys())

_ALPHA = {"evidence": ["//alpha:alpha_test"], "id": "alpha.one", "native": False, "owner": "//alpha", "state": "evidenced"}
_BETA = {"evidence": ["//beta:beta_test"], "id": "beta.one", "native": True, "owner": "//beta:target", "state": "gapped"}
_FIXTURE = {"//alpha": [_ALPHA], "//beta": [_BETA]}
_FIXTURE_FAMILIES = ["alpha", "beta"]

def v1_inventory_registry_tests(name):
    """Instantiates the inventory shape and registry-completeness tests."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("the promise IDs are sorted", _ids(), sorted(_ids())),
            expect_equal("the sharded inventory has no problem", problems(SHARDS, PROMISE_COUNT, PROMISE_FAMILIES), []),
            expect_equal("a clean fixture inventory has no problem", problems(_FIXTURE, 2, _FIXTURE_FAMILIES), []),
            expect_equal("an omitted shard breaks the count", problems({"//alpha": [_ALPHA]}, 2, ["alpha"]), ["count: 1 promise rows, want 2"]),
            expect_equal("a duplicate ID across shards is reported", problems({"//alpha": [_ALPHA], "//beta": [_BETA, dict(_ALPHA, owner = "//beta")]}, 3, _FIXTURE_FAMILIES), ["alpha.one: duplicate promise ID", "shard //beta: rows are not sorted by ID"]),
            expect_equal("a stale empty shard is reported", problems({"//alpha": [_ALPHA], "//beta": [_BETA], "//gamma": []}, 2, _FIXTURE_FAMILIES), ["shard //gamma: no promise row"]),
            expect_equal("a row in the wrong owner shard is reported", problems({"//alpha": [_ALPHA, _BETA]}, 2, _FIXTURE_FAMILIES), ["beta.one: owner //beta:target belongs in shard //beta, not //alpha"]),
            expect_equal("a row without evidence is reported", problems({"//alpha": [dict(_ALPHA, evidence = [])], "//beta": [_BETA]}, 2, _FIXTURE_FAMILIES), ["alpha.one: names no executable evidence"]),
            expect_equal("a changed family list is reported", problems(_FIXTURE, 2, ["alpha", "gamma"]), ["beta.one: family beta is not declared", "family gamma: no promise row"]),
            expect_equal("a changed count is reported", problems(_FIXTURE, 3, _FIXTURE_FAMILIES), ["count: 2 promise rows, want 3"]),
            expect_equal("a bad state and native value are reported", problems({"//alpha": [dict(_ALPHA, state = "done", native = "no")], "//beta": [_BETA]}, 2, _FIXTURE_FAMILIES), ["alpha.one: records a non-bool native applicability", "alpha.one: records an unknown state"]),
            expect_equal("unmoved rows skip only the ownership check", problems({_UNMOVED: [_ALPHA, dict(_BETA, owner = "")]}, 2, _FIXTURE_FAMILIES), ["beta.one: names no owner"]),
            expect_equal("every owner maps to its home shard", [promise_home(owner) for owner in ["//:MODULE.bazel", "//docs:command_docs", "@dx_tools//:x", "ADR", "//cli/cli:man_pages"]], ["//", "//tools/docs", "//quality", "//quality", "//cli/cli"]),
            expect_equal("every package owner is a package or external label", _bad_labels(_label_owners()), []),
            expect_equal("every evidence target is a canonical package test label", _noncanonical_test_labels(V1_EVIDENCE), []),
            expect_equal("the evidence list is sorted", V1_EVIDENCE, sorted(V1_EVIDENCE)),
            expect_equal("the freeze records the base revision", len(FROZEN_BASE_REVISION), 40),
            expect_true("the freeze records its date", FROZEN_ON != ""),
            expect_equal("every adapter tool is inventoried", _missing("tool", registry_tools()), []),
            expect_equal("no tool promise names an unregistered tool", _extra("tool", registry_tools()), []),
            expect_equal("a wired tool is evidenced and an unwired tool is gapped", _unwired_tool_misstates(), []),
            expect_equal("every semantic file class is inventoried", _missing("class", registry_classes()), []),
            expect_equal("no class promise names an unregistered class", _extra("class", registry_classes()), []),
            expect_equal("every taxonomy family is inventoried", _missing("family", registry_families()), []),
            expect_equal("no family promise names an unknown family", _extra("family", registry_families()), []),
            expect_equal("every curated family is inventoried", _missing("curated", registry_curated_families()), []),
            expect_equal("no curated promise names an uncurated family", _extra("curated", registry_curated_families()), []),
            expect_equal("every deferred class is inventoried", _missing("deferred", deferred_classes()), []),
            expect_equal("no deferred promise names an undeferred class", _extra("deferred", deferred_classes()), []),
            expect_equal("every family promise repeats its wrapper owner", [id for id in _ids_for("family") if _row(id)["owner"] != wrapper_owner(_suffix(id))], []),
            expect_equal("every deferred promise repeats its owning decision", [id for id in _ids_for("deferred") if _row(id)["owner"] != PARITY_DEFERRED[_suffix(id)][0]], []),
        ],
    )
