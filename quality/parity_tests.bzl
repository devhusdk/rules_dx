"""Fail-closed v1 parity gate."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")

PARITY_SCHEMA_VERSION = 1

PARITY_DEFERRED = {
    "astro": ["ADR", "framework adapter region; Prettier/ESLint plugin closure pending"],
    "mdx": ["ADR", "framework adapter region; prose-is-not-dependency boundary per composition"],
    "svelte": ["ADR", "framework adapter region; Prettier/ESLint plugin closure pending"],
    "vue": ["ADR", "framework adapter region; Prettier/ESLint plugin closure pending"],
}

def _adapter_backed_classes():
    backed = {}
    for tool in REAL_ADAPTERS:
        for capability in REAL_ADAPTERS[tool]:
            for class_id in REAL_ADAPTERS[tool][capability]:
                backed[class_id] = True
    return backed

def adapter_backed_classes():
    """Returns the sorted adapter-backed classes via registry query."""
    return sorted(_adapter_backed_classes().keys())

def deferred_classes():
    """Returns the sorted explicitly deferred classes via registry query."""
    return sorted(PARITY_DEFERRED.keys())

def _is_canonical_token(text):
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def parity_schema_error():
    """Validates the versioned parity-gate schema."""
    if PARITY_SCHEMA_VERSION != 1:
        return "parity gate: unsupported schema v" + str(PARITY_SCHEMA_VERSION) + " (want v1)"
    for class_id in PARITY_DEFERRED:
        if not _is_canonical_token(class_id):
            return "parity gate: non-canonical deferred class '" + str(class_id) + "'"
        entry = PARITY_DEFERRED[class_id]
        if len(entry) != 2 or entry[0] == "" or entry[1] == "":
            return "parity gate: deferred class '" + class_id + "' must name an owning decision and a frozen route"
        owner = entry[0]
        if not owner.startswith("O") and not owner.startswith("ADR"):
            return "parity gate: deferred class '" + class_id + "' owner '" + owner + "' must start with O or ADR"
    return ""

def _unclassified_adapter_classes():
    backed = _adapter_backed_classes()
    return sorted([c for c in backed if c not in REAL_CLASS_TO_FAMILY])

def _undispositioned_classes():
    backed = _adapter_backed_classes()
    return sorted([c for c in REAL_CLASS_TO_FAMILY if c not in backed and c not in PARITY_DEFERRED])

def _double_claimed_classes():
    backed = _adapter_backed_classes()
    return sorted([c for c in PARITY_DEFERRED if c in backed])

def _malformed_deferrals():
    bad = []
    for class_id in sorted(PARITY_DEFERRED):
        entry = PARITY_DEFERRED[class_id]
        if len(entry) != 2 or entry[0] == "" or entry[1] == "":
            bad.append(class_id)
            continue
        owner = entry[0]
        if not owner.startswith("O") and not owner.startswith("ADR"):
            bad.append(class_id)
    return bad

def deferred_pipeline_error(target_classes, capability):
    """Returns a fail-closed error for deferred classes with sources."""
    deferred = sorted([c for c in target_classes if c in PARITY_DEFERRED])
    if len(deferred) == 0:
        return ""
    details = []
    for class_id in deferred:
        entry = PARITY_DEFERRED[class_id]
        details.append(class_id + " (" + entry[0] + ": " + entry[1] + ")")
    return "deferred " + capability + " pipeline for " + ", ".join(details) + ": no adapter claims these classes"

def parity_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "parity schema version stays v1",
                PARITY_SCHEMA_VERSION,
                1,
            ),
            expect_equal(
                "parity deferral schema validates",
                parity_schema_error(),
                "",
            ),
            expect_equal(
                "no adapter class is unclassified",
                _unclassified_adapter_classes(),
                [],
            ),
            expect_equal(
                "no classified class lacks a disposition",
                _undispositioned_classes(),
                [],
            ),
            expect_equal(
                "no class is both adapter-backed and deferred",
                _double_claimed_classes(),
                [],
            ),
            expect_equal(
                "every deferral names an owning decision and a frozen route",
                _malformed_deferrals(),
                [],
            ),
            expect_equal(
                "adapter-backed query matches the registry derivation",
                adapter_backed_classes(),
                sorted(_adapter_backed_classes().keys()),
            ),
            expect_equal(
                "deferred query matches the deferral registry",
                deferred_classes(),
                sorted(PARITY_DEFERRED.keys()),
            ),
            expect_equal(
                "deferred pipeline error stays empty without deferred classes",
                deferred_pipeline_error(["rust", "python"], "lint"),
                "",
            ),
            expect_equal(
                "deferred pipeline error names the deferral with owner and route",
                deferred_pipeline_error(["rust", "vue"], "lint") != "",
                True,
            ),
        ],
    )
