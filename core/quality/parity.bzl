"""Fail-closed v1 parity gate."""

load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")
load(":tool_defs.bzl", "is_canonical_token")

visibility(["//core/...", "//tools/..."])

PARITY_DEFERRED = {}

def _adapter_backed_classes():
    backed = {}
    for tool in REAL_ADAPTERS:
        for capability in REAL_ADAPTERS[tool]:
            for class_id in REAL_ADAPTERS[tool][capability]:
                backed[class_id] = True
    return backed

def adapter_backed_classes():
    """Returns the sorted adapter-backed classes."""
    return sorted(_adapter_backed_classes().keys())

def deferred_classes():
    """Returns the sorted explicitly deferred classes."""
    return sorted(PARITY_DEFERRED.keys())

def parity_schema_error():
    """Validates the parity-gate schema."""
    for class_id in PARITY_DEFERRED:
        if not is_canonical_token(class_id):
            return "parity gate: non-canonical deferred class '" + str(class_id) + "'"
        entry = PARITY_DEFERRED[class_id]
        if len(entry) != 2 or entry[0] == "" or entry[1] == "":
            return "parity gate: deferred class '" + class_id + "' must name an owning decision and a frozen route"
        owner = entry[0]
        if not owner.startswith("O") and not owner.startswith("ADR"):
            return "parity gate: deferred class '" + class_id + "' owner '" + owner + "' must start with O or ADR"
    return ""

def unclassified_adapter_classes():
    """Returns adapter-backed classes missing from the family map."""
    backed = _adapter_backed_classes()
    return sorted([c for c in backed if c not in REAL_CLASS_TO_FAMILY])

def undispositioned_classes():
    """Returns classified classes neither adapter-backed nor deferred."""
    backed = _adapter_backed_classes()
    return sorted([c for c in REAL_CLASS_TO_FAMILY if c not in backed and c not in PARITY_DEFERRED])

def double_claimed_classes():
    """Returns deferred classes an adapter also claims."""
    backed = _adapter_backed_classes()
    return sorted([c for c in PARITY_DEFERRED if c in backed])

def malformed_deferrals():
    """Returns deferrals lacking an owning decision or frozen route."""
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

def deferred_pipeline_error(target_classes, capability, registry = None):
    """Returns a fail-closed error for deferred classes with sources."""
    if registry == None:
        registry = PARITY_DEFERRED
    deferred = sorted([c for c in target_classes if c in registry])
    if len(deferred) == 0:
        return ""
    details = []
    for class_id in deferred:
        entry = registry[class_id]
        details.append(class_id + " (" + entry[0] + ": " + entry[1] + ")")
    return "deferred " + capability + " pipeline for " + ", ".join(details) + ": no adapter claims these classes"
