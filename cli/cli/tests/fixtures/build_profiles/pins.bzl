"""Build-profile flags plus DX_PROFILE forwarding fixture."""

PROFILE_COMMANDS = [
    "build",
    "run",
    "test",
    "deploy",
    "watch",
]
PROFILE_FLAGS = ["--debug", "--release"]

PROFILE_DEBUG_CONFIG = "dx_debug"
PROFILE_DEV_CONFIG = "dx_dev"
PROFILE_RELEASE_CONFIG = "dx_release"
PROFILE_DEBUG_MODE = "dbg"
PROFILE_DEV_MODE = "fastbuild"
PROFILE_RELEASE_MODE = "opt"

PROFILE_DEFAULT_BUILD = "dev"
PROFILE_DEFAULT_RUN = "dev"
PROFILE_DEFAULT_TEST = "dev"
PROFILE_DEFAULT_DEPLOY = "release"

PROFILE_PRECEDENCE = "flag over attr over default"

PROFILE_NO_DEV_FLAG = True
PROFILE_CONFLICT_EXIT = 2

PROFILE_COVERAGE_UNCHANGED = True

PROFILE_ATTR_VOCABULARY = ["debug", "dev", "release"]

DX_PROFILE_ENV = "DX_PROFILE"
DX_PROFILE_VALUES = ["debug", "dev", "release"]

REJECTED_SILENT_IGNORE = "silent ignore rejected"
REJECTED_DEV_FLAG = "--dev flag rejected"
REJECTED_COVERAGE_PROFILE = "coverage profile rejected"

NO_SUPPORTED = "no Supported claim"
SEED_ONLY = "qualified seed-only"
