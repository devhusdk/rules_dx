import os

from . import shell_completion


class Command:
    def complete(self):
        return shell_completion.complete(os.getcwd())
