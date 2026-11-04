"""Plans the language list and renders the BUILD file of @dx_gazelle."""

DISPATCH = Label("//gazelle/dispatch")

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

def render_hub_build(labels):
    """Renders the BUILD file with one gazelle_binary of the given languages.

    Args:
      labels: Label objects from every dx_gazelle.language tag.

    Returns:
      The BUILD file text.
    """
    languages = "".join(['        "%s",\n' % label for label in hub_languages(labels)])
    return """load("@gazelle//:def.bzl", "gazelle_binary")

gazelle_binary(
    name = "gazelle",
    languages = [
{languages}    ],
    tags = ["no-coverage"],
    version = 1,
    visibility = [
        "@rules_dx//dx:__pkg__",
        "@rules_dx//gazelle:__subpackages__",
    ],
)
""".format(languages = languages)
