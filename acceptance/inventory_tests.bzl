"""Executable checks over the frozen v1 acceptance inventory."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//quality:parity_tests.bzl", "PARITY_DEFERRED", "deferred_classes")
load("//quality:real_aspects.bzl", "real_wired_tools")
load("//quality:registry.bzl", "registry_classes", "registry_curated_families", "registry_families", "registry_tools")
load("//quality:wrapper_owners.bzl", "wrapper_owner")
load(":v1_inventory.bzl", "FROZEN_BASE_REVISION", "FROZEN_ON", "PROMISES", "PROMISE_COUNT", "PROMISE_FAMILIES", "V1_EVIDENCE")

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

def _evidence_targets():
    """Returns the sorted union of every promise's evidence targets."""
    seen = {}
    for promise in PROMISES:
        for target in promise["evidence"]:
            seen[target] = True
    return sorted(seen.keys())

def _bad_labels(texts):
    """Returns the sorted texts that are neither package nor external labels."""
    return sorted([text for text in texts if not (text.startswith("//") or text.startswith("@"))])

def _label_owners():
    """Returns the owners that name the owning package rather than a registry value."""
    return [promise["owner"] for promise in PROMISES if not promise["id"].startswith("deferred.") and not promise["id"].startswith("family.")]

def v1_inventory_registry_tests(name):
    """Instantiates the inventory shape and registry-completeness tests."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("the promise count matches the frozen inventory", len(PROMISES), PROMISE_COUNT),
            expect_equal("the promise IDs are unique", len(_ids()), len({id: True for id in _ids()})),
            expect_equal("the promise IDs are sorted", _ids(), sorted(_ids())),
            expect_equal("every declared family carries a promise", [family for family in PROMISE_FAMILIES if len(_ids_for(family)) == 0], []),
            expect_equal("every promise declares its family", [id for id in _ids() if _suffix(id) == "" or id.split(".", 1)[0] not in PROMISE_FAMILIES], []),
            expect_equal("every promise names an owner", [promise["id"] for promise in PROMISES if promise["owner"] == ""], []),
            expect_equal("every promise names executable evidence", [promise["id"] for promise in PROMISES if len(promise["evidence"]) == 0], []),
            expect_equal("every promise records a known state", sorted([promise["id"] for promise in PROMISES if promise["state"] not in STATES]), []),
            expect_equal("every promise records native applicability", [promise["id"] for promise in PROMISES if type(promise["native"]) != "bool"], []),
            expect_equal("every package owner is a package or external label", _bad_labels(_label_owners()), []),
            expect_equal("every evidence target is a package or external label", _bad_labels(V1_EVIDENCE), []),
            expect_equal("the evidence list is the union of the promise evidence", V1_EVIDENCE, _evidence_targets()),
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
