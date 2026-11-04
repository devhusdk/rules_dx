import click.core
from click import Command


def test_command():
    assert Command is click.core.Command
