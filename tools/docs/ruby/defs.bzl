"""Ruby API reference shards parsed by the pinned YARD on the registered Ruby toolchain."""

load("@rules_ruby//ruby/private:providers.bzl", "RubyFilesInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

_RUBY_TOOLCHAIN = "@rules_ruby//ruby:toolchain_type"

_RubyDirectSrcsInfo = provider(
    doc = "The .rb files one Ruby library lists in its own srcs.",
    fields = {"srcs": "List[File]: the library's direct .rb srcs."},
)

_RubyDocShardInfo = provider(
    doc = "The package and own files of one ruby_doc_shard, for shards that depend on it.",
    fields = {
        "files": "List[File]: the .rb files the shard documents.",
        "package": "Package segment of the shard's symbol ids.",
    },
)

def _direct_srcs_aspect_impl(_target, ctx):
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [_RubyDirectSrcsInfo(srcs = [f for f in srcs if f.extension == "rb"])]

_direct_srcs_aspect = aspect(
    doc = "Reads the direct .rb srcs of a ruby_library, through its public forwarder.",
    implementation = _direct_srcs_aspect_impl,
)

def _where(ctx):
    return "ruby_doc_shard " + str(ctx.label)

def _manifest(ctx, suffix, files):
    manifest = ctx.actions.declare_file(ctx.label.name + suffix)
    lines = [srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in files]
    ctx.actions.write(manifest, "".join(sorted(lines)))
    return manifest

def _yard_lib_dir(ctx):
    for f in ctx.files._yard:
        if f.path.endswith("/lib/yard.rb"):
            return f.dirname
    fail(_where(ctx) + ": @dx_yard holds no lib/yard.rb")

def _ruby_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    own = sorted(lib[_RubyDirectSrcsInfo].srcs, key = lambda f: f.path)
    if not own:
        fail(_where(ctx) + ": lib " + str(lib.label) + " lists no .rb srcs")
    package = ctx.attr.package or lib.label.name
    own_paths = {f.path: True for f in own}
    context = {}
    for f in lib[RubyFilesInfo].transitive_srcs.to_list():
        if f.extension == "rb" and f.path not in own_paths:
            context[f.path] = f
    dep_lines = []
    for dep in ctx.attr.deps:
        info = dep[_RubyDocShardInfo]
        if info.package == package:
            fail(_where(ctx) + ": dep " + str(dep.label) + " uses the same package '" + package + "'")
        for f in info.files:
            if f.path in own_paths:
                continue
            context[f.path] = f
            dep_lines.append(info.package + "\t" + f.path + "\n")
    context_files = [context[path] for path in sorted(context.keys())]

    own_manifest = _manifest(ctx, ".srcs.tsv", own)
    context_manifest = _manifest(ctx, ".context.tsv", context_files)
    deps_manifest = ctx.actions.declare_file(ctx.label.name + ".deps.tsv")
    ctx.actions.write(deps_manifest, "".join(sorted(dep_lines)))

    toolchain = ctx.toolchains[_RUBY_TOOLCHAIN]
    yard_json = ctx.actions.declare_file(ctx.label.name + ".yard.json")
    home = ctx.actions.declare_directory(ctx.label.name + ".yard_home")
    driver = ctx.actions.args()
    driver.add("--disable-gems")
    driver.add("-I", _yard_lib_dir(ctx))
    driver.add(ctx.file._driver)
    driver.add("--own", own_manifest)
    driver.add("--srcs", context_manifest)
    driver.add("--deps", deps_manifest)
    driver.add("--scratch", home.path)
    driver.add("--out", yard_json)
    env = dict(toolchain.env)
    env.update({
        "GEM_HOME": "",
        "GEM_PATH": "",
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "RUBYLIB": "",
        "RUBYOPT": "",
    })
    ctx.actions.run(
        executable = toolchain.ruby,
        arguments = [driver],
        inputs = depset(
            own + context_files + [own_manifest, context_manifest, deps_manifest, ctx.file._driver],
            transitive = [depset(ctx.files._yard)],
        ),
        tools = toolchain.files,
        outputs = [yard_json, home],
        mnemonic = "DxDocsYard",
        progress_message = "Reading Ruby API docs for %{label}",
        use_default_shell_env = False,
        env = env,
    )

    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--json", yard_json)
    extract.add("--package", package)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = ctx.executable._extract,
        arguments = [extract],
        inputs = [yard_json],
        outputs = [shard],
        mnemonic = "DxDocsRubyExtract",
        progress_message = "Extracting Ruby API docs for %{label}",
    )
    return [
        DefaultInfo(files = depset([shard])),
        OutputGroupInfo(
            srcs_manifest = depset([own_manifest]),
            yard_json = depset([yard_json]),
        ),
        DocsShardInfo(language = "ruby", package = package, shard = shard),
        _RubyDocShardInfo(files = own, package = package),
    ]

docs_ruby_shard = rule(
    doc = "Converts one Ruby library's public API into a DocIr shard with YARD.",
    implementation = _ruby_doc_shard_impl,
    attrs = {
        "deps": attr.label_list(
            providers = [_RubyDocShardInfo],
            doc = "ruby_doc_shard targets of libraries this one includes or subclasses from.",
        ),
        "lib": attr.label(
            mandatory = True,
            providers = [RubyFilesInfo],
            aspects = [_direct_srcs_aspect],
            doc = "The documented ruby_library; its direct .rb srcs are documented.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of lib."),
        "_driver": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/ruby:yard_dump.rb"),
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/ruby/extract:ruby_extract"),
            executable = True,
        ),
        "_yard": attr.label(
            default = Label("@dx_yard//:lib"),
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [_RUBY_TOOLCHAIN],
)

def ruby_doc_shard(name, lib, package = "", deps = [], **kwargs):
    """Declares the DocIr shard of one Ruby library.

    Args:
      name: Name of the shard target.
      lib: The documented ruby_library.
      package: Package segment of every symbol id; defaults to the name of lib.
      deps: ruby_doc_shard targets of libraries whose modules this one includes or extends.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_ruby_shard(
        name = name,
        lib = lib,
        package = package,
        deps = deps,
        **kwargs
    )

def _ruby_doc_runtime_impl(ctx):
    toolchain = ctx.toolchains[_RUBY_TOOLCHAIN]
    return [DefaultInfo(
        files = depset([toolchain.ruby]),
        runfiles = ctx.runfiles(files = [toolchain.ruby] + list(toolchain.files)),
    )]

ruby_doc_runtime = rule(
    doc = "Exposes the Ruby interpreter ruby_doc_shard runs YARD with, plus its files, as test data.",
    implementation = _ruby_doc_runtime_impl,
    toolchains = [_RUBY_TOOLCHAIN],
)
