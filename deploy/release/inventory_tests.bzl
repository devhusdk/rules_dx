"""Tests for the shipped crate inventory."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":inventory.bzl", "ShippedCratesInfo", "crate_name_tag", "inventory_text_path", "license_text_member", "license_text_name", "shipped_crates_aspect", "shipped_inventory")

def inventory_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "license_text_name keeps license, licence, copying and notice files",
                [license_text_name(n) for n in ["LICENSE-MIT", "licence.txt", "COPYING", "NOTICE", "Notice.md"]],
                [True, True, True, True, True],
            ),
            expect_equal(
                "license_text_name drops other files",
                [license_text_name(n) for n in ["README.md", "Cargo.toml", "MYLICENSE"]],
                [False, False, False],
            ),
            expect_equal(
                "license_text_member keeps root texts and LICENSES/ files only",
                [license_text_member(n) for n in ["LICENSE", "LICENSES/MIT.txt", "LICENSES/a/MIT.txt", "src/LICENSE", "README.md"]],
                [True, True, False, False, False],
            ),
            expect_equal(
                "crate_name_tag reads the crate-name tag",
                crate_name_tag(["cargo-bazel", "crate-name=aho-corasick", "manual"]),
                "aho-corasick",
            ),
            expect_equal(
                "crate_name_tag is empty without the tag",
                crate_name_tag(["manual"]),
                "",
            ),
            expect_equal(
                "inventory_text_path keys texts by crate and version",
                inventory_text_path("cell_inventory", "serde", "1.0.0"),
                "cell_inventory_texts/cargo/serde-1.0.0/LICENSE",
            ),
        ],
    )

def _shipped_crates_probe_impl(ctx):
    """Writes which probed crates the aspect found."""
    found = [c.crate for c in ctx.attr.app[ShippedCratesInfo].crates.to_list()]
    lines = [p + (" present" if p in found else " absent") for p in ctx.attr.probes]
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(files = depset([out]))]

_shipped_crates_probe = rule(
    implementation = _shipped_crates_probe_impl,
    doc = "Writes one present or absent line per probed crate name.",
    attrs = {
        "app": attr.label(mandatory = True, aspects = [shipped_crates_aspect]),
        "probes": attr.string_list(mandatory = True),
    },
)

def inventory_file_tests(name, app, present, absent):
    """Checks the inventory of one binary lists `present` crates and skips `absent` ones."""
    shipped_inventory(name = name + "_inventory", app = app)
    _shipped_crates_probe(
        name = name + "_probe",
        app = app,
        probes = present + absent,
    )
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            ":" + name + "_inventory": "\n".join([c + "|cargo|" for c in present]),
            ":" + name + "_probe": "\n".join([c + " present" for c in present] + [c + " absent" for c in absent]),
        },
    )
