"""Providers PHP rules expose to dx tooling."""

PhpInfo = provider(
    doc = "PHP sources a target makes loadable, staged by staged_path() below one autoload root.",
    fields = {
        "autoload_files": "Depset of sources Composer requires on every run (its `files` autoload).",
        "package_files": "Depset of every file of the Composer packages of the target and its deps, manifests included.",
        "packages": "Depset of Composer package structs: name, version, manifest (File) and root (exec path of the package root).",
        "transitive_srcs": "Depset of every PHP source of the target and its deps.",
    },
)
