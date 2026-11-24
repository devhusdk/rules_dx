"""The Ruby gem hub: one Bundler tree built from the dx lock of one gem set."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("//core:pins.bzl", "lock_problems")
load("//core/modules:langs.bzl", "LANGS")

_BUNDLER = DATA["dx_ruby"]["archives"]["bundler"]

_GEMFILE = [hub for hub in LANGS["ruby"]["setup"]["hubs"] if hub["ecosystem"] == "ruby"][0]["manifest"]

_REMOTE = "https://rubygems.org/"

_CACHE = "vendor/cache"

_BINSTUBS = "bin/private"

# On Windows, Bundler.setup adds x64-mingw-ucrt to a ruby-only lock and tries to rewrite the read-only lock.
_RUNTIME_ENV = {"BUNDLE_FROZEN": "true"}

def _gemfile(packages):
    lines = ["source \"%s\"" % _REMOTE.rstrip("/"), ""]
    for package in packages:
        lines.append("gem \"%s\", \"= %s\"" % (package["name"], package["version"]))
    return "\n".join(lines) + "\n"

def _gemfile_lock(packages):
    lines = ["GEM", "  remote: " + _REMOTE, "  specs:"]
    for package in packages:
        lines.append("    %s (%s)" % (package["name"], package["version"]))
        deps = package.get("extra", {}).get("gem_deps", {})
        for dep in sorted(deps):
            lines.append("      %s (%s)" % (dep, deps[dep]) if deps[dep] else "      " + dep)
    lines += ["", "PLATFORMS", "  ruby", "", "DEPENDENCIES"]
    for package in packages:
        lines.append("  %s (= %s)" % (package["name"], package["version"]))
    lines += ["", "CHECKSUMS"]
    for package in packages:
        lines.append("  %s (%s) sha256=%s" % (package["name"], package["version"], package["archives"][0]["sha256"]))
    lines += ["", "BUNDLED WITH", "   " + _BUNDLER["version"], ""]
    return "\n".join(lines)

def _executables(rctx, full_name):
    unpacked = "_unpacked/" + full_name
    rctx.extract("%s/%s.gem" % (_CACHE, full_name), output = unpacked, type = "tar")
    data = rctx.path(unpacked + "/data.tar.gz")
    found = []
    if data.exists:
        rctx.extract(data, output = unpacked + "/data", type = "tar.gz")
        for directory in ["bin", "exe"]:
            child = rctx.path(unpacked + "/data/" + directory)
            if child.exists:
                found += [executable.basename for executable in child.readdir()]
    return found

def _ruby_gems_impl(rctx):
    lock = json.decode(rctx.attr.lock)
    packages = sorted(lock["packages"], key = lambda package: package["name"])
    name = rctx.original_name
    gems = []
    fragments = []
    executables = []
    for package in packages:
        if len(package.get("archives", [])) != 1:
            fail("gem %s needs exactly one archive in the lock" % package["name"])
        archive = package["archives"][0]
        full_name = "%s-%s" % (package["name"], package["version"])
        rctx.download(url = archive["urls"], sha256 = archive["sha256"], output = "%s/%s.gem" % (_CACHE, full_name))
        executables += [executable for executable in _executables(rctx, full_name) if executable not in executables]
        gems.append(":" + full_name)
        fragments.append("rb_gem(\n    name = %r,\n    gem = %r,\n)\n" % (full_name, "%s/%s.gem" % (_CACHE, full_name)))
    rctx.delete("_unpacked")
    bundler = "bundler-" + _BUNDLER["version"]
    rctx.download(url = _BUNDLER["urls"], sha256 = _BUNDLER["sha256"], output = "%s/%s.gem" % (_CACHE, bundler))
    gems.append(":" + bundler)
    fragments.append("rb_gem_install(\n    name = %r,\n    gem = %r,\n)\n" % (bundler, "%s/%s.gem" % (_CACHE, bundler)))
    rctx.file(_GEMFILE, _gemfile(packages))
    rctx.file(_GEMFILE + ".lock", _gemfile_lock(packages))
    rctx.file("BUILD.bazel", "\n".join([
        "load(\"@rules_ruby//ruby:defs.bzl\", \"rb_bundle_install\", \"rb_gem\", \"rb_gem_install\")",
        "",
        "package(default_visibility = [\"//visibility:public\"])",
        "",
        "rb_bundle_install(\n    name = %r,\n    gemfile = %r,\n    gemfile_lock = %r,\n    gems = %r,\n    env = %r,\n)\n" % (name, _GEMFILE, _GEMFILE + ".lock", gems, _RUNTIME_ENV),
    ] + fragments))
    for package in packages:
        rctx.file("%s/BUILD.bazel" % package["name"], "\n".join([
            "package(default_visibility = [\"//visibility:public\"])",
            "",
            "alias(\n    name = %r,\n    actual = %r,\n)\n" % (package["name"], "//:" + name),
        ]))
    binaries = []
    for executable in sorted(executables):
        rctx.file("%s/%s" % (_BINSTUBS, executable))
        binaries.append("rb_binary(\n    name = %r,\n    main = %r,\n    deps = [%r],\n)\n" % (executable, "private/" + executable, "//:" + name))
    rctx.file("bin/BUILD.bazel", "\n".join([
        "load(\"@rules_ruby//ruby:defs.bzl\", \"rb_binary\")",
        "",
        "package(default_visibility = [\"//visibility:public\"])",
        "",
    ] + binaries))
    return rctx.repo_metadata(reproducible = True)

ruby_gems = repository_rule(
    implementation = _ruby_gems_impl,
    doc = "Downloads each gem of a dx Ruby lock by its sha256 and the pinned Bundler, and installs them as one `rb_bundle_install` tree with an alias per gem and `bin` shims for gem executables.",
    attrs = {
        "lock": attr.string(mandatory = True, doc = "The decoded dx Ruby lock as JSON text."),
    },
)

def ruby_gems_repos(mctx, tag):
    """Creates the gem hub of one `lock` tag and returns its name."""
    text = mctx.read(tag.lock)
    problems = lock_problems(json.decode(text))
    if problems:
        fail("%s %s" % (tag.lock, "; ".join(problems)))
    ruby_gems(name = tag.name, lock = text)
    return tag.name
