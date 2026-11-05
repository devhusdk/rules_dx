"""Ledger command line entry point."""

import json
import sys

import click
from ledger.core import build_info
from ledger.core.catalog import Catalog, UnknownSku
from ledger.core.config import load_config
from ledger.service.server import make_server


@click.group()
@click.option(
    "--config", "config_path", type=click.Path(dir_okay=False), help="Config JSON file."
)
@click.pass_context
def cli(ctx, config_path):
    """Query the ledger catalog."""
    try:
        ctx.obj = Catalog(load_config(config_path))
    except (OSError, ValueError) as error:
        raise click.ClickException(str(error)) from error


@cli.command()
def info():
    """Print schema and data identity."""
    click.echo(f"schema {build_info.SCHEMA_VERSION}")
    click.echo(f"items {build_info.ITEM_COUNT}")
    click.echo(f"sha256 {build_info.ITEMS_SHA256}")


@cli.command("list")
@click.pass_obj
def list_items(catalog):
    """List catalog items."""
    for item in catalog.items():
        click.echo(f"{item.sku}\t{item.name}\t{item.price_cents}")


@cli.command()
@click.argument("sku")
@click.option("--qty", default=1, show_default=True, help="Units to quote.")
@click.pass_obj
def quote(catalog, sku, qty):
    """Print a quote as JSON."""
    try:
        click.echo(json.dumps(catalog.quote(sku, qty), sort_keys=True))
    except UnknownSku as error:
        raise click.ClickException(f"unknown sku {error.args[0]}") from error
    except ValueError as error:
        raise click.UsageError(str(error)) from error


@cli.command()
@click.option("--port", default=0, show_default=True, help="Port to bind; 0 picks one.")
@click.option(
    "--max-requests",
    default=0,
    show_default=True,
    help="Exit after N requests; 0 means never.",
)
@click.pass_obj
def serve(catalog, port, max_requests):
    """Serve the catalog on 127.0.0.1."""
    server = make_server(catalog, port)
    server.daemon_threads = False
    click.echo(f"listening 127.0.0.1:{server.server_address[1]}")
    sys.stdout.flush()
    served = 0
    while max_requests == 0 or served < max_requests:
        server.handle_request()
        served += 1
    server.server_close()


def main():
    cli()


if __name__ == "__main__":
    main()
