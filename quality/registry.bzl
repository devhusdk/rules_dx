"""Single-sourced versioned registry queries."""

load(":adapters.bzl", "ADAPTER_REGISTRY_SCHEMA_VERSION", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY", "adapter_registry_schema_error")
load(":curated_defaults.bzl", "CURATED_DEFAULTS", "CURATED_SCHEMA_VERSION", "FORMAT_FROZEN", "curated_schema_error")
load(":parity_tests.bzl", "PARITY_DEFERRED", "PARITY_SCHEMA_VERSION", "parity_schema_error")
load(":sources.bzl", "KNOWN_SEMANTIC_FILE_CLASSES", "SOURCES_REGISTRY_SCHEMA_VERSION", "sources_schema_error")
load(":support.bzl", "SUPPORT_SCHEMA_VERSION", "support_schema_error")
load(":wrapper_owners.bzl", "WRAPPER_OWNERS", "WRAPPER_SCHEMA_VERSION", "wrapper_schema_error")

REGISTRY_SCHEMA_VERSION = 1

def registry_classes():
    """Returns the sorted registry classes via query (never duplicated)."""
    return sorted(REAL_CLASS_TO_FAMILY.keys())

def registry_families():
    """Returns the sorted unique owning families via query."""
    seen = {}
    for class_id in REAL_CLASS_TO_FAMILY:
        seen[REAL_CLASS_TO_FAMILY[class_id]] = True
    return sorted(seen.keys())

def registry_tools():
    """Returns the sorted known real adapter tools via query."""
    return sorted(REAL_ADAPTERS.keys())

def registry_curated_families():
    """Returns the sorted curated families via query."""
    return sorted(CURATED_DEFAULTS.keys())

def registry_deferred_classes():
    """Returns the sorted deferred classes via query."""
    return sorted(PARITY_DEFERRED.keys())

def is_registry_class(class_id):
    """Reports whether a class is in the single-sourced registry."""
    return class_id in REAL_CLASS_TO_FAMILY

def is_registry_tool(tool_id):
    """Reports whether a tool is in the single-sourced registry."""
    return tool_id in REAL_ADAPTERS

def is_curated_family(family):
    """Reports whether a family carries curated defaults."""
    return family in CURATED_DEFAULTS

def registry_schema_error():
    """Validates the aggregated versioned registry schemas."""
    if REGISTRY_SCHEMA_VERSION != 1:
        return "registry: unsupported schema v" + str(REGISTRY_SCHEMA_VERSION) + " (want v1)"
    if SOURCES_REGISTRY_SCHEMA_VERSION != 1:
        return "registry: sources schema v" + str(SOURCES_REGISTRY_SCHEMA_VERSION) + " is unsupported (want v1)"
    if ADAPTER_REGISTRY_SCHEMA_VERSION != 1:
        return "registry: adapter schema v" + str(ADAPTER_REGISTRY_SCHEMA_VERSION) + " is unsupported (want v1)"
    if CURATED_SCHEMA_VERSION != 1:
        return "registry: curated schema v" + str(CURATED_SCHEMA_VERSION) + " is unsupported (want v1)"
    if PARITY_SCHEMA_VERSION != 1:
        return "registry: parity schema v" + str(PARITY_SCHEMA_VERSION) + " is unsupported (want v1)"
    if WRAPPER_SCHEMA_VERSION != 1:
        return "registry: wrapper schema v" + str(WRAPPER_SCHEMA_VERSION) + " is unsupported (want v1)"
    if SUPPORT_SCHEMA_VERSION != 1:
        return "registry: support schema v" + str(SUPPORT_SCHEMA_VERSION) + " is unsupported (want v1)"
    err = sources_schema_error()
    if err != "":
        return err
    err = adapter_registry_schema_error()
    if err != "":
        return err
    err = curated_schema_error()
    if err != "":
        return err
    err = parity_schema_error()
    if err != "":
        return err
    err = wrapper_schema_error()
    if err != "":
        return err
    err = support_schema_error()
    if err != "":
        return err

    for class_id in KNOWN_SEMANTIC_FILE_CLASSES:
        if class_id not in REAL_CLASS_TO_FAMILY:
            return "registry: known class '" + class_id + "' has no owning family"
    for tool in REAL_ADAPTERS:
        for capability in REAL_ADAPTERS[tool]:
            for class_id in REAL_ADAPTERS[tool][capability]:
                if class_id not in REAL_CLASS_TO_FAMILY:
                    return "registry: tool '" + tool + "' names unclassified class '" + class_id + "'"
    families = {}
    for class_id in REAL_CLASS_TO_FAMILY:
        families[REAL_CLASS_TO_FAMILY[class_id]] = True
    for family in CURATED_DEFAULTS:
        if family not in families:
            return "registry: curated family '" + family + "' is outside the taxonomy"
    for family in CURATED_DEFAULTS:
        for capability in CURATED_DEFAULTS[family]:
            for tool in CURATED_DEFAULTS[family][capability]:
                if tool not in REAL_ADAPTERS:
                    return "registry: curated tool '" + tool + "' for family '" + family + "' is outside REAL_ADAPTERS"
    for family in FORMAT_FROZEN:
        if family not in CURATED_DEFAULTS:
            return "registry: FORMAT_FROZEN family '" + family + "' has no curated entry"
    for class_id in REAL_CLASS_TO_FAMILY:
        family = REAL_CLASS_TO_FAMILY[class_id]
        if family not in WRAPPER_OWNERS:
            return "registry: taxonomy family '" + family + "' has no wrapper owner or uncovered verdict"
    return ""
