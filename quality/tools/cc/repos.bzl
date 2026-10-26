"""C and C++ tool acquisition inventory."""

CC_TOOL_REPOS = [
    "cc_cppcheck",
]

CC_TOOLS = {
    "cc_cppcheck": {
        "kind": "archive",
        "build_file": "//quality/tools/cc:cppcheck.BUILD",
        "sha256": "f028ff75ca5372738f3737c8b3e8611426a6526b6aea2ef01301ab0f5902f044",
        "strip_prefix": "cppcheck-2.21.0",
        "urls": ["https://github.com/danmar/cppcheck/archive/refs/tags/2.21.0.tar.gz"],
    },
}
