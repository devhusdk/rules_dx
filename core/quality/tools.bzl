"""All quality tool rows, read from the generated per-family registry and ordered by id."""

load(":tools_registry.bzl", "FAMILY_TOOL_ROWS")

def _tool(row):
    return struct(
        adapters = row["adapters"],
        concerns = row["concerns"],
        config = struct(**row["config"]),
        id = row["id"],
        severity = struct(**row["severity"]),
        support = row["support"],
    )

FAMILY_TOOLS = {family: [_tool(row) for row in rows] for family, rows in FAMILY_TOOL_ROWS.items()}

TOOLS = sorted([t for rows in FAMILY_TOOLS.values() for t in rows], key = lambda t: t.id)
