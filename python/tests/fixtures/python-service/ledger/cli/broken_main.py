"""Entry point whose click dependency is deliberately undeclared."""

import click  # ty: ignore[unresolved-import]

click.echo("unreachable")
