"""The `scala` extension: the rules_scala toolchain and compiler repositories."""

load("@rules_scala//scala:toolchains.bzl", "scala_toolchains")

def _scala_impl(mctx):
    scala_toolchains(
        fetch_sources = True,
        scala = True,
        scalatest = True,
    )
    return mctx.extension_metadata(reproducible = True)

scala = module_extension(
    implementation = _scala_impl,
    doc = "Declares the rules_scala Scala and ScalaTest toolchain repositories.",
)
