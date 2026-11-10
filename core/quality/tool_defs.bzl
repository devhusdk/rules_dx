"""Constructors for quality tool rows: tool(), row(), config() and severity()."""

def row(state = "supported", transport = "batched", context = "owned", **fields):
    """Returns one support record; the tool id is added by tool()."""
    return dict(fields, state = state, transport = transport, context = context)

def config(
        suffixes = [],
        files = [],
        ext = "",
        extends_syntax = "",
        shared = False,
        tier_dir = "",
        comment = "",
        pyproject_table = "",
        extras = [],
        references = [],
        kind = "file",
        tier_active = False,
        tier_base = None,
        stage_native = False,
        tier_source = "dx",
        family = "",
        extensions = [],
        init_order = 0,
        suppressions = [],
        suppression_extensions = [],
        rule_keys = []):
    """Returns a tool's config metadata."""
    if tier_base == None:
        tier_base = tier_active and kind == "file"
    if tier_base and (kind != "file" or not tier_active):
        fail("config: tier_base needs a tier_active tool of kind file")
    return struct(
        comment = comment,
        ext = ext,
        extends_syntax = extends_syntax,
        extensions = extensions,
        extras = extras,
        family = family,
        files = files,
        init_order = init_order,
        kind = kind,
        pyproject_table = pyproject_table,
        references = references,
        rule_keys = rule_keys,
        shared = shared,
        stage_native = stage_native,
        suffixes = suffixes,
        suppression_extensions = suppression_extensions,
        suppressions = suppressions,
        tier_active = tier_active,
        tier_base = tier_base,
        tier_dir = tier_dir,
        tier_source = tier_source,
    )

_NO_CONFIG = config(kind = "none")

_LEVELS = ["error", "warning", "info"]

def severity(default = None, categories = {}):
    """Returns a tool's severity map, applied once after its parser has run."""
    for level in [default] + categories.values():
        if level != None and level not in _LEVELS:
            fail("severity level %r is not one of %s" % (level, _LEVELS))
    return struct(categories = categories, default = default)

_NO_SEVERITY = severity()

def is_canonical_token(text):
    """Reports whether a token uses the canonical alphabet."""
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def tool(id, adapters, support = [], config = _NO_CONFIG, severity = _NO_SEVERITY, concerns = {}):
    """Returns one tool: adapters per capability, support records, and concerns not named after the capability."""
    for capability, concern in concerns.items():
        if capability not in adapters or not is_canonical_token(concern):
            fail("tool %s: concerns %s=%s needs an adapter capability and a canonical concern" % (id, capability, concern))
    return struct(
        adapters = adapters,
        concerns = {capability: concerns.get(capability, capability) for capability in adapters},
        config = config,
        id = id,
        severity = severity,
        support = [dict(record, tool = id) for record in support],
    )
