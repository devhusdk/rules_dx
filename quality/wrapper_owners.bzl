"""Wrapper ownership for every quality taxonomy family."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "REAL_CLASS_TO_FAMILY")
load(":taxonomy.bzl", "FAMILY_OWNERS")
load(":tool_defs.bzl", "is_canonical_token")


WRAPPER_OWNERS = FAMILY_OWNERS

def _taxonomy_families():
    seen = {}
    for class_id in REAL_CLASS_TO_FAMILY:
        seen[REAL_CLASS_TO_FAMILY[class_id]] = True
    return sorted(seen.keys())

def wrapper_owner(family):
    """Returns the wrapper owner label for one family, or "other" when explicitly uncovered."""
    if family in WRAPPER_OWNERS:
        return WRAPPER_OWNERS[family]
    return ""

def wrapper_owned_families():
    """Returns the sorted wrapper-owned families."""
    return sorted([f for f in WRAPPER_OWNERS if WRAPPER_OWNERS[f] != "other"])

def uncovered_families():
    """Returns the sorted explicitly uncovered families."""
    return sorted([f for f in WRAPPER_OWNERS if WRAPPER_OWNERS[f] == "other"])

def _is_owner_label(owner):
    if owner.startswith("@rules_dx_"):
        owner = "//" + owner.split("//", 1)[-1]
    return owner.startswith("//") and owner.endswith("/rules:defs.bzl")

def wrapper_schema_error():
    """Validates the wrapper-ownership schema."""
    for family in WRAPPER_OWNERS:
        if not is_canonical_token(family):
            return "wrapper owners: non-canonical family '" + str(family) + "'"
        owner = WRAPPER_OWNERS[family]
        if owner != "other" and not _is_owner_label(owner):
            return "wrapper owners: family '" + family + "' owner '" + str(owner) + "' must be a //.../rules:defs.bzl or @rules_dx_<family>//.../rules:defs.bzl label or \"other\""
    for family in _taxonomy_families():
        if family not in WRAPPER_OWNERS:
            return "wrapper owners: taxonomy family '" + family + "' has no owner or uncovered verdict"
    for family in WRAPPER_OWNERS:
        if family not in _taxonomy_families():
            return "wrapper owners: family '" + family + "' is outside the taxonomy"
    return ""

def _undispositioned_families():
    return sorted([f for f in _taxonomy_families() if f not in WRAPPER_OWNERS])

def _unknown_owner_families():
    taxonomy = _taxonomy_families()
    return sorted([f for f in WRAPPER_OWNERS if f not in taxonomy])

def _malformed_owners():
    bad = []
    for family in sorted(WRAPPER_OWNERS):
        owner = WRAPPER_OWNERS[family]
        if owner != "other" and not _is_owner_label(owner):
            bad.append(family)
    return bad

def wrapper_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "wrapper ownership schema validates",
                wrapper_schema_error(),
                "",
            ),
            expect_equal(
                "no taxonomy family lacks an owner or uncovered verdict",
                _undispositioned_families(),
                [],
            ),
            expect_equal(
                "no owner names a family outside the taxonomy",
                _unknown_owner_families(),
                [],
            ),
            expect_equal(
                "every owner is a wrapper label or the explicit other verdict",
                _malformed_owners(),
                [],
            ),
            expect_equal(
                "owned plus uncovered queries match the owner registry",
                [wrapper_owned_families(), uncovered_families()],
                [
                    sorted([f for f in WRAPPER_OWNERS if WRAPPER_OWNERS[f] != "other"]),
                    sorted([f for f in WRAPPER_OWNERS if WRAPPER_OWNERS[f] == "other"]),
                ],
            ),
            expect_equal(
                "every taxonomy class inherits an owner or uncovered verdict via its family",
                [c for c in sorted(REAL_CLASS_TO_FAMILY) if WRAPPER_OWNERS.get(REAL_CLASS_TO_FAMILY[c], "") == ""],
                [],
            ),
            expect_equal(
                "uncovered query matches the family derivation",
                uncovered_families(),
                sorted([f for f in _taxonomy_families() if WRAPPER_OWNERS.get(f, "") == "other"]),
            ),
            expect_equal(
                "wrapper owner query admits the frozen core",
                [wrapper_owner("rust"), wrapper_owner("python"), wrapper_owner("css")],
                ["//rust/rules:defs.bzl", "//python:defs.bzl", "//javascript:defs.bzl"],
            ),
            expect_equal(
                "wrapper owner query rejects unknown families",
                wrapper_owner("not_a_family"),
                "",
            ),
        ],
    )
