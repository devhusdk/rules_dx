"""Pinned Flang compiler, flang-rt sources and rules_fortran identities."""

FLANG_VERSION = "21.1.8"

RULES_FORTRAN_COMMIT = "f0cac73ebdcbda7bc01b38b8a4ee33869416c220"

RULES_FORTRAN_SHA256 = "ca07c12300ade0a7e5fff92eb95cc27e4618af993841b92a9784d239c34c7557"

RULES_FORTRAN_URL = "https://github.com/miinso/rules_fortran/archive/" + RULES_FORTRAN_COMMIT + ".tar.gz"

FLANG_RT_URL = "https://github.com/llvm/llvm-project/releases/download/llvmorg-" + FLANG_VERSION + "/llvm-project-" + FLANG_VERSION + ".src.tar.xz"

FLANG_RT_SHA256 = "4633a23617fa31a3ea51242586ea7fb1da7140e426bd62fc164261fe036aa142"

FLANG_RT_PREFIX = "llvm-project-" + FLANG_VERSION + ".src"

FLANG_RELEASES = "https://github.com/miinso/flang-releases/releases/download/v" + FLANG_VERSION + "/"

FORTRAN_CELLS = {
    "linux_x86_64": struct(
        constraints = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        url = FLANG_RELEASES + "flang+llvm-" + FLANG_VERSION + "-x86_64-unknown-linux-gnu.tar.gz",
        sha256 = "673f4dc1e95d09b1253ca32078ae058540d68b533bdcbcbf72b7242fbed82dc6",
        strip_prefix = "flang+llvm-" + FLANG_VERSION,
    ),
}
