"""Plans the language list and renders the BUILD file of @dx_gazelle."""

load("//cli/prebuilt:hub.bzl", "KNOWN_CPU", "KNOWN_OS")

DISPATCH = Label("//gazelle/dispatch")

_VISIBILITY = """    visibility = [
        "@rules_dx//dx:__pkg__",
        "@rules_dx//gazelle:__subpackages__",
    ],
"""

def hub_languages(labels):
    """Returns the registered language labels without duplicates, ordered by name, with dispatch last.

    Args:
      labels: Label objects from every dx_gazelle.language tag.

    Returns:
      The ordered Label objects.
    """
    unique = {}
    for label in labels:
        if label != DISPATCH:
            unique[str(label)] = label
    return sorted(unique.values(), key = lambda label: label.name) + [DISPATCH]

def builds_from_source(plan, modules):
    """Returns whether gazelle builds from source instead of using the release binary.

    The release binary holds only the languages rules_dx registers, so a
    language from any other module needs a source build.

    Args:
      plan: The struct plan_root returns.
      modules: Structs with name and tags.language.

    Returns:
      True when gazelle builds from source.
    """
    return plan.source or any([m.name != "rules_dx" and m.tags.language for m in modules])

def _gazelle_binary(name, labels, visibility):
    languages = "".join(['        "%s",\n' % label for label in hub_languages(labels)])
    return """gazelle_binary(
    name = "{name}",
    languages = [
{languages}    ],
    tags = ["no-coverage"],
    version = 1,
{visibility})
""".format(name = name, languages = languages, visibility = visibility)

def render_hub_build(labels):
    """Renders the BUILD file with one gazelle_binary of the given languages.

    Args:
      labels: Label objects from every dx_gazelle.language tag.

    Returns:
      The BUILD file text.
    """
    return 'load("@gazelle//:def.bzl", "gazelle_binary")\n\n' + _gazelle_binary("gazelle", labels, _VISIBILITY)

def unsupported_message(assets):
    """Returns the failure text for a platform with no gazelle asset."""
    return (
        "dx: no prebuilt gazelle for {os} {cpu}. Prebuilt gazelle exists for: " +
        ", ".join(["{} {}".format(a.os, a.cpu) for a in assets]) +
        ". In your root MODULE.bazel, add dx_gazelle.asset(name, url, sha256, os, cpu) for a gazelle you built," +
        " or dx_gazelle.from_source(), which requires a Go toolchain."
    )

def _select(entries, default):
    lines = ["select({"]
    for condition, value in entries:
        lines.append('        "{}": "{}",'.format(condition, value))
    lines.append('        "//conditions:default": "{}",'.format(default))
    lines.append("    })")
    return "\n".join(lines)

def prebuilt_build(assets, labels):
    """Renders the BUILD file that selects a prebuilt gazelle by os and cpu.

    The select runs in the exec configuration of the gazelle rule, so it
    picks the binary for the machine that runs it. A platform whose asset
    has no pin builds gazelle from source; with every asset pinned, nothing
    loads @gazelle, so no Go toolchain is fetched.

    Args:
      assets: The structs plan_assets returns, at least one pinned.
      labels: Label objects from every dx_gazelle.language tag.

    Returns:
      The BUILD file text.
    """
    source = [a for a in assets if not a.repo]
    lines = ['load("@bazel_skylib//rules:native_binary.bzl", "native_binary")']
    if source:
        lines.append('load("@gazelle//:def.bzl", "gazelle_binary")')
    lines += [
        'load("@rules_dx//cli/prebuilt:fail.bzl", "dx_unavailable")',
        "",
        "alias(",
        '    name = "gazelle",',
        "    actual = " + _select([(":" + a.key, ":gazelle_" + a.key if a.repo else ":gazelle_source") for a in assets], ":unsupported") + ",",
        _VISIBILITY + ")",
    ]
    for a in assets:
        lines += [
            "",
            "config_setting(",
            '    name = "{}",'.format(a.key),
            "    constraint_values = [",
            '        "@platforms//os:{}",'.format(a.os),
            '        "@platforms//cpu:{}",'.format(a.cpu),
            "    ],",
            ")",
        ]
        if a.repo:
            lines += [
                "",
                "native_binary(",
                '    name = "gazelle_{}",'.format(a.key),
                '    src = "@{}//file:{}",'.format(a.repo, a.exe),
                '    out = "{}/{}",'.format(a.key, a.exe),
                ")",
            ]
    if source:
        lines += ["", _gazelle_binary("gazelle_source", labels, "").rstrip("\n")]
    lines += [
        "",
        "dx_unavailable(",
        '    name = "unsupported",',
        "    cpu = " + _select([("@platforms//cpu:" + c, c) for c in KNOWN_CPU], "unknown cpu") + ",",
        "    message = {},".format(json.encode(unsupported_message(assets))),
        "    os = " + _select([("@platforms//os:" + o, o) for o in KNOWN_OS], "unknown os") + ",",
        ")",
        "",
    ]
    return "\n".join(lines)
