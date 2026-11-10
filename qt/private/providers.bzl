"""Providers of the Qt rules."""

QtInfo = provider(
    doc = "Qt metadata one Qt target exports to its dependents.",
    fields = {
        "metatypes": "depset[File]: collected moc JSON of the target and its deps.",
        "translations": "depset[str]: 'stem=label' of every translation the target and its deps embed.",
    },
)
