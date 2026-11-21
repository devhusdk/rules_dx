"""Providers PHP rules expose to dx tooling."""

PhpInfo = provider(
    doc = "PHP sources a target makes loadable, staged by external_path() below one autoload root.",
    fields = {
        "autoload_files": "Depset of sources Composer requires on every run (its `files` autoload).",
        "package_files": "Depset of every file of the Composer packages of the target and its deps, manifests included.",
        "packages": "Depset of Composer package structs: name, version, manifest (File) and root (exec path of the package root).",
        "transitive_srcs": "Depset of every PHP source of the target and its deps.",
    },
)

PhpTestInfo = provider(
    doc = "What a php_test runs, so a wrapper can write its own launcher.",
    fields = {
        "autoload_root": "Runfiles path of the staged autoload tree.",
        "ini": "The php.ini settings of the test, as a dict.",
        "tokens": "Launcher tokens after the php flags: PHPUnit, its flags and the test files.",
    },
)
