"""Generated from every <lang>/lang.json setup.hubs entry with a lock."""

LOCK_REGISTRY = {
    "cabal": {
        "hub": "hackage",
        "imports": "name",
        "lock": "haskell_deps.json",
        "manifest": "cabal.project",
    },
    "cargo": {
        "hub": "crates",
        "imports": "name",
        "lock": "Cargo.lock",
        "manifest": "Cargo.toml",
    },
    "composer": {
        "hub": "composer",
        "imports": "provides",
        "lock": "composer.lock.json",
        "manifest": "composer.json",
    },
    "cpan": {
        "hub": "cpan",
        "imports": "name",
        "lock": "cpanfile.snapshot.lock.json",
        "manifest": "cpanfile",
    },
    "dub": {
        "hub": "dub",
        "imports": "provides",
        "lock": "dub.lock.json",
        "manifest": "dub.json",
    },
    "fpm": {
        "hub": "fpm",
        "imports": "name",
        "lock": "fpm.lock.toml",
        "manifest": "fpm.toml",
    },
    "go": {
        "hub": "*",
        "imports": "name",
        "lock": "go.sum",
        "manifest": "go.mod",
    },
    "hex": {
        "hub": "hex",
        "imports": "name",
        "lock": "mix.lock",
        "manifest": "mix.exs",
    },
    "julia": {
        "hub": "julia",
        "imports": "name",
        "lock": "Manifest.bazel.json",
        "manifest": "Project.toml",
    },
    "luarocks": {
        "hub": "rocks",
        "imports": "provides",
        "lock": "luarocks.lock.json",
        "manifest": "*.rockspec",
    },
    "maven": {
        "hub": "maven",
        "imports": "provides",
        "lock": "maven_install.json",
        "manifest": "MODULE.bazel",
    },
    "npm": {
        "hub": "npm",
        "imports": "name",
        "lock": "pnpm-lock.yaml",
        "manifest": "package.json",
    },
    "opam": {
        "hub": "opam",
        "imports": "name",
        "lock": "*.opam.locked",
        "manifest": "*.opam",
    },
    "powershell": {
        "hub": "psgallery",
        "imports": "name",
        "lock": "PSGallery.lock.json",
        "manifest": "PSGallery.requirements.psd1",
    },
    "pub": {
        "hub": "pub_deps",
        "imports": "name",
        "lock": "pubspec.lock",
        "manifest": "pubspec.yaml",
    },
    "ruby": {
        "hub": "bundle",
        "imports": "name",
        "lock": "Gemfile.lock",
        "manifest": "Gemfile",
    },
    "swiftpm": {
        "hub": "swiftpkg",
        "imports": "provides",
        "lock": "swiftpm.lock.json",
        "manifest": "Package.swift",
    },
    "uv": {
        "hub": "pypi",
        "imports": "name",
        "lock": "uv.lock",
        "manifest": "pyproject.toml",
    },
    "zig": {
        "hub": "zig",
        "imports": "provides",
        "lock": "zig.lock.json",
        "manifest": "build.zig.zon",
    },
}
