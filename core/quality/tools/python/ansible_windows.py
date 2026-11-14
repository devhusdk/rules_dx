"""POSIX stand-ins that let ansible-core and ansible-lint run on a Windows host."""

import multiprocessing
import os
import subprocess
import sys
import types

POSIX_ID = 1000

FCNTL_CONSTANTS = {
    "F_GETFD": 1,
    "F_SETFD": 2,
    "F_GETFL": 3,
    "F_SETFL": 4,
    "FD_CLOEXEC": 1,
    "LOCK_SH": 1,
    "LOCK_EX": 2,
    "LOCK_NB": 4,
    "LOCK_UN": 8,
}


def _fcntl_module():
    module = types.ModuleType("fcntl")
    module.__dict__.update(FCNTL_CONSTANTS)

    def fcntl(fd, cmd, arg=0):
        if isinstance(fd, int):
            os.fstat(fd)
        return 0

    def flock(fd, operation):
        return None

    def ioctl(fd, request, arg=0, mutate_flag=True):
        raise OSError("ioctl is not available on Windows")

    module.__dict__.update(fcntl=fcntl, flock=flock, lockf=flock, ioctl=ioctl)
    return module


def _termios_module():
    module = types.ModuleType("termios")

    class error(Exception):
        pass

    def unavailable(*_args):
        raise error("termios is not available on Windows")

    module.__dict__.update(
        error=error,
        tcgetattr=unavailable,
        tcsetattr=unavailable,
        tcflush=unavailable,
        tcgetpgrp=unavailable,
        __getattr__=lambda _name: 0,
    )
    return module


def _pwd_module():
    module = types.ModuleType("pwd")

    def missing(key):
        raise KeyError(key)

    module.__dict__.update(getpwuid=missing, getpwnam=missing, getpwall=lambda: [])
    return module


def _grp_module():
    module = types.ModuleType("grp")

    def missing(key):
        raise KeyError(key)

    module.__dict__.update(getgrgid=missing, getgrnam=missing, getgrall=lambda: [])
    return module


def _spawn_context(original):
    def get_context(method=None):
        return original("spawn" if method == "fork" else method)

    return get_context


def _shim_popen(script, names):
    base = subprocess.Popen

    class ShimPopen(base):
        def __init__(self, args, *rest, **kwargs):
            if isinstance(args, (list, tuple)) and args and args[0] in names:
                args = [sys.executable, script, "--ansible-shim", args[0]] + list(
                    args[1:]
                )
            super().__init__(args, *rest, **kwargs)

    return ShimPopen


def install(script, names):
    """Registers the POSIX stand-ins and routes ansible CLI subprocesses through script."""
    sys.modules.setdefault("fcntl", _fcntl_module())
    sys.modules.setdefault("termios", _termios_module())
    sys.modules.setdefault("pwd", _pwd_module())
    sys.modules.setdefault("grp", _grp_module())
    for name in ("getuid", "geteuid", "getgid", "getegid"):
        if not hasattr(os, name):
            setattr(os, name, lambda: POSIX_ID)
    multiprocessing.get_context = _spawn_context(multiprocessing.get_context)
    subprocess.Popen = _shim_popen(script, frozenset(names))
    os.environ.setdefault("USERNAME", "dx")
